//! Server-side handlers for the `vagent` agent-control API.
//!
//! The `vagent` shim POSTs JSON to `/agent/<op>` on the local hook service. Every operation carries
//! `parentSessionId` from `VLX_SESSION_ID` and may only touch that session's live descendants, so a
//! session can never control siblings or ancestors. Handlers run on per-request worker threads (see
//! server.rs) because `wait` and `spawn` block; the hook loop itself must never stall.
//!
//! `spawn` uses a correlation registry: the handler parks on a channel keyed by a fresh request id,
//! the frontend creates the session and reports back through the `spawn_result` dispatch command,
//! and the parked handler answers the CLI with the real session id.
//! `spawn` resolves profiles and enforces limits from `agent::orchestration`.
//! Registry entries record the parent session id, and `spawn-status` requires the same
//! `parentSessionId` that opened the request; a mismatch answers 404 as if the id were unknown.

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::agent::{inject, orchestration};
use crate::host::AppCtx;
use crate::models::{Session, SessionKind};

/// Outcome reported by the frontend for one correlated spawn request.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnOutcome {
    /// Created session id on success.
    pub session_id: Option<String>,
    /// Failure or cancellation reason; a set value means no session was created.
    pub error: Option<String>,
    /// Worktree creation failure; the session still exists in the parent directory.
    pub worktree_error: Option<String>,
    /// Set when the confirmation card opens. The request remains registered for its final outcome.
    #[serde(default)]
    pub awaiting_confirmation: bool,
}

/// One open spawn request: its start time, originating parent session id, and result channel.
type OpenSpawn = (Instant, String, mpsc::Sender<SpawnOutcome>);

/// Open spawn requests by correlation id. Entries remain while confirmation is pending.
static SPAWN_WAITERS: LazyLock<Mutex<HashMap<String, OpenSpawn>>> = LazyLock::new(Default::default);

/// One stored final outcome: its arrival time and the parent id that may collect it.
type StoredOutcome = (Instant, String, SpawnOutcome);

/// Final outcomes for requests whose handlers already returned `pending`.
static SPAWN_OUTCOMES: LazyLock<Mutex<HashMap<String, StoredOutcome>>> =
    LazyLock::new(Default::default);

/// Retention limits for pending requests and stored outcomes.
const OUTCOME_TTL: Duration = Duration::from_secs(3600);
const OUTCOME_CAP: usize = 128;
const DIFF_PATCH_MAX_BYTES: usize = 256 * 1024;

/// Default answer timeout for `spawn`; a not-started child holds a parallel slot for this same window.
const SPAWN_DEFAULT_TIMEOUT_SECS: i64 = 120;

/// Register a spawn correlation id for `parent` and return the receiver its outcome arrives on.
pub fn register_spawn_waiter(request_id: &str, parent: &str) -> mpsc::Receiver<SpawnOutcome> {
    let (tx, rx) = mpsc::channel();
    let now = Instant::now();
    let mut map = SPAWN_WAITERS.lock().unwrap();
    map.retain(|_, (at, _, _)| now.duration_since(*at) < OUTCOME_TTL);
    map.insert(request_id.to_string(), (now, parent.to_string(), tx));
    rx
}

/// Deliver a spawn outcome to its handler and retain final outcomes for `spawn-status`.
pub fn resolve_spawn(request_id: &str, outcome: SpawnOutcome) -> bool {
    if outcome.awaiting_confirmation {
        let waiter = SPAWN_WAITERS
            .lock()
            .unwrap()
            .get(request_id)
            .map(|(_, _, tx)| tx.clone());
        return match waiter {
            Some(tx) => tx.send(outcome).is_ok(),
            None => false,
        };
    }
    let waiter = SPAWN_WAITERS.lock().unwrap().remove(request_id);
    match waiter {
        Some((_, parent, tx)) => {
            store_outcome(request_id, &parent, outcome.clone());
            tx.send(outcome).is_ok()
        }
        None => false,
    }
}

fn store_outcome(request_id: &str, parent: &str, outcome: SpawnOutcome) {
    let now = Instant::now();
    let mut map = SPAWN_OUTCOMES.lock().unwrap();
    map.retain(|_, (at, _, _)| now.duration_since(*at) < OUTCOME_TTL);
    if map.len() >= OUTCOME_CAP {
        if let Some(oldest) = map
            .iter()
            .min_by_key(|(_, (at, _, _))| *at)
            .map(|(id, _)| id.clone())
        {
            map.remove(&oldest);
        }
    }
    map.insert(request_id.to_string(), (now, parent.to_string(), outcome));
}

/// Remove and return the outcome only when `parent` opened the request.
fn take_outcome(request_id: &str, parent: &str) -> Option<SpawnOutcome> {
    let mut map = SPAWN_OUTCOMES.lock().unwrap();
    match map.get(request_id) {
        Some((_, owner, _)) if owner == parent => {
            map.remove(request_id).map(|(_, _, outcome)| outcome)
        }
        _ => None,
    }
}


/// Handle one `/agent/<op>` request and return (HTTP status, JSON body).
pub fn handle(op: &str, body: &str, app: &AppCtx) -> (u16, String) {
    let parsed: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return err(400, "invalid JSON body"),
    };
    let Some(parent) = str_field(&parsed, "parentSessionId") else {
        return err(400, "missing parentSessionId");
    };
    match op {
        "list" => op_list(app, &parent),
        "status" => op_status(app, &parent, &parsed),
        "wait" => op_wait(app, &parent, &parsed),
        "read" => op_read(app, &parent, &parsed),
        "prompt" => op_prompt(app, &parent, &parsed),
        "cancel" => op_cancel(app, &parent, &parsed),
        "diff" => op_diff(app, &parent, &parsed),
        "merge" => op_merge(app, &parent, &parsed),
        "spawn" => op_spawn(app, &parent, &parsed),
        "spawn-status" => op_spawn_status(app, &parent, &parsed),
        "config" => op_config(app, &parent),
        "cleanup" => op_cleanup(app, &parent, &parsed),
        _ => err(404, "unknown operation"),
    }
}

// ── Operations ──

fn op_list(app: &AppCtx, parent: &str) -> (u16, String) {
    let sessions = match descendants(app, parent) {
        Ok(s) => s,
        Err(e) => return err(500, &e),
    };
    let rows: Vec<Value> = sessions.iter().map(|s| session_row(app, s)).collect();
    ok(json!({ "sessions": rows }))
}

fn op_status(app: &AppCtx, parent: &str, req: &Value) -> (u16, String) {
    match resolve_target(app, parent, req) {
        Ok(s) => ok(session_row(app, &s)),
        Err(r) => r,
    }
}

fn op_wait(app: &AppCtx, parent: &str, req: &Value) -> (u16, String) {
    // Accept one `target` or a `targets` array; `mode` is "all" (default) or "any".
    let mut names: Vec<String> = req
        .get("targets")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if let Some(t) = str_field(req, "target") {
        names.push(t);
    }
    if names.is_empty() {
        return err(400, "missing target");
    }
    let mut ids = Vec::new();
    for name in &names {
        match resolve_named(app, parent, name) {
            Ok(s) => ids.push(s.id),
            Err(r) => return r,
        }
    }
    let any = str_field(req, "mode").as_deref() == Some("any");
    let timeout = req
        .get("timeoutSecs")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| orchestration::load(app).limits.default_timeout_secs);

    let app2 = app.clone();
    let (timed_out, states) = wait_states(
        &ids,
        any,
        Duration::from_secs(timeout),
        Duration::from_millis(500),
        move |sid| state_of(&app2, sid),
    );
    let blocked = blocked_targets(&ids, &states);
    let outcomes: Vec<_> = ids
        .iter()
        .map(|id| session_by_id(app, id).map(|s| turn_outcome_of(&s)).unwrap_or_default())
        .collect();
    let failed = failed_targets(&ids, &outcomes, |id| {
        session_by_id(app, id)
            .map(|session| session.name)
            .unwrap_or_else(|| id.to_string())
    });
    let rows: Vec<Value> = ids
        .iter()
        .zip(&states)
        .zip(&outcomes)
        .map(|((id, state), outcome)| {
            json!({
                "id": id,
                "state": state,
                "lastTurnOutcome": outcome.outcome,
                "lastTurnError": outcome.error.clone(),
            })
        })
        .collect();
    ok(json!({ "timedOut": timed_out, "blocked": blocked, "failed": failed, "sessions": rows }))
}

