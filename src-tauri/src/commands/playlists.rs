use crate::db::Repository;
use crate::error::{AppError, Result};
use crate::events;
use crate::models::{MediaItem, Playlist, PlaylistTrack};
use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

fn ensure_private_readable(state: &AppState, playlist_id: i64) -> Result<()> {
    if state.private_visible() {
        return Ok(());
    }
    let conn = state.unlocked_db()?;
    if Repository::is_playlist_locked(&conn, playlist_id)? {
        return Err(AppError::native(
            "This playlist is private. Reveal private playlists to use it.",
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn list_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>> {
    let conn = state.unlocked_db()?;
    Repository::list_playlists(&conn, state.private_visible())
}

#[tauri::command]
pub fn create_playlist(state: State<'_, AppState>, name: String) -> Result<i64> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::media("Please enter a playlist name."));
    }
    let id = {
        let conn = state.unlocked_db()?;
        Repository::create_playlist(&conn, name)?
    };
    let _ = state.app.emit(events::PLAYLISTS_CHANGED, ());
    Ok(id)
}

#[tauri::command]
pub fn rename_playlist(state: State<'_, AppState>, id: i64, name: String) -> Result<()> {
    ensure_private_readable(&state, id)?;
    {
        let conn = state.unlocked_db()?;
        Repository::rename_playlist(&conn, id, name.trim())?;
    }
    let _ = state.app.emit(events::PLAYLISTS_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub fn delete_playlist(state: State<'_, AppState>, id: i64) -> Result<()> {
    ensure_private_readable(&state, id)?;
    {
        let conn = state.unlocked_db()?;
        Repository::delete_playlist(&conn, id)?;
    }
    let _ = state.app.emit(events::PLAYLISTS_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub fn get_playlist_tracks(
    state: State<'_, AppState>,
    playlist_id: i64,
) -> Result<Vec<PlaylistTrack>> {
    ensure_private_readable(&state, playlist_id)?;
    let conn = state.unlocked_db()?;
    Repository::playlist_tracks(&conn, playlist_id)
}

#[tauri::command]
pub fn add_to_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    media_id: i64,
) -> Result<()> {
    ensure_private_readable(&state, playlist_id)?;
    {
        let conn = state.unlocked_db()?;
        Repository::add_to_playlist(&conn, playlist_id, media_id)?;
    }
    let _ = state.app.emit(events::PLAYLISTS_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub fn remove_from_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    media_id: i64,
) -> Result<()> {
    ensure_private_readable(&state, playlist_id)?;
    {
        let conn = state.unlocked_db()?;
        Repository::remove_from_playlist(&conn, playlist_id, media_id)?;
    }
    let _ = state.app.emit(events::PLAYLISTS_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub fn reorder_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    ordered_ids: Vec<i64>,
) -> Result<()> {
    ensure_private_readable(&state, playlist_id)?;
    {
        let conn = state.unlocked_db()?;
        Repository::reorder_playlist(&conn, playlist_id, &ordered_ids)?;
    }
    let _ = state.app.emit(events::PLAYLISTS_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub fn play_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    index: usize,
) -> Result<()> {
    ensure_private_readable(&state, playlist_id)?;
    let media: Vec<MediaItem> = {
        let conn = state.unlocked_db()?;
        Repository::playlist_tracks(&conn, playlist_id)?
            .into_iter()
            .map(|track| track.media)
            .collect()
    };
    state.play_queue(media, index, "Playlist")
}

#[tauri::command]
pub async fn export_playlist(app: AppHandle, playlist_id: i64, name: String) -> Result<Option<String>> {
    let media: Vec<MediaItem> = {
        // State is not available in async dialog commands, read through managed state.
        let state = app.state::<AppState>();
        ensure_private_readable(&state, playlist_id)?;
        let conn = state.unlocked_db()?;
        Repository::playlist_tracks(&conn, playlist_id)?
            .into_iter()
            .map(|track| track.media)
            .collect()
    };
    let destination = {
        // Native dialogs must not be covered by the app's topmost windows.
        let _topmost = crate::dialog_helper::TopmostGuard::suspend(&app);
        app.dialog()
            .file()
            .set_title("Export playlist")
            .set_file_name(&format!("{name}.m3u8"))
            .add_filter("M3U playlist", &["m3u", "m3u8"])
            .blocking_save_file()
    };
    let Some(destination) = destination.and_then(|path| path.into_path().ok()) else {
        return Ok(None);
    };
    let mut contents = String::from("#EXTM3U\n");
    for item in media {
        contents.push_str(&format!("#EXTINF:{},{} - {}\n", item.duration_ms / 1000, item.artist.unwrap_or_default(), item.title));
        contents.push_str(&item.path);
        contents.push('\n');
    }
    std::fs::write(&destination, contents)?;
    Ok(Some(destination.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn import_playlist(app: AppHandle, playlist_name: Option<String>) -> Result<Option<i64>> {
    let source = {
        let _topmost = crate::dialog_helper::TopmostGuard::suspend(&app);
        app.dialog()
            .file()
            .set_title("Import M3U playlist")
            .add_filter("M3U playlist", &["m3u", "m3u8"])
            .blocking_pick_file()
    };
    let Some(source) = source.and_then(|path| path.into_path().ok()) else {
        return Ok(None);
    };
    let contents = std::fs::read_to_string(&source)?;
    let name = playlist_name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            source
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Imported playlist".to_string())
        });

    let state = app.state::<AppState>();
    let playlist_id = {
        let conn = state.unlocked_db()?;
        Repository::create_playlist(&conn, &name)?
    };
    let base = source.parent().unwrap_or(std::path::Path::new("."));
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let path = std::path::Path::new(line);
        let full = if path.is_absolute() {
            path.to_path_buf()
        } else {
            base.join(path)
        };
        if !full.is_file() {
            continue;
        }
        let conn = state.unlocked_db()?;
        let media = Repository::get_media_by_path(&conn, &full.to_string_lossy())?;
        if let Some(media) = media {
            Repository::add_to_playlist(&conn, playlist_id, media.id)?;
        }
    }
    let _ = app.emit(events::PLAYLISTS_CHANGED, ());
    Ok(Some(playlist_id))
}

#[tauri::command]
pub fn set_playlist_locked(state: State<'_, AppState>, id: i64, locked: bool) -> Result<()> {
    // Without this guard a hidden playlist could be unlocked blindly by
    // guessing its id through the IPC surface.
    ensure_private_readable(&state, id)?;
    {
        let conn = state.unlocked_db()?;
        Repository::set_playlist_locked(&conn, id, locked)?;
    }
    // Relocking one playlist re-hides the whole private set for this session.
    if locked {
        state.hide_private();
    }
    let _ = state.app.emit(events::PLAYLISTS_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub async fn reveal_private_playlists(
    state: State<'_, AppState>,
    password: Option<String>,
) -> Result<bool> {
    // Credential verification uses Argon2; keep it off the main thread.
    let task_state = state.inner().clone();
    let verified = tauri::async_runtime::spawn_blocking(move || {
        let info = task_state
            .active_workspace()
            .ok_or_else(|| AppError::native("Unlock a workspace first."))?;
        let app_db = task_state.app_db.lock().unwrap();
        crate::workspace::verify_workspace_credential(&app_db, &info.id, password.as_deref())
    })
    .await
    .map_err(|err| AppError::native(format!("Could not verify the workspace: {err}")))??;
    if verified {
        state.reveal_private();
        let _ = state.app.emit(events::PLAYLISTS_CHANGED, ());
    }
    Ok(verified)
}

#[tauri::command]
pub fn hide_private_playlists(state: State<'_, AppState>) -> Result<()> {
    state.hide_private();
    let _ = state.app.emit(events::PLAYLISTS_CHANGED, ());
    Ok(())
}
