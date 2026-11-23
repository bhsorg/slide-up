use crate::error::Result;
use crate::models::{MediaFilter, MediaItem, MediaKind, Playlist, PlaylistTrack};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub struct Repository;

#[derive(Debug, Clone)]
pub struct NewMedia {
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
    pub fingerprint: String,
}

const MEDIA_COLUMNS: &str =
    "id, path, kind, title, artist, album, duration_ms, codec, width, height, \
     size_bytes, thumbnail_path, favorite, play_count, last_played_at, added_at";

fn row_to_media(row: &rusqlite::Row<'_>) -> rusqlite::Result<MediaItem> {
    let kind: String = row.get(2)?;
    Ok(MediaItem {
        id: row.get(0)?,
        path: row.get(1)?,
        kind: MediaKind::parse(&kind).unwrap_or(MediaKind::Audio),
        title: row.get(3)?,
        artist: row.get(4)?,
        album: row.get(5)?,
        duration_ms: row.get(6)?,
        codec: row.get(7)?,
        width: row.get(8)?,
        height: row.get(9)?,
        size_bytes: row.get(10)?,
        thumbnail_path: row.get(11)?,
        favorite: row.get::<_, i64>(12)? != 0,
        play_count: row.get(13)?,
        last_played_at: row.get(14)?,
        added_at: row.get(15)?,
    })
}

impl Repository {
    pub fn upsert_media(conn: &Connection, media: &NewMedia) -> Result<(i64, bool)> {
        let existing: Option<(i64, String)> = conn
            .query_row(
                "SELECT id, fingerprint FROM media WHERE path = ?1",
                params![media.path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        if let Some((id, fingerprint)) = existing {
            if fingerprint != media.fingerprint {
                conn.execute(
                    "UPDATE media SET kind=?2, title=?3, artist=?4, album=?5, duration_ms=?6, \
                     codec=?7, width=?8, height=?9, size_bytes=?10, fingerprint=?11 WHERE id=?1",
                    params![
                        id,
                        media.kind.as_str(),
                        media.title,
                        media.artist,
                        media.album,
                        media.duration_ms,
                        media.codec,
                        media.width,
                        media.height,
                        media.size_bytes,
                        media.fingerprint,
                    ],
                )?;
                return Ok((id, true));
            }
            return Ok((id, false));
        }

        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO media (path, kind, title, artist, album, duration_ms, codec, width, \
             height, size_bytes, added_at, fingerprint) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                media.path,
                media.kind.as_str(),
                media.title,
                media.artist,
                media.album,
                media.duration_ms,
                media.codec,
                media.width,
                media.height,
                media.size_bytes,
                now,
                media.fingerprint,
            ],
        )?;
        Ok((conn.last_insert_rowid(), false))
    }

    pub fn get_media(conn: &Connection, id: i64) -> Result<Option<MediaItem>> {
        let sql = format!("SELECT {MEDIA_COLUMNS} FROM media WHERE id = ?1");
        Ok(conn.query_row(&sql, params![id], row_to_media).optional()?)
    }

    pub fn get_media_by_path(conn: &Connection, path: &str) -> Result<Option<MediaItem>> {
        let sql = format!("SELECT {MEDIA_COLUMNS} FROM media WHERE path = ?1");
        Ok(conn.query_row(&sql, params![path], row_to_media).optional()?)
    }

    pub fn list_media(conn: &Connection, filter: &MediaFilter) -> Result<Vec<MediaItem>> {
        let mut sql = format!("SELECT {MEDIA_COLUMNS} FROM media WHERE 1=1");
        let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(kind) = &filter.kind {
            if kind != "all" {
                sql.push_str(" AND kind = ?");
                values.push(Box::new(kind.clone()));
            }
        }
        if let Some(favorite) = filter.favorite {
            sql.push_str(" AND favorite = ?");
            values.push(Box::new(i64::from(favorite)));
        }
        if let Some(search) = &filter.search {
            let term = search.trim();
            if !term.is_empty() {
                sql.push_str(
                    " AND (title LIKE ? ESCAPE '\\' OR artist LIKE ? ESCAPE '\\' \
                     OR album LIKE ? ESCAPE '\\' OR path LIKE ? ESCAPE '\\')",
                );
                let pattern = format!("%{}%", escape_like(term));
                for _ in 0..4 {
                    values.push(Box::new(pattern.clone()));
                }
            }
        }

        sql.push_str(" ORDER BY added_at DESC");
        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            values.push(Box::new(limit));
        } else {
            sql.push_str(" LIMIT 5000");
        }
        if let Some(offset) = filter.offset {
            sql.push_str(" OFFSET ?");
            values.push(Box::new(offset));
        }

