//! Strict workspace isolation.
//!
//! Each workspace owns a separate SQLCipher-encrypted SQLite file. The global
//! registry (`app.db`) contains only workspace metadata and machine-wide
//! settings, so no library query can accidentally cross a workspace boundary
//! just because a `WHERE` clause was forgotten.

use crate::db::{Repository, schema};
use crate::error::{AppError, Result};
use crate::models::AppSettings;
use rusqlite::{Connection, params};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_WORKSPACE_ID: &str = "default";
const WMK_LEN: usize = 32;

/// Settings that belong to the machine, not to a workspace. Everything else
/// lives in the workspace database and is encrypted with it.
pub const GLOBAL_SETTING_KEYS: &[&str] = &[
    "theme",
    "startMinimized",
    "minimizeToTray",
    "playerWidget",
    "keepWidgetOnClose",
    "playerWidgetMinOpacity",
    "playerWidgetMaxOpacity",
    "playerWidgetX",
    "playerWidgetY",
    "customExtensions",
    "taskbarControls",
    "transparentFilterDialog",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub locked: bool,
    pub db_encrypted: bool,
    /// True when this workspace has an optional unlock password. When set,
    /// startup asks for it instead of relying only on Windows account protection.
    pub password_protected: bool,
}

#[derive(Debug, Clone)]
pub struct WorkspaceKeys {
    pub wmk: [u8; WMK_LEN],
    /// Hex form of the SQLCipher raw key (`x'...'`).
    pub db_key_hex: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCreation {
    pub workspace: WorkspaceInfo,
    /// Base64 of the workspace master key. Shown once so the workspace can be
    /// recovered if Windows DPAPI state is lost (for example after an OS
    /// reinstall). Treat it like a password.
    pub recovery_key: String,
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn workspace_dir(data_dir: &Path, id: &str) -> PathBuf {
    data_dir.join("workspaces").join(id)
}

pub fn library_db_path(data_dir: &Path, id: &str) -> PathBuf {
    workspace_dir(data_dir, id).join("library.db")
}

pub fn open_app_db(data_dir: &Path) -> Result<Connection> {
    let conn = Connection::open(data_dir.join("app.db"))?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS workspaces (
            id            TEXT PRIMARY KEY,
            name          TEXT NOT NULL,
            avatar_path   TEXT,
            created_at    INTEGER NOT NULL,
            updated_at    INTEGER NOT NULL,
            locked        INTEGER NOT NULL DEFAULT 0,
            db_encrypted  INTEGER NOT NULL DEFAULT 1,
            lock_salt     BLOB,
            lock_verifier TEXT,
            wrapped_wmk   BLOB NOT NULL,
            password_salt BLOB,
            password_wrapped_wmk BLOB
        );

        CREATE TABLE IF NOT EXISTS global_settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        "#,
    )?;
    // Older registries predate optional workspace passwords.
    for column in ["password_salt BLOB", "password_wrapped_wmk BLOB"] {
        let _ = conn.execute(&format!("ALTER TABLE workspaces ADD COLUMN {column}"), []);
    }
    Ok(conn)
}

fn row_to_info(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkspaceInfo> {
    Ok(WorkspaceInfo {
        id: row.get(0)?,
        name: row.get(1)?,
        avatar_path: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        locked: row.get::<_, i64>(5)? != 0,
        db_encrypted: row.get::<_, i64>(6)? != 0,
        password_protected: row.get::<_, Option<Vec<u8>>>(8)?.is_some(),
    })
}

pub fn get_workspace(conn: &Connection, id: &str) -> Result<Option<WorkspaceInfo>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, avatar_path, created_at, updated_at, locked, db_encrypted, \
         password_salt, password_wrapped_wmk FROM workspaces WHERE id = ?1",
    )?;
    let found = stmt.query_row([id], row_to_info).optional()?;
    Ok(found)
}

