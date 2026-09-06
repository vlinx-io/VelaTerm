//! Local link: the loopback web service every desktop instance runs so an SSH mirror connection can
//! attach to the running desktop app itself rather than to a separate headless server.
//!
//! A URL connection reaches the remote desktop's own process, so mirror mode there syncs with the desktop
//! UI for free. An SSH connection had no such target: it only ever started `vela-server --serve`, which
//! shares the desktop's database file but not its PTYs or its in-memory layout. The local link closes that
//! gap. On startup the desktop binds a plaintext HTTP/WebSocket service on `127.0.0.1` with a random port
//! and a random password, and records `{pid, port, password}` in `vlx-local-link.json` (owner-only) inside
//! its data directory. An SSH client, which already has this user's shell, reads that file over SSH,
//! forwards the port through the tunnel, and logs in with the recorded password. The service is the same
//! `WebServer` the LAN remote-access panel uses, so PTYs, database, mirror hub, and presence are shared
//! with the desktop window automatically.
//!
//! The file carries a plaintext secret and therefore relies on the same trust boundary as `~/.velaterm/
//! run.json` on SSH remotes: anyone who can read this user's files can already act as this user. The
//! listener never leaves loopback, and a stale file is harmless because the reader verifies the PID and
//! then the HTTP health of the forwarded port before trusting it.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{ServeMode, StartAuth, WebServer};
use crate::host::AppCtx;

/// File name inside the data directory; the SSH client looks for exactly this name.
pub const FILENAME: &str = "vlx-local-link.json";

/// On-disk description of the running local link. Field names match `run.json` on SSH remotes so the
/// SSH reader can treat both records alike.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalLink {
    /// PID of the desktop process; the reader checks it is alive before using the port.
    pub pid: u32,
    /// Loopback port the service listens on.
    pub port: u16,
    /// Alphanumeric login password for the service.
    pub password: String,
}

/// Tauri-managed state for the local-link service, kept separate from the LAN `WebServer` so the two
/// instances start and stop independently.
pub struct LocalLinkServer(pub WebServer);

impl LocalLinkServer {
    pub fn new() -> Self {
        Self(WebServer::new())
    }
}

/// Full path of the link file inside a data directory.
pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILENAME)
}

/// Parse a link record and reject one whose password could not be typed into a remote command safely.
/// Shared with the SSH reader, which interpolates the password into a window's init script only, but the
/// same rule keeps both sides honest about what a valid record looks like.
pub fn parse(text: &str) -> Option<LocalLink> {
    let link: LocalLink = serde_json::from_str(text.trim()).ok()?;
    if link.port == 0
        || link.password.is_empty()
        || !link.password.chars().all(|c| c.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(link)
}

/// Pick a free loopback port. The bind is released before the service binds it again, the same small
/// race `ssh_remote::pick_local_port` accepts.
fn free_loopback_port() -> Result<u16, String> {
    let l = std::net::TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| format!("failed to pick a local-link port: {e}"))?;
    l.local_addr()
        .map(|a| a.port())
        .map_err(|e| format!("failed to read the local-link port: {e}"))
}

/// Start the local link on a fresh port and record it in the data directory. Returns the port.
pub fn start(ctx: AppCtx, server: &WebServer) -> Result<u16, String> {
    let port = free_loopback_port()?;
    let password = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let data_dir = ctx.data_dir()?;
    server.start(
        ctx,
        StartAuth::Password(password.clone()),
        Some(port),
        ServeMode::LoopbackHttp,
    )?;
    let link = LocalLink {
        pid: std::process::id(),
        port,
        password,
    };
    write(&data_dir, &link)?;
    Ok(port)
}

/// Write the link record owner-only. Separate from `start` so the file shape is testable without a server.
fn write(data_dir: &Path, link: &LocalLink) -> Result<(), String> {
    let json = serde_json::to_string(link)
        .map_err(|e| format!("failed to serialize the local-link record: {e}"))?;
    super::write_owner_only(&path(data_dir), json.as_bytes())
        .map_err(|e| format!("failed to write the local-link record: {e}"))
}

/// Remove the link record on a clean exit. A crash leaves it behind, which is fine: the reader checks
/// the PID and then the forwarded port before trusting the record.
pub fn remove(data_dir: &Path) {
    let _ = std::fs::remove_file(path(data_dir));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_record_round_trips_and_validates() {
        let dir = std::env::temp_dir().join(format!("vlx-local-link-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let link = LocalLink {
            pid: 4242,
            port: 31337,
            password: "abc123DEF".into(),
        };
        write(&dir, &link).unwrap();
        let text = std::fs::read_to_string(path(&dir)).unwrap();
        assert_eq!(parse(&text), Some(link));
        // The file uses the same keys as run.json so the SSH reader can treat both alike.
        assert!(text.contains("\"pid\":4242"));
        assert!(text.contains("\"port\":31337"));
        remove(&dir);
        assert!(!path(&dir).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_rejects_unsafe_or_incomplete_records() {
        assert!(parse("").is_none());
        assert!(parse("{\"pid\":1,\"port\":0,\"password\":\"x\"}").is_none());
        assert!(parse("{\"pid\":1,\"port\":5,\"password\":\"\"}").is_none());
        // A password with shell metacharacters must never reach an interpolated command.
        assert!(parse("{\"pid\":1,\"port\":5,\"password\":\"a'b\"}").is_none());
        assert!(parse("{\"pid\":1,\"port\":5,\"password\":\"ok\"}").is_some());
    }
}
