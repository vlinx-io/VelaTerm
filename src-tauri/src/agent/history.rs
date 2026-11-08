//! Read native histories without changing their files or starting an agent.

use std::collections::HashSet;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::db::repo;
use crate::host::{AppCtx, TREE_CHANGED};
use crate::models::{Session, SessionKind};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalSession {
    pub kind: SessionKind,
    pub agent_session_id: String,
    pub title: String,
    pub cwd: String,
    pub updated_at: i64,
    pub imported: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub kind: SessionKind,
    pub agent_session_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct History {
    directory: String,
    sessions: Vec<HistoricalSession>,
    warnings: Vec<String>,
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 200
        && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

fn title(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(160).collect()
}

/// No symlink traversal: history roots may contain links back to ancestors.
fn files(root: &Path, extension: &str, recursive: bool) -> Result<Vec<PathBuf>, String> {
    let mut stack = vec![root.to_path_buf()];
    let mut out = Vec::new();
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("Cannot read history directory: {e}")),
        };
        for entry in entries {
            let entry = entry.map_err(|e| format!("Cannot read history entry: {e}"))?;
            let ty = entry.file_type().map_err(|e| e.to_string())?;
            if ty.is_dir() && recursive { stack.push(entry.path()); }
            if ty.is_file() && entry.path().extension().and_then(|x| x.to_str()) == Some(extension) {
                out.push(entry.path());
            }
        }
    }
    Ok(out)
}

/// Last modification of an open history file in milliseconds, or 0 when the platform cannot say.
fn modified_millis(file: &std::fs::File) -> i64 {
    file.metadata().ok().and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// The text blocks of a message's content, which is either a plain string or a list of typed blocks.
fn content_text(content: &Value) -> String {
    content.as_str().map(str::to_string).unwrap_or_else(|| {
        content.as_array().map(|a| a.iter().filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join(" ")).unwrap_or_default()
    })
}

fn jsonl(path: &Path, kind: SessionKind, directory: &Path) -> Result<Option<HistoricalSession>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let updated_at = modified_millis(&file);
    // Only opening metadata and a title are needed. Large initial context can occupy megabytes;
    // missing titles fall back to the native ID and do not make a session unavailable.
    let mut id = String::new();
    let mut cwd = String::new();
    let mut name = String::new();
    let mut bytes_read = 0;
    for line in std::io::BufReader::new(file).lines() {
        if bytes_read >= 4 * 1024 * 1024 && !id.is_empty() && !cwd.is_empty() { break; }
        let line = line.map_err(|e| e.to_string())?;
        bytes_read += line.len();
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue; };
        if kind == SessionKind::Codex {
            if v["type"] == "session_meta" {
                // Subagent threads are helpers of another conversation, like Claude sidechains.
                if !super::resume::is_user_thread_payload(&v["payload"]) { return Ok(None); }
                id = v["payload"]["id"].as_str().unwrap_or_default().to_string();
                cwd = v["payload"]["cwd"].as_str().unwrap_or_default().to_string();
            }
            if v["type"] == "event_msg" && v["payload"]["type"] == "user_message" {
                name = title(v["payload"]["message"].as_str().unwrap_or_default());
            }
            // Current Codex builds may only persist response_item user messages. Skip the
            // startup instruction message as a whole, including its separate environment block.
            if v["type"] == "response_item" && v["payload"]["role"] == "user" {
                if let Some(content) = v["payload"]["content"].as_array() {
                    let texts = content.iter().filter_map(|c| c["text"].as_str()).collect::<Vec<_>>();
                    let injected = texts.iter().any(|text| {
                        let text = text.trim_start();
                        text.starts_with("<environment_context>") || text.starts_with("<user_instructions>")
                            || text.starts_with("# AGENTS.md instructions")
                            || super::transcript::is_injected_context(text)
                    });
                    if !injected { name = title(&texts.join(" ")); }
                }
            }
        } else {
            if v["isSidechain"] == true { return Ok(None); }
            if id.is_empty() { id = v["sessionId"].as_str().unwrap_or_default().to_string(); }
            if cwd.is_empty() { cwd = v["cwd"].as_str().unwrap_or_default().to_string(); }
            if v["type"] == "user" && v["isMeta"] != true {
                let text = content_text(&v["message"]["content"]);
                if !text.starts_with('<') { name = title(&text); }
            }
        }
        if !cwd.is_empty() && !same_dir(Path::new(&cwd), directory) { return Ok(None); }
        if !id.is_empty() && !cwd.is_empty() && !name.is_empty() { break; }
    }
    if !valid_id(&id) || cwd.is_empty() { return Ok(None); }
    Ok(Some(HistoricalSession {
        kind, title: if name.is_empty() { id.clone() } else { name },
        agent_session_id: id, cwd, updated_at, imported: false,
    }))
}

