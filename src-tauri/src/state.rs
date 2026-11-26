use crate::db::Repository;
use crate::error::{AppError, Result};
use crate::events;
use crate::models::{AppSettings, MediaItem, PlayMode};
use crate::mpv::{EndReason, MpvClient};
use crate::workspace::{self, DEFAULT_WORKSPACE_ID, WorkspaceInfo, WorkspaceKeys};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Default)]
pub struct PlayerContext {
    pub queue: Vec<MediaItem>,
    pub current_index: Option<usize>,
    pub name: String,
}

impl PlayerContext {
    pub fn set_queue(&mut self, items: Vec<MediaItem>, start_index: usize, name: String) {
        self.queue = items;
        self.current_index = if self.queue.is_empty() {
            None
        } else {
            Some(start_index.min(self.queue.len() - 1))
        };
        self.name = name;
    }

    pub fn current(&self) -> Option<MediaItem> {
        self.current_index
            .and_then(|index| self.queue.get(index))
            .cloned()
    }

    pub fn next(&mut self, repeat: PlayMode, shuffle: bool) -> Option<MediaItem> {
        if self.queue.is_empty() {
            return None;
        }
        let next_index = match self.current_index {
            None => 0,
            Some(index) if shuffle && self.queue.len() > 1 => {
                let mut seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos() as usize;
                seed ^= index.wrapping_mul(2654435761);
                let mut candidate = seed % self.queue.len();
                if candidate == index {
                    candidate = (candidate + 1) % self.queue.len();
                }
                candidate
            }
            Some(index) if index + 1 < self.queue.len() => index + 1,
            Some(_) if repeat == PlayMode::All => 0,
            Some(_) => return None,
        };
        self.current_index = Some(next_index);
        self.queue.get(next_index).cloned()
    }

    pub fn previous(&mut self) -> Option<MediaItem> {
        let index = self.current_index?;
        if index == 0 {
            return self.queue.first().cloned();
        }
        self.current_index = Some(index - 1);
        self.queue.get(index - 1).cloned()
    }

    pub fn play_next(&mut self, media: MediaItem) {
        let position = self.current_index.map_or(0, |i| i + 1);
        self.queue.insert(position, media);
    }

    pub fn add_to_queue(&mut self, media: MediaItem) {
        self.queue.push(media);
    }

    pub fn remove_at(&mut self, index: usize) -> Option<MediaItem> {
        if index >= self.queue.len() {
            return None;
        }
        let removed = self.queue.remove(index);
        match self.current_index {
            Some(current) if current == index => {
                self.current_index = if self.queue.is_empty() {
                    None
                } else {
                    Some(index.min(self.queue.len() - 1))
                };
            }
            Some(current) if current > index => self.current_index = Some(current - 1),
            _ => {}
        }
        Some(removed)
    }

    pub fn reorder(&mut self, from: usize, to: usize) {
        if from >= self.queue.len() || to >= self.queue.len() || from == to {
            return;
        }
        let current_id = self.current().map(|item| item.id);
        let item = self.queue.remove(from);
        self.queue.insert(to, item);
        self.current_index = current_id
            .and_then(|id| self.queue.iter().position(|item| item.id == id));
    }
}

/// Only one workspace is ever active. When it is locked the workspace DB is
/// replaced with an in-memory connection and this entry is removed, so no
/// command can reach another workspace's data even by accident.
#[derive(Debug, Clone)]
pub struct ActiveWorkspace {
    pub id: String,
    pub keys: WorkspaceKeys,
}

/// Identity needed to derive per-workspace encryption keys for cached media
/// images. Kept copyable so generation can run on a background thread while
/// the active workspace is later locked or switched.
#[derive(Debug, Clone)]
pub struct CacheIdentity {
    pub workspace_id: String,
    pub wmk: [u8; 32],
}

#[derive(Clone)]
pub struct AppState {
    pub app: AppHandle,
    /// Active workspace database. While locked this is an empty in-memory DB.
    /// Commands must go through `unlocked_db()` so a locked workspace cannot
    /// be read or written by accident.
    db_conn: Arc<Mutex<Connection>>,
    /// Global registry + machine-wide settings (never contains media data).
    pub app_db: Arc<Mutex<Connection>>,
    pub active: Arc<Mutex<Option<ActiveWorkspace>>>,
    pub mpv: Arc<MpvClient>,
    pub player: Arc<Mutex<PlayerContext>>,
    pub settings: Arc<Mutex<AppSettings>>,
    pub data_dir: PathBuf,
    /// Private playlists stay hidden until the workspace credential is
    /// presented again. Reset on every workspace switch/lock.
    private_unlocked: Arc<AtomicBool>,
}

