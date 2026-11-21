use crate::onnx_caption_engine::OnnxCaptionEngine;
use futures_util::StreamExt;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

pub struct CaptionEngineState {
    pub engine: Arc<Mutex<Option<OnnxCaptionEngine>>>,
}

#[derive(Serialize)]
pub struct ModelStatus {
    pub is_downloaded: bool,
    pub is_ready: bool,
    pub provider: String,
}

#[derive(Clone, Serialize)]
struct DownloadProgress {
    file: String,
    percentage: f32,
}

fn get_model_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap().join("models/sense-voice")
}

#[tauri::command]
pub async fn check_model_status(
    app: AppHandle,
    state: State<'_, CaptionEngineState>,
) -> Result<ModelStatus, String> {
    let dir = get_model_dir(&app);
    let is_downloaded = dir.join("model.int8.onnx").exists()
        && dir.join("tokens.txt").exists()
        && dir.join("silero_vad.onnx").exists();

    let guard = state.engine.lock().await;
    let (is_ready, provider) = match &*guard {
        Some(eng) => (true, eng.provider_name.clone()),
        None => (false, "Not Loaded".to_string()),
    };

    Ok(ModelStatus {
        is_downloaded,
        is_ready,
        provider,
    })
}

#[tauri::command]
pub async fn download_and_init_model(
    app: AppHandle,
    state: State<'_, CaptionEngineState>,
) -> Result<ModelStatus, String> {
    let dir = get_model_dir(&app);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;

    let files = [
        (
            "silero_vad.onnx",
            "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx",
        ),
        (
            "tokens.txt",
            "https://huggingface.co/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/tokens.txt",
        ),
        (
            "model.int8.onnx",
            "https://huggingface.co/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/model.int8.onnx",
        ),
    ];

    let client = reqwest::Client::new();

    for (filename, url) in files {
        let dest = dir.join(filename);
        if dest.exists() {
            continue;
        }
        let tmp_dest = dir.join(format!("{}.tmp", filename));

        let res = client.get(url).send().await.map_err(|e| e.to_string())?;
        let total_size = res.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;

        let mut file = tokio::fs::File::create(&tmp_dest)
            .await
            .map_err(|e| e.to_string())?;
        let mut stream = res.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let data = chunk.map_err(|e| e.to_string())?;
            file.write_all(&data).await.map_err(|e| e.to_string())?;
            downloaded += data.len() as u64;

            if total_size > 0 {
                let pct = (downloaded as f32 / total_size as f32) * 100.0;
                let _ = app.emit(
                    "model-download-progress",
                    DownloadProgress {
                        file: filename.to_string(),
                        percentage: pct,
                    },
                );
            }
        }

        file.flush().await.map_err(|e| e.to_string())?;
        tokio::fs::rename(&tmp_dest, &dest)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Download hote hi turant GPU/CPU par engine initialize karein
    let engine_instance = tokio::task::spawn_blocking(move || OnnxCaptionEngine::new(&dir))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    let provider = engine_instance.provider_name.clone();
    *state.engine.lock().await = Some(engine_instance);

    Ok(ModelStatus {
        is_downloaded: true,
        is_ready: true,
        provider,
    })
}