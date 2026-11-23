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
            cover_path  TEXT
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
    )
}