impl AppState {
    pub fn new(
        app: AppHandle,
        db: Connection,
        app_db: Connection,
        active: Option<ActiveWorkspace>,
        mpv: Arc<MpvClient>,
        settings: AppSettings,
        data_dir: PathBuf,
    ) -> Arc<Self> {
        let state = Arc::new(Self {
            app,
            db_conn: Arc::new(Mutex::new(db)),
            app_db: Arc::new(Mutex::new(app_db)),
            active: Arc::new(Mutex::new(active)),
            mpv,
            player: Arc::new(Mutex::new(PlayerContext::default())),
            settings: Arc::new(Mutex::new(settings)),
            data_dir,
            private_unlocked: Arc::new(AtomicBool::new(false)),
        });

        let callback_state = Arc::clone(&state);
        state.mpv.on_end(move |reason| {
            callback_state.handle_end(reason);
        });

        // Apply saved playback/audio/subtitle preferences once at startup.
        let initial = state.settings.lock().unwrap().clone();
        let _ = state.mpv.apply_settings(&initial, None);
        let _ = state.mpv.set_volume(initial.volume);
        let _ = state.mpv.set_muted(initial.muted);
        let _ = state.mpv.set_shuffle(initial.shuffle);
        let _ = state.mpv.set_repeat(initial.repeat);

        state
    }

    /// Every library/playlist command must use this instead of the raw
    /// connection so a locked workspace can never be read or written.
    pub fn unlocked_db(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        if self.active.lock().unwrap().is_none() {
            Err(AppError::native("Unlock a workspace before using the library."))
        } else {
            Ok(self.db_conn.lock().unwrap())
        }
    }

    /// Key material for the protected image cache, only available while the
    /// workspace is unlocked.
    pub fn cache_identity(&self) -> Result<CacheIdentity> {
        let guard = self.active.lock().unwrap();
        let active = guard
            .as_ref()
            .ok_or_else(|| AppError::native("Unlock a workspace before using the library."))?;
        Ok(CacheIdentity {
            workspace_id: active.id.clone(),
            wmk: active.keys.wmk,
        })
    }

    pub fn require_unlocked(&self) -> Result<()> {
        drop(self.unlocked_db()?);
        Ok(())
    }

    pub fn is_locked(&self) -> bool {
        self.active.lock().unwrap().is_none()
    }

    /// A queue item can still be listed after its local file is deleted.
    /// Streams have no local path to verify.
    fn media_available(media: &MediaItem) -> bool {
        let path = media.path.as_str();
        if path.starts_with("http://")
            || path.starts_with("https://")
            || path.starts_with("rtmp://")
            || path.starts_with("rtsp://")
        {
            return true;
        }
        std::path::Path::new(path).is_file()
    }

    /// Advance past queue entries whose files no longer exist instead of
    /// handing a dead path to mpv, which reports it as a decode failure.
    fn next_available(&self, repeat: PlayMode, shuffle: bool) -> Option<MediaItem> {
        let mut context = self.player.lock().unwrap();
        let count = context.queue.len();
        for _ in 0..count {
            match context.next(repeat, shuffle) {
                Some(media) if Self::media_available(&media) => return Some(media),
                None => return None,
                Some(_) => continue,
            }
        }
        None
    }

    /// Whether private playlists have been revealed for this workspace session.
    pub fn private_visible(&self) -> bool {
        self.private_unlocked.load(Ordering::SeqCst)
    }

    pub fn reveal_private(&self) {
        self.private_unlocked.store(true, Ordering::SeqCst);
    }

    pub fn hide_private(&self) {
        self.private_unlocked.store(false, Ordering::SeqCst);
    }

    pub fn active_workspace(&self) -> Option<WorkspaceInfo> {
        let id = self.active.lock().unwrap().as_ref().map(|a| a.id.clone())?;
        workspace::get_workspace(&self.app_db.lock().unwrap(), &id).ok().flatten()
    }

