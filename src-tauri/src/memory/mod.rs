//! Application-wide thematic memory. The backend owns documents, provenance and agent jobs.
mod repo;
mod runner;
#[cfg(test)]
mod tests;

use crate::host::AppCtx;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub fn init(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.execute_batch(include_str!("schema.sql"))
        .map_err(|e| e.to_string())?;
    let has_effort: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('memory_jobs') WHERE name='effort')", [], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    if !has_effort {
        conn.execute_batch("ALTER TABLE memory_jobs ADD COLUMN effort TEXT NOT NULL DEFAULT '';").map_err(|e| e.to_string())?;
    }
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
        "memory_models" => Ok(json!(runner::models(app, required("agent")?)?)),
        "memory_list" => repo::list(app, args),
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
            for mut entry in repo::all(&tx)? {
                if entry.id != id && entry.related.iter().any(|target| target == id) {
                    entry.related.retain(|target| target != id);
                    let version = entry.version;
                    repo::put(&tx, entry, version, "unlink")?;
                }
            }
            tx.execute("DELETE FROM memory_entries WHERE id=?1", [id])
                .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(Value::Null)
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
