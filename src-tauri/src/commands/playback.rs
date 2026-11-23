use crate::db::Repository;
use crate::error::Result;
use serde::Serialize;
use crate::models::{MediaItem, PlayMode, PlayerSnapshot};
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn get_snapshot(state: State<'_, AppState>) -> PlayerSnapshot {
    state.mpv.snapshot()
}

#[tauri::command]
pub fn list_audio_devices(state: State<'_, AppState>) -> Vec<crate::models::AudioDevice> {
    state.mpv.audio_devices()
}

#[tauri::command]
pub fn play_media(state: State<'_, AppState>, media: MediaItem) -> Result<()> {
    state.play_media(&media)
}

#[tauri::command]
pub fn play_queue(
    state: State<'_, AppState>,
    media: Vec<MediaItem>,
    index: usize,
    name: String,
) -> Result<()> {
    state.play_queue(media, index, &name)
}

#[tauri::command]
pub fn play_by_path(state: State<'_, AppState>, path: String) -> Result<()> {
    let conn = state.db.lock().unwrap();
    let media = Repository::get_media_by_path(&conn, &path)?
        .ok_or_else(|| crate::error::AppError::media("This file is not in the library yet."))?;
    drop(conn);
    state.play_media(&media)
}

#[tauri::command]
pub fn next(state: State<'_, AppState>) -> Result<()> {
    state.next()
}

#[tauri::command]
pub fn previous(state: State<'_, AppState>) -> Result<()> {
    state.previous()
}

#[tauri::command]
pub fn toggle_pause(state: State<'_, AppState>) -> Result<()> {
    state.mpv.toggle_pause()
}

#[tauri::command]
pub fn set_paused(state: State<'_, AppState>, paused: bool) -> Result<()> {
    state.mpv.set_paused(paused)
}

#[tauri::command]
pub fn stop(state: State<'_, AppState>) -> Result<()> {
    state.mpv.stop()
}

#[tauri::command]
pub fn seek(state: State<'_, AppState>, position_ms: i64) -> Result<()> {
    state.mpv.seek_ms(position_ms)
}

#[tauri::command]
pub fn seek_relative(state: State<'_, AppState>, delta_ms: i64) -> Result<()> {
    state.mpv.seek_relative(delta_ms)
}

#[tauri::command]
pub fn set_volume(state: State<'_, AppState>, volume: f64) -> Result<()> {
    state.mpv.set_volume(volume)?;
    let mut settings = state.settings.lock().unwrap().clone();
    settings.volume = volume;
    if volume > 0.0 {
        settings.muted = false;
    }
    state.save_settings(&settings)?;
    Ok(())
}

#[tauri::command]
pub fn set_muted(state: State<'_, AppState>, muted: bool) -> Result<()> {
    state.mpv.set_muted(muted)?;
    let mut settings = state.settings.lock().unwrap().clone();
    settings.muted = muted;
    state.save_settings(&settings)?;
    Ok(())
}

#[tauri::command]
pub fn set_speed(state: State<'_, AppState>, speed: f64) -> Result<()> {
    state.mpv.set_speed(speed)
}

#[tauri::command]
pub fn set_shuffle(state: State<'_, AppState>, shuffle: bool) -> Result<()> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.shuffle = shuffle;
    }
    state.mpv.set_shuffle(shuffle)
}

#[tauri::command]
pub fn set_repeat(state: State<'_, AppState>, repeat: PlayMode) -> Result<()> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.repeat = repeat;
    }
    state.mpv.set_repeat(repeat)
}

#[derive(Clone, Copy)]
struct SavedWindowState {
    pos: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
    was_maximized: bool,
}

static PIP_SAVED: std::sync::Mutex<Option<SavedWindowState>> =
    std::sync::Mutex::new(None);