/// One Pi or OMP session file: a `session` header with `id` and `cwd`, then an append-only entry tree.
///
/// The name can change at any point (Pi appends `session_info` entries, the latest one wins and an empty name
/// clears it; OMP keeps a `title` line at the top that it rewrites in place), so a matching file is read to
/// the end. Both write `type` as the first key, so lines that cannot name the session are skipped unparsed.
fn pi_jsonl(path: &Path, kind: SessionKind, directory: &Path) -> Result<Option<HistoricalSession>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let updated_at = modified_millis(&file);
    let mut header: Option<(String, String)> = None;
    let mut named: Option<String> = None;
    let mut titled = String::new();
    let mut prompt = String::new();
    for line in std::io::BufReader::new(file).lines() {
        let line = line.map_err(|e| e.to_string())?;
        let tag = |t: &str| line.strip_prefix(r#"{"type":""#).and_then(|rest| rest.strip_prefix(t))
            .is_some_and(|rest| rest.starts_with('"'));
        let wanted = tag("session") || tag("title") || tag("session_info") || (prompt.is_empty() && tag("message"));
        if line.starts_with(r#"{"type":""#) && !wanted { continue; }
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue; };
        match v["type"].as_str() {
            Some("session") if header.is_none() => {
                let cwd = v["cwd"].as_str().unwrap_or_default();
                if cwd.is_empty() || !same_dir(Path::new(cwd), directory) { return Ok(None); }
                header = Some((v["id"].as_str().unwrap_or_default().to_string(), cwd.to_string()));
                if titled.is_empty() { titled = title(v["title"].as_str().unwrap_or_default()); }
            }
            Some("title") => {
                let t = title(v["title"].as_str().unwrap_or_default());
                if !t.is_empty() { titled = t; }
            }
            Some("session_info") => named = Some(title(v["name"].as_str().unwrap_or_default())),
            Some("message") if v["message"]["role"] == "user" => prompt = title(&content_text(&v["message"]["content"])),
            _ => {}
        }
    }
    let Some((id, cwd)) = header else { return Ok(None); };
    if !valid_id(&id) { return Ok(None); }
    let name = [named.unwrap_or_default(), titled, prompt].into_iter().find(|t| !t.is_empty()).unwrap_or_else(|| id.clone());
    Ok(Some(HistoricalSession { kind, agent_session_id: id, title: name, cwd, updated_at, imported: false }))
}

fn scan_jsonl(root: &Path, kind: SessionKind, directory: &Path, out: &mut Vec<HistoricalSession>) -> Result<(), String> {
    // Claude, Pi and OMP keep one conversation per file directly under a per-directory folder; anything deeper
    // (Claude subagents, OMP's per-session artifacts and subagent logs) is not a resumable root.
    let paths = if matches!(kind, SessionKind::Claude | SessionKind::Pi | SessionKind::Omp) {
        let mut paths = Vec::new();
        match std::fs::read_dir(root) {
            Ok(entries) => for entry in entries {
                let entry = entry.map_err(|e| e.to_string())?;
                if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                    paths.extend(files(&entry.path(), "jsonl", false)?);
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.to_string()),
        }
        paths
    } else { files(root, "jsonl", true)? };
    let mut failures = 0;
    for path in paths {
        let found = if matches!(kind, SessionKind::Pi | SessionKind::Omp) { pi_jsonl(&path, kind, directory) }
            else { jsonl(&path, kind, directory) };
        match found {
            Ok(Some(s)) => out.push(s),
            Ok(None) => {},
            Err(_) => failures += 1,
        }
    }
    if failures > 0 { return Err(format!("Could not read {failures} history files")); }
    Ok(())
}

