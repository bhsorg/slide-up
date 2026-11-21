use crate::db::Repository;
use crate::error::{AppError, Result};
use crate::events;
use crate::models::{MediaItem, Playlist, PlaylistTrack};
use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub fn list_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>> {
    let conn = state.db.lock().unwrap();
    Repository::list_playlists(&conn)
}

#[tauri::command]
pub fn create_playlist(state: State<'_, AppState>, name: String) -> Result<i64> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::media("Please enter a playlist name."));
    }
    let id = {
        let conn = state.db.lock().unwrap();
        Repository::create_playlist(&conn, name)?
    };
    let _ = state.app.emit(events::PLAYLISTS_CHANGED, ());
    Ok(id)
}

#[tauri::command]
pub fn rename_playlist(state: State<'_, AppState>, id: i64, name: String) -> Result<()> {
    {
        let conn = state.db.lock().unwrap();
        Repository::rename_playlist(&conn, id, name.trim())?;
    }
    let _ = state.app.emit(events::PLAYLISTS_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub fn delete_playlist(state: State<'_, AppState>, id: i64) -> Result<()> {
    {
        let conn = state.db.lock().unwrap();
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
    let conn = state.db.lock().unwrap();
    Repository::playlist_tracks(&conn, playlist_id)
}

#[tauri::command]
pub fn add_to_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    media_id: i64,
) -> Result<()> {
    {
        let conn = state.db.lock().unwrap();
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
    {
        let conn = state.db.lock().unwrap();
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
    {
        let conn = state.db.lock().unwrap();
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
    let media: Vec<MediaItem> = {
        let conn = state.db.lock().unwrap();
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
        let conn = state.db.lock().unwrap();
        Repository::playlist_tracks(&conn, playlist_id)?
            .into_iter()
            .map(|track| track.media)
            .collect()
    };
    let destination = app
        .dialog()
        .file()
        .set_title("Export playlist")
        .set_file_name(&format!("{name}.m3u8"))
        .add_filter("M3U playlist", &["m3u", "m3u8"])
        .blocking_save_file();
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
    let source = app
        .dialog()
        .file()
        .set_title("Import M3U playlist")
        .add_filter("M3U playlist", &["m3u", "m3u8"])
        .blocking_pick_file();
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
        let conn = state.db.lock().unwrap();
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
        let conn = state.db.lock().unwrap();
        let media = Repository::get_media_by_path(&conn, &full.to_string_lossy())?;
        if let Some(media) = media {
            Repository::add_to_playlist(&conn, playlist_id, media.id)?;
        }
    }
    let _ = app.emit(events::PLAYLISTS_CHANGED, ());
    Ok(Some(playlist_id))
}
