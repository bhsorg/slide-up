//! Offline caption model lifecycle: download, hardware-aware engine init and removal.
//!
//! All disk I/O and model initialization happen off the UI thread. The ONNX
//! Runtime session lives behind a `Mutex` so a single engine is shared by the app.

use crate::error::{AppError, ErrorKind, Result};
use crate::events;
use crate::onnx_caption_engine::{CaptionDevice, OnnxCaptionEngine};
use futures_util::StreamExt;
use serde::Serialize;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::AsyncWriteExt;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

const MODEL_BASE_URL: &str = "https://huggingface.co/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main";
const MODEL_FILE: &str = "model.int8.onnx";
const TOKENS_FILE: &str = "tokens.txt";
const VAD_FILE: &str = "silero_vad.onnx";
const VAD_URL: &str = "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx";
const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);
const BYTES_PER_MB: f64 = 1024.0 * 1024.0;

/// Fallback sizes (bytes) when a HEAD request cannot report Content-Length.
const MODEL_SIZE_FALLBACK: u64 = 239_233_841;
const TOKENS_SIZE_FALLBACK: u64 = 315_894;

/// Process-wide ONNX Runtime API initialization. Once committed, every session
/// shares the same environment, so this must only happen once per process.
static ORT_INITIALIZED: OnceLock<std::result::Result<(), String>> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub is_downloaded: bool,
    pub is_loaded: bool,
    pub active_provider: String,
    pub model_size_mb: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub percentage: f32,
    pub downloaded_mb: f32,
    pub total_mb: f32,
    pub status: String,
}

#[derive(Clone)]
pub struct ModelManager {
    app: AppHandle,
    model_dir: PathBuf,
    model_path: PathBuf,
    tokens_path: PathBuf,
    engine: Arc<Mutex<Option<OnnxCaptionEngine>>>,
    device: Arc<Mutex<CaptionDevice>>,
    active_provider: Arc<Mutex<String>>,
    cancellation: Arc<Mutex<Option<CancellationToken>>>,
    downloading: Arc<Mutex<bool>>,
}

impl ModelManager {
    pub fn new(app: AppHandle, data_dir: &Path, device: CaptionDevice) -> Result<Self> {
        let model_dir = data_dir.join("models").join("sense-voice");
        std::fs::create_dir_all(&model_dir)?;
        Ok(Self {
            app,
            model_path: model_dir.join(MODEL_FILE),
            tokens_path: model_dir.join(TOKENS_FILE),
            model_dir,
            engine: Arc::new(Mutex::new(None)),
            device: Arc::new(Mutex::new(device)),
            active_provider: Arc::new(Mutex::new("Not loaded".to_string())),
            cancellation: Arc::new(Mutex::new(None)),
            downloading: Arc::new(Mutex::new(false)),
        })
    }

    fn tmp_path(final_path: &Path) -> PathBuf {
        let mut name = final_path.file_name().unwrap_or_default().to_os_string();
        name.push(".tmp");
        final_path.with_file_name(name)
    }

    fn is_ready_file(path: &Path) -> bool {
        std::fs::metadata(path)
            .map(|meta| meta.is_file() && meta.len() > 0)
            .unwrap_or(false)
    }

    pub fn model_files_ready(&self) -> bool {
        Self::is_ready_file(&self.model_path)
            && Self::is_ready_file(&self.tokens_path)
            && Self::is_ready_file(&self.model_dir.join(VAD_FILE))
    }

    pub fn status(&self) -> ModelStatus {
        let is_downloaded = self.model_files_ready();
        let is_loaded = self.engine.lock().map(|guard| guard.is_some()).unwrap_or(false);
        let active_provider = self.active_provider
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_else(|_| "Not loaded".to_string());
        let model_size_mb: u64 = [&self.model_path, &self.tokens_path, &self.model_dir.join(VAD_FILE)]
            .iter()
            .filter_map(|path| std::fs::metadata(path).ok().map(|meta| meta.len()))
            .sum::<u64>()
            .div_ceil(1024 * 1024);
        ModelStatus {
            is_downloaded,
            is_loaded,
            active_provider,
            model_size_mb,
        }
    }