pub fn list_workspaces(conn: &Connection) -> Result<Vec<WorkspaceInfo>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, avatar_path, created_at, updated_at, locked, db_encrypted, \
         password_salt, password_wrapped_wmk FROM workspaces ORDER BY created_at",
    )?;
    let rows = stmt.query_map([], row_to_info)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn derive_db_key(wmk: &[u8; WMK_LEN], workspace_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"slideup.workspace.db.v1");
    hasher.update(wmk);
    hasher.update(workspace_id.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn open_library_db(data_dir: &Path, id: &str, keys: &WorkspaceKeys) -> Result<Connection> {
    let path = library_db_path(data_dir, id);
    fs::create_dir_all(path.parent().ok_or_else(|| AppError::native("Invalid workspace path."))?)?;
    let conn = Connection::open(&path)?;
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", keys.db_key_hex))?;
    schema::migrate(&conn)?;
    Ok(conn)
}

fn wrap_key(plain: &[u8]) -> Result<Vec<u8>> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{GetLastError, LocalFree};
        use windows_sys::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptProtectData};

        unsafe {
            let input = CRYPT_INTEGER_BLOB {
                cbData: plain.len() as u32,
                pbData: plain.as_ptr() as *mut u8,
            };
            let mut output: CRYPT_INTEGER_BLOB = std::mem::zeroed();
            let ok = CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                &mut output,
            );
            if ok == 0 {
                return Err(AppError::native(format!(
                    "Windows could not protect the workspace key (error {}).",
                    GetLastError()
                )));
            }
            let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            LocalFree(output.pbData as _);
            Ok(bytes)
        }
    }
    #[cfg(not(windows))]
    {
        let _ = plain;
        Err(AppError::native(
            "Encrypted workspaces are currently supported only on Windows.",
        ))
    }
}

fn unwrap_key(wrapped: &[u8]) -> Result<Vec<u8>> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{GetLastError, LocalFree};
        use windows_sys::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptUnprotectData};

        unsafe {
            let input = CRYPT_INTEGER_BLOB {
                cbData: wrapped.len() as u32,
                pbData: wrapped.as_ptr() as *mut u8,
            };
            let mut output: CRYPT_INTEGER_BLOB = std::mem::zeroed();
            let ok = CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                &mut output,
            );
            if ok == 0 {
                return Err(AppError::native(format!(
                    "This workspace key cannot be unlocked on this Windows account (error {}). Use its recovery key.",
                    GetLastError()
                )));
            }
            let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            LocalFree(output.pbData as _);
            Ok(bytes)
        }
    }
    #[cfg(not(windows))]
    {
        let _ = wrapped;
        Err(AppError::native(
            "Encrypted workspaces are currently supported only on Windows.",
        ))
    }
}

pub fn load_keys(conn: &Connection, id: &str) -> Result<WorkspaceKeys> {
    let wrapped: Vec<u8> = conn.query_row(
        "SELECT wrapped_wmk FROM workspaces WHERE id = ?1",
        [id],
        |row| row.get(0),
    )?;
    let plain = unwrap_key(&wrapped)?;
    let array: [u8; WMK_LEN] = plain
        .as_slice()
        .try_into()
        .map_err(|_| AppError::native("Stored workspace key is invalid."))?;
    Ok(WorkspaceKeys {
        db_key_hex: derive_db_key(&array, id),
        wmk: array,
    })
}

/// Placeholder used while a password-protected workspace is locked. It can
/// never open a database; `AppState` keeps no active workspace in that state.
pub fn blank_keys(id: &str) -> WorkspaceKeys {
    let wmk = [0u8; WMK_LEN];
    WorkspaceKeys {
        wmk,
        db_key_hex: derive_db_key(&wmk, id),
    }
}

pub fn encode_recovery_key(wmk: &[u8; WMK_LEN]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(wmk)
}

fn decode_recovery_key(key: &str) -> Result<[u8; WMK_LEN]> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(key.trim())
        .map_err(|_| AppError::native("The recovery key is not valid."))?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| AppError::native("The recovery key has the wrong length."))
}

pub(crate) fn fill_random(bytes: &mut [u8]) -> Result<()> {
    use rand::TryRngCore;
    rand::rngs::OsRng
        .try_fill_bytes(bytes)
        .map_err(|err| AppError::native(format!("Could not generate a workspace key: {err}")))
}