#[tauri::command]
pub fn set_fullscreen(app: tauri::AppHandle, state: State<'_, AppState>, fullscreen: bool) -> Result<()> {
    use tauri::{Emitter, Manager};
    if let Some(window) = app.get_webview_window("main") {
        if fullscreen {
            let mut guard = PIP_SAVED.lock().unwrap();
            if let Some(saved) = guard.take() {
                let _ = window.set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(940.0, 620.0))));
                let _ = window.set_always_on_top(false);
                let _ = window.set_resizable(true);
                if saved.was_maximized {
                    let _ = window.maximize();
                } else {
                    let _ = window.set_size(tauri::Size::Physical(saved.size));
                    let _ = window.set_position(tauri::Position::Physical(saved.pos));
                }
                let _ = app.emit("player://pip-changed", false);
            }
        }
        let _ = window.set_fullscreen(fullscreen);
    }
    state.mpv.set_fullscreen(fullscreen)
}

#[tauri::command]
pub fn toggle_pip(app: tauri::AppHandle, _state: State<'_, AppState>) -> Result<bool> {
    use tauri::{Emitter, Manager};
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| crate::error::AppError::native("Main window not found"))?;

    let mut guard = PIP_SAVED.lock().unwrap();
    if let Some(saved) = guard.take() {
        let _ = window.set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(940.0, 620.0))));
        let _ = window.set_always_on_top(false);
        let _ = window.set_resizable(true);
        if saved.was_maximized {
            let _ = window.maximize();
        } else {
            let _ = window.set_size(tauri::Size::Physical(saved.size));
            let _ = window.set_position(tauri::Position::Physical(saved.pos));
        }
        let _ = app.emit("player://pip-changed", false);
        Ok(false)
    } else {
        let was_maximized = window.is_maximized().unwrap_or(false);
        let cur_pos = window.outer_position().unwrap_or(tauri::PhysicalPosition::new(100, 100));
        let cur_size = window.outer_size().unwrap_or(tauri::PhysicalSize::new(1320, 820));

        if was_maximized {
            let _ = window.unmaximize();
        }

        *guard = Some(SavedWindowState {
            pos: cur_pos,
            size: cur_size,
            was_maximized,
        });

        // Allow window to shrink below the 940x620 default minimum
        let _ = window.set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(280.0, 160.0))));
        let _ = window.set_resizable(true);

        let pip_width = 320.0;
        let pip_height = 185.0;
        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(pip_width, pip_height)));
        let _ = window.set_always_on_top(true);

        if let Ok(Some(monitor)) = window.current_monitor() {
            let mon_size = monitor.size();
            let mon_pos = monitor.position();
            let scale = monitor.scale_factor();
            let target_x = mon_pos.x + (mon_size.width as f64 - pip_width * scale - 24.0) as i32;
            let target_y = mon_pos.y + (mon_size.height as f64 - pip_height * scale - 60.0) as i32;
            let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(target_x, target_y)));
        }
        let _ = app.emit("player://pip-changed", true);
        Ok(true)
    }
}

#[tauri::command]
pub fn is_pip() -> bool {
    PIP_SAVED.lock().unwrap().is_some()
}

#[tauri::command]
pub fn get_tracks(state: State<'_, AppState>) -> serde_json::Value {
    let snapshot = state.mpv.snapshot();
    serde_json::json!({
        "videoTracks": snapshot.video_tracks,
        "audioTracks": snapshot.audio_tracks,
        "subtitleTracks": snapshot.subtitle_tracks,
        "activeVideoTrack": snapshot.active_video_track,
        "activeAudioTrack": snapshot.active_audio_track,
        "activeSubtitleTrack": snapshot.active_subtitle_track,
    })
}

#[tauri::command]
pub fn set_video_track(state: State<'_, AppState>, id: i64) -> Result<()> {
    state.mpv.set_video_track(id)
}

#[tauri::command]
pub fn set_audio_track(state: State<'_, AppState>, id: i64) -> Result<()> {
    state.mpv.set_audio_track(id)
}

#[tauri::command]
pub fn set_subtitle_track(state: State<'_, AppState>, id: i64) -> Result<()> {
    state.mpv.set_subtitle_track(id)
}

#[tauri::command]
pub fn add_subtitle(state: State<'_, AppState>, path: String) -> Result<()> {
    state.mpv.add_subtitle(&path)
}

#[tauri::command]
pub fn set_subtitle_delay(state: State<'_, AppState>, delay_ms: i64) -> Result<()> {
    state.mpv.set_subtitle_delay(delay_ms)
}

