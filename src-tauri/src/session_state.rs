//! Authoritative per-session facts, held by the backend and broadcast to every client.
//!
//! The problem this solves: a session's *facts* — what its agent is doing, whether it holds a result
//! nobody has looked at, whether a process is still alive — used to be inferred separately by each
//! client from raw events. Two clients watching the same session could therefore disagree, and both be
//! right by their own reasoning. Nothing arbitrated between them.
//!
//! The rule is the one the session tree has always followed: the backend decides, the backend
//! broadcasts, clients display. A client reports what it observes (see the `session_report_*` and
//! `session_mark_*` commands); it never concludes.
//!
//! Three access points, mirroring the tree's:
//! - `snapshot()` backs the `session_states` command, so a client that just connected — or reconnected
//!   after missing broadcasts — can ask for everything at once;
//! - `STATE_EVENT` is a **connection-level** global event, registered in `web/ws.rs` when the socket
//!   opens rather than when a session is attached. That is what makes a session the client has never
//!   opened still show its state;
//! - the mutators below are the only way a record changes.
//!
//! Like `web::mirror`, this is a process-wide singleton rather than `AppCtx` state: the desktop GUI and
//! the web service share one process, and headless `--serve` runs a single instance per process, so
//! there is exactly one set of sessions to describe.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::host::AppCtx;

/// Broadcast carrying the records that just changed, as `{sessionId: record}`. Registered per
/// connection, never per attached session.
pub const STATE_EVENT: &str = "session://state";

/// How long changes accumulate before one broadcast goes out.
///
/// Agent state changes several times per turn, and a busy tree holds hundreds of sessions; a frame per
/// signal would be pure noise on the socket. Coalescing costs at most this much latency on a marker
/// that a human is about to read anyway.
const COALESCE: Duration = Duration::from_millis(100);

/// One session's authoritative record.
///
/// Fields are added as each part of the model moves to the backend, so that a record never advertises a
/// value nobody maintains: an `alive: false` on a running session would be worse than no field at all.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
#[cfg_attr(test, derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    /// A result is waiting for someone to look at it.
    ///
    /// This is a property of the session, not of the window it happens to be displayed in. Whether to
    /// pop a *system notification* remains each client's own decision — that one depends on which
    /// window has focus, which is a device fact, not a session fact.
    pub unread: bool,
    /// The unread marker was set by hand from the session menu rather than by a new result.
    ///
    /// Clients still show and count it like any other marker, but none pops a system notification for
    /// it: the person who set it already knows, and marking a batch would otherwise fire one per session
    /// on every other client.
    pub unread_manual: bool,
    /// Which agent is running here: `claude`, `codex`, `opencode`, and so on. None for a plain terminal.
    pub agent: Option<String>,
    /// What that agent is doing: `working`, `asking`, `waiting`, or `background`. Meaningless without an
    /// agent.
    pub agent_state: Option<String>,
    /// Where the activity state comes from, declared at launch: `chat` for a conversation-view session
    /// driven over the agent's own protocol, `hooks` or `legacy` for a Codex terminal session. Other
    /// terminal sessions leave it unset.
    pub state_source: Option<String>,
    /// Whether modern Codex has completed its SessionStart handshake, proving its hooks actually run.
    pub hook_ready: bool,
    /// A source covering the full turn lifecycle has reported. Once true, guesses may not override it.
    pub authoritative: bool,
    /// This session has been working at least once, the threshold screen detection needs before it may
    /// call an idle screen `waiting`.
    pub ever_worked: bool,
    /// A process is running behind this session right now.
    ///
    /// Clients used to decide this for themselves, and a client that had not started a session had no way
    /// to tell "not running" from "running, just not opened here" — so it mounted a terminal, and mounting
    /// starts a process. That is how a browser connecting to a desktop that had merely *restored* a
    /// workspace launched every one of those sessions for real.
    pub alive: bool,
    /// Commands this session started with `vrun` that are still running, oldest first.
    pub runs: Vec<crate::agent::runs::RunBrief>,
    /// Whether this session's idle replies are consumed by another workflow session rather than the
    /// user: `"parent"` for an active executor/reviewer waiting on its parent/peer session, or
    /// `"children"` for a planner waiting on dispatched workers. While set, automatic completion
    /// signals (`waiting`, `background`, `notify`) do not raise `unread` or trigger notifications;
    /// interactive `asking` prompts and manual Pending markers still alert the user.
    pub workflow_wait: Option<String>,
    /// Milliseconds since the epoch of the last change, for clients that need to order records.
    pub updated_at: u64,
}

/// How long a `working` result holds the display against a result that ends the turn.
///
/// Agents commonly emit "started" and "finished" within the same instant — a tool call that returns
/// immediately, a turn that produces no output. Showing both would flash green and go straight back, so a
/// finish arriving this soon after a start waits out the remainder before it is applied.
///
/// Conversation-view sessions are exempt. Their states are the protocol's own turn boundaries rather
/// than hook callbacks racing each other, and the pane that shows them already ends the turn at once:
/// holding the dot on `working` would keep its stop button up after the turn is over, and hide a
/// permission question that follows an answered one.
const WORKING_HOLD: Duration = Duration::from_millis(1200);

struct Hub {
    states: HashMap<String, SessionState>,
    /// Sessions changed since the last broadcast.
    dirty: HashSet<String>,
    /// When each session last reported that it started working, for the hold above. Kept out of the
    /// record because it changes on every working signal and would make every one of them a broadcast.
    working_since: HashMap<String, Instant>,
}

fn hub() -> &'static Mutex<Hub> {
    static HUB: OnceLock<Mutex<Hub>> = OnceLock::new();
    HUB.get_or_init(|| {
        Mutex::new(Hub {
            states: HashMap::new(),
            dirty: HashSet::new(),
            working_since: HashMap::new(),
        })
    })
}

/// True while a coalescing flush is already scheduled, so a burst schedules one thread, not one per change.
fn flush_scheduled() -> &'static AtomicBool {
    static SCHEDULED: OnceLock<AtomicBool> = OnceLock::new();
    SCHEDULED.get_or_init(|| AtomicBool::new(false))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Every record currently known, for the `session_states` batch query.
