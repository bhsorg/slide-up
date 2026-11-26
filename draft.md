# Slide Up — Strict Workspace Isolation & Protected Media Cache (Design Draft)

> Status: **All phases implemented and tested.**
> Date: 2026-11-25
> Purpose: define a design where the **whole application** is isolated per workspace,
> and reconcile that with bringing back small, encrypted video covers/storyboards.

---

## 1. Goal

The app is not "one library with workspace tags". The goal is:

> Each user gets a strict, private space. Switching workspace swaps the **entire app**:
> library, playlists, settings, history, playback state and media cache. A locked
> workspace is unreachable — not filtered out by a query, but not openable at all.

That changes the architecture fundamentally: **one database per workspace**, not one
database with `workspace_id` columns everywhere. A missed `WHERE workspace_id = ?`
would leak data; a separate database file that is closed while locked cannot.

### Honest security boundary

A client-side desktop app cannot keep a secret from the owner of the machine. "Strict"
here means:

- no other workspace's data is reachable through the running app without unlocking it;
- no plaintext video cover/storyboard is ever written to disk;
- key material exists in memory only while a workspace is unlocked;
- locking closes the workspace database, stops playback and removes workspace UI state.

It does **not** mean protection against a determined administrator who can inspect process
memory or replace the binary. If cryptographic protection of the library database itself is
wanted, that requires SQLCipher — see the open decision in section 11.

---

## 2. Goals and non-goals

### Goals
- Full per-workspace isolation of media, playlists, settings, favorites, history, lyrics,
  playback state and image cache.
- Workspace identity: avatar + display name in a switcher/profile menu.
- Workspace lock: credential-protected access; locked workspace data is not merely hidden.
- Playlist-level security for private playlists.
- Data survives workspace switches and app restarts; each workspace keeps its own state.
- Small encrypted JPEG cache for **video covers + storyboards only** (audio artwork unchanged).

### Non-goals
- Cloud sync / multi-device key escrow.
- Concurrent multi-user access to one workspace.
- Absolute secrecy from the machine owner.
- Encrypting audio artwork, lyrics or subtitle files for now.

---

## 3. Concepts

| Concept | Meaning |
|---|---|
| **Workspace** | A self-contained private space with its own database, settings and cache directory. |
| **App shell DB** | Global database holding only the workspace registry and app-wide settings. |
| **Active workspace** | The one unlocked workspace whose database is open in memory. |
| **Workspace lock** | Optional credential. When locked, its database stays closed and keys are zeroized. |
| **WMK** | Workspace Master Key — random 32 bytes generated per workspace. |
| **DB key / DEK** | Keys derived from the WMK for the workspace database and `.hlv` cache. |
| **Protected image** | AES-256-GCM encrypted JPEG with a `.hlv` extension. |

A workspace is not a Windows account: one Windows user may own several workspaces.

---

## 4. Data model — one database per workspace

### 4.1 On-disk layout

```
app_data/
  app.db                              # registry + global settings only
  workspaces/
    <workspace_id>/
      library.db                      # ALL app data for this workspace
      thumbnails/<media_id>.hlv       # encrypted video cover
      storyboards/<media_id>/<n>.hlv  # encrypted storyboard frames (0..11)
      avatar.hlv                      # optional, if avatars are protected
```

### 4.2 Global database (`app.db`)

```sql
CREATE TABLE workspaces (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    avatar_path     TEXT,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    lock_salt       BLOB,                 -- NULL when no credential is set
    lock_kdf        TEXT,                 -- e.g. "argon2id:v1"
    lock_verifier   BLOB,                 -- wrong-password detection
    wrapped_wmk     BLOB,                 -- WMK wrapped by KEK (credential or OS key)
    db_encrypted    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE global_settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL
);
```

`global_settings` holds only app-shell preferences: theme, language, window geometry,
launch-at-startup, updater preferences and the last active workspace id.

### 4.3 Workspace database (`library.db`)

The current schema moves here **unchanged in spirit** — no `workspace_id` column is
needed anywhere, because the file itself is the boundary:

```sql
media(path UNIQUE, kind, title, artist, album, duration_ms, codec, width, height,
      size_bytes, thumbnail_path, favorite, play_count, last_played_at, added_at, fingerprint);
playlists(id, name, created_at, updated_at, cover_path, locked DEFAULT 0);
playlist_tracks(playlist_id, media_id, position, ...);
lyrics(media_id, track_name, artist_name, synced, plain_text, json_data, updated_at, ...);
settings(key, value);   -- workspace-scoped settings only
```

Favorites, play counts and recently played are columns on `media`, so they are
workspace-scoped automatically. `playlist_tracks` and `lyrics` inherit scope through
their foreign keys. No cross-workspace query path exists: commands resolve the active
workspace's `Connection` from `AppState` or fail with "Workspace is locked".

### 4.4 Runtime enforcement

`AppState` holds at most one open workspace:

