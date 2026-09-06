//! Extraction and discovery of the OMP status-bridge extension.
//!
//! vlx-term bundles an OMP extension via compile-time `include_str!`. When launching an OMP session, it extracts the
//! extension to `omp/` under the application data directory and loads it once with command-line
//! `-e <absolute extension path>` (see the OMP branch in `agent/inject.rs` and `pty/manager.rs`). OMP transpiles
//! `.ts` on demand, so no build step is required.
//!
//! The extension POSTs OMP lifecycle events (`input`, `agent_start`, `tool_approval_requested`,
//! `tool_approval_resolved`, `agent_end`, and `session_start`) to the local hook service using injected
//! `VLX_SESSION_ID`/`VLX_TOKEN`/`VLX_SPAWN_URL`, giving OMP sessions authoritative activity state. See
//! `resources/vlx-omp-notify.ts` for the source.
//!
//! OMP is a fork of Pi, but it registers handlers through `pi.on(...)` instead of returning a handler map, so it
//! needs its own extension source rather than reusing `agent/pi.rs`.
//!
//! Use command-line `-e` instead of writing the user's global extension directory (`~/.omp/agent/extensions/`).
//! `-e` is the official per-launch mechanism, scoped cleanly to one process with no uninstall logic and no effect
//! on OMP instances the user launches independently.

use std::path::{Path, PathBuf};

/// Extension source embedded at compile time and distributed with the binary, requiring no external file.
const PLUGIN: &str = include_str!("../../resources/vlx-omp-notify.ts");
/// Extracted filename.
const PLUGIN_NAME: &str = "vlx-omp-notify.ts";

/// Absolute extracted extension path: `<data_dir>/omp/vlx-omp-notify.ts`.
pub fn plugin_path(data_dir: &Path) -> PathBuf {
    data_dir.join("omp").join(PLUGIN_NAME)
}

/// Extract the OMP extension to `<data_dir>/omp/` and return its path.
///
/// Overwrite on every startup so the extension tracks the application version.
pub fn install(data_dir: &Path) -> std::io::Result<PathBuf> {
    let path = plugin_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, PLUGIN)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_writes_plugin_at_expected_path() {
        let tmp = std::env::temp_dir().join(format!("vlx-omp-plugin-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);

        let path = install(&tmp).expect("writing out the extension should succeed");
        assert_eq!(path, plugin_path(&tmp));

        let written = std::fs::read_to_string(&path).expect("the extension should be readable again");
        assert_eq!(written, PLUGIN, "the contents should be the inlined source");
        // Coarsely verify that the extension registers lifecycle hooks and reports as expected, guarding against an
        // accidentally empty source. OMP registers through `pi.on`, unlike Pi's returned handler map.
        assert!(written.contains("pi.on("), "the extension should register through pi.on");
        assert!(written.contains("agent_end"), "the extension should map agent_end");
        assert!(
            written.contains("tool_approval_requested"),
            "the extension should map the approval prompt to the asking state"
        );
        assert!(
            written.contains("VLX_SESSION_ID"),
            "the extension should read VLX_SESSION_ID to decide whether it is enabled"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