///
/// Sessions with nothing worth saying are simply absent; a client treats a missing record as "no
/// unread, nothing known", which is what a freshly imported session looks like.
pub fn snapshot() -> HashMap<String, SessionState> {
    hub().lock().unwrap().states.clone()
}

/// Chat owns lifecycle facts until another engine declares its source, even after its process stops.
/// Descendant CLIs inherit the session's command environment; their terminal hooks cannot own it.
pub fn has_chat_source(session_id: &str) -> bool {
    hub().lock().unwrap().states.get(session_id)
        .is_some_and(|record| record.state_source.as_deref() == Some("chat"))
}

/// Drop a session's record, called when the session itself is deleted.
///
/// Records are deliberately kept when a *process* exits — an agent that finished its work and left an
/// unread result behind must stay marked — so only removal from the tree clears one.
pub fn forget(session_id: &str) {
    let mut guard = hub().lock().unwrap();
    guard.states.remove(session_id);
    guard.dirty.remove(session_id);
    guard.working_since.remove(session_id);
}

/// Apply `edit` to a session's record and broadcast if anything actually changed.
///
/// Returns whether the record changed. Callers may run on every incoming signal, so an unchanged record
/// must cost nothing: no timestamp bump, no dirty flag, no broadcast.
///
/// One move is not applied straight away. When the edit ends a turn that started less than
/// `WORKING_HOLD` ago, everything else in the edit lands now and the visible state stays `working` until
/// the window closes — see `WORKING_HOLD` for why. The delayed move is re-checked when it fires, so a
/// newer result simply supersedes it.
fn update(ctx: &AppCtx, session_id: &str, edit: impl FnOnce(&mut SessionState)) -> bool {
    update_with(ctx, session_id, true, edit)
}

/// `update`, with the option to skip the end-of-turn hold.
///
/// Skipping is for a state the **user** just caused: after pressing Ctrl+C, a further second of stale
/// green reads as the interrupt having failed. The hold exists for automatic transitions, where the flash
/// has no meaning to anyone.
fn update_with(
    ctx: &AppCtx,
    session_id: &str,
    hold: bool,
    edit: impl FnOnce(&mut SessionState),
) -> bool {
    let mut deferred: Option<(Option<String>, Duration)> = None;
    let changed = {
        let mut guard = hub().lock().unwrap();
        let before = guard.states.get(session_id).cloned().unwrap_or_default();
        let mut candidate = before.clone();
        edit(&mut candidate);

        let left_working =
            before.agent_state.as_deref() == Some("working") && candidate.agent_state.as_deref() != Some("working");
        if left_working && hold && candidate.state_source.as_deref() != Some("chat") {
            if let Some(remaining) = guard
                .working_since
                .get(session_id)
                .and_then(|since| WORKING_HOLD.checked_sub(since.elapsed()))
            {
                deferred = Some((candidate.agent_state.clone(), remaining));
                // Hold the display on `working`; every other part of this edit still applies now. The
                // unread marker in particular belongs to the result, not to when it is shown.
                candidate.agent_state = before.agent_state.clone();
            }
        }
        if candidate.agent_state.as_deref() == Some("working")
            && before.agent_state.as_deref() != Some("working")
        {
            guard.working_since.insert(session_id.to_string(), Instant::now());
        }

        // Compare every field except the timestamp, which the comparison itself decides whether to move.
        let compared = SessionState { updated_at: before.updated_at, ..candidate.clone() };
        if compared == before {
            false
        } else {
            candidate.updated_at = now_ms();
            guard.states.insert(session_id.to_string(), candidate);
            guard.dirty.insert(session_id.to_string());
            true
        }
    };
    if let Some((state, remaining)) = deferred {
        schedule_hold(ctx, session_id, state, remaining);
    }
    if changed {
        schedule_flush(ctx);
    }
    changed
}

/// Apply a held end-of-turn state once its window closes, unless something newer has already moved on.
fn schedule_hold(ctx: &AppCtx, session_id: &str, state: Option<String>, remaining: Duration) {
    let ctx = ctx.clone();
    let session_id = session_id.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(remaining);
        update_with(&ctx, &session_id, false, |record| {
            // Only a session still showing `working` is waiting for this. Anything else means a newer
            // result arrived meanwhile and this one is stale.
            if record.agent_state.as_deref() == Some("working") {
                record.agent_state = state;
            }
        });
    });
}

/// Ensure one broadcast goes out within `COALESCE`, collecting everything that changed meanwhile.
fn schedule_flush(ctx: &AppCtx) {
    if flush_scheduled().swap(true, Ordering::SeqCst) {
        return;
    }
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        std::thread::sleep(COALESCE);
        // Clear the flag before reading the dirty set, so a change landing during this drain schedules
        // the next flush instead of being held until some later change happens to come along.
        flush_scheduled().store(false, Ordering::SeqCst);
        let batch = drain_dirty();
        if !batch.is_empty() {
            ctx.emit(STATE_EVENT, batch);
        }
    });
}

/// Take the changed records, leaving the dirty set empty.
fn drain_dirty() -> HashMap<String, SessionState> {
    let mut guard = hub().lock().unwrap();
    let ids: Vec<String> = guard.dirty.drain().collect();
    ids.into_iter()
        .filter_map(|id| guard.states.get(&id).map(|s| (id.clone(), s.clone())))
        .collect()
}

/// Raise or clear a session's unread marker.
///
/// Clearing is what `session_mark_read` does after a client has had the session on screen, in a focused
/// window, for long enough to count as read. Raising happens from `observe_status` below, and — until
/// screen detection reports through the backend — from a client that saw something worth a look.
pub fn set_unread(ctx: &AppCtx, session_id: &str, unread: bool) -> bool {
    update(ctx, session_id, |s| {
        s.unread = unread;
        s.unread_manual = false;
    })
}

/// Raise the unread marker by hand, from the session menu's Mark as Pending.
///
/// Differs from `set_unread` only in flagging the marker as manual, so clients skip the system
/// notification a new result would bring. A marker already raised by a result keeps its origin.
pub fn mark_pending(ctx: &AppCtx, session_id: &str) -> bool {
    update(ctx, session_id, |s| {
        if !s.unread {
            s.unread = true;
            s.unread_manual = true;
        }
    })
}

