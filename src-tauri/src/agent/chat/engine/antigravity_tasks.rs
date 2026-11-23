//! Antigravity's background work and post-prompt wakeups, read from `.system_generated/`.
//!
//! `agy acp` provides no RPC to list, background, or stop background tasks or subagents, and once a
//! `session/prompt` request answers it stops pushing `session/update` notifications on stdout even when
//! background work wakes the agent on its own. Instead, Antigravity writes every step to
//! `.system_generated/logs/transcript_full.jsonl`, streams shell/workflow output to
//! `.system_generated/tasks/task-<N>.log`, and writes completion and restart notices to
//! `.system_generated/messages/*.json`. Scanning these files incrementally populates
//! `extras.backgroundTasks`, keeps `turn.background_tasks` synchronized, and projects post-prompt
//! reactive wakeup turns onto the live timeline.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::{
    adopt_agent_turn, cap_finished_tasks, emit_extras, handle_turn_end, now_ms, read_tail,
    release_if_idle, settle_background_state, upsert_task, BackgroundTask, ChatProcess, ChatRow,
    TaskOutput, WAKE_GRACE,
};
use crate::agent::chat::antigravity_protocol::{
    arg_string, classify_task_exit, first_line_preview, parse_background_launch,
    parse_system_deliveries, parsed_timestamp_ms, shell_task_id, step_id,
    subagent_conversation_id, tool_row, valid_shell_task_id, wake_summary,
};
use crate::host::AppCtx;
use crate::models::SessionKind;

/// How often the poller scans Antigravity's transcript and message directory.
const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Let a freshly started turn or tool call flush its launch record before the first poll.
const FIRST_POLL: Duration = Duration::from_millis(300);
/// Bound retained unmatched agent completions in case an unrelated sender id appears.
const MAX_PENDING_COMPLETIONS: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
enum ScheduleCondition {
    Never,
    Any,
    Sender(String),
}

#[derive(Clone, Debug)]
struct ScheduleSpec {
    is_cron: bool,
    duration_ms: Option<u64>,
    max_iterations: Option<u64>,
    condition: ScheduleCondition,
}

#[derive(Clone, Debug)]
enum ManageCall {
    TaskList,
    TaskKill { task_id: String },
    TaskStatus { task_id: Option<String> },
    SubagentList,
    SubagentKill { ids: Vec<String> },
    SubagentKillAll,
}

#[derive(Clone, Debug)]
struct PendingCall {
    name: String,
    summary: Option<String>,
    command: Option<String>,
    recipient: Option<String>,
    planner_step: Option<u64>,
    schedule: Option<ScheduleSpec>,
    manage: Option<ManageCall>,
}

#[derive(Clone, Debug)]
struct ReactiveToolCall {
    row_id: String,
    name: String,
    args: Value,
    seen_at_ms: u64,
    recovered: bool,
}

#[derive(Clone, Debug)]
pub(super) struct UnreadMessage {
    pub(super) file_name: String,
    pub(super) sender: String,
    pub(super) content: String,
    pub(super) timestamp_raw: Option<String>,
    pub(super) ts_ms: Option<u64>,
}

#[derive(Clone, Debug)]
pub(super) struct DueSchedule {
    pub(super) task_id: String,
    pub(super) wake_key: String,
    pub(super) description: String,
}

#[derive(Clone)]
enum ReactiveAction {
    OpenWake {
        step_index: u64,
        sender: String,
        body: String,
        at: Option<i64>,
    },
    WakeOnly {
        step_index: u64,
        sender: String,
        body: String,
        at: Option<i64>,
    },
    AdoptTurn,
    Upsert(ChatRow),
    SettleRows(Vec<String>),
    EndTurn(Vec<String>),
    RecoverStalledCall {
        row_id: String,
        name: String,
        args: Value,
    },
    RecoverAuth401 {
        step_index: Option<u64>,
        message: String,
    },
}

enum ResolvedReactiveAction {
    OpenWake {
        wake_row: ChatRow,
    },
    WakeOnly {
        wake_row: ChatRow,
    },
    AdoptTurn,
    Upsert(ChatRow),
    SettleRows(Vec<String>),
    EndTurn(Vec<String>),
    RecoverStalledCall {
        row_id: String,
        name: String,
        args: Value,
    },
    RecoverAuth401 {
        step_index: Option<u64>,
        message: String,
    },
}

#[derive(Clone, Default)]
struct Scanner {
    transcript_offset: u64,
    pending_calls: VecDeque<PendingCall>,
    commands: HashMap<String, String>,
    schedules: HashMap<String, ScheduleSpec>,
    cron_triggers: HashMap<String, u64>,
    task_launch_steps: HashMap<String, u64>,
    seen_messages: HashSet<String>,
    seen_system_msgs: HashSet<(String, String)>,
    pub(super) woken_deliveries: HashSet<(String, String)>,
    prewoken_tasks: HashSet<String>,
    completed_outcomes: HashMap<String, (&'static str, Option<u64>)>,
    pending_agent_completions: HashMap<String, Option<u64>>,
    restart_stopped_at: Option<u64>,
    in_prompted_turn: bool,
    in_reactive_turn: bool,
    reactive_calls: VecDeque<ReactiveToolCall>,
    active_reactive_rows: Vec<String>,
}

#[derive(Default)]
pub(super) struct State {
    pub(super) system_dir: Option<PathBuf>,
    pub(super) polling: bool,
    scan_lock: Arc<Mutex<()>>,
    scanner: Scanner,
    aborted_reactive: bool,
}

impl State {
    pub(super) fn reactive_active(&self) -> bool {
        self.scanner.in_reactive_turn || !self.scanner.active_reactive_rows.is_empty()
    }

    pub(super) fn woken_deliveries(&self) -> HashSet<(String, String)> {
        self.scanner.woken_deliveries.clone()
    }

