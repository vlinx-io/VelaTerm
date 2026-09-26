//! Continue a conversation on its own once the subscription usage limit that stopped it resets.
//!
//! Claude and Codex both refuse a turn when a five-hour or weekly quota is exhausted, and both say when
//! that quota resets. With `autoContinueAtUsageLimit` switched on in the shared `vlx-settings` block, the
//! engine reports the refusal here, this module waits for the reset on the wall clock, and then sends one
//! continuation prompt to the same conversation, starting its agent again if the view had released it.
//!
//! A wait is persisted in `chat_auto_continue`, so a weekly reset survives an application restart. It ends
//! without sending when the user sends a message, rewinds, cancels it from the pane, or switches the setting
//! off, and when the session is archived, deleted or moved back to the terminal engine by the time it fires.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::params;
use serde::Serialize;
use serde_json::{json, Value};

use crate::agent::transcript::CodexRateWindow;
use crate::host::AppCtx;
use crate::models::SessionKind;

pub const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS chat_auto_continue (
    session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    continue_at INTEGER NOT NULL,
    limit_type TEXT,
    rearms INTEGER NOT NULL DEFAULT 0
);";

const VLX_SETTINGS_KEY: &str = "vlx-settings";
const SETTING: &str = "autoContinueAtUsageLimit";

/// Sent as the user turn once the limit has reset.
pub const PROMPT: &str = "The usage limit has reset. Continue the task you were working on when the limit was reached; do not repeat work that is already complete.";

/// Wait this long past the reported reset, so a clock slightly ahead of the provider's does not fire into
/// the same refusal.
const GRACE_SECS: i64 = 60;

/// A continuation refused again right away is armed once more, up to this many times in a row.
const MAX_REARMS: u32 = 3;

/// How often the scheduler compares the wall clock with pending resets. A laptop that slept through a
/// reset fires within this long of waking, which a monotonic timer would not do.
const TICK: Duration = Duration::from_secs(15);

/// When a refused account becomes usable again, as the provider reported it.
#[derive(Clone, Debug, PartialEq)]
pub struct Reset {
    /// Unix seconds.
    pub resets_at: i64,
    /// `five_hour`, `seven_day`, or a provider-specific weekly bucket; None when the provider did not say.
    pub limit_type: Option<String>,
}

/// A conversation waiting for its limit to reset, as the pane shows it.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Waiting {
    /// Unix seconds at which the continuation is sent.
    pub continue_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_type: Option<String>,
    #[serde(skip)]
    rearms: u32,
}

/// How a turn ended, as far as automatic continuation is concerned.
pub enum TurnOutcome {
    Succeeded,
    /// A usage limit refused the turn; the reset is None when the provider did not report one.
    LimitReached(Option<Reset>),
    /// Any other failure, or an interrupt.
    Other,
}

#[derive(Default)]
struct State {
    waiting: HashMap<String, Waiting>,
    /// Sessions whose running turn is a continuation this module sent, with the rearm count it carried.
    fired: HashMap<String, u32>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut guard = STATE.lock().unwrap();
    f(guard.get_or_insert_with(State::default))
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Whether the setting is on. Missing or unreadable settings mean on, matching the frontend default.
pub fn enabled(ctx: &AppCtx) -> bool {
    let json = {
        let Ok(conn) = ctx.db().conn.lock() else { return false };
        crate::db::repo::get_app_settings(&conn).ok().and_then(|mut m| m.remove(VLX_SETTINGS_KEY))
    };
    json.and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get(SETTING).and_then(Value::as_bool))
        .unwrap_or(true)
}

/// The wait in progress for this session, for its snapshot.
pub fn waiting(session_id: &str) -> Option<Waiting> {
    with_state(|state| state.waiting.get(session_id).cloned())
}

fn emit(ctx: &AppCtx, session_id: &str, waiting: Option<&Waiting>, reason: Option<&str>) {
    ctx.emit(
        &super::engine::event_name(session_id),
        json!({"type":"autoContinue","waiting":waiting,"reason":reason}),
    );
}