fn scan_opencode(root: &Path, directory: &Path, out: &mut Vec<HistoricalSession>) -> Result<(), String> {
    let db = std::env::var_os("OPENCODE_DB").filter(|p|!p.is_empty()).map(PathBuf::from).unwrap_or_else(||root.join("opencode.db"));
    if db.exists() {
        let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string())?;
        conn.busy_timeout(std::time::Duration::from_secs(2)).map_err(|e| e.to_string())?;
        for table in ["session","session_v2"] {
        if !crate::agent::opencode_store::has_table(&conn,table) {continue;}
        let mut stmt = conn.prepare(&format!("SELECT id, COALESCE(title,''), directory, time_updated FROM {table} WHERE parent_id IS NULL"))
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |r| Ok(HistoricalSession {
            kind: SessionKind::Opencode, agent_session_id: r.get(0)?, title: r.get(1)?,
            cwd: r.get(2)?, updated_at: r.get(3)?, imported: false,
        })).map_err(|e| e.to_string())?;
        for row in rows {
            let mut s = row.map_err(|e| e.to_string())?;
            if valid_id(&s.agent_session_id) && same_dir(Path::new(&s.cwd), directory) {
                s.title = title(&s.title);
                if s.title.is_empty() { s.title = s.agent_session_id.clone(); }
                out.push(s);
            }
        }
        }
    }
    // Older OpenCode releases keep one JSON object per session.
    for path in files(&root.join("storage/session"), "json", true)? {
        let v: Value = serde_json::from_reader(std::fs::File::open(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let id = v["id"].as_str().unwrap_or_default();
        let cwd = v["directory"].as_str().unwrap_or_default();
        if !valid_id(id) || v["parentID"].as_str().is_some() || !same_dir(Path::new(cwd), directory) { continue; }
        let name = title(v["title"].as_str().unwrap_or(id));
        out.push(HistoricalSession {
            kind: SessionKind::Opencode, agent_session_id: id.to_string(),
            title: if name.is_empty() { id.to_string() } else { name }, cwd: cwd.to_string(),
            updated_at: v["time"]["updated"].as_i64().unwrap_or(0), imported: false,
        });
    }
    Ok(())
}

/// ZCode's session table (`~/.zcode/cli/db/db.sqlite`). Only top-level conversations a person started are listed:
/// soft-deleted rows (`time_deleted`) cannot be resumed, and side chats, workflow children and subagent children
/// (`task_type`) belong to another conversation. Older databases without those columns list every root row.
fn scan_zcode(db: Option<PathBuf>, directory: &Path, out: &mut Vec<HistoricalSession>) -> Result<(), String> {
    let Some(db) = db.filter(|db| db.is_file()) else { return Ok(()); };
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| e.to_string())?;
    // ZCode writes the same WAL database while it runs.
    conn.busy_timeout(std::time::Duration::from_secs(2)).map_err(|e| e.to_string())?;
    if !crate::agent::opencode_store::has_table(&conn, "session") { return Ok(()); }
    let has_column = |name: &str| conn.query_row("SELECT 1 FROM pragma_table_info('session') WHERE name = ?1", [name], |_| Ok(()))
        .optional().map(|found| found.is_some()).map_err(|e| e.to_string());
    let mut filter = vec!["parent_id IS NULL"];
    if has_column("time_deleted")? { filter.push("time_deleted IS NULL"); }
    if has_column("task_type")? { filter.push("task_type IN ('interactive', 'fork')"); }
    let mut stmt = conn.prepare(&format!("SELECT id, COALESCE(title, ''), directory, time_updated FROM session WHERE {}",
        filter.join(" AND "))).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| Ok(HistoricalSession {
        kind: SessionKind::Zcode, agent_session_id: r.get(0)?, title: r.get(1)?,
        cwd: r.get(2)?, updated_at: r.get(3)?, imported: false,
    })).map_err(|e| e.to_string())?;
    for row in rows {
        let mut s = row.map_err(|e| e.to_string())?;
        if valid_id(&s.agent_session_id) && same_dir(Path::new(&s.cwd), directory) {
            s.title = title(&s.title);
            if s.title.is_empty() { s.title = s.agent_session_id.clone(); }
            out.push(s);
        }
    }
    Ok(())
}

fn scan_deepseek(directory: &Path, out: &mut Vec<HistoricalSession>) -> Result<(), String> {
    let Some(root) = crate::agent::chat::deepseek_history::home() else { return Ok(()); };
    let (found, failures) = crate::agent::chat::deepseek_history::list_in(&root, |cwd| same_dir(Path::new(cwd), directory))?;
    out.extend(found.into_iter().filter(|s| valid_id(&s.id)).map(|s| HistoricalSession {
        kind: SessionKind::Deepseek, title: if s.title.is_empty() { s.id.clone() } else { title(&s.title) },
        agent_session_id: s.id, cwd: s.cwd, updated_at: s.updated_at, imported: false,
    }));
    if failures > 0 { return Err(format!("Could not read {failures} history files")); }
    Ok(())
}