    pub(super) fn take_aborted_reactive_rows(&mut self) -> Vec<String> {
        self.aborted_reactive = true;
        self.scanner.in_reactive_turn = false;
        self.scanner.reactive_calls.clear();
        std::mem::take(&mut self.scanner.active_reactive_rows)
    }
}

#[derive(Clone)]
enum TranscriptEvent {
    UserInput {
        at: u64,
    },
    LaunchTask {
        task_id: String,
        task_type: &'static str,
        description: String,
        command: Option<String>,
        started_at: u64,
        planner_step: Option<u64>,
        schedule: Option<ScheduleSpec>,
    },
    LaunchAgent {
        task_id: String,
        description: String,
        started_at: u64,
        planner_step: Option<u64>,
    },
    ReviveAgent {
        task_id: String,
        started_at: u64,
    },
    FinishTask {
        task_id: String,
        status: &'static str,
        ended_at: u64,
        synthetic: bool,
    },
    ReconcileTaskList {
        active_task_ids: HashSet<String>,
        before_planner_step: Option<u64>,
        at: u64,
    },
    ReconcileSubagents {
        killed_ids: Option<Vec<String>>,
        active_ids: Option<HashSet<String>>,
        before_planner_step: Option<u64>,
        status: &'static str,
        at: u64,
    },
    SystemDelivery {
        sender: String,
        content: String,
        ts: Option<u64>,
    },
}

struct MessageEvent {
    sender: String,
    timestamp_raw: Option<String>,
    content: String,
    ts: Option<u64>,
}

/// Initialize task tracking when an Antigravity ACP conversation opens or loads.
pub(super) fn on_open(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, native_id: &str) {
    let dir = crate::agent::antigravity::system_dir(native_id);
    on_open_dir(app, session_id, proc, dir);
}

fn on_open_dir(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, dir: Option<PathBuf>) {
    {
        let mut state = proc.acp.lock().unwrap();
        state.tasks.system_dir = dir;
        state.tasks.scanner = Scanner::default();
        state.tasks.aborted_reactive = false;
    }
    proc.extras.lock().unwrap().background_tasks.clear();
    proc.turn.lock().unwrap().background_tasks.clear();
    refresh(app, session_id, proc, true);
}

/// Clear active reactive rows when an adopted reactive wakeup is stopped or superseded.
pub(super) fn abort_reactive_turn(proc: &Arc<ChatProcess>) {
    let row_ids = proc.acp.lock().unwrap().tasks.take_aborted_reactive_rows();
    settle_rows(proc, &row_ids);
}

/// Mark streaming or running reactive timeline rows finished without rewriting completed rows.
pub(super) fn settle_rows(proc: &ChatProcess, row_ids: &[String]) {
    if row_ids.is_empty() {
        return;
    }
    let mut timeline = proc.timeline.lock().unwrap();
    for id in row_ids {
        let Some(existing) = timeline.get(id).cloned() else {
            continue;
        };
        let settled = match existing {
            ChatRow::Reasoning { id, text, streaming: true } => Some(ChatRow::Reasoning {
                id,
                text,
                streaming: false,
            }),
            ChatRow::Assistant { id, text, streaming: true, model, at, duration_ms } => Some(ChatRow::Assistant {
                id,
                text,
                streaming: false,
                model,
                at,
                duration_ms,
            }),
            ChatRow::Tool { id, name, input, output, is_error, status: "running", subagent, children } => Some(ChatRow::Tool {
                id,
                name,
                input,
                output,
                is_error,
                status: "completed",
                subagent,
                children,
            }),
            _ => None,
        };
        if let Some(row) = settled {
            timeline.upsert(row);
        }
    }
}

/// Synchronize background tasks right before `handle_turn_end` decides between `Background` and `Waiting`.
pub(super) fn before_turn_end(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    if proc.kind != SessionKind::Antigravity {
        return;
    }
    refresh(app, session_id, proc, false);
    {
        // A cancelled or errored ACP prompt may end before emitting a final DONE step; clear the flag so
        // later background wakeups are still projected.
        let mut state = proc.acp.lock().unwrap();
        state.tasks.scanner.in_prompted_turn = false;
        state.tasks.scanner.pending_calls.clear();
    }
    if pending(proc) || settling_active(proc) {
        ensure_poller(app, session_id, proc);
    }
}

/// Keep one background poller running while a turn, unfinished task, wakeup grace, or reactive turn is active.
pub(super) fn ensure_poller(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    if proc.kind != SessionKind::Antigravity {
        return;
    }
    {
        let mut state = proc.acp.lock().unwrap();
        if state.tasks.system_dir.is_none() || state.tasks.polling {
            return;
        }
        state.tasks.polling = true;
    }
    let (app, session_id, proc) = (app.clone(), session_id.to_string(), proc.clone());
    std::thread::spawn(move || {
        let mut first = true;
        let mut saw_post_turn_pending = false;
        loop {
            std::thread::sleep(if first { FIRST_POLL } else { POLL_INTERVAL });
            first = false;
            if proc.released.load(Ordering::Relaxed) || !proc.alive.load(Ordering::Relaxed) {
                break;
            }
            if !app.chat().owns_process(&session_id, &proc) {
                break;
            }
            let ended_reactive = refresh(&app, &session_id, &proc, false);
            let turn_running = proc.turn.lock().unwrap().running;
            let has_pending = pending(&proc);
            let settling = settling_active(&proc);
            let reactive = reactive_active(&proc);
            if turn_running || ended_reactive {
                saw_post_turn_pending = false;
            } else if has_pending {
                saw_post_turn_pending = true;
            }
            if !turn_running && !has_pending && !settling && !reactive {
                let restart = {
                    proc.acp.lock().unwrap().tasks.polling = false;
                    proc.turn.lock().unwrap().running
                        || pending(&proc)
                        || settling_active(&proc)
                        || reactive_active(&proc)
                };
                if restart {
                    ensure_poller(&app, &session_id, &proc);
                    return;
                }
                if saw_post_turn_pending {
                    settle_background_state(&app, &session_id, &proc);
                }
                release_if_idle(&app, &session_id, &proc);
                return;
            }
        }
        proc.acp.lock().unwrap().tasks.polling = false;
    });
}

/// The command or script a background task runs and the tail of `.system_generated/tasks/<id>.log`.
pub(super) fn task_output(proc: &Arc<ChatProcess>, task_id: &str) -> Result<TaskOutput, String> {
    let description = {
        let extras = proc.extras.lock().unwrap();
        let task = extras
            .background_tasks
            .iter()
            .find(|task| task.task_id == task_id)
            .ok_or("Background task is no longer available")?;
        if !matches!(task.task_type.as_str(), "local_bash" | "local_workflow") || !valid_shell_task_id(task_id) {
            return Err("Task output is available only for background shell and workflow tasks".into());
        }
        task.description.clone()
    };
    let (system_dir, stored_command) = {
        let state = proc.acp.lock().unwrap();
        (state.tasks.system_dir.clone(), state.tasks.scanner.commands.get(task_id).cloned())
    };
    let command = stored_command.or(Some(description));
    let Some(dir) = system_dir else {
        return Ok(TaskOutput { command, output: None, truncated: false });
    };
    let log_path = dir.join("tasks").join(format!("{task_id}.log"));
    let (output, truncated) = match log_path.to_str().map(read_tail) {
        Some(Ok((text, truncated))) => (Some(text), truncated),
        Some(Err(err)) if err.kind() == std::io::ErrorKind::NotFound => (Some(String::new()), false),
        _ => (None, false),
    };
    Ok(TaskOutput { command, output, truncated })
}

/// Incrementally read transcript and message files without holding process locks across disk IO, then
/// apply task and reactive wakeup updates under single locks (`acp` -> `extras` -> `acp` -> `turn` -> `timeline`).
///
/// Returns `true` if a reactive wakeup turn ended during this refresh.
pub(super) fn refresh(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, initial_open: bool) -> bool {
    if proc.kind != SessionKind::Antigravity {
        return false;
    }
    let scan_lock = { proc.acp.lock().unwrap().tasks.scan_lock.clone() };
    let _guard = scan_lock.lock().unwrap();
    let (system_dir, mut scanner, acp_prompt_active) = {
        let mut state = proc.acp.lock().unwrap();
        let Some(dir) = state.tasks.system_dir.clone() else {
            return false;
        };
        state.tasks.aborted_reactive = false;
        let prompt_active = state.prompt_active();
        (dir, state.tasks.scanner.clone(), prompt_active)
    };

    let now = now_ms();
    let mode = proc.mode.lock().unwrap().clone();
    if acp_prompt_active {
        scanner.in_reactive_turn = false;
        scanner.reactive_calls.clear();
        scanner.active_reactive_rows.clear();
    }
    let allow_reactive = !initial_open && !acp_prompt_active;
    let (transcript_events, mut reactive_actions) = scan_transcript(&system_dir, &mut scanner, now, allow_reactive, !initial_open);
    if allow_reactive {
        if let Some(action) = detect_stalled_reactive_call(&mut scanner, now, &mode) {
            reactive_actions.push(action);
        }
    }
    let message_events = scan_messages(&system_dir, &mut scanner);

    let (changed, running_ids, newly_finished_ids, resolved_actions) = {
        let mut extras = proc.extras.lock().unwrap();
        let before = signature(&extras.background_tasks);
        let was_running: HashSet<String> = extras
            .background_tasks
            .iter()
            .filter(|task| task.listed && !task.finished())
            .map(|task| task.task_id.clone())
            .collect();

        for event in transcript_events {
            apply_transcript_event(&mut extras.background_tasks, &mut scanner, event, now);
        }
        for event in message_events {
            apply_message_event(&mut extras.background_tasks, &mut scanner, event, now);
        }
        reconcile_outcomes(&mut extras.background_tasks, &mut scanner, initial_open, now);

        cap_finished_tasks(&mut extras.background_tasks);
        let resolved_actions = resolve_reactive_actions(&extras.background_tasks, reactive_actions);
        let after = signature(&extras.background_tasks);
        let running: HashSet<String> = extras
            .background_tasks
            .iter()
            .filter(|task| task.listed && !task.finished())
            .map(|task| task.task_id.clone())
            .collect();
        let newly_finished: Vec<String> = was_running.difference(&running).cloned().collect();
        (before != after, running, newly_finished, resolved_actions)
    };

    let resolved_actions = {
        let mut state = proc.acp.lock().unwrap();
        let prompt_superseded = state.prompt_active();
        let drop_all = initial_open || state.tasks.aborted_reactive;
        if initial_open {
            scanner.in_prompted_turn = false;
        }
        if drop_all || prompt_superseded {
            scanner.in_reactive_turn = false;
            scanner.reactive_calls.clear();
            scanner.active_reactive_rows.clear();
        }
        state.tasks.scanner = scanner;
        if drop_all {
            Vec::new()
        } else if prompt_superseded {
            resolved_actions
                .into_iter()
                .filter(|action| matches!(action, ResolvedReactiveAction::WakeOnly { .. }))
                .collect()
        } else {
            resolved_actions
        }
    };

    let has_reactive_boundary = resolved_actions.iter().any(|action| {
        matches!(
            action,
            ResolvedReactiveAction::OpenWake { .. }
                | ResolvedReactiveAction::AdoptTurn
                | ResolvedReactiveAction::EndTurn(_)
        )
    });
    {
        let now_inst = Instant::now();
        let mut turn = proc.turn.lock().unwrap();
        turn.background_tasks = running_ids;
        turn.settling.retain(|_, deadline| *deadline > now_inst);
        if !initial_open {
            if has_reactive_boundary || turn.running {
                turn.settling.clear();
            } else if !newly_finished_ids.is_empty() {
                let deadline = now_inst + WAKE_GRACE;
                for id in newly_finished_ids {
                    turn.settling.insert(id, deadline);
                }
            }
        }
    }

    if changed {
        emit_extras(app, session_id, proc);
    }

    let mut ended_reactive = false;
    for action in resolved_actions {
        match action {
            ResolvedReactiveAction::OpenWake { wake_row } => {
                proc.timeline.lock().unwrap().upsert(wake_row);
                adopt_agent_turn(app, session_id, proc);
            }
            ResolvedReactiveAction::WakeOnly { wake_row } => {
                proc.timeline.lock().unwrap().upsert(wake_row);
            }
            ResolvedReactiveAction::AdoptTurn => {
                adopt_agent_turn(app, session_id, proc);
            }
            ResolvedReactiveAction::Upsert(mut row) => {
                let model = proc
                    .model
                    .lock()
                    .unwrap()
                    .as_deref()
                    .map(|m| crate::agent::chat::acp_protocol::reply_model(proc.kind, m));
                let row_id = row.id().to_string();
                let mut timeline = proc.timeline.lock().unwrap();
                let first_at = match timeline.get(&row_id) {
                    Some(ChatRow::Assistant { at, .. }) => *at,
                    _ => None,
                };
                if let ChatRow::Assistant { at, model: reply_model, .. } = &mut row {
                    *at = first_at.or(*at).or(Some(now as i64));
                    *reply_model = model;
                }
                timeline.upsert(row);
            }
            ResolvedReactiveAction::SettleRows(ids) => {
                settle_rows(proc, &ids);
            }
            ResolvedReactiveAction::EndTurn(ids) => {
                settle_rows(proc, &ids);
                handle_turn_end(app, session_id, proc, "success", None);
                ended_reactive = true;
                if pending(proc) {
                    ensure_poller(app, session_id, proc);
                }
            }
            ResolvedReactiveAction::RecoverStalledCall { row_id, name, args } => {
                super::acp::recover_stalled_tool(app, session_id, proc, &row_id, &name, &args);
            }
            ResolvedReactiveAction::RecoverAuth401 { step_index, message } => {
                let ids = proc.acp.lock().unwrap().tasks.take_aborted_reactive_rows();
                settle_rows(proc, &ids);
                super::acp::handle_reactive_401(app, session_id, proc, step_index, &message);
                ended_reactive = true;
            }
        }
    }
    ended_reactive
}

fn resolve_reactive_actions(
    tasks: &[BackgroundTask],
    actions: Vec<ReactiveAction>,
) -> Vec<ResolvedReactiveAction> {
    let mut out = Vec::with_capacity(actions.len());
    for action in actions {
        match action {
            ReactiveAction::OpenWake { step_index, sender, body, at } => {
                let lookup_id = shell_task_id(&sender).unwrap_or_else(|| sender.clone());
                let desc = tasks
                    .iter()
                    .find(|task| task.task_id == lookup_id)
                    .map(|task| task.description.as_str());
                if let Some(summary) = wake_summary(&sender, &body, desc) {
                    out.push(ResolvedReactiveAction::OpenWake {
                        wake_row: ChatRow::Wake {
                            id: step_id(step_index),
                            summary,
                            at,
                        },
                    });
                } else {
                    out.push(ResolvedReactiveAction::AdoptTurn);
                }
            }
            ReactiveAction::WakeOnly { step_index, sender, body, at } => {
                let lookup_id = shell_task_id(&sender).unwrap_or_else(|| sender.clone());
                let desc = tasks
                    .iter()
                    .find(|task| task.task_id == lookup_id)
                    .map(|task| task.description.as_str());
                if let Some(summary) = wake_summary(&sender, &body, desc) {
                    out.push(ResolvedReactiveAction::WakeOnly {
                        wake_row: ChatRow::Wake {
                            id: step_id(step_index),
                            summary,
                            at,
                        },
                    });
                }
            }
            ReactiveAction::AdoptTurn => out.push(ResolvedReactiveAction::AdoptTurn),
            ReactiveAction::Upsert(row) => out.push(ResolvedReactiveAction::Upsert(row)),
            ReactiveAction::SettleRows(ids) => out.push(ResolvedReactiveAction::SettleRows(ids)),
            ReactiveAction::EndTurn(ids) => out.push(ResolvedReactiveAction::EndTurn(ids)),
            ReactiveAction::RecoverStalledCall { row_id, name, args } => {
                out.push(ResolvedReactiveAction::RecoverStalledCall { row_id, name, args });
            }
            ReactiveAction::RecoverAuth401 { step_index, message } => {
                out.push(ResolvedReactiveAction::RecoverAuth401 { step_index, message });
            }
        }
    }
    out
}

fn arg_u64(args: &Value, key: &str) -> Option<u64> {
    if let Some(val) = args.get(key) {
        if let Some(n) = val.as_u64() {
            return Some(n);
        }
        if let Some(f) = val.as_f64() {
            if f.is_finite() && f >= 0.0 {
                return Some(f.ceil() as u64);
            }
        }
    }
    let s = arg_string(args, key)?;
    if let Ok(n) = s.parse::<u64>() {
        return Some(n);
    }
    let f = s.parse::<f64>().ok()?;
    (f.is_finite() && f >= 0.0).then_some(f.ceil() as u64)
}

fn parse_timer_duration_ms(desc: &str) -> Option<u64> {
    let rest = desc.trim().strip_prefix("Timer:")?.trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || !rest[digits.len()..].starts_with('s') {
        return None;
    }
    digits.parse::<u64>().ok().map(|s| s.saturating_mul(1_000))
}

/// Minimum idle elapsed time before a headless reactive tool call without follow-up steps is
/// treated as blocked on interactive user input or confirmation.
pub(super) fn headless_stall_limit_ms(name: &str, args: &Value, mode: &str) -> Option<u64> {
    match name {
        "ask_question" => Some(3_000),
        "run_command" => {
            if mode == "bypassPermissions" {
                return None;
            }
            let wait_ms = arg_u64(args, "WaitMsBeforeAsync").unwrap_or(0);
            Some(wait_ms.saturating_add(4_000).max(5_000))
        }
        "write_to_file" | "replace_file_content" | "multi_replace_file_content" | "edit_file" | "write_file" => {
            if mode == "bypassPermissions" {
                None
            } else if matches!(mode, "acceptEdits" | "accept-edits") {
                Some(6_000)
            } else {
                Some(4_000)
            }
        }
        _ => None,
    }
}

fn detect_stalled_reactive_call(scanner: &mut Scanner, now: u64, mode: &str) -> Option<ReactiveAction> {
    if !scanner.in_reactive_turn {
        return None;
    }
    let front = scanner.reactive_calls.front_mut()?;
    if front.recovered {
        return None;
    }
    let limit = headless_stall_limit_ms(&front.name, &front.args, mode)?;
    if now.saturating_sub(front.seen_at_ms) < limit {
        return None;
    }
    front.recovered = true;
    Some(ReactiveAction::RecoverStalledCall {
        row_id: front.row_id.clone(),
        name: front.name.clone(),
        args: front.args.clone(),
    })
}

fn normalize_wake_ts(raw: &str) -> String {
    raw.trim()
        .trim_end_matches(['Z', 'z'])
        .split('.')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Find the oldest unread delivery in `<system_dir>/messages/` (including staged `undelivered/`
/// files) not already surfaced in this process.
pub(super) fn next_unread_message(
    system_dir: &Path,
    surfaced: &HashSet<String>,
    woken_deliveries: &HashSet<(String, String)>,
) -> Option<UnreadMessage> {
    let dir = system_dir.join("messages");
    let read_ids = load_read_message_ids(&dir.join("read.json"));
    let mut seen_stems = HashSet::new();
    let mut unread = Vec::new();
    for (scan_dir, check_read) in [(dir.join("undelivered"), false), (dir.clone(), true)] {
        let Ok(entries) = std::fs::read_dir(&scan_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = match entry.file_name().to_str() {
                Some(s) => s.to_string(),
                None => continue,
            };
            if name == "read.json" || !name.ends_with(".json") {
                continue;
            }
            let stem = name.strip_suffix(".json").unwrap_or(&name).to_string();
            if surfaced.contains(&stem) || surfaced.contains(&name) || !seen_stems.insert(stem.clone()) {
                continue;
            }
            if check_read && (read_ids.contains(&name) || read_ids.contains(&stem)) {
                continue;
            }
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Ok(raw) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            let Some(sender) = value.get("sender").and_then(Value::as_str) else {
                continue;
            };
            if matches!(sender, "user" | "system") {
                continue;
            }
            let timestamp_raw = value.get("timestamp").and_then(Value::as_str).map(str::to_string);
            if timestamp_raw
                .as_deref()
                .map(normalize_wake_ts)
                .is_some_and(|norm| woken_deliveries.contains(&(sender.to_string(), norm)))
            {
                continue;
            }
            let ts_ms = parsed_timestamp_ms(timestamp_raw.as_deref());
            let content = value.get("content").and_then(Value::as_str).unwrap_or_default().to_string();
            unread.push(UnreadMessage {
                file_name: stem,
                sender: sender.to_string(),
                content,
                timestamp_raw,
                ts_ms,
            });
        }
    }
    unread.sort_by(|a, b| {
        a.ts_ms
            .cmp(&b.ts_ms)
            .then_with(|| a.timestamp_raw.cmp(&b.timestamp_raw))
            .then_with(|| a.file_name.cmp(&b.file_name))
    });
    unread.into_iter().next()
}

fn load_read_message_ids(path: &Path) -> HashSet<String> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return HashSet::new();
    };
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&raw) else {
        return HashSet::new();
    };
    map.into_iter()
        .filter_map(|(k, v)| (v == Value::Bool(true)).then_some(k))
        .collect()
}

