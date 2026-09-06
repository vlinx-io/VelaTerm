//! Transactional memory documents, independent full-text search and immutable revisions.
use super::{now, Edit, Entry};
use crate::host::AppCtx;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::collections::HashSet;

pub fn key(title: &str) -> String {
    title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    let array = |col| -> rusqlite::Result<Vec<String>> {
        let raw: String = r.get(col)?;
        serde_json::from_str(&raw).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Text, Box::new(e))
        })
    };
    Ok(Entry {
        id: r.get(0)?,
        title: r.get(1)?,
        summary: r.get(2)?,
        content: r.get(3)?,
        tags: array(4)?,
        related: array(5)?,
        sources: array(6)?,
        version: r.get(7)?,
        created_at: r.get(8)?,
        updated_at: r.get(9)?,
    })
}
const COLS: &str = "id,title,summary,content,tags,related,sources,version,created_at,updated_at";

pub fn get(conn: &Connection, id: &str) -> Result<Option<Entry>, String> {
    conn.query_row(
        &format!("SELECT {COLS} FROM memory_entries WHERE id=?1"),
        [id],
        row,
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn catalog(conn: &Connection) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare("SELECT id,title FROM memory_entries ORDER BY title_key")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(json!({"id":r.get::<_,String>(0)?,"title":r.get::<_,String>(1)?}))
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn all(conn: &Connection) -> Result<Vec<Entry>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {COLS} FROM memory_entries ORDER BY title_key"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], row).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn validate(entry: &mut Entry) -> Result<(), String> {
    entry.title = entry.title.trim().to_string();
    entry.summary = entry.summary.trim().to_string();
    entry.content = entry.content.trim().to_string();
    if entry.title.is_empty()
        || entry.title.chars().count() > 200
        || entry.title.chars().any(char::is_control)
        || entry.summary.chars().count() > 2000
        || entry.content.is_empty()
        || entry.content.chars().count() > 200_000
    {
        return Err("memory_invalid:entry_size".into());
    }
    entry.tags = entry
        .tags
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    entry.tags.sort();
    entry.tags.dedup();
    entry.related.sort();
    entry.related.dedup();
    entry.related.retain(|id| id != &entry.id);
    entry.sources.sort();
    entry.sources.dedup();
    if entry.tags.len() > 32
        || entry
            .tags
            .iter()
            .any(|s| s.chars().count() > 80 || s.chars().any(char::is_control))
        || entry.related.len() > 100
    {
        return Err("memory_invalid:metadata".into());
    }
    Ok(())
}

/// Caller owns a transaction so a failed link, conflict or revision write rolls everything back.
pub fn put(
    conn: &Connection,
    mut entry: Entry,
    expected: i64,
    author: &str,
) -> Result<Entry, String> {
    validate(&mut entry)?;
    let existing = get(conn, &entry.id)?;
    if existing.as_ref().map_or(0, |e| e.version) != expected {
        return Err("memory_conflict".into());
    }
    let duplicate: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM memory_entries WHERE title_key=?1 AND id<>?2)",
            params![key(&entry.title), entry.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if duplicate {
        return Err("memory_duplicate_title".into());
    }
    for id in &entry.sources {
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM memory_sources WHERE id=?1)",
                [id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !exists {
            return Err("memory_invalid:source".into());
        }
    }
    entry.version = expected + 1;
    entry.created_at = existing.as_ref().map_or_else(now, |e| e.created_at);
    entry.updated_at = now();
    let serialized = |v: &Vec<String>| serde_json::to_string(v).map_err(|e| e.to_string());
    conn.execute("INSERT INTO memory_entries(id,title,title_key,summary,content,tags,related,sources,version,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11) ON CONFLICT(id) DO UPDATE SET title=excluded.title,title_key=excluded.title_key,summary=excluded.summary,content=excluded.content,tags=excluded.tags,related=excluded.related,sources=excluded.sources,version=excluded.version,updated_at=excluded.updated_at",
        params![entry.id, entry.title, key(&entry.title), entry.summary, entry.content, serialized(&entry.tags)?, serialized(&entry.related)?, serialized(&entry.sources)?, entry.version, entry.created_at, entry.updated_at]).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO memory_versions(entry_id,version,snapshot,author,created_at) VALUES(?1,?2,?3,?4,?5)", params![entry.id, entry.version, serde_json::to_string(&entry).map_err(|e| e.to_string())?, author, entry.updated_at]).map_err(|e| e.to_string())?;
    Ok(entry)
}

pub fn validate_links(conn: &Connection, entries: &[Entry]) -> Result<(), String> {
    for entry in entries {
        for id in &entry.related {
            if get(conn, id)?.is_none() {
                return Err("memory_invalid:related".into());
            }
        }
    }
    Ok(())
}

pub fn edit(conn: &Connection, input: Edit, author: &str) -> Result<Entry, String> {
    let prior = input
        .id
        .as_deref()
        .map(|id| get(conn, id))
        .transpose()?
        .flatten();
    if input.id.is_some() && prior.is_none() {
        return Err("memory_conflict".into());
    }
    let entry = Entry {
        id: input.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        title: input.title,
        summary: input.summary,
        content: input.content,
        tags: input.tags,
        related: input.related,
        sources: prior.map_or_else(Vec::new, |e| e.sources),
        version: 0,
        created_at: 0,
        updated_at: 0,
    };
    validate_links(conn, std::slice::from_ref(&entry))?;
    put(conn, entry, input.version, author)
}

pub fn list(app: &AppCtx, args: &Value) -> Result<Value, String> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if query.chars().count() > 500 {
        return Err("memory_invalid:query".into());
    }
    let tag = args.get("tag").and_then(Value::as_str).unwrap_or("");
    let page = args
        .get("page")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(100_000);
    let order = if args.get("sort").and_then(Value::as_str) == Some("title") {
        "title_key ASC"
    } else {
        "updated_at DESC,id"
    };
    let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
    let mut conditions = vec![
        "(?1='' OR EXISTS(SELECT 1 FROM json_each(memory_entries.tags) WHERE value=?1))"
            .to_string(),
    ];
    let mut values: Vec<rusqlite::types::Value> = vec![tag.to_string().into()];
    for word in query.split_whitespace() {
        let n = values.len() + 1;
        if word.chars().count() >= 3 {
            conditions.push(format!(
                "rowid IN (SELECT rowid FROM memory_fts WHERE memory_fts MATCH ?{n})"
            ));
            values.push(format!("\"{}\"", word.replace('"', "\"\"")).into());
        } else {
            conditions.push(format!(
                "instr(lower(title||' '||summary||' '||content||' '||tags),lower(?{n}))>0"
            ));
            values.push(word.to_string().into());
        }
    }
    let filter = conditions.join(" AND ");
    let total: i64 = conn
        .query_row(
            &format!("SELECT count(*) FROM memory_entries WHERE {filter}"),
            rusqlite::params_from_iter(values.iter()),
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let sql = format!(
        "SELECT {COLS} FROM memory_entries WHERE {filter} ORDER BY {order} LIMIT 40 OFFSET {}",
        page * 40
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let entries = stmt
        .query_map(rusqlite::params_from_iter(values.iter()), row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    // The list only returns snippets. Complete documents are fetched individually for reading/editing.
    let entries: Vec<Value> = entries.into_iter().map(|e| json!({"id":e.id,"title":e.title,"summary":e.summary,"tags":e.tags,"version":e.version,"updatedAt":e.updated_at,"sourceCount":e.sources.len()})).collect();
    let mut stmt = conn.prepare("SELECT DISTINCT value FROM memory_entries,json_each(memory_entries.tags) ORDER BY value").map_err(|e| e.to_string())?;
    let tags = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(json!({"entries":entries,"total":total,"pageSize":40,"tags":tags}))
}

pub fn detail(app: &AppCtx, id: &str, version: Option<i64>) -> Result<Value, String> {
    let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
    let entry = get(&conn, id)?.ok_or("memory_not_found")?;
    let mut stmt = conn
        .prepare("SELECT id,title FROM memory_entries ORDER BY title_key")
        .map_err(|e| e.to_string())?;
    let catalog = stmt
        .query_map([], |r| {
            Ok(json!({"id":r.get::<_,String>(0)?,"title":r.get::<_,String>(1)?}))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT id,title FROM memory_entries WHERE EXISTS(SELECT 1 FROM json_each(related) WHERE value=?1)").map_err(|e| e.to_string())?;
    let backlinks = stmt
        .query_map([id], |r| {
            Ok(json!({"id":r.get::<_,String>(0)?,"title":r.get::<_,String>(1)?}))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT version,author,created_at FROM memory_versions WHERE entry_id=?1 ORDER BY version DESC").map_err(|e| e.to_string())?;
    let versions = stmt.query_map([id], |r| Ok(json!({"version":r.get::<_,i64>(0)?,"author":r.get::<_,String>(1)?,"createdAt":r.get::<_,i64>(2)?}))).map_err(|e| e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())?;
    let revision: Option<Entry> = match version {
        Some(version) => {
            let raw: String = conn
                .query_row(
                    "SELECT snapshot FROM memory_versions WHERE entry_id=?1 AND version=?2",
                    params![id, version],
                    |r| r.get(0),
                )
                .map_err(|_| "memory_not_found")?;
            Some(serde_json::from_str(&raw).map_err(|e| format!("{e}"))?)
        }
        None => None,
    };
    let source_ids: HashSet<&str> = revision
        .as_ref()
        .unwrap_or(&entry)
        .sources
        .iter()
        .map(String::as_str)
        .collect();
    let mut stmt = conn.prepare("SELECT id,session_name,kind,created_at,digest,session_id FROM memory_sources ORDER BY created_at").map_err(|e| e.to_string())?;
    let sources = stmt.query_map([], |r| Ok(json!({"id":r.get::<_,String>(0)?,"sessionName":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?,"createdAt":r.get::<_,i64>(3)?,"digest":r.get::<_,String>(4)?,"sessionId":r.get::<_,String>(5)?}))).map_err(|e| e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e| e.to_string())?.into_iter().filter(|s| source_ids.contains(s["id"].as_str().unwrap_or(""))).collect::<Vec<_>>();
    Ok(
        json!({"entry":entry,"revision":revision,"catalog":catalog,"backlinks":backlinks,"versions":versions,"sources":sources}),
    )
}

pub fn restore(app: &AppCtx, id: &str, args: &Value) -> Result<Value, String> {
    let target = args
        .get("targetVersion")
        .and_then(Value::as_i64)
        .ok_or("memory_invalid:version")?;
    let expected = args
        .get("version")
        .and_then(Value::as_i64)
        .ok_or("memory_invalid:version")?;
    let mut conn = app.db().conn.lock().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let snapshot: String = tx
        .query_row(
            "SELECT snapshot FROM memory_versions WHERE entry_id=?1 AND version=?2",
            params![id, target],
            |r| r.get(0),
        )
        .map_err(|_| "memory_not_found")?;
    let mut entry: Entry = serde_json::from_str(&snapshot).map_err(|e| e.to_string())?;
    // Deleted targets are no longer navigable; restoring text must not resurrect deleted documents.
    let live: HashSet<String> = all(&tx)?.into_iter().map(|e| e.id).collect();
    entry.related.retain(|id| live.contains(id));
    let result = put(&tx, entry, expected, "restore")?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(json!(result))
}

pub fn source(app: &AppCtx, id: &str) -> Result<Value, String> {
    let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
    conn.query_row("SELECT id,session_id,session_name,kind,agent_session_id,digest,content,created_at FROM memory_sources WHERE id=?1", [id], |r| Ok(json!({"id":r.get::<_,String>(0)?,"sessionId":r.get::<_,String>(1)?,"sessionName":r.get::<_,String>(2)?,"kind":r.get::<_,String>(3)?,"agentSessionId":r.get::<_,String>(4)?,"digest":r.get::<_,String>(5)?,"content":r.get::<_,String>(6)?,"createdAt":r.get::<_,i64>(7)?}))).optional().map_err(|e| e.to_string())?.ok_or("memory_not_found".into())
}