/// Verify the credential protecting a workspace without opening its database.
/// Used by "reveal private playlists": a password-protected workspace requires
/// the password, otherwise the Windows account (DPAPI) is the credential.
pub fn verify_workspace_credential(
    app_db: &Connection,
    id: &str,
    password: Option<&str>,
) -> Result<bool> {
    let row: Option<(Option<Vec<u8>>, Option<Vec<u8>>, Option<Vec<u8>>)> = app_db
        .query_row(
            "SELECT password_salt, password_wrapped_wmk, wrapped_wmk FROM workspaces WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((salt, password_wrapped, dpapi_wrapped)) = row else {
        return Ok(false);
    };
    if let (Some(salt), Some(wrapped)) = (salt, password_wrapped) {
        let Some(password) = password else {
            return Ok(false);
        };
        return Ok(decrypt_wmk(&wrapped, password, &salt, id).is_ok());
    }
    Ok(dpapi_wrapped
        .and_then(|wrapped| unwrap_key(&wrapped).ok())
        .is_some())
}

/// Optional per-workspace password. It wraps the same workspace master key,
/// so the encrypted library can be opened without Windows DPAPI if needed.
fn password_key(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let mut key = [0u8; 32];
    argon2::Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|_| AppError::native("Could not derive the workspace password key."))?;
    Ok(key)
}

fn password_cipher(password: &str, salt: &[u8]) -> Result<aes_gcm::Aes256Gcm> {
    use aes_gcm::KeyInit;
    let key = password_key(password, salt)?;
    aes_gcm::Aes256Gcm::new_from_slice(&key)
        .map_err(|_| AppError::native("Could not initialize the workspace cipher."))
}

fn encrypt_wmk(wmk: &[u8; WMK_LEN], password: &str, id: &str) -> Result<(Vec<u8>, Vec<u8>)> {
    use aes_gcm::aead::{Aead, Payload};
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 12];
    fill_random(&mut salt)?;
    fill_random(&mut nonce)?;
    let cipher = password_cipher(password, &salt)?;
    let encrypted = cipher
        .encrypt(
            aes_gcm::Nonce::from_slice(&nonce),
            Payload {
                msg: wmk,
                aad: id.as_bytes(),
            },
        )
        .map_err(|_| AppError::native("Could not protect the workspace key."))?;
    let mut wrapped = nonce.to_vec();
    wrapped.extend_from_slice(&encrypted);
    Ok((salt.to_vec(), wrapped))
}

fn decrypt_wmk(wrapped: &[u8], password: &str, salt: &[u8], id: &str) -> Result<[u8; WMK_LEN]> {
    use aes_gcm::aead::{Aead, Payload};
    if wrapped.len() < 12 {
        return Err(AppError::native("Stored workspace password data is invalid."));
    }
    let cipher = password_cipher(password, salt)?;
    let plain = cipher
        .decrypt(
            aes_gcm::Nonce::from_slice(&wrapped[..12]),
            Payload {
                msg: &wrapped[12..],
                aad: id.as_bytes(),
            },
        )
        .map_err(|_| AppError::native("Incorrect workspace password."))?;
    plain
        .as_slice()
        .try_into()
        .map_err(|_| AppError::native("Stored workspace key is invalid."))
}

pub fn set_workspace_password(
    conn: &Connection,
    id: &str,
    wmk: &[u8; WMK_LEN],
    password: &str,
) -> Result<()> {
    if password.len() < 4 {
        return Err(AppError::native("Use a password of at least 4 characters."));
    }
    let (salt, wrapped) = encrypt_wmk(wmk, password, id)?;
    // Neutralise the machine-level wrapped key: while a password is set,
    // DPAPI must not be able to unlock this workspace. The column is NOT NULL,
    // so store an empty blob as a sentinel that fails DPAPI unwrap.
    conn.execute(
        "UPDATE workspaces SET password_salt = ?2, password_wrapped_wmk = ?3, \
         wrapped_wmk = ?5, updated_at = ?4 WHERE id = ?1",
        params![id, salt, wrapped, now_ms(), Vec::<u8>::new()],
    )?;
    Ok(())
}