fn op_read(app: &AppCtx, parent: &str, req: &Value) -> (u16, String) {
    let session = match resolve_target(app, parent, req) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let Some(agent_id) = session.agent_session_id.as_deref() else {
        return err(409, "session has no conversation yet");
    };
    let messages = match crate::agent::transcript::read(session.kind, agent_id) {
        Ok(m) => m,
        Err(e) => return err(500, &e),
    };
    let outcome = turn_outcome_of(&session);
    if req.get("full").and_then(Value::as_bool) == Some(true) {
        let rows: Vec<Value> = messages
            .iter()
            .map(|m| json!({ "role": m.role, "text": m.text, "timestamp": m.timestamp }))
            .collect();
        return ok(json!({
            "id": session.id,
            "messages": rows,
            "lastTurnOutcome": outcome.outcome,
            "lastTurnError": outcome.error,
        }));
    }
    if outcome.outcome == crate::agent::transcript::TurnOutcome::Error {
        return ok(json!({
            "id": session.id,
            "role": "error",
            "text": outcome.error.unwrap_or_else(|| "agent turn failed".to_string()),
            "outcome": outcome.outcome,
        }));
    }
    match messages.iter().rev().find(|m| m.role == "assistant") {
        Some(m) => ok(json!({
            "id": session.id,
            "role": "assistant",
            "text": m.text,
            "timestamp": m.timestamp,
            "tools": m.tools,
        })),
        None => err(404, "no assistant response yet"),
    }
}

fn op_prompt(app: &AppCtx, parent: &str, req: &Value) -> (u16, String) {
    let session = match resolve_target(app, parent, req) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let Some(text) = str_field(req, "text").filter(|t| !t.trim().is_empty()) else {
        return err(400, "missing text");
    };
    if !app.pty().is_running(&session.id) {
        return err(409, "session is not running; open it in VelaTerm first");
    }
    let state = state_of(app, &session.id);
    let force = req.get("force").and_then(Value::as_bool) == Some(true);
    if state == "working" && !force {
        return err(409, "session is working; wait for it or pass force");
    }
    // Bracketed paste keeps multiline text as one input block, then CR submits it. The short delay
    // lets the agent TUI ingest the paste before the submit keystroke.
    if let Err(e) = app
        .pty()
        .write(&session.id, &format!("\x1b[200~{text}\x1b[201~"))
    {
        return err(500, &e);
    }
    std::thread::sleep(Duration::from_millis(150));
    if let Err(e) = app.pty().write(&session.id, "\r") {
        return err(500, &e);
    }
    ok(json!({ "id": session.id, "delivered": true }))
}

fn op_cancel(app: &AppCtx, parent: &str, req: &Value) -> (u16, String) {
    if req.get("all").and_then(Value::as_bool) == Some(true) {
        let sessions = match descendants(app, parent) {
            Ok(s) => s,
            Err(e) => return err(500, &e),
        };
        let (mut interrupted, mut skipped) = (Vec::new(), Vec::new());
        for s in sessions {
            match app.pty().is_running(&s.id) && app.pty().write(&s.id, "\x1b").is_ok() {
                true => interrupted.push(s.id),
                false => skipped.push(s.id),
            }
        }
        return ok(json!({ "interrupted": interrupted, "skipped": skipped }));
    }
    let session = match resolve_target(app, parent, req) {
        Ok(s) => s,
        Err(r) => return r,
    };
    if !app.pty().is_running(&session.id) {
        return err(409, "session is not running");
    }
    // Escape interrupts the active turn in Claude and Codex TUIs; the session stays alive.
    if let Err(e) = app.pty().write(&session.id, "\x1b") {
        return err(500, &e);
    }
    ok(json!({ "id": session.id, "interrupted": true }))
}

fn op_spawn(app: &AppCtx, parent: &str, req: &Value) -> (u16, String) {
    let Some(prompt) = str_field(req, "prompt").filter(|p| !p.trim().is_empty()) else {
        return err(400, "missing prompt");
    };
    let config = orchestration::load(app);
    let profile = match str_field(req, "profile") {
        Some(name) => match config.profiles.get(&name) {
            Some(p) => Some(p.clone()),
            None => {
                return (
                    400,
                    json!({
                        "error": format!("unknown profile \"{name}\""),
                        "available": config.profile_names(),
                    })
                    .to_string(),
                )
            }
        },
        None => None,
    };

    // Reject invalid spawns before registering a result waiter.
    let counts = match subtree_counts(app, parent) {
        Ok(c) => c,
        Err(e) => return err(500, &e),
    };
    if let Some(rejection) = orchestration::check_limits(
        &config.limits,
        counts.depth,
        counts.descendants,
        counts.active,
    ) {
        return (429, rejection.to_json().to_string());
    }

    let resolved = orchestration::resolve_spawn(
        profile.as_ref(),
        str_field(req, "kind"),
        str_field(req, "model"),
        str_field(req, "effort"),
        req.get("worktree").and_then(Value::as_bool),
        str_field(req, "permissionMode"),
    );

    // Curated launch values are advisory only. Installed CLIs can accept newer values than this build knows.
    let launch_warnings = if !req
        .get("allowUnknownLaunchValues")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let kind = effective_kind(app, parent, resolved.kind.as_deref());
        launch_value_warnings(
            kind,
            resolved.model.as_deref(),
            resolved.effort.as_deref(),
        )
    } else {
        Vec::new()
    };

    let permission_mode = resolved.permission_mode;
    if let Some(mode) = permission_mode.as_deref() {
        if !PERMISSION_MODES.contains(&mode) {
            return (
                400,
                json!({
                    "error": format!("unknown permission mode \"{mode}\""),
                    "field": "permissionMode",
                    "value": mode,
                    "available": PERMISSION_MODES,
                })
                .to_string(),
            );
        }
    }

    let request_id = uuid::Uuid::new_v4().to_string();
    let rx = register_spawn_waiter(&request_id, parent);
    let force_confirm = orchestration::needs_confirmation(&config.limits, counts.active)
        || !launch_warnings.is_empty();
    let spawn_req = crate::agent::server::SpawnRequest {
        parent_session_id: parent.to_string(),
        prompt,
        kind: resolved.kind,
        worktree: Some(resolved.worktree),
        model: resolved.model,
        effort: resolved.effort,
        name: str_field(req, "name"),
        agent_args: str_field(req, "agentArgs"),
        permission_mode,
        auto_approve: config.limits.auto_approve && !force_confirm,
        request_id: Some(request_id.clone()),
        force_confirm: force_confirm.then_some(true),
        launch_warnings,
    };
    app.emit("spawn://request", spawn_req);

    let timeout = req
        .get("timeoutSecs")
        .and_then(Value::as_u64)
        .unwrap_or(SPAWN_DEFAULT_TIMEOUT_SECS as u64);
    match rx.recv_timeout(Duration::from_secs(timeout)) {
        Ok(outcome) if outcome.awaiting_confirmation => ok(json!({
            "pending": true,
            "requestId": request_id,
            "awaitingConfirmation": true,
        })),
        Ok(outcome) => spawn_outcome_response(app, outcome),
        Err(_) => {
            ok(json!({ "pending": true, "requestId": request_id }))
        }
    }
}

/// Convert a final spawn outcome into the CLI response.
fn spawn_outcome_response(app: &AppCtx, outcome: SpawnOutcome) -> (u16, String) {
    match outcome.session_id {
        Some(sid) => {
            let mut row = match session_by_id(app, &sid) {
                Some(s) => session_row(app, &s),
                None => json!({ "id": sid }),
            };
            if let Some(we) = outcome.worktree_error {
                row["worktreeError"] = json!(we);
            }
            ok(row)
        }
        None => err(
            409,
            outcome.error.as_deref().unwrap_or("spawn was not completed"),
        ),
    }
}

