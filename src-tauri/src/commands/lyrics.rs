use crate::error::Result;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLineDto {
    pub time_ms: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResultDto {
    pub lines: Vec<LyricLineDto>,
    pub synced: bool,
    pub track_name: String,
    pub artist_name: String,
}

/// Envelope stored on disk so a cached "not found" is different from "no cache".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedLyrics {
    pub found: bool,
    pub result: Option<LyricsResultDto>,
}

fn cache_dir(state: &AppState) -> std::path::PathBuf {
    state.data_dir.join("lyrics")
}

fn cache_path(state: &AppState, media_id: i64) -> std::path::PathBuf {
    cache_dir(state).join(format!("{media_id}.json"))
}

/// Read lyrics previously fetched for a media item.
/// `None` means there is no cache entry; `Some` with `found: false` means the
/// previous lookup determined that no lyrics exist for this track.
#[tauri::command]
pub fn get_lyrics(state: State<'_, AppState>, media_id: i64) -> Result<Option<CachedLyrics>> {
    let path = cache_path(&state, media_id);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str::<CachedLyrics>(&raw)?))
}

/// Persist lyrics (or a not-found result) so lookups survive app restarts.
#[tauri::command]
pub fn save_lyrics(
    state: State<'_, AppState>,
    media_id: i64,
    result: Option<LyricsResultDto>,
) -> Result<()> {
    let directory = cache_dir(&state);
    std::fs::create_dir_all(&directory)?;
    let final_path = cache_path(&state, media_id);
    let temp_path = final_path.with_extension("json.tmp");
    let envelope = CachedLyrics {
        found: result.is_some(),
        result,
    };
    std::fs::write(&temp_path, serde_json::to_string(&envelope)?)?;
    // On Windows rename fails if the destination already exists.
    let _ = std::fs::remove_file(&final_path);
    std::fs::rename(temp_path, final_path)?;
    Ok(())
}