fn project_directory(conn: &Connection, project_id: &str) -> Result<String, String> {
    let root: String = conn.query_row("SELECT root_path FROM projects WHERE id = ?1", [project_id], |r| r.get(0))
        .optional().map_err(|e| e.to_string())?.ok_or("Project not found")?;
    if root.is_empty() || !Path::new(&root).is_dir() { return Err("Project directory is unavailable".into()); }
    Ok(root)
}

/// Audit counts and timings only; history titles, prompts and paths never enter logs.
fn audit(job_id: &str, step: &str, status: &str, input: usize, output: usize, duration: u128) {
    crate::diagnostics::record(if status=="failed"{"ERROR"}else{"INFO"},"session_import",serde_json::json!({"jobId":job_id,"step":step,"status":status,"inputCount":input,"outputCount":output,"durationMs":duration as u64}));
}

pub fn discover(ctx: &AppCtx, project_id: &str) -> Result<History, String> {
    let job = uuid::Uuid::new_v4().to_string();
    let started = std::time::Instant::now();
    audit(&job, "discover", "started", 1, 0, 0);
    let result = discover_inner(ctx, project_id);
    let status = match &result { Ok(h) if !h.warnings.is_empty() => "partial", Ok(_) => "success", Err(_) => "failed" };
    audit(&job, "discover", status, 1, result.as_ref().map(|h| h.sessions.len()).unwrap_or(0), started.elapsed().as_millis());
    result
}

fn discover_inner(ctx: &AppCtx, project_id: &str) -> Result<History, String> {
    let directory = project_directory(&ctx.db().conn.lock().unwrap(), project_id)?;
    let home = crate::host::home_dir().ok_or("Home directory is unavailable")?;
    let codex = super::resume::codex_home().ok_or("Codex home is unavailable")?;
    let claude = super::resume::claude_home().ok_or("Claude home is unavailable")?;
    let opencode = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share")).join("opencode");
    let mut sessions = Vec::new();
    let mut warnings = Vec::new();
    for (label, result) in [
        ("Codex", scan_jsonl(&codex.join("sessions"), SessionKind::Codex, Path::new(&directory), &mut sessions)),
        ("Claude", scan_jsonl(&claude.join("projects"), SessionKind::Claude, Path::new(&directory), &mut sessions)),
        ("OpenCode", scan_opencode(&opencode, Path::new(&directory), &mut sessions)),
        ("Pi", super::resume::pi_agent_home(SessionKind::Pi).map_or(Ok(()), |home|
            scan_jsonl(&home.join("sessions"), SessionKind::Pi, Path::new(&directory), &mut sessions))),
        ("OMP", super::resume::pi_agent_home(SessionKind::Omp).map_or(Ok(()), |home|
            scan_jsonl(&home.join("sessions"), SessionKind::Omp, Path::new(&directory), &mut sessions))),
        ("ZCode", scan_zcode(crate::agent::zcode::session_db_path(), Path::new(&directory), &mut sessions)),
        ("DeepSeek", scan_deepseek(Path::new(&directory), &mut sessions)),
    ] {
        if let Err(e) = result { warnings.push(format!("{label}: {e}")); }
    }
    match super::kiro_store::Roots::native() {
        Ok(roots) => scan_kiro(&roots, Path::new(&directory), &mut sessions, &mut warnings),
        Err(e) => warnings.push(e),
    }
    // Prefer user-assigned Codex names when the optional native title index is available.
    match std::fs::File::open(codex.join("session_index.jsonl")) {
        Ok(file) => for line in std::io::BufReader::new(file).lines() {
            let Ok(line) = line else { warnings.push("Codex: Cannot read the session title index".into()); break; };
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue; };
            if let (Some(id), Some(name)) = (v["id"].as_str(), v["thread_name"].as_str()) {
                let name = title(name);
                if !name.is_empty() {
                    for s in sessions.iter_mut().filter(|s| s.kind == SessionKind::Codex && s.agent_session_id == id) {
                        s.title = name.clone();
                    }
                }
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(_) => warnings.push("Codex: Cannot read the session title index".into()),
    }
    let conn = ctx.db().conn.lock().unwrap();
    let existing = repo::list_all_sessions(&conn)?;
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.agent_session_id.cmp(&b.agent_session_id)));
    let mut seen = HashSet::new();
    sessions.retain(|s| seen.insert((s.kind.as_str(), s.agent_session_id.clone())));
    for s in &mut sessions {
        s.imported = existing.iter().any(|e| e.kind == s.kind && e.agent_session_id.as_deref() == Some(&s.agent_session_id));
    }
    Ok(History { directory, sessions, warnings })
}