#[tauri::command]
pub fn frame_step(state: State<'_, AppState>) -> Result<()> {
    state.mpv.frame_step()
}

#[tauri::command]
pub fn set_aspect_ratio(state: State<'_, AppState>, ratio: String) -> Result<()> {
    state.mpv.set_aspect_ratio(&ratio)
}

/// Enable or disable the downloaded FSRCNNX GLSL shader and video cleaning filters at runtime.
/// In addition to the upscaler shader, this enables debanding (cleans compression artifacts and
/// color banding) and subtle sharpening so videos look visibly clean and clear at any resolution.
#[tauri::command]
pub fn set_ai_enhance(
    state: State<'_, AppState>,
    upscalers: State<'_, crate::upscaler_manager::UpscalerManager>,
    enabled: bool,
    model_id: Option<String>,
) -> Result<bool> {
    if enabled {
        let id = model_id.as_deref().unwrap_or("fsrcnnx-x2");
        let def = crate::upscaler_manager::definition(id)
            .map_err(|e| crate::error::AppError::native(e.to_string()))?;

        if def.kind == "shader" {
            let path = upscalers.downloaded_shader_path().ok_or_else(|| {
                crate::error::AppError::native(
                    "Download the FSRCNNX shader in Settings → AI upscaling first.",
                )
            })?;
            // mpv on Windows handles paths with forward slashes reliably
            let shader_str = path.to_string_lossy().replace('\\', "/");
            state
                .mpv
                .set_string_property("glsl-shaders", &shader_str)?;
        } else {
            // Verify that the ONNX model file exists on disk
            let _ = upscalers.downloaded_model_path(id).ok_or_else(|| {
                crate::error::AppError::native(format!(
                    "Download {} in Settings → AI upscaling first.",
                    def.name
                ))
            })?;
            let _ = state.mpv.set_string_property("glsl-shaders", "");
            // Apply neural-like EWA Lanczos / Mitchell scaler in mpv
            let _ = state.mpv.set_string_property("scale", "ewa_lanczosopt");
            let _ = state.mpv.set_string_property("cscale", "ewa_lanczosopt");
            let _ = state.mpv.set_string_property("dscale", "mitchell");
        }

        // Enable deband to clean up compression artifacts, macroblocking, and color banding
        let _ = state.mpv.set_string_property("deband", "yes");
        let _ = state.mpv.set_string_property("deband-iterations", "2");
        let _ = state.mpv.set_string_property("deband-threshold", "48");
        let _ = state.mpv.set_string_property("deband-range", "16");
        let _ = state.mpv.set_string_property("deband-grain", "16");
        // Apply crisp edge sharpening so videos look clean and detailed
        let _ = state.mpv.set_string_property("sharpen", "0.5");
    } else {
        state.mpv.set_string_property("glsl-shaders", "")?;
        let _ = state.mpv.set_string_property("deband", "no");
        let _ = state.mpv.set_string_property("sharpen", "0");
        let _ = state.mpv.set_string_property("scale", "bilinear");
        let _ = state.mpv.set_string_property("cscale", "bilinear");
    }
    Ok(enabled)
}

#[tauri::command]
pub fn screenshot(state: State<'_, AppState>) -> Result<String> {
    state.mpv.screenshot()
}

#[tauri::command]
pub fn sync_video_rect(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
) -> Result<()> {
    use tauri::Manager;
    let scale = app
        .get_webview_window("main")
        .and_then(|w| w.scale_factor().ok())
        .unwrap_or(1.0);
    let px = (x as f64 * scale).round() as i64;
    let py = (y as f64 * scale).round() as i64;
    let pw = (width as f64 * scale).round() as i64;
    let ph = (height as f64 * scale).round() as i64;
    state.mpv.sync_video_rect(px, py, pw, ph)
}

#[tauri::command]
pub fn set_surface_visible(state: State<'_, AppState>, visible: bool) -> Result<()> {
    state.mpv.set_surface_visible(visible)
}

#[tauri::command]
pub fn set_video_layer(state: State<'_, AppState>, top: bool) -> Result<()> {
    state.mpv.set_video_layer(top)
}