fn persist(ctx: &AppCtx, session_id: &str, waiting: &Waiting) {
    let conn = ctx.db().conn.lock().unwrap();
    if let Err(e) = conn.execute(
        "INSERT INTO chat_auto_continue(session_id,continue_at,limit_type,rearms) VALUES (?1,?2,?3,?4)
         ON CONFLICT(session_id) DO UPDATE SET continue_at=excluded.continue_at,limit_type=excluded.limit_type,rearms=excluded.rearms",
        params![session_id, waiting.continue_at, waiting.limit_type, waiting.rearms],
    ) {
        crate::diagnostic_warn!("auto-continue: failed to save the wait for {session_id}: {e}");
    }
}

fn forget(ctx: &AppCtx, session_id: &str) {
    let conn = ctx.db().conn.lock().unwrap();
    let _ = conn.execute("DELETE FROM chat_auto_continue WHERE session_id=?1", params![session_id]);
}

/// A turn of this conversation ended. Called by the engine for every agent kind.
pub fn turn_ended(ctx: &AppCtx, session_id: &str, outcome: TurnOutcome) {
    match outcome {
        TurnOutcome::LimitReached(reset) => limit_reached(ctx, session_id, reset),
        TurnOutcome::Succeeded => {
            if with_state(|state| {
                state.fired.remove(session_id);
                state.waiting.remove(session_id).is_some()
            }) {
                forget(ctx, session_id);
                emit(ctx, session_id, None, None);
            }
        }
        TurnOutcome::Other => {
            with_state(|state| state.fired.remove(session_id));
        }
    }
}

fn limit_reached(ctx: &AppCtx, session_id: &str, reset: Option<Reset>) {
    let previous = with_state(|state| state.fired.remove(session_id));
    if !enabled(ctx) || crate::security::session_active(session_id) {
        return;
    }
    let Some(reset) = reset else {
        emit(ctx, session_id, None, Some("unknownReset"));
        return;
    };
    let rearms = previous.map_or(0, |count| count + 1);
    if rearms > MAX_REARMS {
        cancel(ctx, session_id);
        emit(ctx, session_id, None, Some("repeated"));
        return;
    }
    // A reset already in the past still refused this turn; try again shortly rather than never.
    let waiting = Waiting {
        continue_at: reset.resets_at.max(now_secs()) + GRACE_SECS,
        limit_type: reset.limit_type,
        rearms,
    };
    with_state(|state| state.waiting.insert(session_id.to_string(), waiting.clone()));
    persist(ctx, session_id, &waiting);
    crate::diagnostics::record("INFO", "agent_auto_continue", json!({"sessionId":session_id,"status":"armed"}));
    emit(ctx, session_id, Some(&waiting), None);
}

/// Drop the wait for this session, if any. The user sent a message, rewound, or cancelled it.
pub fn cancel(ctx: &AppCtx, session_id: &str) {
    let removed = with_state(|state| {
        state.fired.remove(session_id);
        state.waiting.remove(session_id).is_some()
    });
    if removed {
        forget(ctx, session_id);
        emit(ctx, session_id, None, None);
    }
}

/// Start the scheduler. Safe to call once per process entry point (GUI and headless).
pub fn start(ctx: AppCtx) {
    std::thread::spawn(move || {
        restore(&ctx);
        loop {
            std::thread::sleep(TICK);
            tick(&ctx);
        }
    });
}

fn restore(ctx: &AppCtx) {
    let rows: Vec<(String, Waiting)> = {
        let conn = ctx.db().conn.lock().unwrap();
        let Ok(mut stmt) = conn.prepare("SELECT session_id,continue_at,limit_type,rearms FROM chat_auto_continue") else { return };
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, Waiting {
                continue_at: row.get(1)?,
                limit_type: row.get(2)?,
                rearms: row.get::<_, i64>(3)?.max(0) as u32,
            }))
        });
        match rows {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(_) => return,
        }
    };
    with_state(|state| {
        for (id, waiting) in rows {
            state.waiting.entry(id).or_insert(waiting);
        }
    });
}

