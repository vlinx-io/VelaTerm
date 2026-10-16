//! Durable child-task requests. Clients submit decisions and read receipts; the backend alone
//! creates sessions and dispatches their first task. Retries retain request/session/message identity.

use std::{fs::{File, OpenOptions}, time::{Duration, Instant}};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::{command_core as core, db::repo, git::OwnedWorktree, host::{AppCtx, TREE_CHANGED}, models::{Session, SessionKind}};
use super::{launch_options, permission_catalog, server::SpawnRequest, session_settings};

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS spawn_requests (
 id TEXT PRIMARY KEY, request TEXT NOT NULL, approved TEXT,
 decision TEXT NOT NULL DEFAULT 'pending', state TEXT NOT NULL DEFAULT 'pending',
 session_id TEXT NOT NULL UNIQUE, message_id TEXT NOT NULL UNIQUE,
 launch TEXT, initial_prompt TEXT, error TEXT, created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE TABLE IF NOT EXISTS spawn_worktrees (
 id TEXT PRIMARY KEY, receipt TEXT NOT NULL, session_id TEXT, error TEXT
);
CREATE TABLE IF NOT EXISTS agent_session_creations (
 id TEXT PRIMARY KEY, request TEXT NOT NULL, session_id TEXT NOT NULL
);
";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub request_id: String,
    pub request: SpawnRequest,
    #[serde(default)]
    pub resolved_plan_execute: Option<super::plan_execute::Config>,
    pub decision: String,
    pub state: String,
    pub session_id: String,
    pub message_id: String,
    pub session: Option<Session>,
    pub error: Option<String>,
}

/// OS locks release on process exit, so a crashed creator cannot strand an in-memory claim.
/// Lock files stay in place: unlinking one could let another process lock a different inode.
pub(super) fn request_lock(app: &AppCtx, id: &str) -> Result<File, String> {
    uuid::Uuid::parse_str(id).map_err(|_| "Invalid spawn request identifier")?;
    let dir = app.data_dir()?.join("spawn-locks");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = OpenOptions::new().create(true).truncate(false).read(true).write(true)
        .open(dir.join(format!("{id}.lock"))).map_err(|e| e.to_string())?;
    let started = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) if started.elapsed() < Duration::from_secs(30) => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => return Err(format!("The spawn request is still being processed: {error}")),
        }
    }
}

fn encoded<T: Serialize>(value: &T) -> Result<String, String> { serde_json::to_string(value).map_err(|e| e.to_string()) }
fn decoded<T: for<'a> Deserialize<'a>>(value: &str) -> Result<T, String> { serde_json::from_str(value).map_err(|e| e.to_string()) }
fn settings(conn: &Connection) -> Result<Value, String> {
    match repo::get_app_settings(conn)?.remove("vlx-settings") {
        Some(raw) => decoded(&raw), None => Ok(json!({})),
    }
}
fn active_session(conn: &Connection, id: &str) -> Result<Session, String> {
    repo::get_session(conn, id)?.filter(|s| s.archived_at.is_none()).ok_or_else(|| "The session is missing or archived".into())
}
fn validate_request(conn: &Connection, request: &SpawnRequest) -> Result<(), String> {
    if request.prompt.trim().is_empty() { return Err("A child task needs a prompt".into()); }
    active_session(conn, &request.parent_session_id)?;
    if let Some(kind) = &request.kind { parse_launch_kind(kind)?; }
    core::check_images(&request.images)?;
    if let Some(cwd) = request.cwd.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if !std::path::Path::new(cwd).is_absolute() || !std::path::Path::new(cwd).is_dir() {
            return Err("Select an existing absolute working directory".into());
        }
    }
    Ok(())
}
fn parse_launch_kind(value: &str) -> Result<SessionKind, String> {
    if value == "terminal" { Ok(SessionKind::Terminal) } else { parse_agent(value) }
}
fn parse_agent(value: &str) -> Result<SessionKind, String> {
    let kind: SessionKind = serde_json::from_value(json!(value)).map_err(|_| "Unknown agent kind")?;
    if !launch_options::catalog().iter().any(|item| item.id == kind && item.accepts_task) {
        return Err("Select an agent that can receive a task".into());
    }
    Ok(kind)
}

pub fn read(app: &AppCtx, id: &str) -> Result<Receipt, String> {
    let conn = app.db().conn.lock().unwrap();
    read_conn(&conn, id)
}
fn read_conn(conn: &Connection, id: &str) -> Result<Receipt, String> {
    let (raw, decision, mut state, session_id, message_id, mut error): (String, String, String, String, String, Option<String>) = conn.query_row(
        "SELECT COALESCE(approved,request),decision,state,session_id,message_id,error FROM spawn_requests WHERE id=?1", [id],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)),
    ).map_err(|e| format!("Spawn request not found: {e}"))?;
    let session = repo::get_session(conn, &session_id)?;
    if decision == "confirmed" && session.as_ref().is_some_and(|s| s.engine == "chat") && state != "cancelled" {
        if let Some(outcome) = submission_outcome(conn, &session_id, &message_id)? {
            match serde_json::from_str::<Result<String,String>>(&outcome) {
                Ok(Ok(status)) if status == "queued" => { state = "waiting".into(); error = None; }
                Ok(Ok(_)) => { state = "complete".into(); error = None; }
                Ok(Err(cause)) => { state = "uncertain".into(); error = Some(cause); }
                Err(_) => {
                    let tagged: Value = decoded(&outcome)?;
                    if let Some(cause) = tagged["rejected"].as_str() { state = "failed".into(); error = Some(cause.into()); }
                    else { state = "uncertain".into(); error = Some("chat_submission_pending".into()); }
                }
            }
        }
    }
    let request: SpawnRequest = decoded(&raw)?;
    let resolved_plan_execute = if decision == "confirmed" && request.plan_execute.is_some() { workflow_config(conn, id)? } else { None };
    Ok(Receipt { request_id: id.into(), request, resolved_plan_execute, decision, state, session_id, message_id, session, error })
}

/// The run owns successful creation snapshots; the spawn launch slot also survives creation failure.
pub(super) fn workflow_config(conn: &Connection, id: &str) -> Result<Option<super::plan_execute::Config>, String> {
    let saved: Option<String> = conn.query_row("SELECT config FROM plan_execute_runs WHERE id=?1", [id], |r| r.get(0))
        .optional().map_err(|e| e.to_string())?;
    if let Some(saved) = saved { return decoded(&saved).map(Some); }
    pending_workflow_config(conn, id)
}

pub(super) fn pending_workflow_config(conn: &Connection, id: &str) -> Result<Option<super::plan_execute::Config>, String> {
    let launch: Option<Option<String>> = conn.query_row("SELECT launch FROM spawn_requests WHERE id=?1", [id], |r| r.get(0))
        .optional().map_err(|e| e.to_string())?;
    match launch.flatten() {
        Some(raw) => serde_json::from_value(decoded::<Value>(&raw)?["planExecute"].clone()).map_err(|e| e.to_string()),
        None => Ok(None),
    }
}

pub(super) fn workflow_start_config(conn: &Connection, id: &str, request: &SpawnRequest) -> Result<Option<super::plan_execute::Config>, String> {
    let approved: Option<(String, String)> = conn.query_row("SELECT decision,COALESCE(approved,request) FROM spawn_requests WHERE id=?1",
        [id], |r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e| e.to_string())?;
    if let Some((decision, approved)) = approved {
        if decision != "confirmed" || encoded(&decoded::<SpawnRequest>(&approved)?)? != encoded(request)? {
            return Err("The spawn request identity cannot be changed".into());
        }
    }
    pending_workflow_config(conn, id)
}

pub(super) fn save_workflow_config(conn: &Connection, id: &str, config: &super::plan_execute::Config) -> Result<(), String> {
    conn.execute("UPDATE spawn_requests SET launch=?2 WHERE id=?1 AND decision='confirmed' AND launch IS NULL",
        params![id,encoded(&json!({"planExecute":config}))?]).map_err(|e| e.to_string())?;
    Ok(())
}