pub fn clear_workspace_password(
    conn: &Connection,
    id: &str,
    wmk: &[u8; WMK_LEN],
    password: &str,
) -> Result<()> {
    let (salt, wrapped): (Vec<u8>, Vec<u8>) = conn.query_row(
        "SELECT password_salt, password_wrapped_wmk FROM workspaces WHERE id = ?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let recovered = decrypt_wmk(&wrapped, password, &salt, id)?;
    if recovered != *wmk {
        return Err(AppError::native("Incorrect workspace password."));
    }
    // Restore the machine-level fallback before removing the password.
    let machine = wrap_key(wmk)?;
    conn.execute(
        "UPDATE workspaces SET password_salt = NULL, password_wrapped_wmk = NULL, \
         wrapped_wmk = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, machine, now_ms()],
    )?;
    Ok(())
}

/// Unlock a workspace with its optional password, without relying on DPAPI.
pub fn unlock_with_password(
    app_db: &Connection,
    data_dir: &Path,
    id: &str,
    password: &str,
) -> Result<(WorkspaceInfo, WorkspaceKeys)> {
    let (salt, wrapped): (Vec<u8>, Vec<u8>) = conn_query_password(app_db, id)?;
    let wmk = decrypt_wmk(&wrapped, password, &salt, id)?;
    let keys = WorkspaceKeys {
        wmk,
        db_key_hex: derive_db_key(&wmk, id),
    };
    // Verify the key actually opens this workspace's database before unlocking.
    let probe = open_library_db(data_dir, id, &keys)?;
    drop(probe);
    let info = get_workspace(app_db, id)?
        .ok_or_else(|| AppError::native("This workspace no longer exists."))?;
    Ok((info, keys))
}

fn conn_query_password(conn: &Connection, id: &str) -> Result<(Vec<u8>, Vec<u8>)> {
    let found = conn
        .query_row(
            "SELECT password_salt, password_wrapped_wmk FROM workspaces WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    found.ok_or_else(|| AppError::native("This workspace has no password configured."))
}

pub fn set_locked(conn: &Connection, id: &str, locked: bool) -> Result<()> {
    conn.execute(
        "UPDATE workspaces SET locked = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, if locked { 1 } else { 0 }, now_ms()],
    )?;
    Ok(())
}

/// Convert the legacy plaintext `slideup.db` into an encrypted workspace DB.
/// `sqlcipher_export` rewrites every page through the cipher; a raw rename
/// would leave an unreadable file.
fn migrate_legacy_database(data_dir: &Path, id: &str, keys: &WorkspaceKeys) -> Result<Option<String>> {
    let legacy = data_dir.join("slideup.db");
    let target = library_db_path(data_dir, id);
    if !legacy.exists() || target.exists() {
        return Ok(None);
    }
    // ATTACH cannot create a missing parent directory, but the workspace
    // folder does not exist yet on a first run. Create it before attaching.
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let source = Connection::open(&legacy)?;
    let old_settings = Repository::get_setting(&source, "app")?;
    let target_text = target.to_string_lossy().replace('\'', "''");
    source.execute_batch(&format!(
        "ATTACH DATABASE '{target_text}' AS encrypted KEY \"x'{}'\"; \
         SELECT sqlcipher_export('encrypted'); \
         DETACH DATABASE encrypted;",
        keys.db_key_hex
    ))?;
    drop(source);
    let _ = fs::rename(&legacy, data_dir.join("slideup.db.legacy"));
    Ok(old_settings)
}

/// Create the default workspace on first launch, including data migration.
pub fn initialize_default(app_db: &Connection, data_dir: &Path) -> Result<WorkspaceInfo> {
    if let Some(info) = get_workspace(app_db, DEFAULT_WORKSPACE_ID)? {
        return Ok(info);
    }

    let mut wmk = [0u8; WMK_LEN];
    fill_random(&mut wmk)?;
    let keys = WorkspaceKeys {
        wmk,
        db_key_hex: derive_db_key(&wmk, DEFAULT_WORKSPACE_ID),
    };
    let old_settings = migrate_legacy_database(data_dir, DEFAULT_WORKSPACE_ID, &keys)?;
    {
        let conn = open_library_db(data_dir, DEFAULT_WORKSPACE_ID, &keys)?;
        if let Some(json) = old_settings {
            let (global, local) = split_settings_json(&json)?;
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('app', ?1)",
                [&local],
            )?;
            app_db.execute(
                "INSERT OR REPLACE INTO global_settings (key, value) VALUES ('app', ?1)",
                [&global],
            )?;
        }
    }
    let created = now_ms();
    app_db.execute(
        "INSERT INTO workspaces (id, name, created_at, updated_at, locked, db_encrypted, wrapped_wmk) \
         VALUES (?1, ?2, ?3, ?4, 0, 1, ?5)",
        params![
            DEFAULT_WORKSPACE_ID,
            "My Library",
            created,
            created,
            wrap_key(&wmk)?
        ],
    )?;
    Ok(get_workspace(app_db, DEFAULT_WORKSPACE_ID)?.ok_or_else(|| {
        AppError::native("The default workspace could not be created.")
    })?)
}

pub fn create_workspace(
    app_db: &Connection,
    data_dir: &Path,
    name: &str,
) -> Result<WorkspaceCreation> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::native("Enter a name for the workspace."));
    }
    let id = format!("ws-{}", chrono::Utc::now().timestamp_millis());
    if get_workspace(app_db, &id)?.is_some() {
        return Err(AppError::native("Could not create a unique workspace id."));
    }
    let mut wmk = [0u8; WMK_LEN];
    fill_random(&mut wmk)?;
    let keys = WorkspaceKeys {
        wmk,
        db_key_hex: derive_db_key(&wmk, &id),
    };
    open_library_db(data_dir, &id, &keys)?;
    let created = now_ms();
    app_db.execute(
        "INSERT INTO workspaces (id, name, created_at, updated_at, locked, db_encrypted, wrapped_wmk) \
         VALUES (?1, ?2, ?3, ?4, 0, 1, ?5)",
        params![id, name, created, created, wrap_key(&wmk)?],
    )?;
    Ok(WorkspaceCreation {
        workspace: get_workspace(app_db, &id)?.ok_or_else(|| {
            AppError::native("The workspace could not be created.")
        })?,
        recovery_key: encode_recovery_key(&wmk),
    })
}

