use crate::db::Repository;
use crate::error::Result;
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

static PIP_SAVED: std::sync::Mutex<Option<(tauri::PhysicalPosition<i32>, tauri::PhysicalSize<u32>)>> =
    std::sync::Mutex::new(None);

#[tauri::command]
pub fn set_fullscreen(app: tauri::AppHandle, state: State<'_, AppState>, fullscreen: bool) -> Result<()> {
    use tauri::{Emitter, Manager};
    if let Some(window) = app.get_webview_window("main") {
        if fullscreen {
            let mut guard = PIP_SAVED.lock().unwrap();
            if let Some((pos, size)) = guard.take() {
                let _ = window.set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(940.0, 620.0))));
                let _ = window.set_always_on_top(false);
                let _ = window.set_size(tauri::Size::Physical(size));
                let _ = window.set_position(tauri::Position::Physical(pos));
                let _ = window.set_resizable(true);
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
    if let Some((pos, size)) = guard.take() {
        let _ = window.set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(940.0, 620.0))));
        let _ = window.set_always_on_top(false);
        let _ = window.set_size(tauri::Size::Physical(size));
        let _ = window.set_position(tauri::Position::Physical(pos));
        let _ = window.set_resizable(true);
        let _ = app.emit("player://pip-changed", false);
        Ok(false)
    } else {
        let cur_pos = window.outer_position().unwrap_or(tauri::PhysicalPosition::new(100, 100));
        let cur_size = window.outer_size().unwrap_or(tauri::PhysicalSize::new(1320, 820));
        *guard = Some((cur_pos, cur_size));

        // Allow window to shrink below the 940x620 default minimum
        let _ = window.set_min_size(Some(tauri::Size::Logical(tauri::LogicalSize::new(360.0, 220.0))));
        let _ = window.set_resizable(true);

        let pip_width = 560.0;
        let pip_height = 340.0;
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
        "audioTracks": snapshot.audio_tracks,
        "subtitleTracks": snapshot.subtitle_tracks,
        "activeAudioTrack": snapshot.active_audio_track,
        "activeSubtitleTrack": snapshot.active_subtitle_track,
    })
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

/// Enable or disable the downloaded FSRCNNX GLSL shader at runtime.
/// The shader path is resolved in Rust so the frontend never needs filesystem access.
#[tauri::command]
pub fn set_ai_enhance(
    state: State<'_, AppState>,
    upscalers: State<'_, crate::upscaler_manager::UpscalerManager>,
    enabled: bool,
) -> Result<bool> {
    if enabled {
        let path = upscalers.downloaded_shader_path().ok_or_else(|| {
            crate::error::AppError::native(
                "Download the FSRCNNX shader in Settings → AI upscaling first.",
            )
        })?;
        state
            .mpv
            .set_string_property("glsl-shaders", &path.to_string_lossy())?;
    } else {
        state.mpv.set_string_property("glsl-shaders", "")?;
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
