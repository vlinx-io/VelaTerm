//! A local-agent audit workflow. Credentials and model execution remain with the user's CLI.
mod native;
mod scope;
#[cfg(test)]
mod tests;
mod upstream;
mod workflow;

use crate::{db::repo, host::AppCtx, models::SessionKind};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};

pub type Result<T> = std::result::Result<T, String>;

/// Internal stdio verifier. No application state, credentials, or services are initialized.
pub fn run_draft_verifier(args: &[String]) {
    let result = if args.len() == 5 {
        native::verify_draft(std::path::Path::new(&args[2]), std::path::Path::new(&args[3]), &args[4])
    } else {
        Err("security_upstream_invalid".into())
    };
    let value = result.unwrap_or_else(|_| json!({"ok":false,"error":"security_draft_verification_unavailable"}));
    println!("{value}");
}
static ACTIVE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
fn active() -> &'static Mutex<HashSet<String>> {
    ACTIVE.get_or_init(Default::default)
}
pub fn session_active(id: &str) -> bool {
    active().lock().unwrap().contains(id)
}
pub fn cancel_session(app: &AppCtx, session_id: &str) -> Result<()> {
    if !session_active(session_id) {
        return Ok(());
    }
    let id: Option<String> = {
        use rusqlite::OptionalExtension;
        app.db().conn.lock().unwrap().query_row(
            "SELECT id FROM security_runs WHERE session_id=?1 AND status IN ('running','waiting')",
            [session_id], |row| row.get(0)).optional().map_err(|e| e.to_string())?
    };
    if let Some(id) = id {
        dispatch(app, "security_cancel", &json!({"id":id}))?;
    }
    Ok(())
}

fn now() -> i64 {
    crate::memory::now()
}

pub fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS security_runs (
        id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        session_id TEXT NOT NULL, status TEXT NOT NULL, created_at INTEGER NOT NULL, data TEXT NOT NULL
    ); CREATE INDEX IF NOT EXISTS security_runs_project ON security_runs(project_id,created_at);")
        .map_err(|e| e.to_string())
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub project_id: String,
    pub agent: String,
    pub scope: String,
    #[serde(default)]
    pub path: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
}
fn default_language() -> String {
    "en".into()
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub project_id: String,
    pub session_id: String,
    pub agent: String,
    pub root: String,
    pub scope: String,
    pub path: String,
    pub status: String,
    pub phase: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub files: Vec<scope::File>,
    pub excluded: Vec<String>,
    pub reviewed: Vec<String>,
    pub findings: Vec<workflow::Finding>,
    pub threat_model: String,
    pub steps: Vec<Value>,
    pub gaps: Vec<String>,
    pub error: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub upstream: Option<Value>,
}
fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or_else(|| format!("security_invalid:{key}"))
}
fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Run> {
    let text: String = row.get(0)?;
    serde_json::from_str(&text).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}
fn get(app: &AppCtx, id: &str) -> Result<Run> {
    app.db()
        .conn
        .lock()
        .unwrap()
        .query_row("SELECT data FROM security_runs WHERE id=?1", [id], read)
        .map_err(|_| "security_not_found".into())
}
fn save(app: &AppCtx, run: &mut Run) -> Result<()> {
    run.updated_at = now();
    let n = app.db().conn.lock().unwrap().execute(
        "UPDATE security_runs SET data=?2,status=?3 WHERE id=?1 AND status IN ('running','waiting')",
        params![run.id, serde_json::to_string(run).map_err(|e|e.to_string())?,run.status]).map_err(|e|e.to_string())?;
    if n == 0 {
        return Err("security_canceled".into());
    }
    Ok(())
}
fn details(app: &AppCtx, run: Run) -> Result<Run> {
    let mut run = reconcile(app, run)?;
    if matches!(run.status.as_str(), "canceled" | "failed" | "interrupted")
        && run.upstream.is_some()
    {
        match native::recover_terminal_artifacts(&mut run) {
            Ok(true) => {
                run.updated_at = now();
                app.db()
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "UPDATE security_runs SET data=?2 WHERE id=?1 AND status=?3",
                        params![
                            run.id,
                            serde_json::to_string(&run).map_err(|e| e.to_string())?,
                            run.status
                        ],
                    )
                    .map_err(|e| e.to_string())?;
            }
            Ok(false) => {}
            Err(error) => {
                if let Some(value) = run.upstream.as_mut() {
                    value["artifactReadError"] = error.into();
                }
            }
        }
    }
    Ok(run)
}
fn reconcile(app: &AppCtx, mut run: Run) -> Result<Run> {
    if matches!(run.status.as_str(), "running" | "waiting") && !session_active(&run.session_id) {
        match native::reconcile_stopped(app, &mut run) {
            Ok(true) => return Ok(run),
            Ok(false) => {}
            Err(error) => {
                run.status = "failed".into();
                run.error = error;
                save(app, &mut run)?;
                return Ok(run);
            }
        }
        run.status = "interrupted".into();
        run.error = "security_interrupted".into();
        if let Err(error) = save(app, &mut run) {
            // A completion or cancellation may have committed after this read.
            if error == "security_canceled" {
                return get(app, &run.id);
            }
            return Err(error);
        }
    }
    Ok(run)
}
fn settings(app: &AppCtx, agent: &str) -> Result<Value> {
    let all = repo::get_app_settings(&app.db().conn.lock().unwrap())?;
    Ok(all
        .get("vlx-settings")
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .and_then(|v| v.get("agentDefaults").and_then(|a| a.get(agent)).cloned())
        .unwrap_or(json!({})))
}
fn models(app: &AppCtx, agent: &str) -> Result<Value> {
    catalog(app, agent, false, None)
}