/// Collect the result of a spawn that returned `pending`. A request id opened by a different
/// parent answers the same 404 as an unknown id, so the response does not reveal that it exists.
fn op_spawn_status(app: &AppCtx, parent: &str, req: &Value) -> (u16, String) {
    let Some(request_id) = str_field(req, "requestId") else {
        return err(400, "missing requestId");
    };
    match take_outcome(&request_id, parent) {
        Some(outcome) => spawn_outcome_response(app, outcome),
        None => {
            let known = SPAWN_WAITERS
                .lock()
                .unwrap()
                .get(&request_id)
                .is_some_and(|(_, owner, _)| owner == parent);
            if known {
                ok(json!({ "pending": true, "requestId": request_id }))
            } else {
                err(404, "unknown or expired requestId")
            }
        }
    }
}

/// Permission modes accepted for child sessions.
const PERMISSION_MODES: [&str; 2] = ["default", "skip"];

/// Resolve the agent kind used for launch-value validation.
fn effective_kind(app: &AppCtx, parent: &str, requested: Option<&str>) -> SessionKind {
    match requested {
        Some(k) => SessionKind::from_db(k),
        None => session_by_id(app, parent)
            .map(|s| s.kind)
            .unwrap_or(SessionKind::Claude),
    }
}

/// Warn when a launch value is outside the build's curated list. The installed agent remains authoritative.
fn launch_value_warnings(
    kind: SessionKind,
    model: Option<&str>,
    effort: Option<&str>,
) -> Vec<String> {
    let check = |field: &str, value: Option<&str>, known: Option<&'static [&'static str]>| {
        let value = value?;
        let known = known?;
        if known.contains(&value) {
            return None;
        }
        Some(format!(
            "The {field} \"{value}\" is outside VelaTerm's curated values for {}. Verify that the installed CLI accepts it.",
            kind.as_str()
        ))
    };
    [
        check("model", model, inject::known_models(kind)),
        check("effort", effort, inject::known_efforts(kind)),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Report effective orchestration settings and current descendant counts.
fn op_config(app: &AppCtx, parent: &str) -> (u16, String) {
    let config = orchestration::load(app);
    let counts = match subtree_counts(app, parent) {
        Ok(c) => c,
        Err(e) => return err(500, &e),
    };
    ok(json!({
        "profiles": config.profiles_json(),
        "limits": config.limits.to_json(),
        "counts": {
            "descendants": counts.descendants,
            "active": counts.active,
            "depth": counts.depth,
        },
    }))
}

/// List clean worktrees for finished children, or remove them when `confirm` is set.
fn op_cleanup(app: &AppCtx, parent: &str, req: &Value) -> (u16, String) {
    let sessions = match descendants(app, parent) {
        Ok(s) => s,
        Err(e) => return err(500, &e),
    };
    let confirm = req.get("confirm").and_then(Value::as_bool) == Some(true);

    let (mut candidates, mut blocked, mut removed, mut failed) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for s in sessions {
        let Some(path) = s.worktree_path.clone().filter(|p| !p.trim().is_empty()) else {
            continue;
        };
        let row = |reason: &str| {
            json!({ "id": s.id, "name": s.name, "path": path, "reason": reason })
        };
        let state = state_of(app, &s.id);
        if state == "working" || state == "starting" || app.pty().is_running(&s.id) {
            blocked.push(row("session is still running"));
            continue;
        }
        if crate::git::worktree_has_changes(&path) {
            blocked.push(row("worktree has uncommitted changes"));
            continue;
        }
        if !confirm {
            candidates.push(json!({
                "id": s.id,
                "name": s.name,
                "path": path,
                "branch": crate::git::worktree_branch(&path),
            }));
            continue;
        }
        match crate::git::worktree_remove(&path, false) {
            Ok(()) => {
                if let Ok(conn) = app.db().conn.lock() {
                    let _ = crate::db::repo::clear_node_worktree(
                        &conn,
                        crate::models::NodeKind::Session,
                        &s.id,
                    );
                }
                removed.push(json!({ "id": s.id, "name": s.name, "path": path }));
            }
            Err(e) => failed.push(row(&e)),
        }
    }
    if confirm {
        ok(json!({ "removed": removed, "failed": failed, "blocked": blocked }))
    } else {
        ok(json!({ "candidates": candidates, "blocked": blocked }))
    }
}

fn op_diff(app: &AppCtx, parent: &str, req: &Value) -> (u16, String) {
    let session = match resolve_target(app, parent, req) {
        Ok(session) => session,
        Err(response) => return response,
    };
    let Some(path) = session
        .worktree_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    else {
        return err(409, "session has no worktree");
    };
    let targets = match crate::git::land_targets(session.worktree_base_ref.as_deref(), path) {
        Ok(targets) => targets,
        Err(error) => return err(500, &error),
    };
    let patch = match crate::git::branch_diff_patch(path, &targets.base_ref, &targets.branch) {
        Ok(patch) => patch,
        Err(error) => return err(500, &error),
    };
    let (patch, truncated) = cap_patch(patch);
    ok(json!({
        "id": session.id,
        "name": session.name,
        "baseRef": targets.base_ref,
        "baseBranch": targets.base_branch,
        "branch": targets.branch,
        "diffStat": targets.diff_stat,
        "patch": patch,
        "truncated": truncated,
        "hasUncommitted": targets.has_uncommitted,
    }))
}

fn cap_patch(mut patch: String) -> (String, bool) {
    if patch.len() <= DIFF_PATCH_MAX_BYTES {
        return (patch, false);
    }
    let mut end = DIFF_PATCH_MAX_BYTES;
    while !patch.is_char_boundary(end) {
        end -= 1;
    }
    patch.truncate(end);
    (patch, true)
}

fn op_merge(app: &AppCtx, parent: &str, req: &Value) -> (u16, String) {
    let session = match resolve_target(app, parent, req) {
        Ok(session) => session,
        Err(response) => return response,
    };
    let Some(path) = session
        .worktree_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    else {
        return err(409, "session has no worktree");
    };
    let targets = match crate::git::land_targets(session.worktree_base_ref.as_deref(), path) {
        Ok(targets) => targets,
        Err(error) => return err(500, &error),
    };
    if targets.has_uncommitted {
        return (
            409,
            json!({
                "error": "worktree has uncommitted changes",
                "id": session.id,
                "name": session.name,
                "branch": targets.branch,
            })
            .to_string(),
        );
    }

    let preview =
        match crate::git::merge_branches_preview(path, &targets.branch, &targets.base_branch) {
            Ok(preview) => preview,
            Err(error) => return err(500, &error),
        };
    if preview.source_dirty {
        return (
            409,
            json!({
                "error": "worktree has uncommitted changes",
                "id": session.id,
                "name": session.name,
                "branch": targets.branch,
            })
            .to_string(),
        );
    }
    if !preview.available {
        return (
            409,
            json!({
                "error": "merge is unavailable",
                "reason": preview.reason,
                "source": targets.branch,
                "target": targets.base_branch,
            })
            .to_string(),
        );
    }

    let outcome =
        match crate::git::merge_branches_apply(path, &targets.branch, &targets.base_branch, None) {
            Ok(outcome) => outcome,
            Err(error) => return err(500, &error),
        };
    if outcome.conflict {
        return (
            409,
            json!({
                "error": "merge conflict",
                "source": targets.branch,
                "target": targets.base_branch,
                "conflicts": outcome.conflicts,
            })
            .to_string(),
        );
    }

    let delete_worktree = req.get("deleteWorktree").and_then(Value::as_bool) == Some(true);
    if delete_worktree {
        if let Err(error) = crate::git::worktree_remove(path, false) {
            return (
                500,
                json!({
                    "error": error,
                    "merged": true,
                    "source": targets.branch,
                    "target": targets.base_branch,
                    "worktreeDeleted": false,
                })
                .to_string(),
            );
        }
        if let Ok(conn) = app.db().conn.lock() {
            let _ = crate::db::repo::clear_node_worktree(
                &conn,
                crate::models::NodeKind::Session,
                &session.id,
            );
        }
    }

    ok(json!({
        "id": session.id,
        "name": session.name,
        "source": targets.branch,
        "target": targets.base_branch,
        "merged": outcome.merged,
        "message": outcome.message,
        "worktreeDeleted": delete_worktree,
    }))
}

// ── Shared helpers ──

/// Live descendant counts and depth used by the spawn guardrails.
struct SubtreeCounts {
    descendants: u32,
    active: u32,
    depth: u32,
}

/// Whether a session in `state` holds a parallel slot. "not-started" counts only inside the grace
/// window, because a spawn returns before its PTY starts; an older not-started session is a
/// leftover from a restart or a closed pane and must not hold a slot forever. "asking" counts
/// because the child holds a worktree and resumes as soon as the prompt is answered.
fn consumes_parallel_slot(state: &str, age_secs: i64) -> bool {
    match state {
        "starting" | "working" | "asking" => true,
        // A negative age comes from a clock change and stays inside the window.
        "not-started" => age_secs < SPAWN_DEFAULT_TIMEOUT_SECS,
        _ => false,
    }
}

fn subtree_counts(app: &AppCtx, parent: &str) -> Result<SubtreeCounts, String> {
    let sessions = descendants(app, parent)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let active = sessions
        .iter()
        .filter(|s| consumes_parallel_slot(&state_of(app, &s.id), now - s.created_at))
        .count();
    let depth = {
        let db = app.db();
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        crate::db::repo::session_depth(&conn, parent)?
    };
    Ok(SubtreeCounts {
        descendants: sessions.len() as u32,
        active: active as u32,
        depth,
    })
}

/// Current lifecycle state of a session's PTY: `working`/`asking`/`waiting` from the hook cache,
/// `starting` while running without a state yet, `exited` after the PTY ends, `not-started` before
/// the first launch.
pub fn state_of(app: &AppCtx, sid: &str) -> String {
    let pty = app.pty();
    if pty.is_running(sid) {
        pty.cached_agent_state(sid)
            .unwrap_or_else(|| "starting".to_string())
    } else if pty.status_snapshot(sid).is_empty() {
        "not-started".to_string()
    } else {
        "exited".to_string()
    }
}

/// Targets that settled the wait on a permission prompt instead of finishing.
fn blocked_targets<'a>(ids: &'a [String], states: &[String]) -> Vec<&'a String> {
    ids.iter()
        .zip(states)
        .filter(|(_, state)| state.as_str() == "asking")
        .map(|(id, _)| id)
        .collect()
}

