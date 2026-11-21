use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Audio,
    Video,
}

impl MediaKind {
    pub fn as_str(self) -> &'static str {
        match self {
            MediaKind::Audio => "audio",
            MediaKind::Video => "video",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "audio" => Some(MediaKind::Audio),
            "video" => Some(MediaKind::Video),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub id: i64,
    pub path: String,
    pub kind: MediaKind,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: i64,
    pub codec: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub size_bytes: i64,
    pub thumbnail_path: Option<String>,
    pub favorite: bool,
    pub play_count: i64,
    pub last_played_at: Option<i64>,
    pub added_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFilter {
    pub kind: Option<String>,
    pub favorite: Option<bool>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub found: usize,
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub track_count: i64,
    pub cover_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistTrack {
    pub media: MediaItem,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInfo {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub kind: MediaKind,
    pub thumbnail_path: Option<String>,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaTrack {
    pub id: i64,
    pub name: String,
    pub language: Option<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayMode {
    Off,
    All,
    One,
}

impl PlayMode {
    pub fn as_str(self) -> &'static str {
        match self {
            PlayMode::Off => "off",
            PlayMode::All => "all",
            PlayMode::One => "one",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "all" => PlayMode::All,
            "one" => PlayMode::One,
            _ => PlayMode::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayerStatus {
    Stopped,
    Loading,
    Playing,
    Paused,
    Error,
}

impl PlayerStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            PlayerStatus::Stopped => "stopped",
            PlayerStatus::Loading => "loading",
            PlayerStatus::Playing => "playing",
            PlayerStatus::Paused => "paused",
            PlayerStatus::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub status: PlayerStatus,
    pub current: Option<TrackInfo>,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
    pub shuffle: bool,
    pub repeat: PlayMode,
    pub fullscreen: bool,
    pub audio_tracks: Vec<MediaTrack>,
    pub subtitle_tracks: Vec<MediaTrack>,
    pub active_audio_track: i64,
    pub active_subtitle_track: i64,
    pub video_width: i64,
    pub video_height: i64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressSnapshot {
    pub position_ms: i64,
    pub duration_ms: i64,
    pub status: PlayerStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shortcuts {
    pub play_pause: String,
    pub seek_back: String,
    pub seek_forward: String,
    pub volume_up: String,
    pub volume_down: String,
    pub fullscreen: String,
    pub mute: String,
    pub next: String,
    pub previous: String,
    pub exit_fullscreen: String,
    pub open_file: String,
    pub open_folder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub theme: String,
    pub default_media_folder: Option<String>,
    pub volume: f64,
    pub muted: bool,
    pub playback_speed: f64,
    pub shuffle: bool,
    pub repeat: PlayMode,
    pub hardware_acceleration: bool,
    pub audio_output: String,
    pub audio_device: String,
    pub subtitle_font: String,
    pub subtitle_size: i64,
    pub subtitle_color: String,
    pub subtitle_border_size: i64,
    pub subtitle_margin: i64,
    pub subtitle_delay_ms: i64,
    pub scan_recursive: bool,
    pub scan_hidden: bool,
    pub scan_audio: bool,
    pub scan_video: bool,
    pub generate_thumbnails: bool,
    pub remove_missing_on_scan: bool,
    pub auto_scan_on_startup: bool,
    pub start_minimized: bool,
    pub minimize_to_tray: bool,
    #[serde(default)]
    pub custom_extensions: Vec<CustomExtension>,
    #[serde(default)]
    pub live_captions_enabled: bool,
    pub shortcuts: Shortcuts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub name: String,
    pub description: String,
}

/// An extension added by the user. Built-in formats are never stored here, so
/// the UI can reliably tell user-owned entries apart from the built-in list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomExtension {
    pub ext: String,
    /// "audio" or "video" — only used for grouping in the UI.
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileExtension {
    pub ext: String,
    pub kind: String,
    pub registered: bool,
    pub custom: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileAssociations {
    /// True when every built-in format is currently associated with Slide Up.
    pub registered: bool,
    pub audio: Vec<FileExtension>,
    pub video: Vec<FileExtension>,
    pub custom: Vec<FileExtension>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            default_media_folder: None,
            volume: 0.8,
            muted: false,
            playback_speed: 1.0,
            shuffle: false,
            repeat: PlayMode::Off,
            hardware_acceleration: true,
            audio_output: "auto".into(),
            audio_device: "auto".into(),
            subtitle_font: "Sans".into(),
            subtitle_size: 22,
            subtitle_color: "#FFFFFF".into(),
            subtitle_border_size: 3,
            subtitle_margin: 24,
            subtitle_delay_ms: 0,
            scan_recursive: true,
            scan_hidden: false,
            scan_audio: true,
            scan_video: true,
            generate_thumbnails: true,
            remove_missing_on_scan: false,
            auto_scan_on_startup: false,
            start_minimized: false,
            minimize_to_tray: false,
            custom_extensions: Vec::new(),
            live_captions_enabled: false,
            shortcuts: Shortcuts {
                play_pause: "Space".into(),
                seek_back: "ArrowLeft".into(),
                seek_forward: "ArrowRight".into(),
                volume_up: "ArrowUp".into(),
                volume_down: "ArrowDown".into(),
                fullscreen: "f".into(),
                mute: "m".into(),
                next: "n".into(),
                previous: "p".into(),
                exit_fullscreen: "Escape".into(),
                open_file: "Ctrl+O".into(),
                open_folder: "Ctrl+Shift+O".into(),
            },
        }
    }
}
