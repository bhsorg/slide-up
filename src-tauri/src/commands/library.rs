use crate::db::repository::NewMedia;
use crate::db::Repository;
use crate::error::{AppError, Result};
use crate::events;
use crate::metadata::{self, AudioMetadata};
use crate::models::{MediaFilter, MediaItem, MediaKind, ScanResult};
use crate::protected_cache;
use crate::state::{AppState, CacheIdentity};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, OnceLock};
use tokio::sync::mpsc;
use tokio::sync::Semaphore;
use tauri::{AppHandle, Emitter};
use tauri_plugin_dialog::DialogExt;
use walkdir::WalkDir;

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "wav", "flac", "aac", "ogg", "oga", "m4a", "wma", "aiff", "aif", "opus", "mp2",
];
// Cover generation runs libmpv decoders. Cap concurrent decodes so a grid of
// videos cannot spawn an unbounded number of native decoder threads.
static COVER_SEMAPHORE: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(2));
static COVER_SEQ: AtomicU64 = AtomicU64::new(0);

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "webm", "m4v", "wmv", "flv", "ts", "m2ts", "mpg", "mpeg", "3gp",
];

/// Encrypt the seek-preview frames staged by the metadata probe into the
/// active workspace's private cache. Disabled settings simply discard them.
fn persist_storyboard(
    data_dir: &Path,
    identity: &CacheIdentity,
    media_id: i64,
    enabled: bool,
) {
    let temp = metadata::video::storyboard_staging_dir(media_id);
    if !enabled {
        let _ = std::fs::remove_dir_all(&temp);
        return;
    }
    if !temp.exists() {
        return;
    }
    let root = protected_cache::workspace_dir(data_dir, &identity.workspace_id).join("storyboards");
    let target = root.join(media_id.to_string());
    let _ = std::fs::create_dir_all(&root);
    let _ = std::fs::remove_dir_all(&target);
    for index in 0..metadata::video::STORYBOARD_FRAMES {
        let staged = temp.join(format!("{index}.jpg"));
        let Ok(bytes) = std::fs::read(&staged) else { continue };
        let _ = protected_cache::write_file(
            &target.join(format!("{index}.hlv")),
            identity,
            "storyboard",
            media_id,
            index,
            &bytes,
        );
    }
    let _ = std::fs::remove_dir_all(&temp);
}

type StoryboardJob = (AppHandle, PathBuf, CacheIdentity, i64, PathBuf);

static STORYBOARD_SENDER: OnceLock<mpsc::UnboundedSender<StoryboardJob>> = OnceLock::new();

/// One persistent worker serializes storyboard decodes. Starting a fresh OS
/// thread per scan let several library scans run many mpv instances at once.
fn storyboard_sender() -> &'static mpsc::UnboundedSender<StoryboardJob> {
    STORYBOARD_SENDER.get_or_init(|| {
        let (sender, mut receiver) = mpsc::unbounded_channel::<StoryboardJob>();
        tauri::async_runtime::spawn(async move {
            while let Some(job) = receiver.recv().await {
                // Each decode blocks for seconds; keep it off the async runtime
                // and process jobs one at a time.
                let _ = tauri::async_runtime::spawn_blocking(move || run_storyboard_job(job)).await;
            }
        });
        sender
    })
}

fn run_storyboard_job((app, data_dir, identity, media_id, path): StoryboardJob) {
    let progress_app = app.clone();
    let progress = std::sync::Arc::new(move |done: usize, total: usize| {
        let _ = progress_app.emit(
            events::STORYBOARD_PROGRESS,
            serde_json::json!({
                "mediaId": media_id,
                "done": done,
                "total": total,
                "status": "running",
            }),
        );
    });

    let res = metadata::video::probe(&path, media_id, true, Some(progress));
    persist_storyboard(&data_dir, &identity, media_id, true);

    let _ = app.emit(
        events::STORYBOARD_PROGRESS,
        serde_json::json!({
            "mediaId": media_id,
            "done": metadata::video::STORYBOARD_FRAMES,
            "total": metadata::video::STORYBOARD_FRAMES,
            "status": if res.is_ok() { "done" } else { "error" },
        }),
    );
}

