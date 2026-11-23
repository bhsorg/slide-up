use crate::db::Repository;
use crate::error::{AppError, Result};
use crate::events;
use crate::models::{AppSettings, MediaItem, PlayMode};
use crate::mpv::{EndReason, MpvClient};
use rusqlite::Connection;
use std::path::PathBuf;
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

#[derive(Clone)]
pub struct AppState {
    pub app: AppHandle,
    pub db: Arc<Mutex<Connection>>,
    pub mpv: Arc<MpvClient>,
    pub player: Arc<Mutex<PlayerContext>>,
    pub settings: Arc<Mutex<AppSettings>>,
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn new(
        app: AppHandle,
        db: Connection,
        mpv: Arc<MpvClient>,
        settings: AppSettings,
        data_dir: PathBuf,
    ) -> Arc<Self> {
        let state = Arc::new(Self {
            app,
            db: Arc::new(Mutex::new(db)),
            mpv,
            player: Arc::new(Mutex::new(PlayerContext::default())),
            settings: Arc::new(Mutex::new(settings)),
            data_dir,
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

    fn handle_end(&self, reason: EndReason) {
        match reason {
            EndReason::Eof => {
                let (next, repeat, shuffle) = {
                    let mut context = self.player.lock().unwrap();
                    let settings = self.settings.lock().unwrap();
                    (
                        context.next(settings.repeat, settings.shuffle),
                        settings.repeat,
                        settings.shuffle,
                    )
                };
                let _ = (repeat, shuffle);
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
    }

    pub fn play_media(&self, media: &MediaItem) -> Result<()> {
        let mut actual_media = media.clone();
        if let Ok(conn) = self.db.lock() {
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
        let (item, repeat, shuffle) = {
            let mut context = self.player.lock().unwrap();
            let settings = self.settings.lock().unwrap();
            (
                context.next(settings.repeat, settings.shuffle),
                settings.repeat,
                settings.shuffle,
            )
        };
        let _ = (repeat, shuffle);
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
        let json = serde_json::to_string(settings)?;
        {
            let conn = self.db.lock().unwrap();
            Repository::set_setting(&conn, "app", &json)?;
        }
        let previous = {
            let mut guard = self.settings.lock().unwrap();
            let old = guard.clone();
            *guard = settings.clone();
            old
        };
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
