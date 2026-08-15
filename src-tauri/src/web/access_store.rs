//! Persistence for remote-access pairing state: shared pairing token, device display registry, and
//! device blocklist. Follows the `e2ee::ServerKeys::load_or_create` pattern — one file in the data
//! directory, mode 0600 on Unix — so pairing links and revocations survive app/server restarts
//! (GitHub issue #15). The explicit "Regenerate link" rotation remains the invalidation path: it
//! overwrites this file with a fresh token and empty registry/blocklist.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::auth::DeviceEntry;

/// Pairing-state filename in the data directory.
const FILENAME: &str = "vlx-web-access.json";

/// On-disk pairing state, serialized in camelCase like the other web-facing structs.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedAccess {
    /// Shared admission token embedded in pairing links.
    pub pairing_token: String,
    /// Blocked device IDs; must survive restarts so a revoked device cannot return with a long-lived token.
    #[serde(default)]
    pub blocked_devices: Vec<String>,
    /// Display registry of client-reported devices, persisted so the panel stays coherent after restart.
    #[serde(default)]
    pub devices: Vec<DeviceEntry>,
}

/// Full path of the pairing-state file inside a data directory.
fn path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILENAME)
}

/// Load persisted pairing state; None when the file is missing, unreadable, corrupt, or has an empty
/// token — callers then create a fresh token and rewrite the file, matching the E2EE key-file pattern.
pub fn load(data_dir: &Path) -> Option<PersistedAccess> {
    let text = std::fs::read_to_string(path(data_dir)).ok()?;
    serde_json::from_str::<PersistedAccess>(&text)
        .ok()
        .filter(|p| !p.pairing_token.trim().is_empty())
}

/// Flush a file's data to disk via a write-capable handle. `File::open` would return a read-only
/// handle, which suffices for fsync on Unix but breaks on Windows: there `sync_all` maps to
/// `FlushFileBuffers`, which requires GENERIC_WRITE access on the handle and fails with
/// ERROR_ACCESS_DENIED on a read-only one. Opening with `write(true)` (no truncate) does not
/// modify the file's contents.
fn fsync_file(path: &Path) -> std::io::Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .sync_all()
}

/// Atomically save pairing state: write a temp file created owner-only (0600 at open time, so the token
/// never exists with default umask permissions), fsync it, rename it over the target, and fsync the
/// directory. The rename alone only makes the swap atomic against a crash mid-write; without the fsyncs
/// a power failure could promote a zero-length/partial temp file over the previous good state, or lose
/// the renamed directory entry itself.
pub fn save(data_dir: &Path, access: &PersistedAccess) -> Result<(), String> {
    let target = path(data_dir);
    let tmp = data_dir.join(format!("{FILENAME}.tmp"));
    let json = serde_json::to_string(access)
        .map_err(|e| format!("failed to serialize remote-access state: {e}"))?;
    super::write_owner_only(&tmp, json.as_bytes())
        .map_err(|e| format!("failed to write remote-access state: {e}"))?;
    // Flush the temp file's DATA to disk before the rename makes it the current state.
    fsync_file(&tmp).map_err(|e| format!("failed to sync remote-access state: {e}"))?;
    std::fs::rename(&tmp, &target)
        .map_err(|e| format!("failed to persist remote-access state: {e}"))?;
    // Flush the directory entry so the rename itself survives a power failure. Directories cannot be
    // fsynced on Windows; there the durable-data rename above is the best available guarantee.
    #[cfg(unix)]
    std::fs::File::open(data_dir)
        .and_then(|d| d.sync_all())
        .map_err(|e| format!("failed to sync remote-access state directory: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vlx-access-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempdir("roundtrip");
        let access = PersistedAccess {
            pairing_token: "tok-123".into(),
            blocked_devices: vec!["bad-device".into()],
            devices: vec![DeviceEntry {
                device_id: "dev-a".into(),
                name: "Phone".into(),
                first_seen_at: 1,
                last_seen_at: 2,
            }],
        };
        save(&dir, &access).unwrap();
        let loaded = load(&dir).expect("saved state should load");
        assert_eq!(loaded.pairing_token, "tok-123");
        assert_eq!(loaded.blocked_devices, vec!["bad-device".to_string()]);
        assert_eq!(loaded.devices.len(), 1);
        assert_eq!(loaded.devices[0].device_id, "dev-a");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_corrupt_or_empty_token_yields_none() {
        let dir = tempdir("corrupt");
        // Missing file.
        assert!(load(&dir).is_none());
        // Corrupt JSON.
        std::fs::write(dir.join(FILENAME), "{not json").unwrap();
        assert!(load(&dir).is_none());
        // Valid JSON but empty token must not be treated as a usable credential.
        std::fs::write(dir.join(FILENAME), r#"{"pairingToken":"  "}"#).unwrap();
        assert!(load(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The sync step of `save` must propagate I/O errors instead of swallowing them: a state that
    /// was never flushed must not be reported as persisted. A missing file makes the write-mode
    /// open inside `fsync_file` fail, exercising the same `?`-propagation path `save` relies on.
    #[test]
    fn fsync_file_propagates_open_errors() {
        let dir = tempdir("fsync-err");
        let missing = dir.join("does-not-exist.tmp");
        assert!(fsync_file(&missing).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Regression guard for the Windows fsync fix: the sync handle must be opened with WRITE
    /// access (read-only handles fail FlushFileBuffers on Windows), and that write-mode open must
    /// still succeed on the owner-only 0600 files `write_owner_only` produces — without altering
    /// the file's contents.
    #[cfg(unix)]
    #[test]
    fn fsync_file_works_on_owner_only_file_without_modifying_it() {
        let dir = tempdir("fsync-0600");
        let file = dir.join("state.tmp");
        crate::web::write_owner_only(&file, b"payload").unwrap();
        fsync_file(&file).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"payload");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn saved_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir("perm");
        let access = PersistedAccess {
            pairing_token: "tok".into(),
            blocked_devices: Vec::new(),
            devices: Vec::new(),
        };
        save(&dir, &access).unwrap();
        let mode = std::fs::metadata(dir.join(FILENAME))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