pub fn restore_workspace(
    app_db: &Connection,
    data_dir: &Path,
    recovery_key: &str,
) -> Result<WorkspaceInfo> {
    let wmk = decode_recovery_key(recovery_key)?;
    let rows: Vec<String> = {
        let mut stmt = app_db.prepare("SELECT id FROM workspaces")?;
        let iter = stmt.query_map([], |row| row.get::<_, String>(0))?;
        iter.filter_map(|row| row.ok()).collect()
    };
    for id in rows {
        let current: Vec<u8> = app_db.query_row(
            "SELECT wrapped_wmk FROM workspaces WHERE id = ?1",
            [&id],
            |row| row.get(0),
        )?;
        // A wrapped key that cannot be opened on this account is the signal
        // that this is the workspace the recovery key belongs to.
        if unwrap_key(&current).is_ok() {
            continue;
        }
        let candidate = open_library_db(data_dir, &id, &WorkspaceKeys {
            wmk,
            db_key_hex: derive_db_key(&wmk, &id),
        });
        if candidate.is_err() {
            continue;
        }
        app_db.execute(
            "UPDATE workspaces SET wrapped_wmk = ?2, locked = 0, updated_at = ?3 WHERE id = ?1",
            params![id, wrap_key(&wmk)?, now_ms()],
        )?;
        let mut info = get_workspace(app_db, &id)?.ok_or_else(|| {
            AppError::native("The workspace could not be restored.")
        })?;
        info.locked = false;
        return Ok(info);
    }
    Err(AppError::native("No workspace matches this recovery key."))
}

pub fn rename_workspace(conn: &Connection, id: &str, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::native("Enter a name for the workspace."));
    }
    conn.execute(
        "UPDATE workspaces SET name = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, name, now_ms()],
    )?;
    Ok(())
}

/// Copy a user-selected image into the workspace directory and use it as the
/// workspace avatar. Only small image-like extensions are accepted.
pub fn set_avatar(conn: &Connection, data_dir: &Path, id: &str, source: &str) -> Result<String> {
    let source = Path::new(source);
    if !source.is_file() {
        return Err(AppError::native("The selected avatar file does not exist."));
    }
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let extension = if ["png", "jpg", "jpeg", "webp"].contains(&extension.as_str()) {
        extension
    } else {
        "png".to_string()
    };

    let directory = workspace_dir(data_dir, id);
    fs::create_dir_all(&directory)?;
    if let Ok(entries) = fs::read_dir(&directory) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("avatar.") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    let target = directory.join(format!("avatar.{extension}"));
    fs::copy(source, &target)?;
    let path = target.to_string_lossy().to_string();
    conn.execute(
        "UPDATE workspaces SET avatar_path = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, path, now_ms()],
    )?;
    Ok(path)
}