    fn emit_progress(&self, downloaded: u64, total: u64, status: &str) {
        let percentage = if total == 0 {
            0.0
        } else {
            ((downloaded as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as f32
        };
        let payload = DownloadProgress {
            percentage,
            downloaded_mb: (downloaded as f64 / BYTES_PER_MB) as f32,
            total_mb: (total as f64 / BYTES_PER_MB) as f32,
            status: status.to_string(),
        };
        let _ = self.app.emit(events::MODEL_DOWNLOAD_PROGRESS, payload);
    }

    /// All files that make up a usable caption engine.
    const MODEL_FILES: [&'static str; 3] = [VAD_FILE, TOKENS_FILE, MODEL_FILE];

    fn remote_url(file: &str) -> String {
        if file == VAD_FILE {
            VAD_URL.to_string()
        } else {
            format!("{MODEL_BASE_URL}/{file}")
        }
    }

    fn size_fallback(file: &str) -> u64 {
        match file {
            MODEL_FILE => MODEL_SIZE_FALLBACK,
            TOKENS_FILE => TOKENS_SIZE_FALLBACK,
            _ => 2_500_000, // silero_vad.onnx
        }
    }

    /// Probe all files so the progress bar covers the whole download.
    async fn probe_total(client: &reqwest::Client) -> u64 {
        let mut total = 0;
        for file in Self::MODEL_FILES {
            let url = Self::remote_url(file);
            let length = client
                .head(&url)
                .send()
                .await
                .ok()
                .and_then(|response| response.content_length());
            total += length.unwrap_or_else(|| Self::size_fallback(file));
        }
        total
    }

    async fn download_file(
        &self,
        client: &reqwest::Client,
        file: &str,
        token: &CancellationToken,
        total: u64,
        already_downloaded: u64,
        last_emit: &mut Instant,
    ) -> Result<u64> {
        let final_path = self.model_dir.join(file);
        let tmp_path = Self::tmp_path(&final_path);
        let resume_from = tokio::fs::metadata(&tmp_path)
            .await
            .map(|meta| meta.len())
            .unwrap_or(0);

        let mut request = client.get(Self::remote_url(file));
        if resume_from > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={resume_from}-"));
        }
        let response = request.send().await.map_err(|error| {
            AppError::new(
                ErrorKind::Native,
                format!("Could not reach the model server: {error}"),
            )
        })?;
        let status = response.status();
        if !status.is_success() {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(AppError::new(
                ErrorKind::Native,
                format!("The model server returned {status}."),
            ));
        }
        let append = status == reqwest::StatusCode::PARTIAL_CONTENT && resume_from > 0;
        let file_total = response.content_length().unwrap_or(0)
            + if append { resume_from } else { 0 };

        let mut file_handle = if append {
            tokio::fs::OpenOptions::new().append(true).open(&tmp_path).await?
        } else {
            tokio::fs::File::create(&tmp_path).await?
        };
        let mut written = if append { resume_from } else { 0 };
        let mut stream = response.bytes_stream();

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    let _ = file_handle.flush().await;
                    let _ = file_handle.sync_all().await;
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    return Err(AppError::new(ErrorKind::Native, "Download cancelled."));
                }
                chunk = stream.next() => {
                    let Some(chunk) = chunk else { break };
                    let chunk = chunk.map_err(|error| {
                        AppError::new(ErrorKind::Native, format!("Download interrupted: {error}"))
                    })?;
                    file_handle.write_all(&chunk).await?;
                    written += chunk.len() as u64;
                }
            }

            if last_emit.elapsed() >= PROGRESS_INTERVAL {
                self.emit_progress(already_downloaded + written, total, "downloading");
                *last_emit = Instant::now();
            }
        }

