//! Offline video upscaling with the downloaded Real-ESRGAN ONNX model.
//!
//! mpv itself cannot run ONNX models, so this decodes the current video to raw
//! RGB frames with ffmpeg, runs each frame through the model, and re-encodes the
//! result while copying the original audio. ffmpeg/ffprobe are discovered on PATH.

use crate::error::{AppError, ErrorKind, Result};
use crate::events;
use crate::model_manager::ModelManager;
use crate::upscaler_manager::UpscalerManager;
use ort::ep::CPU;
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::Tensor;
use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use tauri::{AppHandle, Emitter, Manager, State};

const MODEL_ID: &str = "realesr-general-x4v3";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscaleProgress {
    pub output_path: String,
    pub processed_frames: u64,
    pub total_frames: u64,
    pub percentage: f32,
    pub status: String,
}

fn native(message: impl Into<String>) -> AppError {
    AppError::new(ErrorKind::Native, message.into())
}

fn find_on_path(file: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(file))
        .find(|candidate| candidate.is_file())
}

fn ffmpeg_command() -> Command {
    let mut command = Command::new("ffmpeg");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Avoid flashing a console window for every spawned process.
        command.creation_flags(0x0800_0000);
    }
    command
}

#[derive(serde::Deserialize)]
struct ProbeResponse {
    streams: Vec<ProbeStream>,
}

#[derive(serde::Deserialize)]
struct ProbeStream {
    width: u64,
    height: u64,
    #[serde(default)]
    nb_read_frames: Option<u64>,
    #[serde(default)]
    avg_frame_rate: Option<String>,
}

fn probe_video(ffprobe: &Path, input: &Path) -> Result<(usize, usize, u64, f64)> {
    let output = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-count_frames",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,nb_read_frames,avg_frame_rate",
            "-of",
            "json",
        ])
        .arg(input)
        .output()
        .map_err(|error| native(format!("Could not inspect the video with ffprobe: {error}")))?;
    if !output.status.success() {
        return Err(native("ffprobe could not read this video."));
    }
    let probe: ProbeResponse = serde_json::from_slice(&output.stdout)
        .map_err(|error| native(format!("Could not parse ffprobe output: {error}")))?;
    let stream = probe
        .streams
        .first()
        .ok_or_else(|| native("The file does not contain a video stream."))?;
    let fps = stream
        .avg_frame_rate
        .as_deref()
        .and_then(|rate| rate.split_once('/'))
        .and_then(|(num, den)| {
            let n = num.parse::<f64>().ok()?;
            let d = den.parse::<f64>().ok()?;
            (d > 0.0).then_some(n / d)
        })
        .filter(|fps| fps.is_finite() && *fps > 0.0)
        .unwrap_or(25.0);
    Ok((
        stream.width as usize,
        stream.height as usize,
        stream.nb_read_frames.unwrap_or(0),
        fps,
    ))
}

fn rgb_to_nchw(rgb: &[u8], height: usize, width: usize) -> Result<Tensor<f32>> {
    let pixels = height * width;
    let mut data = Vec::with_capacity(pixels * 3);
    for channel in 0..3 {
        data.extend(
            (0..pixels).map(|index| rgb[index * 3 + channel] as f32 / 255.0),
        );
    }
    Tensor::from_array((vec![1, 3, height, width], data))
        .map_err(|error| native(format!("Could not create an input tensor: {error}")))
}

fn nchw_to_rgb(data: &[f32], height: usize, width: usize) -> Vec<u8> {
    let pixels = height * width;
    let mut rgb = vec![0u8; pixels * 3];
    for index in 0..pixels {
        for channel in 0..3 {
            let value = data[channel * pixels + index] * 255.0;
            rgb[index * 3 + channel] = value.round().clamp(0.0, 255.0) as u8;
        }
    }
    rgb
}