        let refs: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(refs.as_slice(), row_to_media)?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn get_recent(conn: &Connection, limit: i64) -> Result<Vec<MediaItem>> {
        let sql = format!(
            "SELECT {MEDIA_COLUMNS} FROM media WHERE last_played_at IS NOT NULL \
             ORDER BY last_played_at DESC LIMIT ?1"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![limit], row_to_media)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn clear_recent(conn: &Connection) -> Result<()> {
        conn.execute(
            "UPDATE media SET last_played_at = NULL WHERE last_played_at IS NOT NULL",
            [],
        )?;
        Ok(())
    }

    pub fn remove_from_recent(conn: &Connection, id: i64) -> Result<()> {
        conn.execute(
            "UPDATE media SET last_played_at = NULL WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn set_favorite(conn: &Connection, id: i64, favorite: bool) -> Result<()> {
        conn.execute(
            "UPDATE media SET favorite = ?2 WHERE id = ?1",
            params![id, i64::from(favorite)],
        )?;
        Ok(())
    }

    pub fn set_thumbnail(conn: &Connection, id: i64, path: Option<&str>) -> Result<()> {
        conn.execute(
            "UPDATE media SET thumbnail_path = ?2 WHERE id = ?1",
            params![id, path],
        )?;
        Ok(())
    }

    pub fn record_play(conn: &Connection, id: i64) -> Result<()> {
        conn.execute(
            "UPDATE media SET play_count = play_count + 1, last_played_at = ?2 WHERE id = ?1",
            params![id, chrono::Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn delete_media(conn: &Connection, id: i64) -> Result<()> {
        conn.execute("DELETE FROM media WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Deletes every library entry of a kind and returns the removed rows so
    /// the caller can clean up their thumbnail files.
    pub fn clear_media_by_kind(conn: &Connection, kind: MediaKind) -> Result<Vec<MediaItem>> {
        let filter = MediaFilter {
            kind: Some(kind.as_str().to_string()),
            favorite: None,
            search: None,
            limit: None,
            offset: None,
        };
        let items = Self::list_media(conn, &filter)?;
        conn.execute("DELETE FROM media WHERE kind = ?1", params![kind.as_str()])?;
        Ok(items)
    }

    /// Removes library entries whose files no longer exist on disk.
    /// Scanned `keep` paths are ignored on purpose: a single-folder scan must
    /// never purge media that lives elsewhere.
    pub fn remove_missing(conn: &Connection, _keep: &[String]) -> Result<usize> {
        let mut removed = 0usize;
        let mut stmt = conn.prepare("SELECT id, path FROM media")?;
        let rows: Vec<(i64, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        for (id, path) in rows {
            if !std::path::Path::new(&path).exists() {
                conn.execute("DELETE FROM media WHERE id = ?1", params![id])?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    #[allow(dead_code)]
    pub fn path_exists(conn: &Connection, path: &str) -> Result<bool> {
        Ok(conn
            .query_row("SELECT 1 FROM media WHERE path = ?1", params![path], |_| Ok(()))
            .optional()?
            .is_some())
    }

    // --- Playlists ---------------------------------------------------------

    pub fn list_playlists(conn: &Connection) -> Result<Vec<Playlist>> {
        let mut stmt = conn.prepare(
            "SELECT p.id, p.name, p.created_at, p.updated_at, p.cover_path, \
             (SELECT COUNT(*) FROM playlist_tracks t WHERE t.playlist_id = p.id) \
             FROM playlists p ORDER BY p.updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Playlist {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                cover_path: row.get(4)?,
                track_count: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn create_playlist(conn: &Connection, name: &str) -> Result<i64> {
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO playlists (name, created_at, updated_at) VALUES (?1, ?2, ?3)",
            params![name, now, now],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn rename_playlist(conn: &Connection, id: i64, name: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE playlists SET name = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, name, now],
        )?;
        Ok(())
    }

    pub fn delete_playlist(conn: &Connection, id: i64) -> Result<()> {
        conn.execute("DELETE FROM playlists WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn add_to_playlist(conn: &Connection, playlist_id: i64, media_id: i64) -> Result<()> {
        let position: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |row| row.get(0),
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO playlist_tracks (playlist_id, media_id, position) \
             VALUES (?1, ?2, ?3)",
            params![playlist_id, media_id, position],
        )?;
        Self::touch_playlist(conn, playlist_id)?;
        Ok(())
    }

    pub fn remove_from_playlist(conn: &Connection, playlist_id: i64, media_id: i64) -> Result<()> {
        conn.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND media_id = ?2",
            params![playlist_id, media_id],
        )?;
        Self::touch_playlist(conn, playlist_id)?;
        Ok(())
    }

    pub fn reorder_playlist(
        conn: &Connection,
        playlist_id: i64,
        ordered_ids: &[i64],
    ) -> Result<()> {
        let tx = conn.unchecked_transaction()?;
        for (position, media_id) in ordered_ids.iter().enumerate() {
            tx.execute(
                "UPDATE playlist_tracks SET position = ?3 WHERE playlist_id = ?1 AND media_id = ?2",
                params![playlist_id, media_id, position as i64],
            )?;
        }
        tx.commit()?;
        Self::touch_playlist(conn, playlist_id)?;
        Ok(())
    }

    pub fn playlist_tracks(conn: &Connection, playlist_id: i64) -> Result<Vec<PlaylistTrack>> {
        let sql = format!(
            "SELECT {md}, t.position FROM playlist_tracks t \
             JOIN media m ON m.id = t.media_id WHERE t.playlist_id = ?1 ORDER BY t.position",
            md = MEDIA_COLUMNS
                .split(", ")
                .map(|c| format!("m.{c}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![playlist_id], |row| {
            Ok(PlaylistTrack {
                media: row_to_media(row)?,
                position: row.get(16)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    #[allow(dead_code)]
    pub fn playlist_media_ids(conn: &Connection, playlist_id: i64) -> Result<Vec<i64>> {
        let mut stmt = conn.prepare(
            "SELECT media_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position",
        )?;
        let rows = stmt.query_map(params![playlist_id], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    #[allow(dead_code)]
    pub fn set_playlist_cover(conn: &Connection, id: i64, path: Option<&str>) -> Result<()> {
        conn.execute(
            "UPDATE playlists SET cover_path = ?2 WHERE id = ?1",
            params![id, path],
        )?;
        Ok(())
    }

    fn touch_playlist(conn: &Connection, id: i64) -> Result<()> {
        conn.execute(
            "UPDATE playlists SET updated_at = ?2 WHERE id = ?1",
            params![id, chrono::Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    // --- Settings ----------------------------------------------------------

    pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
        Ok(conn
            .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| row.get(0))
            .optional()?)
    }

    pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // --- Lyrics ------------------------------------------------------------

    pub fn get_lyrics(conn: &Connection, media_id: i64) -> Result<Option<String>> {
        Ok(conn
            .query_row(
                "SELECT json_data FROM lyrics WHERE media_id = ?1",
                params![media_id],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn save_lyrics(
        conn: &Connection,
        media_id: i64,
        track_name: &str,
        artist_name: &str,
        synced: bool,
        plain_text: &str,
        json_data: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO lyrics (media_id, track_name, artist_name, synced, plain_text, json_data, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(media_id) DO UPDATE SET \
                track_name = excluded.track_name, \
                artist_name = excluded.artist_name, \
                synced = excluded.synced, \
                plain_text = excluded.plain_text, \
                json_data = excluded.json_data, \
                updated_at = excluded.updated_at",
            params![
                media_id,
                track_name,
                artist_name,
                if synced { 1 } else { 0 },
                plain_text,
                json_data,
                now
            ],
        )?;
        Ok(())
    }

    pub fn search_media_by_lyrics(conn: &Connection, query: &str) -> Result<Vec<i64>> {
        let term = query.trim();
        if term.is_empty() {
            return Ok(Vec::new());
        }
        let pattern = format!("%{}%", escape_like(term));
        let mut stmt = conn.prepare(
            "SELECT media_id FROM lyrics WHERE plain_text LIKE ?1 ESCAPE '\\' \
             ORDER BY updated_at DESC LIMIT 50",
        )?;
        let rows = stmt.query_map(params![pattern], |row| row.get(0))?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row?);
        }
        Ok(ids)
    }

    #[allow(dead_code)]
    pub fn count_media(conn: &Connection) -> Result<i64> {
        Ok(conn.query_row("SELECT COUNT(*) FROM media", [], |row| row.get(0))?)
    }

    pub fn fingerprint(path: &Path, size: u64, modified: u64) -> String {
        format!("{}|{}|{}", path.to_string_lossy(), size, modified)
    }
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