        file_handle.flush().await?;
        file_handle.sync_all().await?;
        drop(file_handle);
        // Atomic publish: a partial file is never visible under its final name.
        tokio::fs::rename(&tmp_path, &final_path).await?;
        let _ = file_total;
        Ok(written)
    }

    async fn download_inner(&self, token: CancellationToken) -> Result<()> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .build()
            .map_err(|error| {
                AppError::new(ErrorKind::Native, format!("Could not create HTTP client: {error}"))
            })?;
        let total = Self::probe_total(&client).await.max(1);
        let mut last_emit = Instant::now() - PROGRESS_INTERVAL;
        self.emit_progress(0, total, "downloading");

        let mut downloaded = 0;
        for file in Self::MODEL_FILES {
            downloaded += self
                .download_file(&client, file, &token, total, downloaded, &mut last_emit)
                .await?;
        }
        Ok(())
    }

    pub async fn download(&self) -> Result<()> {
        {
            let mut guard = self.downloading.lock().map_err(|_| {
                AppError::new(ErrorKind::Native, "Caption manager is unavailable.")
            })?;
            if *guard {
                return Err(AppError::new(
                    ErrorKind::Native,
                    "A caption model download is already running.",
                ));
            }
            *guard = true;
        }

        let token = CancellationToken::new();
        {
            let mut guard = self.cancellation.lock().map_err(|_| {
                AppError::new(ErrorKind::Native, "Caption manager is unavailable.")
            })?;
            *guard = Some(token.clone());
        }

        let result = self.download_inner(token.clone()).await;
        let was_cancelled = token.is_cancelled();

        if result.is_err() {
            for file in Self::MODEL_FILES {
                let path = self.model_dir.join(file);
                let _ = tokio::fs::remove_file(Self::tmp_path(&path)).await;
            }
            self.emit_progress(0, 0, if was_cancelled { "cancelled" } else { "error" });
        } else {
            // The model files are ready, but the ONNX engine stays unloaded
            // until the user explicitly asks for it (Settings → Live captions).
            // Loading 230 MB of weights and DirectML at startup or after a
            // download made the whole app and OS stutter.
            let total: u64 = [&self.model_path, &self.tokens_path]
                .iter()
                .filter_map(|path| std::fs::metadata(path).ok().map(|meta| meta.len()))
                .sum();
            self.emit_progress(total, total, "completed");
        }

        {
            let mut guard = self.downloading.lock().map_err(|_| {
                AppError::new(ErrorKind::Native, "Caption manager is unavailable.")
            })?;
            *guard = false;
        }
        {
            let mut guard = self.cancellation.lock().map_err(|_| {
                AppError::new(ErrorKind::Native, "Caption manager is unavailable.")
            })?;
            *guard = None;
        }
        result
    }

    pub fn cancel(&self) {
        if let Ok(guard) = self.cancellation.lock() {
            if let Some(token) = guard.as_ref() {
                token.cancel();
            }
        }
    }

    /// Locate the ONNX Runtime shared library shipped with the app (or one the
    /// user supplied through `ORT_DYLIB_PATH`).
    fn find_ort_library(&self) -> Option<PathBuf> {
        let file = if cfg!(windows) {
            "onnxruntime.dll"
        } else if cfg!(target_os = "macos") {
            "libonnxruntime.dylib"
        } else {
            "libonnxruntime.so"
        };

        let mut candidates = Vec::new();
        if let Ok(path) = std::env::var("ORT_DYLIB_PATH") {
            candidates.push(PathBuf::from(path));
        }
        if let Ok(resource) = self.app.path().resolve(format!("binaries/{file}"), BaseDirectory::Resource) {
            candidates.push(resource);
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("binaries").join(file));
                candidates.push(dir.join(file));
            }
        }
        // Development builds run from the source tree rather than the bundle.
        candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries").join(file));

        candidates.into_iter().find(|path| path.is_file())
    }

    /// Initialize the process-wide ONNX Runtime API exactly once. Loading the
    /// native library can panic inside ort's C bindings, so both calls are
    /// guarded with `catch_unwind` and the outcome is cached for the process.
    fn ensure_ort_initialized(&self) -> Result<()> {
        let result = ORT_INITIALIZED.get_or_init(|| {
            let library = self.find_ort_library().ok_or_else(|| {
                "ONNX Runtime could not be found. Expected binaries/onnxruntime.dll next to the app."
                    .to_string()
            })?;

            if let Some(dir) = library.parent() {
                let mut paths = std::env::var_os("PATH")
                    .map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
                    .unwrap_or_default();
                paths.insert(0, dir.to_path_buf());
                if let Ok(joined) = std::env::join_paths(paths) {
                    // SAFETY: initialization runs once, before any session is created.
                    unsafe { std::env::set_var("PATH", joined) };
                }
            }
            // SAFETY: initialization runs once, before any session is created.
            unsafe { std::env::set_var("ORT_DYLIB_PATH", &library) };

            let builder = match catch_unwind(|| ort::init_from(&library)) {
                Ok(Ok(builder)) => builder,
                Ok(Err(error)) => {
                    return Err(format!(
                        "ONNX Runtime failed to load from {}: {error}",
                        library.display()
                    ));
                }
                Err(_) => {
                    return Err(format!(
                        "ONNX Runtime failed to load from {}.",
                        library.display()
                    ));
                }
            };
            match catch_unwind(AssertUnwindSafe(|| builder.commit())) {
                Ok(true) => {
                    tracing::info!("ONNX Runtime initialized from {}", library.display());
                    Ok(())
                }
                // `false` means an environment was already configured; reusing it is fine.
                Ok(false) => Ok(()),
                Err(_) => Err("ONNX Runtime initialization panicked.".to_string()),
            }
        });

        result
            .clone()
            .map_err(|message| AppError::new(ErrorKind::Native, message))
    }

    /// Shared ONNX Runtime initialization for other inference features.
    pub fn ensure_ort(&self) -> Result<()> {
        self.ensure_ort_initialized()
    }

    /// Initialize the caption engine using the configured device preference.
    /// `Auto` prefers the platform's GPU provider with a CPU fallback, while
    /// `Cpu` / `Gpu` force one and report a clear error if it is unavailable.
    pub fn initialize_engine(&self) -> Result<String> {
        if !self.model_files_ready() {
            return Err(AppError::new(
                ErrorKind::Native,
                "The caption model files are missing. Download the model first.",
            ));
        }
        self.ensure_ort_initialized()?;

        let device = *self.device.lock().map_err(|_| {
            AppError::new(ErrorKind::Native, "Caption device preference is unavailable.")
        })?;

        // ONNX Runtime's C bindings can panic on provider setup, so catch that
        // here and surface a user-facing error instead.
        let engine = catch_unwind(AssertUnwindSafe(|| OnnxCaptionEngine::new(&self.model_dir, device)))
            .map_err(|_| {
                AppError::new(
                    ErrorKind::Native,
                    "ONNX Runtime panicked while loading the caption model.",
                )
            })?
            .map_err(|error| {
                AppError::new(
                    ErrorKind::Native,
                    format!("The caption engine could not be initialized: {error}"),
                )
            })?;

        let label = engine.provider_name.clone();
        *self.engine.lock().map_err(|_| {
            AppError::new(ErrorKind::Native, "Caption engine lock is unavailable.")
        })? = Some(engine);
        *self.active_provider.lock().map_err(|_| {
            AppError::new(ErrorKind::Native, "Caption engine lock is unavailable.")
        })? = label.clone();
        tracing::info!("Caption engine initialized on {label}");
        Ok(label)
    }

    /// Change the preferred execution provider. If a session is already live it
    /// is rebuilt immediately so the hardware badge reflects the new setting.
    pub fn set_device(&self, device: CaptionDevice) -> Result<String> {
        *self.device.lock().map_err(|_| {
            AppError::new(ErrorKind::Native, "Caption device preference is unavailable.")
        })? = device;

        if self.status().is_loaded {
            self.initialize_engine()
        } else {
            Ok("Not loaded".to_string())
        }
    }

    /// Drop the live ONNX session so its weights and GPU allocations are freed.
    /// The model files stay on disk and can be loaded again at any time.
    pub fn unload_engine(&self) -> Result<()> {
        *self.engine.lock().map_err(|_| {
            AppError::new(ErrorKind::Native, "Caption engine lock is unavailable.")
        })? = None;
        *self.active_provider.lock().map_err(|_| {
            AppError::new(ErrorKind::Native, "Caption engine lock is unavailable.")
        })? = "Not loaded".to_string();
        tracing::info!("Caption engine unloaded");
        Ok(())
    }

    pub fn delete(&self) -> Result<()> {
        self.cancel();
        *self.engine.lock().map_err(|_| {
            AppError::new(ErrorKind::Native, "Caption engine lock is unavailable.")
        })? = None;
        *self.active_provider.lock().map_err(|_| {
            AppError::new(ErrorKind::Native, "Caption engine lock is unavailable.")
        })? = "Not loaded".to_string();

        for file in Self::MODEL_FILES {
            let final_path = self.model_dir.join(file);
            for path in [final_path, Self::tmp_path(&self.model_dir.join(file))] {
                if path.exists() {
                    std::fs::remove_file(path)?;
                }
            }
        }
        Ok(())
    }
}