/// Record whether a process is running behind this session.
///
/// Set when one starts and cleared when it ends, including a deliberate kill. The record itself survives:
/// a session whose agent finished and left an unread result behind still has something to say.
pub fn set_alive(ctx: &AppCtx, session_id: &str, alive: bool) -> bool {
    update(ctx, session_id, |s| s.alive = alive)
}

/// Record the session's live `vrun` commands, as the runs registry last found them.
pub fn set_runs(ctx: &AppCtx, session_id: &str, runs: Vec<crate::agent::runs::RunBrief>) -> bool {
    update(ctx, session_id, |s| s.runs = runs)
}

/// Synchronize active workflow wait roles across all known sessions in one pass.
///
/// Entering a workflow wait clears any non-manual `unread` marker unless the session is actively
/// `asking` the user for confirmation, so stale Pending dots clear when work is handed off.
pub fn sync_workflow_waits(ctx: &AppCtx, desired: HashMap<String, &'static str>) -> bool {
    let changed = {
        let mut guard = hub().lock().unwrap();
        let mut ids: HashSet<String> = guard
            .states
            .iter()
            .filter_map(|(id, record)| record.workflow_wait.as_ref().map(|_| id.clone()))
            .collect();
        ids.extend(desired.keys().cloned());

        let mut any_changed = false;
        for session_id in ids {
            let next = desired.get(&session_id).map(|role| (*role).to_string());
            let before = match guard.states.get(&session_id) {
                Some(existing) => existing.clone(),
                None if next.is_some() => SessionState::default(),
                None => continue,
            };
            let mut candidate = before.clone();
            let entered_wait = before.workflow_wait.is_none() && next.is_some();
            candidate.workflow_wait = next;
            if entered_wait
                && !candidate.unread_manual
                && candidate.agent_state.as_deref() != Some("asking")
            {
                candidate.unread = false;
            }
            let compared = SessionState { updated_at: before.updated_at, ..candidate.clone() };
            if compared != before {
                candidate.updated_at = now_ms();
                guard.states.insert(session_id.clone(), candidate);
                guard.dirty.insert(session_id);
                any_changed = true;
            }
        }
        any_changed
    };
    if changed {
        schedule_flush(ctx);
    }
    changed
}

/// A deliberately stopped chat process has no active turn. Apply this immediately, without the
/// display hold, so a delayed Waiting transition cannot overwrite a replacement's Working state.
pub fn set_stopped(ctx: &AppCtx, session_id: &str) -> bool {
    update_with(ctx, session_id, false, |record| {
        record.alive = false;
        record.agent_state = Some("waiting".to_string());
    })
}

/// A restarted host explicitly declares dormant chat sessions instead of retaining a client's old work state.
pub fn restore_stopped_chat(ctx: &AppCtx, session_id: &str, kind: &str) {
    update_with(ctx,session_id,false,|record| {
        record.alive=false;
        record.agent=Some(kind.to_string());
        record.agent_state=Some("waiting".into());
        record.state_source=Some("chat".into());
        record.authoritative=true;
    });
}

/// Agent states that mean "a human should look at this". `background` counts: its turn has a reply to
/// read even though the work that turn started is still running.
const NOTIFY_STATES: [&str; 3] = ["asking", "waiting", "background"];

