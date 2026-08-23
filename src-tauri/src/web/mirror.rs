//! Shared UI-mirror state: the one authoritative layout snapshot that every client pushes to and follows.
//!
//! The terminal byte stream is already shared (one PTY, see the communication doc §6.3 owner model), but the
//! *layout* around it — tabs, split trees, the active session, sidebar selection — used to live only in each
//! client's own `localStorage`. Mirror mode publishes that layout through this hub so a remote browser renders
//! the same arrangement as the desktop.
//!
//! Semantics are deliberately minimal, because the conflict policy is last-writer-wins:
//! - the state is an opaque JSON blob; only the frontend knows its shape, so a schema change needs no Rust edit;
//! - `rev` is a monotonic counter used to drop out-of-order frames, NOT an optimistic lock — a push always wins;
//! - `source` is the pushing client's connection ID (`desktop` or `ws-N`), so a client can drop its own echo.
//!
//! The hub is a process-wide singleton rather than `AppCtx` state: the desktop GUI and the web service share one
//! process, and headless `--serve` runs a single instance per process, so there is exactly one layout to mirror.

use std::sync::{Mutex, OnceLock};

use serde_json::Value;

/// Broadcast on every accepted push, carrying `{rev, source, state}`.
pub const LAYOUT_EVENT: &str = "mirror://layout";
/// Broadcast when the host toggles mirror mode, carrying `{enabled}`.
pub const MODE_EVENT: &str = "mirror://mode";

/// The published layout, plus who published it and when (as a revision counter).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// Monotonic revision; 0 means nothing has ever been published.
    pub rev: u64,
    /// Connection ID of the last publisher: `desktop`, or `ws-N` for a WebSocket client.
    pub source: String,
    /// Opaque frontend layout blob; `None` until the first push.
    pub state: Option<Value>,
}

impl Snapshot {
    /// JSON form shared by the `mirror_get` reply and the `mirror://layout` event payload.
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "rev": self.rev,
            "source": self.source,
            "state": self.state.clone().unwrap_or(Value::Null),
        })
    }
}

fn cell() -> &'static Mutex<Snapshot> {
    static HUB: OnceLock<Mutex<Snapshot>> = OnceLock::new();
    HUB.get_or_init(|| Mutex::new(Snapshot::default()))
}

/// Read the current snapshot, for `mirror_get` and for a freshly connected client's first alignment.
pub fn current() -> Snapshot {
    cell().lock().unwrap().clone()
}

/// Publish `state` on behalf of `source` and return the stored snapshot with its new revision.
pub fn push(source: &str, state: Value) -> Snapshot {
    let mut guard = cell().lock().unwrap();
    guard.rev = guard.rev.saturating_add(1);
    guard.source = source.to_string();
    guard.state = Some(state);
    guard.clone()
}

/// Drop the published layout. Called when mirror mode is switched off so re-enabling starts from whoever
/// publishes first instead of resurrecting a stale arrangement.
///
/// The revision counter deliberately keeps counting. Clients drop frames older than the newest one they
/// have seen, so restarting the count at zero would make every push after a switch-off look stale to a
/// client that was already connected — the two would then never converge again.
pub fn clear() {
    let mut guard = cell().lock().unwrap();
    guard.source.clear();
    guard.state = None;
}

/// Serialize tests that touch the hub and start each from an empty one.
///
/// The hub is a process-wide singleton while cargo runs tests in parallel threads, so any two tests
/// asserting on published state would otherwise see each other's pushes. Every such test — here and in
/// `dispatch.rs` — must hold this guard for its whole body.
#[cfg(test)]
pub(crate) fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static SERIAL: OnceLock<Mutex<()>> = OnceLock::new();
    let lock = SERIAL.get_or_init(|| Mutex::new(()));
    // A test that panicked while holding it poisoned the lock; `clear` rebuilds the state anyway.
    let g = lock.lock().unwrap_or_else(|e| e.into_inner());
    *cell().lock().unwrap() = Snapshot::default();
    g
}

#[cfg(test)]
mod tests {
    use super::test_guard as guard;
    use super::*;
    use serde_json::json;

    /// A push records the state and its publisher, and every push advances the revision.
    #[test]
    fn push_records_source_and_advances_rev() {
        let _g = guard();
        assert_eq!(current(), Snapshot::default());

        let first = push("desktop", json!({"openTabs": ["a"]}));
        assert_eq!(first.rev, 1);
        assert_eq!(first.source, "desktop");
        assert_eq!(first.state, Some(json!({"openTabs": ["a"]})));

        // Last-writer-wins: a second publisher overwrites without needing to match the previous rev.
        let second = push("ws-3", json!({"openTabs": ["b"]}));
        assert_eq!(second.rev, 2);
        assert_eq!(second.source, "ws-3");
        assert_eq!(current().state, Some(json!({"openTabs": ["b"]})));
    }

    /// Clearing drops the layout but keeps the revision counting, so a push after a switch-off is never
    /// mistaken for a stale frame by a client that stayed connected through it.
    #[test]
    fn clear_drops_the_layout_but_keeps_revisions_moving() {
        let _g = guard();
        push("desktop", json!({"openTabs": ["a"]}));
        clear();
        let now = current();
        assert_eq!(now.state, None);
        assert!(now.source.is_empty());
        assert_eq!(now.rev, 1, "the counter must not rewind");
        assert_eq!(push("ws-1", json!({"openTabs": ["b"]})).rev, 2);
    }

    /// The wire form always carries a JSON value for `state`, using null before the first push, so the
    /// frontend never has to distinguish "absent" from "empty".
    #[test]
    fn json_form_uses_null_before_first_push() {
        let _g = guard();
        assert_eq!(current().to_json()["state"], Value::Null);
        push("desktop", json!({"openTabs": []}));
        assert_eq!(current().to_json()["state"], json!({"openTabs": []}));
        assert_eq!(current().to_json()["rev"], json!(1));
    }
}