fn submission_outcome(conn: &Connection, session_id: &str, message_id: &str) -> Result<Option<String>, String> {
    let outcome: Option<Option<String>> = conn.query_row("SELECT outcome FROM chat_submissions WHERE session_id=?1 AND id=?2",
        params![session_id,message_id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    // A claimed row without an outcome is deliberately represented as an unknown marker.
    Ok(outcome.map(|value| value.unwrap_or_else(|| "{}".into())))
}

pub fn list(app: &AppCtx) -> Result<Vec<Receipt>, String> {
    let conn = app.db().conn.lock().unwrap();
    let mut statement = conn.prepare("SELECT id FROM spawn_requests ORDER BY created_at,id").map_err(|e| e.to_string())?;
    let ids = statement.query_map([], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    ids.iter().map(|id| read_conn(&conn, id)).collect()
}

fn changed(app: &AppCtx, source: &str, receipt: &Receipt) {
    app.emit("spawn://resolved", json!({"source":source,"requestId":receipt.request_id,
        "parentSessionId":receipt.request.parent_session_id,"prompt":receipt.request.prompt,
        "confirmed":receipt.decision == "confirmed"}));
}

pub fn register(app: &AppCtx, mut request: SpawnRequest) -> Result<Receipt, String> {
    let id = request.request_id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    request.request_id = Some(id.clone());
    let guard = request_lock(app, &id)?;
    let raw = encoded(&request)?;
    let auto = {
        let conn = app.db().conn.lock().unwrap();
        let previous: Option<String> = conn.query_row("SELECT request FROM spawn_requests WHERE id=?1", [&id], |r| r.get(0))
            .optional().map_err(|e| e.to_string())?;
        if let Some(previous) = previous {
            if previous != raw { return Err("The request identifier already belongs to another task".into()); }
            drop(conn); drop(guard);
            return resume(app, &id);
        }
        validate_request(&conn, &request)?;
        let auto = request.no_confirm || settings(&conn)?["spawnConfirm"].as_bool() == Some(false);
        conn.execute("INSERT INTO spawn_requests(id,request,approved,decision,session_id,message_id) VALUES (?1,?2,?3,?4,?5,?6)",
            params![id, raw, auto.then_some(&raw), if auto { "confirmed" } else { "pending" }, uuid::Uuid::new_v4().to_string(), format!("msg-{id}")])
            .map_err(|e| e.to_string())?;
        auto
    };
    crate::diagnostics::record("INFO", "spawn_request", json!({"requestId":id,"status":"registered","automatic":auto}));
    app.emit("spawn://request", request);
    drop(guard);
    if auto { resume(app, &id) } else { read(app, &id) }
}

pub fn decide(app: &AppCtx, source: &str, id: &str, confirmed: bool, request: Option<SpawnRequest>) -> Result<Receipt, String> {
    let guard = request_lock(app, id)?;
    {
        let conn = app.db().conn.lock().unwrap();
        let receipt = read_conn(&conn, id)?;
        if receipt.decision == "pending" {
            let approved = request.unwrap_or(receipt.request.clone());
            if approved.request_id.as_deref() != Some(id) || approved.parent_session_id != receipt.request.parent_session_id
                || approved.plan_execute.is_some() != receipt.request.plan_execute.is_some() {
                return Err("The spawn request identity cannot be changed".into());
            }
            if confirmed { validate_request(&conn, &approved)?; }
            conn.execute("UPDATE spawn_requests SET approved=?2,decision=?3,state=?4,error=NULL WHERE id=?1 AND decision='pending'",
                params![id, confirmed.then(|| encoded(&approved)).transpose()?, if confirmed { "confirmed" } else { "cancelled" },
                    if confirmed { "pending" } else { "cancelled" }]).map_err(|e| e.to_string())?;
        } else if !confirmed && receipt.decision == "confirmed" && receipt.state == "failed" {
            // Closing a failed launch abandons its retry. A session it already created stays in the tree.
            conn.execute("UPDATE spawn_requests SET decision='cancelled',state='cancelled',error=NULL WHERE id=?1 AND decision='confirmed'",
                [id]).map_err(|e| e.to_string())?;
        }
    }
    let receipt = read(app, id)?;
    crate::diagnostics::record("INFO", "spawn_decision", json!({"requestId":id,"status":receipt.decision}));
    changed(app, source, &receipt);
    drop(guard);
    if receipt.decision == "confirmed" { resume(app, id) } else { Ok(receipt) }
}

pub fn resume(app: &AppCtx, id: &str) -> Result<Receipt, String> {
    let _guard = request_lock(app, id)?;
    let receipt = read(app, id)?;
    if receipt.decision != "confirmed" || matches!(receipt.state.as_str(), "complete" | "cancelled" | "dispatching" | "uncertain") { return Ok(receipt); }
    let mut diagnostic = crate::diagnostics::Span::new("spawn_execute", json!({"requestId":id,"sessionId":receipt.session_id}));
    let result = execute(app, &receipt);
    diagnostic.finish(&result);
    if let Err(error) = result {
        app.db().conn.lock().unwrap().execute(
            "UPDATE spawn_requests SET state=CASE WHEN state IN ('dispatching','uncertain') THEN state ELSE 'failed' END,error=?2 WHERE id=?1",
            params![id,error]).map_err(|e| e.to_string())?;
    }
    let receipt = read(app, id)?;
    changed(app, "backend", &receipt);
    Ok(receipt)
}

#[derive(Clone, Serialize, Deserialize)]
struct Launch {
    project_id: String, group_id: Option<String>, parent_id: Option<String>, name: String, kind: SessionKind,
    cwd: Option<String>, args: Option<String>, permission: Option<String>, engine: String,
    preset_id: Option<String>, agent_path: Option<String>, worktree: Option<String>, base: Option<String>,
    selection: session_settings::Selection,
}

fn launch_defaults(app: &AppCtx, conn: &Connection, parent: Option<&Session>, kind: SessionKind) -> Result<(Option<String>, Option<String>, String), String> {
    let prefs = settings(conn)?;
    let same = parent.filter(|p| p.kind == kind);
    let args = same.and_then(|p| p.agent_args.clone()).or_else(|| prefs["agentDefaults"][kind.as_str()]["args"].as_str().map(str::to_owned));
    let permission = permission_catalog::effective(conn, kind, same.and_then(|p| p.permission_mode.as_deref()))?;
    permission_catalog::validate(kind, permission.as_deref())?;
    let engine = if kind == SessionKind::Zcode { "chat" }
        else if !kind.supports_chat() { "tui" }
        else if let Some(parent) = parent.filter(|p| p.kind.supports_chat()) { &parent.engine }
        else { prefs["agentDefaults"][kind.as_str()]["engine"].as_str()
            .unwrap_or(if matches!(kind, SessionKind::Claude | SessionKind::Codex) { "chat" } else { "tui" }) };
    let _ = app;
    if !matches!(engine, "tui" | "chat") { return Err("Invalid default session engine".into()); }
    Ok((args, permission, engine.into()))
}

pub(crate) fn default_selection(conn: &Connection, kind: SessionKind, args: Option<&str>) -> Result<session_settings::Selection, String> {
    let mut selection = session_settings::from_args(kind, args);
    if selection != session_settings::Selection::default() { return Ok(selection); }
    let prefs = settings(conn)?;
    let key = kind.as_str();
    selection.model = session_settings::clean(prefs["chatModelByKind"][key].as_str().or_else(||
        (kind == SessionKind::Claude).then(|| prefs["chatModel"].as_str()).flatten()));
    let model = selection.model.as_deref().unwrap_or("");
    selection.effort = session_settings::clean(prefs["chatEffortByModel"][format!("{key}:{model}")].as_str().or_else(||
        (kind == SessionKind::Claude).then(|| prefs["chatEffortByModel"][model].as_str()).flatten()));
    Ok(selection)
}

fn prepare_launch(app: &AppCtx, request: &SpawnRequest) -> Result<Launch, String> {
    let (parent, mut launch) = {
        let conn = app.db().conn.lock().unwrap();
        validate_request(&conn, request)?;
        let parent = active_session(&conn, &request.parent_session_id)?;
        let kind = match request.kind.as_deref() {
            Some(kind) => parse_launch_kind(kind)?,
            None => parse_agent(parent.kind.as_str()).unwrap_or(SessionKind::Claude),
        };
        let (args, permission, engine) = launch_defaults(app, &conn, Some(&parent), kind)?;
        let args = launch_options::apply(kind, args.as_deref(), request.model.as_deref(), request.effort.as_deref())?;
        let cwd = request.cwd.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned)
            .or_else(|| app.pty().cwd(&parent.id)).or(parent.cwd.clone()).or(repo::get_project_root(&conn, &parent.project_id)?);
        let launch = Launch { project_id: parent.project_id.clone(), group_id: parent.group_id.clone(), parent_id: Some(parent.id.clone()),
            name: request.prompt.trim().chars().take(24).collect(), kind, cwd, selection: default_selection(&conn, kind, args.as_deref())?,
            args, permission, engine, preset_id: None, agent_path: None, worktree: None, base: None };
        (parent, launch)
    };
    if parent.kind == launch.kind && launch.kind.supports_chat() {
        let mut inherited = session_settings::resolve(app, &parent)?;
        if request.model.is_some() { inherited.model = session_settings::clean(request.model.as_deref()); }
        if request.effort.is_some() { inherited.effort = session_settings::clean(request.effort.as_deref()); }
        launch.selection = inherited;
    }
    if request.model.is_some() { launch.selection.model = session_settings::clean(request.model.as_deref()); }
    if request.effort.is_some() { launch.selection.effort = session_settings::clean(request.effort.as_deref()); }
    Ok(launch)
}

fn insert_session(conn: &Connection, id: &str, launch: &Launch) -> Result<Session, String> {
    let mut session = repo::create_session_identified(conn, id, &launch.project_id, launch.group_id.as_deref(), &launch.name, launch.kind,
        None, launch.cwd.as_deref(), None, launch.parent_id.as_deref(), launch.worktree.as_deref(), launch.args.as_deref(), launch.permission.as_deref(),
        launch.base.as_deref(), launch.preset_id.as_deref(), launch.agent_path.as_deref(), Some(&launch.engine))?;
    if matches!(launch.kind, SessionKind::Zoo | SessionKind::Grok) {
        repo::set_agent_session_id(conn, id, id, launch.kind)?;
        session.agent_session_id = Some(id.to_string());
    }
    session_settings::save(conn, id, &launch.selection, &json!({}))?;
    Ok(session)
}

fn execute(app: &AppCtx, receipt: &Receipt) -> Result<(), String> {
    if receipt.request.plan_execute.is_some() {
        let result = super::plan_execute::start(app, &receipt.request)?;
        let session_id = result["planner"]["id"].as_str().ok_or("Workflow did not return its planner")?;
        let error = (result["run"]["state"].as_str() == Some("blocked")).then(|| result["run"]["summary"].as_str().unwrap_or("Workflow launch is blocked").to_string());
        app.db().conn.lock().unwrap().execute("UPDATE spawn_requests SET session_id=?2,state=?3,error=?4 WHERE id=?1",
            params![receipt.request_id,session_id,if error.is_some() { "failed" } else { "complete" },error]).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let session = match &receipt.session {
        Some(session) => {
            if session.archived_at.is_some() { return Err("The created session is archived; it was not recreated".into()); }
            session.clone()
        }
        None => {
            // A deleted successful result must not turn a retry into another child.
            let previously_created: bool = app.db().conn.lock().unwrap().query_row(
                "SELECT initial_prompt IS NOT NULL FROM spawn_requests WHERE id=?1", [&receipt.request_id], |r| r.get(0)).map_err(|e| e.to_string())?;
            if previously_created { return Err("The created session was deleted; it was not recreated".into()); }
            let raw: Option<String> = app.db().conn.lock().unwrap().query_row("SELECT launch FROM spawn_requests WHERE id=?1", [&receipt.request_id], |r| r.get(0)).map_err(|e| e.to_string())?;
            let mut launch: Launch = match raw { Some(raw) => decoded(&raw)?, None => {
                let launch = prepare_launch(app, &receipt.request)?;
                app.db().conn.lock().unwrap().execute("UPDATE spawn_requests SET launch=?2 WHERE id=?1", params![receipt.request_id,encoded(&launch)?]).map_err(|e| e.to_string())?;
                launch
            }};
            let creation = (|| -> Result<Session, String> {
                if receipt.request.worktree != Some(false) {
                    if let Some(cwd) = &launch.cwd {
                        // Ordinary spawn falls back only after any partial creation was safely cleaned up.
                        if crate::git::worktree_list(cwd).is_ok() {
                            match owned_worktree(app, &receipt.request_id, cwd, &launch.name, false) {
                                Ok(wt) => {
                                    launch.cwd = Some(wt.path.clone()); launch.worktree = Some(wt.path); launch.base = Some(wt.base_ref);
                                }
                                Err(error) => {
                                    cleanup_worktree(app, &receipt.request_id).map_err(|cleanup|
                                        format!("{error}; worktree cleanup needs attention: {cleanup}"))?;
                                    crate::diagnostics::record("WARN", "spawn_worktree_fallback", json!({"requestId":receipt.request_id,"status":"cleaned"}));
                                }
                            }
                        }
                    }
                }
                let prompt = initial_prompt(app, &receipt.request_id, &receipt.request, &launch)?;
                let mut conn = app.db().conn.lock().unwrap();
                let tx = conn.transaction().map_err(|e| e.to_string())?;
                active_session(&tx, &receipt.request.parent_session_id)?;
                let session = insert_session(&tx, &receipt.session_id, &launch)?;
                tx.execute("UPDATE spawn_requests SET state='ready',initial_prompt=?2,error=NULL WHERE id=?1", params![receipt.request_id,prompt]).map_err(|e| e.to_string())?;
                bind_worktree(&tx, &receipt.request_id, &session.id)?;
                tx.commit().map_err(|e| e.to_string())?;
                Ok(session)
            })();
            match creation {
                Ok(session) => { app.emit(TREE_CHANGED, ()); session }
                Err(error) => {
                    return match cleanup_worktree(app, &receipt.request_id) {
                        Ok(()) => Err(error), Err(cleanup) => Err(format!("{error}; worktree cleanup needs attention: {cleanup}")),
                    };
                }
            }
        }
    };
    if session.kind == SessionKind::Terminal {
        app.db().conn.lock().unwrap().execute("UPDATE spawn_requests SET state='complete',error=NULL WHERE id=?1", [&receipt.request_id]).map_err(|e| e.to_string())?;
    } else if session.engine == "chat" {
        let payload = serde_json::to_vec(&json!([receipt.request.prompt,receipt.request.images,"queue"])).map_err(|e| e.to_string())?;
        if !app.chat().is_alive(&session.id) {
            super::chat::submissions::recover_queued(&app.db().conn.lock().unwrap(), &session.id, &receipt.message_id, &payload)?;
        }
        core::chat_start(app, &session.id, None, None, false)?;
        let status = core::chat_send(app, &session.id, &receipt.request.prompt, receipt.request.images.clone(), Some("queue"), Some(&receipt.message_id))?;
        app.db().conn.lock().unwrap().execute("UPDATE spawn_requests SET state=?2,error=NULL WHERE id=?1",
            params![receipt.request_id,if status == "queued" { "waiting" } else { "complete" }]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn initial_prompt(app: &AppCtx, id: &str, request: &SpawnRequest, launch: &Launch) -> Result<String, String> {
    if launch.engine == "chat" || request.images.is_empty() { return Ok(request.prompt.clone()); }
    use base64::Engine;
    let dir = app.data_dir()?.join("spawn-images").join(id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut prompt = request.prompt.clone();
    for (index, image) in request.images.iter().enumerate() {
        let bytes = base64::engine::general_purpose::STANDARD.decode(&image.data).map_err(|e| e.to_string())?;
        let extension = match image.mime_type.as_str() { "image/jpeg" => "jpg", "image/png" => "png", "image/gif" => "gif", "image/webp" => "webp", _ => return Err("Unsupported image type".into()) };
        let path = dir.join(format!("{index}.{extension}"));
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        prompt.push_str(&format!("\n{}{}", if launch.kind == SessionKind::Codex { "image_path: " } else { "" }, path.to_string_lossy()));
    }
    Ok(prompt)
}

pub fn owned_worktree(app: &AppCtx, id: &str, cwd: &str, name: &str, sibling: bool) -> Result<crate::git::WorktreeInfo, String> {
    let saved: Option<String> = app.db().conn.lock().unwrap().query_row("SELECT receipt FROM spawn_worktrees WHERE id=?1", [id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    let receipt: OwnedWorktree = if let Some(raw) = saved { decoded(&raw)? } else {
        let receipt = crate::git::prepare_owned_worktree(cwd, name, sibling)?;
        app.db().conn.lock().unwrap().execute("INSERT INTO spawn_worktrees(id,receipt) VALUES (?1,?2)", params![id,encoded(&receipt)?]).map_err(|e| e.to_string())?;
        receipt
    };
    crate::git::materialize_owned_worktree(&receipt)
}

pub fn bind_worktree(conn: &Connection, id: &str, session_id: &str) -> Result<(), String> {
    conn.execute("UPDATE spawn_worktrees SET session_id=?2,error=NULL WHERE id=?1", params![id,session_id]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn cleanup_worktree(app: &AppCtx, id: &str) -> Result<(), String> {
    let mut diagnostic = crate::diagnostics::Span::new("spawn_worktree_cleanup", json!({"requestId":id}));
    let mut database = app.db().conn.lock().unwrap();
    // Reserve the SQLite writer across the reference check and removal, including other backends.
    let conn = database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|e| e.to_string())?;
    let saved: Option<(String, Option<String>)> = conn.query_row("SELECT receipt,session_id FROM spawn_worktrees WHERE id=?1", [id], |r| Ok((r.get(0)?,r.get(1)?)))
        .optional().map_err(|e| e.to_string())?;
    let Some((raw, bound)) = saved else { diagnostic.success(); return Ok(()); };
    let receipt: OwnedWorktree = decoded(&raw)?;
    let tree = repo::list_tree(&conn)?;
    let archived = repo::list_archived(&conn)?;
    let canonical = std::fs::canonicalize(&receipt.path).ok();
    let contains = |value: Option<&str>| value.is_some_and(|p| {
        std::path::Path::new(p).starts_with(&receipt.path) || canonical.as_ref().is_some_and(|root|
            std::fs::canonicalize(p).is_ok_and(|path| path.starts_with(root)))
    });
    let referenced = bound.as_deref().map(|id| repo::get_session(&conn, id)).transpose()?.flatten().is_some()
        || tree.sessions.iter().chain(archived.iter()).any(|s| contains(s.cwd.as_deref()) || contains(s.worktree_path.as_deref()))
        || tree.groups.iter().any(|g| contains(g.worktree_path.as_deref()))
        || tree.projects.iter().any(|p| contains(Some(&p.root_path)));
    let result = (if referenced { Err("A session, group or project still references this worktree; it was preserved".into()) }
        else { crate::git::rollback_owned_worktree(&receipt) }).map_err(|error: String|
            format!("{error} (path: {}; branch: {}; creation: {})", receipt.path, receipt.branch, receipt.owner));
    match &result {
        Ok(()) => { conn.execute("DELETE FROM spawn_worktrees WHERE id=?1", [id]).map_err(|e| e.to_string())?; }
        Err(error) => { conn.execute("UPDATE spawn_worktrees SET error=?2 WHERE id=?1", params![id,error]).map_err(|e| e.to_string())?; }
    }
    conn.commit().map_err(|e| e.to_string())?;
    diagnostic.finish(&result);
    result
}

#[derive(Clone)]
pub struct PtyInitial { pub request_id: String, pub message_id: String, pub prompt: String }

/// Durable spawn sessions use their current persisted launch settings, never a stale client's copy.
pub fn pty_session(app: &AppCtx, session_id: &str) -> Result<Option<Session>, String> {
    let conn = app.db().conn.lock().unwrap();
    let owned: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM spawn_requests WHERE session_id=?1)",
        [session_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if owned { active_session(&conn, session_id).map(Some) } else { Ok(None) }
}

pub fn pty_initial(app: &AppCtx, session_id: &str) -> Result<Option<PtyInitial>, String> {
    let conn = app.db().conn.lock().unwrap();
    let row: Option<(String, String, String, String)> = conn.query_row(
        "SELECT id,message_id,initial_prompt,state FROM spawn_requests WHERE session_id=?1 AND decision='confirmed' AND initial_prompt IS NOT NULL", [session_id],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(|e| e.to_string())?;
    match row {
        None => Ok(None),
        Some((_,_,_,state)) if state == "complete" => Ok(None),
        Some((_,_,_,state)) if matches!(state.as_str(), "dispatching" | "uncertain") => Err("spawn_initial_delivery_uncertain".into()),
        Some((request_id,message_id,prompt,_)) => Ok(Some(PtyInitial { request_id,message_id,prompt })),
    }
}
pub fn pty_dispatching(app: &AppCtx, initial: &PtyInitial) -> Result<(), String> {
    let changed = app.db().conn.lock().unwrap().execute(
        "UPDATE spawn_requests SET state='dispatching',error=NULL WHERE id=?1 AND message_id=?2 AND state IN ('ready','failed') AND decision='confirmed'",
        params![initial.request_id,initial.message_id]).map_err(|e| e.to_string())?;
    if changed != 1 { return Err("The initial task was already claimed; refresh its receipt".into()); }
    Ok(())
}
pub fn pty_finished(app: &AppCtx, initial: &PtyInitial, result: Result<(), String>) -> Result<(), String> {
    let error = result.as_ref().err();
    app.db().conn.lock().unwrap().execute("UPDATE spawn_requests SET state=?3,error=?4 WHERE id=?1 AND message_id=?2 AND state='dispatching'",
        params![initial.request_id,initial.message_id,if result.is_ok() { "complete" } else { "uncertain" },error]).map_err(|e| e.to_string())?;
    changed(app, "backend", &read(app, &initial.request_id)?);
    Ok(())
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSessionContext {
    pub project_id: Option<String>, pub group_id: Option<String>,
    pub active_session_id: Option<String>, pub placement: Placement,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Placement { Sibling, Child }
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentSessionRequest {
    pub request_id: String, pub context: AgentSessionContext,
    pub kind: Option<String>, pub preset_id: Option<String>,
}

struct Location { project_id: String, group_id: Option<String>, parent_id: Option<String>, cwd: Option<String>, worktree: Option<String>, base: Option<String>, names: Vec<String> }
fn agent_location(conn: &Connection, context: &AgentSessionContext) -> Result<Location, String> {
    let tree = repo::list_tree(conn)?;
    let project_id = context.project_id.as_deref().ok_or("Select a project before creating a session")?;
    let project = tree.projects.iter().find(|p| p.id == project_id).ok_or("The selected project no longer exists")?;
    let active = context.active_session_id.as_deref().map(|id| active_session(conn, id)).transpose()?;
    if active.as_ref().is_some_and(|s| s.project_id != project_id || s.group_id != context.group_id) {
        return Err("The active session no longer belongs to the selected project and group".into());
    }
    let group_id = context.group_id.clone();
    let group = group_id.as_ref().map(|id| tree.groups.iter().find(|g| &g.id == id && g.project_id == project_id).ok_or("The selected group no longer exists in this project")).transpose()?;
    let parent_id = active.as_ref().and_then(|s| match context.placement { Placement::Sibling => s.parent_session_id.clone(), Placement::Child => Some(s.id.clone()) });
    let parent = parent_id.as_deref().map(|id| active_session(conn, id)).transpose()?;
    if parent.as_ref().is_some_and(|p| p.project_id != project_id || p.group_id != group_id) { return Err("The parent session has moved; refresh the creation location".into()); }
    let worktree = active.as_ref().and_then(|s| s.worktree_path.clone()).or_else(|| group.and_then(|g| g.worktree_path.clone()));
    let base = active.as_ref().and_then(|s| s.worktree_base_ref.clone()).or_else(|| group.and_then(|g| g.worktree_base_ref.clone()));
    let cwd = active.as_ref().and_then(|s| s.cwd.clone()).or(worktree.clone()).or_else(|| (!project.root_path.trim().is_empty()).then(|| project.root_path.clone()));
    let mut names = vec![project.name.clone()];
    if let Some(group) = group { names.push(group.name.clone()); }
    if let Some(parent) = parent { names.push(parent.name); }
    Ok(Location { project_id: project_id.into(), group_id, parent_id, cwd, worktree, base, names })
}

pub fn prepare_agent_session(app: &AppCtx, context: &AgentSessionContext) -> Result<Value, String> {
    let conn = app.db().conn.lock().unwrap();
    let location = agent_location(&conn, context)?;
    Ok(json!({"context":context,"options":launch_options::catalog().into_iter().filter(|o| o.accepts_task).collect::<Vec<_>>(),
        "presets":repo::list_agent_presets(&conn)?,"locationNames":location.names}))
}

pub fn create_agent_session(app: &AppCtx, request: &AgentSessionRequest) -> Result<Session, String> {
    let _guard = request_lock(app, &request.request_id)?;
    let raw = encoded(request)?;
    let mut conn = app.db().conn.lock().unwrap();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let existing: Option<(String, String)> = tx.query_row("SELECT request,session_id FROM agent_session_creations WHERE id=?1", [&request.request_id], |r| Ok((r.get(0)?,r.get(1)?)))
        .optional().map_err(|e| e.to_string())?;
    if let Some((saved, id)) = existing {
        if saved != raw { return Err("The creation request already has a different selection or location".into()); }
        return active_session(&tx, &id);
    }
    let location = agent_location(&tx, &request.context)?;
    let preset = match (&request.kind, &request.preset_id) {
        (Some(_), None) => None,
        (None, Some(id)) => Some(repo::list_agent_presets(&tx)?.into_iter().find(|p| &p.id == id).ok_or("The selected agent preset no longer exists")?),
        _ => return Err("Select either an agent or a preset".into()),
    };
    let kind = if let Some(preset) = &preset { parse_agent(preset.base_kind.as_str())? } else { parse_agent(request.kind.as_deref().unwrap())? };
    let (mut args, mut permission, engine) = launch_defaults(app, &tx, None, kind)?;
    if let Some(preset) = &preset {
        // A preset's empty arguments deliberately clear the kind's custom CLI arguments.
        args = preset.agent_args.clone();
        permission = permission_catalog::effective(&tx, kind, preset.permission_mode.as_deref())?;
    }
    permission_catalog::validate(kind, permission.as_deref())?;
    let name = if let Some(preset) = &preset { preset.name.clone() } else {
        let label = launch_options::label(kind);
        let prefix = format!("{label} ");
        let next = repo::list_tree(&tx)?.sessions.iter().filter(|s| s.project_id == location.project_id && s.kind == kind)
            .filter_map(|s| s.name.strip_prefix(&prefix)?.parse::<u64>().ok()).max().unwrap_or(0).saturating_add(1);
        format!("{label} {next}")
    };
    let launch = Launch { project_id: location.project_id, group_id: location.group_id, parent_id: location.parent_id,
        name, kind,
        cwd: location.cwd, selection: default_selection(&tx, kind, args.as_deref())?, args, permission, engine,
        preset_id: preset.as_ref().map(|p| p.id.clone()), agent_path: preset.and_then(|p| p.exec_path), worktree: location.worktree, base: location.base };
    let session = insert_session(&tx, &uuid::Uuid::new_v4().to_string(), &launch)?;
    tx.execute("INSERT INTO agent_session_creations(id,request,session_id) VALUES (?1,?2,?3)", params![request.request_id,raw,session.id]).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    drop(conn);
    app.emit(TREE_CHANGED, ());
    Ok(session)
}

/// Draft display choices are read from the same backend rules used at final creation.
pub fn prepare(app: &AppCtx, id: &str, kind: Option<&str>) -> Result<Value, String> {
    let receipt = read(app, id)?;
    if receipt.decision == "confirmed" {
        let raw: Option<String> = app.db().conn.lock().unwrap().query_row("SELECT launch FROM spawn_requests WHERE id=?1", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
        if let Some(raw) = raw {
            let launch: Launch = decoded(&raw)?;
            let cwd = receipt.session.as_ref().and_then(|s| s.cwd.as_ref()).or(launch.cwd.as_ref());
            return Ok(json!({"kind":launch.kind,"cwd":cwd,"model":launch.selection.model,"effort":launch.selection.effort}));
        }
    }
    let mut request = receipt.request;
    if let Some(kind) = kind {
        let original = prepare_launch(app, &request)?;
        if original.kind.as_str() != kind { request.model = None; request.effort = None; }
        request.kind = Some(kind.into());
    }
    let launch = prepare_launch(app, &request)?;
    Ok(json!({"kind":launch.kind,"cwd":launch.cwd,"model":launch.selection.model,"effort":launch.selection.effort}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::{Path, PathBuf}, sync::{Arc, Barrier}};
    struct Directory(PathBuf);
    impl Drop for Directory { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    struct Fixture { app: AppCtx, directory: Directory, parent: Session }
    fn git(path: &Path, args: &[&str]) -> String {
        let out = crate::host::command("git").arg("-C").arg(path).args(args).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().into()
    }
    fn fixture() -> Fixture {
        let directory = Directory(std::env::temp_dir().join(format!("vlx-spawn-test-{}",uuid::Uuid::new_v4())));
        std::fs::create_dir_all(directory.0.join("data")).unwrap();
        std::fs::create_dir_all(directory.0.join("repo")).unwrap();
        let root = directory.0.join("repo");
        git(&root, &["init", "-b", "main"]);
        git(&root, &["config", "user.email", "spawn-test@example.invalid"]);
        git(&root, &["config", "user.name", "Spawn test"]);
        git(&root, &["commit", "--allow-empty", "-m", "initial"]);
        let app = reopen(&directory.0);
        let parent = {
            let conn = app.db().conn.lock().unwrap();
            let project = repo::import_project(&conn, root.to_str().unwrap()).unwrap();
            repo::create_session_full(&conn, &project.id, None, "Parent", SessionKind::Claude, None, root.to_str(), None, None, None,
                Some("--model sonnet --effort high"), Some("bypassPermissions"), None, None, None, Some("tui")).unwrap()
        };
        Fixture { app, directory, parent }
    }
    fn reopen(path: &Path) -> AppCtx {
        let data = path.join("data");
        let db = crate::db::Db::open(&data.join("state.db")).unwrap();
        let host = Arc::new(crate::host::HeadlessHost::new(data, db));
        host.set_hooks(super::super::server::HookServer { port: 0, token: "fixture".into() });
        AppCtx::Headless(host)
    }
    fn request(f: &Fixture, worktree: bool) -> SpawnRequest {
        serde_json::from_value(json!({"requestId":uuid::Uuid::new_v4().to_string(),"parentSessionId":f.parent.id,
            "prompt":"Investigate the task","worktree":worktree})).unwrap()
    }
    fn confirm(f: &Fixture, req: SpawnRequest) -> Receipt {
        let receipt = register(&f.app, req.clone()).unwrap();
        decide(&f.app, "test", &receipt.request_id, true, Some(req)).unwrap()
    }

    #[test]
    fn zcode_launch_preparation_keeps_explicit_model_and_max_effort() {
        let f = fixture();
        let mut req = request(&f, false);
        req.kind = Some("zcode".into());
        req.model = Some("deepseek/deepseek-flash".into());
        req.effort = Some("max".into());
        req.no_confirm = true;
        let launch = prepare_launch(&f.app, &req).unwrap();
        assert_eq!(launch.kind, SessionKind::Zcode);
        assert_eq!(launch.selection.model.as_deref(), Some("deepseek/deepseek-flash"));
        assert_eq!(launch.selection.effort.as_deref(), Some("max"));
    }
    fn owned(f: &Fixture, id: &str) -> OwnedWorktree {
        let raw: String = f.app.db().conn.lock().unwrap().query_row("SELECT receipt FROM spawn_worktrees WHERE id=?1", [id], |r| r.get(0)).unwrap();
        decoded(&raw).unwrap()
    }
    fn children(f: &Fixture) -> Vec<Session> {
        repo::list_tree(&f.app.db().conn.lock().unwrap()).unwrap().sessions.into_iter().filter(|s| s.parent_session_id.as_deref() == Some(&f.parent.id)).collect()
    }

    #[test]
    fn equal_prompts_have_independent_identity_and_retries_keep_one_result() {
        let f = fixture(); let a = request(&f, false); let b = request(&f, false);
        let one = confirm(&f, a.clone()); let two = confirm(&f, b);
        assert_ne!(one.request_id, two.request_id); assert_ne!(one.session_id, two.session_id);
        assert_eq!(register(&f.app, a.clone()).unwrap().session_id, one.session_id);
        let mut changed = a; changed.prompt.push_str(" changed");
        assert!(register(&f.app, changed).is_err());
        assert_eq!(children(&f).len(), 2);
    }

    #[test]
    fn concurrent_clients_confirm_only_one_session_and_worktree() {
        let f = fixture(); let req = request(&f, true); let id = register(&f.app, req.clone()).unwrap().request_id;
        let gate = Arc::new(Barrier::new(6));
        let handles: Vec<_> = (0..6).map(|_| {
            let app = f.app.clone(); let id = id.clone(); let req = req.clone(); let gate = gate.clone();
            std::thread::spawn(move || { gate.wait(); decide(&app, "client", &id, true, Some(req)).unwrap().session_id })
        }).collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert!(results.iter().all(|s| s == &results[0])); assert_eq!(children(&f).len(), 1);
        assert_eq!(crate::git::worktree_list(f.parent.cwd.as_deref().unwrap()).unwrap().len(), 2);
    }

    #[test]
    fn cancellation_and_confirmation_have_one_durable_decision() {
        let f = fixture(); let req = request(&f, false); let id = register(&f.app, req.clone()).unwrap().request_id;
        let cancelled = decide(&f.app, "phone", &id, false, None).unwrap();
        assert_eq!(cancelled.decision, "cancelled");
        assert_eq!(decide(&f.app, "desktop", &id, true, Some(req)).unwrap().decision, "cancelled");
        assert!(children(&f).is_empty());
        let next = confirm(&f, request(&f, false));
        let late = decide(&f.app, "phone", &next.request_id, false, None).unwrap();
        assert_eq!(late.decision, "confirmed"); assert_eq!(late.session_id, next.session_id);
    }

    #[test]
    fn concurrent_confirmation_and_cancellation_share_one_final_decision() {
        let f=fixture();let req=request(&f,false);let receipt=register(&f.app,req.clone()).unwrap();
        let gate=Arc::new(Barrier::new(2));
        let handles:Vec<_>=[true,false].into_iter().map(|confirmed| {
            let app=f.app.clone();let id=receipt.request_id.clone();let request=req.clone();let gate=gate.clone();
            std::thread::spawn(move||{gate.wait();decide(&app,"client",&id,confirmed,Some(request)).unwrap()})
        }).collect();
        let results:Vec<_>=handles.into_iter().map(|h|h.join().unwrap()).collect();
        assert_eq!(results[0].decision,results[1].decision);
        let result=read(&reopen(&f.directory.0),&receipt.request_id).unwrap();
        assert_eq!(result.decision,results[0].decision);
        assert_eq!(children(&f).len(),usize::from(result.decision=="confirmed"));
    }

    #[test]
    fn automatic_decisions_use_persisted_settings_and_ignore_client_defaults() {
        let f = fixture();
        f.app.db().conn.lock().unwrap().execute("INSERT INTO app_settings(key,value,updated_at) VALUES ('vlx-settings',?1,unixepoch())", [json!({"spawnConfirm":false}).to_string()]).unwrap();
        let result = register(&f.app, request(&f, false)).unwrap();
        assert_eq!(result.decision, "confirmed"); assert!(result.session.is_some());
        f.app.db().conn.lock().unwrap().execute("UPDATE app_settings SET value='{}' WHERE key='vlx-settings'", []).unwrap();
        let mut req = request(&f, false); req.no_confirm = true;
        assert!(register(&f.app, req).unwrap().session.is_some());
    }

    #[test]
    fn pending_review_and_lost_creation_response_recover_after_database_reopen() {
        let f = fixture(); let pending = register(&f.app, request(&f, false)).unwrap();
        let complete = confirm(&f, request(&f, true));
        let app = reopen(&f.directory.0);
        assert_eq!(read(&app, &pending.request_id).unwrap().decision, "pending");
        let recovered = resume(&app, &complete.request_id).unwrap();
        assert_eq!(recovered.session_id, complete.session_id); assert_eq!(recovered.message_id, complete.message_id);
        assert_eq!(children(&f).len(), 1);
    }

    #[test]
    fn creation_failure_rolls_back_only_owned_worktree_and_retry_keeps_session_identity() {
        let f = fixture(); let req = request(&f, true); let receipt = register(&f.app, req.clone()).unwrap();
        f.app.db().conn.lock().unwrap().execute_batch("CREATE TRIGGER fail_spawn BEFORE INSERT ON sessions BEGIN SELECT RAISE(FAIL,'injected create failure'); END;").unwrap();
        let failed = decide(&f.app, "client", &receipt.request_id, true, Some(req)).unwrap();
        assert_eq!(failed.state, "failed"); assert!(failed.error.unwrap().contains("injected create failure"));
        assert_eq!(crate::git::worktree_list(f.parent.cwd.as_deref().unwrap()).unwrap().len(), 1);
        assert!(git(Path::new(f.parent.cwd.as_deref().unwrap()), &["for-each-ref", "refs/heads/vlx/", "refs/velaterm/worktrees/"]).is_empty());
        f.app.db().conn.lock().unwrap().execute_batch("DROP TRIGGER fail_spawn").unwrap();
        let result = resume(&f.app, &receipt.request_id).unwrap();
        assert_eq!(result.session_id, receipt.session_id); assert!(result.session.is_some());
    }

    #[test]
    fn a_failed_confirmed_launch_can_be_closed_without_retrying() {
        let f = fixture(); let mut req = request(&f, false); req.model = Some("opus 5.5".into()); req.no_confirm = true;
        let failed = register(&f.app, req.clone()).unwrap();
        assert_eq!((failed.decision.as_str(), failed.state.as_str()), ("confirmed", "failed"));
        let closed = decide(&f.app, "client", &failed.request_id, false, None).unwrap();
        assert_eq!((closed.decision.as_str(), closed.state.as_str()), ("cancelled", "cancelled"));
        assert!(closed.error.is_none());
        assert_eq!(register(&f.app, req).unwrap().decision, "cancelled");
        assert!(children(&f).is_empty());
    }

    #[test]
    fn session_and_execution_result_commit_together() {
        let f = fixture(); let req = request(&f, true); let receipt = register(&f.app, req.clone()).unwrap();
        f.app.db().conn.lock().unwrap().execute_batch("CREATE TRIGGER fail_result BEFORE UPDATE OF state ON spawn_requests WHEN NEW.state='ready' BEGIN SELECT RAISE(FAIL,'injected result failure'); END;").unwrap();
        let failed = decide(&f.app, "client", &receipt.request_id, true, Some(req)).unwrap();
        assert_eq!(failed.state, "failed"); assert!(children(&f).is_empty());
        assert_eq!(crate::git::worktree_list(f.parent.cwd.as_deref().unwrap()).unwrap().len(), 1);
        f.app.db().conn.lock().unwrap().execute_batch("DROP TRIGGER fail_result").unwrap();
        assert!(resume(&f.app, &receipt.request_id).unwrap().session.is_some());
    }

    #[test]
    fn worktree_created_before_a_crash_is_reused_from_its_durable_intent() {
        let f = fixture(); let req = request(&f, true); let receipt = register(&f.app, req.clone()).unwrap();
        let launch = prepare_launch(&f.app, &req).unwrap();
        f.app.db().conn.lock().unwrap().execute("UPDATE spawn_requests SET decision='confirmed',approved=request,launch=?2 WHERE id=?1",
            params![receipt.request_id,encoded(&launch).unwrap()]).unwrap();
        let wt = owned_worktree(&f.app, &receipt.request_id, f.parent.cwd.as_deref().unwrap(), "crash", false).unwrap();
        let app = reopen(&f.directory.0);
        let result = resume(&app, &receipt.request_id).unwrap();
        assert_eq!(result.session.unwrap().worktree_path.as_deref(), Some(wt.path.as_str()));
        assert_eq!(children(&f).len(), 1);
    }

    #[test]
    fn dirty_or_committed_worktrees_are_retained_with_recoverable_diagnostics() {
        let f = fixture(); let id = uuid::Uuid::new_v4().to_string();
        let wt = owned_worktree(&f.app, &id, f.parent.cwd.as_deref().unwrap(), "dirty", false).unwrap();
        std::fs::write(Path::new(&wt.path).join("untracked.txt"), "user work").unwrap();
        assert!(cleanup_worktree(&f.app, &id).unwrap_err().contains("preserved"));
        assert!(Path::new(&wt.path).join("untracked.txt").exists());
        git(Path::new(&wt.path), &["add", "untracked.txt"]); git(Path::new(&wt.path), &["commit", "-m", "user change"]);
        assert!(cleanup_worktree(&f.app, &id).unwrap_err().contains("changed"));
        assert_eq!(owned(&f, &id).path, wt.path);
    }

    #[test]
    fn successful_session_reference_prevents_cleanup_even_after_delivery_failure() {
        let f = fixture(); let result = confirm(&f, request(&f, true));
        let initial = pty_initial(&f.app, &result.session_id).unwrap().unwrap();
        pty_dispatching(&f.app, &initial).unwrap();
        pty_finished(&f.app, &initial, Err("injected writer failure".into())).unwrap();
        assert!(cleanup_worktree(&f.app, &result.request_id).unwrap_err().contains("references"));
        let recovered = resume(&f.app, &result.request_id).unwrap();
        assert_eq!(recovered.session_id, result.session_id); assert_eq!(recovered.state, "uncertain");
        assert!(Path::new(recovered.session.unwrap().worktree_path.as_deref().unwrap()).is_dir());
    }

    #[test]
    fn unknown_existing_paths_and_branches_are_never_removed() {
        let f = fixture();
        let wt = crate::git::prepare_owned_worktree(f.parent.cwd.as_deref().unwrap(), "existing", false).unwrap();
        std::fs::create_dir_all(&wt.path).unwrap(); std::fs::write(Path::new(&wt.path).join("keep"), "keep").unwrap();
        assert!(crate::git::materialize_owned_worktree(&wt).is_err());
        assert!(crate::git::rollback_owned_worktree(&wt).is_err());
        assert!(Path::new(&wt.path).join("keep").exists());
        let other = crate::git::prepare_owned_worktree(f.parent.cwd.as_deref().unwrap(), "existing-branch", false).unwrap();
        git(Path::new(&other.repo), &["branch", &other.branch]);
        assert!(crate::git::materialize_owned_worktree(&other).is_err());
        assert!(crate::git::rollback_owned_worktree(&other).is_err());
        assert_eq!(git(Path::new(&other.repo), &["rev-parse", &other.branch]), other.start_commit);
    }

    #[test]
    fn ignored_files_and_external_session_references_block_cleanup() {
        let f = fixture(); let id = uuid::Uuid::new_v4().to_string();
        let wt = owned_worktree(&f.app, &id, f.parent.cwd.as_deref().unwrap(), "ignored", false).unwrap();
        std::fs::write(Path::new(&wt.path).join(".gitignore"), "secret\n").unwrap();
        git(Path::new(&wt.path), &["add", ".gitignore"]);
        // An uncommitted index also blocks removal, independently of the ignored file.
        std::fs::write(Path::new(&wt.path).join("secret"), "preserve").unwrap();
        assert!(cleanup_worktree(&f.app, &id).is_err());
        assert!(Path::new(&wt.path).join("secret").exists());
    }

    #[test]
    fn pty_initial_task_is_durable_and_a_second_claim_never_dispatches() {
        let f = fixture(); let result = confirm(&f, request(&f, false));
        let a = pty_initial(&f.app, &result.session_id).unwrap().unwrap();
        let b = pty_initial(&f.app, &result.session_id).unwrap().unwrap();
        assert_eq!(a.message_id, result.message_id); assert_eq!(a.prompt, result.request.prompt);
        pty_dispatching(&f.app, &a).unwrap(); assert!(pty_dispatching(&f.app, &b).is_err());
        assert!(pty_initial(&reopen(&f.directory.0), &result.session_id).is_err());
        pty_finished(&f.app, &a, Ok(())).unwrap();
        assert!(pty_initial(&f.app, &result.session_id).unwrap().is_none());
        assert_eq!(read(&f.app, &result.request_id).unwrap().state, "complete");
    }

    #[test]
    fn pty_spawn_settings_are_persisted_and_archived_sessions_cannot_restart() {
        let f=fixture();let result=confirm(&f,request(&f,true));
        assert!(pty_session(&f.app,&f.parent.id).unwrap().is_none());
        let session=pty_session(&reopen(&f.directory.0),&result.session_id).unwrap().unwrap();
        assert_eq!(session.kind,SessionKind::Claude);assert_eq!(session.cwd,session.worktree_path);
        assert_eq!(session.agent_args,f.parent.agent_args);assert_eq!(session.permission_mode,f.parent.permission_mode);
        f.app.db().conn.lock().unwrap().execute("UPDATE sessions SET archived_at=1 WHERE id=?1",[&result.session_id]).unwrap();
        assert!(pty_session(&f.app,&result.session_id).is_err());
    }

    #[test]
    fn ordinary_worktree_creation_failure_falls_back_after_cleaning_reserved_refs() {
        let f=fixture();let root=Path::new(f.parent.cwd.as_deref().unwrap());
        std::fs::write(root.join(".vlx-worktrees"),"existing file must survive").unwrap();
        let result=confirm(&f,request(&f,true));
        assert_eq!(result.state,"ready");let session=result.session.unwrap();
        assert_eq!(session.cwd,f.parent.cwd);assert!(session.worktree_path.is_none());
        assert_eq!(std::fs::read_to_string(root.join(".vlx-worktrees")).unwrap(),"existing file must survive");
        assert!(git(root,&["for-each-ref","--format=%(refname)","refs/velaterm/worktrees/","refs/heads/vlx/"]).is_empty());
        let count:i64=f.app.db().conn.lock().unwrap().query_row("SELECT count(*) FROM spawn_worktrees",[],|r|r.get(0)).unwrap();
        assert_eq!(count,0);
    }

    #[test]
    fn image_paths_survive_reopen_without_a_new_upload_or_task_identity() {
        let f = fixture(); let mut req = request(&f, false);
        req.kind = Some("codex".into()); req.images = vec![super::super::chat::protocol::ChatImage { mime_type: "image/png".into(), data:"AQID".into() }];
        let result = confirm(&f, req); let one = pty_initial(&f.app, &result.session_id).unwrap().unwrap();
        let two = pty_initial(&reopen(&f.directory.0), &result.session_id).unwrap().unwrap();
        assert_eq!(one.prompt, two.prompt); assert!(one.prompt.contains("image_path: "));
        let path = one.prompt.split("image_path: ").nth(1).unwrap(); assert_eq!(std::fs::read(path).unwrap(), [1,2,3]);
    }

    #[test]
    fn confirmed_preparation_uses_the_saved_launch_after_parent_defaults_change() {
        let f=fixture();let result=confirm(&f,request(&f,true));
        f.app.db().conn.lock().unwrap().execute("UPDATE sessions SET agent_args='--model changed --effort low' WHERE id=?1",[&f.parent.id]).unwrap();
        let prepared=prepare(&reopen(&f.directory.0),&result.request_id,Some("codex")).unwrap();
        assert_eq!(prepared["kind"],"claude");assert_eq!(prepared["model"],"sonnet");assert_eq!(prepared["effort"],"high");
        assert_eq!(prepared["cwd"],json!(result.session.unwrap().cwd));
    }

    #[test]
    fn ordinary_spawn_inherits_same_agent_settings_and_cross_agent_defaults() {
        let f = fixture();
        f.app.db().conn.lock().unwrap().execute("INSERT INTO app_settings(key,value,updated_at) VALUES ('vlx-settings',?1,unixepoch())",
            [json!({"agentDefaults":{"codex":{"args":"--model gpt-custom","permissionMode":"read-only","engine":"chat"}}}).to_string()]).unwrap();
        let same = confirm(&f, request(&f, false)).session.unwrap();
        assert_eq!(same.agent_args, f.parent.agent_args); assert_eq!(same.permission_mode, f.parent.permission_mode); assert_eq!(same.engine, "tui");
        let mut req = request(&f, false); req.kind = Some("codex".into());
        let cross = confirm(&f, req).session.unwrap();
        assert_eq!(cross.agent_args.as_deref(), Some("--model gpt-custom")); assert_eq!(cross.permission_mode.as_deref(), Some("read-only")); assert_eq!(cross.engine,"tui");
    }

    fn selector(f: &Fixture) -> AgentSessionRequest {
        AgentSessionRequest { request_id: uuid::Uuid::new_v4().to_string(), context: AgentSessionContext {
            project_id: Some(f.parent.project_id.clone()), group_id: None, active_session_id: Some(f.parent.id.clone()), placement: Placement::Child,
        }, kind: Some("claude".into()), preset_id: None }
    }

    #[test]
    fn selector_retries_return_the_original_snapshot_and_kind_or_preset_is_exclusive() {
        let f = fixture(); let mut req = selector(&f);
        let first = create_agent_session(&f.app, &req).unwrap();
        assert_eq!(first.parent_session_id.as_deref(), Some(f.parent.id.as_str()));
        let duplicate = create_agent_session(&reopen(&f.directory.0), &req).unwrap(); assert_eq!(first.id,duplicate.id);
        req.preset_id = Some("missing".into()); assert!(create_agent_session(&f.app, &req).is_err());
        req.request_id = uuid::Uuid::new_v4().to_string(); assert!(create_agent_session(&f.app, &req).is_err());
        req.kind = None; req.preset_id = None; assert!(create_agent_session(&f.app, &req).is_err());
    }

    #[test]
    fn selector_rejects_stale_projects_groups_parents_and_cross_project_context() {
        let f = fixture(); let mut req = selector(&f);
        req.context.project_id = None; assert!(create_agent_session(&f.app, &req).is_err());
        req.context.project_id = Some("missing".into()); assert!(create_agent_session(&f.app, &req).is_err());
        req.context.project_id = Some(f.parent.project_id.clone()); req.context.group_id = Some("missing".into());
        assert!(create_agent_session(&f.app, &req).is_err()); req.context.group_id = None;
        req.context.active_session_id = Some("temporary-client-session".into()); assert!(create_agent_session(&f.app, &req).is_err());
        req.context.active_session_id = Some(f.parent.id.clone());
        let conn = f.app.db().conn.lock().unwrap();
        let other = repo::create_virtual_project(&conn, "Other").unwrap(); drop(conn);
        req.context.project_id = Some(other.id); assert!(create_agent_session(&f.app, &req).is_err());
        req.context.project_id = Some(f.parent.project_id.clone());
        f.app.db().conn.lock().unwrap().execute("UPDATE sessions SET archived_at=1 WHERE id=?1", [&f.parent.id]).unwrap();
        assert!(create_agent_session(&f.app, &req).is_err());
    }

    #[test]
    fn selector_preset_snapshot_survives_updates_and_deletion_and_empty_path_inherits() {
        let f = fixture(); let preset = repo::create_agent_preset(&f.app.db().conn.lock().unwrap(), "Custom", SessionKind::Claude, Some(""), Some("--model custom"), Some("default"), None).unwrap();
        let mut req = selector(&f); req.kind = None; req.preset_id = Some(preset.id.clone());
        let first = create_agent_session(&f.app, &req).unwrap(); assert_eq!(first.name,"Custom"); assert_eq!(first.agent_path,None);
        let conn = f.app.db().conn.lock().unwrap();
        repo::update_agent_preset(&conn, &preset.id, "Changed", Some("/different"), Some("--model changed"), Some("plan"), None).unwrap();
        repo::delete_agent_preset(&conn, &preset.id).unwrap(); drop(conn);
        let retry = create_agent_session(&f.app, &req).unwrap(); assert_eq!(retry.id,first.id); assert_eq!(retry.agent_args.as_deref(),Some("--model custom"));
        req.request_id = uuid::Uuid::new_v4().to_string(); assert!(create_agent_session(&f.app, &req).is_err());
    }

    #[cfg(unix)]
    fn local_peer(f: &Fixture) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let path=f.directory.0.join("peer.py"); let log=f.directory.0.join("peer-input.jsonl");
        let script=format!("#!/usr/bin/env python3\nimport sys,json\nfor line in sys.stdin:\n value=json.loads(line)\n if value.get('type')=='user':\n  with open({},'a') as out: out.write(line);out.flush()\n",json!(log.to_string_lossy()).to_string());
        std::fs::write(&path,script).unwrap(); std::fs::set_permissions(&path,std::fs::Permissions::from_mode(0o700)).unwrap();
        (path,log)
    }
    #[cfg(unix)]
    fn wait_for_peer(log: &Path) -> Vec<Value> {
        let started=Instant::now();
        loop {
            let text=std::fs::read_to_string(log).unwrap_or_default();
            if !text.is_empty() { return text.lines().map(|line|serde_json::from_str(line).unwrap()).collect(); }
            assert!(started.elapsed()<Duration::from_secs(10),"local peer did not receive its task");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[cfg(unix)]
    #[test]
    fn failed_chat_launch_recovers_in_the_same_session_and_concurrent_lost_ack_retries_write_once() {
        let f=fixture();
        f.app.db().conn.lock().unwrap().execute("UPDATE sessions SET engine='chat' WHERE id=?1",[&f.parent.id]).unwrap();
        repo::set_app_settings(&f.app.db().conn.lock().unwrap(),&std::collections::HashMap::from([("vlx-settings".into(),
            json!({"agentDefaults":{"claude":{"path":"/missing/spawn-test-agent"}}}).to_string())])).unwrap();
        let receipt=confirm(&f,request(&f,true)); assert_eq!(receipt.state,"failed");
        let wt=receipt.session.as_ref().unwrap().worktree_path.clone().unwrap(); assert!(Path::new(&wt).is_dir());
        let (peer,log)=local_peer(&f);
        f.app.db().conn.lock().unwrap().execute("UPDATE sessions SET agent_path=?2 WHERE id=?1",params![receipt.session_id,peer.to_str()]).unwrap();
        let handles:Vec<_>=(0..4).map(|_| { let app=f.app.clone();let id=receipt.request_id.clone();std::thread::spawn(move||resume(&app,&id).unwrap()) }).collect();
        for handle in handles { let result=handle.join().unwrap();assert_eq!(result.state,"complete");assert_eq!(result.session_id,receipt.session_id); }
        assert_eq!(wait_for_peer(&log).len(),1);
        f.app.chat().stop(&f.app,&receipt.session_id).unwrap();
        let recovered=resume(&reopen(&f.directory.0),&receipt.request_id).unwrap();assert_eq!(recovered.state,"complete");
        assert_eq!(wait_for_peer(&log).len(),1);assert_eq!(children(&f).len(),1);assert!(Path::new(&wt).is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn restarted_queued_task_dispatches_the_edited_payload_once() {
        let f=fixture();let receipt=queued_fixture(&f);let (peer,log)=local_peer(&f);
        edit_queued(&f.app,&receipt.session_id,&receipt.message_id,"Edited after enqueue",&[]).unwrap();
        f.app.db().conn.lock().unwrap().execute("UPDATE sessions SET agent_path=?2 WHERE id=?1",params![receipt.session_id,peer.to_str()]).unwrap();
        let app=reopen(&f.directory.0);
        let result=resume(&app,&receipt.request_id).unwrap();assert_eq!(result.state,"complete");assert_eq!(result.session_id,receipt.session_id);
        let rows=wait_for_peer(&log);assert_eq!(rows.len(),1);assert!(rows[0].to_string().contains("Edited after enqueue"));
        assert_eq!(resume(&app,&receipt.request_id).unwrap().state,"complete");
        app.chat().stop(&app,&receipt.session_id).unwrap();assert_eq!(wait_for_peer(&log).len(),1);
    }

    #[test]
    fn workflow_creation_failure_rolls_back_and_delivery_retries_preserve_roles_images_and_defaults() {
        let f=fixture(); let mut req=request(&f,true);
        req.plan_execute=Some(serde_json::from_value(json!({"worktreeMode":"each","plan":{"agent":"claude"},"exec":{"agent":"claude"}})).unwrap());
        req.images=vec![super::super::chat::protocol::ChatImage{mime_type:"image/png".into(),data:"AQID".into()}];
        f.app.db().conn.lock().unwrap().execute("UPDATE sessions SET agent_path='/missing/spawn-plan-agent' WHERE id=?1",[&f.parent.id]).unwrap();
        f.app.db().conn.lock().unwrap().execute_batch("CREATE TRIGGER fail_flow BEFORE INSERT ON plan_execute_runs BEGIN SELECT RAISE(FAIL,'workflow binding failure'); END;").unwrap();
        assert!(super::super::plan_execute::start(&f.app,&req).unwrap_err().contains("workflow binding failure"));
        assert!(children(&f).is_empty());assert_eq!(crate::git::worktree_list(f.parent.cwd.as_deref().unwrap()).unwrap().len(),1);
        f.app.db().conn.lock().unwrap().execute_batch("DROP TRIGGER fail_flow").unwrap();
        let result=super::super::plan_execute::start(&f.app,&req).unwrap();assert_eq!(result["run"]["state"],"blocked");
        let conn=f.app.db().conn.lock().unwrap();
        conn.execute("UPDATE sessions SET agent_args='--model changed --effort low' WHERE id=?1",[&f.parent.id]).unwrap();
        let images:String=conn.query_row("SELECT images FROM plan_execute_attachments WHERE message_id=?1",[format!("msg-{}",req.request_id.as_ref().unwrap())],|r|r.get(0)).unwrap();
        assert!(images.contains("AQID"));drop(conn);
        let retry=super::super::plan_execute::start(&reopen(&f.directory.0),&req).unwrap();assert_eq!(result["planner"]["id"],retry["planner"]["id"]);
        assert_eq!(children(&f).len(),1);assert_eq!(crate::git::worktree_list(f.parent.cwd.as_deref().unwrap()).unwrap().len(),2);
        req.prompt.push_str(" changed");assert!(super::super::plan_execute::start(&f.app,&req).is_err());
    }

    #[test]
    fn confirmed_workflow_snapshot_survives_creation_failure_changed_defaults_and_restart() {
        let f=fixture(); let mut req=request(&f,false);
        req.plan_execute=Some(serde_json::from_value(json!({"plan":{},"exec":{},"splitTasks":true,"worktreeMode":"none"})).unwrap());
        req.images=vec![super::super::chat::protocol::ChatImage{mime_type:"image/png".into(),data:"AQID".into()}];
        let pending=register(&f.app,req.clone()).unwrap();
        assert!(pending.resolved_plan_execute.is_none());
        {
            let conn=f.app.db().conn.lock().unwrap();
            conn.execute("UPDATE sessions SET agent_path='/missing/spawn-plan-agent' WHERE id=?1",[&f.parent.id]).unwrap();
            conn.execute_batch("CREATE TRIGGER fail_snapshot BEFORE INSERT ON plan_execute_runs BEGIN SELECT RAISE(FAIL,'snapshot binding failure'); END;").unwrap();
        }
        let first=decide(&f.app,"other-client",&pending.request_id,true,Some(req.clone())).unwrap();
        assert_eq!(first.state,"failed");assert!(first.error.as_deref().unwrap().contains("snapshot binding failure"));
        let config=first.resolved_plan_execute.as_ref().unwrap();
        assert_eq!(config.plan.agent,Some(SessionKind::Claude));assert_eq!(config.exec.agent,Some(SessionKind::Claude));
        assert_eq!(config.plan.model.as_deref(),Some("sonnet"));assert_eq!(config.exec.effort.as_deref(),Some("high"));assert!(config.split_tasks);
        assert!(children(&f).is_empty());
        let mut altered=req.clone();altered.prompt.push_str(" changed outside confirmation");
        assert!(super::super::plan_execute::start(&f.app,&altered).unwrap_err().contains("identity cannot be changed"));
        {
            let conn=f.app.db().conn.lock().unwrap();
            conn.execute("UPDATE sessions SET agent_args='--model later --effort low' WHERE id=?1",[&f.parent.id]).unwrap();
            conn.execute_batch("DROP TRIGGER fail_snapshot").unwrap();
        }
        let restarted=reopen(&f.directory.0);
        let recovered=read(&restarted,&pending.request_id).unwrap();
        assert_eq!(encoded(&recovered.resolved_plan_execute).unwrap(),encoded(&first.resolved_plan_execute).unwrap());
        assert_eq!(recovered.request.images,req.images);
        let retried=resume(&restarted,&pending.request_id).unwrap();
        assert_eq!(retried.state,"failed");assert_eq!(children(&f).len(),1);
        assert_eq!(encoded(&retried.resolved_plan_execute).unwrap(),encoded(&first.resolved_plan_execute).unwrap());
        // Existing workflows created before the receipt field was introduced expose the run snapshot.
        restarted.db().conn.lock().unwrap().execute("UPDATE spawn_requests SET launch=NULL WHERE id=?1",[&pending.request_id]).unwrap();
        assert_eq!(encoded(&read(&restarted,&pending.request_id).unwrap().resolved_plan_execute).unwrap(),encoded(&first.resolved_plan_execute).unwrap());
        assert_eq!(resume(&restarted,&pending.request_id).unwrap().session_id,retried.session_id);
    }

    fn queued_fixture(f: &Fixture) -> Receipt {
        let result = confirm(f, request(f, false));
        let conn = f.app.db().conn.lock().unwrap();
        conn.execute("UPDATE sessions SET engine='chat' WHERE id=?1", [&result.session_id]).unwrap();
        let payload = serde_json::to_vec(&json!([result.request.prompt,[],"queue"])).unwrap();
        super::super::chat::submissions::claim(&conn, &result.session_id, &result.message_id, &payload).unwrap();
        super::super::chat::submissions::finish(&conn, &result.session_id, &result.message_id, &Ok("queued".into())).unwrap();
        result
    }

    #[test]
    fn queued_edits_survive_restart_and_old_payload_cannot_reclaim_the_message() {
        let f = fixture(); let result = queued_fixture(&f);
        let images = vec![super::super::chat::protocol::ChatImage { mime_type:"image/png".into(), data:"AQID".into() }];
        edit_queued(&f.app, &result.session_id, &result.message_id, "Edited task", &images).unwrap();
        let app = reopen(&f.directory.0); let recovered = read(&app, &result.request_id).unwrap();
        assert_eq!(recovered.request.prompt,"Edited task"); assert_eq!(recovered.request.images.len(),1);
        assert_eq!(recovered.state,"waiting");
        let conn = app.db().conn.lock().unwrap();
        let old = serde_json::to_vec(&json!([result.request.prompt,[],"queue"])).unwrap();
        let new = serde_json::to_vec(&json!(["Edited task",images,"queue"])).unwrap();
        use super::super::chat::submissions::{self,Claim};
        assert!(!submissions::recover_queued(&conn,&result.session_id,&result.message_id,&old).unwrap());
        assert!(submissions::recover_queued(&conn,&result.session_id,&result.message_id,&new).unwrap());
        assert!(matches!(submissions::claim_retry(&conn,&result.session_id,&result.message_id,&new).unwrap(),Claim::New));
        submissions::begin_dispatch(&conn,&result.session_id,&result.message_id).unwrap();
        drop(conn);
        assert!(edit_queued(&app,&result.session_id,&result.message_id,"Too late",&[]).is_err());
        assert!(cancel_queued(&app,&result.session_id,&result.message_id).is_err());
        assert_eq!(read(&app,&result.request_id).unwrap().state,"uncertain");
    }

    #[test]
    fn cancelled_queue_never_reappears_and_database_failure_preserves_the_queue_receipt() {
        let f = fixture(); let result = queued_fixture(&f);
        f.app.db().conn.lock().unwrap().execute_batch("CREATE TRIGGER fail_cancel BEFORE UPDATE OF state ON spawn_requests WHEN NEW.state='cancelled' BEGIN SELECT RAISE(FAIL,'cancel storage failure'); END;").unwrap();
        assert!(cancel_queued_many(&f.app,&result.session_id,&["ordinary-message".into(),result.message_id.clone()]).is_err());
        assert_eq!(read(&f.app,&result.request_id).unwrap().state,"waiting");
        f.app.db().conn.lock().unwrap().execute_batch("DROP TRIGGER fail_cancel").unwrap();
        cancel_queued_many(&f.app,&result.session_id,&["ordinary-message".into(),result.message_id.clone()]).unwrap();
        let recovered = resume(&reopen(&f.directory.0),&result.request_id).unwrap();
        assert_eq!(recovered.state,"cancelled"); assert!(recovered.session.is_some());
        assert_eq!(children(&f).len(),1);
    }

    #[test]
    fn chat_receipt_known_rejection_can_retry_but_pending_and_legacy_errors_cannot() {
        let f = fixture(); let result = confirm(&f,request(&f,false));
        f.app.db().conn.lock().unwrap().execute("UPDATE sessions SET engine='chat' WHERE id=?1",[&result.session_id]).unwrap();
        let send = || core::chat_send(&f.app,&result.session_id,&result.request.prompt,vec![],Some("queue"),Some(&result.message_id));
        assert!(send().unwrap_err().contains("running agent"));
        let raw = submission_outcome(&f.app.db().conn.lock().unwrap(),&result.session_id,&result.message_id).unwrap().unwrap();
        assert!(raw.contains("rejected"));
        assert!(send().unwrap_err().contains("running agent"));
        let conn = f.app.db().conn.lock().unwrap();
        conn.execute("UPDATE chat_submissions SET outcome=NULL WHERE session_id=?1",[&result.session_id]).unwrap(); drop(conn);
        assert_eq!(send().unwrap_err(),"chat_submission_pending");
        let conn = f.app.db().conn.lock().unwrap();
        conn.execute("UPDATE chat_submissions SET outcome=?2 WHERE session_id=?1",params![result.session_id,encoded(&Err::<String,_>("legacy failure")).unwrap()]).unwrap(); drop(conn);
        assert_eq!(send().unwrap_err(),"legacy failure");
        let conn = f.app.db().conn.lock().unwrap();
        conn.execute("UPDATE chat_submissions SET outcome=?2 WHERE session_id=?1",params![result.session_id,encoded(&Ok::<_,String>("sent")).unwrap()]).unwrap(); drop(conn);
        assert_eq!(send().unwrap(),"sent");
        assert!(core::chat_send(&f.app,&result.session_id,"different",vec![],None,Some(&result.message_id)).unwrap_err().contains("another submission"));
    }

    #[test]
    #[cfg(unix)] // Windows hosts refuse shell mode before any receipt exists.
    fn shell_receipts_reuse_results_reject_conflicts_and_preserve_uncertainty() {
        let f = fixture(); let sid = &f.parent.id; let id = format!("msg-{}",uuid::Uuid::new_v4());
        let receipt_id = super::super::chat::shell::submission_id(&id);
        f.app.db().conn.lock().unwrap().execute("UPDATE sessions SET agent_path='/missing/spawn-test-agent' WHERE id=?1",[sid]).unwrap();
        assert!(core::chat_run_shell(&f.app,sid,"echo test",&id).is_err());
        assert!(submission_outcome(&f.app.db().conn.lock().unwrap(),sid,&receipt_id).unwrap().unwrap().contains("rejected"));
        assert!(core::chat_run_shell(&f.app,sid,"echo test",&id).is_err());
        let conn = f.app.db().conn.lock().unwrap();
        conn.execute("UPDATE chat_submissions SET outcome=NULL WHERE session_id=?1 AND id=?2",params![sid,receipt_id]).unwrap(); drop(conn);
        assert_eq!(core::chat_run_shell(&f.app,sid,"echo test",&id).unwrap_err(),"chat_submission_pending");
        let conn = f.app.db().conn.lock().unwrap();
        super::super::chat::submissions::finish(&conn,sid,&receipt_id,&Ok("accepted".into())).unwrap(); drop(conn);
        assert!(core::chat_run_shell(&f.app,sid,"echo test",&id).is_ok());
        assert!(core::chat_run_shell(&f.app,sid,"echo other",&id).unwrap_err().contains("another submission"));
        assert_ne!(receipt_id,id);
    }

    #[test]
    fn selector_defaults_and_empty_preset_args_are_resolved_without_active_agent_inheritance() {
        let f = fixture(); let req = selector(&f);
        repo::set_app_settings(&f.app.db().conn.lock().unwrap(),&std::collections::HashMap::from([("vlx-settings".into(),
            json!({"agentDefaults":{
                "claude":{"args":"--model configured","permissionMode":"plan","engine":"chat"},
                "codex":{"args":"--model cross-default","permissionMode":"read-only","engine":"chat"}
            }}).to_string())])).unwrap();
        let result = create_agent_session(&f.app,&req).unwrap();
        assert_eq!(result.agent_args.as_deref(),Some("--model configured")); assert_eq!(result.permission_mode.as_deref(),Some("plan"));
        assert_eq!(f.parent.engine,"tui");assert_eq!(result.engine,"chat"); assert_eq!(result.name,"Claude 1");
        let mut cross=selector(&f);cross.kind=Some("codex".into());
        let result=create_agent_session(&f.app,&cross).unwrap();
        assert_eq!(result.agent_args.as_deref(),Some("--model cross-default"));
        assert_eq!(result.permission_mode.as_deref(),Some("read-only"));assert_eq!(result.engine,"chat");
        let preset = repo::create_agent_preset(&f.app.db().conn.lock().unwrap(),"Empty",SessionKind::Claude,None,None,None,None).unwrap();
        let mut req = selector(&f); req.kind=None; req.preset_id=Some(preset.id);
        let result = create_agent_session(&f.app,&req).unwrap(); assert_eq!(result.agent_args,None);
        assert_eq!(result.permission_mode.as_deref(),Some("plan"));assert_eq!(result.engine,"chat");
        let preset=repo::create_agent_preset(&f.app.db().conn.lock().unwrap(),"Explicit",SessionKind::Claude,
            Some("/preset/agent"),Some("--model preset"),Some("default"),None).unwrap();
        let mut req=selector(&f);req.kind=None;req.preset_id=Some(preset.id);
        let result=create_agent_session(&f.app,&req).unwrap();
        assert_eq!(result.agent_args.as_deref(),Some("--model preset"));assert_eq!(result.permission_mode.as_deref(),Some("default"));
        assert_eq!(result.agent_path.as_deref(),Some("/preset/agent"));assert_eq!(result.engine,"chat");
    }

    #[test]
    fn ignored_only_files_and_rewritten_branch_history_are_preserved() {
        let f = fixture(); let root=Path::new(f.parent.cwd.as_deref().unwrap());
        git(root,&["config","core.excludesFile",f.directory.0.join("ignore").to_str().unwrap()]);
        std::fs::write(f.directory.0.join("ignore"),"secret\n").unwrap();
        let id=uuid::Uuid::new_v4().to_string(); let wt=owned_worktree(&f.app,&id,root.to_str().unwrap(),"ignored-only",false).unwrap();
        std::fs::write(Path::new(&wt.path).join("secret"),"keep").unwrap();
        assert!(git(Path::new(&wt.path),&["status","--porcelain"]).is_empty());
        assert!(cleanup_worktree(&f.app,&id).is_err());
        std::fs::remove_file(Path::new(&wt.path).join("secret")).unwrap();
        git(Path::new(&wt.path),&["commit","--allow-empty","-m","later"]);
        git(Path::new(&wt.path),&["reset","--hard",&owned(&f,&id).start_commit]);
        assert!(cleanup_worktree(&f.app,&id).unwrap_err().contains("history"));
    }

    #[cfg(unix)]
    #[test]
    fn a_session_using_a_symlink_to_the_worktree_prevents_cleanup() {
        let f=fixture(); let id=uuid::Uuid::new_v4().to_string();
        let wt=owned_worktree(&f.app,&id,f.parent.cwd.as_deref().unwrap(),"linked",false).unwrap();
        let link=f.directory.0.join("alias"); std::os::unix::fs::symlink(&wt.path,&link).unwrap();
        f.app.db().conn.lock().unwrap().execute("UPDATE sessions SET cwd=?2 WHERE id=?1",params![f.parent.id,link.to_str()]).unwrap();
        assert!(cleanup_worktree(&f.app,&id).unwrap_err().contains("references")); assert!(Path::new(&wt.path).is_dir());
    }

    #[test]
    fn selector_sibling_child_and_group_root_use_backend_locations() {
        let f = fixture(); let mut req = selector(&f); req.context.placement = Placement::Sibling;
        assert_eq!(create_agent_session(&f.app, &req).unwrap().parent_session_id, None);
        req.request_id = uuid::Uuid::new_v4().to_string(); req.context.placement = Placement::Child;
        assert_eq!(create_agent_session(&f.app, &req).unwrap().parent_session_id.as_deref(),Some(f.parent.id.as_str()));
        let group = repo::create_group(&f.app.db().conn.lock().unwrap(), &f.parent.project_id, None, "Group").unwrap();
        req.request_id = uuid::Uuid::new_v4().to_string(); req.context.active_session_id = None; req.context.group_id = Some(group.id.clone());
        assert_eq!(create_agent_session(&f.app, &req).unwrap().group_id.as_deref(),Some(group.id.as_str()));
    }
}


/// A queue edit is explicit user input. Persist it before changing the in-memory queue so a
/// restarted first-task delivery uses the edited payload and its matching submission fingerprint.
pub fn edit_queued(app: &AppCtx, session_id: &str, message_id: &str, text: &str, images: &[super::chat::protocol::ChatImage]) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    core::check_images(images)?;
    let mut conn = app.db().conn.lock().unwrap();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let row: Option<(String, String)> = tx.query_row("SELECT id,approved FROM spawn_requests WHERE session_id=?1 AND message_id=?2 AND decision='confirmed' AND state!='cancelled'",
        params![session_id,message_id], |r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e| e.to_string())?;
    let Some((id, raw)) = row else { return Ok(()); };
    let mut request: SpawnRequest = decoded(&raw)?;
    request.prompt = text.into(); request.images = images.to_vec();
    let payload = serde_json::to_vec(&json!([text,images,"queue"])).map_err(|e| e.to_string())?;
    let fingerprint = format!("{:x}", Sha256::digest(&payload));
    let queued = encoded(&Ok::<_,String>("queued"))?;
    let changed = tx.execute("UPDATE chat_submissions SET fingerprint=?3,outcome=?4 WHERE session_id=?1 AND id=?2 AND (outcome IS NULL OR outcome=?4)",
        params![session_id,message_id,fingerprint,queued]).map_err(|e| e.to_string())?;
    if changed != 1 { return Err("The initial task is no longer available for queue editing".into()); }
    tx.execute("UPDATE spawn_requests SET approved=?2,initial_prompt=?3,state='waiting',error=NULL WHERE id=?1",
        params![id,encoded(&request)?,text]).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// Removing a queued initial task is final for that task identity, not a request to delete its session.
pub fn cancel_queued(app: &AppCtx, session_id: &str, message_id: &str) -> Result<(), String> {
    cancel_queued_many(app, session_id, &[message_id.to_string()])
}

pub fn cancel_queued_many(app: &AppCtx, session_id: &str, message_ids: &[String]) -> Result<(), String> {
    let mut conn = app.db().conn.lock().unwrap();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let cancelled = encoded(&Err::<String,_>("The queued initial task was cancelled"))?;
    let queued = encoded(&Ok::<_,String>("queued"))?;
    for message_id in message_ids {
        let id: Option<String> = tx.query_row("SELECT id FROM spawn_requests WHERE session_id=?1 AND message_id=?2 AND decision='confirmed'",
            params![session_id,message_id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
        let Some(id) = id else { continue; };
        let changed = tx.execute("UPDATE chat_submissions SET outcome=?3 WHERE session_id=?1 AND id=?2 AND (outcome IS NULL OR outcome=?4)",
            params![session_id,message_id,cancelled,queued]).map_err(|e| e.to_string())?;
        if changed != 1 { return Err("The initial task is no longer available for queue cancellation".into()); }
        tx.execute("UPDATE spawn_requests SET state='cancelled',error=NULL WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