fn failed_targets(
    ids: &[String],
    outcomes: &[crate::agent::transcript::TurnOutcomeInfo],
    name_of: impl Fn(&str) -> String,
) -> Vec<Value> {
    ids.iter()
        .zip(outcomes)
        .filter(|(_, outcome)| outcome.outcome == crate::agent::transcript::TurnOutcome::Error)
        .map(|(id, outcome)| {
            json!({ "id": id, "name": name_of(id), "error": outcome.error.clone() })
        })
        .collect()
}

/// Block until the targets leave busy states (`working`/`starting`) or the timeout passes.
/// `any` returns once one target settles; otherwise all must settle. Returns (timed_out, final
/// states in target order). `poll` is injectable for tests.
fn wait_states(
    ids: &[String],
    any: bool,
    timeout: Duration,
    poll: Duration,
    state_fn: impl Fn(&str) -> String,
) -> (bool, Vec<String>) {
    let deadline = Instant::now() + timeout;
    loop {
        let states: Vec<String> = ids.iter().map(|id| state_fn(id)).collect();
        let settled = |s: &String| s != "working" && s != "starting";
        let done = if any {
            states.iter().any(settled)
        } else {
            states.iter().all(settled)
        };
        if done {
            return (false, states);
        }
        if Instant::now() >= deadline {
            return (true, states);
        }
        std::thread::sleep(poll);
    }
}

fn descendants(app: &AppCtx, parent: &str) -> Result<Vec<Session>, String> {
    let db = app.db();
    let conn = db.conn.lock().unwrap();
    crate::db::repo::live_descendants(&conn, parent)
}

fn session_by_id(app: &AppCtx, sid: &str) -> Option<Session> {
    let db = app.db();
    let conn = db.conn.lock().ok()?;
    crate::db::repo::get_session(&conn, sid).ok().flatten()
}

/// Resolve the request's `target` (session id or unique name) among the parent's live descendants.
fn resolve_target(app: &AppCtx, parent: &str, req: &Value) -> Result<Session, (u16, String)> {
    let Some(target) = str_field(req, "target") else {
        return Err(err(400, "missing target"));
    };
    resolve_named(app, parent, &target)
}

fn resolve_named(app: &AppCtx, parent: &str, target: &str) -> Result<Session, (u16, String)> {
    let sessions = descendants(app, parent).map_err(|e| err(500, &e))?;
    if let Some(s) = sessions.iter().find(|s| s.id == target) {
        return Ok(s.clone());
    }
    let by_name: Vec<&Session> = sessions.iter().filter(|s| s.name == target).collect();
    match by_name.as_slice() {
        [] => Err(err(
            404,
            &format!("no live child session matches \"{target}\""),
        )),
        [one] => Ok((*one).clone()),
        many => {
            let ids: Vec<&str> = many.iter().map(|s| s.id.as_str()).collect();
            Err(err(
                409,
                &format!("name \"{target}\" is ambiguous; use an id: {}", ids.join(", ")),
            ))
        }
    }
}

fn session_row(app: &AppCtx, s: &Session) -> Value {
    let outcome = turn_outcome_of(s);
    json!({
        "id": s.id,
        "name": s.name,
        "kind": s.kind.as_str(),
        "model": s.model,
        "effort": s.effort,
        "state": state_of(app, &s.id),
        "lastTurnOutcome": outcome.outcome,
        "lastTurnError": outcome.error,
        "worktreePath": s.worktree_path,
        "parentSessionId": s.parent_session_id,
    })
}