fn scan_kiro(roots: &super::kiro_store::Roots, directory: &Path, sessions: &mut Vec<HistoricalSession>, warnings: &mut Vec<String>) {
    let (found, issues) = super::kiro_store::discover(roots, directory);
    warnings.extend(issues);
    sessions.extend(found.into_iter().map(|s| HistoricalSession {
        kind: SessionKind::Kiro, agent_session_id: s.id, title: s.title,
        cwd: s.cwd.to_string_lossy().into_owned(), updated_at: s.updated_at, imported: false,
    }));
}

/// Re-discover on submission; titles, cwd, defaults and duplicate checks remain server-owned.
pub fn import(ctx: &AppCtx, project_id: &str, selected: Vec<Selection>) -> Result<Vec<Session>, String> {
    let job = uuid::Uuid::new_v4().to_string();
    let started = std::time::Instant::now();
    let count = selected.len();
    audit(&job, "import", "started", count, 0, 0);
    let result = (|| {
        if selected.is_empty() { return Err("Select at least one session".into()); }
        let history = discover_inner(ctx, project_id)?;
        audit(&job, "discover", if history.warnings.is_empty() { "success" } else { "partial" }, count, history.sessions.len(), started.elapsed().as_millis());
        import_discovered(ctx, project_id, selected, history)
    })();
    audit(&job, "import", if result.is_ok() { "success" } else { "failed" }, count,
        result.as_ref().map(Vec::len).unwrap_or(0), started.elapsed().as_millis());
    result
}