fn emit(app: &AppHandle, output: &Path, processed: u64, total: u64, status: &str) {
    let percentage = if total == 0 {
        0.0
    } else {
        ((processed as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as f32
    };
    let _ = app.emit(
        events::UPSCALE_PROGRESS,
        UpscaleProgress {
            output_path: output.to_string_lossy().into_owned(),
            processed_frames: processed,
            total_frames: total,
            percentage,
            status: status.to_string(),
        },
    );
}

fn stream_frames(
    app: &AppHandle,
    session: &mut Session,
    input_name: &str,
    output_name: &str,
    decoder: &mut Child,
    encoder: &mut Child,
    width: usize,
    height: usize,
    total_frames: u64,
    output: &Path,
) -> Result<u64> {
    let mut raw = vec![0u8; width * height * 3];
    let decoder_out = decoder
        .stdout
        .as_mut()
        .ok_or_else(|| native("Could not read ffmpeg's decoded frames."))?;
    let encoder_in = encoder
        .stdin
        .as_mut()
        .ok_or_else(|| native("Could not write frames to ffmpeg."))?;
    let mut processed = 0u64;

    loop {
        match decoder_out.read_exact(&mut raw) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(native(format!("Frame decode failed: {error}"))),
        }

        let tensor = rgb_to_nchw(&raw, height, width)?;
        let outputs = session
            .run(ort::inputs![input_name => tensor])
            .map_err(|error| native(format!("Real-ESRGAN inference failed: {error}")))?;
        let result = outputs
            .get(output_name)
            .ok_or_else(|| native("The ONNX model returned an unexpected output."))?;
        let (shape, values) = result
            .try_extract_tensor::<f32>()
            .map_err(|error| native(format!("Could not read the ONNX output: {error}")))?;
        if shape.len() != 4 {
            return Err(native("The ONNX model returned an invalid image shape."));
        }
        let out_height = shape[2] as usize;
        let out_width = shape[3] as usize;
        encoder_in
            .write_all(&nchw_to_rgb(values, out_height, out_width))
            .map_err(|error| native(format!("Frame encode failed: {error}")))?;

        processed += 1;
        if processed % 2 == 0 {
            emit(app, output, processed, total_frames, "processing");
        }
    }
    Ok(processed)
}

fn run_upscale(
    app: &AppHandle,
    models: &ModelManager,
    model_path: &Path,
    input: &Path,
    output: &Path,
    tmp: &Path,
) -> Result<String> {
    find_on_path(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" })
        .ok_or_else(|| native("ffmpeg was not found on PATH. Install ffmpeg to upscale videos."))?;
    let ffprobe = find_on_path(if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" })
        .ok_or_else(|| native("ffprobe was not found on PATH. Install ffmpeg to upscale videos."))?;
    let (width, height, total_frames, fps) = probe_video(&ffprobe, input)?;

    models.ensure_ort()?;
    let threads = std::thread::available_parallelism()
        .map(|count| (count.get() / 2).clamp(1, 4))
        .unwrap_or(2);
    let mut session = Session::builder()
        .map_err(|error| native(format!("Could not create an ONNX session: {error}")))?
        .with_execution_providers([CPU::default().build()])
        .map_err(|error| native(format!("Could not set the CPU provider: {error}")))?
        .with_intra_threads(threads)
        .map_err(|error| native(format!("Could not configure ONNX threads: {error}")))?
        .with_optimization_level(GraphOptimizationLevel::Level1)
        .map_err(|error| native(format!("Could not configure ONNX optimization: {error}")))?
        .commit_from_file(model_path)
        .map_err(|error| native(format!("Could not load the Real-ESRGAN model: {error}")))?;
    let input_name = session
        .inputs()
        .first()
        .map(|outlet| outlet.name().to_string())
        .ok_or_else(|| native("The ONNX model has no input."))?;
    let output_name = session
        .outputs()
        .first()
        .map(|outlet| outlet.name().to_string())
        .ok_or_else(|| native("The ONNX model has no output."))?;

    let mut decoder = ffmpeg_command()
        .args(["-v", "error", "-i"])
        .arg(input)
        .args([
            "-map",
            "0:v:0",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-vsync",
            "0",
            "-an",
            "pipe:1",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| native(format!("Could not start ffmpeg for decoding: {error}")))?;

    let mut encoder = ffmpeg_command()
        .args(["-y", "-v", "error", "-f", "rawvideo", "-pix_fmt", "rgb24"])
        .args([
            "-video_size",
            &format!("{}x{}", width * 4, height * 4),
            "-framerate",
            &fps.to_string(),
            "-i",
            "pipe:0",
        ])
        .arg("-i")
        .arg(input)
        .args([
            "-map",
            "0:v:0",
            "-map",
            "1:a?",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "18",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "copy",
            "-movflags",
            "+faststart",
        ])
        .arg(tmp)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| native(format!("Could not start ffmpeg for encoding: {error}")))?;

    let result = stream_frames(
        app,
        &mut session,
        &input_name,
        &output_name,
        &mut decoder,
        &mut encoder,
        width,
        height,
        total_frames,
        output,
    );

    match result {
        Ok(processed) => {
            drop(encoder.stdin.take());
            let decode_status = decoder
                .wait()
                .map_err(|error| native(format!("ffmpeg decoder failed: {error}")))?;
            let encode_status = encoder
                .wait()
                .map_err(|error| native(format!("ffmpeg encoder failed: {error}")))?;
            if !decode_status.success() || !encode_status.success() {
                let _ = std::fs::remove_file(tmp);
                return Err(native("ffmpeg could not encode the upscaled video."));
            }
            std::fs::rename(tmp, output)?;
            emit(app, output, processed, processed, "completed");
            Ok(output.to_string_lossy().into_owned())
        }
        Err(error) => {
            let _ = decoder.kill();
            let _ = encoder.kill();
            let _ = std::fs::remove_file(tmp);
            emit(app, output, 0, total_frames, "error");
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn upscale_video_with_realesrgan(
    app: AppHandle,
    path: String,
    models: State<'_, ModelManager>,
    upscalers: State<'_, UpscalerManager>,
) -> Result<String> {
    let input = PathBuf::from(&path);
    if !input.is_file() {
        return Err(native("The selected video file no longer exists."));
    }
    let model_path = upscalers
        .downloaded_model_path(MODEL_ID)
        .ok_or_else(|| native("Download Real-ESRGAN Compact in Settings → AI upscaling first."))?;

    let output_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| native(format!("Could not locate the app data folder: {error}")))?
        .join("upscaled");
    std::fs::create_dir_all(&output_dir)?;
    let stem = input
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let output = output_dir.join(format!("{stem}-realesrgan.mp4"));
    let tmp = output_dir.join(format!("{stem}-realesrgan.tmp.mp4"));

    let models = models.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_upscale(&app, &models, &model_path, &input, &output, &tmp)
    })
    .await
    .map_err(|error| native(format!("Upscale task failed: {error}")))?
}