/// Whether Antigravity has either active background work or an unsurfaced unread delivery.
pub(super) fn has_pending_or_unread(
    proc: &Arc<ChatProcess>,
    surfaced: &HashSet<String>,
    woken_deliveries: &HashSet<(String, String)>,
) -> bool {
    if pending(proc) || settling_active(proc) {
        return true;
    }
    let system_dir = { proc.acp.lock().unwrap().tasks.system_dir.clone() };
    system_dir
        .as_deref()
        .is_some_and(|dir| next_unread_message(dir, surfaced, woken_deliveries).is_some())
}

/// Return the earliest expired one-shot `schedule` timer that has not yet woken a parked `Stop` hook.
///
/// `agy` flushes in-process `schedule` messages only after returning from a parked `Stop` hook, so
/// waiting solely on `messages/*.json` deadlocks when the timer turn parks before expiry. Adding
/// `1_250ms` absorbs whole-second `created_at` truncation (`0..999ms`) plus internal timer slack.
pub(super) fn next_due_schedule(
    proc: &Arc<ChatProcess>,
    now: u64,
    woken_schedules: &HashSet<String>,
) -> Option<DueSchedule> {
    let schedules: HashMap<String, ScheduleSpec> = {
        proc.acp.lock().unwrap().tasks.scanner.schedules.clone()
    };
    let tasks: Vec<(String, String, Option<u64>)> = {
        let extras = proc.extras.lock().unwrap();
        extras
            .background_tasks
            .iter()
            .filter(|task| !task.finished())
            .map(|task| (task.task_id.clone(), task.description.clone(), task.started_at))
            .collect()
    };
    tasks
        .into_iter()
        .filter_map(|(task_id, description, started_at)| {
            let spec = schedules.get(&task_id)?;
            if spec.is_cron {
                return None;
            }
            let duration_ms = spec.duration_ms?;
            let started = started_at?;
            let wake_key = format!("{task_id}:0");
            if woken_schedules.contains(&wake_key) {
                return None;
            }
            let due_at = started.saturating_add(duration_ms).saturating_add(1_250);
            (now >= due_at).then_some((due_at, DueSchedule { task_id, wake_key, description }))
        })
        .min_by_key(|(due_at, _)| *due_at)
        .map(|(_, due)| due)
}

/// Emit a timeline `Wake` separator when a parked ACP `Stop` hook wakes for a due `schedule` timer,
/// and mark the task so the subsequent transcript `SYSTEM_MESSAGE` does not duplicate that `Wake` row.
pub(super) fn emit_schedule_wake(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    due: &DueSchedule,
) {
    let scan_lock = { proc.acp.lock().unwrap().tasks.scan_lock.clone() };
    {
        let _guard = scan_lock.lock().unwrap();
        proc.acp
            .lock()
            .unwrap()
            .tasks
            .scanner
            .prewoken_tasks
            .insert(due.task_id.clone());
    }
    proc.timeline.lock().unwrap().upsert(ChatRow::Wake {
        id: format!("agy-wake-sched-{}", due.task_id),
        summary: format!("Background task \"{}\" completed", due.description),
        at: i64::try_from(now_ms()).ok(),
    });
    adopt_agent_turn(app, session_id, proc);
}

/// Emit a timeline `Wake` separator when a parked ACP `Stop` hook wakes for an unread delivery,
/// and mark the delivery so later transcript scans do not duplicate that `Wake` row.
pub(super) fn emit_unread_wake(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    msg: &UnreadMessage,
) {
    let desc = {
        let lookup_id = shell_task_id(&msg.sender).unwrap_or_else(|| msg.sender.clone());
        let extras = proc.extras.lock().unwrap();
        extras
            .background_tasks
            .iter()
            .find(|task| task.task_id == lookup_id)
            .map(|task| task.description.clone())
    };
    if let Some(summary) = wake_summary(&msg.sender, &msg.content, desc.as_deref()) {
        let at = msg.ts_ms.and_then(|ms| i64::try_from(ms).ok());
        proc.timeline.lock().unwrap().upsert(ChatRow::Wake {
            id: format!("agy-wake-{}", msg.file_name.trim_end_matches(".json")),
            summary,
            at,
        });
    }
    if let Some(raw_ts) = &msg.timestamp_raw {
        let scan_lock = { proc.acp.lock().unwrap().tasks.scan_lock.clone() };
        let _guard = scan_lock.lock().unwrap();
        proc.acp
            .lock()
            .unwrap()
            .tasks
            .scanner
            .woken_deliveries
            .insert((msg.sender.clone(), normalize_wake_ts(raw_ts)));
    }
    adopt_agent_turn(app, session_id, proc);
}

pub(super) fn pending(proc: &Arc<ChatProcess>) -> bool {
    proc.extras
        .lock()
        .unwrap()
        .background_tasks
        .iter()
        .any(|task| task.listed && !task.finished())
}

pub(super) fn has_background_activity(proc: &Arc<ChatProcess>) -> bool {
    pending(proc) || settling_active(proc) || reactive_active(proc)
}

fn settling_active(proc: &Arc<ChatProcess>) -> bool {
    let now = Instant::now();
    proc.turn
        .lock()
        .unwrap()
        .settling
        .values()
        .any(|deadline| *deadline > now)
}

fn reactive_active(proc: &Arc<ChatProcess>) -> bool {
    proc.acp.lock().unwrap().tasks.reactive_active()
}

fn scan_transcript(
    system_dir: &Path,
    scanner: &mut Scanner,
    now: u64,
    allow_reactive: bool,
    emit_live_wakes: bool,
) -> (Vec<TranscriptEvent>, Vec<ReactiveAction>) {
    let path = system_dir.join("logs/transcript_full.jsonl");
    let Ok(mut file) = File::open(&path) else {
        return (Vec::new(), Vec::new());
    };
    let Ok(len) = file.metadata().map(|meta| meta.len()) else {
        return (Vec::new(), Vec::new());
    };
    if len < scanner.transcript_offset {
        scanner.transcript_offset = 0;
        scanner.pending_calls.clear();
        scanner.schedules.clear();
        scanner.cron_triggers.clear();
        scanner.task_launch_steps.clear();
        scanner.prewoken_tasks.clear();
        scanner.in_prompted_turn = false;
        scanner.in_reactive_turn = false;
        scanner.reactive_calls.clear();
        scanner.active_reactive_rows.clear();
    }
    if len == scanner.transcript_offset {
        return (Vec::new(), Vec::new());
    }
    if file.seek(SeekFrom::Start(scanner.transcript_offset)).is_err() {
        return (Vec::new(), Vec::new());
    }
    let mut buffer = Vec::new();
    if file.read_to_end(&mut buffer).is_err() {
        return (Vec::new(), Vec::new());
    }
    // Consume only complete newline-terminated JSONL lines so partial writes wait for the next poll.
    let Some(last_newline) = buffer.iter().rposition(|byte| *byte == b'\n') else {
        return (Vec::new(), Vec::new());
    };
    let complete = &buffer[..=last_newline];
    scanner.transcript_offset += complete.len() as u64;

    let mut events = Vec::new();
    let mut reactive = Vec::new();
    for line in complete.split(|byte| *byte == b'\n') {
        let trimmed = std::str::from_utf8(line).unwrap_or_default().trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(step) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        let step_index = step.get("step_index").and_then(Value::as_u64);
        let step_ts_ms = parsed_timestamp_ms(step.get("created_at").and_then(Value::as_str));
        let step_ts = step_ts_ms.unwrap_or(now);
        let step_at = step_ts_ms.and_then(|ms| i64::try_from(ms).ok());
        let done = step.get("status").and_then(Value::as_str) == Some("DONE");
        match step.get("type").and_then(Value::as_str) {
            Some("USER_INPUT") => {
                events.push(TranscriptEvent::UserInput { at: step_ts });
                scanner.pending_calls.clear();
                scanner.in_prompted_turn = true;
                if scanner.in_reactive_turn {
                    scanner.in_reactive_turn = false;
                    scanner.reactive_calls.clear();
                    let to_settle = std::mem::take(&mut scanner.active_reactive_rows);
                    if !to_settle.is_empty() {
                        reactive.push(ReactiveAction::SettleRows(to_settle));
                    }
                }
            }
            Some("PLANNER_RESPONSE") => {
                let tool_calls = step.get("tool_calls").and_then(Value::as_array);
                let tool_calls_empty = tool_calls.is_none_or(Vec::is_empty);
                for call in tool_calls.into_iter().flatten() {
                    let Some(name) = call.get("name").and_then(Value::as_str) else {
                        continue;
                    };
                    let args = call.get("args").unwrap_or(&Value::Null);
                    scanner.pending_calls.push_back(parse_pending_call(name, args, step_index));
                }
                if scanner.in_prompted_turn {
                    if done && tool_calls_empty {
                        scanner.in_prompted_turn = false;
                    }
                } else if allow_reactive {
                    if !scanner.in_reactive_turn {
                        scanner.in_reactive_turn = true;
                        reactive.push(ReactiveAction::AdoptTurn);
                    }
                    if let Some(idx) = step_index {
                        let base_id = step_id(idx);
                        if let Some(thinking) = step.get("thinking").and_then(Value::as_str).filter(|v| !v.is_empty()) {
                            let row_id = format!("{base_id}-thinking");
                            if !done && !scanner.active_reactive_rows.contains(&row_id) {
                                scanner.active_reactive_rows.push(row_id.clone());
                            }
                            reactive.push(ReactiveAction::Upsert(ChatRow::Reasoning {
                                id: row_id,
                                text: thinking.into(),
                                streaming: !done,
                            }));
                        }
                        if let Some(text) = step.get("content").and_then(Value::as_str).filter(|v| !v.is_empty()) {
                            if !done && !scanner.active_reactive_rows.contains(&base_id) {
                                scanner.active_reactive_rows.push(base_id.clone());
                            }
                            reactive.push(ReactiveAction::Upsert(ChatRow::Assistant {
                                id: base_id.clone(),
                                text: text.into(),
                                streaming: !done,
                                model: None,
                                at: step_at,
                                duration_ms: None,
                            }));
                        }
                        for (ordinal, call) in tool_calls.into_iter().flatten().enumerate() {
                            let Some(name) = call.get("name").and_then(Value::as_str) else {
                                continue;
                            };
                            let args = call.get("args").cloned().unwrap_or(Value::Null);
                            let call_id = format!("{base_id}-call-{ordinal}");
                            scanner.active_reactive_rows.push(call_id.clone());
                            scanner.reactive_calls.push_back(ReactiveToolCall {
                                row_id: call_id.clone(),
                                name: name.to_string(),
                                args: args.clone(),
                                seen_at_ms: now,
                                recovered: false,
                            });
                            reactive.push(ReactiveAction::Upsert(tool_row(
                                call_id, name, &args, None, false, false,
                            )));
                        }
                    }
                    if done && tool_calls_empty {
                        scanner.in_reactive_turn = false;
                        scanner.reactive_calls.clear();
                        let to_settle = std::mem::take(&mut scanner.active_reactive_rows);
                        reactive.push(ReactiveAction::EndTurn(to_settle));
                    }
                }
            }
            Some("GENERIC" | "ERROR_MESSAGE" | "LIST_DIRECTORY" | "VIEW_FILE" | "CODE_ACTION" | "ASK_QUESTION") => {
                let call = scanner.pending_calls.pop_front();
                let is_error = step.get("type").and_then(Value::as_str) == Some("ERROR_MESSAGE");
                let content = step.get("content").and_then(Value::as_str).unwrap_or_default();

                if scanner.in_reactive_turn {
                    if let Some(rcall) = scanner.reactive_calls.pop_front() {
                        let finished = is_error || done;
                        if finished {
                            scanner.active_reactive_rows.retain(|id| id != &rcall.row_id);
                        }
                        reactive.push(ReactiveAction::Upsert(tool_row(
                            rcall.row_id,
                            &rcall.name,
                            &rcall.args,
                            Some(content.to_string()),
                            is_error,
                            finished,
                        )));
                    } else if is_error && !content.is_empty() {
                        if crate::agent::chat::antigravity_protocol::is_oauth_401_error(content) {
                            scanner.in_reactive_turn = false;
                            scanner.reactive_calls.clear();
                            reactive.push(ReactiveAction::RecoverAuth401 {
                                step_index,
                                message: content.to_string(),
                            });
                        } else if let Some(idx) = step_index {
                            reactive.push(ReactiveAction::Upsert(ChatRow::Error {
                                id: step_id(idx),
                                message: content.to_string(),
                            }));
                        }
                    }
                }

                if is_error {
                    continue;
                }
                if let Some((task_id, task_desc)) = parse_background_launch(content) {
                    let task_type = if call.as_ref().is_some_and(|c| c.name == "run_workflow") {
                        "local_workflow"
                    } else {
                        "local_bash"
                    };
                    let command = call
                        .as_ref()
                        .and_then(|c| c.command.clone())
                        .or_else(|| (!task_desc.is_empty()).then(|| task_desc.clone()));
                    let description = call
                        .as_ref()
                        .and_then(|c| c.summary.clone())
                        .or_else(|| first_line_preview(&task_desc, 160))
                        .unwrap_or_else(|| task_id.clone());
                    let planner_step = call.as_ref().and_then(|c| c.planner_step);
                    let mut schedule = call.and_then(|c| c.schedule);
                    if let Some(ms) = parse_timer_duration_ms(&task_desc) {
                        match schedule.as_mut() {
                            Some(spec) if !spec.is_cron && spec.duration_ms.is_none() => {
                                spec.duration_ms = Some(ms);
                            }
                            None => {
                                schedule = Some(ScheduleSpec {
                                    is_cron: false,
                                    duration_ms: Some(ms),
                                    max_iterations: None,
                                    condition: ScheduleCondition::Never,
                                });
                            }
                            _ => {}
                        }
                    }
                    events.push(TranscriptEvent::LaunchTask {
                        task_id,
                        task_type,
                        description,
                        command,
                        started_at: step_ts,
                        planner_step,
                        schedule,
                    });
                } else if call.as_ref().is_some_and(|c| c.name == "invoke_subagent") {
                    if let Some(child_id) = subagent_conversation_id(content) {
                        let planner_step = call.as_ref().and_then(|c| c.planner_step);
                        let description = call
                            .and_then(|c| c.summary)
                            .unwrap_or_else(|| "Subagent".into());
                        events.push(TranscriptEvent::LaunchAgent {
                            task_id: child_id,
                            description,
                            started_at: step_ts,
                            planner_step,
                        });
                    }
                } else if call.as_ref().is_some_and(|c| c.name == "send_message") {
                    if let Some(recipient) = call.and_then(|c| c.recipient) {
                        events.push(TranscriptEvent::ReviveAgent {
                            task_id: recipient,
                            started_at: step_ts,
                        });
                    }
                } else if let Some(PendingCall { planner_step, manage: Some(manage), .. }) = call {
                    match manage {
                        ManageCall::TaskList => {
                            events.push(TranscriptEvent::ReconcileTaskList {
                                active_task_ids: extract_shell_task_ids(content),
                                before_planner_step: planner_step,
                                at: step_ts,
                            });
                        }
                        ManageCall::TaskKill { task_id } => {
                            events.push(TranscriptEvent::FinishTask {
                                task_id,
                                status: "canceled",
                                ended_at: step_ts,
                                synthetic: true,
                            });
                        }
                        ManageCall::TaskStatus { task_id } => {
                            let (parsed_id, status, running) = parse_manage_task_status(content);
                            if !running {
                                if let (Some(resolved_id), Some(st)) = (task_id.or(parsed_id), status) {
                                    events.push(TranscriptEvent::FinishTask {
                                        task_id: resolved_id,
                                        status: st,
                                        ended_at: step_ts,
                                        synthetic: true,
                                    });
                                }
                            }
                        }
                        ManageCall::SubagentKill { ids } => {
                            events.push(TranscriptEvent::ReconcileSubagents {
                                killed_ids: Some(ids),
                                active_ids: None,
                                before_planner_step: planner_step,
                                status: "stopped",
                                at: step_ts,
                            });
                        }
                        ManageCall::SubagentKillAll => {
                            events.push(TranscriptEvent::ReconcileSubagents {
                                killed_ids: None,
                                active_ids: None,
                                before_planner_step: planner_step,
                                status: "stopped",
                                at: step_ts,
                            });
                        }
                        ManageCall::SubagentList => {
                            events.push(TranscriptEvent::ReconcileSubagents {
                                killed_ids: None,
                                active_ids: Some(extract_native_ids(content)),
                                before_planner_step: planner_step,
                                status: "ended",
                                at: step_ts,
                            });
                        }
                    }
                }
            }
            Some("SYSTEM_MESSAGE") => {
                let content = step.get("content").and_then(Value::as_str).unwrap_or_default();
                let deliveries = parse_system_messages(content);
                if (allow_reactive || emit_live_wakes) && !scanner.in_prompted_turn && !scanner.in_reactive_turn {
                    if let Some((timestamp_raw, sender, body, delivery_ts)) =
                        deliveries.iter().find(|(_, s, _, _)| s != "system")
                    {
                        let norm_ts = timestamp_raw.as_deref().map(normalize_wake_ts);
                        let prewoken = shell_task_id(sender)
                            .is_some_and(|tid| scanner.prewoken_tasks.remove(&tid));
                        let already_woken = prewoken
                            || norm_ts
                                .as_ref()
                                .is_some_and(|raw| scanner.woken_deliveries.contains(&(sender.clone(), raw.clone())));
                        if let Some(idx) = step_index {
                            let at = step_at.or_else(|| delivery_ts.and_then(|ms| i64::try_from(ms).ok()));
                            if allow_reactive {
                                scanner.in_reactive_turn = true;
                                if already_woken {
                                    reactive.push(ReactiveAction::AdoptTurn);
                                } else {
                                    if let Some(raw) = norm_ts {
                                        scanner.woken_deliveries.insert((sender.clone(), raw));
                                    }
                                    reactive.push(ReactiveAction::OpenWake {
                                        step_index: idx,
                                        sender: sender.clone(),
                                        body: body.clone(),
                                        at,
                                    });
                                }
                            } else {
                                // `agy` injected a timer/task wakeup directly inside an active `session/prompt`;
                                // resume prompted-turn tracking and emit `WakeOnly` unless already prewoken.
                                scanner.in_prompted_turn = true;
                                if !already_woken {
                                    if let Some(raw) = norm_ts {
                                        scanner.woken_deliveries.insert((sender.clone(), raw));
                                    }
                                    reactive.push(ReactiveAction::WakeOnly {
                                        step_index: idx,
                                        sender: sender.clone(),
                                        body: body.clone(),
                                        at,
                                    });
                                }
                            }
                        }
                    }
                }
                for (timestamp_raw, sender, body, ts) in deliveries {
                    if let Some(tid) = shell_task_id(&sender) {
                        scanner.prewoken_tasks.remove(&tid);
                    }
                    if let Some(raw) = timestamp_raw {
                        scanner.woken_deliveries.insert((sender.clone(), normalize_wake_ts(&raw)));
                        scanner.seen_system_msgs.insert((sender.clone(), raw));
                    }
                    events.push(TranscriptEvent::SystemDelivery {
                        sender,
                        content: body,
                        ts,
                    });
                }
            }
            _ => {}
        }
    }
    (events, reactive)
}