fn tick(ctx: &AppCtx) {
    let ids: Vec<String> = with_state(|state| state.waiting.keys().cloned().collect());
    if ids.is_empty() {
        return;
    }
    if !enabled(ctx) {
        for id in ids {
            cancel(ctx, &id);
        }
        return;
    }
    let now = now_secs();
    let due: Vec<(String, Waiting)> = with_state(|state| {
        let due: Vec<String> = state.waiting.iter()
            .filter(|(_, waiting)| waiting.continue_at <= now)
            .map(|(id, _)| id.clone())
            .collect();
        due.into_iter().filter_map(|id| state.waiting.remove(&id).map(|waiting| (id, waiting))).collect()
    });
    for (id, waiting) in due {
        fire(ctx, &id, waiting);
    }
}

fn fire(ctx: &AppCtx, session_id: &str, waiting: Waiting) {
    // Claiming the row keeps a second backend on the same database from sending the prompt twice.
    let claimed = {
        let conn = ctx.db().conn.lock().unwrap();
        conn.execute(
            "DELETE FROM chat_auto_continue WHERE session_id=?1 AND continue_at=?2",
            params![session_id, waiting.continue_at],
        ).unwrap_or(0) == 1
    };
    emit(ctx, session_id, None, None);
    if !claimed {
        return;
    }
    let session = {
        let conn = ctx.db().conn.lock().unwrap();
        crate::db::repo::get_session(&conn, session_id).ok().flatten()
    };
    let Some(session) = session else { return };
    if session.archived_at.is_some()
        || session.engine != "chat"
        || !matches!(session.kind, SessionKind::Claude | SessionKind::Codex)
        || crate::security::session_active(session_id)
    {
        return;
    }
    // The agent already picked the task up again on its own.
    if ctx.chat().turn_in_progress(session_id) {
        return;
    }
    with_state(|state| state.fired.insert(session_id.to_string(), waiting.rearms));
    let result = (|| {
        if !ctx.chat().is_alive(session_id) {
            crate::command_core::chat_start(ctx, session_id, None, None, false)?;
        }
        ctx.chat().send(ctx, session_id, PROMPT, Vec::new(), "queue")
    })();
    match result {
        Ok(_) => {
            crate::diagnostics::record("INFO", "agent_auto_continue", json!({"sessionId":session_id,"status":"sent"}));
        }
        Err(e) => {
            with_state(|state| state.fired.remove(session_id));
            crate::diagnostic_warn!("auto-continue: failed to continue {session_id}: {e}");
            emit(ctx, session_id, None, Some("failed"));
        }
    }
}

/// Claude's last `rate_limit_info`, when it says the account is refused and when that ends.
pub fn claude_reset(info: Option<&Value>) -> Option<Reset> {
    let info = info?;
    if info.get("status").and_then(Value::as_str) != Some("rejected") {
        return None;
    }
    Some(Reset {
        resets_at: info.get("resetsAt").and_then(Value::as_i64)?,
        limit_type: info.get("rateLimitType").and_then(Value::as_str).map(str::to_string),
    })
}

/// The Codex window that stopped the account: the exhausted window that resets last, or, when no window
/// reads as exhausted and `fullest` is set, the fullest one.
pub fn codex_reset<'a>(windows: impl IntoIterator<Item = &'a CodexRateWindow>, fullest: bool) -> Option<Reset> {
    let windows: Vec<&CodexRateWindow> = windows.into_iter().filter(|w| w.resets_at.is_some()).collect();
    let exhausted = windows.iter().filter(|w| w.used_percent >= 100.0).max_by_key(|w| w.resets_at);
    let chosen = exhausted.or_else(|| {
        if !fullest { return None; }
        windows.iter().max_by(|a, b| {
            a.used_percent.partial_cmp(&b.used_percent).unwrap_or(std::cmp::Ordering::Equal)
                .then(a.resets_at.cmp(&b.resets_at))
        })
    })?;
    Some(Reset {
        resets_at: chosen.resets_at?,
        limit_type: match chosen.window_minutes {
            0 => None,
            minutes if minutes <= 6 * 60 => Some("five_hour".into()),
            minutes if minutes >= 7 * 24 * 60 => Some("seven_day".into()),
            _ => None,
        },
    })
}

