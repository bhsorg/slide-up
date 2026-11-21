use crate::error::{AppError, Result};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use std::path::Path;

#[derive(Debug, Default)]
pub struct AudioMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: i64,
    pub codec: Option<String>,
    pub cover: Option<(Vec<u8>, String)>,
}

fn clean(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub fn read(path: &Path) -> Result<AudioMetadata> {
    let tagged = Probe::open(path)
        .map_err(|err| AppError::media(format!("Could not open this audio file: {err}")))?
        .read()
        .map_err(|err| AppError::media(format!("Could not read audio information: {err}")))?;

    let tag = tagged.primary_tag();
    let title = clean(tag.and_then(|t| t.title()).map(|c| c.into_owned()));
    let artist = clean(tag.and_then(|t| t.artist()).map(|c| c.into_owned()));
    let album = clean(tag.and_then(|t| t.album()).map(|c| c.into_owned()));

    let duration_ms = tagged.properties().duration().as_millis() as i64;
    let codec = Some(format!("{:?}", tagged.file_type()));

    let cover = tag.and_then(|t| t.pictures().first()).and_then(|picture| {
        let mime = picture
            .mime_type()
            .map(|m| format!("{m:?}"))
            .unwrap_or_else(|| "image/jpeg".to_string());
        let extension = match mime.as_str() {
            m if m.contains("png") => "png",
            m if m.contains("webp") => "webp",
            _ => "jpg",
        };
        Some((picture.data().to_vec(), extension.to_string()))
    });

    Ok(AudioMetadata {
        title,
        artist,
        album,
        duration_ms,
        codec,
        cover,
    })
}