fn import_discovered(ctx: &AppCtx, project_id: &str, selected: Vec<Selection>, history: History) -> Result<Vec<Session>, String> {
    let candidates = selected.iter().map(|pick| history.sessions.iter()
        .find(|s| s.kind == pick.kind && s.agent_session_id == pick.agent_session_id)
        .ok_or_else(|| "A selected session is no longer available in this directory. Refresh and try again.".to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let imported = {
        let mut conn = ctx.db().conn.lock().unwrap();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        if project_directory(&tx, project_id)? != history.directory { return Err("Project directory changed. Refresh and try again.".into()); }
        let settings = repo::get_app_settings(&tx)?;
        let settings: Value = settings.get("vlx-settings").map(|s| serde_json::from_str(s))
            .transpose().map_err(|e| format!("Cannot read agent defaults: {e}"))?.unwrap_or(Value::Null);
        let mut imported = Vec::new();
        for s in candidates {
            let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE kind = ?1 AND agent_session_id = ?2)",
                rusqlite::params![s.kind.as_str(), s.agent_session_id], |r| r.get(0)).map_err(|e| e.to_string())?;
            if exists { continue; }
            let defaults = &settings["agentDefaults"][s.kind.as_str()];
            // Imported conversations retain the agent's terminal engine and native resume identity.
            let mut record = repo::create_session_full(&tx, project_id, None, &s.title, s.kind,
                None, Some(&s.cwd), None, None, None, defaults["args"].as_str(),
                defaults["permissionMode"].as_str(), None, None, None, None)?;
            repo::set_agent_session_id(&tx, &record.id, &s.agent_session_id, s.kind)?;
            record.agent_session_id = Some(s.agent_session_id.clone());
            imported.push(record);
        }
        tx.commit().map_err(|e| e.to_string())?;
        imported
    };
    if !imported.is_empty() { ctx.emit(TREE_CHANGED, ()); }
    Ok(imported)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("vlx-history-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Temp { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    fn write(path: &Path, values: &[Value]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, values.iter().map(Value::to_string).collect::<Vec<_>>().join("\n")).unwrap();
    }

    #[test]
    fn native_jsonl_matches_directory_and_excludes_claude_subagents() {
        let temp = Temp::new();
        let cwd = temp.0.to_string_lossy().to_string();
        let codex = temp.0.join("codex/2026/09/rollout-test.jsonl");
        write(&codex, &[
            serde_json::json!({"type":"session_meta","payload":{"id":"codex-1","cwd":cwd}}),
            serde_json::json!({"type":"response_item","payload":{"role":"user","content":"Injected context"}}),
            serde_json::json!({"type":"event_msg","payload":{"type":"user_message","message":"Repair\n the editor"}}),
        ]);
        let session = jsonl(&codex, SessionKind::Codex, &temp.0).unwrap().unwrap();
        assert_eq!(session.title, "Repair the editor");
        let current = temp.0.join("codex/current.jsonl");
        write(&current, &[
            serde_json::json!({"type":"session_meta","payload":{"id":"codex-2","cwd":cwd}}),
            serde_json::json!({"type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions"},{"type":"input_text","text":"<environment_context>context</environment_context>"}]}}),
            serde_json::json!({"type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"Fix current history"}]}}),
        ]);
        assert_eq!(jsonl(&current, SessionKind::Codex, &temp.0).unwrap().unwrap().title, "Fix current history");
        assert!(jsonl(&codex, SessionKind::Codex, &temp.0.join("another")).unwrap().is_none());
        let subagent = temp.0.join("codex/subagent.jsonl");
        write(&subagent, &[
            serde_json::json!({"type":"session_meta","payload":{"id":"codex-3","cwd":cwd,"thread_source":"subagent",
                "source":{"subagent":{"thread_spawn":{"parent_thread_id":"codex-2","depth":1}}}}}),
            serde_json::json!({"type":"event_msg","payload":{"type":"user_message","message":"Fix current history"}}),
        ]);
        assert!(jsonl(&subagent, SessionKind::Codex, &temp.0).unwrap().is_none());
        let claude = temp.0.join("claude/project/c1.jsonl");
        write(&claude, &[
            serde_json::json!({"type":"queue-operation","sessionId":"claude-1"}),
            serde_json::json!({"type":"user","sessionId":"claude-1","cwd":cwd,"message":{"content":[{"type":"text","text":"Fix navigation"}]}}),
        ]);
        write(&temp.0.join("claude/project/c1/subagents/agent-1.jsonl"), &[
            serde_json::json!({"type":"user","sessionId":"child-1","cwd":cwd,"message":{"content":"child"}}),
        ]);
        let mut found = Vec::new();
        scan_jsonl(&temp.0.join("claude"), SessionKind::Claude, &temp.0, &mut found).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Fix navigation");
        write(&claude, &[serde_json::json!({"type":"user","isSidechain":true,"sessionId":"child","cwd":cwd})]);
        assert!(jsonl(&claude, SessionKind::Claude, &temp.0).unwrap().is_none());
        assert!(!valid_id("id; command"));
    }

    #[test]
    fn opencode_reads_sqlite_and_legacy_without_writing_source() {
        let temp = Temp::new();
        let conn = Connection::open(temp.0.join("opencode.db")).unwrap();
        conn.execute_batch("CREATE TABLE session(id TEXT,title TEXT,directory TEXT,time_updated INTEGER,parent_id TEXT);").unwrap();
        conn.execute("INSERT INTO session VALUES('ses_main','Main',?1,12,NULL),('ses_child','Child',?1,13,'ses_main'),('ses_other','Other','/elsewhere',14,NULL)",
            [temp.0.to_string_lossy().as_ref()]).unwrap();
        drop(conn);
        let before = std::fs::read(temp.0.join("opencode.db")).unwrap();
        write(&temp.0.join("storage/session/project/legacy.json"), &[
            serde_json::json!({"id":"ses_legacy","directory":temp.0,"title":"Legacy","time":{"updated":11}}),
        ]);
        let mut found = Vec::new();
        scan_opencode(&temp.0, &temp.0, &mut found).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].agent_session_id, "ses_main");
        assert_eq!(found[1].agent_session_id, "ses_legacy");
        assert_eq!(std::fs::read(temp.0.join("opencode.db")).unwrap(), before);
    }

    #[test]
    fn import_is_atomic_deduplicated_and_uses_backend_defaults() {
        let temp = Temp::new();
        let db = crate::db::Db::open(&temp.0.join("app.db")).unwrap();
        let ctx = AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(temp.0.clone(), db)));
        let project = crate::command_core::import_project(&ctx, temp.0.to_str().unwrap()).unwrap();
        let history = || History {
            directory: project.root_path.clone(), warnings: Vec::new(),
            sessions: [SessionKind::Codex, SessionKind::Claude, SessionKind::Opencode].into_iter().map(|kind| HistoricalSession {
                kind, agent_session_id: format!("native-{}", kind.as_str()), title: "Native title".into(),
                cwd: project.root_path.clone(), updated_at: 12, imported: false,
            }).collect(),
        };
        let picks = || history().sessions.iter().map(|s| Selection { kind:s.kind, agent_session_id:s.agent_session_id.clone() }).collect();
        {
            let conn = ctx.db().conn.lock().unwrap();
            repo::set_app_settings(&conn, &std::collections::HashMap::from([("vlx-settings".into(),
                serde_json::json!({"agentDefaults":{"codex":{"args":"--model test", "permissionMode":"skip"}}}).to_string())])).unwrap();
            conn.execute_batch("CREATE TRIGGER reject_claude BEFORE INSERT ON sessions WHEN NEW.kind = 'claude' BEGIN SELECT RAISE(ABORT, 'test failure'); END;").unwrap();
        }
        assert!(import_discovered(&ctx, &project.id, picks(), history()).is_err());
        {
            let conn = ctx.db().conn.lock().unwrap();
            assert!(repo::list_all_sessions(&conn).unwrap().is_empty());
            conn.execute_batch("DROP TRIGGER reject_claude;").unwrap();
        }
        let records = import_discovered(&ctx, &project.id, picks(), history()).unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].agent_args.as_deref(), Some("--model test"));
        assert_eq!(records[0].permission_mode.as_deref(), Some("skip"));
        assert_eq!(records[0].cwd.as_deref(), Some(project.root_path.as_str()));
        assert_eq!(records[0].agent_session_id.as_deref(), Some("native-codex"));
        {
            let conn = ctx.db().conn.lock().unwrap();
            conn.execute("UPDATE sessions SET archived_at = 10 WHERE kind = 'codex'", []).unwrap();
        }
        assert!(import_discovered(&ctx, &project.id, picks(), history()).unwrap().is_empty());
        let invalid = vec![Selection { kind: SessionKind::Codex, agent_session_id:"foreign-session".into() }];
        assert!(import_discovered(&ctx, &project.id, invalid, history()).is_err());
    }

    #[test]
    fn kiro_discovery_import_preserves_native_identity_and_is_repeatable() {
        let native = super::super::kiro_store::tests::Fixture::new();
        native.pair("kiro-first", &native.dir, "First native question");
        native.pair("kiro-second", &native.dir, "Second native question");
        native.pair("kiro-other", &native.dir.join("other"), "Another project");
        let db = crate::db::Db::open(&native.dir.join("app.db")).unwrap();
        let ctx = AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(native.dir.clone(), db)));
        let project = crate::command_core::import_project(&ctx, native.dir.to_str().unwrap()).unwrap();
        let history = || {
            let mut sessions=Vec::new(); let mut warnings=Vec::new();
            scan_kiro(&native.roots, &native.dir, &mut sessions, &mut warnings);
            assert!(warnings.is_empty());
            History { directory:project.root_path.clone(),sessions,warnings }
        };
        let picks = || vec![Selection {kind:SessionKind::Kiro,agent_session_id:"kiro-first".into()},
            Selection {kind:SessionKind::Kiro,agent_session_id:"kiro-first".into()},
            Selection {kind:SessionKind::Kiro,agent_session_id:"kiro-second".into()}];
        let records = import_discovered(&ctx, &project.id, picks(), history()).unwrap();
        assert_eq!(records.len(),2);
        assert_eq!(records[0].agent_session_id.as_deref(),Some("kiro-first"));
        assert_eq!(records[0].cwd.as_deref(),native.dir.to_str());
        assert!(same_dir(Path::new(records[0].cwd.as_deref().unwrap()),Path::new(&project.root_path)));
        assert_eq!(records[0].engine,"tui");
        assert!(import_discovered(&ctx, &project.id, picks(), history()).unwrap().is_empty());
        let invalid=vec![Selection {kind:SessionKind::Kiro,agent_session_id:"kiro-other".into()}];
        assert!(import_discovered(&ctx, &project.id, invalid, history()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn matching_resolves_symlink_without_traversing_history_links() {
        let temp = Temp::new();
        let link = temp.0.join("alias");
        std::os::unix::fs::symlink(&temp.0, &link).unwrap();
        assert!(same_dir(&temp.0, &link));
        assert!(files(&temp.0, "jsonl", true).unwrap().is_empty());
    }

    #[test]
    fn pi_and_omp_files_take_their_latest_name_and_skip_nested_logs() {
        let temp = Temp::new();
        let cwd = temp.0.to_string_lossy().to_string();
        let pi = temp.0.join("pi/--w--/2026-07-16T02-29-48-720Z_pi-1.jsonl");
        write(&pi, &[
            serde_json::json!({"type":"session","version":3,"id":"pi-1","cwd":cwd}),
            serde_json::json!({"type":"message","id":"a","message":{"role":"user","content":[{"type":"text","text":"Fix\n the build"}]}}),
            serde_json::json!({"type":"session_info","id":"b","name":"Old name"}),
            serde_json::json!({"type":"session_info","id":"c","name":"Build fix"}),
        ]);
        let session = pi_jsonl(&pi, SessionKind::Pi, &temp.0).unwrap().unwrap();
        assert_eq!((session.agent_session_id.as_str(), session.title.as_str()), ("pi-1", "Build fix"));
        // An empty name clears it, so the first prompt names the session again.
        write(&pi, &[
            serde_json::json!({"type":"session","version":3,"id":"pi-1","cwd":cwd}),
            serde_json::json!({"type":"message","id":"a","message":{"role":"user","content":"Fix the build"}}),
            serde_json::json!({"type":"session_info","id":"c","name":""}),
        ]);
        assert_eq!(pi_jsonl(&pi, SessionKind::Pi, &temp.0).unwrap().unwrap().title, "Fix the build");
        assert!(pi_jsonl(&pi, SessionKind::Pi, &temp.0.join("another")).unwrap().is_none());
        let omp = temp.0.join("omp/-w/2026-09-12T02-12-18-240Z_omp-1.jsonl");
        write(&omp, &[
            serde_json::json!({"type":"title","v":1,"title":"Default sessions to TUI","source":"auto"}),
            serde_json::json!({"type":"session","version":3,"id":"omp-1","cwd":cwd,"title":"Earlier title"}),
            serde_json::json!({"type":"message","id":"a","message":{"role":"user","content":"Do it"}}),
        ]);
        write(&temp.0.join("omp/-w/2026-09-12T02-12-18-240Z_omp-1/Helper.jsonl"), &[
            serde_json::json!({"type":"session","version":3,"id":"omp-2","cwd":cwd}),
        ]);
        let mut found = Vec::new();
        scan_jsonl(&temp.0.join("omp"), SessionKind::Omp, &temp.0, &mut found).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].agent_session_id.as_str(), found[0].title.as_str()), ("omp-1", "Default sessions to TUI"));
    }

    #[test]
    fn zcode_lists_live_root_conversations_in_the_directory() {
        let temp = Temp::new();
        let cwd = temp.0.to_string_lossy().to_string();
        let db = temp.0.join("db.sqlite");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE session (id TEXT PRIMARY KEY, parent_id TEXT, directory TEXT NOT NULL,
            title TEXT NOT NULL, time_updated INTEGER NOT NULL, task_type TEXT NOT NULL DEFAULT 'interactive',
            time_deleted INTEGER);").unwrap();
        for (id, parent, dir, task, deleted) in [
            ("sess_live", None, cwd.as_str(), "interactive", None),
            ("sess_child", Some("sess_live"), cwd.as_str(), "subagent_child", None),
            ("sess_side", None, cwd.as_str(), "selection_side_chat", None),
            ("sess_gone", None, cwd.as_str(), "interactive", Some(5)),
            ("sess_other", None, "/elsewhere", "interactive", None),
        ] {
            conn.execute("INSERT INTO session VALUES (?1, ?2, ?3, 'Fix  the\nlogin', 7, ?4, ?5)",
                rusqlite::params![id, parent, dir, task, deleted]).unwrap();
        }
        drop(conn);
        let mut found = Vec::new();
        scan_zcode(Some(db), &temp.0, &mut found).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].agent_session_id.as_str(), found[0].title.as_str()), ("sess_live", "Fix the login"));
        found.clear();
        scan_zcode(Some(temp.0.join("missing.sqlite")), &temp.0, &mut found).unwrap();
        assert!(found.is_empty());
    }
}
