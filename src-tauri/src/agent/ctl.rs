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

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::host::AppCtx;
use crate::models::Session;

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
}

static SPAWN_WAITERS: LazyLock<Mutex<HashMap<String, mpsc::Sender<SpawnOutcome>>>> =
    LazyLock::new(Default::default);

/// Register a spawn correlation id and return the receiver its outcome arrives on.
pub fn register_spawn_waiter(request_id: &str) -> mpsc::Receiver<SpawnOutcome> {
    let (tx, rx) = mpsc::channel();
    SPAWN_WAITERS
        .lock()
        .unwrap()
        .insert(request_id.to_string(), tx);
    rx
}

/// Deliver a spawn outcome to its parked handler. False when the waiter already timed out or the
/// id is unknown; the frontend treats that as informational.
pub fn resolve_spawn(request_id: &str, outcome: SpawnOutcome) -> bool {
    let waiter = SPAWN_WAITERS.lock().unwrap().remove(request_id);
    match waiter {
        Some(tx) => tx.send(outcome).is_ok(),
        None => false,
    }
}

fn drop_spawn_waiter(request_id: &str) {
    SPAWN_WAITERS.lock().unwrap().remove(request_id);
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
        "spawn" => op_spawn(app, &parent, &parsed),
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
        .unwrap_or(1800);

    let app2 = app.clone();
    let (timed_out, states) = wait_states(
        &ids,
        any,
        Duration::from_secs(timeout),
        Duration::from_millis(500),
        move |sid| state_of(&app2, sid),
    );
    let rows: Vec<Value> = ids
        .iter()
        .zip(states)
        .map(|(id, state)| json!({ "id": id, "state": state }))
        .collect();
    ok(json!({ "timedOut": timed_out, "sessions": rows }))
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
    if req.get("full").and_then(Value::as_bool) == Some(true) {
        let rows: Vec<Value> = messages
            .iter()
            .map(|m| json!({ "role": m.role, "text": m.text, "timestamp": m.timestamp }))
            .collect();
        return ok(json!({ "id": session.id, "messages": rows }));
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
    let request_id = uuid::Uuid::new_v4().to_string();
    let rx = register_spawn_waiter(&request_id);
    let spawn_req = crate::agent::server::SpawnRequest {
        parent_session_id: parent.to_string(),
        prompt,
        kind: str_field(req, "kind"),
        worktree: req.get("worktree").and_then(Value::as_bool),
        model: str_field(req, "model"),
        effort: str_field(req, "effort"),
        name: str_field(req, "name"),
        agent_args: str_field(req, "agentArgs"),
        request_id: Some(request_id.clone()),
    };
    app.emit("spawn://request", spawn_req);

    let timeout = req
        .get("timeoutSecs")
        .and_then(Value::as_u64)
        .unwrap_or(120);
    match rx.recv_timeout(Duration::from_secs(timeout)) {
        Ok(outcome) => match outcome.session_id {
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
        },
        Err(_) => {
            // Confirmation may still be open; a later spawn_result finds no waiter and is dropped.
            drop_spawn_waiter(&request_id);
            ok(json!({ "pending": true, "requestId": request_id }))
        }
    }
}

// ── Shared helpers ──

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
    json!({
        "id": s.id,
        "name": s.name,
        "kind": s.kind.as_str(),
        "model": s.model,
        "effort": s.effort,
        "state": state_of(app, &s.id),
        "worktreePath": s.worktree_path,
        "parentSessionId": s.parent_session_id,
    })
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
    use crate::models::SessionKind;
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

    #[test]
    fn spawn_registry_roundtrip_and_timeout() {
        let rx = register_spawn_waiter("req-1");
        assert!(resolve_spawn(
            "req-1",
            SpawnOutcome {
                session_id: Some("s1".into()),
                ..Default::default()
            }
        ));
        assert_eq!(rx.recv().unwrap().session_id.as_deref(), Some("s1"));
        // A resolved or unknown id reports false instead of blocking.
        assert!(!resolve_spawn("req-1", SpawnOutcome::default()));
        assert!(!resolve_spawn("unknown", SpawnOutcome::default()));
    }

    #[test]
    fn wait_states_any_vs_all_and_timeout() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        // b settles after two polls; a stays busy, so all-mode times out and any-mode returns.
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
    fn resolve_scopes_to_descendants_and_reports_ambiguity() {
        let app = headless_app();
        let root = seed(&app, "root", None);
        let child = seed(&app, "worker", Some(&root.id));
        let _dup = seed(&app, "worker", Some(&root.id));
        let outsider = seed(&app, "outsider", None);

        // Unique id resolves; a session outside the subtree does not.
        assert_eq!(resolve_named(&app, &root.id, &child.id).unwrap().id, child.id);
        assert_eq!(resolve_named(&app, &root.id, &outsider.id).unwrap_err().0, 404);
        // A duplicated name is a 409 listing candidates.
        let (status, body) = resolve_named(&app, &root.id, "worker").unwrap_err();
        assert_eq!(status, 409);
        assert!(body.contains(&child.id));
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

        let (status, body) = handle(
            "status",
            &format!(r#"{{"parentSessionId":"{}","target":"worker"}}"#, root.id),
            &app,
        );
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["kind"], "claude");

        // Unknown operations and malformed bodies are structured errors.
        let with_parent = format!(r#"{{"parentSessionId":"{}"}}"#, root.id);
        assert_eq!(handle("bogus", &with_parent, &app).0, 404);
        assert_eq!(handle("list", "not json", &app).0, 400);
        assert_eq!(handle("list", "{}", &app).0, 400);
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

        // Resolve the pending request from a helper thread once it appears in the registry.
        let child_id = child.id.clone();
        let resolver = std::thread::spawn(move || loop {
            // Only claim UUID keys so parallel registry tests with short ids are not disturbed.
            let pending: Vec<String> = SPAWN_WAITERS
                .lock()
                .unwrap()
                .keys()
                .filter(|k| k.len() == 36)
                .cloned()
                .collect();
            if let Some(id) = pending.first() {
                resolve_spawn(
                    id,
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

        // An unresolved spawn returns a pending handle instead of blocking forever.
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
}
