//! Downloadable AI upscaler models for video playback.
//!
//! This is intentionally separate from the caption model manager: upscaler
//! files are small, are never loaded into an ONNX session here, and only one
//! download may run at a time across the whole catalog.

use crate::error::{AppError, ErrorKind, Result};
use crate::events;
use crate::model_manager::DownloadProgress;
use futures_util::StreamExt;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use tokio::io::AsyncWriteExt;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

const REAL_ESRGAN_URL: &str = "https://huggingface.co/CoderViking/realesr-general-x4v3-onnx/resolve/main/realesr-general-x4v3.onnx";
const FSRCNNX_URL: &str = "https://github.com/igv/FSRCNN-TensorFlow/releases/download/1.1/FSRCNNX_x2_8-0-4-1.glsl";
const SPAN_X2_URL: &str = "https://huggingface.co/huggingworld/onnx-image-models/resolve/main/2x-spanx2-ch48.onnx";
const SPAN_X4_URL: &str = "https://huggingface.co/huggingworld/onnx-image-models/resolve/main/4xPurePhoto-Span.onnx";
const ESPCN_X2_URL: &str = "https://github.com/onnx/models/raw/main/validated/vision/super_resolution/sub_pixel_cnn_2016/model/super-resolution-10.onnx";
const ESPCN_X4_URL: &str = "https://huggingface.co/huggingworld/onnx-image-models/resolve/main/4x-ClearRealityV1.onnx";

const BYTES_PER_MB: f64 = 1024.0 * 1024.0;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

/// Catalog of upscaler models.
pub struct Definition {
    pub id: &'static str,
    pub name: &'static str,
    pub file_name: &'static str,
    pub size_bytes: u64,
    pub speed: &'static str,
    pub description: &'static str,
    pub url: Option<&'static str>,
    /// `"model"` for ONNX files, `"shader"` for mpv GLSL shaders.
    pub kind: &'static str,
}

const DEFINITIONS: &[Definition] = &[
    Definition {
        id: "span-x2",
        name: "SPAN x2",
        file_name: "spanx2_ch48.onnx",
        size_bytes: 1_659_067,
        speed: "3–8 ms GPU / 25 ms CPU",
        description: "Fast real-time upscaler (NTIRE winner, no heavy attention layers).",
        url: Some(SPAN_X2_URL),
        kind: "model",
    },
    Definition {
        id: "span-x4",
        name: "SPAN x4",
        file_name: "spanx4.onnx",
        size_bytes: 1_721_421,
        speed: "3–8 ms GPU / 25 ms CPU",
        description: "Fast real-time 4x upscaler (NTIRE winner).",
        url: Some(SPAN_X4_URL),
        kind: "model",
    },
    Definition {
        id: "realesr-general-x4v3",
        name: "Real-ESRGAN Compact",
        file_name: "realesr-general-x4v3.onnx",
        size_bytes: 4_866_417,
        speed: "8–15 ms DirectML / CoreML",
        description: "Removes noise and pixelation from old, blurry or compressed videos.",
        url: Some(REAL_ESRGAN_URL),
        kind: "model",
    },
    Definition {
        id: "espcn-x2",
        name: "ESPCN x2",
        file_name: "ESPCN_x2.onnx",
        size_bytes: 240_078,
        speed: "< 2 ms on CPU",
        description: "Instant edge sharpening and 2x upscaling with minimal resources.",
        url: Some(ESPCN_X2_URL),
        kind: "model",
    },
    Definition {
        id: "espcn-x4",
        name: "ClearReality 4x",
        file_name: "ESPCN_x4.onnx",
        size_bytes: 1_721_421,
        speed: "< 5 ms GPU / 30 ms CPU",
        description: "Instant edge sharpening and 4x upscaling with minimal resources.",
        url: Some(ESPCN_X4_URL),
        kind: "model",
    },
    Definition {
        id: "fsrcnnx-x2",
        name: "FSRCNNX x2",
        file_name: "FSRCNNX_x2_8-0-4-1.glsl",
        size_bytes: 71_296,
        speed: "GPU shader (mpv)",
        description: "Lightweight FSRCNN-based GLSL shader for real-time 2x upscaling.",
        url: Some(FSRCNNX_URL),
        kind: "shader",
    },
];

pub fn definition(id: &str) -> Result<&'static Definition> {
    DEFINITIONS
        .iter()
        .find(|model| model.id == id)
        .ok_or_else(|| AppError::new(ErrorKind::Native, format!("Unknown upscaler model: {id}")))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscalerModelStatus {
    pub id: String,
    pub name: String,
    pub file_name: String,
    pub size_mb: f32,
    pub speed: String,
    pub description: String,
    pub kind: String,
    pub is_downloaded: bool,
    pub is_available: bool,
    pub size_on_disk_mb: f32,
}