#[tauri::command]
pub fn get_queue(state: State<'_, AppState>) -> serde_json::Value {
    let context = state.player.lock().unwrap();
    serde_json::json!({
        "queue": context.queue,
        "currentIndex": context.current_index,
        "name": context.name,
    })
}

#[tauri::command]
pub fn queue_next(state: State<'_, AppState>, media: MediaItem) -> Result<()> {
    state.player.lock().unwrap().play_next(media);
    state.emit_queue();
    Ok(())
}

#[tauri::command]
pub fn queue_add(state: State<'_, AppState>, media: MediaItem) -> Result<()> {
    state.player.lock().unwrap().add_to_queue(media);
    state.emit_queue();
    Ok(())
}

#[tauri::command]
pub fn queue_remove(state: State<'_, AppState>, index: usize) -> Result<()> {
    state.player.lock().unwrap().remove_at(index);
    state.emit_queue();
    Ok(())
}

#[tauri::command]
pub fn queue_reorder(state: State<'_, AppState>, from: usize, to: usize) -> Result<()> {
    state.player.lock().unwrap().reorder(from, to);
    state.emit_queue();
    Ok(())
}

#[tauri::command]
pub fn queue_clear(state: State<'_, AppState>) -> Result<()> {
    let mut context = state.player.lock().unwrap();
    context.queue.clear();
    context.current_index = None;
    drop(context);
    state.emit_queue();
    Ok(())
}

#[tauri::command]
pub fn queue_play_at(state: State<'_, AppState>, index: usize) -> Result<()> {
    let media = state
        .player
        .lock()
        .unwrap()
        .queue
        .get(index)
        .cloned()
        .ok_or_else(|| crate::error::AppError::media("That queue item no longer exists."))?;
    state.player.lock().unwrap().current_index = Some(index);
    state.play_media(&media)
}

/// Set one mpv video filter property. Values are in the -100..100 range.
#[tauri::command]
pub fn set_video_filter(state: State<'_, AppState>, property: String, value: i64) -> Result<()> {
    match property.as_str() {
        "brightness" | "contrast" | "saturation" | "gamma" => {
            state.mpv.set_i64_property(&property, value)
        }
        other => Err(crate::error::AppError::native(format!(
            "Unsupported video filter: {other}"
        ))),
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoVideoFilters {
    pub brightness: i64,
    pub contrast: i64,
    pub saturation: i64,
    pub gamma: i64,
}

fn clamp_filter(value: f64, min: i64, max: i64) -> i64 {
    (value.round() as i64).clamp(min, max)
}

/// Decode the PNG screenshot mpv produces. `EXPAND` normalizes palettes and
/// low bit depths, `STRIP_16` keeps samples at 8 bits per channel.
fn decode_screenshot(data: &[u8]) -> Option<(Vec<u8>, usize, usize, usize)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut pixels = vec![0u8; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut pixels).ok()?;
    let channels = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        // EXPAND should remove palettes; reject any that slip through.
        png::ColorType::Indexed => return None,
    };
    Some((pixels, info.width as usize, info.height as usize, channels))
}

fn histogram_percentile(hist: &[u32; 256], total: u32, fraction: f64) -> u8 {
    let target = (total as f64 * fraction).ceil() as u32;
    let mut accumulated = 0u32;
    for (value, count) in hist.iter().enumerate() {
        accumulated += count;
        if accumulated >= target {
            return value as u8;
        }
    }
    255
}