/// Queues storyboard seek-previews for newly imported videos without blocking
/// the UI or spawning a new decode thread per scan.
fn spawn_storyboard_worker(
    app: AppHandle,
    data_dir: PathBuf,
    identity: CacheIdentity,
    queue: Vec<(i64, PathBuf)>,
) {
    let sender = storyboard_sender();
    for (media_id, path) in queue {
        let _ = sender.send((app.clone(), data_dir.clone(), identity.clone(), media_id, path));
    }
}

fn classify(path: &Path) -> Option<MediaKind> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if AUDIO_EXTENSIONS.contains(&extension.as_str()) {
        Some(MediaKind::Audio)
    } else if VIDEO_EXTENSIONS.contains(&extension.as_str()) {
        Some(MediaKind::Video)
    } else {
        None
    }
}

fn fingerprint(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    Ok(Repository::fingerprint(path, metadata.len(), modified))
}

fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .map(|name| name.to_string_lossy().replace(['_', '-'], " "))
        .unwrap_or_else(|| "Untitled".to_string())
}

#[tauri::command]
pub async fn pick_files(app: AppHandle) -> Result<Vec<String>> {
    let files = {
        // Drop always-on-top while the native dialog is open; otherwise it
        // renders behind the player (PiP / detached widgets).
        let _topmost = crate::dialog_helper::TopmostGuard::suspend(&app);
        app.dialog()
            .file()
            .add_filter("Media files", &["mp3", "wav", "flac", "aac", "ogg", "m4a", "mp4", "mkv", "avi", "mov", "webm"])
            .set_title("Open media files")
            .blocking_pick_files()
    };
    Ok(files
        .into_iter()
        .flatten()
        .filter_map(|path| path.into_path().ok())
        .map(|path| path.to_string_lossy().into_owned())
        .collect())
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<String>> {
    let folder = {
        let _topmost = crate::dialog_helper::TopmostGuard::suspend(&app);
        app.dialog()
            .file()
            .set_title("Select a media folder")
            .blocking_pick_folder()
    };
    Ok(folder
        .and_then(|path| path.into_path().ok())
        .map(|path| path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub fn list_media(state: tauri::State<'_, AppState>, filter: MediaFilter) -> Result<Vec<MediaItem>> {
    let conn = state.unlocked_db()?;
    Repository::list_media(&conn, &filter)
}

#[tauri::command]
pub fn get_recent(state: tauri::State<'_, AppState>, limit: Option<i64>) -> Result<Vec<MediaItem>> {
    let conn = state.unlocked_db()?;
    Repository::get_recent(&conn, limit.unwrap_or(12))
}

#[tauri::command]
pub fn clear_recent(state: tauri::State<'_, AppState>) -> Result<()> {
    {
        let conn = state.unlocked_db()?;
        Repository::clear_recent(&conn)?;
    }
    let _ = state.app.emit(events::LIBRARY_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub fn remove_from_recent(state: tauri::State<'_, AppState>, ids: Vec<i64>) -> Result<()> {
    {
        let conn = state.unlocked_db()?;
        for id in ids {
            Repository::remove_from_recent(&conn, id)?;
        }
    }
    let _ = state.app.emit(events::LIBRARY_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub fn get_media(state: tauri::State<'_, AppState>, id: i64) -> Result<Option<MediaItem>> {
    let conn = state.unlocked_db()?;
    Repository::get_media(&conn, id)
}

#[tauri::command]
pub fn set_favorite(state: tauri::State<'_, AppState>, id: i64, favorite: bool) -> Result<()> {
    {
        let conn = state.unlocked_db()?;
        Repository::set_favorite(&conn, id, favorite)?;
    }
    let _ = state.app.emit(events::LIBRARY_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub fn delete_media(state: tauri::State<'_, AppState>, id: i64, delete_file: bool) -> Result<()> {
    let item = {
        let conn = state.unlocked_db()?;
        let media = Repository::get_media(&conn, id)?;
        Repository::delete_media(&conn, id)?;
        media
    };
    if let Some(item) = item {
        if delete_file {
            let _ = std::fs::remove_file(&item.path);
        }
        crate::metadata::thumbnail::remove_thumbnail(&state.data_dir, item.thumbnail_path.as_deref());
        crate::metadata::thumbnail::remove_storyboard(&state.data_dir, item.id);
    }
    let _ = state.app.emit(events::LIBRARY_CHANGED, ());
    Ok(())
}

/// Removes every audio or video entry from the library. Source files on disk
/// are never deleted; only the library catalog is cleared.
#[tauri::command]
pub fn clear_library(state: tauri::State<'_, AppState>, kind: String) -> Result<usize> {
    let kind = MediaKind::parse(&kind).ok_or_else(|| {
        AppError::new(
            crate::error::ErrorKind::Library,
            "Unknown media type. Expected 'audio' or 'video'.",
        )
    })?;
    let removed = {
        let conn = state.unlocked_db()?;
        let items = Repository::clear_media_by_kind(&conn, kind)?;
        for item in &items {
            crate::metadata::thumbnail::remove_thumbnail(
                &state.data_dir,
                item.thumbnail_path.as_deref(),
            );
            crate::metadata::thumbnail::remove_storyboard(&state.data_dir, item.id);
        }
        items.len()
    };
    let _ = state.app.emit(events::LIBRARY_CHANGED, ());
    Ok(removed)
}

#[tauri::command]
pub fn scan_folder(state: tauri::State<'_, AppState>, folder: String) -> Result<ScanResult> {
    scan_folder_impl(&state, &folder)
}

/// Shared implementation used by the command and the startup auto-scan.
pub(crate) fn scan_folder_impl(state: &AppState, folder: &str) -> Result<ScanResult> {
    let root = PathBuf::from(folder);
    if !root.is_dir() {
        return Err(AppError::media("The selected folder does not exist."));
    }

    let prefs = {
        let settings = state.settings.lock().unwrap();
        (
            settings.scan_recursive,
            settings.scan_hidden,
            settings.scan_audio,
            settings.scan_video,
            settings.generate_thumbnails,
            settings.remove_missing_on_scan,
            settings.auto_generate_storyboard,
        )
    };

    let mut found = 0usize;
    let mut added = 0usize;
    let mut updated = 0usize;
    let mut paths = Vec::new();
    let mut storyboard_queue = Vec::new();

    let walker = WalkDir::new(&root).follow_links(false);
    let walker = if prefs.0 { walker } else { walker.max_depth(1) };
    for entry in walker
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            prefs.1
                || !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with('.')
        })
    {
        let Some(kind) = classify(entry.path()) else {
            continue;
        };
        if (kind == MediaKind::Audio && !prefs.2) || (kind == MediaKind::Video && !prefs.3) {
            continue;
        }
        found += 1;
        let path_string = entry.path().to_string_lossy().into_owned();
        let fingerprint = match fingerprint(entry.path()) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let size = entry.metadata().map(|m| m.len() as i64).unwrap_or(0);

        let mut title = title_from_path(entry.path());
        let (artist, album, duration_ms, codec, width, height, cover) = match kind {
            MediaKind::Audio => {
                let info: AudioMetadata = metadata::audio::read(entry.path()).unwrap_or_default();
                if let Some(value) = info.title {
                    title = value;
                }
                (
                    info.artist,
                    info.album,
                    info.duration_ms,
                    info.codec,
                    None,
                    None,
                    info.cover,
                )
            }
            MediaKind::Video => {
                // Metadata only during scan; storyboards are generated in the
                // background after the library updates.
                let info = match metadata::video::probe(entry.path(), 0, false, None) {
                    Ok(info) => info,
                    Err(_) => continue,
                };
                (None, None, info.duration_ms, info.codec, Some(info.width), Some(info.height), None)
            }
        };

        let new_media = NewMedia {
            path: path_string.clone(),
            kind,
            title,
            artist,
            album,
            duration_ms,
            codec,
            width,
            height,
            size_bytes: size,
            fingerprint,
        };

        {
            let conn = state.unlocked_db()?;
            match Repository::upsert_media(&conn, &new_media) {
                Ok((id, changed)) => {
                    if changed {
                        updated += 1;
                    } else {
                        added += 1;
                    }
                    // Only embedded audio artwork is cached in `thumbnails`.
                    // Video items use the UI placeholder; their storyboards
                    // (when enabled) live in `storyboards/<id>`.
                    let thumbnail = if kind == MediaKind::Audio && prefs.4 {
                        cover.as_ref().and_then(|data| {
                            metadata::thumbnail::save_cover(&state.data_dir, id, data).ok()
                        })
                    } else {
                        None
                    };
                    if let Some(path) = thumbnail {
                        let _ = Repository::set_thumbnail(&conn, id, Some(&path));
                    }
                    if kind == MediaKind::Video && prefs.6 && new_media.duration_ms > 0 {
                        storyboard_queue.push((id, entry.path().to_path_buf()));
                    }
                }
                Err(_) => continue,
            }
        }
        paths.push(path_string);
        let _ = state.app.emit(
            events::LIBRARY_SCAN_PROGRESS,
            serde_json::json!({ "found": found, "path": entry.path().file_name().map(|v| v.to_string_lossy().into_owned()) }),
        );
    }

    let removed = if prefs.5 {
        let keep: HashSet<String> = paths.into_iter().collect();
        let conn = state.unlocked_db()?;
        Repository::remove_missing(&conn, &keep.into_iter().collect::<Vec<_>>()).unwrap_or(0)
    } else {
        0
    };

    // Library changed event emitted immediately so UI updates and shows all items at once!
    let _ = state.app.emit(events::LIBRARY_CHANGED, ());

    // Queue storyboard generation in the background so it never blocks the list.
    if prefs.6 && !storyboard_queue.is_empty() {
        spawn_storyboard_worker(
            state.app.clone(),
            state.data_dir.clone(),
            state.cache_identity()?,
            storyboard_queue,
        );
    }

    Ok(ScanResult {
        found,
        added,
        updated,
        removed,
    })
}

#[tauri::command]
pub fn import_files(state: tauri::State<'_, AppState>, paths: Vec<String>) -> Result<ScanResult> {
    import_files_impl(&state, paths)
}

pub(crate) fn import_files_impl(state: &AppState, paths: Vec<String>) -> Result<ScanResult> {
    let prefs = {
        let settings = state.settings.lock().unwrap();
        (
            settings.scan_audio,
            settings.scan_video,
            settings.generate_thumbnails,
            settings.auto_generate_storyboard,
        )
    };
    let mut added = 0usize;
    let mut updated = 0usize;
    let mut storyboard_queue = Vec::new();

    for path in &paths {
        let path = Path::new(path);
        let Some(kind) = classify(path) else {
            continue;
        };
        if (kind == MediaKind::Audio && !prefs.0) || (kind == MediaKind::Video && !prefs.1) {
            continue;
        }
        let fingerprint = fingerprint(path)?;
        let size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
        let mut title = title_from_path(path);
        let (artist, album, duration_ms, codec, width, height, cover) = match kind {
            MediaKind::Audio => {
                let info = metadata::audio::read(path).unwrap_or_default();
                if let Some(value) = info.title {
                    title = value;
                }
                (
                    info.artist,
                    info.album,
                    info.duration_ms,
                    info.codec,
                    None,
                    None,
                    info.cover,
                )
            }
            MediaKind::Video => {
                // Metadata only on import (false for storyboard).
                let info = metadata::video::probe(path, 0, false, None)?;
                (None, None, info.duration_ms, info.codec, Some(info.width), Some(info.height), None)
            }
        };
        let new_media = NewMedia {
            path: path.to_string_lossy().into_owned(),
            kind,
            title,
            artist,
            album,
            duration_ms,
            codec,
            width,
            height,
            size_bytes: size,
            fingerprint,
        };
        let conn = state.unlocked_db()?;
        let (id, changed) = Repository::upsert_media(&conn, &new_media)?;
        if changed { updated += 1 } else { added += 1 }
        let thumbnail = if kind == MediaKind::Audio && prefs.2 {
            cover.as_ref().and_then(|data| {
                metadata::thumbnail::save_cover(&state.data_dir, id, data).ok()
            })
        } else {
            None
        };
        if let Some(path) = thumbnail {
            let _ = Repository::set_thumbnail(&conn, id, Some(&path));
        }
        if kind == MediaKind::Video && prefs.3 && new_media.duration_ms > 0 {
            storyboard_queue.push((id, path.to_path_buf()));
        }
    }

    // Library changed event emitted immediately so UI updates and shows newly imported items instantly!
    let _ = state.app.emit(events::LIBRARY_CHANGED, ());

    // Generate storyboards asynchronously in background
    if prefs.3 && !storyboard_queue.is_empty() {
        spawn_storyboard_worker(
            state.app.clone(),
            state.data_dir.clone(),
            state.cache_identity()?,
            storyboard_queue,
        );
    }

    Ok(ScanResult { found: paths.len(), added, updated, removed: 0 })
}

#[tauri::command]
pub fn get_thumbnail(state: tauri::State<'_, AppState>, path: String) -> Result<Option<String>> {
    let conn = state.unlocked_db()?;
    Ok(Repository::get_media_by_path(&conn, &path)?
        .and_then(|media| media.thumbnail_path))
}

/// Return a video cover as a data URL, generating and encrypting it lazily on
/// first request. Audio artwork is unchanged; only video covers live in the
/// workspace's protected cache.
#[tauri::command]
pub async fn get_video_cover(
    state: tauri::State<'_, AppState>,
    media_id: i64,
) -> Result<Option<String>> {
    let identity = state.cache_identity()?;
    let cached = protected_cache::workspace_dir(&state.data_dir, &identity.workspace_id)
        .join("thumbnails")
        .join(format!("{media_id}.hlv"));
    if let Some(bytes) = protected_cache::read_file(&cached, &identity, "cover", media_id, 0)? {
        return Ok(Some(protected_cache::data_url(&bytes)));
    }

    let (path, kind, duration_ms) = {
        let conn = state.unlocked_db()?;
        let media = Repository::get_media(&conn, media_id)?
            .ok_or_else(|| AppError::media("This item is no longer in the library."))?;
        (media.path, media.kind, media.duration_ms)
    };
    if kind != MediaKind::Video {
        return Ok(None);
    }

    // Limit concurrent native decoders and re-check the cache: another request
    // for the same video may have finished while this one waited for a slot.
    let _permit = COVER_SEMAPHORE
        .acquire()
        .await
        .map_err(|_| AppError::native("Cover generation is shutting down."))?;
    if let Some(bytes) = protected_cache::read_file(&cached, &identity, "cover", media_id, 0)? {
        return Ok(Some(protected_cache::data_url(&bytes)));
    }

    // Decoding a frame can take seconds; never run it on the main UI thread.
    let result: Result<Option<String>> = tauri::async_runtime::spawn_blocking(move || {
        let seq = COVER_SEQ.fetch_add(1, Ordering::Relaxed);
        let temp = std::env::temp_dir().join(format!(
            "slideup-cover-{media_id}-{}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&temp);
        let result = metadata::video::capture_cover(&PathBuf::from(path), duration_ms, &temp);
        let _ = std::fs::remove_dir_all(&temp);
        let bytes = result?;
        protected_cache::write_file(&cached, &identity, "cover", media_id, 0, &bytes)?;
        Ok(Some(protected_cache::data_url(&bytes)))
    })
    .await
    .map_err(|err| AppError::native(format!("Could not generate the video cover: {err}")))?;
    result
}

/// Returns a cached seek-preview frame as a data URL, or `None` when storyboard
/// generation was disabled or failed for this video.
#[tauri::command]
pub fn get_storyboard_frame(
    state: tauri::State<'_, AppState>,
    media_id: i64,
    index: usize,
) -> Result<Option<String>> {
    if index >= metadata::video::STORYBOARD_FRAMES {
        return Ok(None);
    }
    let identity = state.cache_identity()?;
    let path = protected_cache::workspace_dir(&state.data_dir, &identity.workspace_id)
        .join("storyboards")
        .join(media_id.to_string())
        .join(format!("{index}.hlv"));
    Ok(protected_cache::read_file(&path, &identity, "storyboard", media_id, index)?
        .map(|bytes| protected_cache::data_url(&bytes)))
}

/// Whether a storyboard has already been cached for this video.
#[tauri::command]
pub fn has_storyboard(state: tauri::State<'_, AppState>, media_id: i64) -> Result<bool> {
    let identity = state.cache_identity()?;
    Ok(protected_cache::workspace_dir(&state.data_dir, &identity.workspace_id)
        .join("storyboards")
        .join(media_id.to_string())
        .join("0.hlv")
        .is_file())
}

/// Regenerate the 12 cached seek-preview frames for a video on demand.
#[tauri::command]
pub async fn generate_storyboard(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    media_id: i64,
) -> Result<()> {
    let (path, kind) = {
        let conn = state.unlocked_db()?;
        let media = Repository::get_media(&conn, media_id)?
            .ok_or_else(|| AppError::media("This item is no longer in the library."))?;
        (media.path, media.kind)
    };
    if kind != MediaKind::Video {
        return Err(AppError::media("Storyboards are only available for videos."));
    }
    let data_dir = state.data_dir.clone();
    let identity = state.cache_identity()?;
    let progress_app = app.clone();
    let progress = std::sync::Arc::new(move |done: usize, total: usize| {
        let _ = progress_app.emit(
            events::STORYBOARD_PROGRESS,
            serde_json::json!({
                "mediaId": media_id,
                "done": done,
                "total": total,
                "status": "running",
            }),
        );
    });
    let result = tauri::async_runtime::spawn_blocking(move || {
        let result = metadata::video::probe(
            &PathBuf::from(path),
            media_id,
            true,
            Some(progress),
        );
        persist_storyboard(&data_dir, &identity, media_id, true);
        result
    })
    .await
    .map_err(|error| AppError::native(error.to_string()))?;

    let _ = app.emit(
        events::STORYBOARD_PROGRESS,
        serde_json::json!({
            "mediaId": media_id,
            "done": metadata::video::STORYBOARD_FRAMES,
            "total": metadata::video::STORYBOARD_FRAMES,
            "status": if result.is_ok() { "done" } else { "error" },
        }),
    );
    result.map(|_| ())
}

/// Whether a sidecar subtitle file already exists next to the video. Used to
/// disable "Load subtitle file…" when no extra subtitle is needed.
#[tauri::command]
pub fn has_subtitle(state: tauri::State<'_, AppState>, media_id: i64) -> Result<bool> {
    let path = {
        let conn = state.unlocked_db()?;
        let media = Repository::get_media(&conn, media_id)?
            .ok_or_else(|| AppError::media("This item is no longer in the library."))?;
        media.path
    };
    let video = PathBuf::from(path);
    let parent = video.parent().unwrap_or_else(|| Path::new(""));
    let Some(stem) = video.file_stem().and_then(|value| value.to_str()) else {
        return Ok(false);
    };
    const EXTENSIONS: &[&str] = &["srt", "vtt", "ass", "ssa", "sub"];
    Ok(EXTENSIONS.iter().any(|extension| {
        parent.join(format!("{stem}.{extension}")).is_file()
            || parent
                .join(format!("{}.{}", stem.to_ascii_lowercase(), extension))
                .is_file()
    }))
}
