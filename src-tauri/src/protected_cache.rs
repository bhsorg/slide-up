//! Encrypted cache for video images (`.hlv`).
//!
//! Video covers and storyboard frames are derived from the media itself, so
//! they are written only while a workspace is unlocked and are encrypted with
//! that workspace's key. A missing or damaged file is simply discarded and
//! regenerated on demand; other workspaces can never decrypt it.

use crate::error::{AppError, Result};
use crate::state::CacheIdentity;
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use sha2::{Digest, Sha256};
use std::path::Path;

const MAGIC: &[u8; 4] = b"HLV1";
const NONCE_LEN: usize = 12;

fn derive_key(identity: &CacheIdentity, kind: &str, media_id: i64, index: usize) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"slideup.hlv.image.v1");
    hasher.update(identity.workspace_id.as_bytes());
    hasher.update([0u8]);
    hasher.update(identity.wmk);
    hasher.update(kind.as_bytes());
    hasher.update(media_id.to_be_bytes());
    hasher.update(index.to_be_bytes());
    hasher.finalize().into()
}

fn aad(identity: &CacheIdentity, kind: &str, media_id: i64, index: usize) -> Vec<u8> {
    format!(
        "{}|{}|{}|{}",
        identity.workspace_id, kind, media_id, index
    )
    .into_bytes()
}

fn cipher(key: &[u8; 32]) -> Result<Aes256Gcm> {
    Aes256Gcm::new_from_slice(key)
        .map_err(|error| AppError::native(format!("Could not initialise image encryption: {error}")))
}

/// encrypt bytes into the `.hlv` container format.
pub fn encrypt(
    identity: &CacheIdentity,
    kind: &str,
    media_id: i64,
    index: usize,
    plaintext: &[u8],
) -> Result<Vec<u8>> {
    let key = derive_key(identity, kind, media_id, index);
    let mut nonce = [0u8; NONCE_LEN];
    crate::workspace::fill_random(&mut nonce)?;
    let payload = Payload {
        msg: plaintext,
        aad: &aad(identity, kind, media_id, index),
    };
    let ciphertext = cipher(&key)?
        .encrypt(Nonce::from_slice(&nonce), payload)
        .map_err(|_| AppError::native("Could not encrypt a cached video image."))?;
    let mut output = Vec::with_capacity(MAGIC.len() + NONCE_LEN + ciphertext.len());
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

fn decrypt(
    identity: &CacheIdentity,
    kind: &str,
    media_id: i64,
    index: usize,
    stored: &[u8],
) -> Result<Vec<u8>> {
    if stored.len() < MAGIC.len() + NONCE_LEN || &stored[..MAGIC.len()] != MAGIC {
        return Err(AppError::native("Cached image has an invalid format."));
    }
    let nonce = &stored[MAGIC.len()..MAGIC.len() + NONCE_LEN];
    let ciphertext = &stored[MAGIC.len() + NONCE_LEN..];
    let key = derive_key(identity, kind, media_id, index);
    let payload = Payload {
        msg: ciphertext,
        aad: &aad(identity, kind, media_id, index),
    };
    cipher(&key)?
        .decrypt(Nonce::from_slice(nonce), payload)
        .map_err(|_| AppError::native("Cached image could not be decrypted."))
}

/// Atomically write a protected image so a crash or lock switch never leaves a
/// partially written cache file.
pub fn write_file(
    path: &Path,
    identity: &CacheIdentity,
    kind: &str,
    media_id: i64,
    index: usize,
    plaintext: &[u8],
) -> Result<()> {
    let encrypted = encrypt(identity, kind, media_id, index, plaintext)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, encrypted)?;
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    std::fs::rename(&temporary, path)?;
    Ok(())
}

/// Read a protected image. Missing files return `None`; unreadable files are
/// deleted so the caller can regenerate them lazily.
pub fn read_file(
    path: &Path,
    identity: &CacheIdentity,
    kind: &str,
    media_id: i64,
    index: usize,
) -> Result<Option<Vec<u8>>> {
    let Ok(stored) = std::fs::read(path) else {
        return Ok(None);
    };
    match decrypt(identity, kind, media_id, index, &stored) {
        Ok(plaintext) => Ok(Some(plaintext)),
        Err(_) => {
            let _ = std::fs::remove_file(path);
            Ok(None)
        }
    }
}

/// Data URL for a cached image, sniffing the two formats mpv's image VO can
/// write. Both stay inside the encrypted container until read.
pub fn data_url(bytes: &[u8]) -> String {
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else {
        "image/jpeg"
    };
    format!(
        "data:{mime};base64,{}",
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
    )
}

/// Directory holding protected images for one workspace.
pub fn workspace_dir(data_dir: &Path, workspace_id: &str) -> std::path::PathBuf {
    data_dir.join("workspaces").join(workspace_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn identity(workspace_id: &str, byte: u8) -> CacheIdentity {
        CacheIdentity {
            workspace_id: workspace_id.to_string(),
            wmk: [byte; 32],
        }
    }

    fn temp_file(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("slideup-cache-test-{name}-{nonce}.hlv"))
    }

    #[test]
    fn roundtrip_preserves_plaintext() {
        let path = temp_file("roundtrip");
        let owner = identity("left", 1);
        let bytes = b"\x89PNG\r\n\x1a\nframe";
        write_file(&path, &owner, "cover", 42, 0, bytes).unwrap();
        assert_eq!(read_file(&path, &owner, "cover", 42, 0).unwrap().unwrap(), bytes);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn wrong_master_key_is_rejected_and_file_is_removed() {
        let path = temp_file("wrong-key");
        let owner = identity("left", 1);
        let intruder = identity("left", 2);
        write_file(&path, &owner, "cover", 42, 0, b"secret").unwrap();
        assert_eq!(read_file(&path, &intruder, "cover", 42, 0).unwrap(), None);
        assert!(!path.exists(), "unreadable cache must be discarded");
    }

    #[test]
    fn other_workspace_cannot_replay_the_file() {
        let path = temp_file("cross-workspace");
        let owner = identity("left", 1);
        // Same master key, different workspace id: AAD and key must both differ.
        let intruder = identity("right", 1);
        write_file(&path, &owner, "cover", 42, 0, b"secret").unwrap();
        assert_eq!(read_file(&path, &intruder, "cover", 42, 0).unwrap(), None);
        assert!(!path.exists());
    }

    #[test]
    fn tampered_container_is_discarded() {
        let path = temp_file("tampered");
        let owner = identity("left", 1);
        write_file(&path, &owner, "storyboard", 7, 3, b"frame").unwrap();
        let mut stored = std::fs::read(&path).unwrap();
        let last = stored.len() - 1;
        stored[last] ^= 0xff;
        std::fs::write(&path, stored).unwrap();
        assert_eq!(read_file(&path, &owner, "storyboard", 7, 3).unwrap(), None);
        assert!(!path.exists());
    }

    #[test]
    fn data_url_sniffs_png_and_jpeg() {
        assert!(data_url(b"\x89PNG\r\n\x1a\nxyz").starts_with("data:image/png;base64,"));
        assert!(data_url(b"\xff\xd8\xffxyz").starts_with("data:image/jpeg;base64,"));
    }
}