fn turn_outcome_of(s: &Session) -> crate::agent::transcript::TurnOutcomeInfo {
    s.agent_session_id
        .as_deref()
        .map(|id| crate::agent::transcript::last_turn_outcome(s.kind, id))
        .unwrap_or_default()
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn ok(body: Value) -> (u16, String) {
    (200, body.to_string())
}

fn err(status: u16, message: &str) -> (u16, String) {
    (status, json!({ "error": message }).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::host::HeadlessHost;
    use std::sync::Arc;

    fn headless_app() -> AppCtx {
        let path = std::env::temp_dir().join(format!(
            "vlx-ctl-{}-{:?}.db",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
        let db = Db::open(&path).unwrap();
        AppCtx::Headless(Arc::new(HeadlessHost::new(std::env::temp_dir(), db)))
    }

    fn seed(app: &AppCtx, name: &str, parent: Option<&str>) -> Session {
        let db = app.db();
        let conn = db.conn.lock().unwrap();
        let project =
            crate::db::repo::import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        crate::db::repo::create_session_full(
            &conn,
            &project.id,
            None,
            name,
            SessionKind::Claude,
            None,
            None,
            None,
            parent,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap()
    }

    fn git(dir: &std::path::Path, args: &[&str]) -> String {
        let output = crate::host::command("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn init_repo() -> std::path::PathBuf {
        let repo = std::env::temp_dir().join(format!("vlx-ctl-git-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q"]);
        git(&repo, &["config", "user.email", "t@example.com"]);
        git(&repo, &["config", "user.name", "tester"]);
        git(&repo, &["config", "commit.gpgsign", "false"]);
        std::fs::write(repo.join("a.txt"), "base\n").unwrap();
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "init"]);
        git(&repo, &["branch", "-M", "main"]);
        repo
    }

    fn seed_worktree(
        app: &AppCtx,
        parent: &Session,
        name: &str,
        worktree: &crate::git::WorktreeInfo,
    ) -> Session {
        let db = app.db();
        let conn = db.conn.lock().unwrap();
        let project =
            crate::db::repo::import_project(&conn, std::env::temp_dir().to_str().unwrap()).unwrap();
        crate::db::repo::create_session_full(
            &conn,
            &project.id,
            None,
            name,
            SessionKind::Claude,
            None,
            Some(&worktree.path),
            None,
            Some(&parent.id),
            Some(&worktree.path),
            None,
            None,
            Some(&worktree.base_ref),
            None,
            None,
        )
        .unwrap()
    }

    fn set_settings(app: &AppCtx, json: &str) {
        let db = app.db();
        let conn = db.conn.lock().unwrap();
        crate::db::repo::set_app_settings(
            &conn,
            &HashMap::from([("vlx-settings".to_string(), json.to_string())]),
        )
        .unwrap();
    }

    /// Collect every emitted `spawn://request` payload so a test can inspect what the frontend
    /// would receive without completing the spawn.
    fn capture_spawns(app: &AppCtx) -> Arc<Mutex<Vec<Value>>> {
        let seen: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        app.listen("spawn://request", move |payload| {
            if let Ok(v) = serde_json::from_str::<Value>(payload) {
                sink.lock().unwrap().push(v);
            }
        });
        seen
    }

    #[test]
    fn spawn_registry_roundtrip_and_timeout() {
        let rx = register_spawn_waiter("req-1", "parent-1");
        assert!(resolve_spawn(
            "req-1",
            SpawnOutcome {
                session_id: Some("s1".into()),
                ..Default::default()
            }
        ));
        assert_eq!(rx.recv().unwrap().session_id.as_deref(), Some("s1"));
        assert!(!resolve_spawn("req-1", SpawnOutcome::default()));
        assert!(!resolve_spawn("unknown", SpawnOutcome::default()));
    }

    #[test]
    fn wait_states_any_vs_all_and_timeout() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = AtomicUsize::new(0);
        let state_fn = |id: &str| -> String {
            if id == "a" {
                "working".to_string()
            } else if calls.fetch_add(1, Ordering::SeqCst) >= 2 {
                "waiting".to_string()
            } else {
                "working".to_string()
            }
        };
        let ids = vec!["a".to_string(), "b".to_string()];
        let (timed_out, states) = wait_states(
            &ids,
            true,
            Duration::from_secs(5),
            Duration::from_millis(1),
            state_fn,
        );
        assert!(!timed_out);
        assert_eq!(states, vec!["working", "waiting"]);

        let (timed_out, _) = wait_states(
            &ids,
            false,
            Duration::from_millis(10),
            Duration::from_millis(1),
            |_| "working".to_string(),
        );
        assert!(timed_out);
    }

    #[test]
    fn blocked_targets_separates_a_permission_prompt_from_a_finished_turn() {
        let ids: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let states: Vec<String> = ["waiting", "asking", "exited"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(blocked_targets(&ids, &states), vec![&ids[1]]);

        let states: Vec<String> = ["waiting", "exited", "not-started"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(blocked_targets(&ids, &states).is_empty());
    }

    #[test]
    fn failed_targets_reports_provider_text_and_excludes_ok_and_unknown() {
        use crate::agent::transcript::{TurnOutcome, TurnOutcomeInfo};

        let ids: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let outcomes = vec![
            TurnOutcomeInfo {
                outcome: TurnOutcome::Error,
                error: Some("provider rejected the model".to_string()),
            },
            TurnOutcomeInfo {
                outcome: TurnOutcome::Ok,
                error: None,
            },
            TurnOutcomeInfo::default(),
        ];
        let failed = failed_targets(&ids, &outcomes, |id| format!("worker-{id}"));
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0]["id"], "a");
        assert_eq!(failed[0]["name"], "worker-a");
        assert_eq!(failed[0]["error"], "provider rejected the model");
    }

    #[test]
    fn resolve_scopes_to_descendants_and_reports_ambiguity() {
        let app = headless_app();
        let root = seed(&app, "root", None);
        let child = seed(&app, "worker", Some(&root.id));
        let _dup = seed(&app, "worker", Some(&root.id));
        let outsider = seed(&app, "outsider", None);

        assert_eq!(resolve_named(&app, &root.id, &child.id).unwrap().id, child.id);
        assert_eq!(resolve_named(&app, &root.id, &outsider.id).unwrap_err().0, 404);
        let (status, body) = resolve_named(&app, &root.id, "worker").unwrap_err();
        assert_eq!(status, 409);
        assert!(body.contains(&child.id));

        let request = format!(
            r#"{{"parentSessionId":"{}","target":"{}"}}"#,
            root.id, outsider.id
        );
        assert_eq!(handle("diff", &request, &app).0, 404);
        assert_eq!(handle("merge", &request, &app).0, 404);
    }

    #[test]
    fn handle_list_and_status_report_not_started() {
        let app = headless_app();
        let root = seed(&app, "root", None);
        let child = seed(&app, "worker", Some(&root.id));

        let (status, body) =
            handle("list", &format!(r#"{{"parentSessionId":"{}"}}"#, root.id), &app);
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["sessions"][0]["id"], child.id.as_str());
        assert_eq!(v["sessions"][0]["state"], "not-started");
        assert_eq!(v["sessions"][0]["lastTurnOutcome"], "unknown");

        let (status, body) = handle(
            "status",
            &format!(r#"{{"parentSessionId":"{}","target":"worker"}}"#, root.id),
            &app,
        );
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["kind"], "claude");
        assert_eq!(v["lastTurnOutcome"], "unknown");

        let with_parent = format!(r#"{{"parentSessionId":"{}"}}"#, root.id);
        assert_eq!(handle("bogus", &with_parent, &app).0, 404);
        assert_eq!(handle("list", "not json", &app).0, 400);
        assert_eq!(handle("list", "{}", &app).0, 400);
    }

    #[test]
    fn diff_and_merge_round_trip_removes_the_worktree() {
        let repo = init_repo();
        let repo_str = repo.to_string_lossy().to_string();
        let worktree = crate::git::worktree_add(&repo_str, "round trip").unwrap();
        std::fs::write(
            std::path::Path::new(&worktree.path).join("worker.txt"),
            "worker change\n",
        )
        .unwrap();
        git(std::path::Path::new(&worktree.path), &["add", "-A"]);
        git(
            std::path::Path::new(&worktree.path),
            &["commit", "-q", "-m", "worker change"],
        );

        let app = headless_app();
        let root = seed(&app, "root", None);
        let child = seed_worktree(&app, &root, "worker", &worktree);
        let request = format!(r#"{{"parentSessionId":"{}","target":"worker"}}"#, root.id);

        let (status, body) = handle("diff", &request, &app);
        assert_eq!(status, 200);
        let diff: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(diff["id"], child.id);
        assert_eq!(diff["baseRef"], "refs/heads/main");
        assert_eq!(diff["baseBranch"], "main");
        assert_eq!(diff["branch"], worktree.branch);
        assert!(diff["diffStat"].as_str().unwrap().contains("worker.txt"));
        let patch = diff["patch"].as_str().unwrap();
        assert!(patch.contains("diff --git a/worker.txt b/worker.txt"));
        assert!(patch.contains("+worker change"));
        assert_eq!(diff["truncated"], false);
        assert_eq!(diff["hasUncommitted"], false);

        let merge_request = format!(
            r#"{{"parentSessionId":"{}","target":"worker","deleteWorktree":true}}"#,
            root.id
        );
        let (status, body) = handle("merge", &merge_request, &app);
        assert_eq!(status, 200);
        let merged: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(merged["merged"], true);
        assert_eq!(merged["source"], worktree.branch);
        assert_eq!(merged["target"], "main");
        assert_eq!(merged["worktreeDeleted"], true);
        assert!(repo.join("worker.txt").is_file());
        assert!(!std::path::Path::new(&worktree.path).exists());
        assert_eq!(session_by_id(&app, &child.id).unwrap().worktree_path, None);

        std::fs::remove_dir_all(&repo).unwrap();
    }

    #[test]
    fn diff_caps_large_patches() {
        let repo = init_repo();
        let repo_str = repo.to_string_lossy().to_string();
        let worktree = crate::git::worktree_add(&repo_str, "large diff").unwrap();
        let worktree_path = std::path::Path::new(&worktree.path);
        std::fs::write(worktree_path.join("large.txt"), "review line\n".repeat(30_000)).unwrap();
        git(worktree_path, &["add", "-A"]);
        git(worktree_path, &["commit", "-q", "-m", "large change"]);

        let app = headless_app();
        let root = seed(&app, "root", None);
        seed_worktree(&app, &root, "worker", &worktree);
        let request = format!(r#"{{"parentSessionId":"{}","target":"worker"}}"#, root.id);

        let (status, body) = handle("diff", &request, &app);
        assert_eq!(status, 200);
        let diff: Value = serde_json::from_str(&body).unwrap();
        assert!(diff["patch"].as_str().unwrap().len() <= 256 * 1024);
        assert_eq!(diff["truncated"], true);

        crate::git::worktree_remove(&worktree.path, true).unwrap();
        std::fs::remove_dir_all(&repo).unwrap();
    }

    #[test]
    fn merge_conflict_reports_files_and_preserves_branch_heads() {
        let repo = init_repo();
        let repo_str = repo.to_string_lossy().to_string();
        let worktree = crate::git::worktree_add(&repo_str, "conflict").unwrap();
        let worktree_path = std::path::Path::new(&worktree.path);
        std::fs::write(worktree_path.join("a.txt"), "from worker\n").unwrap();
        git(worktree_path, &["commit", "-q", "-am", "worker edit"]);
        std::fs::write(repo.join("a.txt"), "from parent\n").unwrap();
        git(&repo, &["commit", "-q", "-am", "parent edit"]);
        let source_head = git(&repo, &["rev-parse", &worktree.branch]);
        let target_head = git(&repo, &["rev-parse", "main"]);

        let app = headless_app();
        let root = seed(&app, "root", None);
        seed_worktree(&app, &root, "worker", &worktree);
        let request = format!(r#"{{"parentSessionId":"{}","target":"worker"}}"#, root.id);
        let (status, body) = handle("merge", &request, &app);
        assert_eq!(status, 409);
        let conflict: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(conflict["error"], "merge conflict");
        assert_eq!(conflict["source"], worktree.branch);
        assert_eq!(conflict["target"], "main");
        assert!(conflict["conflicts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path == "a.txt"));
        assert_eq!(git(&repo, &["rev-parse", &worktree.branch]), source_head);
        assert_eq!(git(&repo, &["rev-parse", "main"]), target_head);

        git(&repo, &["merge", "--abort"]);
        crate::git::worktree_remove(&worktree.path, true).unwrap();
        std::fs::remove_dir_all(&repo).unwrap();
    }

    #[test]
    fn merge_refuses_uncommitted_worker_changes() {
        let repo = init_repo();
        let repo_str = repo.to_string_lossy().to_string();
        let worktree = crate::git::worktree_add(&repo_str, "dirty").unwrap();
        std::fs::write(
            std::path::Path::new(&worktree.path).join("dirty.txt"),
            "not committed\n",
        )
        .unwrap();
        let source_head = git(&repo, &["rev-parse", &worktree.branch]);
        let target_head = git(&repo, &["rev-parse", "main"]);

        let app = headless_app();
        let root = seed(&app, "root", None);
        seed_worktree(&app, &root, "worker", &worktree);
        let request = format!(r#"{{"parentSessionId":"{}","target":"worker"}}"#, root.id);
        let (status, body) = handle("merge", &request, &app);
        assert_eq!(status, 409);
        let refused: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(refused["error"], "worktree has uncommitted changes");
        assert_eq!(refused["branch"], worktree.branch);
        assert_eq!(git(&repo, &["rev-parse", &worktree.branch]), source_head);
        assert_eq!(git(&repo, &["rev-parse", "main"]), target_head);
        assert!(std::path::Path::new(&worktree.path)
            .join("dirty.txt")
            .is_file());

        crate::git::worktree_remove(&worktree.path, true).unwrap();
        std::fs::remove_dir_all(&repo).unwrap();
    }

    #[test]
    fn handle_prompt_and_cancel_require_running_session() {
        let app = headless_app();
        let root = seed(&app, "root", None);
        let child = seed(&app, "worker", Some(&root.id));
        let body = format!(
            r#"{{"parentSessionId":"{}","target":"{}","text":"hi"}}"#,
            root.id, child.id
        );
        assert_eq!(handle("prompt", &body, &app).0, 409);
        assert_eq!(handle("cancel", &body, &app).0, 409);
    }

    #[test]
    fn handle_spawn_resolves_through_registry() {
        let app = headless_app();
        let root = seed(&app, "root", None);
        let child = seed(&app, "spawned", Some(&root.id));

        let emitted = capture_spawns(&app);
        let child_id = child.id.clone();
        let resolver = std::thread::spawn(move || loop {
            let request_id = emitted
                .lock()
                .unwrap()
                .first()
                .and_then(|v| v["requestId"].as_str().map(str::to_string));
            if let Some(id) = request_id {
                resolve_spawn(
                    &id,
                    SpawnOutcome {
                        session_id: Some(child_id.clone()),
                        worktree_error: Some("not a git repository".into()),
                        ..Default::default()
                    },
                );
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        });
        let (status, body) = handle(
            "spawn",
            &format!(
                r#"{{"parentSessionId":"{}","prompt":"do work","timeoutSecs":5}}"#,
                root.id
            ),
            &app,
        );
        resolver.join().unwrap();
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["id"], child.id.as_str());
        assert_eq!(v["worktreeError"], "not a git repository");

        let (status, body) = handle(
            "spawn",
            &format!(
                r#"{{"parentSessionId":"{}","prompt":"slow","timeoutSecs":0}}"#,
                root.id
            ),
            &app,
        );
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["pending"], true);
    }

    #[test]
    fn spawn_rejects_an_unknown_profile() {
        let app = headless_app();
        let root = seed(&app, "root", None);
        let (status, body) = handle(
            "spawn",
            &format!(
                r#"{{"parentSessionId":"{}","prompt":"work","profile":"fast"}}"#,
                root.id
            ),
            &app,
        );
        assert_eq!(status, 400);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["error"], "unknown profile \"fast\"");
        assert_eq!(
            v["available"],
            json!(["database", "frontend", "quick-edits", "tests"])
        );
    }

    #[test]
    fn spawn_enforces_descendant_and_depth_limits() {
        let app = headless_app();
        set_settings(&app, r#"{"orchestration":{"maxDescendants":2,"maxDepth":2}}"#);
        let root = seed(&app, "root", None);
        let mid = seed(&app, "mid", Some(&root.id));
        let leaf = seed(&app, "leaf", Some(&mid.id));

        let (status, body) = handle(
            "spawn",
            &format!(r#"{{"parentSessionId":"{}","prompt":"work"}}"#, root.id),
            &app,
        );
        assert_eq!(status, 429);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["limit"], "max_descendants");
        assert_eq!(v["limitValue"], 2);
        assert_eq!(v["current"], 2);
        assert_eq!(
            v["error"],
            "max_descendants limit reached (2 of 2 retained descendants). Waiting does not free a slot; archive or remove a finished session."
        );

        let (status, body) = handle(
            "spawn",
            &format!(r#"{{"parentSessionId":"{}","prompt":"work"}}"#, leaf.id),
            &app,
        );
        assert_eq!(status, 429);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["limit"], "max_depth");
        assert_eq!(v["current"], 3);

        let (status, body) = handle(
            "spawn",
            &format!(
                r#"{{"parentSessionId":"{}","prompt":"work","timeoutSecs":0}}"#,
                mid.id
            ),
            &app,
        );
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["pending"], true);
    }

    #[test]
    fn spawn_counts_a_not_started_child_against_max_parallel() {
        let app = headless_app();
        set_settings(&app, r#"{"orchestration":{"maxParallel":1}}"#);
        let root = seed(&app, "root", None);
        let _worker = seed(&app, "worker", Some(&root.id));

        let (status, body) = handle(
            "spawn",
            &format!(r#"{{"parentSessionId":"{}","prompt":"work"}}"#, root.id),
            &app,
        );
        assert_eq!(status, 429);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["limit"], "max_parallel");
        assert_eq!(v["limitValue"], 1);
        assert_eq!(v["current"], 1);
        assert_eq!(v["error"], "max_parallel limit reached (1 of 1 active children)");
    }

    #[test]
    fn consumes_parallel_slot_covers_every_state() {
        for state in ["starting", "working", "asking"] {
            assert!(
                consumes_parallel_slot(state, 10_000),
                "{state} must hold a slot at any age"
            );
        }
        assert!(consumes_parallel_slot("not-started", 0));
        assert!(consumes_parallel_slot("not-started", SPAWN_DEFAULT_TIMEOUT_SECS - 1));
        assert!(
            consumes_parallel_slot("not-started", -5),
            "a negative age must stay inside the window"
        );
        assert!(!consumes_parallel_slot("not-started", SPAWN_DEFAULT_TIMEOUT_SECS));
        for state in ["waiting", "exited"] {
            assert!(!consumes_parallel_slot(state, 0), "{state} must not hold a slot");
        }
    }

    #[test]
    fn an_old_not_started_child_does_not_hold_a_parallel_slot() {
        let app = headless_app();
        set_settings(&app, r#"{"orchestration":{"maxParallel":1}}"#);
        let root = seed(&app, "root", None);
        let stale = seed(&app, "stale", Some(&root.id));
        {
            let db = app.db();
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "UPDATE sessions SET created_at = created_at - 3600 WHERE id = ?1",
                [stale.id.as_str()],
            )
            .unwrap();
        }

        let (status, body) = handle(
            "spawn",
            &format!(
                r#"{{"parentSessionId":"{}","prompt":"work","timeoutSecs":0}}"#,
                root.id
            ),
            &app,
        );
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["pending"], true);
    }

    #[test]
    fn spawn_status_answers_a_foreign_parent_with_the_unknown_id_404() {
        let app = headless_app();
        let root = seed(&app, "root", None);
        let other = seed(&app, "other", None);
        let child = seed(&app, "spawned", Some(&root.id));
        let ask = |parent: &str, request_id: &str| {
            handle(
                "spawn-status",
                &format!(r#"{{"parentSessionId":"{parent}","requestId":"{request_id}"}}"#),
                &app,
            )
        };

        let _rx = register_spawn_waiter("req-scope", &root.id);
        let (_, unknown_body) = ask(&other.id, "req-nonexistent");
        let (status, body) = ask(&other.id, "req-scope");
        assert_eq!(status, 404);
        assert_eq!(body, unknown_body);
        let (status, body) = ask(&root.id, "req-scope");
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["pending"], true);

        resolve_spawn(
            "req-scope",
            SpawnOutcome {
                session_id: Some(child.id.clone()),
                ..Default::default()
            },
        );
        let (status, body) = ask(&other.id, "req-scope");
        assert_eq!(status, 404);
        assert_eq!(body, unknown_body, "a mismatch must not consume the outcome");
        let (status, body) = ask(&root.id, "req-scope");
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["id"], child.id.as_str());
    }

    #[test]
    fn spawn_applies_profile_values_and_confirmation_threshold() {
        let app = headless_app();
        set_settings(
            &app,
            r#"{"orchestrationProfiles":{"quick":{"agent":"codex","model":"gpt-5.6-luna","effort":"xhigh","worktree":true,"permissionMode":"skip"}},
                "orchestration":{"requireConfirmationAbove":1,"autoApprove":true}}"#,
        );
        let root = seed(&app, "root", None);
        let _a = seed(&app, "a", Some(&root.id));
        let _b = seed(&app, "b", Some(&root.id));
        let emitted = capture_spawns(&app);

        let (status, _) = handle(
            "spawn",
            &format!(
                r#"{{"parentSessionId":"{}","prompt":"work","profile":"quick","model":"gpt-5.6-sol","timeoutSecs":0}}"#,
                root.id
            ),
            &app,
        );
        assert_eq!(status, 200);
        let req = emitted.lock().unwrap()[0].clone();
        assert_eq!(req["kind"], "codex");
        assert_eq!(req["model"], "gpt-5.6-sol");
        assert_eq!(req["effort"], "xhigh");
        assert_eq!(req["worktree"], true);
        assert_eq!(req["permissionMode"], "skip");
        assert_eq!(req["forceConfirm"], true);
        assert_eq!(req["autoApprove"], false);

        set_settings(
            &app,
            r#"{"orchestration":{"requireConfirmationAbove":10,"autoApprove":true}}"#,
        );
        let (status, _) = handle(
            "spawn",
            &format!(
                r#"{{"parentSessionId":"{}","prompt":"work","timeoutSecs":0}}"#,
                root.id
            ),
            &app,
        );
        assert_eq!(status, 200);
        let req = emitted.lock().unwrap()[1].clone();
        assert_eq!(req["kind"], Value::Null);
        assert_eq!(req["model"], Value::Null);
        assert_eq!(req["worktree"], false);
        assert_eq!(req["forceConfirm"], Value::Null);
        assert_eq!(req["autoApprove"], true);
    }

    #[test]
    fn config_reports_profiles_limits_and_counts() {
        let app = headless_app();
        set_settings(
            &app,
            r#"{"orchestrationProfiles":{"quick":{"description":"Use for quick fixes.","agent":"codex"}},"orchestration":{"maxParallel":2}}"#,
        );
        let root = seed(&app, "root", None);
        let mid = seed(&app, "mid", Some(&root.id));
        let _leaf = seed(&app, "leaf", Some(&mid.id));

        let (status, body) = handle(
            "config",
            &format!(r#"{{"parentSessionId":"{}"}}"#, mid.id),
            &app,
        );
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["profiles"]["quick"]["description"], "Use for quick fixes.");
        assert_eq!(v["profiles"]["quick"]["agent"], "codex");
        assert_eq!(v["limits"]["maxParallel"], 2);
        assert_eq!(v["limits"]["maxDescendants"], 10);
        assert_eq!(v["limits"]["worktreeCopyPatterns"][0], "docs/plans/**");
        assert_eq!(v["counts"]["descendants"], 1);
        assert_eq!(v["counts"]["active"], 1);
        assert_eq!(v["counts"]["depth"], 1);
        assert_eq!(v["counts"].get("children"), None);
        assert_eq!(v["counts"].get("working"), None);
    }

    #[test]
    fn cancel_all_reports_every_descendant() {
        let app = headless_app();
        let root = seed(&app, "root", None);
        let child = seed(&app, "worker", Some(&root.id));

        let (status, body) = handle(
            "cancel",
            &format!(r#"{{"parentSessionId":"{}","all":true}}"#, root.id),
            &app,
        );
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["interrupted"], json!([]));
        assert_eq!(v["skipped"], json!([child.id]));
    }

    #[test]
    fn wait_default_timeout_comes_from_settings() {
        let app = headless_app();
        set_settings(&app, r#"{"orchestration":{"defaultTimeoutSecs":0}}"#);
        let root = seed(&app, "root", None);
        let child = seed(&app, "worker", Some(&root.id));

        let (status, body) = handle(
            "wait",
            &format!(
                r#"{{"parentSessionId":"{}","targets":["{}"]}}"#,
                root.id, child.id
            ),
            &app,
        );
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["timedOut"], false);
        assert!(v["failed"].as_array().unwrap().is_empty());
        assert_eq!(v["sessions"][0]["state"], "not-started");
        assert_eq!(v["sessions"][0]["lastTurnOutcome"], "unknown");
    }

    #[test]
    fn handle_cleanup_offers_then_removes_clean_worktrees() {
        let repo = std::env::temp_dir().join(format!("vlx-cleanup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&repo).unwrap();
        let repo_str = repo.to_string_lossy().to_string();
        let git = |args: &[&str]| {
            crate::host::command("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .output()
                .unwrap();
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "tester"]);
        std::fs::write(repo.join("a.txt"), "hello\n").unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "init"]);

        let clean = crate::git::worktree_add(&repo_str, "clean").unwrap();
        let dirty = crate::git::worktree_add(&repo_str, "dirty").unwrap();
        std::fs::write(
            std::path::Path::new(&dirty.path).join("a.txt"),
            "uncommitted\n",
        )
        .unwrap();

        let app = headless_app();
        let root = seed(&app, "root", None);
        let mk = |name: &str, path: &str| {
            let db = app.db();
            let conn = db.conn.lock().unwrap();
            let project =
                crate::db::repo::import_project(&conn, std::env::temp_dir().to_str().unwrap())
                    .unwrap();
            crate::db::repo::create_session_full(
                &conn,
                &project.id,
                None,
                name,
                SessionKind::Claude,
                None,
                None,
                None,
                Some(root.id.as_str()),
                Some(path),
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap()
        };
        let clean_session = mk("clean-worker", &clean.path);
        mk("dirty-worker", &dirty.path);
        seed(&app, "no-worktree", Some(&root.id));

        let body = format!(r#"{{"parentSessionId":"{}"}}"#, root.id);
        let (status, out) = handle("cleanup", &body, &app);
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["candidates"].as_array().unwrap().len(), 1);
        assert_eq!(v["candidates"][0]["name"], "clean-worker");
        assert_eq!(v["blocked"].as_array().unwrap().len(), 1);
        assert_eq!(v["blocked"][0]["name"], "dirty-worker");
        assert!(
            std::path::Path::new(&clean.path).is_dir(),
            "a dry run must not remove anything"
        );

        let (status, out) = handle(
            "cleanup",
            &format!(r#"{{"parentSessionId":"{}","confirm":true}}"#, root.id),
            &app,
        );
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["removed"].as_array().unwrap().len(), 1);
        assert_eq!(v["removed"][0]["name"], "clean-worker");
        assert!(v["failed"].as_array().unwrap().is_empty());
        assert!(!std::path::Path::new(&clean.path).exists());
        assert!(
            std::path::Path::new(&dirty.path).is_dir(),
            "a worktree with uncommitted changes must survive"
        );

        let stored = session_by_id(&app, &clean_session.id).unwrap();
        assert_eq!(stored.worktree_path, None);

        std::fs::remove_dir_all(&repo).unwrap();
    }

    #[test]
    fn launch_value_check_warns_without_rejecting_unknown_values() {
        let warnings = launch_value_warnings(SessionKind::Claude, Some("opus-5"), None);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("opus-5"));

        assert!(launch_value_warnings(SessionKind::Claude, Some("opus"), Some("high")).is_empty());

        let warnings = launch_value_warnings(SessionKind::Claude, Some("opus"), Some("xhigh"));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("effort"));
        assert!(launch_value_warnings(
            SessionKind::Codex,
            Some("gpt-5.6-luna"),
            Some("xhigh")
        )
        .is_empty());
        assert_eq!(
            launch_value_warnings(SessionKind::Codex, Some("gpt-5.6-luna"), Some("ultra")).len(),
            1
        );

        assert!(launch_value_warnings(SessionKind::Grok, Some("whatever"), Some("nonsense")).is_empty());
        assert!(launch_value_warnings(SessionKind::Claude, None, None).is_empty());
    }

    #[test]
    fn spawn_warns_about_an_unknown_model_and_reaches_the_confirmation_card() {
        let app = headless_app();
        let root = seed(&app, "lead", None);
        let emitted = capture_spawns(&app);

        let (status, out) = handle(
            "spawn",
            &format!(
                r#"{{"parentSessionId":"{}","prompt":"work","kind":"claude","model":"opus-5","timeoutSecs":0}}"#,
                root.id
            ),
            &app,
        );
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["pending"], true);
        let request = emitted.lock().unwrap().first().cloned().unwrap();
        assert_eq!(request["forceConfirm"], true);
        assert_eq!(request["launchWarnings"].as_array().unwrap().len(), 1);

        let (status, out) = handle(
            "spawn",
            &format!(
                r#"{{"parentSessionId":"{}","prompt":"work","kind":"claude","model":"opus-9","allowUnknownLaunchValues":true,"timeoutSecs":0}}"#,
                root.id
            ),
            &app,
        );
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["pending"], true);
    }

    #[test]
    fn spawn_returns_pending_on_a_confirmation_card_and_the_child_is_collectable_later() {
        let app = headless_app();
        let root = seed(&app, "root", None);
        let child = seed(&app, "confirmed", Some(&root.id));

        let emitted = capture_spawns(&app);
        let seen = std::sync::Arc::new(Mutex::new(String::new()));
        let seen_for_thread = std::sync::Arc::clone(&seen);
        let signaller = std::thread::spawn(move || loop {
            let request_id = emitted
                .lock()
                .unwrap()
                .first()
                .and_then(|v| v["requestId"].as_str().map(str::to_string));
            if let Some(id) = request_id {
                *seen_for_thread.lock().unwrap() = id.clone();
                resolve_spawn(
                    &id,
                    SpawnOutcome {
                        awaiting_confirmation: true,
                        ..Default::default()
                    },
                );
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        });

        let (status, body) = handle(
            "spawn",
            &format!(
                r#"{{"parentSessionId":"{}","prompt":"do work","timeoutSecs":600}}"#,
                root.id
            ),
            &app,
        );
        signaller.join().unwrap();
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["pending"], true);
        assert_eq!(v["awaitingConfirmation"], true);
        let request_id = v["requestId"].as_str().unwrap().to_string();
        assert_eq!(request_id, *seen.lock().unwrap());

        let (status, body) = handle(
            "spawn-status",
            &format!(r#"{{"parentSessionId":"{}","requestId":"{request_id}"}}"#, root.id),
            &app,
        );
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["pending"], true);

        resolve_spawn(
            &request_id,
            SpawnOutcome {
                session_id: Some(child.id.clone()),
                ..Default::default()
            },
        );
        let (status, body) = handle(
            "spawn-status",
            &format!(r#"{{"parentSessionId":"{}","requestId":"{request_id}"}}"#, root.id),
            &app,
        );
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["id"], child.id.as_str());

        let (status, _) = handle(
            "spawn-status",
            &format!(r#"{{"parentSessionId":"{}","requestId":"{request_id}"}}"#, root.id),
            &app,
        );
        assert_eq!(status, 404);
    }

    #[test]
    fn spawn_carries_an_explicit_permission_mode_and_rejects_an_unknown_one() {
        let app = headless_app();
        let root = seed(&app, "lead", None);
        let emitted = capture_spawns(&app);

        let (status, _) = handle(
            "spawn",
            &format!(
                r#"{{"parentSessionId":"{}","prompt":"work","permissionMode":"skip","timeoutSecs":0}}"#,
                root.id
            ),
            &app,
        );
        assert_eq!(status, 200);
        assert_eq!(emitted.lock().unwrap()[0]["permissionMode"], "skip");

        let (status, _) = handle(
            "spawn",
            &format!(
                r#"{{"parentSessionId":"{}","prompt":"work","timeoutSecs":0}}"#,
                root.id
            ),
            &app,
        );
        assert_eq!(status, 200);
        assert!(emitted.lock().unwrap()[1]["permissionMode"].is_null());

        let (status, out) = handle(
            "spawn",
            &format!(
                r#"{{"parentSessionId":"{}","prompt":"work","permissionMode":"yolo"}}"#,
                root.id
            ),
            &app,
        );
        assert_eq!(status, 400);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["field"], "permissionMode");
    }

    /// Keep the backend launch-value lists aligned with the frontend dropdowns.
    #[test]
    fn known_launch_values_match_the_spawn_card() {
        let source = include_str!("../../../src/components/SpawnConfirmModal.tsx");
        let list = |name: &str, agent: &str| -> Vec<String> {
            let block = source
                .split_once(name)
                .unwrap_or_else(|| panic!("{name} should exist in SpawnConfirmModal.tsx"))
                .1;
            let block = block.split_once("};").expect("the record should be closed").0;
            let line = block
                .lines()
                .find(|l| l.trim_start().starts_with(agent))
                .unwrap_or_else(|| panic!("{name} should list {agent}"));
            line.split('"')
                .skip(1)
                .step_by(2)
                .map(str::to_string)
                .collect()
        };
        for (agent, kind) in [("claude:", SessionKind::Claude), ("codex:", SessionKind::Codex)] {
            assert_eq!(
                list("const MODEL_OPTIONS", agent),
                inject::known_models(kind).unwrap(),
                "MODEL_OPTIONS for {agent} must match known_models"
            );
            assert_eq!(
                list("const EFFORT_OPTIONS", agent),
                inject::known_efforts(kind).unwrap(),
                "EFFORT_OPTIONS for {agent} must match known_efforts"
            );
        }
    }

    #[test]
    fn codex_launch_values_use_current_chatgpt_identifiers() {
        assert_eq!(
            inject::known_models(SessionKind::Codex).unwrap(),
            &["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"]
        );
        assert_eq!(
            inject::known_efforts(SessionKind::Codex).unwrap(),
            &["low", "medium", "high", "xhigh", "max"]
        );
    }
}
