//! `vapp`: lets the agent in any session operate the app; nothing runs until the user confirms it.
//!
//! Each app area ("domain") describes its actions, supplies a secret-free state summary, validates what
//! the agent proposed, and runs the actions the backend owns. A request is checked whole before it is
//! stored: if any action is invalid, nothing reaches the user and the agent gets every problem back.
//! The source session's view then shows the actions for confirmation; consecutive backend actions are
//! confirmed together, and client-owned actions (navigation, prefilled forms) run in the window and
//! report back through `assist_mark`.
//!
//! Sidebar actions may name items created earlier in the same request (`"as": "g1"`, then `"$g1"`).
//! Each such name is replaced by the real ID in the stored plan once the action creating it has run.

mod files;
mod knowledge;
mod projects;
mod settings;
mod tree;

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::{json, Value};
use crate::host::AppCtx;
use crate::web::dispatch::CallOrigin;

type Result<T> = std::result::Result<T, String>;

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS assist_plans (id TEXT PRIMARY KEY, created_at INTEGER NOT NULL, value TEXT NOT NULL);";
const RETAINED: usize = 200;
/// One request may reorganize a large project, e.g. renaming and moving every session in it.
const MAX_ACTIONS: usize = 500;
const MAX_SUMMARY: usize = 4000;
/// Event telling the source session's view that a request waits for the user.
pub(crate) const REQUEST_EVENT: &str = "assist://request";
/// Event telling every client that actions of a request were decided, so other views drop them too.
pub(crate) const CHANGED_EVENT: &str = "assist://changed";
/// The area actions without a `domain` belong to: the sidebar commands (`vapp session ...`) omit it.
const DEFAULT_DOMAIN: &str = "projects";
/// Fields that hold item IDs, and so may hold a `$name` for an item created earlier in the request.
const ID_FIELDS: &[&str] = &["id", "projectId", "groupId", "parentGroupId", "parentSessionId", "collectionId", "parentId", "sessionId"];

/// A problem with one proposed action: the field at fault and a reason code.
pub(crate) type Problem = (&'static str, &'static str);

/// How an area checks the actions proposed for it.
pub(crate) enum Check {
    /// Each action on its own, against the area's state summary. `Err` is a reason code.
    Each(fn(&AppCtx, &Value, &Value, CallOrigin) -> std::result::Result<Value, &'static str>),
    /// In order, on one copy of the sidebar tree, so later actions see what earlier ones create or move.
    Tree(fn(&AppCtx, &Value, &mut tree::Model, usize, CallOrigin) -> std::result::Result<Value, Problem>),
}

/// One app area the agent can act in.
pub(crate) struct Domain {
    pub id: &'static str,
    /// Action shapes and rules, printed by `vapp catalog`.
    pub guide: &'static str,
    /// Compact state the agent may reference. Must never contain credentials.
    pub context: fn(&AppCtx, CallOrigin) -> Result<Value>,
    /// Normalizes one proposed action. Problems are returned to the agent; the user never sees them.
    pub check: Check,
    /// Kinds the backend executes through `assist_run`; every other kind runs in the client.
    pub backend: &'static [&'static str],
    /// Runs a confirmed backend action. `args` carries the user's choice when the action offers several.
    pub run: fn(&AppCtx, &Value, &Value, CallOrigin) -> Result<Value>,
}

fn domains() -> &'static [Domain] { &[projects::DOMAIN, files::DOMAIN, knowledge::DOMAIN, settings::DOMAIN] }
fn domain(id: &str) -> Option<&'static Domain> { domains().iter().find(|d| d.id == id) }

fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str> { args[key].as_str().filter(|s| !s.is_empty()).ok_or_else(|| format!("{key} is required")) }

/// Trimmed single-line text without control characters, or `None` when absent or empty.
pub(crate) fn text(value: &Value, max: usize) -> std::result::Result<Option<String>, &'static str> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) => {
            let s = s.trim();
            if s.chars().count() > max || s.chars().any(|c| c.is_control()) { return Err("invalid"); }
            Ok((!s.is_empty()).then(|| s.to_string()))
        }
        _ => Err("invalid"),
    }
}

/// Calls an app command through the shared dispatcher, so the caller's trust level and guards apply.
pub(crate) fn call(app: &AppCtx, cmd: &str, args: Value, origin: CallOrigin) -> Result<Value> {
    crate::web::dispatch::dispatch(app, cmd, &args, crate::pty::manager::DESKTOP_SOURCE, origin)
}