#[derive(Clone)]
pub struct UpscalerManager {
    app: AppHandle,
    model_dir: PathBuf,
    shader_dir: PathBuf,
    cancellation: Arc<Mutex<Option<CancellationToken>>>,
    active_download: Arc<Mutex<Option<String>>>,
}

impl UpscalerManager {
    pub fn new(app: AppHandle, data_dir: &Path) -> Result<Self> {
        let model_dir = data_dir.join("models").join("upscalers");
        let shader_dir = data_dir.join("shaders");
        std::fs::create_dir_all(&model_dir)?;
        std::fs::create_dir_all(&shader_dir)?;
        Ok(Self {
            app,
            model_dir,
            shader_dir,
            cancellation: Arc::new(Mutex::new(None)),
            active_download: Arc::new(Mutex::new(None)),
        })
    }

    fn storage_dir(&self, model: &Definition) -> &Path {
        if model.kind == "shader" {
            &self.shader_dir
        } else {
            &self.model_dir
        }
    }

    fn final_path(&self, model: &Definition) -> PathBuf {
        self.storage_dir(model).join(model.file_name)
    }

    fn tmp_path(&self, model: &Definition) -> PathBuf {
        self.storage_dir(model)
            .join(format!("{}.tmp", model.file_name))
    }

    /// Full path of the first downloaded shader, for the player's AI Enhance toggle.
    pub fn downloaded_shader_path(&self) -> Option<PathBuf> {
        DEFINITIONS
            .iter()
            .find(|model| model.kind == "shader" && model.url.is_some())
            .map(|model| self.final_path(model))
            .filter(|path| path.is_file())
    }

    /// Full path of a downloaded model file, e.g. for offline upscaling.
    pub fn downloaded_model_path(&self, id: &str) -> Option<PathBuf> {
        DEFINITIONS
            .iter()
            .find(|model| model.id == id && model.url.is_some())
            .map(|model| self.final_path(model))
            .filter(|path| path.is_file())
    }

    fn statuses(&self) -> Vec<UpscalerModelStatus> {
        DEFINITIONS
            .iter()
            .map(|model| {
                let path = self.final_path(model);
                let size_on_disk_mb = std::fs::metadata(&path)
                    .ok()
                    .filter(|meta| meta.is_file() && meta.len() > 0)
                    .map(|meta| meta.len() as f64 / BYTES_PER_MB)
                    .unwrap_or(0.0) as f32;
                UpscalerModelStatus {
                    id: model.id.to_string(),
                    name: model.name.to_string(),
                    file_name: model.file_name.to_string(),
                    size_mb: (model.size_bytes as f64 / BYTES_PER_MB) as f32,
                    speed: model.speed.to_string(),
                    description: model.description.to_string(),
                    kind: model.kind.to_string(),
                    is_downloaded: size_on_disk_mb > 0.0,
                    is_available: model.url.is_some(),
                    size_on_disk_mb,
                }
            })
            .collect()
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
        let _ = self.app.emit(events::UPSCALER_DOWNLOAD_PROGRESS, payload);
    }

    async fn download(&self, id: &str) -> Result<()> {
        let model = definition(id)?;
        let url = model.url.ok_or_else(|| {
            AppError::new(
                ErrorKind::Native,
                format!("No verified download source is configured for {}.", model.name),
            )
        })?;

        {
            let mut guard = self.active_download.lock().map_err(|_| {
                AppError::new(ErrorKind::Native, "Upscaler manager is unavailable.")
            })?;
            if guard.is_some() {
                return Err(AppError::new(
                    ErrorKind::Native,
                    "Another upscaler model is already downloading.",
                ));
            }
            *guard = Some(id.to_string());
        }

        let token = CancellationToken::new();
        {
            let mut guard = self.cancellation.lock().map_err(|_| {
                AppError::new(ErrorKind::Native, "Upscaler manager is unavailable.")
            })?;
            *guard = Some(token.clone());
        }

        // Notify connecting state right away so UI displays progress block with 0%
        self.emit_progress(0, model.size_bytes, "connecting");

        let result = self.download_file(model, url, &token, model.size_bytes).await;
        let was_cancelled = token.is_cancelled();

        if let Ok(actual_total) = result {
            self.emit_progress(actual_total, actual_total, "completed");
        } else {
            let _ = tokio::fs::remove_file(self.tmp_path(model)).await;
            self.emit_progress(0, 0, if was_cancelled { "cancelled" } else { "error" });
        }

        {
            let mut guard = self.active_download.lock().map_err(|_| {
                AppError::new(ErrorKind::Native, "Upscaler manager is unavailable.")
            })?;
            *guard = None;
        }
        {
            let mut guard = self.cancellation.lock().map_err(|_| {
                AppError::new(ErrorKind::Native, "Upscaler manager is unavailable.")
            })?;
            *guard = None;
        }
        result.map(|_| ())
    }

