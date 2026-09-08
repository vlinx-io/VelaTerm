//! Process-wide view of which sessions are working, asking, or waiting, plus a way to block until that
//! changes.
//!
//! The authoritative state machine already knows all of this: agent hooks post to the local service and
//! it maps them to [`AgentState`]. What was missing is a way for something *outside* the frontend to ask,
//! and to wait. `vstat` is that consumer, and an orchestration's coordinator session lives on the waiting
//! form: it blocks here instead of polling, so watching several child sessions costs nothing between
//! events.
//!
//! **The version counter is the point.** Without it a waiter that was busy handling one change would miss
//! the next one and then block until its timeout, reporting nothing while work had actually finished. A
//! caller passes back the version it last saw; if the world moved on meanwhile, it returns immediately.
//! This is the same idea as an incremental read cursor.
//!
//! One `StatusWatch` per process, like the hook server it is fed by.

use std::collections::HashMap;
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::pty::AgentState;

/// One session's last known state.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusRow {
    pub session_id: String,
    pub state: AgentState,
    /// Unix seconds when this state was recorded, for showing how long it has been that way.
    pub updated_at: i64,
}

#[derive(Default)]
struct Inner {
    /// Bumped on every recorded change; callers use it as a cursor.
    version: u64,
    states: HashMap<String, StatusRow>,
}

pub struct StatusWatch {
    inner: Mutex<Inner>,
    cv: Condvar,
}

/// The process's status watch.
fn watch() -> &'static StatusWatch {
    static WATCH: OnceLock<StatusWatch> = OnceLock::new();
    WATCH.get_or_init(|| StatusWatch {
        inner: Mutex::new(Inner::default()),
        cv: Condvar::new(),
    })
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Record a session's state and wake anything waiting.
///
/// Called from the hook service's request loop, so it must stay a fast in-memory operation: that loop
/// also carries agent status callbacks and cannot be delayed.
///
/// An unchanged state still refreshes the timestamp but does **not** bump the version, so a session
/// re-reporting `working` on every tool call cannot wake every waiter each time.
pub fn record(session_id: &str, state: AgentState) {
    let Ok(mut inner) = watch().inner.lock() else {
        return;
    };
    let changed = inner
        .states
        .get(session_id)
        .map(|prev| prev.state != state)
        .unwrap_or(true);
    inner.states.insert(
        session_id.to_string(),
        StatusRow {
            session_id: session_id.to_string(),
            state,
            updated_at: now_secs(),
        },
    );
    if changed {
        inner.version += 1;
        drop(inner);
        watch().cv.notify_all();
    }
}

/// Mark a session that has already reported as no longer busy, for when its process ends.
///
/// Only a known session is touched: a plain terminal that never reported must not start appearing in
/// listings just because it closed. An agent whose process exits mid-turn sends no Stop, and without
/// this a watcher would wait on it forever.
pub fn settle(session_id: &str) {
    let known = watch()
        .inner
        .lock()
        .map(|inner| inner.states.contains_key(session_id))
        .unwrap_or(false);
    if known {
        record(session_id, AgentState::Waiting);
    }
}

/// Forget a session, so a deleted one stops appearing in listings.
pub fn forget(session_id: &str) {
    let Ok(mut inner) = watch().inner.lock() else {
        return;
    };
    if inner.states.remove(session_id).is_some() {
        inner.version += 1;
        drop(inner);
        watch().cv.notify_all();
    }
}

/// Current version and every known session's state, ordered by id for a stable listing.
pub fn snapshot() -> (u64, Vec<StatusRow>) {
    let Ok(inner) = watch().inner.lock() else {
        return (0, Vec::new());
    };
    let mut rows: Vec<StatusRow> = inner.states.values().cloned().collect();
    rows.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    (inner.version, rows)
}