/// The catalogue a scan offers (`accept = false`) or accepts (`accept = true`). For Claude the accepted set
/// also names the curated table, because the CLI's list is a shortlist and a scan saved with an identifier
/// it no longer names must still start (see claude_models::accepted_for_bin).
fn catalog(app: &AppCtx, agent: &str, accept: bool, selected: Option<&str>) -> Result<Value> {
    let defaults = settings(app, agent)?;
    let bin = defaults
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(agent);
    let rows = match agent {
        "claude" if accept => serde_json::to_value(crate::agent::claude_models::accepted_for_bin(app, bin, selected)),
        "claude" => serde_json::to_value(crate::agent::claude_models::list_for_bin(app, bin)),
        "codex" => {
            let args = crate::agent::inject::split_extra_args(
                defaults.get("args").and_then(Value::as_str),
            );
            serde_json::to_value(crate::agent::codex_models::list(bin, &args)?)
        }
        _ => return Err("security_invalid:agent".into()),
    };
    rows.map_err(|e| e.to_string())
}

fn validate_selection(model: Option<&str>, effort: Option<&str>, catalog: &Value) -> Result<()> {
    for value in [model, effort].into_iter().flatten() {
        if value.len() > 256 || value.chars().any(char::is_control) {
            return Err("security_invalid:selection".into());
        }
    }
    let model = model.map(str::trim).filter(|s| !s.is_empty());
    let effort = effort.map(str::trim).filter(|s| !s.is_empty());
    if model.is_none() && effort.is_none() {
        return Ok(());
    }
    let row = catalog
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["id"].as_str() == model));
    let Some(row) = row else {
        return Err("security_invalid:model".into());
    };
    if let Some(effort) = effort {
        if !row["effortLevels"]
            .as_array()
            .is_some_and(|levels| levels.iter().any(|v| v.as_str() == Some(effort)))
        {
            return Err("security_invalid:effort".into());
        }
    }
    Ok(())
}

