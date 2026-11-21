use crate::error::Result;
use std::path::Path;

/// Persists embedded artwork extracted by `lofty` into the application data dir.
pub fn save_cover(
    data_dir: &Path,
    media_id: i64,
    cover: &(Vec<u8>, String),
) -> Result<String> {
    let thumbs = data_dir.join("thumbnails");
    std::fs::create_dir_all(&thumbs)?;
    let path = thumbs.join(format!("{media_id}.{}", cover.1));
    std::fs::write(&path, &cover.0)?;
    Ok(path.to_string_lossy().into_owned())
}

pub fn remove_thumbnail(data_dir: &Path, path: Option<&str>) {
    if let Some(path) = path {
        let full = Path::new(path);
        if full.exists() && full.starts_with(data_dir) {
            let _ = std::fs::remove_file(full);
        }
    }
}

/// Removes the cached seek-preview frames for a deleted media item.
pub fn remove_storyboard(data_dir: &Path, media_id: i64) {
    let path = data_dir.join("storyboards").join(media_id.to_string());
    if path.exists() && path.starts_with(data_dir) {
        let _ = std::fs::remove_dir_all(path);
    }
}