    fn activate_workspace(&self, id: &str) -> Result<WorkspaceInfo> {
        let keys = workspace::load_keys(&self.app_db.lock().unwrap(), id)?;
        self.activate_with_keys(id, keys)
    }

    fn activate_with_keys(&self, id: &str, keys: WorkspaceKeys) -> Result<WorkspaceInfo> {
        let app_db = self.app_db.lock().unwrap();
        let mut info = workspace::get_workspace(&app_db, id)?
            .ok_or_else(|| AppError::native("This workspace no longer exists."))?;
        let conn = workspace::open_library_db(&self.data_dir, id, &keys)?;
        let global = app_db
            .query_row(
                "SELECT value FROM global_settings WHERE key = 'app'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok();
        let local = Repository::get_setting(&conn, "app")?;
        let settings = workspace::merge_settings(global.as_deref(), local.as_deref())?;
        drop(app_db);

        let _ = self.mpv.stop();
        self.player
            .lock()
            .unwrap()
            .set_queue(Vec::new(), 0, String::new());
        self.hide_private();
        {
            *self.db_conn.lock().unwrap() = conn;
            *self.active.lock().unwrap() = Some(ActiveWorkspace {
                id: id.to_string(),
                keys,
            });
            *self.settings.lock().unwrap() = settings.clone();
        }

        if info.locked {
            workspace::set_locked(&self.app_db.lock().unwrap(), id, false)?;
            info.locked = false;
        }
        workspace::set_last_active(&self.app_db.lock().unwrap(), id)?;
        let _ = self.app.emit(events::WORKSPACE_CHANGED, &info);
        let _ = self.app.emit(events::SETTINGS_CHANGED, &settings);
        let _ = self.app.emit(events::LIBRARY_CHANGED, ());
        let _ = self.app.emit(events::PLAYLISTS_CHANGED, ());
        Ok(info)
    }

    pub fn switch_workspace(&self, id: &str) -> Result<WorkspaceInfo> {
        if self.active.lock().unwrap().as_ref().is_some_and(|a| a.id == id) {
            return self.active_workspace().ok_or_else(|| AppError::native("Workspace is locked."));
        }
        let info = workspace::get_workspace(&self.app_db.lock().unwrap(), id)?
            .ok_or_else(|| AppError::native("This workspace no longer exists."))?;
        if info.locked && info.password_protected {
            return Err(AppError::native("Enter this workspace's password to switch to it."));
        }
        self.activate_workspace(id)
    }

    /// Select a workspace without unlocking it. Used when the user picks a
    /// password-protected workspace from the chooser: the target becomes the
    /// pending workspace and the lock screen asks for its password.
    pub fn select_workspace(&self, id: &str) -> Result<WorkspaceInfo> {
        if self.is_locked() {
            // No data is active; make sure the DB handle is still a blank one.
            let mut conn = self.db_conn.lock().unwrap();
            *conn = rusqlite::Connection::open_in_memory()?;
        } else {
            self.lock_workspace()?;
        }
        {
            let app_db = self.app_db.lock().unwrap();
            workspace::set_locked(&app_db, id, true)?;
            workspace::set_last_active(&app_db, id)?;
        }
        let info = {
            let app_db = self.app_db.lock().unwrap();
            workspace::get_workspace(&app_db, id)?
                .ok_or_else(|| AppError::native("Workspace not found."))?
        };
        let _ = self.app.emit(events::WORKSPACE_CHANGED, &info);
        Ok(info)
    }

    pub fn lock_workspace(&self) -> Result<WorkspaceInfo> {
        let id = self
            .active
            .lock()
            .unwrap()
            .as_ref()
            .map(|a| a.id.clone())
            .ok_or_else(|| AppError::native("No workspace is active."))?;
        let _ = self.mpv.stop();
        self.hide_private();
        self.player
            .lock()
            .unwrap()
            .set_queue(Vec::new(), 0, String::new());
        if let Some(mut active) = self.active.lock().unwrap().take() {
            workspace::zeroize(&mut active.keys);
        }
        *self.db_conn.lock().unwrap() = Connection::open_in_memory()?;
        {
            let app_db = self.app_db.lock().unwrap();
            workspace::set_locked(&app_db, &id, true)?;
            let global = app_db
                .query_row(
                    "SELECT value FROM global_settings WHERE key = 'app'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .ok();
            *self.settings.lock().unwrap() = workspace::merge_settings(global.as_deref(), None)?;
        }
        let info = workspace::get_workspace(&self.app_db.lock().unwrap(), &id)?
            .ok_or_else(|| AppError::native("This workspace no longer exists."))?;
        let _ = self.app.emit(events::WORKSPACE_CHANGED, &info);
        let _ = self.app.emit(events::QUEUE_CHANGED, serde_json::json!({
            "queue": Vec::<MediaItem>::new(),
            "currentIndex": Option::<usize>::None,
            "name": "",
        }));
        Ok(info)
    }

    pub fn unlock_workspace(&self, id: Option<String>) -> Result<WorkspaceInfo> {
        let id = id
            .or_else(|| workspace::last_active(&self.app_db.lock().unwrap()).ok().flatten())
            .unwrap_or_else(|| DEFAULT_WORKSPACE_ID.to_string());
        let info = workspace::get_workspace(&self.app_db.lock().unwrap(), &id)?
            .ok_or_else(|| AppError::native("Workspace not found."))?;
        if info.locked && info.password_protected {
            return Err(AppError::native(
                "This workspace has a password. Enter it to unlock.",
            ));
        }
        self.activate_workspace(&id)
    }

    pub fn unlock_with_password(&self, id: &str, password: &str) -> Result<WorkspaceInfo> {
        let (_, keys) = workspace::unlock_with_password(
            &self.app_db.lock().unwrap(),
            &self.data_dir,
            id,
            password,
        )?;
        self.activate_with_keys(id, keys)
    }

    pub fn set_workspace_password(&self, password: &str) -> Result<WorkspaceInfo> {
        let guard = self.active.lock().unwrap();
        let active = guard
            .as_ref()
            .ok_or_else(|| AppError::native("Unlock a workspace before setting its password."))?;
        let id = active.id.clone();
        let wmk = active.keys.wmk;
        drop(guard);
        workspace::set_workspace_password(&self.app_db.lock().unwrap(), &id, &wmk, password)?;
        self.active_workspace().ok_or_else(|| AppError::native("Workspace is locked."))
    }

    pub fn clear_workspace_password(&self, password: &str) -> Result<WorkspaceInfo> {
        let guard = self.active.lock().unwrap();
        let active = guard
            .as_ref()
            .ok_or_else(|| AppError::native("Unlock a workspace before changing its password."))?;
        let id = active.id.clone();
        let wmk = active.keys.wmk;
        drop(guard);
        workspace::clear_workspace_password(&self.app_db.lock().unwrap(), &id, &wmk, password)?;
        self.active_workspace().ok_or_else(|| AppError::native("Workspace is locked."))
    }

    fn handle_end(&self, reason: EndReason) {
        match reason {
            EndReason::Eof => {
                let (repeat, shuffle) = {
                    let settings = self.settings.lock().unwrap();
                    (settings.repeat, settings.shuffle)
                };
                let next = self.next_available(repeat, shuffle);
                if let Some(media) = next {
                    if let Err(err) = self.play_media(&media) {
                        let _ = self.app.emit(events::TOAST, err);
                    }
                } else {
                    // Clear the engine state so a later refresh cannot resurrect
                    // a "playing" status for a file that already ended.
                    let _ = self.mpv.stop();
                }
            }
            EndReason::Error => {
                let _ = self.app.emit(
                    events::PLAYER_ERROR,
                    "This media file could not be decoded. It may be corrupted or use an unsupported codec.",
                );
            }
            _ => {}
        }
    }

    pub fn emit_queue(&self) {
        let context = self.player.lock().unwrap();
        let _ = self.app.emit(
            events::QUEUE_CHANGED,
            serde_json::json!({
                "queue": context.queue,
                "currentIndex": context.current_index,
                "name": context.name,
            }),
        );

        // Remember the last queue so a restart can restore the same playlist.
        // Save only ids: media rows already hold every other field, and this
        // runs on every track change, so the payload must stay tiny.
        let session = crate::models::PlaybackSession {
            queue: context.queue.iter().map(|item| item.id).collect(),
            current_index: context.current_index.unwrap_or(0),
            name: context.name.clone(),
        };
        drop(context);
        if let Ok(json) = serde_json::to_string(&session) {
            // Never block playback if another operation holds the database.
            if !self.is_locked() && let Ok(conn) = self.db_conn.try_lock() {
                let _ = Repository::set_setting(&conn, "playback_session", &json);
            }
        }
    }

    pub fn play_media(&self, media: &MediaItem) -> Result<()> {
        self.require_unlocked()?;
        let mut actual_media = media.clone();
        if let Ok(conn) = self.db_conn.lock() {
            if actual_media.id <= 0
                || actual_media.path.starts_with("http://")
                || actual_media.path.starts_with("https://")
                || actual_media.path.starts_with("rtmp://")
                || actual_media.path.starts_with("rtsp://")
            {
                let new_media = crate::db::repository::NewMedia {
                    path: actual_media.path.clone(),
                    kind: actual_media.kind,
                    title: actual_media.title.clone(),
                    artist: actual_media.artist.clone(),
                    album: actual_media.album.clone(),
                    duration_ms: actual_media.duration_ms,
                    codec: actual_media.codec.clone(),
                    width: actual_media.width,
                    height: actual_media.height,
                    size_bytes: actual_media.size_bytes,
                    fingerprint: format!("stream:{}", actual_media.path),
                };
                if let Ok((id, _)) = Repository::upsert_media(&conn, &new_media) {
                    actual_media.id = id;
                    if let Some(thumb) = &actual_media.thumbnail_path {
                        let _ = Repository::set_thumbnail(&conn, id, Some(thumb));
                    }
                }
            }
            let _ = Repository::record_play(&conn, actual_media.id);
        }
        if !Self::media_available(&actual_media) {
            return Err(AppError::media("This media file is no longer available on disk."));
        }
        self.mpv.load(&actual_media)?;
        let _ = self.app.emit(events::LIBRARY_CHANGED, ());
        self.emit_queue();
        Ok(())
    }

    pub fn play_queue(&self, media: Vec<MediaItem>, index: usize, name: &str) -> Result<()> {
        let item = media.get(index).cloned().ok_or_else(|| {
            AppError::media("This playlist has no playable tracks.")
        })?;
        self.player
            .lock()
            .unwrap()
            .set_queue(media, index, name.to_string());
        self.play_media(&item)
    }

    pub fn next(&self) -> Result<()> {
        let (repeat, shuffle) = {
            let settings = self.settings.lock().unwrap();
            (settings.repeat, settings.shuffle)
        };
        let item = self.next_available(repeat, shuffle);
        if let Some(media) = item {
            self.play_media(&media)
        } else {
            self.mpv.stop()
        }
    }

    pub fn previous(&self) -> Result<()> {
        let snapshot = self.mpv.snapshot();
        if snapshot.duration_ms > 0 && snapshot.position_ms > 3000 {
            return self.mpv.seek_ms(0);
        }
        let item = self.player.lock().unwrap().previous();
        if let Some(media) = item {
            self.play_media(&media)
        } else {
            Ok(())
        }
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        self.require_unlocked()?;
        if !(0..=100).contains(&settings.player_widget_min_opacity)
            || !(0..=100).contains(&settings.player_widget_max_opacity)
            || settings.player_widget_min_opacity > settings.player_widget_max_opacity
        {
            return Err(AppError::new(
                crate::error::ErrorKind::Settings,
                "Player widget opacity must be between 0 and 100%, with minimum no greater than maximum.",
            ));
        }
        let (global, local) = workspace::split_settings(settings)?;
        {
            let app_db = self.app_db.lock().unwrap();
            app_db.execute(
                "INSERT OR REPLACE INTO global_settings (key, value) VALUES ('app', ?1)",
                [&global],
            )?;
        }
        {
            let conn = self.db_conn.lock().unwrap();
            Repository::set_setting(&conn, "app", &local)?;
        }
        let previous = {
            let mut guard = self.settings.lock().unwrap();
            let old = guard.clone();
            *guard = settings.clone();
            old
        };
        if previous.player_widget != settings.player_widget {
            crate::widget::sync(&self.app, settings.player_widget);
        }
        self.mpv.apply_settings(settings, Some(&previous))?;
        if previous.shuffle != settings.shuffle {
            self.mpv.set_shuffle(settings.shuffle)?;
        }
        if previous.repeat != settings.repeat {
            self.mpv.set_repeat(settings.repeat)?;
        }
        let _ = self.app.emit(events::SETTINGS_CHANGED, settings);
        Ok(())
    }
}
