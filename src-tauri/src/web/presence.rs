//! Live registry of connected remote clients, so the host can tell who is on the other end.
//!
//! Mirror mode makes a remote client an equal peer: it publishes layouts the host then follows (see
//! [`super::mirror`]). The follower already knows what is going on — it wears the `Mirrored` badge — but the
//! host had no way to tell an empty service from one with somebody watching and rearranging its tabs. Tracking
//! live WebSocket connections is that missing signal.
//!
//! A bare count answers "is anyone there" but not "who", which is the next question a host asks before it lets
//! a peer move its tabs around. So each connection is kept with what identifies it: the device name and ID it
//! reported during the E2EE handshake, its address, and when it arrived. All three of those are absent or
//! spoofable in the plaintext modes — the address is the only part the server observes itself — so this is a
//! display aid, never an authorization input.
//!
//! Only WebSocket connections are tracked. The desktop that hosts the service talks over Tauri IPC and never
//! opens one, so on a host the list is exactly "the other windows attached". A desktop window that connects
//! *out* to a remote service is itself a WebSocket client of that service and appears there, which is
//! correct: on that machine it is one of the peers, not the host.
//!
//! Process-wide singleton for the same reason as the mirror hub: one process serves one instance.

use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{json, Value};

/// Broadcast whenever a remote client connects or disconnects, carrying `{count, clients}`.
pub const CLIENTS_EVENT: &str = "clients://changed";

/// One attached client, as shown behind the host's mirror badge.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    /// Connection ID (`ws-N`). Also the publisher ID in mirror snapshots, so the host can tell which
    /// entry in this list just moved its tabs.
    pub source: String,
    /// Self-reported device name from the E2EE handshake, such as `macOS · Chrome`. Absent on the
    /// plaintext paths (loopback and plain LAN), which carry no identity at all.
    pub name: Option<String>,
    /// Self-reported device ID, matching the paired-devices registry in [`super::auth`]. Absent for the
    /// same reason as `name`.
    pub device_id: Option<String>,
    /// Peer address observed by the server. The one field a client cannot choose for itself.
    pub ip: String,
    /// Unix seconds at which this connection was accepted, for showing how long it has been attached.
    pub since: u64,
}

/// Connections currently attached, in arrival order.
///
/// A `Vec` rather than a map: the list is at most a handful of entries, and arrival order is exactly the
/// order the badge wants to show them in.
fn registry() -> MutexGuard<'static, Vec<ClientInfo>> {
    static CLIENTS: OnceLock<Mutex<Vec<ClientInfo>>> = OnceLock::new();
    let lock = CLIENTS.get_or_init(|| Mutex::new(Vec::new()));
    // A panic elsewhere while holding the lock must not take the badge down with it; the list is display
    // state and recovers on the next connect or disconnect.
    lock.lock().unwrap_or_else(|e| e.into_inner())
}

impl ClientInfo {
    /// Build an entry for a connection being accepted right now, stamping `since` here so every caller
    /// agrees on what that clock is.
    pub fn arriving(
        source: String,
        name: Option<String>,
        device_id: Option<String>,
        ip: String,
    ) -> Self {
        Self {
            source,
            name,
            device_id,
            ip,
            since: now_secs(),
        }
    }
}

/// Current Unix time in seconds.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Current number of connected WebSocket clients.
pub fn count() -> usize {
    registry().len()
}

/// Snapshot of every connected client, for a client aligning right after it connects.
pub fn list() -> Vec<ClientInfo> {
    registry().clone()
}

/// Event payload shape shared by [`join`], [`leave`], and the alignment reply.
fn payload(clients: &[ClientInfo]) -> Value {
    json!({"count": clients.len(), "clients": clients})
}

/// Register a newly accepted connection and return the resulting broadcast payload.
pub fn join(info: ClientInfo) -> Value {
    let mut clients = registry();
    clients.push(info);
    payload(&clients)
}

/// Deregister a closed connection by its source ID and return the resulting broadcast payload.
///
/// Removing by ID rather than decrementing a counter: an unbalanced or duplicated call then changes
/// nothing instead of leaving the badge stuck on forever.
pub fn leave(source: &str) -> Value {
    let mut clients = registry();
    if let Some(pos) = clients.iter().position(|c| c.source == source) {
        clients.remove(pos);
    }
    payload(&clients)
}

/// Serialize tests that touch the registry and start each from an empty one.
///
/// The registry is a process-wide singleton while cargo runs tests in parallel threads, so two tests
/// asserting on it would otherwise see each other's connections.
#[cfg(test)]
pub(crate) fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static SERIAL: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    let lock = SERIAL.get_or_init(|| std::sync::Mutex::new(()));
    // A test that panicked while holding it poisoned the lock; the reset below rebuilds the state anyway.
    let g = lock.lock().unwrap_or_else(|e| e.into_inner());
    registry().clear();
    g
}

#[cfg(test)]
mod tests {
    use super::test_guard as guard;
    use super::*;

    /// Build an entry with only the source set; the rest is display detail these tests do not exercise.
    fn client(source: &str) -> ClientInfo {
        ClientInfo::arriving(source.to_string(), None, None, "127.0.0.1".to_string())
    }

    /// Connect and disconnect are symmetric, so the list empties once everyone has left.
    #[test]
    fn join_and_leave_balance_out() {
        let _g = guard();
        assert_eq!(count(), 0);
        assert_eq!(join(client("ws-1"))["count"], 1);
        assert_eq!(join(client("ws-2"))["count"], 2);
        assert_eq!(leave("ws-1")["count"], 1);
        assert_eq!(leave("ws-2")["count"], 0);
    }

    /// An unbalanced leave must not corrupt the list; the badge would then stay on forever.
    #[test]
    fn leave_without_join_stays_empty() {
        let _g = guard();
        assert_eq!(leave("ws-9")["count"], 0);
        assert_eq!(count(), 0);
    }

    /// Each connection is identified individually, so leaving removes that client and nobody else.
    #[test]
    fn leave_removes_only_its_own_connection() {
        let _g = guard();
        join(client("ws-1"));
        join(client("ws-2"));
        leave("ws-1");
        let left = list();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].source, "ws-2");
    }

    /// The reported identity travels with the connection; the badge reads its name from here.
    #[test]
    fn join_carries_the_reported_identity() {
        let _g = guard();
        let mut c = client("ws-1");
        c.name = Some("macOS · Chrome".to_string());
        c.device_id = Some("dev-1".to_string());
        let out = join(c);
        assert_eq!(out["clients"][0]["name"], "macOS · Chrome");
        assert_eq!(out["clients"][0]["deviceId"], "dev-1");
        assert_eq!(out["clients"][0]["ip"], "127.0.0.1");
    }
}