/// Expands `~`, requires an absolute path and applies the remote path policy.
pub(crate) fn path(app: &AppCtx, value: &Value, origin: CallOrigin) -> std::result::Result<Option<String>, &'static str> {
    let Some(raw) = text(value, 4096)? else { return Ok(None) };
    let home = crate::host::home_dir();
    let expanded = match (raw.as_str(), raw.strip_prefix("~/"), &home) {
        ("~", _, Some(home)) => home.clone(),
        (_, Some(rest), Some(home)) => home.join(rest),
        _ => std::path::PathBuf::from(&raw),
    };
    if !expanded.is_absolute() { return Err("invalidPath"); }
    let expanded = expanded.to_string_lossy().into_owned();
    crate::web::dispatch::guard_remote_path(app, origin, &expanded).map_err(|_| "forbiddenPath")?;
    Ok(Some(expanded))
}

/// Picks one value from a fixed list, case-insensitively.
pub(crate) fn one_of(value: &Value, allowed: &[&'static str], fallback: Option<&'static str>) -> std::result::Result<&'static str, &'static str> {
    match value.as_str().map(str::trim).filter(|s| !s.is_empty()) {
        None => fallback.ok_or("invalid"),
        Some(v) => allowed.iter().find(|a| a.eq_ignore_ascii_case(v)).copied().ok_or("invalid"),
    }
}

/// Serializes plan status changes; plans are small and changes are user-paced.
static PLANS: Mutex<()> = Mutex::new(());

fn plans_lock() -> Result<std::sync::MutexGuard<'static, ()>> { PLANS.lock().map_err(|_| "App requests are unavailable".to_string()) }

fn load(app: &AppCtx, id: &str) -> Result<Value> {
    let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
    conn.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
    let value: Option<String> = conn.query_row("SELECT value FROM assist_plans WHERE id=?1", [id], |r| r.get(0)).ok();
    serde_json::from_str(&value.ok_or("This request is no longer available. Ask the agent to submit it again.")?).map_err(|e| e.to_string())
}

fn store(app: &AppCtx, plan: &Value) -> Result<()> {
    let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
    conn.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO assist_plans(id,created_at,value) VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET value=excluded.value",
        rusqlite::params![required(plan, "id")?, plan["createdAt"].as_u64().unwrap_or_else(now) as i64, plan.to_string()]).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM assist_plans WHERE id NOT IN (SELECT id FROM assist_plans ORDER BY created_at DESC, rowid DESC LIMIT ?1)", [RETAINED as i64]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Characters of area state `vapp catalog` prints, so a large tree does not flood the agent's context.
const STATE_BUDGET: usize = 24_000;

/// Shortens the longest lists until the state fits the budget, marking each shortened list.
fn fit(contexts: &mut [(&Domain, Value)], budget: usize) {
    let size = |contexts: &[(&Domain, Value)]| contexts.iter().map(|(_, c)| c.to_string().len()).sum::<usize>();
    while size(contexts) > budget {
        let longest = contexts.iter_mut().flat_map(|(_, c)| c.as_object_mut().into_iter().flat_map(|o| o.iter_mut()))
            .filter(|(_, v)| v.as_array().is_some_and(|a| a.len() > 1))
            .max_by_key(|(_, v)| v.to_string().len());
        let Some((_, list)) = longest else { return };
        let items = list.as_array_mut().unwrap();
        let keep = items.len() / 2;
        items.truncate(keep);
        items.push(json!("…more items omitted"));
    }
}

fn contexts(app: &AppCtx, only: Option<&str>) -> Vec<(&'static Domain, Value)> {
    // One unavailable area must not block the others.
    domains().iter().filter(|d| only.is_none_or(|id| d.id == id))
        .filter_map(|d| (d.context)(app, CallOrigin::Local).ok().map(|c| (d, c))).collect()
}

/// Validates the proposed actions in order. Returns the normalized actions, or every problem found.
fn build(app: &AppCtx, list: &[Value], contexts: &[(&Domain, Value)], origin: CallOrigin) -> Result<std::result::Result<Vec<Value>, Vec<Value>>> {
    let (mut actions, mut problems, mut model) = (Vec::new(), Vec::new(), None);
    for (index, raw) in list.iter().enumerate() {
        let kind = raw["kind"].as_str().unwrap_or_default().chars().take(60).collect::<String>();
        let area = raw["domain"].as_str().unwrap_or(DEFAULT_DOMAIN);
        let checked = match contexts.iter().find(|(d, _)| d.id == area) {
            _ if !raw.is_object() => Err(("kind", "invalid")),
            None => Err(("domain", "unsupported")),
            Some((domain, context)) => match domain.check {
                Check::Each(check) => check(app, raw, context, origin).map_err(|reason| ("", reason)),
                Check::Tree(check) => {
                    // Loaded once per request; every sidebar action replays on the same copy.
                    if model.is_none() { model = Some(tree::load(app)?); }
                    check(app, raw, model.as_mut().unwrap(), index, origin)
                }
            }.map(|action| (*domain, action)),
        };
        match checked {
            Ok((domain, mut action)) => {
                action["domain"] = json!(domain.id); action["kind"] = json!(kind); action["status"] = json!("pending");
                action["backend"] = json!(domain.backend.contains(&kind.as_str()));
                actions.push(action);
            }
            Err((field, reason)) => {
                let mut problem = json!({"index": index, "domain": area.chars().take(40).collect::<String>(), "kind": kind, "reason": reason});
                if !field.is_empty() { problem["field"] = json!(field); }
                problems.push(problem);
            }
        }
    }
    Ok(if problems.is_empty() { Ok(actions) } else { Err(problems) })
}

/// For `vapp catalog`: what each area can do and its current state, so the agent can name real ids.
pub(crate) fn catalog(app: &AppCtx, only: Option<&str>) -> Result<Value> {
    let mut contexts = contexts(app, only);
    if contexts.is_empty() { return Err(format!("Unknown area. Use one of: {}", domains().iter().map(|d| d.id).collect::<Vec<_>>().join(", "))); }
    fit(&mut contexts, STATE_BUDGET);
    Ok(json!({"domains": contexts.iter().map(|(d, c)| json!({"id": d.id, "guide": d.guide, "state": c})).collect::<Vec<_>>()}))
}

/// For `vapp do`: validates the actions an agent proposes and, when all are valid, shows them in its
/// session's view for the user to confirm. Nothing runs here.
pub(crate) fn submit(app: &AppCtx, session_id: &str, summary: &str, actions: &Value) -> Result<Value> {
    if session_id.is_empty() { return Err("Run vapp inside a VelaTerm session".into()); }
    // Unsaved draft terminals have no stored row yet; they are still the user's own sessions.
    let session = {
        let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
        crate::db::repo::get_session(&conn, session_id)?
    };
    let summary = summary.trim().chars().take(MAX_SUMMARY).collect::<String>();
    let list = actions.as_array().filter(|a| !a.is_empty()).ok_or("Provide a non-empty JSON array of actions")?;
    if list.len() > MAX_ACTIONS { return Err(format!("Submit at most {MAX_ACTIONS} actions at a time")); }
    let actions = match build(app, list, &contexts(app, None), CallOrigin::Local)? {
        Ok(actions) => actions,
        // A request reaches the user whole or not at all, so they never see half a plan.
        Err(problems) => return Ok(json!({"accepted": false, "problems": problems})),
    };
    let plan = json!({"id": uuid::Uuid::new_v4().to_string(), "createdAt": now(), "summary": summary,
        "source": {"sessionId": session_id, "sessionName": session.as_ref().map(|s| s.name.clone()).unwrap_or_default(), "agent": session.as_ref().map(|s| s.kind)},
        "actions": actions});
    { let _guard = plans_lock()?; store(app, &plan)?; }
    app.emit(REQUEST_EVENT, json!({"id": plan["id"], "sessionId": session_id}));
    let mut result = outcome_of(&plan);
    result["accepted"] = json!(true);
    Ok(result)
}

fn open(action: &Value) -> bool { matches!(action["status"].as_str(), Some("pending" | "running")) }

/// Requests from one session's agent that still wait for the user, oldest first. The session view shows
/// them beside its conversation, so they survive a reload.
fn pending(app: &AppCtx, session_id: &str) -> Result<Value> {
    let rows = {
        let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
        conn.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
        let mut statement = conn.prepare("SELECT value FROM assist_plans ORDER BY created_at, rowid").map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?
            .filter_map(std::result::Result::ok).collect::<Vec<_>>();
        rows
    };
    Ok(Value::Array(rows.iter().filter_map(|raw| serde_json::from_str::<Value>(raw).ok())
        .filter(|plan| plan["source"]["sessionId"] == session_id)
        .filter(|plan| plan["actions"].as_array().is_some_and(|a| a.iter().any(open)))
        .collect()))
}

fn outcome_of(plan: &Value) -> Value {
    let actions = plan["actions"].as_array().into_iter().flatten().enumerate().map(|(index, a)| {
        let mut item = json!({"index": index, "domain": a["domain"], "kind": a["kind"], "status": a["status"]});
        for key in ["error", "createdId", "as"] { if !a[key].is_null() { item[key] = a[key].clone(); } }
        item
    }).collect::<Vec<_>>();
    let settled = !plan["actions"].as_array().into_iter().flatten().any(open);
    json!({"id": plan["id"], "settled": settled, "actions": actions})
}

/// For `vapp status`: each action's state, settled once none is waiting for the user.
pub(crate) fn outcome(app: &AppCtx, id: &str) -> Result<Value> {
    let plan = { let _guard = plans_lock()?; load(app, id)? };
    Ok(outcome_of(&plan))
}

/// For `vapp session list`: the groups and sessions of one project, or of the calling session's project.
pub(crate) fn list(app: &AppCtx, project: Option<&str>, session_id: &str) -> Result<Value> { tree::list(app, project, session_id) }

/// The `$name`s an action still refers to, i.e. items an earlier action of the request has yet to create.
fn references(action: &Value) -> Vec<String> {
    let ids = action["ids"].as_array().into_iter().flatten();
    ID_FIELDS.iter().map(|key| &action[*key]).chain(ids)
        .filter_map(|v| v.as_str().filter(|s| s.starts_with('$')).map(str::to_string)).collect()
}

/// Puts created IDs in place of `$name`s, and fails actions whose referenced item will never exist
/// because the action creating it failed or was skipped.
fn link(plan: &mut Value) {
    const MISSING: &str = "An earlier change in this request that this one depends on did not run";
    let Some(actions) = plan["actions"].as_array_mut() else { return };
    // Repeated until stable: failing one action can strand the actions that depend on it.
    loop {
        let mut changes = Vec::new();
        {
            let creator = |name: &str| actions.iter().find(|a| a["ref"] == name);
            for (index, action) in actions.iter().enumerate().filter(|(_, a)| a["status"] == "pending") {
                for name in references(action) {
                    let Some(creator) = creator(&name) else { continue };
                    match creator["status"].as_str() {
                        Some("done") if creator["createdId"].is_string() => changes.push((index, Some((name, creator["createdId"].clone())))),
                        Some("done" | "failed" | "dismissed") => changes.push((index, None)),
                        _ => {}
                    }
                }
            }
        }
        if changes.is_empty() { return; }
        let mut stranded = false;
        for (index, change) in changes {
            let action = &mut actions[index];
            match change {
                Some((name, id)) => {
                    for key in ID_FIELDS { if action[*key] == name.as_str() { action[*key] = id.clone(); } }
                    for item in action["ids"].as_array_mut().into_iter().flatten() { if *item == name.as_str() { *item = id.clone(); } }
                }
                None if action["status"] == "pending" => { action["status"] = json!("failed"); action["error"] = json!(MISSING); stranded = true; }
                None => {}
            }
        }
        if !stranded { return; }
    }
}

/// The actions a command names: `indices` in the order given, or a single `index`.
fn indices(args: &Value) -> Result<Vec<usize>> {
    match args["indices"].as_array() {
        Some(list) if !list.is_empty() => list.iter().map(|i| i.as_u64().map(|i| i as usize).ok_or_else(|| "indices must be numbers".to_string())).collect(),
        _ => Ok(vec![args["index"].as_u64().ok_or("index is required")? as usize]),
    }
}

fn action_at(plan: &mut Value, index: usize) -> Result<&mut Value> {
    plan["actions"].get_mut(index).ok_or_else(|| "This action is not part of the request".to_string())
}

fn changed(app: &AppCtx, plan: &Value) {
    app.emit(CHANGED_EVENT, json!({"id": plan["id"], "sessionId": plan["source"]["sessionId"]}));
}

/// Runs backend-owned actions once each, in the order given. A started action is never run again, even
/// after a failure; a failure does not stop the others, except those that depend on what it was to create.
fn run(app: &AppCtx, args: &Value, origin: CallOrigin) -> Result<Value> {
    if args["confirmed"] != true { return Err("Confirm the action before running it".into()); }
    let _guard = plans_lock()?;
    let mut plan = load(app, required(args, "id")?)?;
    let list = indices(args)?;
    // Every named action is checked before any runs, so a stale view cannot run half a batch.
    for &index in &list {
        let action = action_at(&mut plan, index)?;
        let domain = domain(action["domain"].as_str().unwrap_or_default()).ok_or("This action's area is unavailable")?;
        if !domain.backend.contains(&action["kind"].as_str().unwrap_or_default()) { return Err("This action runs in the app window, not on the host".into()); }
        if action["status"] != "pending" { return Err("This action has already run. Ask the agent to submit it again to repeat it.".into()); }
    }
    let mut first_error = None;
    for &index in &list {
        link(&mut plan);
        let action = action_at(&mut plan, index)?.clone();
        // Stranded by an earlier failure while this batch ran.
        if action["status"] != "pending" { first_error.get_or_insert_with(|| action["error"].as_str().unwrap_or_default().to_string()); continue; }
        if !references(&action).is_empty() { return finish(app, plan, Err("Confirm the earlier change this one depends on first".into())); }
        let domain = domain(action["domain"].as_str().unwrap_or_default()).ok_or("This action's area is unavailable")?;
        action_at(&mut plan, index)?["status"] = json!("running"); store(app, &plan)?;
        let result = (domain.run)(app, &action, args, origin);
        let slot = action_at(&mut plan, index)?;
        match result {
            Ok(value) => {
                slot["status"] = json!("done");
                if !slot["ref"].is_null() { slot["createdId"] = value["id"].clone(); }
                slot["result"] = value;
            }
            Err(error) => {
                slot["status"] = json!("failed"); slot["error"] = json!(error.chars().take(2000).collect::<String>());
                first_error.get_or_insert(error);
            }
        }
    }
    link(&mut plan);
    // A single action reports its failure as an error, as before; a batch returns each outcome in the plan.
    let result = match first_error { Some(error) if list.len() == 1 => Err(error), _ => Ok(()) };
    finish(app, plan, result)
}

fn finish(app: &AppCtx, plan: Value, result: Result<()>) -> Result<Value> {
    store(app, &plan)?;
    changed(app, &plan);
    result.map(|_| plan)
}

/// Records the outcome of a client-owned action, or the user skipping any actions.
fn mark(app: &AppCtx, args: &Value) -> Result<Value> {
    let _guard = plans_lock()?;
    let mut plan = load(app, required(args, "id")?)?;
    let status = required(args, "status")?;
    if !matches!(status, "done" | "failed" | "dismissed") { return Err("Unknown action status".into()); }
    let list = indices(args)?;
    if status != "dismissed" && list.len() > 1 { return Err("Record one window action at a time".into()); }
    for index in list {
        let action = action_at(&mut plan, index)?;
        let domain = domain(action["domain"].as_str().unwrap_or_default()).ok_or("This action's area is unavailable")?;
        // Host actions record their own outcome when they run; the user can still skip them beforehand.
        if status != "dismissed" && domain.backend.contains(&action["kind"].as_str().unwrap_or_default()) { return Err("This action runs on the host".into()); }
        if action["status"] != "pending" { continue; }
        action["status"] = json!(status);
        if let Some(error) = args["error"].as_str() { action["error"] = json!(error.chars().take(2000).collect::<String>()); }
    }
    link(&mut plan);
    finish(app, plan, Ok(()))
}

pub fn dispatch(app: &AppCtx, cmd: &str, args: &Value, origin: CallOrigin) -> Result<Value> {
    match cmd {
        "assist_get" => { let _guard = plans_lock()?; load(app, required(args, "id")?) }
        "assist_pending" => { let _guard = plans_lock()?; pending(app, required(args, "sessionId")?) }
        "assist_run" => run(app, args, origin),
        "assist_mark" => mark(app, args),
        _ => Err(format!("Unknown command: {cmd}")),
    }
}

#[cfg(test)]
mod tests;