/// Block until the version moves past `since`, or `timeout` elapses.
///
/// Returns the current version and rows either way; a caller distinguishes "something happened" from
/// "nothing happened" by comparing the returned version with the one it passed in.
///
/// Callers run this off the hook service's accept loop — it parks for as long as the caller asked.
pub fn wait_for_change(since: u64, timeout: Duration) -> (u64, Vec<StatusRow>) {
    let Ok(inner) = watch().inner.lock() else {
        return (0, Vec::new());
    };
    let (inner, _) = watch()
        .cv
        .wait_timeout_while(inner, timeout, |i| i.version <= since)
        .unwrap_or_else(|e| e.into_inner());
    let mut rows: Vec<StatusRow> = inner.states.values().cloned().collect();
    rows.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    (inner.version, rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests share one process-wide watch and assert on its version counter, so they must not overlap:
    /// another test recording a state would bump the version between two snapshots here.
    fn exclusive() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn ids(tag: &str) -> (String, String) {
        (format!("{tag}-a"), format!("{tag}-b"))
    }

    #[test]
    fn record_bumps_the_version_only_on_a_real_change() {
        let _guard = exclusive();
        let (a, _) = ids("bump");
        let (before, _) = snapshot();
        record(&a, AgentState::Working);
        let (after_first, rows) = snapshot();
        assert!(after_first > before, "a new session is a change");
        assert_eq!(
            rows.iter().find(|r| r.session_id == a).map(|r| r.state),
            Some(AgentState::Working)
        );

        // Re-reporting the same state must not wake waiters: an agent posts `working` on every tool
        // call, and each one would otherwise be an event.
        record(&a, AgentState::Working);
        let (after_same, _) = snapshot();
        assert_eq!(after_same, after_first);

        record(&a, AgentState::Waiting);
        let (after_change, _) = snapshot();
        assert!(after_change > after_first);
    }

    #[test]
    fn wait_returns_at_once_when_the_caller_is_behind() {
        let _guard = exclusive();
        let (a, _) = ids("behind");
        record(&a, AgentState::Working);
        let (version, _) = snapshot();
        // The change already happened, so waiting on an older version must not park at all — this is
        // exactly the lost-wakeup case the version counter exists for.
        let started = std::time::Instant::now();
        let (got, _) = wait_for_change(version - 1, Duration::from_secs(30));
        assert!(got >= version);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "waiting on a stale version must return immediately, took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn wait_wakes_on_a_change_and_otherwise_times_out() {
        let _guard = exclusive();
        let (_, b) = ids("wake");
        let (version, _) = snapshot();
        let sid = b.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(120));
            record(&sid, AgentState::Asking);
        });
        let started = std::time::Instant::now();
        let (got, rows) = wait_for_change(version, Duration::from_secs(10));
        assert!(got > version, "the waiter should have been woken");
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(
            rows.iter().find(|r| r.session_id == b).map(|r| r.state),
            Some(AgentState::Asking)
        );

        // With nothing happening it returns on the deadline rather than hanging.
        let (current, _) = snapshot();
        let started = std::time::Instant::now();
        let (after, _) = wait_for_change(current, Duration::from_millis(200));
        assert_eq!(after, current, "a timeout reports no change");
        assert!(started.elapsed() >= Duration::from_millis(150));
    }

    #[test]
    fn settle_only_touches_sessions_that_have_reported() {
        let _guard = exclusive();
        let (a, b) = ids("settle");
        record(&a, AgentState::Working);
        let (version, _) = snapshot();
        settle(&a);
        let (after, rows) = snapshot();
        assert!(after > version, "an exiting worker is a change");
        assert_eq!(
            rows.iter().find(|r| r.session_id == a).map(|r| r.state),
            Some(AgentState::Waiting)
        );
        // A session nobody has heard from stays absent: closing a terminal is not a report.
        settle(&b);
        let (unchanged, rows) = snapshot();
        assert_eq!(unchanged, after);
        assert!(!rows.iter().any(|r| r.session_id == b));
    }

    #[test]
    fn forget_drops_a_session_from_listings() {
        let _guard = exclusive();
        let (a, _) = ids("forget");
        record(&a, AgentState::Working);
        assert!(snapshot().1.iter().any(|r| r.session_id == a));
        forget(&a);
        assert!(!snapshot().1.iter().any(|r| r.session_id == a));
        // Forgetting something unknown is a no-op, not a spurious wakeup.
        let (version, _) = snapshot();
        forget("never-existed");
        assert_eq!(snapshot().0, version);
    }
}
