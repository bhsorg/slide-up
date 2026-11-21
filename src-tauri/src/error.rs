use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone)]
pub enum ErrorKind {
    Media,
    Playback,
    Library,
    Playlist,
    Settings,
    Native,
    Io,
}

#[derive(Debug, Clone)]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
}

impl AppError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn media(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Media, message)
    }

    pub fn playback(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Playback, message)
    }

    pub fn native(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Native, message)
    }

    pub fn code(&self) -> &'static str {
        match self.kind {
            ErrorKind::Media => "MEDIA_ERROR",
            ErrorKind::Playback => "PLAYBACK_ERROR",
            ErrorKind::Library => "LIBRARY_ERROR",
            ErrorKind::Playlist => "PLAYLIST_ERROR",
            ErrorKind::Settings => "SETTINGS_ERROR",
            ErrorKind::Native => "NATIVE_ERROR",
            ErrorKind::Io => "IO_ERROR",
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AppError {}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("AppError", 3)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.message)?;
        state.serialize_field("title", match self.kind {
            ErrorKind::Media => "Media problem",
            ErrorKind::Playback => "Playback problem",
            ErrorKind::Library => "Library problem",
            ErrorKind::Playlist => "Playlist problem",
            ErrorKind::Settings => "Settings problem",
            ErrorKind::Native => "System problem",
            ErrorKind::Io => "File problem",
        })?;
        state.end()
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::new(ErrorKind::Io, friendly_io_message(&err))
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        Self::new(ErrorKind::Library, format!("Library database error: {err}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(ErrorKind::Settings, format!("Could not read saved data: {err}"))
    }
}

fn friendly_io_message(err: &std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::NotFound => "The file could not be found.".to_string(),
        std::io::ErrorKind::PermissionDenied => {
            "Access to this file was denied. Check its permissions.".to_string()
        }
        std::io::ErrorKind::InvalidData => "This file appears to be corrupted.".to_string(),
        _ => format!("File operation failed: {err}"),
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