#[tauri::command]
pub fn check_model_status(manager: State<'_, ModelManager>) -> ModelStatus {
    manager.status()
}

#[tauri::command]
pub async fn download_caption_model(app: AppHandle, manager: State<'_, ModelManager>) -> Result<()> {
    let _ = app; // The manager already holds the handle used for progress events.
    let manager = manager.inner().clone();
    manager.download().await
}

#[tauri::command]
pub fn cancel_model_download(manager: State<'_, ModelManager>) -> Result<()> {
    manager.cancel();
    Ok(())
}

#[tauri::command]
pub async fn load_caption_engine(manager: State<'_, ModelManager>) -> Result<String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.initialize_engine())
        .await
        .map_err(|error| {
            AppError::new(ErrorKind::Native, format!("Engine initialization failed: {error}"))
        })?
}

#[tauri::command]
pub fn unload_caption_engine(manager: State<'_, ModelManager>) -> Result<()> {
    manager.unload_engine()
}

#[tauri::command]
pub fn delete_caption_model(manager: State<'_, ModelManager>) -> Result<()> {
    manager.delete()
}

#[tauri::command]
pub async fn set_caption_device(manager: State<'_, ModelManager>, device: String) -> Result<String> {
    let manager = manager.inner().clone();
    tauri::async_runtime::spawn_blocking(move || manager.set_device(CaptionDevice::parse(&device)))
        .await
        .map_err(|error| {
            AppError::new(ErrorKind::Native, format!("Caption device change failed: {error}"))
        })?
}
