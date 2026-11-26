use rusqlite::Connection;

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS media (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            path            TEXT    NOT NULL UNIQUE,
            kind            TEXT    NOT NULL,
            title           TEXT    NOT NULL,
            artist          TEXT,
            album           TEXT,
            duration_ms     INTEGER NOT NULL DEFAULT 0,
            codec           TEXT,
            width           INTEGER,
            height          INTEGER,
            size_bytes      INTEGER NOT NULL DEFAULT 0,
            thumbnail_path  TEXT,
            favorite        INTEGER NOT NULL DEFAULT 0,
            play_count      INTEGER NOT NULL DEFAULT 0,
            last_played_at  INTEGER,
            added_at        INTEGER NOT NULL,
            fingerprint     TEXT    NOT NULL DEFAULT ''
        );

        CREATE INDEX IF NOT EXISTS idx_media_kind ON media(kind);
        CREATE INDEX IF NOT EXISTS idx_media_favorite ON media(favorite);
        CREATE INDEX IF NOT EXISTS idx_media_title ON media(title COLLATE NOCASE);
        CREATE INDEX IF NOT EXISTS idx_media_last_played ON media(last_played_at DESC);
        CREATE INDEX IF NOT EXISTS idx_media_fingerprint ON media(fingerprint);

        CREATE TABLE IF NOT EXISTS playlists (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            created_at  INTEGER NOT NULL,
            updated_at  INTEGER NOT NULL,
            cover_path  TEXT,
            locked      INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS playlist_tracks (
            playlist_id INTEGER NOT NULL,
            media_id    INTEGER NOT NULL,
            position    INTEGER NOT NULL,
            PRIMARY KEY (playlist_id, media_id),
            FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
            FOREIGN KEY (media_id) REFERENCES media(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_playlist_tracks_order
            ON playlist_tracks(playlist_id, position);

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS lyrics (
            media_id    INTEGER PRIMARY KEY,
            track_name  TEXT NOT NULL DEFAULT '',
            artist_name TEXT NOT NULL DEFAULT '',
            synced      INTEGER NOT NULL DEFAULT 0,
            plain_text  TEXT NOT NULL DEFAULT '',
            json_data   TEXT NOT NULL,
            updated_at  INTEGER NOT NULL,
            FOREIGN KEY (media_id) REFERENCES media(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_lyrics_plain_text ON lyrics(plain_text COLLATE NOCASE);
        "#,
    )?;

    // Older workspace databases created before playlist privacy need the new
    // column. Check first so repeated migrations stay cheap and idempotent.
    let has_locked = conn
        .prepare("PRAGMA table_info(playlists)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .any(|name| name.map(|name| name == "locked").unwrap_or(false));
    if !has_locked {
        conn.execute("ALTER TABLE playlists ADD COLUMN locked INTEGER NOT NULL DEFAULT 0", [])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_locked_column(conn: &Connection) -> bool {
        conn.prepare("PRAGMA table_info(playlists)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .any(|name| name.map(|name| name == "locked").unwrap_or(false))
    }

    #[test]
    fn migration_adds_playlist_locked_column_idempotently() {
        let conn = Connection::open_in_memory().unwrap();
        // A database created before playlist privacy existed.
        conn.execute_batch(
            "CREATE TABLE playlists (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                cover_path TEXT
             );",
        )
        .unwrap();

        migrate(&conn).unwrap();
        assert!(has_locked_column(&conn));

        conn.execute(
            "INSERT INTO playlists (name, created_at, updated_at) VALUES ('Legacy', 0, 0)",
            [],
        )
        .unwrap();
        let locked: i64 = conn
            .query_row("SELECT locked FROM playlists WHERE name = 'Legacy'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(locked, 0);

        // Running the migration again must not fail or duplicate the column.
        migrate(&conn).unwrap();
        assert!(has_locked_column(&conn));
    }
}
