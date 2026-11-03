//! Application-wide thematic memory. The backend owns documents, provenance and agent jobs.
mod hierarchy;
mod repo;
mod queue;
mod runner;
#[cfg(test)]
mod tests;

use crate::host::{AppCtx, TREE_CHANGED};
use crate::models::NodeKind;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub fn resume(app: &AppCtx) {
    if let Err(error) = queue::resume(app) {
        runner::process::audit(app, "system", "ERROR", "queue_failed", &json!({"error":error}));
    }
}

pub fn init(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.execute_batch(include_str!("schema.sql"))
        .map_err(|e| e.to_string())?;
    let has_effort: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('memory_jobs') WHERE name='effort')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !has_effort {
        conn.execute_batch("ALTER TABLE memory_jobs ADD COLUMN effort TEXT NOT NULL DEFAULT '';")
            .map_err(|e| e.to_string())?;
    }
    hierarchy::init(conn)?;
    Ok(())
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    #[serde(default)]
    pub session_id: String,
    pub title: String,
    pub summary: String,
    pub content: String,
    pub tags: Vec<String>,
    pub related: Vec<String>,
    pub sources: Vec<String>,
    pub version: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Edit {
    pub id: Option<String>,
    pub version: i64,
    pub title: String,
    pub summary: String,
    pub content: String,
    pub tags: Vec<String>,
    pub related: Vec<String>,
}

pub fn dispatch(app: &AppCtx, cmd: &str, args: &Value) -> Result<Value, String> {
    let required = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("memory_invalid:{key}"))
    };
    match cmd {
        "memory_options" => runner::options(app),
        "memory_list" => repo::list(app, args),
        // Archived sessions grouped by their original project for the Collections hierarchy.
        "memory_collections" => {
            let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
            hierarchy::collections(&conn)
        }
        "memory_get" => repo::detail(
            app,
            required("id")?,
            args.get("version").and_then(Value::as_i64),
        ),
        "memory_save" => {
            let edit: Edit =
                serde_json::from_value(args.clone()).map_err(|_| "memory_invalid:entry")?;
            let mut conn = app.db().conn.lock().map_err(|e| e.to_string())?;
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            let entry = repo::edit(&tx, edit, "manual")?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(json!(entry))
        }
        "memory_delete" => {
            let id = required("id")?;
            let expected = args.get("version").and_then(Value::as_i64).unwrap_or(-1);
            let mut conn = app.db().conn.lock().map_err(|e| e.to_string())?;
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            if repo::get(&tx, id)?.as_ref().map(|e| e.version) != Some(expected) {
                return Err("memory_conflict".into());
            }
            // Removing a target also revises incoming links, so later edits have no invisible stale IDs.
            repo::remove(&tx, &[id.to_string()])?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        // Rename from the knowledge tree: title only, recorded as a manual revision.
        "memory_rename" => {
            let mut conn = app.db().conn.lock().map_err(|e| e.to_string())?;
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            let entry = repo::rename_entry(
                &tx,
                required("id")?,
                args.get("version").and_then(Value::as_i64).unwrap_or(-1),
                required("title")?,
                "manual",
            )?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(json!(entry))
        }
        // Reassign an entry to another session group. `__manual__` is the tree's id for the manual
        // collection; the stored grouping for it is the empty string.
        "memory_move" => {
            let target = required("sessionId")?;
            let session_id = if target == "__manual__" { "" } else { target };
            let mut conn = app.db().conn.lock().map_err(|e| e.to_string())?;
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            if !session_id.is_empty() {
                let exists: bool = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM memory_sessions WHERE id=?1)",
                        [session_id],
                        |r| r.get(0),
                    )
                    .map_err(|e| e.to_string())?;
                if !exists {
                    return Err("memory_invalid:group".into());
                }
            }
            let entry = repo::move_entry(
                &tx,
                required("id")?,
                args.get("version").and_then(Value::as_i64).unwrap_or(-1),
                session_id,
                "manual",
            )?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(json!(entry))
        }
        // Rename a tree group: the live project/session is renamed first so both trees agree, then the
        // snapshot that supplies the knowledge tree's label is updated. Synthetic groups are refused.
        "memory_group_rename" => {
            let kind = required("kind")?;
            let id = required("id")?;
            let name = required("name")?.trim();
            if id.starts_with("__") || !matches!(kind, "project" | "session") || name.is_empty() {
                return Err("memory_invalid:group".into());
            }
            let mut conn = app.db().conn.lock().map_err(|e| e.to_string())?;
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            let node = if kind == "project" { NodeKind::Project } else { NodeKind::Session };
            crate::db::repo::rename_node(&tx, node, id, name).map_err(|e| e.to_string())?;
            if kind == "project" {
                hierarchy::rename_project_snapshot(&tx, id, name)?;
            } else {
                hierarchy::rename_snapshot(&tx, id, name)?;
            }
            tx.commit().map_err(|e| e.to_string())?;
            app.emit(TREE_CHANGED, ());
            Ok(Value::Null)
        }
        // Move a session into another project from the knowledge tree: the live session move keeps the
        // workspace consistent, the snapshot decides which project group the entries appear under.
        "memory_group_move" => {
            let session_id = required("sessionId")?;
            let target_project = required("targetProjectId")?;
            let mut conn = app.db().conn.lock().map_err(|e| e.to_string())?;
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            let project_name: Option<String> = tx
                .query_row(
                    "SELECT name FROM projects WHERE id=?1",
                    [target_project],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| e.to_string())?;
            let project_name = project_name.ok_or_else(|| "memory_invalid:project".to_string())?;
            crate::db::repo::move_node(
                &tx,
                NodeKind::Session,
                session_id,
                Some(target_project),
                None,
                None,
                now(),
            )?;
            hierarchy::move_snapshot(&tx, session_id, target_project, &project_name)?;
            tx.commit().map_err(|e| e.to_string())?;
            app.emit(TREE_CHANGED, ());
            Ok(Value::Null)
        }
        // Delete every entry under one tree group. The real project or session is kept.
        "memory_group_delete" => {
            let kind = required("kind")?;
            let id = required("id")?;
            let mut conn = app.db().conn.lock().map_err(|e| e.to_string())?;
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            let removed = repo::delete_group(&tx, kind, id)?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(json!({"removed":removed}))
        }
        "memory_restore" => repo::restore(app, required("id")?, args),
        "memory_source" => repo::source(app, required("id")?),
        "memory_start" => runner::start(
            app,
            required("sessionId")?,
            required("agent")?,
            args.get("model").and_then(Value::as_str).unwrap_or(""),
            args.get("effort").and_then(Value::as_str).unwrap_or(""),
        ),
        "memory_retry" => runner::retry(app, required("id")?),
        "memory_cancel" => runner::cancel(app, required("id")?),
        "memory_jobs" => runner::jobs(app, args),
        _ => Err("memory_invalid:command".into()),
    }
}
