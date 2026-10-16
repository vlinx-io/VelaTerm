//! Persistent planning and execution with independent task workflows over the existing chat engine.

use std::io::Read;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub mod menu;
pub mod split;
mod query;
mod review;
use super::chat::protocol::ChatImage;

use crate::{
    command_core as core,
    db::repo,
    host::AppCtx,
    models::{Session, SessionKind},
};

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS plan_execute_runs (
 id TEXT PRIMARY KEY, owner_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
 planner_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
 executor_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
 reviewer_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
 review_status TEXT NOT NULL DEFAULT 'legacy',
 config TEXT NOT NULL, task TEXT NOT NULL, state TEXT NOT NULL, round INTEGER NOT NULL DEFAULT 0,
 summary TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS plan_execute_messages (
 id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES plan_execute_runs(id) ON DELETE CASCADE,
 sender_id TEXT NOT NULL, target_id TEXT NOT NULL, action TEXT NOT NULL, round INTEGER NOT NULL,
 fingerprint TEXT NOT NULL, wire TEXT NOT NULL, origin TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS plan_execute_attachments (
 message_id TEXT PRIMARY KEY REFERENCES plan_execute_messages(id) ON DELETE CASCADE,
 images TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS plan_execute_target ON plan_execute_messages(target_id);
CREATE TABLE IF NOT EXISTS plan_execute_menu_launches (
 run_id TEXT PRIMARY KEY REFERENCES plan_execute_runs(id) ON DELETE CASCADE,
 request TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS plan_execute_proposals (
 run_id TEXT PRIMARY KEY REFERENCES plan_execute_runs(id) ON DELETE CASCADE,
 message_id TEXT NOT NULL UNIQUE, fingerprint TEXT NOT NULL, tasks TEXT NOT NULL,
 state TEXT NOT NULL, confirmation TEXT, notice_id TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS plan_execute_tasks (
 parent_id TEXT NOT NULL REFERENCES plan_execute_runs(id) ON DELETE CASCADE,
 run_id TEXT PRIMARY KEY REFERENCES plan_execute_runs(id) ON DELETE CASCADE,
 position INTEGER NOT NULL, name TEXT NOT NULL, dispatch_id TEXT NOT NULL UNIQUE,
 launch_pending INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS plan_execute_tasks_parent ON plan_execute_tasks(parent_id);
";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RoleConfig {
    pub agent: Option<SessionKind>,
    pub model: Option<String>,
    pub effort: Option<String>,
}

impl RoleConfig {
    fn is_empty(&self) -> bool { self.agent.is_none() && self.model.is_none() && self.effort.is_none() }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    /// None preserves the two-role review protocol of already-created workflows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "RoleConfig::is_empty")]
    pub review: RoleConfig,
    /// Missing on older workflows, whose executors always share the planner's directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_mode: Option<WorktreeMode>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub split_tasks: bool,
    #[serde(default)]
    pub plan: RoleConfig,
    #[serde(default)]
    pub exec: RoleConfig,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WorktreeMode {
    None,
    Shared,
    Each,
}

impl Config {
    fn initial_review_status(&self) -> &'static str {
        match self.review_enabled { Some(true) => "pending", Some(false) => "not_enabled", None => "legacy" }
    }

    fn worktree_mode(&self, legacy_worktree: bool) -> WorktreeMode {
        self.worktree_mode.unwrap_or(if legacy_worktree {
            WorktreeMode::Shared
        } else {
            WorktreeMode::None
        })
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub owner_id: String,
    pub planner_id: String,
    pub executor_id: Option<String>,
    pub reviewer_id: Option<String>,
    pub review_status: String,
    pub config: Config,
    pub task: String,
    pub state: String,
    pub round: u32,
    pub summary: String,
}

pub fn migrate(conn: &rusqlite::Connection) -> Result<(), String> {
    let columns: Vec<String> = conn.prepare("PRAGMA table_info(plan_execute_runs)").map_err(|e|e.to_string())?
        .query_map([], |r|r.get(1)).map_err(|e|e.to_string())?.collect::<Result<_,_>>().map_err(|e|e.to_string())?;
    if !columns.iter().any(|c|c == "reviewer_id") {
        conn.execute_batch("ALTER TABLE plan_execute_runs ADD COLUMN reviewer_id TEXT REFERENCES sessions(id) ON DELETE SET NULL;").map_err(|e|e.to_string())?;
    }
    if !columns.iter().any(|c|c == "review_status") {
        conn.execute_batch("ALTER TABLE plan_execute_runs ADD COLUMN review_status TEXT NOT NULL DEFAULT 'legacy';").map_err(|e|e.to_string())?;
    }
    Ok(())
}

pub fn supported(kind: SessionKind) -> bool {
    matches!(
        kind,
        SessionKind::Claude
            | SessionKind::Codex
            | SessionKind::Zcode
            | SessionKind::Opencode
            | SessionKind::Pi
            | SessionKind::Omp
    )
}

pub(super) fn operation_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn session(app: &AppCtx, id: &str) -> Result<Session, String> {
    repo::get_session(&app.db().conn.lock().unwrap(), id)?
        .filter(|s| s.archived_at.is_none())
        .ok_or_else(|| "The workflow session is missing or archived".into())
}

fn get(app: &AppCtx, id: &str) -> Result<Run, String> {
    app.db().conn.lock().unwrap().query_row(
        "SELECT id,owner_id,planner_id,executor_id,config,task,state,round,summary,reviewer_id,review_status FROM plan_execute_runs WHERE id=?1", [id],
        |r| Ok((r.get::<_,String>(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get::<_,String>(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?)),
    ).map_err(|_| "Workflow not found".to_string()).and_then(|(id,owner_id,planner_id,executor_id,config,task,state,round,summary,reviewer_id,review_status)| {
        Ok(Run { id,owner_id,planner_id,executor_id,reviewer_id,review_status,config:serde_json::from_str(&config).map_err(|e|e.to_string())?,task,state,round,summary })
    })
}

fn brief(run: &Run) -> Value {
    json!({"id":run.id,"ownerId":run.owner_id,"plannerId":run.planner_id,
        "executorId":run.executor_id,"reviewerId":run.reviewer_id,"reviewStatus":run.review_status,
        "config":run.config,"state":run.state,"round":run.round,
        "summary":run.summary.chars().take(2000).collect::<String>(),
        "summaryTruncated":run.summary.chars().count()>2000})
}

fn normalize(app: &AppCtx, config: &RoleConfig, parent: &Session) -> Result<RoleConfig, String> {
    let kind = config.agent.unwrap_or(if supported(parent.kind) {
        parent.kind
    } else {
        SessionKind::Claude
    });
    if !supported(kind) {
        return Err("Planning and execution require chat-capable agents".into());
    }
    let inherited = if kind == parent.kind && (config.model.is_none() || config.effort.is_none()) {
        super::session_settings::resolve(app, parent)?
    } else {
        Default::default()
    };
    let model = config.model.clone().or(inherited.model);
    let effort = config.effort.clone().or(inherited.effort);
    Ok(RoleConfig {
        agent: Some(kind),
        model,
        effort,
    })
}

/// Drafts must remain editable even when a caller supplied an invalid identifier. Validate only when
/// starting, before any session or worktree is created.
fn validate_config(config: &Config) -> Result<(), String> {
    let mut roles = vec![("Planner", &config.plan), ("Executor", &config.exec)];
    if config.review_enabled == Some(true) { roles.push(("Reviewer", &config.review)); }
    for (role, value) in roles {
        let kind = value.agent.ok_or("Missing workflow agent")?;
        if !supported(kind) {
            return Err(format!(
                "{role}: Planning and execution require chat-capable agents"
            ));
        }
        super::launch_options::apply(kind, None, value.model.as_deref(), value.effort.as_deref())
            .map_err(|error| format!("{role}: {error}"))?;
    }
    Ok(())
}

fn message_images(app: &AppCtx, id: &str) -> Result<Vec<ChatImage>, String> {
    let stored: Option<String> = app
        .db()
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT images FROM plan_execute_attachments WHERE message_id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    stored
        .map(|value| serde_json::from_str(&value).map_err(|e| e.to_string()))
        .unwrap_or_else(|| Ok(vec![]))
}

fn save_images(conn: &rusqlite::Connection, id: &str, images: &[ChatImage]) -> Result<(), String> {
    if !images.is_empty() {
        conn.execute(
            "INSERT INTO plan_execute_attachments(message_id,images) VALUES (?1,?2)",
            params![
                id,
                serde_json::to_string(images).map_err(|e| e.to_string())?
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn defaults(app: &AppCtx, parent_id: &str, config: &Config) -> Result<Value, String> {
    let parent = session(app, parent_id)?;
    let plan = normalize(app, &config.plan, &parent)?;
    let mut exec = config.exec.clone();
    if exec.agent.is_none() {
        exec.agent = plan.agent;
    }
    let exec = normalize(app, &exec, &parent)?;
    let mut review = config.review.clone();
    if review.agent.is_none() { review.agent = plan.agent; }
    let review = if config.review_enabled == Some(false) { review } else { normalize(app, &review, &parent)? };
    Ok(json!(Config { plan, exec, review, review_enabled:Some(config.review_enabled.unwrap_or(false)), ..config.clone() }))
}

/// Stable role identities close the creation window before the workflow result is committed.
fn role_id(run_id: &str, role: &str) -> String {
    let hash = Sha256::digest(format!("{run_id}:{role}"));
    uuid::Uuid::from_bytes(hash[..16].try_into().unwrap()).to_string()
}

#[allow(clippy::too_many_arguments)]
fn create_role_bound(
    app: &AppCtx, id: &str, project_id: &str, group_id: Option<&str>, parent: Option<&Session>,
    name: &str, config: &RoleConfig, cwd: Option<&str>, new_worktree: bool,
    inherited_worktree: Option<&str>, inherited_base: Option<&str>,
    bind: impl FnOnce(&rusqlite::Connection, &Session) -> Result<(), String>,
) -> Result<Session, String> {
    let _guard = super::spawn_requests::request_lock(app, id)?;
    if let Some(existing) = repo::get_session(&app.db().conn.lock().unwrap(), id)? { return Ok(existing); }
    let result = (|| -> Result<Session, String> {
        let wt = if new_worktree { Some(super::spawn_requests::owned_worktree(app, id,
            cwd.ok_or("Select a working directory")?, name, true)?) } else { None };
        let directory = wt.as_ref().map(|w| w.path.as_str()).or(cwd);
        let kind = config.agent.ok_or("Missing workflow agent")?;
        let args = super::launch_options::apply(kind, None, config.model.as_deref(), config.effort.as_deref())?;
        let mut conn = app.db().conn.lock().unwrap();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        if let Some(parent) = parent {
            if repo::get_session(&tx, &parent.id)?.is_none_or(|s| s.archived_at.is_some()) {
                return Err("The parent session is missing or archived".into());
            }
        }
        let permission = super::permission_catalog::effective(&tx, kind,
            parent.filter(|p| p.kind == kind).and_then(|p| p.permission_mode.as_deref()))?;
        let created = repo::create_session_identified(&tx, id, project_id, group_id, name, kind, None,
            directory, None, parent.map(|p| p.id.as_str()), wt.as_ref().map(|w| w.path.as_str()).or(inherited_worktree),
            args.as_deref(), permission.as_deref(), wt.as_ref().map(|w| w.base_ref.as_str()).or(inherited_base), None,
            parent.filter(|p| p.kind == kind).and_then(|p| p.agent_path.as_deref()), Some("chat"))?;
        super::session_settings::save(&tx, id, &super::session_settings::Selection {
            model: super::session_settings::clean(config.model.as_deref()), effort: super::session_settings::clean(config.effort.as_deref()),
        }, &json!({}))?;
        bind(&tx, &created)?;
        super::spawn_requests::bind_worktree(&tx, id, id)?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(created)
    })();
    match result {
        Ok(created) => { app.emit(crate::host::TREE_CHANGED, ()); Ok(created) }
        Err(error) => match super::spawn_requests::cleanup_worktree(app, id) {
            Ok(()) => Err(error), Err(cleanup) => Err(format!("{error}; worktree cleanup needs attention: {cleanup}")),
        },
    }
}

/// Called only after the ordinary spawn card has been confirmed (or explicitly skipped).
pub fn start(app: &AppCtx, request: &super::server::SpawnRequest) -> Result<Value, String> {
    let _guard = operation_lock().lock().unwrap();
    let parent = session(app, &request.parent_session_id)?;
    let requested = request
        .plan_execute
        .as_ref()
        .ok_or("Missing planning configuration")?;
    let id = request
        .request_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    uuid::Uuid::parse_str(&id).map_err(|_| "Invalid workflow request ID")?;
    if request.prompt.trim().is_empty() {
        return Err("A workflow needs a task".into());
    }
    core::check_images(&request.images)?;
    let start_fingerprint = format!("start-v1:{:x}", Sha256::digest(serde_json::to_vec(&json!([
        request.parent_session_id,request.prompt,request.images,requested,request.cwd,request.worktree
    ])).map_err(|e| e.to_string())?));
    if let Ok(existing) = get(app, &id) {
        let saved: String = app.db().conn.lock().unwrap().query_row(
            "SELECT fingerprint FROM plan_execute_messages WHERE run_id=?1 AND action='start'", [&id], |r| r.get(0)).map_err(|e| e.to_string())?;
        if saved.starts_with("start-v1:") {
            if saved != start_fingerprint { return Err("This request ID already has a different task or launch configuration".into()); }
            let planner = session(app, &existing.planner_id)?;
            if existing.round == 0 && existing.state == "blocked" { bootstrap(app, &existing)?; }
            return Ok(json!({"run":brief(&get(app,&id)?),"planner":planner}));
        }
    }
    let saved = super::spawn_requests::workflow_start_config(&app.db().conn.lock().unwrap(), &id, request)?;
    let config = if let Some(saved) = saved { saved } else {
        let plan = normalize(app, &requested.plan, &parent)?;
        // The execution role follows the selected agent unless explicitly configured, never a stale CLI flag.
        let mut exec = requested.exec.clone();
        if exec.agent.is_none() { exec.agent = plan.agent; }
        let exec = normalize(app, &exec, &parent)?;
        let mut review = requested.review.clone();
        if review.agent.is_none() { review.agent = plan.agent; }
        let review = if requested.review_enabled != Some(true) { review } else { normalize(app, &review, &parent)? };
        Config { plan, exec, review, review_enabled:Some(requested.review_enabled.unwrap_or(false)), ..requested.clone() }
    };
    validate_config(&config)?;
    super::spawn_requests::save_workflow_config(&app.db().conn.lock().unwrap(), &id, &config)?;
    core::check_images(&request.images)?;
    if let Ok(existing) = get(app, &id) {
        if existing.owner_id != parent.id
            || existing.task != request.prompt
            || message_images(app, &format!("msg-{id}"))? != request.images
            || serde_json::to_value(&existing.config).ok() != serde_json::to_value(&config).ok()
        {
            return Err(
                "This request ID already has a different task or launch configuration".into(),
            );
        }
        let planner = session(app, &existing.planner_id)?;
        if existing.round == 0 && existing.state == "blocked" {
            bootstrap(app, &existing)?;
        }
        return Ok(json!({"run":brief(&get(app,&id)?),"planner":planner}));
    }
    let root = repo::get_project_root(&app.db().conn.lock().unwrap(), &parent.project_id)?;
    let cwd = request.cwd.clone().or(parent.cwd.clone()).or(root);
    let planner = create_role_bound(app, &role_id(&id, "planner"), &parent.project_id,
        parent.group_id.as_deref(), Some(&parent), "Plan · Execute", &config.plan, cwd.as_deref(),
        config.worktree_mode(request.worktree == Some(true)) != WorktreeMode::None, None, None,
        |tx, planner| {
            let task = format!("{}\n\nWorkflow ID: {id}\nRole: planner\nWorking directory: {}\n\nUser task:\n{}",
                review::protocol(&config), planner.cwd.as_deref().unwrap_or(""), request.prompt);
            let message_id = format!("msg-{id}");
            let origin = json!({"sessionId":parent.id,"name":parent.name,"agent":parent.kind,"role":"initiator","runId":id,"round":0});
            let wire = format!("[VelaTerm message {message_id}]\n{origin}\n\n{task}").trim_end().to_owned();
            tx.execute("INSERT INTO plan_execute_runs(id,owner_id,planner_id,config,task,state,review_status) VALUES (?1,?2,?3,?4,?5,'planning',?6)",
                params![id,parent.id,planner.id,serde_json::to_string(&config).map_err(|e|e.to_string())?,request.prompt,config.initial_review_status()]).map_err(|e|e.to_string())?;
            tx.execute("INSERT INTO plan_execute_messages(id,run_id,sender_id,target_id,action,round,fingerprint,wire,origin) VALUES (?1,?2,?3,?4,'start',0,?7,?5,?6)",
                params![message_id,id,parent.id,planner.id,wire,origin.to_string(),start_fingerprint]).map_err(|e|e.to_string())?;
            save_images(tx, &message_id, &request.images)
        })?;
    bootstrap(app, &get(app, &id)?)?;
    Ok(json!({"run":brief(&get(app,&id)?),"planner":planner}))
}

fn bootstrap(app: &AppCtx, run: &Run) -> Result<(), String> {
    let (id, wire): (String, String) = app
        .db()
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT id,wire FROM plan_execute_messages WHERE run_id=?1 AND action='start'",
            [&run.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    // Retrying uses the original session and submission ID, including uncertain-delivery protection.
    let outcome = deliver(app, run, &run.planner_id, &wire, &id);
    let (state, summary) = match outcome {
        Ok(_) => ("planning", String::new()),
        Err(error) => ("blocked", error),
    };
    app.db()
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE plan_execute_runs SET state=?2,summary=?3 WHERE id=?1",
            params![run.id, state, summary],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub session_id: String,
    /// A workflow ID for existing actions; a session reference (or empty for self) for read-only list.
    pub run_id: String,
    pub action: String,
    #[serde(default)]
    pub message_id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub round: u32,
}

fn fingerprint(req: &Request) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&json!([req.session_id, req.action, req.round, req.text])).unwrap()
        )
    )
}

fn health(app: &AppCtx, id: &str) -> Value {
    let snapshot = app.chat().snapshot_window(id, Some(&Default::default()));
    let (_, states) = super::status_watch::snapshot();
    let current = session(app, id).ok();
    json!({"sessionId":id,"cwd":current.as_ref().and_then(|s|s.cwd.as_deref()),
        "worktreePath":current.as_ref().and_then(|s|s.worktree_path.as_deref()),
        "running":snapshot.running,"state":states.iter().find(|s|s.session_id==id).map(|s|s.state),
        "permissionPending":!snapshot.permissions.is_empty(),"queued":snapshot.queue.len(),
        "error":snapshot.rows.iter().rev().take_while(|r| !matches!(r, super::chat::engine::ChatRow::User{..})).find_map(|r| match r {super::chat::engine::ChatRow::Error{message,..}=>Some(message),_=>None})})
}

pub fn action(app: &AppCtx, req: &Request) -> Result<Value, String> {
    if req.action == "report" {
        return Err("Submit execution reports with vtell --report".into());
    }
    let _guard = operation_lock().lock().unwrap();
    if req.action == "list" {
        return query::list(app, &req.session_id, &req.run_id);
    }
    apply_action(app, req)
}

fn status(app: &AppCtx, run: &Run) -> Result<Value, String> {
    let conn = app.db().conn.lock().unwrap();
    let mut query=conn.prepare("SELECT m.id,m.target_id,m.action,m.round,s.outcome FROM plan_execute_messages m LEFT JOIN chat_submissions s ON s.session_id=m.target_id AND s.id=m.id WHERE m.run_id=?1 ORDER BY m.rowid DESC LIMIT 5").map_err(|e|e.to_string())?;
    let receipts=query.query_map([&run.id],|r|Ok(json!({"messageId":r.get::<_,String>(0)?,"targetSessionId":r.get::<_,String>(1)?,"action":r.get::<_,String>(2)?,"round":r.get::<_,u32>(3)?,"receipt":r.get::<_,Option<String>>(4)?})))
        .map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
    drop(query);
    drop(conn);
    Ok(json!({"planner":health(app,&run.planner_id),"executor":run.executor_id.as_deref().map(|id|health(app,id)),
        "reviewer":run.reviewer_id.as_deref().map(|id|health(app,id)),"run":brief(run),"recentDeliveries":receipts,
        "parentRunId":split::parent_id(app,&run.id)?,"tasks":split::tasks(app,&run.id)?,
        "proposal":if run.config.split_tasks {split::read(app,&run.id).ok()} else {None}}))
}

/// Resolve a report from its real executor; a target override must name that workflow's planner.
pub fn report(
    app: &AppCtx,
    req: &super::tell::Request,
    target: Option<&str>,
) -> Result<Value, String> {
    let round = req
        .round
        .ok_or("Reports require the current --round N from vflow status")?;
    let _guard = operation_lock().lock().unwrap();
    let run_id = {
        let conn = app.db().conn.lock().unwrap();
        let saved: Option<String> = conn
            .query_row(
                "SELECT run_id FROM plan_execute_messages WHERE id=?1",
                [&req.message_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(id) = saved {
            id
        } else {
            let mut query = conn.prepare("SELECT id FROM plan_execute_runs WHERE executor_id=?1 AND state NOT IN ('completed','stopped')").map_err(|e|e.to_string())?;
            let ids = query
                .query_map([&req.session_id], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            if ids.len() != 1 {
                return Err("This session must be the executor of one active workflow".into());
            }
            ids[0].clone()
        }
    };
    let run = get(app, &run_id)?;
    if run.executor_id.as_deref() != Some(req.session_id.as_str()) {
        return Err("Only the workflow executor can submit a report".into());
    }
    if target.is_some_and(|id| id != run.planner_id && run.reviewer_id.as_deref() != Some(id)) {
        return Err("A report must target this workflow's planning session or review session".into());
    }
    apply_action(
        app,
        &Request {
            session_id: req.session_id.clone(),
            run_id,
            action: "report".into(),
            message_id: req.message_id.clone(),
            text: req.text.clone(),
            round,
        },
    )
}

fn apply_action(app: &AppCtx, req: &Request) -> Result<Value, String> {
    let mut run = get(app, &req.run_id)?;
    split::check_action(app, &run, &req.action)?;
    let is_plan = req.session_id == run.planner_id;
    let is_exec = run.executor_id.as_deref() == Some(req.session_id.as_str());
    let is_review = run.reviewer_id.as_deref() == Some(req.session_id.as_str());
    if !is_plan && !is_exec && !is_review && req.session_id != run.owner_id {
        return Err("This session does not belong to the workflow".into());
    }
    session(app, &req.session_id)?;
    if req.action == "status" {
        return status(app, &run);
    }
    if req.action == "read-report" {
        let conn = app.db().conn.lock().unwrap();
        let (wire, origin, round): (String,String,u32) = conn.query_row(
            "SELECT wire,origin,round FROM plan_execute_messages WHERE id=?1 AND run_id=?2 AND action='report'",
            params![req.message_id,run.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|"Execution report not found".to_owned())?;
        return Ok(json!({"messageId":req.message_id,"runId":run.id,"round":round,"wire":wire,"origin":serde_json::from_str::<Value>(&origin).map_err(|e|e.to_string())?}));
    }
    if req.action == "propose" { return split::propose(app, &run, req); }
    if req.action == "stop" {
        if run.state == "completed" {
            return Err("This workflow has already completed".into());
        }
        app.db()
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE plan_execute_runs SET state='stopped' WHERE id=?1",
                [&run.id],
            )
            .map_err(|e| e.to_string())?;
        let mut failures = Vec::new();
        let mut runs = vec![run.clone()];
        for id in split::task_ids(app,&run.id)? { runs.push(get(app,&id)?); }
        let mut peers = Vec::new();
        // Stopping one task must not interrupt the planner shared by its siblings.
        if split::parent_id(app,&run.id)?.is_none() {
            peers.push(run.planner_id.clone());
            if let Some(id) = &run.reviewer_id { peers.push(id.clone()); }
        }
        for item in &runs {
            app.db().conn.lock().unwrap().execute("UPDATE plan_execute_runs SET state='stopped' WHERE id=?1 AND state!='completed'",[&item.id]).map_err(|e|e.to_string())?;
            if item.state != "completed" {
                if let Some(id) = &item.executor_id { peers.push(id.clone()); }
            }
        }
        app.db().conn.lock().unwrap().execute("UPDATE plan_execute_proposals SET state='stopped' WHERE run_id=?1 AND state='pending'",[&run.id]).map_err(|e|e.to_string())?;
        let mut cleanup_peers = peers.clone();
        if run.config.review_enabled.is_some() && split::parent_id(app,&run.id)?.is_some() {
            cleanup_peers.push(run.planner_id.clone());
            if let Some(id) = &run.reviewer_id { cleanup_peers.push(id.clone()); }
        }
        cleanup_peers.sort(); cleanup_peers.dedup();
        for id in &cleanup_peers {
            for item in app
                .chat()
                .snapshot_window(id, Some(&Default::default()))
                .queue
            {
                let owned=app.db().conn.lock().unwrap().query_row(
                    "SELECT 1 FROM plan_execute_messages WHERE id=?1 AND (run_id=?2 OR run_id IN (SELECT run_id FROM plan_execute_tasks WHERE parent_id=?2)) AND target_id=?3",
                    params![item.id,run.id,id],|_|Ok(())).optional().map_err(|e|e.to_string())?.is_some();
                if owned {
                    if let Err(error) = core::chat_queue_remove(app, id, &item.id) {
                        failures.push(format!("{id}: {error}"));
                    }
                }
            }
            if peers.contains(id) && app.chat().turn_in_progress(id) {
                if let Err(error) = core::chat_interrupt(app, id) {
                    failures.push(format!("{id}: {error}"));
                }
            }
        }
        app.emit("plan-execute://proposal",json!({"runId":run.id,"resolved":true}));
        return Ok(
            json!({"run":brief(&get(app,&run.id)?),"interruptErrors":failures,
            "note":"Further workflow dispatches are disabled; an interrupted agent may still be finishing its current operation."}),
        );
    }

    super::tell::validate_message(&req.message_id, &req.text)?;
    let ordinary = app
        .db()
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT 1 FROM session_tells WHERE id=?1",
            [&req.message_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if ordinary.is_some() {
        return Err("This message ID already belongs to an ordinary session message".into());
    }
    let saved: Option<(String,String,String)> = app.db().conn.lock().unwrap().query_row(
        "SELECT fingerprint,target_id,wire FROM plan_execute_messages WHERE id=?1 AND run_id=?2",
        params![req.message_id,run.id], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).optional().map_err(|e|e.to_string())?;
    if let Some((hash, target, wire)) = saved {
        if hash != fingerprint(req) {
            return Err("This message ID already belongs to a different request".into());
        }
        if run.state == "stopped" {
            return Err("This workflow was stopped; pending actions cannot be replayed".into());
        }
        if req.round < run.round && !review::pending_progress(app,&run,&req.message_id)? {
            return Err("This message belongs to an earlier workflow round".into());
        }
        if let Some(parent) = split::parent_id(app,&run.id)? { split::refresh_parent(app,&parent)?; }
        return review::deliver_handoff(app, &run, &target, &wire, &req.message_id);
    }
    let reopening = req.action == "dispatch" && run.state == "completed"
        && split::parent_id(app,&run.id)?.is_some()
        && run.config.review_enabled.is_some() && (is_review || is_plan);
    if run.state == "stopped" || (run.state == "completed" && !reopening) {
        return Err("This workflow has already ended".into());
    }
    let (target, state, role) = match req.action.as_str() {
        "dispatch"
            if ((is_plan && (run.state == "planning" || run.state == "summarizing" || run.state == "blocked" || reopening
                    || (run.config.review_enabled.is_none() && run.state == "reviewing")))
                || (is_review && (matches!(run.state.as_str(), "reviewing" | "blocked") || reopening) && review::has_report(app,&run)?))
                && req.round == run.round + 1 =>
        {
            if run.executor_id.is_none() {
                let planner = session(app, &run.planner_id)?;
                let name = split::task_name(app, &run.id)?.unwrap_or_else(|| "Execute".into());
                let executor = create_role_bound(app, &role_id(&run.id, "executor"), &planner.project_id,
                    planner.group_id.as_deref(), Some(&planner), &name, &run.config.exec, planner.cwd.as_deref(),
                    run.config.worktree_mode == Some(WorktreeMode::Each), None, None, |tx, executor| {
                        tx.execute("UPDATE plan_execute_runs SET executor_id=?2 WHERE id=?1 AND executor_id IS NULL",
                            params![run.id,executor.id]).map_err(|e| e.to_string())?;
                        Ok(())
                    })?;
                run.executor_id = Some(executor.id);
            }
            (run.executor_id.clone().unwrap(), "executing", if is_review {"review"} else {"plan"})
        }
        // A blocker records an incomplete handoff; it must not prevent the assigned executor from
        // completing that handoff after an interruption or reporting a result that is ready for review.
        "report"
            if is_exec
                && (matches!(run.state.as_str(), "executing" | "blocked")
                    || (run.config.review_enabled.is_some() && matches!(run.state.as_str(), "reviewing" | "summarizing")))
                && req.round == run.round =>
        {
            if run.config.review_enabled == Some(true) {
                review::ensure_reviewer(app, &mut run)?;
                (run.reviewer_id.clone().unwrap(), "reviewing", "exec")
            } else {
                (run.planner_id.clone(), if run.config.review_enabled == Some(false) {"summarizing"} else {"reviewing"}, "exec")
            }
        }
        "accept" if is_plan && run.config.review_enabled.is_none() && (run.state == "reviewing" || (run.config.split_tasks && run.state == "blocked")) && req.round == run.round => {
            (run.owner_id.clone(), "completed", "plan")
        }
        "accept" if is_review && matches!(run.state.as_str(), "reviewing" | "blocked") && req.round == run.round
            && (run.config.split_tasks || review::has_report(app,&run)?) => {
            (run.planner_id.clone(), "summarizing", "review")
        }
        "finish" if is_plan && run.config.review_enabled.is_some() && req.round == run.round
            && matches!(run.state.as_str(), "summarizing" | "blocked")
            && (run.config.split_tasks || review::has_report(app,&run)?)
            && (run.config.review_enabled == Some(false) || run.review_status == "passed") => {
            (run.owner_id.clone(), "completed", "plan")
        }
        "block" if (is_plan || is_exec || (is_review && matches!(run.state.as_str(),"reviewing"|"blocked")
                && (run.config.split_tasks || review::has_report(app,&run)?))) && req.round == run.round => (
            if is_exec || is_review {
                run.planner_id.clone()
            } else {
                run.owner_id.clone()
            },
            "blocked",
            if is_exec { "exec" } else if is_review { "review" } else { "plan" },
        ),
        _ => {
            return Err(
                "The role, action or round does not match the current workflow state".into(),
            )
        }
    };
    let sender = session(app, &req.session_id)?;
    let origin = json!({"sessionId":sender.id,"name":sender.name,"agent":sender.kind,"role":role,"runId":run.id,"round":req.round});
    let text = if req.action == "dispatch" && run.round == 0 {
        format!(
            "{}\n\nWorkflow ID: {}\nRole: executor\nRound: {}\nWorking directory: {}\n\nImplementation task:\n{}",
            review::protocol(&run.config),
            run.id,
            req.round,
            session(app, &target)?.cwd.as_deref().unwrap_or(""),
            req.text
        )
    } else if req.action == "report" && run.config.review_enabled == Some(true) {
        format!("{}\n\nWorkflow ID: {}\nRole: reviewer\nRound: {}\nExecutor directory: {}\nOriginal task:\n{}\n\nExecution report:\n{}",
            review::protocol(&run.config),run.id,req.round,session(app,run.executor_id.as_deref().unwrap())?.cwd.as_deref().unwrap_or(""),run.task,req.text)
    } else if req.action == "finish" && run.config.review_enabled == Some(false) {
        format!("No independent review was performed. Verification results are reported by Execute.\n\n{}",req.text)
    } else {
        req.text.clone()
    };
    let wire = format!(
        "[VelaTerm message {}]\n{}\n\n{}",
        req.message_id, origin, text
    )
    .trim_end()
    .to_owned();
    // The executor receives the user's original references once, together with its first assignment.
    let images = if (req.action == "dispatch" && run.round == 0) || (req.action == "report" && run.config.review_enabled == Some(true) && run.round == 1) {
        message_images(app, &format!("msg-{}", run.id))?
    } else {
        vec![]
    };
    {
        let mut conn = app.db().conn.lock().unwrap();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO plan_execute_messages(id,run_id,sender_id,target_id,action,round,fingerprint,wire,origin) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![req.message_id,run.id,req.session_id,target,req.action,req.round,fingerprint(req),wire,origin.to_string()]).map_err(|e|e.to_string())?;
        save_images(&tx, &req.message_id, &images)?;
        let review_status = if run.config.review_enabled == Some(false) { "not_enabled" }
            else if run.config.review_enabled.is_none() { "legacy" }
            else if req.action == "accept" { "passed" }
            else if req.action == "dispatch" && is_review { "changes_requested" }
            else if req.action == "dispatch" || req.action == "report" { "pending" }
            else if req.action == "block" && is_review { "blocked" }
            else { &run.review_status };
        tx.execute(
            "UPDATE plan_execute_runs SET executor_id=?2,state=?3,round=?4,summary=?5,review_status=?6 WHERE id=?1",
            params![run.id, run.executor_id, state, req.round,
                if req.action == "finish" && run.config.review_enabled == Some(false) { &text } else { &req.text }, review_status],
        )
        .map_err(|e| e.to_string())?;
        if run.config.review_enabled == Some(true) && (req.action == "report" || is_review) {
            review::save_plan_notice(&tx, &run, req, &origin)?;
        }
        tx.commit().map_err(|e| e.to_string())?;
    }
    if let Some(parent) = split::parent_id(app,&run.id)? { split::refresh_parent(app,&parent)?; }
    run = get(app, &run.id)?;
    review::deliver_handoff(app, &run, &target, &wire, &req.message_id)
}

fn deliver(app: &AppCtx, run: &Run, target: &str, wire: &str, id: &str) -> Result<Value, String> {
    // A menu-launched workflow is owned by its planner. Final audits and failure notices stay in its
    // ledger and conversation output rather than prompting the planner to review its own conclusion.
    if run.owner_id == run.planner_id && target == run.planner_id {
        let (action, sender): (String, String) = app
            .db()
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT action,sender_id FROM plan_execute_messages WHERE id=?1 AND run_id=?2",
                params![id, run.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        if sender == run.planner_id && matches!(action.as_str(), "accept" | "finish" | "block" | "notice") {
            return Ok(
                json!({"delivery":"recorded","messageId":id,"targetSessionId":target,"run":brief(run)}),
            );
        }
    }
    let target_session = session(app, target)?;
    // The initiating session can be a terminal. Its final report stays in the workflow and planner.
    if target_session.engine != "chat" {
        return Ok(
            json!({"delivery":"retained","reason":"The initiating session is not a chat session","run":brief(&run)}),
        );
    }
    let outcome =
        // Workflow handoffs always wait for the recipient's turn to end: a dispatch or a report is a new
        // assignment, not a remark on work already under way.
        super::tell::deliver_with_images(app, target, wire, id, message_images(app, id)?, "queue")?;
    Ok(json!({"delivery":outcome,"messageId":id,"targetSessionId":target,"run":brief(&run)}))
}

/// Workflow peers remain available while the user opens another pane.
pub fn active(app: &AppCtx, id: &str) -> bool {
    app.db().conn.lock().unwrap().query_row(
        "SELECT 1 FROM plan_execute_runs WHERE (planner_id=?1 OR executor_id=?1 OR reviewer_id=?1) AND state NOT IN ('completed','stopped') LIMIT 1",
        [id],|_|Ok(())).optional().ok().flatten().is_some()
}

/// Observe turn boundaries, not a polling timer. A missing report is a blocker, never a pass.
pub fn observe(app: &AppCtx, id: &str, event: &Value) {
    let kind = event["type"].as_str().unwrap_or("");
    if !matches!(kind, "turnCompleted" | "turnInterrupted" | "exited") || event["released"] == true
    {
        return;
    }
    let app = app.clone();
    let id = id.to_owned();
    let interrupted = kind != "turnCompleted";
    std::thread::spawn(move || {
        let _guard = operation_lock().lock().unwrap();
        let ids: Vec<String> = {
            let db = app.db();
            let conn = db.conn.lock().unwrap();
            let mut query = match conn.prepare("SELECT id FROM plan_execute_runs WHERE (executor_id=?1 AND state='executing') OR (reviewer_id=?1 AND state='reviewing') OR (planner_id=?1 AND state IN ('planning','reviewing','summarizing'))") {
                Ok(query) => query, Err(_) => return,
            };
            let rows = match query.query_map([&id], |r|r.get(0)) { Ok(rows) => rows, Err(_) => return };
            rows.filter_map(Result::ok).collect()
        };
        for run_id in ids {
            if let Ok(run) = get(&app, &run_id) { observe_run(&app, &id, &run, interrupted); }
        }
    });
}

fn observe_run(app: &AppCtx, id: &str, run: &Run, interrupted: bool) {
        if run.config.review_enabled.is_some() && id == run.planner_id && run.state == "reviewing" { return; }
        if app.chat().turn_in_progress(&id) && !interrupted {
            return;
        }
        let snapshot = app.chat().snapshot_window(&id, Some(&Default::default()));
        // A report may have already queued the next round while the previous turn is ending.
        if !interrupted
            && snapshot
                .queue
                .iter()
                .any(|q| app.db().conn.lock().unwrap().query_row(
                    "SELECT 1 FROM plan_execute_messages WHERE run_id=?1 AND target_id=?2 AND round=?3 AND wire=?4",
                    params![run.id,id,run.round,q.text], |_| Ok(())).optional().ok().flatten().is_some())
        {
            return;
        }
        if run.executor_id.as_deref() == Some(&id) || run.reviewer_id.as_deref() == Some(&id) {
            let wires = snapshot.rows.iter().rev().filter_map(|row| match row {
                super::chat::engine::ChatRow::User { text, .. } => Some(text), _=>None,
            });
            let mut relevant = wires;
            let current = if run.reviewer_id.as_deref() == Some(id) { relevant.next().is_some_and(|wire|app.db().conn.lock().unwrap().query_row(
                "SELECT 1 FROM plan_execute_messages WHERE run_id=?1 AND target_id=?2 AND round=?3 AND wire=?4",
                params![run.id,id,run.round,wire],|_|Ok(())).optional().ok().flatten().is_some()) } else {
                relevant.any(|wire|app.db().conn.lock().unwrap().query_row(
                    "SELECT 1 FROM plan_execute_messages WHERE run_id=?1 AND target_id=?2 AND round=?3 AND wire=?4",
                    params![run.id,id,run.round,wire],|_|Ok(())).optional().ok().flatten().is_some())
            };
            if !current && !interrupted {
                return;
            }
        }
        let summary = if interrupted {
            "A workflow session stopped before submitting its result. Inspect its conversation before resuming."
        } else {
            "A workflow turn ended without a dispatch, report or acceptance. Inspect its conversation and resolve the missing handoff before resuming."
        };
        let _ = app.db().conn.lock().unwrap().execute(
            "UPDATE plan_execute_runs SET state='blocked',summary=?2,review_status=CASE WHEN reviewer_id=?3 THEN 'blocked' ELSE review_status END WHERE id=?1",
            params![run.id, summary,id],
        );
        crate::diagnostics::record(
            "WARN",
            "plan_execute_handoff",
            json!({"runId":run.id,"sessionId":id,"round":run.round,"status":"blocked"}),
        );
        // The platform notice is distinct from an executor's report and never implies acceptance.
        let target = if run.executor_id.as_deref() == Some(&id) || run.reviewer_id.as_deref() == Some(&id) {
            &run.planner_id
        } else {
            &run.owner_id
        };
        let message_id = format!("msg-{}", uuid::Uuid::new_v4());
        let origin = json!({"sessionId":id,"name":"VelaTerm","agent":"terminal","role":"system","runId":run.id,"round":run.round});
        let wire=format!("[VelaTerm message {message_id}]\n{origin}\n\nWorkflow {} is blocked: {summary}\nSession: {id}\nUse vflow status {} before resuming. Blocking does not prevent ordinary vtell messages. The executor can submit the current round with vtell --report without another dispatch. Use a new planner dispatch for a new assignment.",run.id,run.id);
        let inserted=app.db().conn.lock().unwrap().execute(
            "INSERT INTO plan_execute_messages(id,run_id,sender_id,target_id,action,round,fingerprint,wire,origin) VALUES (?1,?2,?3,?4,'notice',?5,'system',?6,?7)",
            params![message_id,run.id,id,target,run.round,wire,origin.to_string()]);
        if inserted.is_ok() {
            let _ = deliver(&app, &run, target, &wire, &message_id);
        }
}

pub fn handle(app: AppCtx, mut request: crate::diagnostics::HttpRequest, token: String) {
    let started = Instant::now();
    let request_id = request.id().to_owned();
    let authorized = request
        .headers()
        .iter()
        .any(|h| h.field.equiv("Authorization") && h.value.as_str() == format!("Bearer {token}"));
    let mut body = String::new();
    let result = if !authorized {
        Err((401, "Authentication required".to_owned()))
    } else if request.method() != &tiny_http::Method::Post {
        Err((405, "Use POST".to_owned()))
    } else if request
        .as_reader()
        .take(131073)
        .read_to_string(&mut body)
        .is_err()
        || body.len() > 131072
    {
        Err((413, "Request too large".to_owned()))
    } else {
        serde_json::from_str::<Request>(&body)
            .map_err(|e| e.to_string())
            .and_then(|req| action(&app, &req))
            .map_err(|e| (400, e))
    };
    let (status, value) = match result {
        Ok(result) => (200, json!({"result":result})),
        Err((code, error)) => (code, json!({"error":error})),
    };
    let _ = request.respond(
        tiny_http::Response::from_string(value.to_string())
            .with_status_code(status)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
            ),
    );
    crate::diagnostics::record(
        if status == 200 { "INFO" } else { "WARN" },
        "plan_execute",
        json!({"requestId":request_id,"statusCode":status,"durationMs":started.elapsed().as_millis() as u64}),
    );
}

pub fn run_cli(args: &[String]) -> ! {
    let result = (|| -> Result<Value, String> {
        let rest = &args[args.len().min(2)..];
        if rest.is_empty() || rest[0] == "--help" {
            return Ok(
                json!({"usage":"vflow list [session]\nvflow status|stop|propose|dispatch|accept|finish|block|read-report <run-id> [--round N --message-id msg-UUID] < message.txt","note":"list is read-only and defaults to the calling session. It returns related workflows and direct child sessions with saved properties; session accepts an ID, ID prefix, exact name or unique name substring. propose reads a JSON object with tasks [{name,prompt,config:{agent,model,effort}}]. It requires user confirmation before execution. Other mutation text is read from stdin. Reuse the exact message ID, round and text when retrying."}),
            );
        }
        if rest[0] == "list" {
            let caller = std::env::var("VLX_SESSION_ID").map_err(|_| "Run inside a VelaTerm session")?;
            let req = query::list_request(&caller, &rest[1..])?;
            return super::tell::post_local("plan-execute", &serde_json::to_value(req).unwrap());
        }
        if !matches!(
            rest[0].as_str(),
            "status" | "stop" | "propose" | "dispatch" | "accept" | "finish" | "block" | "read-report"
        ) {
            return Err(if rest[0] == "report" {
                "Use vtell --report --round N to submit an execution report"
            } else {
                "Unknown workflow action"
            }
            .into());
        }
        let mut req = Request {
            session_id: std::env::var("VLX_SESSION_ID")
                .map_err(|_| "Run inside a VelaTerm session")?,
            run_id: rest.get(1).ok_or("Missing workflow ID")?.clone(),
            action: rest[0].clone(),
            round: 0,
            message_id: String::new(),
            text: String::new(),
        };
        let mut i = 2;
        while i < rest.len() {
            let value = rest.get(i + 1).ok_or("Missing option value")?;
            match rest[i].as_str() {
                "--round" => req.round = value.parse().map_err(|_| "Invalid round")?,
                "--message-id" => req.message_id = value.clone(),
                _ => return Err("Unknown option".into()),
            }
            i += 2;
        }
        if !matches!(req.action.as_str(), "status" | "stop" | "read-report") {
            std::io::stdin()
                .take(65537)
                .read_to_string(&mut req.text)
                .map_err(|_| "Cannot read task text")?;
        }
        super::tell::post_local("plan-execute", &serde_json::to_value(req).unwrap())
    })();
    match result {
        Ok(value) => {
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
            std::process::exit(0)
        }
        Err(error) => {
            eprintln!("vflow: {error}");
            std::process::exit(1)
        }
    }
}