fn scan_messages(system_dir: &Path, scanner: &mut Scanner) -> Vec<MessageEvent> {
    let dir = system_dir.join("messages");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut filenames: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            if name == "read.json" || !name.ends_with(".json") || scanner.seen_messages.contains(&name) {
                return None;
            }
            let path = entry.path();
            path.is_file().then_some((name, path))
        })
        .collect();
    filenames.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = Vec::new();
    for (name, path) in filenames {
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            // Leave unreadable or half-flushed JSON files unmarked so the next poll retries them.
            continue;
        };
        scanner.seen_messages.insert(name);
        let Some(sender) = value.get("sender").and_then(Value::as_str) else {
            continue;
        };
        let timestamp_raw = value.get("timestamp").and_then(Value::as_str).map(str::to_string);
        if sender != "user" {
            if let Some(raw_ts) = timestamp_raw.as_ref() {
                if scanner.seen_system_msgs.contains(&(sender.to_string(), raw_ts.clone())) {
                    // Skip messages already observed in chronological order inside transcript SYSTEM_MESSAGE.
                    continue;
                }
            }
        }
        let content = value.get("content").and_then(Value::as_str).unwrap_or_default().to_string();
        let ts = parsed_timestamp_ms(timestamp_raw.as_deref());
        out.push(MessageEvent {
            sender: sender.to_string(),
            timestamp_raw,
            content,
            ts,
        });
    }
    // Sort by parsed timestamp ascending so completions play chronologically even across unordered UUID filenames.
    out.sort_by(|a, b| a.ts.cmp(&b.ts).then_with(|| a.timestamp_raw.cmp(&b.timestamp_raw)));
    out
}

fn apply_transcript_event(
    tasks: &mut Vec<BackgroundTask>,
    scanner: &mut Scanner,
    event: TranscriptEvent,
    now: u64,
) {
    match event {
        TranscriptEvent::UserInput { at } => {
            cancel_matching_schedules(tasks, scanner, "user", Some(at), now, true);
        }
        TranscriptEvent::LaunchTask {
            task_id,
            task_type,
            description,
            command,
            started_at,
            planner_step,
            schedule,
        } => {
            if let Some(cmd) = command {
                scanner.commands.insert(task_id.clone(), cmd);
            }
            if let Some(step) = planner_step {
                scanner.task_launch_steps.insert(task_id.clone(), step);
            }
            scanner.cron_triggers.remove(&task_id);
            if let Some(spec) = schedule {
                scanner.schedules.insert(task_id.clone(), spec);
            } else {
                scanner.schedules.remove(&task_id);
            }
            let task = upsert_task(tasks, &task_id);
            task.task_type = task_type.into();
            task.description = description;
            task.listed = true;
            task.in_inventory = false;
            if task.started_at.is_none() || !task.finished() {
                task.started_at = Some(started_at);
            }
            if !task.finished() {
                task.status = "running".into();
            }
        }
        TranscriptEvent::LaunchAgent {
            task_id,
            description,
            started_at,
            planner_step,
        } => {
            if let Some(step) = planner_step {
                scanner.task_launch_steps.insert(task_id.clone(), step);
            }
            let task = upsert_task(tasks, &task_id);
            task.task_type = "local_agent".into();
            if task.description.is_empty() {
                task.description = description;
            }
            task.listed = true;
            task.in_inventory = false;
            task.started_at = Some(started_at);
            if task.finished() {
                task.set_status("running", started_at);
            }
        }
        TranscriptEvent::ReviveAgent { task_id, started_at } => {
            if let Some(task) = tasks
                .iter_mut()
                .find(|task| task.task_id == task_id && task.task_type == "local_agent")
            {
                scanner.pending_agent_completions.remove(&task_id);
                task.set_status("running", started_at);
                task.in_inventory = false;
            }
        }
        TranscriptEvent::FinishTask {
            task_id,
            status,
            ended_at,
            synthetic,
        } => {
            if let Some(task) = tasks.iter_mut().find(|task| task.task_id == task_id) {
                if !task.finished() {
                    task.set_status(status, ended_at);
                    task.synthetic_end = synthetic;
                }
            }
        }
        TranscriptEvent::ReconcileTaskList {
            active_task_ids,
            before_planner_step,
            at,
        } => {
            for task in tasks.iter_mut() {
                if !matches!(task.task_type.as_str(), "local_bash" | "local_workflow") || task.finished() {
                    continue;
                }
                if launched_before_step(&scanner.task_launch_steps, &task.task_id, before_planner_step)
                    && !active_task_ids.contains(&task.task_id)
                {
                    let status = if scanner.schedules.contains_key(&task.task_id) {
                        "canceled"
                    } else {
                        "ended"
                    };
                    task.set_status(status, at);
                    task.synthetic_end = true;
                }
            }
        }
        TranscriptEvent::ReconcileSubagents {
            killed_ids,
            active_ids,
            before_planner_step,
            status,
            at,
        } => {
            if let Some(ids) = killed_ids {
                for task in tasks.iter_mut() {
                    if task.task_type == "local_agent"
                        && !task.finished()
                        && ids.iter().any(|id| id == &task.task_id)
                    {
                        task.set_status(status, at);
                        task.synthetic_end = true;
                    }
                }
            } else {
                let active = active_ids.unwrap_or_default();
                for task in tasks.iter_mut() {
                    if task.task_type != "local_agent" || task.finished() {
                        continue;
                    }
                    if launched_before_step(&scanner.task_launch_steps, &task.task_id, before_planner_step)
                        && !active.contains(&task.task_id)
                    {
                        task.set_status(status, at);
                        task.synthetic_end = true;
                    }
                }
            }
        }
        TranscriptEvent::SystemDelivery { sender, content, ts } => {
            apply_delivery(tasks, scanner, &sender, &content, ts, now, true);
        }
    }
}

fn launched_before_step(
    launch_steps: &HashMap<String, u64>,
    task_id: &str,
    before_planner_step: Option<u64>,
) -> bool {
    match (before_planner_step, launch_steps.get(task_id)) {
        (Some(limit), Some(&step)) => step < limit,
        _ => true,
    }
}

fn apply_message_event(
    tasks: &mut Vec<BackgroundTask>,
    scanner: &mut Scanner,
    event: MessageEvent,
    now: u64,
) {
    apply_delivery(tasks, scanner, &event.sender, &event.content, event.ts, now, false);
}

