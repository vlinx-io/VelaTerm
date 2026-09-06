//! Project code intelligence. CodeGraph owns its index; VelaTerm owns jobs and memory associations.
mod runtime;
mod graph;
mod links;
pub mod agent;
#[cfg(test)]
mod tests;

use crate::host::AppCtx;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashMap;

type Result<T> = std::result::Result<T, String>;
pub fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(include_str!("schema.sql")).map_err(|e| e.to_string())
}
fn now() -> i64 { crate::memory::now() }
fn required<'a>(args: &'a Value, name: &str) -> Result<&'a str> {
    args.get(name).and_then(Value::as_str).filter(|s| !s.is_empty() && s.len() <= 4096)
        .ok_or_else(|| "knowledge_invalid".into())
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub id: String, pub project_id: String, pub root: String, pub enabled: bool,
    pub status: String, pub error: String, pub stats: Value, pub job_id: String, pub updated_at: i64,
}
fn read_index(row: &rusqlite::Row<'_>) -> rusqlite::Result<Index> {
    Ok(Index { id: row.get(0)?, project_id: row.get(1)?, root: row.get(2)?, enabled: row.get(3)?,
        status: row.get(4)?, error: row.get(5)?, stats: serde_json::from_str(&row.get::<_,String>(6)?).unwrap_or(json!({})),
        job_id: row.get(7)?, updated_at: row.get(8)? })
}
const INDEX_SQL: &str = "SELECT id,project_id,root,enabled,status,error,stats,job_id,updated_at FROM knowledge_indexes";
fn get(app: &AppCtx, id: &str) -> Result<Index> {
    let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
    conn.query_row(&format!("{INDEX_SQL} WHERE id=?1"), [id], read_index).optional()
        .map_err(|e| e.to_string())?.ok_or_else(|| "knowledge_not_found".into())
}
fn canonical(path: &Path) -> Result<PathBuf> {
    let root = path.canonicalize().map_err(|_| "knowledge_missing_directory")?;
    if !root.is_dir() || root.parent().is_none() || crate::host::home_dir().as_deref() == Some(root.as_path()) {
        return Err("knowledge_invalid_directory".into());
    }
    Ok(root)
}
// Resolve each checkout separately; nested worktrees must never inherit the parent's index.
fn checkout(path: &Path) -> Result<PathBuf> {
    let path = canonical(path)?;
    Ok(git_root(&path).unwrap_or(path))
}
fn git_root(path: &Path) -> Option<PathBuf> {
    let output = crate::host::command("git").args(["-C"]).arg(&path)
        .args(["rev-parse", "--show-toplevel"]).output();
    if let Ok(output) = output {
        if output.status.success() {
            return canonical(Path::new(String::from_utf8_lossy(&output.stdout).trim())).ok();
        }
    }
    None
}
fn roots(app: &AppCtx, project_id: &str) -> Result<Vec<String>> {
    let tree = {
        let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
        crate::db::repo::list_tree(&conn)?
    };
    let project = tree.projects.iter().find(|p| p.id == project_id).ok_or("knowledge_not_found")?;
    let project_root = canonical(Path::new(&project.root_path)).ok();
    let mut paths = Vec::new();
    paths.extend(tree.groups.iter().filter(|g| g.project_id == project_id).filter_map(|g| g.worktree_path.clone()));
    for session in tree.sessions.iter().filter(|s| s.project_id == project_id) {
        paths.extend(session.worktree_path.clone());
        paths.extend(session.cwd.clone());
    }
    paths.sort(); paths.dedup();
    let project_checkout = project_root.as_ref().and_then(|p|git_root(p));
    let mut result = project_root.as_ref().map(|r|vec![r.to_string_lossy().into_owned()]).unwrap_or_default();
    for path in paths.into_iter().filter(|p| !p.is_empty()) {
        if let Ok(root) = checkout(Path::new(&path)) {
            if project_root.as_ref().is_some_and(|p| canonical(Path::new(&path)).is_ok_and(|d|d.starts_with(p)) && git_root(&root)==project_checkout) { continue; }
            let root = root.to_string_lossy().into_owned();
            if !result.contains(&root) { result.push(root); }
        }
    }
    Ok(result)
}
fn list(app: &AppCtx, project_id: &str) -> Result<Value> {
    let roots = roots(app, project_id)?;
    let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
    for root in roots {
        conn.execute("INSERT OR IGNORE INTO knowledge_indexes(id,project_id,root,updated_at) VALUES(?1,?2,?3,?4)",
            params![uuid::Uuid::new_v4().to_string(),project_id,root,now()]).map_err(|e| e.to_string())?;
    }
    conn.execute("UPDATE knowledge_indexes SET status='failed',error='knowledge_interrupted',job_id='' WHERE status IN ('indexing','syncing') AND updated_at<?1", [now()-120_000]).map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(&format!("{INDEX_SQL} WHERE project_id=?1 ORDER BY root")).map_err(|e| e.to_string())?;
    let indexes = stmt.query_map([project_id], read_index).map_err(|e| e.to_string())?
        .collect::<std::result::Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
    Ok(json!({"indexes":indexes,"runtime":runtime::status(app)?}))
}
// Per-directory locks also serialize projects that point at the same physical checkout.
fn directory_lock(root: &str) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();
    LOCKS.get_or_init(Default::default).lock().unwrap().entry(root.to_string()).or_default().clone()
}
fn start(app: &AppCtx, id: &str) -> Result<Value> {
    let index = get(app, id)?;
    canonical(Path::new(&index.root))?;
    runtime::executable(app)?;
    let job = uuid::Uuid::new_v4().to_string();
    {
        let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
        let changed = conn.execute("UPDATE knowledge_indexes SET enabled=1,status=?1,error='',job_id=?2,updated_at=?3 WHERE id=?4 AND (status NOT IN ('indexing','syncing') OR updated_at<?5)",
            params![if Path::new(&index.root).join(".codegraph/codegraph.db").is_file() {"syncing"} else {"indexing"},job,now(),id,now()-120_000]).map_err(|e| e.to_string())?;
        if changed == 0 { return Err("knowledge_busy".into()); }
    }
    let ctx = app.clone(); let task = job.clone();
    std::thread::spawn(move || {
        let result = synchronize(&ctx, &index, Some(&task));
        if let Ok(conn) = ctx.db().conn.lock() {
            let (status, error, stats) = match result {
                Ok(stats) => ("ready", String::new(), stats),
                Err(error) => ("failed", error, index.stats),
            };
            let _ = conn.execute("UPDATE knowledge_indexes SET status=?1,error=?2,stats=?3,updated_at=?4,job_id='' WHERE id=?5 AND job_id=?6 AND enabled=1",
                params![status,error,stats.to_string(),now(),index.id,task]);
        }
        ctx.emit("knowledge://changed", ());
    });
    Ok(json!({"jobId":job}))
}
fn synchronize(app: &AppCtx, index: &Index, job: Option<&str>) -> Result<Value> {
    let lock = directory_lock(&index.root);
    let _guard = lock.lock().map_err(|_| "knowledge_busy")?;
    let root = canonical(Path::new(&index.root))?;
    for path in [root.join(".codegraph"),root.join(".codegraph/codegraph.db")] {
        if path.symlink_metadata().is_ok() && !path.canonicalize().map_err(|_| "knowledge_invalid_directory")?.starts_with(&root) {
            return Err("knowledge_invalid_directory".into());
        }
    }
    // A checkout moved beneath another repository still needs its own explicitly selected root.
    if root.to_string_lossy()!=index.root { return Err("knowledge_directory_changed".into()); }
    let current = get(app, &index.id)?;
    if !current.enabled || job.is_some_and(|j| j != current.job_id) { return Err("knowledge_disabled".into()); }
    let started = std::time::Instant::now();
    runtime::audit(app, &index.id, "INFO", "sync_started", 0);
    let command = if root.join(".codegraph/codegraph.db").is_file() { "sync" } else { "init" };
    let result = (|| {
        runtime::run(app, index, job, command)?;
        let stats = graph::stats(&root)?;
        if stats["state"] != "complete" { return Err("knowledge_partial".into()); }
        Ok(stats)
    })();
    runtime::audit(app, &index.id, if result.is_ok() { "INFO" } else { "ERROR" },
        if result.is_ok() { "sync_completed" } else { "sync_failed" }, started.elapsed().as_millis());
    result
}
fn ready(app: &AppCtx, id: &str) -> Result<Index> {
    let index = get(app, id)?;
    if !index.enabled { return Err("knowledge_disabled".into()); }
    if index.status == "indexing" || index.status == "syncing" { return Err("knowledge_busy".into()); }
    let result = synchronize(app, &index, None);
    let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
    match result {
        Ok(stats) => {
            conn.execute("UPDATE knowledge_indexes SET status='ready',error='',stats=?1,updated_at=?2 WHERE id=?3 AND enabled=1 AND job_id=''", params![stats.to_string(),now(),id]).map_err(|e| e.to_string())?;
            Ok(index)
        },
        Err(error) => {
            conn.execute("UPDATE knowledge_indexes SET status='failed',error=?1,updated_at=?2 WHERE id=?3 AND enabled=1 AND job_id=''", params![error,now(),id]).map_err(|e| e.to_string())?;
            Err(error)
        }
    }
}
pub fn dispatch(app: &AppCtx, cmd: &str, args: &Value) -> Result<Value> {
    match cmd {
        "knowledge_list" => list(app, required(args,"projectId")?),
        "knowledge_install" => runtime::install_start(app),
        "knowledge_runtime" => runtime::status(app),
        "knowledge_start" => start(app, required(args,"id")?),
        "knowledge_disable" => {
            let id = required(args,"id")?; get(app,id)?;
            app.db().conn.lock().map_err(|e| e.to_string())?.execute("UPDATE knowledge_indexes SET enabled=0,status='disabled',job_id='',error='',updated_at=?1 WHERE id=?2",params![now(),id]).map_err(|e| e.to_string())?;
            app.emit("knowledge://changed", ()); Ok(Value::Null)
        },
        "knowledge_search" => {
            let index = ready(app, required(args,"id")?)?;
            let query = args.get("query").and_then(Value::as_str).unwrap_or("");
            graph::search(Path::new(&index.root),query,args.get("page").and_then(Value::as_u64).unwrap_or(0))
        },
        "knowledge_node" => {
            let index = ready(app, required(args,"id")?)?;
            let mut value = graph::detail(Path::new(&index.root), required(args,"nodeId")?)?;
            value["memories"] = links::for_node(app, &index, &value["node"])?;
            Ok(value)
        },
        "knowledge_links" => links::for_entry(app, required(args,"entryId")?),
        "knowledge_link" | "knowledge_unlink" | "knowledge_review" => {
            let result = match cmd { "knowledge_link" => links::create(app,args), "knowledge_unlink" => links::remove(app,args), _ => links::review(app,args) }?;
            app.emit("knowledge://changed", ()); Ok(result)
        },
        _ => Err("knowledge_invalid".into()),
    }
}

/// Apply the caller's existing file-access policy before entering code-index operations.
pub fn guard_paths(app: &AppCtx, cmd: &str, args: &Value, guard: impl Fn(&str) -> Result<()>) -> Result<()> {
    if cmd=="knowledge_list" {
        for root in roots(app,required(args,"projectId")?)? { guard(&root)?; }
    } else if matches!(cmd,"knowledge_start" | "knowledge_search" | "knowledge_node" | "knowledge_disable") {
        guard(&get(app,required(args,"id")?)?.root)?;
    } else if cmd=="knowledge_link" { guard(&get(app,required(args,"indexId")?)?.root)?; }
    else if matches!(cmd,"knowledge_links" | "knowledge_review" | "knowledge_unlink") {
        for link in links::all(app,required(args,"entryId")?,"")? {
            guard(link["root"].as_str().ok_or("knowledge_invalid")?)?;
        }
    }
    Ok(())
}