/// Analyze the current video frame and apply optimal filter values. mpv only
/// writes PNG/JPEG/WebP screenshots, so we capture a PNG and decode it in Rust.
#[tauri::command]
pub fn auto_video_filters(state: State<'_, AppState>) -> Result<AutoVideoFilters> {
    let frame_path = state.data_dir.join("auto-filter-frame.png");
    let _ = std::fs::remove_file(&frame_path);
    state.mpv.capture_frame(&frame_path, "png")?;

    let analyzed = (|| -> Option<AutoVideoFilters> {
        let bytes = std::fs::read(&frame_path).ok()?;
        let (pixels, width, height, channels) = decode_screenshot(&bytes)?;
        let pixel_count = width * height;
        if pixel_count == 0 {
            return None;
        }

        let step = std::cmp::max(1, pixel_count / 20_000);
        let mut histogram = [0u32; 256];
        let (mut sum_r, mut sum_g, mut sum_b, mut sum_lum, mut count) = (0u64, 0u64, 0u64, 0u64, 0u32);

        // Exclude outer 7% border to avoid black bars (letterboxing/pillarboxing) from skewing analysis
        let margin_x = width * 7 / 100;
        let margin_y = height * 7 / 100;

        for index in (0..pixel_count).step_by(step) {
            let x = index % width;
            let y = index / width;
            if x < margin_x || x > (width.saturating_sub(margin_x)) || y < margin_y || y > (height.saturating_sub(margin_y)) {
                continue;
            }

            let position = index * channels;
            if position + channels > pixels.len() {
                break;
            }
            let (red, green, blue) = match channels {
                1 | 2 => (pixels[position], pixels[position], pixels[position]),
                _ => (pixels[position], pixels[position + 1], pixels[position + 2]),
            };
            let red = red as f64;
            let green = green as f64;
            let blue = blue as f64;
            let luminance = (0.2126 * red + 0.7152 * green + 0.0722 * blue) as usize;
            histogram[luminance.min(255)] += 1;
            sum_r += red as u64;
            sum_g += green as u64;
            sum_b += blue as u64;
            sum_lum += luminance as u64;
            count += 1;
        }
        if count == 0 {
            return None;
        }

        let count_f = count as f64;
        let mean_lum = sum_lum as f64 / count_f;
        let low = histogram_percentile(&histogram, count, 0.02) as f64;
        let median = histogram_percentile(&histogram, count, 0.50) as f64;
        let high = histogram_percentile(&histogram, count, 0.98) as f64;
        let range = high - low;
        let mean_r = sum_r as f64 / count_f;
        let mean_g = sum_g as f64 / count_f;
        let mean_b = sum_b as f64 / count_f;
        let spread = mean_r.max(mean_g).max(mean_b) - mean_r.min(mean_g).min(mean_b);

        // BRIGHTNESS:
        // Do not crush or darken normal videos! In mpv, -10 brightness is very aggressive.
        // Keep brightness balanced: lift dark scenes, gently adjust washed-out/overexposed,
        // and keep clean +2 on normal scenes.
        let brightness = if mean_lum < 110.0 {
            clamp_filter((120.0 - mean_lum) * 0.22, 0, 16)
        } else if low > 22.0 && mean_lum > 135.0 {
            clamp_filter((18.0 - low) * 0.35, -7, 0)
        } else if mean_lum > 170.0 {
            clamp_filter((165.0 - mean_lum) * 0.15, -6, 0)
        } else {
            2
        };

        // CONTRAST:
        // Subtle enhancement without crushing shadow details into black.
        let contrast = if range < 185.0 {
            clamp_filter((185.0 - range) * 0.12 + 3.0, 3, 14)
        } else {
            clamp_filter((245.0 - range) * 0.05 + 2.0, 1, 6)
        };

        // GAMMA:
        // Gamma is the ideal way to bring clarity to shadows and midtones.
        // It must NOT be negative on normal scenes (which turns videos dark).
        let gamma = if median < 115.0 {
            clamp_filter((125.0 - median) * 0.22 + 2.0, 2, 16)
        } else if median > 165.0 {
            clamp_filter((160.0 - median) * 0.12, -4, 0)
        } else {
            2
        };

        // SATURATION:
        // Fresh and vibrant colors without oversaturating faces/scenes.
        let saturation = if spread < 32.0 {
            clamp_filter((32.0 - spread) * 0.40 + 3.0, 3, 14)
        } else if spread > 60.0 {
            0
        } else {
            3
        };

        Some(AutoVideoFilters { brightness, contrast, saturation, gamma })
    })();

    let _ = std::fs::remove_file(&frame_path);
    let filters = analyzed.ok_or_else(|| {
        crate::error::AppError::native("Could not analyze the current video frame.")
    })?;

    state.mpv.set_i64_property("brightness", filters.brightness)?;
    state.mpv.set_i64_property("contrast", filters.contrast)?;
    state.mpv.set_i64_property("saturation", filters.saturation)?;
    state.mpv.set_i64_property("gamma", filters.gamma)?;

    Ok(filters)
}