fn apply_delivery(
    tasks: &mut Vec<BackgroundTask>,
    scanner: &mut Scanner,
    sender: &str,
    content: &str,
    ts: Option<u64>,
    now: u64,
    from_transcript: bool,
) {
    if sender == "system" {
        if content.contains("All your subagents and background tasks have been stopped") {
            let stop_at = ts.unwrap_or(now);
            scanner.restart_stopped_at = Some(scanner.restart_stopped_at.map_or(stop_at, |prev| prev.max(stop_at)));
        }
        return;
    }
    cancel_matching_schedules(tasks, scanner, sender, ts, now, from_transcript);
    if sender == "user" {
        return;
    }
    if let Some(task_id) = shell_task_id(sender) {
        let status = if let Some(spec) = scanner.schedules.get(&task_id) {
            if spec.is_cron {
                let count = scanner
                    .cron_triggers
                    .entry(task_id.clone())
                    .and_modify(|c| *c += 1)
                    .or_insert(1);
                if spec.max_iterations.is_none_or(|max| *count < max)
                    && !content.contains("Command exited with code ")
                {
                    return;
                }
            }
            "completed"
        } else {
            classify_task_exit(content)
        };
        if let Some(task) = tasks.iter_mut().find(|task| task.task_id == task_id) {
            if !task.finished() || task.synthetic_end {
                task.set_status(status, ts.or(Some(now)).unwrap());
            }
        } else {
            scanner.completed_outcomes.insert(task_id, (status, ts));
        }
        return;
    }
    if crate::agent::antigravity::is_native_id(sender) {
        if let Some(task) = tasks
            .iter_mut()
            .find(|task| task.task_id == sender && task.task_type == "local_agent")
        {
            if !task.finished() || task.synthetic_end {
                task.set_status("completed", ts.or(Some(now)).unwrap());
            }
        } else {
            if scanner.pending_agent_completions.len() >= MAX_PENDING_COMPLETIONS {
                if let Some(oldest) = scanner.pending_agent_completions.keys().next().cloned() {
                    scanner.pending_agent_completions.remove(&oldest);
                }
            }
            scanner.pending_agent_completions.insert(sender.to_string(), ts);
        }
    }
}

fn sender_matches_condition(sender: &str, target: &str) -> bool {
    sender == target
        || (shell_task_id(sender).is_some() && shell_task_id(sender) == shell_task_id(target))
}

/// Cancel active `schedule` timers whose `TimerCondition` is satisfied by an arriving delivery or user turn.
fn cancel_matching_schedules(
    tasks: &mut [BackgroundTask],
    scanner: &Scanner,
    sender: &str,
    ts: Option<u64>,
    now: u64,
    from_transcript: bool,
) {
    if sender == "system" {
        return;
    }
    let sender_task = shell_task_id(sender);
    let event_at = ts.unwrap_or(now);
    for task in tasks.iter_mut() {
        if task.finished() {
            continue;
        }
        let Some(spec) = scanner.schedules.get(&task.task_id) else {
            continue;
        };
        if sender_task.as_deref() == Some(task.task_id.as_str()) {
            continue;
        }
        // Disk message timestamps outlive an already-finished previous task; ignore deliveries older than the timer.
        if !from_transcript {
            if let (Some(msg_ts), Some(started)) = (ts, task.started_at) {
                if msg_ts + 1_000 < started {
                    continue;
                }
            }
        }
        let matched = match &spec.condition {
            ScheduleCondition::Never => false,
            ScheduleCondition::Any => true,
            ScheduleCondition::Sender(target) => sender_matches_condition(sender, target),
        };
        if matched {
            task.set_status("canceled", event_at);
            task.synthetic_end = true;
        }
    }
}

fn reconcile_outcomes(
    tasks: &mut Vec<BackgroundTask>,
    scanner: &mut Scanner,
    initial_open: bool,
    now: u64,
) {
    for task in tasks.iter_mut() {
        if matches!(task.task_type.as_str(), "local_bash" | "local_workflow") {
            if let Some((status, ts)) = scanner.completed_outcomes.remove(&task.task_id) {
                if !task.finished() || task.synthetic_end {
                    task.set_status(status, ts.unwrap_or(now));
                }
            }
        } else if task.task_type == "local_agent" {
            if let Some(ts) = scanner.pending_agent_completions.remove(&task.task_id) {
                if !task.finished() || task.synthetic_end {
                    task.set_status("completed", ts.unwrap_or(now));
                }
            }
        }
    }

    let finished_targets: Vec<(String, u64)> = tasks
        .iter()
        .filter(|task| task.finished())
        .map(|task| (task.task_id.clone(), task.ended_at.unwrap_or(now)))
        .collect();
    for task in tasks.iter_mut() {
        if task.finished() {
            continue;
        }
        let Some(spec) = scanner.schedules.get(&task.task_id) else {
            continue;
        };
        let ScheduleCondition::Sender(target) = &spec.condition else {
            continue;
        };
        let timer_started_at = task.started_at.unwrap_or(0);
        if let Some((_, other_ended_at)) = finished_targets.iter().find(|(other_id, ended)| {
            other_id != &task.task_id
                && sender_matches_condition(other_id, target)
                && *ended + 1_000 >= timer_started_at
        }) {
            task.set_status("canceled", *other_ended_at);
            task.synthetic_end = true;
        }
    }

    if let Some(stop_at) = scanner.restart_stopped_at {
        for task in tasks.iter_mut() {
            if !task.finished() && task.started_at.unwrap_or(0) <= stop_at {
                task.set_status("stopped", stop_at);
            }
        }
    }

    // A reopened or newly loaded ACP process does not inherit background tasks from an exited prior
    // process, and Antigravity only logs the restart notice when a new user prompt arrives.
    if initial_open {
        for task in tasks.iter_mut() {
            if !task.finished() {
                task.set_status("ended", now);
                task.synthetic_end = true;
            }
        }
    }
}

fn parse_pending_call(name: &str, args: &Value, planner_step: Option<u64>) -> PendingCall {
    let mut schedule = None;
    let mut manage = None;
    let (summary, command, recipient) = match name {
        "run_command" => (arg_string(args, "toolSummary"), arg_string(args, "CommandLine"), None),
        "run_workflow" => (
            arg_string(args, "Description"),
            arg_string(args, "Script").or_else(|| arg_string(args, "ScriptPath")),
            None,
        ),
        "schedule" => {
            let prompt = arg_string(args, "Prompt");
            let is_cron = arg_string(args, "CronExpression").is_some();
            let duration_ms = if is_cron {
                None
            } else {
                arg_u64(args, "DurationSeconds").map(|s| s.saturating_mul(1_000))
            };
            let max_iterations = arg_u64(args, "MaxIterations");
            let condition = if is_cron {
                ScheduleCondition::Never
            } else {
                match arg_string(args, "TimerCondition") {
                    None => ScheduleCondition::Never,
                    Some(cond) if cond.eq_ignore_ascii_case("never") => ScheduleCondition::Never,
                    Some(cond) if cond.eq_ignore_ascii_case("any") => ScheduleCondition::Any,
                    Some(cond) => ScheduleCondition::Sender(cond),
                }
            };
            schedule = Some(ScheduleSpec {
                is_cron,
                duration_ms,
                max_iterations,
                condition,
            });
            (prompt.clone(), prompt, None)
        }
        "invoke_subagent" => (
            arg_string(args, "Role").or_else(|| arg_string(args, "TypeName")),
            arg_string(args, "Prompt"),
            None,
        ),
        "send_message" => (None, None, arg_string(args, "Recipient")),
        "manage_task" => {
            manage = match arg_string(args, "Action").map(|a| a.to_ascii_lowercase()).as_deref() {
                Some("list") => Some(ManageCall::TaskList),
                Some("kill") => arg_string(args, "TaskId")
                    .and_then(|raw| shell_task_id(&raw))
                    .map(|task_id| ManageCall::TaskKill { task_id }),
                Some("status") => Some(ManageCall::TaskStatus {
                    task_id: arg_string(args, "TaskId").and_then(|raw| shell_task_id(&raw)),
                }),
                _ => None,
            };
            (None, None, None)
        }
        "manage_subagents" => {
            manage = match arg_string(args, "Action").map(|a| a.to_ascii_lowercase()).as_deref() {
                Some("list") => Some(ManageCall::SubagentList),
                Some("kill") => {
                    let ids: Vec<String> = args
                        .get("ConversationIds")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|v| v.as_str())
                        .filter(|id| crate::agent::antigravity::is_native_id(id))
                        .map(str::to_string)
                        .collect();
                    (!ids.is_empty()).then_some(ManageCall::SubagentKill { ids })
                }
                Some("kill_all") => Some(ManageCall::SubagentKillAll),
                _ => None,
            };
            (None, None, None)
        }
        _ => (None, None, None),
    };
    PendingCall {
        name: name.to_string(),
        summary,
        command,
        recipient,
        planner_step,
        schedule,
        manage,
    }
}

fn extract_shell_task_ids(content: &str) -> HashSet<String> {
    content
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/')))
        .filter_map(shell_task_id)
        .collect()
}

fn extract_native_ids(content: &str) -> HashSet<String> {
    content
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_')))
        .filter(|token| crate::agent::antigravity::is_native_id(token))
        .map(str::to_string)
        .collect()
}

/// Read only `manage_task(status)` header lines before `Log output:`/`Output:` so log contents
/// cannot spoof the task status.
fn parse_manage_task_status(content: &str) -> (Option<String>, Option<&'static str>, bool) {
    let mut task_id = None;
    let mut status = None;
    let mut running = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Log output:") || trimmed.starts_with("Output:") {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Task:") {
            if task_id.is_none() {
                task_id = shell_task_id(rest);
            }
            continue;
        }
        if status.is_none() && !running {
            if let Some(rest) = trimmed.strip_prefix("Status:") {
                match rest.trim().to_ascii_lowercase().as_str() {
                    "running" | "starting" | "pending" => running = true,
                    "killed" | "canceled" | "cancelled" => status = Some("canceled"),
                    "stopped" | "shutdown" => status = Some("stopped"),
                    "failed" | "error" => status = Some("failed"),
                    "done" | "state_done" | "completed" | "finished" => status = Some("completed"),
                    _ => {}
                }
            }
        }
    }
    (task_id, status, running)
}

fn parse_system_messages(content: &str) -> Vec<(Option<String>, String, String, Option<u64>)> {
    parse_system_deliveries(content)
        .into_iter()
        .map(|(timestamp_raw, sender, body)| {
            let ts = parsed_timestamp_ms(timestamp_raw.as_deref());
            (timestamp_raw, sender, body, ts)
        })
        .collect()
}