/// Inspect a `pty://status/{id}` payload on its way out and raise the unread marker when it warrants one.
///
/// `previous` is the state this session last broadcast, read before the status cache is overwritten.
/// The conditions match what every client used to apply for itself:
/// - a **state** signal that is not `silent` (a silent one is a correction or a snapshot replayed on
///   attach, not a new result), whose state is asking, waiting, or background, and which is an actual
///   transition rather than a repeat of what is already displayed;
/// - a **notify** signal (OSC 9 / OSC 777) on a session with no authoritative hook source, which is the
///   same fallback rule the frontend applied to avoid duplicating a hook-driven notification.
///
/// The same kind of transition into `working` clears the marker. The session has moved past the result
/// or question the marker pointed at, and keeping it would hide the working state behind it until
/// someone opened the session. The marker comes back when this turn ends, with the new result.
///
/// What deliberately does *not* appear here is the visibility test. A client used to skip the marker
/// when its own window was focused and showing the session; that made "unread" mean "unread on this
/// device". The marker is now raised unconditionally, and the client watching the session clears it
/// moments later through the normal read path.
pub fn observe_status(
    ctx: &AppCtx,
    session_id: &str,
    payload: &serde_json::Value,
    previous: Option<&serde_json::Value>,
) {
    let kind = payload.get("kind").and_then(serde_json::Value::as_str);
    // The state a non-silent state signal moves the session into, when it differs from the one displayed.
    let entered = match kind {
        Some("state") => {
            let silent = payload
                .get("silent")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let state = payload.get("state").and_then(serde_json::Value::as_str);
            let prev_state = previous
                .and_then(|p| p.get("state"))
                .and_then(serde_json::Value::as_str);
            state.filter(|_| !silent && state != prev_state)
        }
        _ => None,
    };
    let notable = match kind {
        Some("state") => entered.is_some_and(|s| NOTIFY_STATES.contains(&s)),
        Some("notify") => !previous
            .and_then(|p| p.get("authoritative"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        _ => false,
    };
    let resumed = entered == Some("working");
    let mut notify_state: Option<&str> = None;
    // One update for both, so a signal that changes state and raises the marker produces one broadcast.
    update(ctx, session_id, |record| {
        apply_signal(record, payload);
        // Active workflow sessions awaiting a parent or child peer do not need user attention for
        // ordinary turn completions; only interactive `asking` prompts still raise Pending.
        let suppress_unread = record.workflow_wait.is_some() && entered != Some("asking");
        if notable && !suppress_unread {
            record.unread = true;
            record.unread_manual = false;
            notify_state = Some(
                payload
                    .get("state")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("waiting"),
            );
        }
        if resumed {
            record.unread = false;
            record.unread_manual = false;
        }
    });
    if let Some(state) = notify_state {
        crate::mobile_push::observe(ctx, session_id, state, payload.get("body").and_then(serde_json::Value::as_str));
    }
}

/// Fold one status signal into a session's record.
///
/// These rules are the ones every client used to apply to its own copy, moved here **unchanged** so
/// that this step only relocates the answer and does not alter it. Each exists because of a specific
/// failure:
/// - an authoritative source latches `authoritative`, because a hook that reports the whole turn must
///   not be second-guessed afterwards by a guess drawn from raw output;
/// - `busy` (sustained output) may stand in for state only while there is an agent, it is not Codex,
///   and nothing authoritative has spoken. Codex is excluded outright: its modern builds are hook-only,
///   and inferring an exact state from legacy output would be inventing precision that is not there;
/// - `ever_worked` latches on any working result, because screen detection may only read an idle screen
///   as `waiting` for a session that has actually worked — otherwise a freshly opened, empty prompt
///   would report a finished turn that never ran.
///
/// Screen detection itself is absent on purpose: it reads a rendered terminal grid, which exists only in
/// a client. It reaches the backend as a report in a later step.
fn apply_signal(record: &mut SessionState, payload: &serde_json::Value) {
    let str_at = |key: &str| {
        payload
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    };
    let bool_at = |key: &str| {
        payload
            .get(key)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    };
    match payload.get("kind").and_then(serde_json::Value::as_str) {
        Some("agent") => {
            let agent = str_at("agent");
            match agent.as_deref() {
                None => {
                    // The agent is gone — the session dropped back to a plain shell. Everything derived
                    // from it goes with it, or a stale state would keep colouring a shell prompt.
                    record.agent_state = None;
                    record.state_source = None;
                    record.authoritative = false;
                    record.hook_ready = false;
                }
                Some("codex") => {
                    // Codex declares its source at launch. A hook-capable session counts as authoritative
                    // from that moment, before its first lifecycle event, so nothing guessed from output
                    // can win the startup race. Any state replayed by an earlier attach is dropped: the
                    // cached lifecycle snapshot follows this marker and restores the real one.
                    record.state_source = Some(str_at("state_source").unwrap_or_else(|| "legacy".to_string()));
                    record.authoritative = record.state_source.as_deref() == Some("hooks");
                    record.agent_state = None;
                    record.hook_ready = false;
                }
                Some(_) => record.state_source = str_at("state_source"),
            }
            record.agent = agent;
        }
        Some("hook_ready") => {
            // Only modern Codex sends this, and only its hook-only mode has a handshake to prove.
            if record.agent.as_deref() == Some("codex") && record.state_source.as_deref() == Some("hooks") {
                record.hook_ready = true;
            }
        }
        Some("state") => {
            let state = str_at("state");
            let authoritative = bool_at("authoritative");
            // Every agent but Codex and Crush covers its whole turn, so any state from one settles the question.
            // Codex is an exception because its legacy notification reports only completion; taking that
            // as full authority would freeze the session on `waiting` for the rest of its life. Crush is the
            // mirror case: its only hook, PreToolUse, reports `working` and nothing else, so latching on it
            // would discard the screen readings that are its only route back to `waiting` or `asking`.
            let partial_hooks = matches!(record.agent.as_deref(), Some("codex") | Some("crush"));
            if !partial_hooks || authoritative {
                record.authoritative = true;
            }
            if record.agent.as_deref() == Some("codex") && authoritative {
                record.hook_ready = true;
            }
            if state.as_deref() == Some("working") {
                record.ever_worked = true;
            }
            record.agent_state = state;
        }
        Some("busy") => {
            let busy = bool_at("busy");
            if record.agent.is_some()
                && record.agent.as_deref() != Some("codex")
                && !record.authoritative
            {
                record.agent_state =
                    Some(if busy { "working" } else { "waiting" }.to_string());
                if busy {
                    record.ever_worked = true;
                }
            }
        }
        _ => {}
    }
}

/// What a client saw on a session's rendered terminal screen.
///
/// This mirrors the frontend's `ScreenDetection` because the reading itself cannot move here: the backend
/// parses the output stream only far enough to recognise bells and OSC sequences, while this reads a laid
/// out grid — the box-drawing rules, the prompt glyph, the spinner frames — which exists only where a
/// terminal is actually rendered. So the client reports what it saw, and the arbitration stays here.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenReport {
    /// The state the screen suggests.
    pub state: String,
    /// A confirmation prompt is plainly on screen — a strong signal, not a guess.
    pub visible_blocker: bool,
    /// A work indicator is plainly on screen.
    pub visible_working: bool,
    /// The screen is a transcript viewer or some other non-live view and says nothing about state.
    pub skip: bool,
}

/// Fold a screen reading from the client that is displaying this terminal.
///
/// `reporter` must be the session's current size owner — the one client actually rendering it. Two
/// clients showing the same session render it at the same grid because only the owner may resize, so
/// only the owner's reading describes what the session really looks like; anyone else is reading a
/// scaled copy and is refused.
///
/// Everything after that is the arbitration this used to do in the client, unchanged:
/// - Codex is never read from its screen at all. Its official events are its only source, and an absent
///   event stays visibly absent rather than being guessed at;
/// - an authoritative source outranks the screen outright;
/// - a visible confirmation prompt or a visible work indicator is taken at face value;
/// - anything weaker needs the session to have worked at least once, or a freshly opened empty prompt
///   reads as a finished turn that never ran.
pub fn report_screen(
    ctx: &AppCtx,
    session_id: &str,
    reporter: &str,
    owner: Option<&str>,
    screen: ScreenReport,
) -> Result<(), String> {
    if owner != Some(reporter) {
        return Err("screen report ignored: this client does not own the terminal".to_string());
    }
    if screen.skip {
        return Ok(());
    }
    update(ctx, session_id, |record| {
        if record.agent.is_none()
            || record.agent.as_deref() == Some("codex")
            || record.authoritative
        {
            return;
        }
        let resolved = if screen.visible_blocker {
            "asking"
        } else if screen.visible_working {
            "working"
        } else if !record.ever_worked {
            // Neither indicator is on screen and this session has never worked: there is nothing here
            // worth overriding a quiet state with.
            return;
        } else {
            match screen.state.as_str() {
                "working" | "asking" | "waiting" => &screen.state,
                _ => return,
            }
        };
        if resolved == "working" {
            record.ever_worked = true;
        }
        record.agent_state = Some(resolved.to_string());
    });
    Ok(())
}

/// Fold a user interrupt — Ctrl+C, or Esc on its own — reported by a client.
///
/// Claude and the other fallback agents emit no completion event after an interrupt, so without this the
/// session sits on `working` until something else happens to correct it. Codex is excluded: it is
/// hook-only, its Stop event owns this transition, and a missing one must stay visible rather than be
/// papered over. Only a session that is currently working has anything to interrupt.
pub fn report_interrupt(ctx: &AppCtx, session_id: &str) {
    update_with(ctx, session_id, false, |record| {
        if record.agent.is_none() || record.agent.as_deref() == Some("codex") {
            return;
        }
        if record.agent_state.as_deref() == Some("working") {
            record.agent_state = Some("waiting".to_string());
        }
    });
}

/// Serialize tests that touch the hub and start each from an empty one.
///
/// The hub is a process-wide singleton while cargo runs tests in parallel threads, so any two tests
/// would otherwise see each other's records. Waiting out one coalescing window before resetting also
/// lets a flush thread left over from the previous test finish: it would otherwise drain this test's
/// changes into the previous test's context, where nobody is listening.
#[cfg(test)]
pub fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let guard = LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    std::thread::sleep(COALESCE * 2);
    let mut hub = hub().lock().unwrap();
    hub.states.clear();
    hub.dirty.clear();
    hub.working_since.clear();
    flush_scheduled().store(false, Ordering::SeqCst);
    guard
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A headless AppCtx over a throwaway database, matching the helper used elsewhere.
    fn ctx(tag: &str) -> AppCtx {
        let dir = std::env::temp_dir().join(format!("vlx-session-state-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("t.db")).unwrap();
        AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(dir, db)))
    }

    fn state(s: &str, silent: bool) -> serde_json::Value {
        json!({ "kind": "state", "state": s, "silent": silent, "authoritative": true })
    }

    fn unread_of(sid: &str) -> bool {
        snapshot().get(sid).map(|s| s.unread).unwrap_or(false)
    }

    /// A turn that ends with a result nobody has seen raises the marker. It is raised for the session,
    /// not for one client: the visibility test that used to suppress it lived on the client and made
    /// "unread" mean "unread in this window".
    #[test]
    fn a_finished_turn_raises_the_marker() {
        let _lock = test_lock();
        let app = ctx("finished");
        observe_status(&app, "s1", &state("waiting", false), Some(&state("working", false)));
        assert!(unread_of("s1"));
    }

    /// A turn that ends with its background work still running has a reply to read all the same, and
    /// that work finishing later is a result of its own.
    #[test]
    fn a_turn_ending_on_background_raises_the_marker() {
        let _lock = test_lock();
        let app = ctx("background");
        observe_status(&app, "s1", &state("background", false), Some(&state("working", false)));
        assert!(unread_of("s1"));
        assert_eq!(snapshot()["s1"].agent_state.as_deref(), Some("background"));
        set_unread(&app, "s1", false);
        observe_status(&app, "s1", &state("waiting", false), Some(&state("background", false)));
        assert!(unread_of("s1"));
    }

    /// Work starting is not something to read. Only asking and waiting mean a human is needed.
    #[test]
    fn starting_work_raises_nothing() {
        let _lock = test_lock();
        let app = ctx("working");
        observe_status(&app, "s1", &state("working", false), None);
        assert!(!unread_of("s1"));
    }

    /// Work starting again clears the marker. The session has moved past the result it pointed at, and
    /// the working state must show instead of a marker nobody has looked at yet.
    #[test]
    fn resuming_work_clears_the_marker() {
        let _lock = test_lock();
        let app = ctx("resume");
        observe_status(&app, "s1", &state("waiting", false), Some(&state("working", false)));
        assert!(unread_of("s1"));
        observe_status(&app, "s1", &state("working", false), Some(&state("waiting", false)));
        assert!(!unread_of("s1"));
        assert_eq!(snapshot()["s1"].agent_state.as_deref(), Some("working"));
    }

    /// Only a real transition clears it. A silent replay on attach, or a repeat of the working state
    /// already shown, says nothing new about the session.
    #[test]
    fn a_silent_or_repeated_working_keeps_the_marker() {
        let _lock = test_lock();
        let app = ctx("resume-silent");
        set_unread(&app, "s1", true);
        observe_status(&app, "s1", &state("working", true), Some(&state("waiting", false)));
        assert!(unread_of("s1"));
        observe_status(&app, "s1", &state("working", false), Some(&state("working", false)));
        assert!(unread_of("s1"));
    }

    /// A silent signal is a correction or a snapshot replayed on attach, not a new result. Marking on
    /// one would light up every session a client attaches to.
    #[test]
    fn a_silent_correction_raises_nothing() {
        let _lock = test_lock();
        let app = ctx("silent");
        observe_status(&app, "s1", &state("waiting", true), Some(&state("working", false)));
        assert!(!unread_of("s1"));
    }

    /// Repeating the state already on display is not a transition. Agents re-emit their state freely,
    /// and each repeat would otherwise re-raise a marker the user just cleared.
    #[test]
    fn repeating_the_current_state_raises_nothing() {
        let _lock = test_lock();
        let app = ctx("repeat");
        observe_status(&app, "s1", &state("waiting", false), Some(&state("waiting", false)));
        assert!(!unread_of("s1"));
    }

    /// OSC 9 / OSC 777 is the fallback for sessions whose agent has no authoritative hooks. Honouring
    /// it on a hook-driven session would mark the same turn twice.
    #[test]
    fn a_terminal_notification_counts_only_without_authoritative_hooks() {
        let _lock = test_lock();
        let app = ctx("notify");
        let notify = json!({ "kind": "notify", "body": "done" });

        observe_status(&app, "s1", &notify, None);
        assert!(unread_of("s1"), "no hook source: the fallback is all there is");

        observe_status(&app, "s2", &notify, Some(&state("working", false)));
        assert!(!unread_of("s2"), "hooks already cover this session");
    }

    /// Reading clears the marker for everyone, and the record stops changing once it agrees — an
    /// unchanged record must not bump its timestamp or queue another broadcast.
    #[test]
    fn reading_clears_the_marker_and_settles() {
        let _lock = test_lock();
        let app = ctx("read");
        assert!(set_unread(&app, "s1", true));
        assert!(!set_unread(&app, "s1", true), "an unchanged record reports no change");
        let stamped = snapshot()["s1"].updated_at;
        assert!(set_unread(&app, "s1", false));
        assert!(!unread_of("s1"));
        assert!(snapshot()["s1"].updated_at >= stamped);
    }

    /// Deleting a session drops its record. A process exiting must not, or an agent that finished its
    /// work and left a result behind would lose the very marker pointing at it.
    #[test]
    fn deleting_a_session_drops_its_record() {
        let _lock = test_lock();
        let app = ctx("forget");
        set_unread(&app, "s1", true);
        forget("s1");
        assert!(snapshot().get("s1").is_none());
    }

    /// Feed `payload` through the same path a real signal takes and return the resulting record.
    fn feed(app: &AppCtx, sid: &str, payloads: &[serde_json::Value]) -> SessionState {
        let mut previous: Option<serde_json::Value> = None;
        for p in payloads {
            observe_status(app, sid, p, previous.as_ref());
            if p.get("kind").and_then(serde_json::Value::as_str) == Some("state") {
                previous = Some(p.clone());
            }
        }
        snapshot().get(sid).cloned().unwrap_or_default()
    }

    /// The agent and its state reach the record, which is what lets a client show a dot on a session it
    /// has never opened — the whole point of moving this to the backend.
    #[test]
    fn the_record_follows_the_agent_and_its_state() {
        let _lock = test_lock();
        let app = ctx("agent");
        let got = feed(
            &app,
            "s1",
            &[
                json!({ "kind": "agent", "agent": "claude" }),
                json!({ "kind": "state", "state": "working", "authoritative": true }),
            ],
        );
        assert_eq!(got.agent.as_deref(), Some("claude"));
        assert_eq!(got.agent_state.as_deref(), Some("working"));
        assert!(got.authoritative);
        assert!(got.ever_worked, "working latches the screen-detection threshold");
    }

    /// An agent that goes away takes everything derived from it along. A leftover state would keep
    /// colouring what is now an ordinary shell prompt.
    #[test]
    fn losing_the_agent_clears_what_it_implied() {
        let _lock = test_lock();
        let app = ctx("agent-gone");
        let got = feed(
            &app,
            "s1",
            &[
                json!({ "kind": "agent", "agent": "claude" }),
                json!({ "kind": "state", "state": "waiting", "authoritative": true }),
                json!({ "kind": "agent", "agent": serde_json::Value::Null }),
            ],
        );
        assert!(got.agent.is_none());
        assert!(got.agent_state.is_none());
        assert!(!got.authoritative);
    }

    /// Sustained output may stand in for state only until something authoritative speaks. A hook that
    /// reports the whole turn must not be second-guessed afterwards by a guess drawn from raw bytes.
    #[test]
    fn output_activity_yields_to_an_authoritative_source() {
        let _lock = test_lock();
        let app = ctx("busy");

        let guessed = feed(
            &app,
            "s1",
            &[
                json!({ "kind": "agent", "agent": "crush" }),
                json!({ "kind": "busy", "busy": true }),
            ],
        );
        assert_eq!(guessed.agent_state.as_deref(), Some("working"), "no better source yet");

        feed(
            &app,
            "s1",
            &[
                json!({ "kind": "state", "state": "asking", "authoritative": true }),
                json!({ "kind": "busy", "busy": false }),
            ],
        );
        // The end-of-turn hold delays the visible move off `working`; wait it out before reading.
        std::thread::sleep(WORKING_HOLD + Duration::from_millis(300));
        assert_eq!(
            snapshot()["s1"].agent_state.as_deref(),
            Some("asking"),
            "the hook still holds"
        );
    }

    /// Codex never derives state from output. Modern builds are hook-only, and inferring an exact state
    /// from legacy output would invent precision that is not there.
    #[test]
    fn codex_never_derives_state_from_output() {
        let _lock = test_lock();
        let app = ctx("codex-busy");
        let got = feed(
            &app,
            "s1",
            &[
                json!({ "kind": "agent", "agent": "codex", "state_source": "hooks" }),
                json!({ "kind": "busy", "busy": true }),
            ],
        );
        assert!(got.agent_state.is_none());
        assert_eq!(got.state_source.as_deref(), Some("hooks"));
    }

    /// The Codex handshake counts only for the hook-only mode that has something to prove.
    #[test]
    fn the_hook_handshake_counts_only_for_hook_driven_codex() {
        let _lock = test_lock();
        let app = ctx("handshake");

        let proven = feed(
            &app,
            "s1",
            &[
                json!({ "kind": "agent", "agent": "codex", "state_source": "hooks" }),
                json!({ "kind": "hook_ready" }),
            ],
        );
        assert!(proven.hook_ready);

        let irrelevant = feed(
            &app,
            "s2",
            &[
                json!({ "kind": "agent", "agent": "claude" }),
                json!({ "kind": "hook_ready" }),
            ],
        );
        assert!(!irrelevant.hook_ready);
    }

    /// Whether a process stands behind a session is a fact about the session, so it lives in the record
    /// and reaches clients that never started it. Ending the process does not end the record: a session
    /// whose agent finished and left an unread result behind still has something to say.
    #[test]
    fn the_record_tracks_whether_a_process_is_running() {
        let _lock = test_lock();
        let app = ctx("alive");

        assert!(set_alive(&app, "s1", true));
        assert!(snapshot()["s1"].alive);
        assert!(!set_alive(&app, "s1", true), "an unchanged record reports no change");

        set_unread(&app, "s1", true);
        assert!(set_alive(&app, "s1", false));
        assert!(!snapshot()["s1"].alive);
        assert!(snapshot()["s1"].unread, "the marker outlives the process");
    }

    fn screen(state: &str, blocker: bool, working: bool) -> ScreenReport {
        ScreenReport {
            state: state.to_string(),
            visible_blocker: blocker,
            visible_working: working,
            skip: false,
        }
    }

    /// Only the client rendering the terminal at its real size may report what is on it. Anyone else is
    /// looking at a scaled copy, and the copy is what the arbitration must not be fed.
    #[test]
    fn only_the_size_owner_may_report_a_screen() {
        let _lock = test_lock();
        let app = ctx("owner");
        feed(&app, "s1", &[json!({ "kind": "agent", "agent": "crush" })]);

        assert!(report_screen(&app, "s1", "ws-2", Some("desktop"), screen("waiting", false, false)).is_err());
        assert!(report_screen(&app, "s1", "desktop", Some("desktop"), screen("working", false, true)).is_ok());
        assert_eq!(snapshot()["s1"].agent_state.as_deref(), Some("working"));
    }

    /// An authoritative source outranks the screen, and Codex is never read from its screen at all.
    #[test]
    fn a_screen_reading_never_overrides_official_events() {
        let _lock = test_lock();
        let app = ctx("screen-vs-hooks");

        feed(
            &app,
            "s1",
            &[
                json!({ "kind": "agent", "agent": "claude" }),
                json!({ "kind": "state", "state": "asking", "authoritative": true }),
            ],
        );
        report_screen(&app, "s1", "desktop", Some("desktop"), screen("waiting", false, false)).unwrap();
        assert_eq!(snapshot()["s1"].agent_state.as_deref(), Some("asking"));

        feed(&app, "s2", &[json!({ "kind": "agent", "agent": "codex", "state_source": "hooks" })]);
        report_screen(&app, "s2", "desktop", Some("desktop"), screen("working", false, true)).unwrap();
        assert!(snapshot()["s2"].agent_state.is_none(), "codex is never read from its screen");
    }

    /// Crush's only hook reports `working`. It must not lock the session, or the screen could never bring a
    /// finished turn back to `waiting` or show a permission prompt.
    #[test]
    fn crush_working_hook_leaves_the_screen_in_charge() {
        let _lock = test_lock();
        let app = ctx("crush-partial");
        feed(
            &app,
            "s1",
            &[
                json!({ "kind": "agent", "agent": "crush" }),
                json!({ "kind": "state", "state": "working", "authoritative": false }),
            ],
        );
        assert_eq!(snapshot()["s1"].agent_state.as_deref(), Some("working"));
        assert!(!snapshot()["s1"].authoritative);

        // Let the end-of-turn hold expire so the screen's result is what is displayed.
        std::thread::sleep(WORKING_HOLD + Duration::from_millis(300));
        report_screen(&app, "s1", "desktop", Some("desktop"), screen("waiting", true, false)).unwrap();
        assert_eq!(snapshot()["s1"].agent_state.as_deref(), Some("asking"));
        report_screen(&app, "s1", "desktop", Some("desktop"), screen("waiting", false, false)).unwrap();
        assert_eq!(snapshot()["s1"].agent_state.as_deref(), Some("waiting"));
    }

    /// A session that has never worked cannot be talked into `waiting` by a quiet screen: a freshly
    /// opened, empty prompt looks exactly like a finished turn.
    #[test]
    fn a_quiet_screen_needs_a_session_that_has_actually_worked() {
        let _lock = test_lock();
        let app = ctx("ever-worked");
        feed(&app, "s1", &[json!({ "kind": "agent", "agent": "crush" })]);

        report_screen(&app, "s1", "desktop", Some("desktop"), screen("waiting", false, false)).unwrap();
        assert!(snapshot()["s1"].agent_state.is_none(), "nothing has run yet");

        // A plainly visible confirmation prompt needs no such history: it is on screen right now.
        report_screen(&app, "s1", "desktop", Some("desktop"), screen("waiting", true, false)).unwrap();
        assert_eq!(snapshot()["s1"].agent_state.as_deref(), Some("asking"));
    }

    /// A transcript viewer is not the live screen and says nothing about state.
    #[test]
    fn a_non_live_screen_is_ignored() {
        let _lock = test_lock();
        let app = ctx("skip");
        feed(
            &app,
            "s1",
            &[
                json!({ "kind": "agent", "agent": "crush" }),
                json!({ "kind": "busy", "busy": true }),
            ],
        );
        let mut view = screen("waiting", false, false);
        view.skip = true;

        report_screen(&app, "s1", "desktop", Some("desktop"), view).unwrap();

        assert_eq!(snapshot()["s1"].agent_state.as_deref(), Some("working"));
    }

    /// Ctrl+C ends a turn that emits no completion event of its own — except for Codex, whose Stop event
    /// owns that transition and whose absence must stay visible.
    #[test]
    fn an_interrupt_ends_a_turn_that_reports_nothing() {
        let _lock = test_lock();
        let app = ctx("interrupt");

        feed(
            &app,
            "s1",
            &[
                json!({ "kind": "agent", "agent": "claude" }),
                json!({ "kind": "state", "state": "working", "authoritative": true }),
            ],
        );
        // An interrupt the user just typed applies at once: another second of stale green would read as
        // the interrupt having failed.
        report_interrupt(&app, "s1");
        assert_eq!(snapshot()["s1"].agent_state.as_deref(), Some("waiting"));

        feed(
            &app,
            "s2",
            &[
                json!({ "kind": "agent", "agent": "codex", "state_source": "hooks" }),
                json!({ "kind": "state", "state": "working", "authoritative": true }),
            ],
        );
        report_interrupt(&app, "s2");
        assert_eq!(snapshot()["s2"].agent_state.as_deref(), Some("working"));
    }

    /// A turn that finishes the instant it starts would flash green and go straight back. The finish
    /// waits out the window; everything else in the same edit still lands immediately.
    #[test]
    fn a_turn_that_ends_immediately_holds_its_working_state() {
        let _lock = test_lock();
        let app = ctx("hold");
        feed(
            &app,
            "s1",
            &[
                json!({ "kind": "agent", "agent": "claude" }),
                json!({ "kind": "state", "state": "working", "authoritative": true }),
                json!({ "kind": "state", "state": "waiting", "authoritative": true }),
            ],
        );

        assert_eq!(snapshot()["s1"].agent_state.as_deref(), Some("working"), "held");
        assert!(snapshot()["s1"].unread, "the result is there even while the display waits");

        std::thread::sleep(WORKING_HOLD + Duration::from_millis(300));
        assert_eq!(snapshot()["s1"].agent_state.as_deref(), Some("waiting"), "released");
    }

    /// A finish arriving after the window is applied at once; nothing is held that does not need to be.
    #[test]
    fn a_normal_turn_ends_without_waiting() {
        let _lock = test_lock();
        let app = ctx("no-hold");
        feed(
            &app,
            "s1",
            &[
                json!({ "kind": "agent", "agent": "claude" }),
                json!({ "kind": "state", "state": "working", "authoritative": true }),
            ],
        );
        std::thread::sleep(WORKING_HOLD + Duration::from_millis(50));

        feed(&app, "s1", &[json!({ "kind": "state", "state": "waiting", "authoritative": true })]);

        assert_eq!(snapshot()["s1"].agent_state.as_deref(), Some("waiting"));
    }

    /// A conversation-view session reports real turn boundaries, so its finish lands at once, and so does
    /// a permission question that follows straight after an answered one.
    #[test]
    fn a_chat_session_is_never_held() {
        let _lock = test_lock();
        let app = ctx("chat-no-hold");
        feed(
            &app,
            "chat-a",
            &[
                json!({ "kind": "agent", "agent": "claude", "state_source": "chat" }),
                json!({ "kind": "state", "state": "working", "authoritative": true }),
                json!({ "kind": "state", "state": "waiting", "authoritative": true }),
            ],
        );
        assert_eq!(snapshot()["chat-a"].agent_state.as_deref(), Some("waiting"));

        feed(
            &app,
            "chat-b",
            &[
                json!({ "kind": "agent", "agent": "codex", "state_source": "chat" }),
                json!({ "kind": "state", "state": "working", "authoritative": true }),
                json!({ "kind": "state", "state": "asking", "authoritative": true }),
            ],
        );
        let record = snapshot()["chat-b"].clone();
        assert_eq!(record.agent_state.as_deref(), Some("asking"));
        assert!(record.authoritative);

        // The same session back in a terminal declares its own source, and the hold applies again.
        feed(
            &app,
            "chat-a",
            &[
                json!({ "kind": "agent", "agent": "claude" }),
                json!({ "kind": "state", "state": "working", "authoritative": true }),
                json!({ "kind": "state", "state": "waiting", "authoritative": true }),
            ],
        );
        assert_eq!(snapshot()["chat-a"].agent_state.as_deref(), Some("working"), "held");
    }

    /// Changes accumulate into one broadcast instead of one frame per signal, and the payload carries
    /// only what changed.
    #[test]
    fn changes_coalesce_into_one_broadcast() {
        let _lock = test_lock();
        let app = ctx("coalesce");
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let sink = seen.clone();
        app.listen(STATE_EVENT, move |payload| {
            sink.lock().unwrap().push(payload.to_string());
        });

        set_unread(&app, "s1", true);
        set_unread(&app, "s2", true);
        set_unread(&app, "s3", true);
        std::thread::sleep(COALESCE * 3);

        let frames = seen.lock().unwrap().clone();
        assert_eq!(frames.len(), 1, "three changes, one frame: {frames:?}");
        let batch: HashMap<String, SessionState> = serde_json::from_str(&frames[0]).unwrap();
        assert_eq!(batch.len(), 3);
        assert!(batch.values().all(|s| s.unread));
    }

    #[test]
    fn workflow_wait_suppresses_waiting_unread_but_keeps_asking_and_manual() {
        let _lock = test_lock();
        let app = ctx("workflow-wait");

        let mut waits = HashMap::new();
        waits.insert("exec".to_string(), "parent");
        assert!(sync_workflow_waits(&app, waits));
        assert_eq!(snapshot()["exec"].workflow_wait.as_deref(), Some("parent"));

        // Normal turn completions hand off to the parent session instead of raising Pending.
        feed(
            &app,
            "exec",
            &[
                json!({ "kind": "agent", "agent": "claude", "state_source": "chat" }),
                json!({ "kind": "state", "state": "working", "authoritative": true }),
                json!({ "kind": "state", "state": "waiting", "authoritative": true }),
            ],
        );
        let record = snapshot()["exec"].clone();
        assert_eq!(record.agent_state.as_deref(), Some("waiting"));
        assert!(!record.unread, "executor waiting on parent must not become unread");

        // Background turn completion is likewise quiet while awaiting the workflow.
        feed(
            &app,
            "exec",
            &[
                json!({ "kind": "state", "state": "working", "authoritative": true }),
                json!({ "kind": "state", "state": "background", "authoritative": true }),
            ],
        );
        assert!(!snapshot()["exec"].unread);

        // Permission prompts still need the user, so `asking` raises unread as usual.
        feed(
            &app,
            "exec",
            &[
                json!({ "kind": "state", "state": "working", "authoritative": true }),
                json!({ "kind": "state", "state": "asking", "authoritative": true }),
            ],
        );
        assert!(snapshot()["exec"].unread, "interactive asking must still alert the user");

        // Resuming clears unread, and the subsequent waiting turn stays quiet again.
        feed(
            &app,
            "exec",
            &[
                json!({ "kind": "state", "state": "working", "authoritative": true }),
                json!({ "kind": "state", "state": "waiting", "authoritative": true }),
            ],
        );
        assert!(!snapshot()["exec"].unread);

        // Manual Pending marks are never suppressed or stripped by workflow wait.
        assert!(mark_pending(&app, "exec"));
        assert!(snapshot()["exec"].unread);
        assert!(snapshot()["exec"].unread_manual);

        // Entering workflow wait clears a stale non-manual unread marker on a planner.
        set_unread(&app, "plan", true);
        let mut both = HashMap::new();
        both.insert("exec".to_string(), "parent");
        both.insert("plan".to_string(), "children");
        sync_workflow_waits(&app, both);
        assert!(!snapshot()["plan"].unread, "entering workflow wait clears stale automatic unread");
        assert!(snapshot()["exec"].unread, "manual pending survives workflow sync");

        // Clearing workflow wait restores ordinary unread behavior on the next reply.
        sync_workflow_waits(&app, HashMap::new());
        assert_eq!(snapshot()["exec"].workflow_wait, None);
        set_unread(&app, "exec", false);
        feed(
            &app,
            "exec",
            &[
                json!({ "kind": "state", "state": "working", "authoritative": true }),
                json!({ "kind": "state", "state": "waiting", "authoritative": true }),
            ],
        );
        assert!(snapshot()["exec"].unread, "ended workflow session notifies like a normal session");
    }
}