```rust
struct ActiveWorkspace {
    id: String,
    db: Connection,       // dropped on lock/switch/exit
    wmk: [u8; 32],
    db_key: [u8; 32],     // zeroized on lock/switch/exit
    cache_dek: [u8; 32],  // zeroized on lock/switch/exit
}
```

- Every library/playlist/settings command accesses only `ActiveWorkspace.db`.
- On switch/lock: drop the connection, zeroize keys, stop mpv, clear queue and
  in-memory stores, hide/destroy detached widgets.
- On unlock: open that workspace's `library.db`, load its settings, let the frontend
  reinitialize its stores from that database.

---

## 5. Security architecture

### 5.1 Key lifecycle

**Workspace creation**
1. Generate `WMK = random(32)`.
2. If a credential is set:
   - `salt = random(16)`;
   - `KEK = Argon2id(credential, salt)`;
   - `wrapped_wmk = AES-GCM(KEK, WMK)`;
   - store a verifier for wrong-password detection.
3. If no credential is set: wrap WMK with an OS-backed key (Windows DPAPI under the
   current user account; non-Windows fallback is an open decision).
4. Derive `DB key = HKDF(WMK, "slideup.db.v1")` and
   `cache DEK = HKDF(WMK, "slideup.hlv.v1")`.

**Unlock** — derive KEK, decrypt WMK, derive both keys, open the workspace database.

**Lock / switch / exit** — drop the DB connection, zeroize all key material, clear UI state.

If the workspace database is encrypted (SQLCipher), losing the WMK means losing that
workspace's library — so a recovery/export mechanism is mandatory before enabling it.
See section 11.

### 5.2 `.hlv` file format

```
offset  size   field
0       4      magic "HLV1"
4       1      version (1 = AES-256-GCM / JPEG)
5       12     random nonce
17      N      ciphertext || GCM tag
```

AAD = `workspace_id || kind || media_id` (`kind` = `cover` or `storyboard`), preventing
cross-workspace or cross-video replay even if files are copied around. A decrypt failure
is treated as a cache miss and regenerated lazily.

### 5.3 Playlist security

- `playlists.locked` marks a private playlist; locked playlists are hidden until the
  workspace is unlocked (later: a separate playlist PIN/key).
- M3U/M3U8 export of a locked playlist requires an explicit unlock step.
- Because the whole database is closed while the workspace is locked, playlist data is
  unreachable regardless of UI state.

---

## 6. Protected image cache

### 6.1 Generation

- `metadata::video::probe()` already decodes one frame via `vo=image`, `frames=1`;
  reuse that frame instead of discarding it — no second decode.
- Output JPEG (`vo-image-format=jpg`, fallback PNG bytes inside `.hlv` if the
  bundled mpv rejects `jpg`; extension stays `.hlv`).
- Seek time: `min(max(1s, duration × 0.10), 30s)` — avoids black intro frames.
- One cover per video; 12-frame storyboards remain opt-in.

### 6.2 Reading and display

- `<img src>` cannot read encrypted bytes, so Rust decrypts on demand and returns
  `data:image/jpeg;base64,...` via `load_protected_image(path)` and
  `load_storyboard_frame(media_id, index)`.
- Frontend keeps a small LRU of decrypted data URLs keyed by path.
- Decrypt failure deletes the file, shows the clapperboard placeholder and regenerates
  lazily — never an error dialog for a cache miss.

### 6.3 Policy

- **Generate video covers** — default ON for new workspaces, independent of storyboards.
- Missing covers are generated lazily when displayed; no silent startup backfill.
- Optional manual **Generate missing covers** action in Settings → Library.
- **Auto-generate video storyboards** stays OFF by default.
- If generation fails, keep the placeholder.

All stored paths and generated files live under the active workspace directory. Audio
artwork keeps its current format and location inside that workspace.

---

## 7. Workspace lifecycle

| Action | Behavior |
|---|---|
| Create | Generate WMK, create directory + `library.db` with current schema, add registry row. |
| Switch | Lock current workspace, clear stores, unlock target (prompt if locked), open its DB. |
| Rename | Update registry row only; directory id and DB path stay stable. |
| Lock | Drop DB, zeroize keys, stop playback, clear widgets, show unlock screen. |
| Delete | Confirm; delete wrapped key, workspace directory and all its data/backup files. |
| Autostart | Restore last active workspace; if locked, show unlock screen — see open decision #7 about the widget-only startup requirement. |

Settings preservation is automatic: each workspace's `settings` table and cache travel
with it. Global shell settings stay in `app.db`.

---

## 8. UI additions

- Workspace switcher in sidebar/topbar: avatar, name, workspace list, create/manage.
- Profile page: display name + avatar picker.
- Lock screen: credential entry with wrong-password feedback; shown on start/switch/lock.
- Playlist lock icon and unlock prompt.
- Grid items show a neutral placeholder while a `.hlv` cover is being decrypted.

---

## 9. Implementation phases