fn start(app: &AppCtx, request: Request) -> Result<Run> {
    if ![
        "en", "zh-CN", "zh-TW", "ja", "ko", "fr", "de", "es", "pt-BR", "ru", "vi",
    ]
    .contains(&request.language.as_str())
    {
        return Err("security_invalid:language".into());
    }
    if request.path.len() > 4096 || request.project_id.len() > 100 {
        return Err("security_invalid:request".into());
    }
    let kind = match request.agent.as_str() {
        "codex" => SessionKind::Codex,
        "claude" => SessionKind::Claude,
        _ => return Err("security_invalid:agent".into()),
    };
    let explicit = request
        .model
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty())
        || request
            .effort
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty());
    let catalog = if explicit {
        catalog(app, &request.agent, true, request.model.as_deref())?
    } else {
        json!([])
    };
    validate_selection(
        request.model.as_deref(),
        request.effort.as_deref(),
        &catalog,
    )?;
    let root = repo::get_project_root(&app.db().conn.lock().unwrap(), &request.project_id)?
        .ok_or("security_missing_root")?;
    let root = scope::target(&root, &request.scope, &request.path)?;
    let defaults = settings(app, &request.agent)?;
    let id = uuid::Uuid::new_v4().to_string();
    let plugin = upstream::materialize(&app.data_dir()?)?;
    let state = app
        .data_dir()?
        .join("security")
        .join("runs")
        .join(&id)
        .join("state");
    let existing_args = defaults.get("args").and_then(Value::as_str).unwrap_or("");
    let selected_args = if request.agent == "claude" {
        upstream::selection_args(
            existing_args,
            request.model.as_ref().is_some_and(|s| !s.trim().is_empty()),
            request
                .effort
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty()),
        )
    } else {
        existing_args.to_owned()
    };
    let launch_args = upstream::launch_args(&request.agent, &plugin, &state, &selected_args)?;
    let session = crate::command_core::create_session(
        app,
        &request.project_id,
        None,
        "Security audit",
        kind,
        None,
        Some(&root),
        None,
        None,
        None,
        Some(&launch_args),
        defaults.get("permissionMode").and_then(Value::as_str),
        None,
        None,
        None,
        defaults.get("path").and_then(Value::as_str),
        Some("chat"),
    )?;
    let mut selection = crate::agent::session_settings::resolve(app, &session)?;
    if request.model.is_some() {
        selection.model = crate::agent::session_settings::clean(request.model.as_deref());
    }
    if request.effort.is_some() {
        selection.effort = crate::agent::session_settings::clean(request.effort.as_deref());
    }
    crate::agent::session_settings::persist(app, &session, &selection)?;
    let mut run = Run {
        id,
        project_id: request.project_id,
        session_id: session.id,
        agent: request.agent,
        root,
        scope: request.scope,
        path: request.path,
        status: "running".into(),
        phase: "preflight".into(),
        created_at: now(),
        updated_at: now(),
        files: vec![],
        excluded: vec![],
        reviewed: vec![],
        findings: vec![],
        threat_model: String::new(),
        steps: vec![],
        gaps: vec![],
        error: String::new(),
        language: request.language,
        model: selection.model,
        effort: selection.effort,
        upstream: None,
    };
    if let Err(error) = native::register(&plugin, &state, &mut run) {
        let _ = crate::command_core::delete_node(
            app,
            crate::models::NodeKind::Session,
            &run.session_id,
        );
        return Err(error);
    }
    let mut registry = active().lock().unwrap();
    app.db()
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO security_runs VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                run.id,
                run.project_id,
                run.session_id,
                run.status,
                run.created_at,
                serde_json::to_string(&run).map_err(|e| e.to_string())?
            ],
        )
        .map_err(|e| e.to_string())?;
    registry.insert(run.session_id.clone());
    drop(registry);
    let mut worker = run.clone();
    let ctx = app.clone();
    if let Err(error) = std::thread::Builder::new().name("security-audit".into()).spawn(move || {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| native::run(&ctx, &mut worker)));
        let error = match outcome { Ok(Ok(())) => None, Ok(Err(e)) => Some(e), Err(_) => Some("security_worker_failed".into()) };
        if let Some(error) = error {
            worker.status = if error == "security_canceled" { "canceled" } else { "failed" }.into();
            worker.error = error;
            let _ = save(&ctx, &mut worker);
        }
        workflow::log(&ctx,&worker,"finish",&json!({"method":"program","status":worker.status,"durationMs":now()-worker.created_at,"inputCount":worker.upstream.as_ref().and_then(|v|v["scan"]["progress"]["coverage"]["filesTotal"].as_u64()).unwrap_or(worker.files.len() as u64),"outputCount":worker.findings.len()}));
        active().lock().unwrap().remove(&worker.session_id);
        let _ = ctx.chat().stop(&ctx, &worker.session_id);
    }) {
        active().lock().unwrap().remove(&run.session_id);
        let mut failed = run.clone(); failed.status = "failed".into(); failed.error = error.to_string(); save(app,&mut failed)?;
        return Err("security_worker_failed".into());
    }
    Ok(run)
}

pub fn dispatch(app: &AppCtx, cmd: &str, args: &Value) -> Result<Value> {
    match cmd {
        "security_options" => Ok(
            json!({"agents":[{"id":"codex","label":"Codex"},{"id":"claude","label":"Claude Code"}],
            "scopes":["repository","path","working-tree"],"defaultAgent":"codex","defaultScope":"repository",
            "workflow":{"phases":["preflight","threat","discovery","validation","attack_path","report"],"package":"@openai/codex-security","packageVersion":upstream::PACKAGE_VERSION,"pluginVersion":upstream::PLUGIN_VERSION,"adapter":upstream::ADAPTER_VERSION}}),
        ),
        "security_models" => models(app, required(args, "agent")?),
        "security_start" => Ok(json!(start(
            app,
            serde_json::from_value(args.clone()).map_err(|_| "security_invalid:request")?
        )?)),
        "security_get" => Ok(json!(details(app, get(app, required(args, "id")?)?)?)),
        "security_list" => {
            let project = required(args, "projectId")?;
            let runs = {
                let conn = app.db().conn.lock().unwrap();
                let mut stmt = conn.prepare("SELECT data FROM security_runs WHERE project_id=?1 ORDER BY created_at DESC").map_err(|e|e.to_string())?;
                let rows = stmt.query_map([project], read).map_err(|e| e.to_string())?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| e.to_string())?
            };
            let mut result = vec![];
            for run in runs {
                let r = reconcile(app, run)?;
                result.push(json!({"id":r.id,"agent":r.agent,"status":r.status,"phase":r.phase,"createdAt":r.created_at,"scope":r.scope,"path":r.path,"findingCount":r.findings.len(),"model":r.model,"effort":r.effort}));
            }
            Ok(json!(result))
        }
        "security_cancel" => {
            let mut run = get(app, required(args, "id")?)?;
            if matches!(run.status.as_str(), "running" | "waiting") {
                native::cancel(&run)?;
                run.status = "canceled".into();
                if let Err(error) = save(app, &mut run) {
                    if error == "security_canceled" {
                        return Ok(json!(get(app, &run.id)?));
                    }
                    return Err(error);
                }
                let _ = app.chat().stop(app, &run.session_id);
            }
            Ok(json!(run))
        }
        "security_export" => {
            let run = details(app, get(app, required(args, "id")?)?)?;
            Ok(json!({"markdown":workflow::report(&run),"run":run}))
        }
        _ => Err("security_unknown_command".into()),
    }
}
