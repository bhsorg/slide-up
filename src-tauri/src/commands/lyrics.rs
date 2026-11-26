use crate::db::Repository;
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

/// Envelope stored in DB/cache so a cached "not found" is different from "no cache".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedLyrics {
    pub found: bool,
    pub result: Option<LyricsResultDto>,
}

fn legacy_cache_dir(state: &AppState) -> std::path::PathBuf {
    state.data_dir.join("lyrics")
}

fn legacy_cache_path(state: &AppState, media_id: i64) -> std::path::PathBuf {
    legacy_cache_dir(state).join(format!("{media_id}.json"))
}

/// Read lyrics previously fetched for a media item.
/// Checks the SQLite database first; if not found, falls back to legacy JSON cache
/// and automatically migrates it to the database.
#[tauri::command]
pub fn get_lyrics(state: State<'_, AppState>, media_id: i64) -> Result<Option<CachedLyrics>> {
    let conn = state.unlocked_db()?;
    if let Some(json_str) = Repository::get_lyrics(&conn, media_id)? {
        let cached = serde_json::from_str::<CachedLyrics>(&json_str)?;
        return Ok(Some(cached));
    }

    // Fallback: check legacy JSON file on disk and auto-migrate to DB
    let path = legacy_cache_path(&state, media_id);
    if path.exists() {
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(cached) = serde_json::from_str::<CachedLyrics>(&raw) {
                if let Some(ref res) = cached.result {
                    let plain = res
                        .lines
                        .iter()
                        .map(|l| l.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    let _ = Repository::save_lyrics(
                        &conn,
                        media_id,
                        &res.track_name,
                        &res.artist_name,
                        res.synced,
                        &plain,
                        &raw,
                    );
                }
                return Ok(Some(cached));
            }
        }
    }

    Ok(None)
}

/// Persist lyrics into SQLite so lookups and voice/mic searches work across the app.
#[tauri::command]
pub fn save_lyrics(
    state: State<'_, AppState>,
    media_id: i64,
    result: Option<LyricsResultDto>,
) -> Result<()> {
    let envelope = CachedLyrics {
        found: result.is_some(),
        result: result.clone(),
    };
    let json_data = serde_json::to_string(&envelope)?;

    let conn = state.unlocked_db()?;
    if let Some(res) = result {
        let plain = res
            .lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        Repository::save_lyrics(
            &conn,
            media_id,
            &res.track_name,
            &res.artist_name,
            res.synced,
            &plain,
            &json_data,
        )?;
    } else {
        // Record negative lookup cache into DB
        Repository::save_lyrics(
            &conn,
            media_id,
            "",
            "",
            false,
            "",
            &json_data,
        )?;
    }

    Ok(())
}

/// Search media IDs whose lyrics contain the query phrase (for voice/mic or text lyrics search).
#[tauri::command]
pub fn search_lyrics(state: State<'_, AppState>, query: String) -> Result<Vec<i64>> {
    let conn = state.unlocked_db()?;
    Repository::search_media_by_lyrics(&conn, &query)
}