pub fn clear_avatar(conn: &Connection, data_dir: &Path, id: &str) -> Result<()> {
    let directory = workspace_dir(data_dir, id);
    if let Ok(entries) = fs::read_dir(&directory) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with("avatar.") {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    conn.execute(
        "UPDATE workspaces SET avatar_path = NULL, updated_at = ?2 WHERE id = ?1",
        params![id, now_ms()],
    )?;
    Ok(())
}

pub fn delete_workspace(conn: &Connection, data_dir: &Path, id: &str) -> Result<()> {
    if id == DEFAULT_WORKSPACE_ID {
        return Err(AppError::native("The default workspace cannot be deleted."));
    }
    conn.execute("DELETE FROM workspaces WHERE id = ?1", [id])?;
    let _ = fs::remove_dir_all(workspace_dir(data_dir, id));
    Ok(())
}

pub fn set_last_active(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO global_settings (key, value) VALUES ('lastWorkspace', ?1)",
        [id],
    )?;
    Ok(())
}

pub fn last_active(conn: &Connection) -> Result<Option<String>> {
    let found = conn
        .query_row(
            "SELECT value FROM global_settings WHERE key = 'lastWorkspace'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(found)
}

pub fn split_settings_json(json: &str) -> Result<(String, String)> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    let mut object = value
        .as_object()
        .cloned()
        .ok_or_else(|| AppError::native("Stored settings are invalid."))?;
    let mut global = serde_json::Map::new();
    for key in GLOBAL_SETTING_KEYS {
        if let Some(value) = object.remove(*key) {
            global.insert((*key).to_string(), value);
        }
    }
    Ok((
        serde_json::to_string(&serde_json::Value::Object(global))?,
        serde_json::to_string(&serde_json::Value::Object(object))?,
    ))
}

pub fn split_settings(settings: &AppSettings) -> Result<(String, String)> {
    let json = serde_json::to_string(settings)?;
    split_settings_json(&json)
}

pub fn merge_settings(global: Option<&str>, local: Option<&str>) -> Result<AppSettings> {
    let mut object = local
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    if let Some(json) = global {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
            if let Some(map) = value.as_object() {
                for (key, value) in map {
                    object.insert(key.clone(), value.clone());
                }
            }
        }
    }
    Ok(serde_json::from_value(serde_json::Value::Object(object))?)
}

pub fn zeroize(keys: &mut WorkspaceKeys) {
    for byte in keys.wmk.iter_mut() {
        *byte = 0;
    }
    keys.db_key_hex.clear();
}

use rusqlite::OptionalExtension;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AppSettings;

    #[test]
    fn settings_split_and_merge_round_trip() {
        let original = AppSettings::default();
        let full = serde_json::to_string(&original).unwrap();
        let (global, local) = split_settings_json(&full).unwrap();

        let global_value: serde_json::Value = serde_json::from_str(&global).unwrap();
        let local_value: serde_json::Value = serde_json::from_str(&local).unwrap();
        assert!(global_value.get("theme").is_some());
        assert!(global_value.get("startMinimized").is_some());
        assert!(global_value.get("volume").is_none());
        assert!(local_value.get("volume").is_some());

        let merged = merge_settings(Some(&global), Some(&local)).unwrap();
        assert_eq!(
            serde_json::to_value(&merged).unwrap(),
            serde_json::to_value(&original).unwrap()
        );
    }

    #[test]
    fn database_key_is_bound_to_workspace_and_master_key() {
        let first = [7u8; WMK_LEN];
        let second = [8u8; WMK_LEN];
        let key = derive_db_key(&first, "one");
        assert_eq!(key.len(), 64);
        assert_ne!(key, derive_db_key(&first, "two"));
        assert_ne!(key, derive_db_key(&second, "one"));

        // Zeroizing a stored key clears its bytes.
        let mut keys = WorkspaceKeys {
            wmk: first,
            db_key_hex: derive_db_key(&first, "one"),
        };
        zeroize(&mut keys);
        assert!(keys.wmk.iter().all(|byte| *byte == 0));
        assert!(keys.db_key_hex.is_empty());
    }
}