fn signature(tasks: &[BackgroundTask]) -> String {
    let mut out = String::new();
    for task in tasks {
        out.push_str(&format!(
            "{}|{}|{}|{}|{:?}|{:?}|{:?}|{}|{}\n",
            task.task_id,
            task.task_type,
            task.status,
            task.description,
            task.summary,
            task.started_at,
            task.ended_at,
            task.listed,
            task.synthetic_end,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use serde_json::json;

    use super::*;
    use crate::agent::chat::engine::tests::{ctx, inert_process};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("vlx-agy-tasks-{tag}-{}-{n}", std::process::id()));
            std::fs::create_dir_all(dir.join("logs")).unwrap();
            std::fs::create_dir_all(dir.join("messages")).unwrap();
            std::fs::create_dir_all(dir.join("tasks")).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn append_step(dir: &Path, step: Value, newline: bool) {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("logs/transcript_full.jsonl"))
            .unwrap();
        let line = if newline {
            format!("{step}\n")
        } else {
            step.to_string()
        };
        f.write_all(line.as_bytes()).unwrap();
    }

    fn finish_incomplete_line(dir: &Path) {
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(dir.join("logs/transcript_full.jsonl"))
            .unwrap();
        f.write_all(b"\n").unwrap();
    }

    fn write_message(dir: &Path, name: &str, sender: &str, timestamp: &str, content: &str) {
        let payload = json!({
            "sender": sender,
            "timestamp": timestamp,
            "content": content,
        });
        std::fs::write(dir.join("messages").join(name), payload.to_string()).unwrap();
    }

    #[test]
    fn tracks_shell_workflow_and_completion_messages_incrementally() {
        let dir = TempDir::new("shell");
        let app = ctx("agy_tasks_shell");
        let proc = inert_process(SessionKind::Antigravity);
        on_open_dir(&app, "s", &proc, Some(dir.0.clone()));

        // Partial line at EOF is deferred until terminated with newline.
        append_step(&dir.0, json!({
            "step_index": 0,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T04:00:00Z",
            "tool_calls": [
                {"name": "run_command", "args": {"CommandLine": "\"cargo test\"", "toolSummary": "\"Unit tests\""}},
                {"name": "run_workflow", "args": {"Description": "Audit workflow", "Script": "print('hi')"}},
                {"name": "schedule", "args": {"Prompt": "Remind in 60s"}}
            ]
        }), false);
        refresh(&app, "s", &proc, false);
        assert!(proc.acp.lock().unwrap().tasks.scanner.pending_calls.is_empty());

        finish_incomplete_line(&dir.0);
        append_step(&dir.0, json!({
            "step_index": 1,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-15T04:00:01Z",
            "content": "Created At: 2026-11-15T12:00:01+08:00\nTool is running as a background task with task id: conv-1/task-41\nTask Description: cargo test\nTask logs are available at: file:///tmp/task-41.log"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 2,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-15T04:00:02Z",
            "content": "Tool is running as a background task with task id: conv-1/task-42\nTask Description: Audit workflow"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 3,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-15T04:00:03Z",
            "content": "Tool is running as a background task with task id: conv-1/task-43\nTask Description: Remind in 60s"
        }), true);

        refresh(&app, "s", &proc, false);
        {
            let extras = proc.extras.lock().unwrap().published();
            assert_eq!(extras.background_tasks.len(), 3);
            assert_eq!(extras.background_tasks[0].task_id, "task-41");
            assert_eq!(extras.background_tasks[0].task_type, "local_bash");
            assert_eq!(extras.background_tasks[0].description, "Unit tests");
            assert_eq!(extras.background_tasks[0].status, "running");
            assert!(!extras.background_tasks[0].can_stop);
            assert_eq!(extras.background_tasks[1].task_id, "task-42");
            assert_eq!(extras.background_tasks[1].task_type, "local_workflow");
            assert_eq!(extras.background_tasks[2].task_id, "task-43");
            assert_eq!(extras.background_tasks[2].task_type, "local_bash");
        }
        assert_eq!(proc.turn.lock().unwrap().background_tasks.len(), 3);

        // Task output works before and after the log file exists, for both bash and workflow.
        let empty = task_output(&proc, "task-41").unwrap();
        assert_eq!(empty.command.as_deref(), Some("cargo test"));
        assert_eq!(empty.output.as_deref(), Some(""));
        assert!(!empty.truncated);

        std::fs::write(dir.0.join("tasks/task-42.log"), "step 1\nstep 2\n").unwrap();
        let wf = task_output(&proc, "task-42").unwrap();
        assert_eq!(wf.command.as_deref(), Some("print('hi')"));
        assert_eq!(wf.output.as_deref(), Some("step 1\nstep 2\n"));

        // Truncated failure message still classifies code 2 on line 5 as failed.
        write_message(&dir.0, "m1.json", "conv-1/task-41", "2026-11-15T04:00:10Z",
            "Task id \"conv-1/task-41\" finished with result:\nThe tool's output was truncated...\n\nHere is a preview:\nCommand exited with code 2.\n\nCommand output:\nfail");
        write_message(&dir.0, "m2.json", "conv-1/task-42", "2026-11-15T04:00:11Z",
            "Task id \"conv-1/task-42\" was canceled with result:\nTool execution was canceled");
        write_message(&dir.0, "m3.json", "conv-1/task-43", "2026-11-15T04:00:12Z",
            "Task id \"conv-1/task-43\" finished with result:\nCommand exited with code 0.\n\nCommand output:\nok");
        // read.json and directory entries are ignored.
        std::fs::write(dir.0.join("messages/read.json"), "{\"read\":[]}").unwrap();
        std::fs::create_dir_all(dir.0.join("messages/undelivered")).unwrap();

        refresh(&app, "s", &proc, false);
        let extras = proc.extras.lock().unwrap().published();
        assert_eq!(extras.background_tasks[0].status, "failed");
        assert!(extras.background_tasks[0].finished);
        assert_eq!(extras.background_tasks[1].status, "canceled");
        assert_eq!(extras.background_tasks[2].status, "completed");
        assert!(proc.turn.lock().unwrap().background_tasks.is_empty());
    }

    #[test]
    fn tracks_subagent_lifecycle_and_orphaned_tasks_on_open() {
        let dir = TempDir::new("agent");
        let app = ctx("agy_tasks_agent");
        let proc = inert_process(SessionKind::Antigravity);

        append_step(&dir.0, json!({
            "step_index": 0,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T04:00:00Z",
            "tool_calls": [
                {"name": "invoke_subagent", "args": {"Role": "Researcher", "TypeName": "research", "Prompt": "Find files"}},
                {"name": "run_command", "args": {"CommandLine": "sleep 99", "toolSummary": "Old orphan"}}
            ]
        }), true);
        append_step(&dir.0, json!({
            "step_index": 1,
            "type": "GENERIC",
            "status": "DONE",
            "created_at": "2026-11-15T04:00:01Z",
            "content": "Subagent started: {\"conversationId\":\"child-uuid-1\"}"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 2,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-15T04:00:01Z",
            "content": "Tool is running as a background task with task id: conv-1/task-9\nTask Description: sleep 99"
        }), true);

        // Reopening settles pre-existing unfinished tasks because a fresh ACP process does not own them.
        on_open_dir(&app, "s", &proc, Some(dir.0.clone()));
        {
            let extras = proc.extras.lock().unwrap().published();
            assert_eq!(extras.background_tasks.len(), 2);
            assert_eq!(extras.background_tasks[0].task_id, "child-uuid-1");
            assert_eq!(extras.background_tasks[0].task_type, "local_agent");
            assert_eq!(extras.background_tasks[0].status, "ended");
            assert_eq!(extras.background_tasks[1].status, "ended");
            assert!(task_output(&proc, "child-uuid-1").is_err());
        }

        // Reviving the subagent via send_message puts it back to running, and a stale SYSTEM_MESSAGE
        // delivered earlier in the same transcript does not re-complete it.
        append_step(&dir.0, json!({
            "step_index": 3,
            "type": "SYSTEM_MESSAGE",
            "created_at": "2026-11-15T04:01:00Z",
            "content": "<SYSTEM_MESSAGE>\n[Message] timestamp=2026-11-15T04:01:00Z sender=child-uuid-1 priority=MESSAGE_PRIORITY_LOW content=First done"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 4,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T04:01:01Z",
            "tool_calls": [{"name": "send_message", "args": {"Recipient": "child-uuid-1", "Message": "Keep going"}}]
        }), true);
        append_step(&dir.0, json!({
            "step_index": 5,
            "type": "GENERIC",
            "status": "DONE",
            "created_at": "2026-11-15T04:01:02Z",
            "content": "Delivered"
        }), true);
        write_message(&dir.0, "m-old.json", "child-uuid-1", "2026-11-15T04:01:00Z", "First done");
        refresh(&app, "s", &proc, false);
        assert_eq!(proc.extras.lock().unwrap().published().background_tasks[0].status, "running");

        // A new message from the child marks it completed.
        write_message(&dir.0, "m-new.json", "child-uuid-1", "2026-11-15T04:02:00Z", "Second done");
        refresh(&app, "s", &proc, false);
        assert_eq!(proc.extras.lock().unwrap().published().background_tasks[0].status, "completed");
    }

    #[test]
    fn system_restart_notice_marks_earlier_running_tasks_stopped() {
        let dir = TempDir::new("restart");
        let app = ctx("agy_tasks_restart");
        let proc = inert_process(SessionKind::Antigravity);
        on_open_dir(&app, "s", &proc, Some(dir.0.clone()));

        append_step(&dir.0, json!({
            "step_index": 0,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T04:00:00Z",
            "tool_calls": [{"name": "run_command", "args": {"CommandLine": "sleep 50"}}]
        }), true);
        append_step(&dir.0, json!({
            "step_index": 1,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-15T04:00:01Z",
            "content": "Tool is running as a background task with task id: conv-1/task-5\nTask Description: sleep 50"
        }), true);
        refresh(&app, "s", &proc, false);
        assert_eq!(proc.extras.lock().unwrap().published().background_tasks[0].status, "running");

        write_message(&dir.0, "m-sys.json", "system", "2026-11-15T04:00:30Z",
            "[Notice] All your subagents and background tasks have been stopped due to server restart");
        refresh(&app, "s", &proc, false);
        let extras = proc.extras.lock().unwrap().published();
        assert_eq!(extras.background_tasks[0].status, "stopped");
        assert!(extras.background_tasks[0].finished);
    }

    #[test]
    fn vrun_checkpoint_is_completed_and_reactive_wakeup_projects_progress_report() {
        let dir = TempDir::new("reactive");
        let app = ctx("agy_tasks_reactive");
        let statuses = Arc::new(Mutex::new(Vec::<String>::new()));
        let captured = statuses.clone();
        app.listen(&crate::pty::StatusSignal::event_name("s"), move |payload| {
            let v: Value = serde_json::from_str(payload).unwrap();
            if let Some(state) = v.get("state").and_then(Value::as_str) {
                captured.lock().unwrap().push(state.to_string());
            }
        });

        let proc = inert_process(SessionKind::Antigravity);
        *proc.model.lock().unwrap() = Some("gemini-pro-orchestrator".into());
        on_open_dir(&app, "s", &proc, Some(dir.0.clone()));

        // Initial prompted turn launches `task-2`.
        append_step(&dir.0, json!({
            "step_index": 0,
            "type": "USER_INPUT",
            "created_at": "2026-11-15T04:00:00Z",
            "content": "<USER_REQUEST>run story</USER_REQUEST>"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 1,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T04:00:01Z",
            "tool_calls": [{"name": "run_command", "args": {"CommandLine": "vrun story echo", "toolSummary": "Generate story"}}]
        }), true);
        append_step(&dir.0, json!({
            "step_index": 2,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-15T04:00:02Z",
            "content": "Tool is running as a background task with task id: conv-1/task-2\nTask Description: Generate story"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 3,
            "type": "PLANNER_RESPONSE",
            "status": "DONE",
            "created_at": "2026-11-15T04:00:03Z",
            "content": "Started in background."
        }), true);
        before_turn_end(&app, "s", &proc);
        assert!(proc.timeline.lock().unwrap().rows.is_empty());

        // `task-2` hits its 3-minute `vrun` checkpoint (exit code 75) -> classified as completed + holds wake grace.
        let report_msg = "Task id \"conv-1/task-2\" finished with result:\nCommand exited with code 75.\n\nCommand output:\nvrun: story is still running · 180s elapsed · PID 42\nvrun: progress report; the command keeps running. Continue with `vrun --wait story`, which reports again in 180s.";
        write_message(&dir.0, "m-vrun-1.json", "conv-1/task-2", "2026-11-15T04:03:00Z", report_msg);
        assert!(!refresh(&app, "s", &proc, false));
        {
            let extras = proc.extras.lock().unwrap().published();
            assert_eq!(extras.background_tasks[0].status, "completed");
            assert!(extras.background_tasks[0].finished);
        }
        assert!(proc.turn.lock().unwrap().settling.contains_key("task-2"));

        // `agy` wakes asynchronously: poll 1 sees SYSTEM_MESSAGE + vrun --wait tool call (`Working`).
        statuses.lock().unwrap().clear();
        append_step(&dir.0, json!({
            "step_index": 4,
            "type": "SYSTEM_MESSAGE",
            "created_at": "2026-11-15T04:03:01Z",
            "content": format!("<SYSTEM_MESSAGE>\n[Message] timestamp=2026-11-15T04:03:00Z sender=conv-1/task-2 priority=MESSAGE_PRIORITY_HIGH content={report_msg}")
        }), true);
        append_step(&dir.0, json!({
            "step_index": 5,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T04:03:02Z",
            "thinking": "Report progress and keep waiting.",
            "tool_calls": [{"name": "run_command", "args": {"CommandLine": "vrun --wait story", "toolSummary": "Wait for story"}}]
        }), true);
        assert!(!refresh(&app, "s", &proc, false));
        assert!(proc.turn.lock().unwrap().running);
        assert!(proc.turn.lock().unwrap().settling.is_empty());
        assert_eq!(statuses.lock().unwrap().last().map(String::as_str), Some("working"));

        // Poll 2 sees `task-3` launch + final prose progress report -> settles tool, computes duration, enters `Background`.
        statuses.lock().unwrap().clear();
        append_step(&dir.0, json!({
            "step_index": 6,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-15T04:03:03Z",
            "content": "Tool is running as a background task with task id: conv-1/task-3\nTask Description: Wait for story"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 7,
            "type": "PLANNER_RESPONSE",
            "status": "DONE",
            "created_at": "2026-11-15T04:03:05Z",
            "content": "Progress: 3/10 complete; next report in 3 minutes."
        }), true);
        assert!(refresh(&app, "s", &proc, false));
        assert!(!proc.turn.lock().unwrap().running);
        assert!(proc.turn.lock().unwrap().background_tasks.contains("task-3"));
        assert_eq!(statuses.lock().unwrap().last().map(String::as_str), Some("background"));
        {
            let rows = &proc.timeline.lock().unwrap().rows;
            assert!(matches!(&rows[0], ChatRow::Wake { id, summary, .. }
                if id == "agy-step-4" && summary == "Background task \"Generate story\" completed"));
            assert!(matches!(&rows[1], ChatRow::Reasoning { id, streaming: false, .. } if id == "agy-step-5-thinking"));
            assert!(matches!(&rows[2], ChatRow::Tool { id, name, status: "completed", .. } if id == "agy-step-5-call-0" && name == "Bash"));
            assert!(matches!(&rows[3], ChatRow::Assistant { id, text, model, duration_ms: Some(_), .. }
                if id == "agy-step-7" && text.contains("3/10 complete") && model.as_deref() == Some("gemini-pro-orchestrator")));
        }

        // Final completion wakeup with no new background task transitions back to `Waiting`.
        statuses.lock().unwrap().clear();
        append_step(&dir.0, json!({
            "step_index": 8,
            "type": "SYSTEM_MESSAGE",
            "created_at": "2026-11-15T04:06:00Z",
            "content": "<SYSTEM_MESSAGE>\n[Message] timestamp=2026-11-15T04:06:00Z sender=conv-1/task-3 content=Command exited with code 0."
        }), true);
        append_step(&dir.0, json!({
            "step_index": 9,
            "type": "PLANNER_RESPONSE",
            "status": "DONE",
            "created_at": "2026-11-15T04:06:02Z",
            "content": "All 10 items finished."
        }), true);
        assert!(refresh(&app, "s", &proc, false));
        assert!(proc.turn.lock().unwrap().background_tasks.is_empty());
        assert_eq!(statuses.lock().unwrap().as_slice(), ["working", "waiting"]);
    }

    #[test]
    fn initial_open_and_prompted_turns_do_not_emit_reactive_rows() {
        let dir = TempDir::new("no_dup");
        let app = ctx("agy_tasks_no_dup");
        let proc = inert_process(SessionKind::Antigravity);

        // Pre-existing historical wakeup turn in transcript on open must not be projected live.
        append_step(&dir.0, json!({
            "step_index": 0,
            "type": "SYSTEM_MESSAGE",
            "created_at": "2026-11-15T04:00:00Z",
            "content": "[Message] timestamp=2026-11-15T04:00:00Z sender=conv-1/task-1 content=Command exited with code 0."
        }), true);
        append_step(&dir.0, json!({
            "step_index": 1,
            "type": "PLANNER_RESPONSE",
            "status": "DONE",
            "created_at": "2026-11-15T04:00:02Z",
            "content": "Old wakeup answer"
        }), true);
        on_open_dir(&app, "s", &proc, Some(dir.0.clone()));
        assert!(proc.timeline.lock().unwrap().rows.is_empty());

        // Live USER_INPUT turn steps (including mid-turn SYSTEM_MESSAGE) are driven by ACP, not projected here.
        append_step(&dir.0, json!({
            "step_index": 2,
            "type": "USER_INPUT",
            "created_at": "2026-11-15T04:01:00Z",
            "content": "<USER_REQUEST>status?</USER_REQUEST>"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 3,
            "type": "SYSTEM_MESSAGE",
            "created_at": "2026-11-15T04:01:01Z",
            "content": "[Message] timestamp=2026-11-15T04:01:01Z sender=conv-1/task-9 content=Command exited with code 0."
        }), true);
        append_step(&dir.0, json!({
            "step_index": 4,
            "type": "PLANNER_RESPONSE",
            "status": "DONE",
            "created_at": "2026-11-15T04:01:02Z",
            "content": "Prompted answer"
        }), true);
        refresh(&app, "s", &proc, false);
        assert!(proc.timeline.lock().unwrap().rows.is_empty());
        assert!(!proc.acp.lock().unwrap().tasks.reactive_active());
    }

    #[test]
    fn reactive_wakeup_supports_steer_and_interrupt() {
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};

        let dir = TempDir::new("steer_stop");
        let app = ctx("agy_tasks_steer_stop");
        let proc = inert_process(SessionKind::Antigravity);
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        *proc.stdin.lock().unwrap() = child.stdin.take();
        let mut agent = BufReader::new(child.stdout.take().unwrap());
        *proc.child.lock().unwrap() = child;
        *proc.agent_session_id.lock().unwrap() = Some("agy-1".into());
        proc.ready.store(true, Ordering::Relaxed);

        on_open_dir(&app, "s", &proc, Some(dir.0.clone()));

        // Open a reactive turn with an in-progress tool call, then steer into it without a prior prompt.
        append_step(&dir.0, json!({
            "step_index": 0,
            "type": "SYSTEM_MESSAGE",
            "created_at": "2026-11-15T04:02:00Z",
            "content": "[Message] timestamp=2026-11-15T04:02:00Z sender=conv-1/task-1 content=Command exited with code 0."
        }), true);
        append_step(&dir.0, json!({
            "step_index": 1,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T04:02:01Z",
            "tool_calls": [{"name": "view_file", "args": {"AbsolutePath": "/tmp/a.txt"}}]
        }), true);
        refresh(&app, "s", &proc, false);
        assert!(proc.turn.lock().unwrap().running);
        assert!(proc.acp.lock().unwrap().tasks.reactive_active());

        super::super::acp::steer(&proc, "focus on tests", &[]).unwrap();
        let mut steered_line = String::new();
        agent.read_line(&mut steered_line).unwrap();
        let steered: Value = serde_json::from_str(&steered_line).unwrap();
        assert_eq!(steered["method"], "session/prompt");
        assert!(!proc.acp.lock().unwrap().tasks.reactive_active());
        assert!(super::super::acp::has_active_prompt(&proc));
        assert!(matches!(
            proc.timeline.lock().unwrap().get("agy-step-1-call-0"),
            Some(ChatRow::Tool { status: "completed", .. })
        ));

        // Simulate steer finishing normally so turn returns idle, then open a second reactive turn to interrupt.
        super::super::acp::handle_line(
            &app,
            "s",
            &proc,
            &json!({"jsonrpc": "2.0", "id": steered["id"], "result": {"stopReason": "end_turn"}}).to_string(),
        );
        assert!(!proc.turn.lock().unwrap().running);

        append_step(&dir.0, json!({
            "step_index": 2,
            "type": "SYSTEM_MESSAGE",
            "created_at": "2026-11-15T04:05:00Z",
            "content": "[Message] timestamp=2026-11-15T04:05:00Z sender=conv-1/task-2 content=Command exited with code 0."
        }), true);
        append_step(&dir.0, json!({
            "step_index": 3,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T04:05:01Z",
            "tool_calls": [{"name": "run_command", "args": {"CommandLine": "sleep 60"}}]
        }), true);
        refresh(&app, "s", &proc, false);
        assert!(proc.turn.lock().unwrap().running);
        assert!(!super::super::acp::has_active_prompt(&proc));

        let ended = {
            let mut turn = proc.turn.lock().unwrap();
            super::super::interrupt_locked(&app, &proc, &mut turn).unwrap()
        };
        let end = ended.expect("interrupting a reactive turn ends it immediately");
        super::super::finish_turn_end(&app, "s", &proc, "interrupted", None, end);

        let mut cancel_line = String::new();
        agent.read_line(&mut cancel_line).unwrap();
        let cancelled: Value = serde_json::from_str(&cancel_line).unwrap();
        assert_eq!(cancelled["method"], "session/cancel");
        assert!(!proc.turn.lock().unwrap().running);
        assert!(!proc.acp.lock().unwrap().tasks.reactive_active());
        assert!(matches!(
            proc.timeline.lock().unwrap().get("agy-step-3-call-0"),
            Some(ChatRow::Tool { status: "completed", .. })
        ));
    }

    #[test]
    fn schedule_timer_condition_and_one_shot_expiry_reconcile_cleanly() {
        let dir = TempDir::new("sched");
        let app = ctx("agy_tasks_sched");
        let proc = inert_process(SessionKind::Antigravity);
        on_open_dir(&app, "s", &proc, Some(dir.0.clone()));

        append_step(&dir.0, json!({
            "step_index": 0,
            "type": "USER_INPUT",
            "created_at": "2026-11-15T10:58:00Z",
            "content": "<USER_REQUEST>run tests</USER_REQUEST>"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 1,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T10:58:10Z",
            "tool_calls": [{"name": "run_command", "args": {"CommandLine": "cargo test", "toolSummary": "Run tests"}}]
        }), true);
        append_step(&dir.0, json!({
            "step_index": 2,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-15T10:58:11Z",
            "content": "Tool is running as a background task with task id: conv-1/task-461\nTask Description: Run tests"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 3,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T10:59:42Z",
            "tool_calls": [{"name": "schedule", "args": {"DurationSeconds": 180, "Prompt": "Check completion", "TimerCondition": "conv-1/task-461"}}]
        }), true);
        append_step(&dir.0, json!({
            "step_index": 4,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-15T10:59:43Z",
            "content": "Tool is running as a background task with task id: conv-1/task-489\nTask Description: Timer: 3m"
        }), true);
        refresh(&app, "s", &proc, false);
        {
            let extras = proc.extras.lock().unwrap().published();
            assert_eq!(extras.background_tasks.len(), 2);
            assert!(extras.background_tasks.iter().all(|t| t.status == "running"));
        }

        // Delivery for task-461 satisfies task-489's TimerCondition and cancels task-489 without its own message.
        write_message(&dir.0, "m1.json", "conv-1/task-461", "2026-11-15T11:02:26Z",
            "Task id \"conv-1/task-461\" finished with result:\nCommand exited with code 0.\n\nCommand output:\nok");
        refresh(&app, "s", &proc, false);
        {
            let extras = proc.extras.lock().unwrap().published();
            assert_eq!(extras.background_tasks[0].task_id, "task-461");
            assert_eq!(extras.background_tasks[0].status, "completed");
            assert_eq!(extras.background_tasks[1].task_id, "task-489");
            assert_eq!(extras.background_tasks[1].status, "canceled");
        }
        assert!(proc.turn.lock().unwrap().background_tasks.is_empty());

        // A TimerCondition="any" timer cancels on USER_INPUT.
        append_step(&dir.0, json!({
            "step_index": 5,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T11:03:00Z",
            "tool_calls": [{"name": "schedule", "args": {"DurationSeconds": 30, "Prompt": "Wait for any message", "TimerCondition": "any"}}]
        }), true);
        append_step(&dir.0, json!({
            "step_index": 6,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-15T11:03:01Z",
            "content": "Tool is running as a background task with task id: conv-1/task-490\nTask Description: Timer: 30s"
        }), true);
        refresh(&app, "s", &proc, false);
        assert_eq!(proc.extras.lock().unwrap().published().background_tasks[2].status, "running");

        append_step(&dir.0, json!({
            "step_index": 7,
            "type": "USER_INPUT",
            "created_at": "2026-11-15T11:03:05Z",
            "content": "<USER_REQUEST>interrupt timer</USER_REQUEST>"
        }), true);
        refresh(&app, "s", &proc, false);
        assert_eq!(proc.extras.lock().unwrap().published().background_tasks[2].status, "canceled");

        // A one-shot timer that fires naturally settles as completed, while a cron timer stays running until MaxIterations.
        append_step(&dir.0, json!({
            "step_index": 8,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T11:04:00Z",
            "tool_calls": [
                {"name": "schedule", "args": {"DurationSeconds": 10, "Prompt": "One shot"}},
                {"name": "schedule", "args": {"CronExpression": "* * * * *", "MaxIterations": 2, "Prompt": "Cron job"}}
            ]
        }), true);
        append_step(&dir.0, json!({
            "step_index": 9,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-15T11:04:01Z",
            "content": "Tool is running as a background task with task id: conv-1/task-491\nTask Description: One shot"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 10,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-15T11:04:01Z",
            "content": "Tool is running as a background task with task id: conv-1/task-492\nTask Description: Cron job"
        }), true);
        write_message(&dir.0, "m2.json", "conv-1/task-491", "2026-11-15T11:04:11Z",
            "Scheduled timer of 10s (task \"conv-1/task-491\") has expired.");
        write_message(&dir.0, "m3.json", "conv-1/task-492", "2026-11-15T11:05:00Z",
            "[task-492] Cron triggered (iteration 1).");
        refresh(&app, "s", &proc, false);
        {
            let extras = proc.extras.lock().unwrap().published();
            assert_eq!(extras.background_tasks[3].task_id, "task-491");
            assert_eq!(extras.background_tasks[3].status, "completed");
            assert_eq!(extras.background_tasks[4].task_id, "task-492");
            assert_eq!(extras.background_tasks[4].status, "running");
        }
        write_message(&dir.0, "m4.json", "conv-1/task-492", "2026-11-15T11:06:00Z",
            "[task-492] Cron triggered (iteration 2).");
        refresh(&app, "s", &proc, false);
        assert_eq!(proc.extras.lock().unwrap().published().background_tasks[4].status, "completed");
    }

    #[test]
    fn manage_task_list_kill_and_status_reconcile_silent_exits_without_clobbering_concurrent_launches() {
        let dir = TempDir::new("manage");
        let app = ctx("agy_tasks_manage");
        let proc = inert_process(SessionKind::Antigravity);
        on_open_dir(&app, "s", &proc, Some(dir.0.clone()));

        append_step(&dir.0, json!({
            "step_index": 1,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T11:00:00Z",
            "tool_calls": [
                {"name": "run_command", "args": {"CommandLine": "sleep 10", "toolSummary": "First"}},
                {"name": "run_command", "args": {"CommandLine": "sleep 20", "toolSummary": "Second"}}
            ]
        }), true);
        append_step(&dir.0, json!({
            "step_index": 2,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-15T11:00:01Z",
            "content": "Tool is running as a background task with task id: conv-1/task-10\nTask Description: First"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 2,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-15T11:00:01Z",
            "content": "Tool is running as a background task with task id: conv-1/task-11\nTask Description: Second"
        }), true);

        // `manage_task(kill)` marks task-10 canceled immediately.
        append_step(&dir.0, json!({
            "step_index": 3,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T11:00:05Z",
            "tool_calls": [{"name": "manage_task", "args": {"Action": "kill", "TaskId": "conv-1/task-10"}}]
        }), true);
        append_step(&dir.0, json!({
            "step_index": 4,
            "type": "GENERIC",
            "status": "DONE",
            "created_at": "2026-11-15T11:00:06Z",
            "content": "Killed task conv-1/task-10"
        }), true);
        refresh(&app, "s", &proc, false);
        {
            let extras = proc.extras.lock().unwrap().published();
            assert_eq!(extras.background_tasks[0].status, "canceled");
            assert_eq!(extras.background_tasks[1].status, "running");
        }

        // Same planner turn calls both `run_command` (task-12) and `manage_task(list)`: task-11 settles, concurrent task-12 stays running.
        append_step(&dir.0, json!({
            "step_index": 5,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T11:00:10Z",
            "tool_calls": [
                {"name": "run_command", "args": {"CommandLine": "sleep 30", "toolSummary": "Third"}},
                {"name": "manage_task", "args": {"Action": "list"}}
            ]
        }), true);
        append_step(&dir.0, json!({
            "step_index": 6,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-15T11:00:11Z",
            "content": "Tool is running as a background task with task id: conv-1/task-12\nTask Description: Third"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 7,
            "type": "GENERIC",
            "status": "DONE",
            "created_at": "2026-11-15T11:00:12Z",
            "content": "No background tasks are currently running."
        }), true);
        refresh(&app, "s", &proc, false);
        {
            let extras = proc.extras.lock().unwrap().published();
            assert_eq!(extras.background_tasks[1].task_id, "task-11");
            assert_eq!(extras.background_tasks[1].status, "ended");
            assert_eq!(extras.background_tasks[2].task_id, "task-12");
            assert_eq!(extras.background_tasks[2].status, "running");
        }

        // `Log output:` lines cannot spoof header status; only a DONE header completes task-12.
        append_step(&dir.0, json!({
            "step_index": 8,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T11:00:20Z",
            "tool_calls": [{"name": "manage_task", "args": {"Action": "status", "TaskId": "conv-1/task-12"}}]
        }), true);
        append_step(&dir.0, json!({
            "step_index": 9,
            "type": "GENERIC",
            "status": "DONE",
            "created_at": "2026-11-15T11:00:21Z",
            "content": "Task: conv-1/task-12\nStatus: RUNNING\nLog output:\nStatus: DONE\n"
        }), true);
        refresh(&app, "s", &proc, false);
        assert_eq!(proc.extras.lock().unwrap().published().background_tasks[2].status, "running");

        append_step(&dir.0, json!({
            "step_index": 10,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-15T11:00:25Z",
            "tool_calls": [{"name": "manage_task", "args": {"Action": "status", "TaskId": "conv-1/task-12"}}]
        }), true);
        append_step(&dir.0, json!({
            "step_index": 11,
            "type": "GENERIC",
            "status": "DONE",
            "created_at": "2026-11-15T11:00:26Z",
            "content": "Task: conv-1/task-12\nStatus: DONE\nLog output:\ndone\n"
        }), true);
        refresh(&app, "s", &proc, false);
        assert_eq!(proc.extras.lock().unwrap().published().background_tasks[2].status, "completed");
    }

    #[test]
    fn unread_message_helper_skips_read_system_and_dedupes_wake_rows() {
        let dir = TempDir(std::env::temp_dir().join(format!("agy-unread-{}", uuid::Uuid::new_v4())));
        let msg_dir = dir.0.join("messages");
        let undelivered_dir = msg_dir.join("undelivered");
        std::fs::create_dir_all(&undelivered_dir).unwrap();
        std::fs::write(msg_dir.join("read.json"), r#"{"r1":true,"u1":true}"#).unwrap();
        std::fs::write(msg_dir.join("r1.json"), json!({"sender":"task-1","timestamp":"2026-11-16T01:00:00Z","content":"old"}).to_string()).unwrap();
        std::fs::write(msg_dir.join("sys.json"), json!({"sender":"system","timestamp":"2026-11-16T01:00:30Z","content":"ignore"}).to_string()).unwrap();
        std::fs::write(msg_dir.join("u2.json"), json!({"sender":"task-2","timestamp":"2026-11-16T01:02:00.789012Z","content":"Command exited with code 0"}).to_string()).unwrap();
        // Same id staged in undelivered with microsecond timestamp is still discovered before delivery.
        std::fs::write(undelivered_dir.join("u1.json"), json!({"sender":"task-1","timestamp":"2026-11-16T01:01:00.123456Z","content":"Command exited with code 0"}).to_string()).unwrap();

        let surfaced = HashSet::new();
        let woken = HashSet::new();
        let mut surfaced_mut = surfaced;
        let first = next_unread_message(&dir.0, &surfaced_mut, &woken).unwrap();
        assert_eq!(first.file_name, "u1");
        surfaced_mut.insert(first.file_name.clone());
        let second = next_unread_message(&dir.0, &surfaced_mut, &woken).unwrap();
        assert_eq!(second.file_name, "u2");

        let app = ctx("agy-wake-dedup");
        let proc = inert_process(SessionKind::Antigravity);
        proc.acp.lock().unwrap().tasks.system_dir = Some(dir.0.clone());
        emit_unread_wake(&app, "s", &proc, &first);
        assert_eq!(proc.timeline.lock().unwrap().rows.iter().filter(|r| matches!(r, ChatRow::Wake { .. })).count(), 1);

        // Later transcript replay of the same delivery with whole-second timestamp must NOT duplicate the Wake row.
        std::fs::create_dir_all(dir.0.join("logs")).unwrap();
        append_step(&dir.0, json!({
            "step_index": 0,
            "type": "SYSTEM_MESSAGE",
            "created_at": "2026-11-16T01:01:01Z",
            "content": "<SYSTEM_MESSAGE>\n[Message] timestamp=2026-11-16T01:01:00Z sender=task-1 priority=MESSAGE_PRIORITY_HIGH content=Command exited with code 0\n</SYSTEM_MESSAGE>"
        }), true);
        refresh(&app, "s", &proc, false);
        assert_eq!(proc.timeline.lock().unwrap().rows.iter().filter(|r| matches!(r, ChatRow::Wake { .. })).count(), 1);

        // Inline SYSTEM_MESSAGE delivery while an ACP prompt is still active emits WakeOnly cleanly.
        proc.acp.lock().unwrap().prompt = Some("p-live".into());
        append_step(&dir.0, json!({
            "step_index": 1,
            "type": "PLANNER_RESPONSE",
            "status": "DONE",
            "created_at": "2026-11-16T01:01:10Z",
            "content": "SCHEDULED"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 2,
            "type": "SYSTEM_MESSAGE",
            "created_at": "2026-11-16T01:02:00Z",
            "content": "<SYSTEM_MESSAGE>\n[Message] timestamp=2026-11-16T01:02:00Z sender=task-3 priority=MESSAGE_PRIORITY_HIGH content=Timer woke\n</SYSTEM_MESSAGE>"
        }), true);
        refresh(&app, "s", &proc, false);
        assert_eq!(proc.timeline.lock().unwrap().rows.iter().filter(|r| matches!(r, ChatRow::Wake { .. })).count(), 2);
    }

    #[test]
    fn headless_stall_detection_respects_mode_wait_and_fires_once() {
        assert_eq!(headless_stall_limit_ms("ask_question", &json!({}), "bypassPermissions"), Some(3_000));
        assert_eq!(headless_stall_limit_ms("run_command", &json!({}), "bypassPermissions"), None);
        assert_eq!(headless_stall_limit_ms("run_command", &json!({"WaitMsBeforeAsync": 5000}), "default"), Some(9_000));
        assert_eq!(headless_stall_limit_ms("run_command", &json!({"WaitMsBeforeAsync": "5000"}), "default"), Some(9_000));
        assert_eq!(headless_stall_limit_ms("replace_file_content", &json!({}), "default"), Some(4_000));

        let mut scanner = Scanner::default();
        scanner.in_reactive_turn = true;
        scanner.reactive_calls.push_back(ReactiveToolCall {
            row_id: "agy-step-1-call-0".into(),
            name: "ask_question".into(),
            args: json!({"questions": []}),
            seen_at_ms: 10_000,
            recovered: false,
        });
        assert!(detect_stalled_reactive_call(&mut scanner, 12_000, "default").is_none());
        assert!(matches!(
            detect_stalled_reactive_call(&mut scanner, 13_500, "default"),
            Some(ReactiveAction::RecoverStalledCall { row_id, name, .. }) if row_id == "agy-step-1-call-0" && name == "ask_question"
        ));
        assert!(detect_stalled_reactive_call(&mut scanner, 20_000, "default").is_none());
    }

    #[test]
    fn schedule_deadline_wakes_once_and_suppresses_duplicate_system_message_wake() {
        let dir = TempDir::new("sched-due");
        let app = ctx("agy-sched-due");
        let proc = inert_process(SessionKind::Antigravity);
        proc.acp.lock().unwrap().tasks.system_dir = Some(dir.0.clone());

        append_step(&dir.0, json!({
            "step_index": 0,
            "type": "USER_INPUT",
            "created_at": "2026-11-16T01:34:51Z",
            "content": "Schedule a 8s timer"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 1,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-16T01:34:51Z",
            "tool_calls": [{"name": "schedule", "args": {"DurationSeconds": "8", "Prompt": "Wake"}}]
        }), true);
        append_step(&dir.0, json!({
            "step_index": 2,
            "type": "GENERIC",
            "status": "RUNNING",
            "created_at": "2026-11-16T01:34:59Z",
            "content": "Tool is running as a background task with task id: c1/task-6\nTask Description: Timer: 8s, Prompt: Wake\n"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 3,
            "type": "PLANNER_RESPONSE",
            "status": "DONE",
            "created_at": "2026-11-16T01:34:59Z",
            "content": "SCHEDULED"
        }), true);

        proc.acp.lock().unwrap().prompt = Some("p-live".into());
        refresh(&app, "s", &proc, false);

        let started_ms = crate::agent::chat::antigravity_protocol::parsed_timestamp_ms(Some("2026-11-16T01:34:59Z")).unwrap();
        let woken_schedules = HashSet::new();
        assert!(next_due_schedule(&proc, started_ms + 8_500, &woken_schedules).is_none());
        let due = next_due_schedule(&proc, started_ms + 9_250, &woken_schedules).expect("schedule due after grace");
        assert_eq!(due.task_id, "task-6");
        assert_eq!(due.wake_key, "task-6:0");

        emit_schedule_wake(&app, "s", &proc, &due);
        assert_eq!(proc.timeline.lock().unwrap().rows.iter().filter(|r| matches!(r, ChatRow::Wake { .. })).count(), 1);

        // Subsequent SYSTEM_MESSAGE delivery for the same timer must not emit a duplicate Wake row.
        append_step(&dir.0, json!({
            "step_index": 4,
            "type": "SYSTEM_MESSAGE",
            "created_at": "2026-11-16T01:35:08Z",
            "content": "<SYSTEM_MESSAGE>\n[Message] timestamp=2026-11-16T01:35:08Z sender=c1/task-6 priority=MESSAGE_PRIORITY_HIGH content=Wake\n</SYSTEM_MESSAGE>"
        }), true);
        refresh(&app, "s", &proc, false);
        assert_eq!(proc.timeline.lock().unwrap().rows.iter().filter(|r| matches!(r, ChatRow::Wake { .. })).count(), 1);
    }

    #[test]
    fn reactive_ask_question_pops_on_ask_question_step_and_avoids_false_stall() {
        let dir = TempDir::new("reactive-ask-q");
        let mut scanner = Scanner::default();
        append_step(&dir.0, json!({
            "step_index": 0,
            "type": "SYSTEM_MESSAGE",
            "created_at": "2026-11-16T02:00:00Z",
            "content": "<SYSTEM_MESSAGE>\n[Message] timestamp=2026-11-16T02:00:00Z sender=c1/task-1 priority=MESSAGE_PRIORITY_HIGH content=Done\n</SYSTEM_MESSAGE>"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 1,
            "type": "PLANNER_RESPONSE",
            "created_at": "2026-11-16T02:00:01Z",
            "tool_calls": [
                {"name": "ask_question", "args": {"questions": [{"question": "Continue?", "is_multi_select": false, "options": [{"id": "1", "text": "Yes"}]}]}},
                {"name": "view_file", "args": {"AbsolutePath": "/tmp/a"}}
            ]
        }), true);
        append_step(&dir.0, json!({
            "step_index": 2,
            "type": "ASK_QUESTION",
            "status": "DONE",
            "created_at": "2026-11-16T02:00:02Z",
            "content": "Created At: 2026-11-16T10:00:00+08:00\nCompleted At: 2026-11-16T10:00:02+08:00\nA1: Yes"
        }), true);
        append_step(&dir.0, json!({
            "step_index": 3,
            "type": "VIEW_FILE",
            "status": "DONE",
            "created_at": "2026-11-16T02:00:03Z",
            "content": "ok"
        }), true);

        let (_, reactive) = scan_transcript(&dir.0, &mut scanner, 10_000, true, false);
        assert_eq!(scanner.reactive_calls.len(), 0);
        assert!(detect_stalled_reactive_call(&mut scanner, 20_000, "default").is_none());
        assert!(reactive.iter().any(|action| matches!(
            action,
            ReactiveAction::Upsert(ChatRow::Tool { id, name, input, output, status: "completed", .. })
                if id == "agy-step-1-call-0" && name == "AskUserQuestion" && input["answers"] == json!({"Continue?": "Yes"}) && output.as_deref() == Some("A1: Yes")
        )));
        assert!(reactive.iter().any(|action| matches!(
            action,
            ReactiveAction::Upsert(ChatRow::Tool { id, name, output, status: "completed", .. })
                if id == "agy-step-1-call-1" && name == "Read" && output.as_deref() == Some("ok")
        )));
    }
}