1. **Workspace isolation core** — `app.db` registry, per-workspace directory + `library.db`,
   active-workspace state, lock/unlock/switch, clearing stores and stopping playback,
   global vs workspace settings split. No new UI beyond a temporary switcher.
2. **Protected cache** — JPEG probe frame, `.hlv` reader/writer, decrypt commands,
   frontend data-URL loading + LRU, lazy generation, settings toggles.
3. **Workspace UI** — switcher, avatar/name, lock screen, settings scoping polish.
4. **Playlist security** — locked flag, UI gating, export guard.
5. **Migration & hardening** — move existing library into default workspace, split legacy
   settings, convert/delete legacy plaintext video thumbnails, optional SQLCipher, tests
   for wrong key, missing key, cross-workspace AAD rejection and lock/switch teardown.

### Acceptance criteria
- No library data exists outside a workspace directory; the app shell DB has no media rows.
- A locked workspace has no open DB connection and no key material in memory.
- Switching workspace fully swaps library, playlists, settings, history and cache.
- No plaintext video cover/storyboard exists on disk.
- Two workspaces cannot read each other's covers (separate DEKs + AAD binding).
- Decrypt failure regenerates lazily and never crashes the app.
- `vue-tsc --noEmit` and `cargo check` stay clean at every phase.

---

## 10. Open decisions

1. **At-rest database encryption** — SQLCipher per workspace (strict, but adds a native
   dependency and requires a key-loss recovery story) vs separate plaintext DB files
   protected only by OS account/isolation.
2. **Credential type** — password, numeric PIN, or Windows Hello integration.
3. **Non-Windows key protection** — DPAPI on Windows with what fallback elsewhere?
4. **Key-loss recovery** — if the DB is encrypted, offer a recovery phrase or encrypted
   export? Without one, losing the credential loses the library.
5. **Avatar** — encrypted `.hlv` or plain profile file?
6. **Playlist security** — reuse the workspace credential or give each protected playlist
   its own PIN/key?
7. **Autostart conflict** — issue #5 wants widget-only startup; a locked workspace cannot
   show widget media. Show the unlock screen instead while locked?
8. **JPEG support** — confirm bundled `libmpv-2.dll` accepts `vo-image-format=jpg`;
   otherwise keep PNG bytes inside the `.hlv` container.

---

## 11. Immediate next step if approved

Implement **Phase 1** with a single default workspace: create `app.db` + a per-workspace
`library.db`, move existing data into it, and make the active workspace the only DB the
app can open. That establishes strict isolation first; Phase 2 can then add encrypted
covers inside that boundary without ever depending on a global key.

---

## 12. Decisions locked in / implementation status

- **At-rest DB encryption:** SQLCipher (strictest) with a one-time recovery key per
  workspace. Workspace master key is protected by Windows DPAPI; the recovery key can
  re-wrap it if DPAPI cannot be used.
- **Locked autostart:** show the unlock screen. No widget media is restored until the
  workspace is unlocked (this overrides the earlier widget-only startup behavior).
- **Phase 1 (complete):** `app.db` registry, one SQLCipher `library.db` per workspace,
  active-workspace-only DB access, lock/unlock/switch/rename/delete/restore, global vs
  workspace settings split, legacy DB migration, lock overlay, temporary workspace
  manager in Settings. Verified with `vue-tsc --noEmit` and `cargo check`.
- **Workspace password (complete):** optional per-workspace password (Argon2id + AES-GCM
  wrapped master key) with DPAPI fallback. After splash, multiple workspaces show a
  chooser; a single workspace is auto-selected; password-protected workspaces prompt
  before the main UI is shown.
- **Phase 2 (complete):** encrypted `.hlv` video covers and storyboards (AES-256-GCM with
  workspace/media-bound AAD), lazy single-frame cover capture at ~10% of duration,
  plaintext staging removal, and data-URL delivery to all artwork/storyboard consumers.
  Verified with `vue-tsc --noEmit` and `cargo check`.
- **Phase 3 (complete):** workspace switcher in the top bar with avatar/name, per-workspace
  avatar upload/remove stored inside the workspace directory, avatars on chooser/lock
  screens, and settings-scoping hints. Verified with `vue-tsc --noEmit` and `cargo check`.
- **Phase 4 (complete):** private playlists (`playlists.locked`), hidden from listings until the
  workspace credential is verified, guarded mutations/tracks/export, lock icons and
  reveal/hide actions in the Playlists view. Verified with `vue-tsc --noEmit` and `cargo check`.
- **Phase 5 (complete):** storyboard frames are staged only in OS temp before encryption,
  legacy plaintext storyboard data is removed at startup, and automated tests cover the
  encrypted cache container (wrong key, cross-workspace replay, tampering, data URLs),
  the idempotent `playlists.locked` schema migration, and settings split/merge plus
  workspace key derivation/zeroization. `cargo test`: 9 passed, 0 failed, 1 ignored
  (optional real-video probe). All phases verified with `vue-tsc --noEmit` and `cargo check`.