    async fn download_file(
        &self,
        model: &Definition,
        url: &str,
        token: &CancellationToken,
        fallback_total: u64,
    ) -> Result<u64> {
        let tmp = self.tmp_path(model);
        let resume_from = tokio::fs::metadata(&tmp)
            .await
            .map(|meta| meta.len())
            .unwrap_or(0);

        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .build()
            .map_err(|error| {
                AppError::new(ErrorKind::Native, format!("Could not create HTTP client: {error}"))
            })?;
        let mut request = client.get(url);
        if resume_from > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={resume_from}-"));
        }
        let response = request.send().await.map_err(|error| {
            AppError::new(
                ErrorKind::Native,
                format!("Could not reach the model server: {error}"),
            )
        })?;
        if !response.status().is_success() {
            return Err(AppError::new(
                ErrorKind::Native,
                format!("The model server returned {}.", response.status()),
            ));
        }
        let append = response.status() == reqwest::StatusCode::PARTIAL_CONTENT && resume_from > 0;
        let content_len = response.content_length().unwrap_or(0);
        let total = if content_len > 0 {
            content_len + if append { resume_from } else { 0 }
        } else {
            fallback_total
        };

        let mut file = if append {
            tokio::fs::OpenOptions::new().append(true).open(&tmp).await?
        } else {
            tokio::fs::File::create(&tmp).await?
        };
        let mut downloaded = if append { resume_from } else { 0 };
        let mut last_emit = Instant::now() - PROGRESS_INTERVAL;
        let mut stream = response.bytes_stream();

        // Emit initial progress with actual total
        self.emit_progress(downloaded, total, "downloading");

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    let _ = file.flush().await;
                    let _ = file.sync_all().await;
                    return Err(AppError::new(ErrorKind::Native, "Download cancelled."));
                }
                chunk = stream.next() => {
                    let Some(chunk) = chunk else { break };
                    let chunk = chunk.map_err(|error| {
                        AppError::new(ErrorKind::Native, format!("Download interrupted: {error}"))
                    })?;
                    file.write_all(&chunk).await?;
                    downloaded += chunk.len() as u64;
                }
            }
            if last_emit.elapsed() >= PROGRESS_INTERVAL {
                self.emit_progress(downloaded, total, "downloading");
                last_emit = Instant::now();
            }
        }

        file.flush().await?;
        file.sync_all().await?;
        drop(file);

        let final_path = self.final_path(model);
        if final_path.exists() {
            let _ = tokio::fs::remove_file(&final_path).await;
        }
        tokio::fs::rename(&tmp, final_path).await?;
        Ok(downloaded)
    }

    fn cancel(&self) {
        if let Ok(guard) = self.cancellation.lock() {
            if let Some(token) = guard.as_ref() {
                token.cancel();
            }
        }
    }

    fn delete(&self, id: &str) -> Result<()> {
        let model = definition(id)?;
        if let Ok(mut guard) = self.active_download.lock() {
            if guard.as_deref() == Some(id) {
                if let Ok(cancellation) = self.cancellation.lock() {
                    if let Some(token) = cancellation.as_ref() {
                        token.cancel();
                    }
                }
            }
            *guard = None;
        }
        for path in [self.final_path(model), self.tmp_path(model)] {
            if path.exists() {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }
}

#[tauri::command]
pub fn list_upscaler_models(manager: State<'_, UpscalerManager>) -> Vec<UpscalerModelStatus> {
    manager.statuses()
}

#[tauri::command]
pub async fn download_upscaler_model(
    id: String,
    manager: State<'_, UpscalerManager>,
) -> Result<()> {
    let manager = manager.inner().clone();
    manager.download(&id).await
}

#[tauri::command]
pub fn cancel_upscaler_download(manager: State<'_, UpscalerManager>) -> Result<()> {
    manager.cancel();
    Ok(())
}

#[tauri::command]
pub fn delete_upscaler_model(id: String, manager: State<'_, UpscalerManager>) -> Result<()> {
    manager.delete(&id)
}