/// Merge one sparse `account/rateLimits/updated` snapshot into the buckets seen so far, keyed by limit id.
/// A null field means "not reported this time", not "cleared".
pub fn merge_codex_rate_limits(stored: &mut Value, update: &Value) {
    let Some(update) = update.as_object() else { return };
    let limit_id = update.get("limitId").and_then(Value::as_str).unwrap_or("codex").to_string();
    if !stored.is_object() {
        *stored = json!({});
    }
    let bucket = stored.as_object_mut().unwrap().entry(limit_id).or_insert_with(|| json!({}));
    if !bucket.is_object() {
        *bucket = json!({});
    }
    let bucket = bucket.as_object_mut().unwrap();
    for (key, value) in update {
        if !value.is_null() {
            bucket.insert(key.clone(), value.clone());
        }
    }
}

/// Every window in the merged Codex buckets.
pub fn codex_windows(stored: &Value) -> Vec<CodexRateWindow> {
    let Some(buckets) = stored.as_object() else { return Vec::new() };
    buckets.values()
        .flat_map(|bucket| ["primary", "secondary"].into_iter().filter_map(move |key| bucket.get(key)))
        .filter_map(|window| Some(CodexRateWindow {
            used_percent: window.get("usedPercent")?.as_f64()?,
            window_minutes: window.get("windowDurationMins").and_then(Value::as_u64).unwrap_or(0),
            resets_at: window.get("resetsAt").and_then(Value::as_i64),
        }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(used: f64, minutes: u64, resets: i64) -> CodexRateWindow {
        CodexRateWindow { used_percent: used, window_minutes: minutes, resets_at: Some(resets) }
    }

    #[test]
    fn claude_reset_requires_a_rejection_with_a_reset_time() {
        assert_eq!(
            claude_reset(Some(&json!({"status":"rejected","resetsAt":1_900_000_000,"rateLimitType":"seven_day"}))),
            Some(Reset { resets_at: 1_900_000_000, limit_type: Some("seven_day".into()) }),
        );
        assert_eq!(claude_reset(Some(&json!({"status":"allowed_warning","resetsAt":1}))), None);
        assert_eq!(claude_reset(Some(&json!({"status":"rejected"}))), None);
        assert_eq!(claude_reset(None), None);
    }

    #[test]
    fn codex_reset_prefers_the_exhausted_window_that_resets_last() {
        let windows = [window(100.0, 300, 10), window(100.0, 10080, 50), window(40.0, 300, 90)];
        assert_eq!(codex_reset(&windows, false), Some(Reset { resets_at: 50, limit_type: Some("seven_day".into()) }));
        let partial = [window(97.0, 300, 10), window(60.0, 10080, 50)];
        assert_eq!(codex_reset(&partial, false), None);
        assert_eq!(codex_reset(&partial, true), Some(Reset { resets_at: 10, limit_type: Some("five_hour".into()) }));
    }

    #[test]
    fn sparse_codex_updates_keep_windows_they_do_not_mention() {
        let mut stored = Value::Null;
        merge_codex_rate_limits(&mut stored, &json!({"limitId":"codex","primary":{"usedPercent":80,"windowDurationMins":300,"resetsAt":7},"secondary":{"usedPercent":20,"windowDurationMins":10080,"resetsAt":9}}));
        merge_codex_rate_limits(&mut stored, &json!({"limitId":null,"primary":{"usedPercent":100,"windowDurationMins":300,"resetsAt":8},"secondary":null}));
        let windows = codex_windows(&stored);
        assert_eq!(windows.len(), 2);
        assert_eq!(codex_reset(&windows, false), Some(Reset { resets_at: 8, limit_type: Some("five_hour".into()) }));
    }
}
