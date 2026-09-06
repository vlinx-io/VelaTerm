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

fn jsonl(path: &Path, kind: SessionKind, directory: &Path) -> Result<Option<HistoricalSession>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let updated_at = file.metadata().ok().and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64).unwrap_or(0);
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
                let content = &v["message"]["content"];
                let text = content.as_str().map(str::to_string).unwrap_or_else(|| {
                    content.as_array().map(|a| a.iter().filter(|b| b["type"] == "text")
                        .filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join(" ")).unwrap_or_default()
                });
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

fn scan_jsonl(root: &Path, kind: SessionKind, directory: &Path, out: &mut Vec<HistoricalSession>) -> Result<(), String> {
    // Claude's top-level project files are conversations; nested subagents are not resumable roots.
    let paths = if kind == SessionKind::Claude {
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
        match jsonl(&path, kind, directory) {
            Ok(Some(s)) => out.push(s),
            Ok(None) => {},
            Err(_) => failures += 1,
        }
    }
    if failures > 0 { return Err(format!("Could not read {failures} history files")); }
    Ok(())
}

fn scan_opencode(root: &Path, directory: &Path, out: &mut Vec<HistoricalSession>) -> Result<(), String> {
    let db = root.join("opencode.db");
    if db.exists() {
        let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string())?;
        conn.busy_timeout(std::time::Duration::from_secs(2)).map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare("SELECT id, title, directory, time_updated FROM session WHERE parent_id IS NULL")
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

fn project_directory(conn: &Connection, project_id: &str) -> Result<String, String> {
    let root: String = conn.query_row("SELECT root_path FROM projects WHERE id = ?1", [project_id], |r| r.get(0))
        .optional().map_err(|e| e.to_string())?.ok_or("Project not found")?;
    if root.is_empty() || !Path::new(&root).is_dir() { return Err("Project directory is unavailable".into()); }
    Ok(root)
}

/// Audit counts and timings only; history titles, prompts and paths never enter logs.
fn audit(job_id: &str, step: &str, status: &str, input: usize, output: usize, duration: u128) {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    eprintln!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} [{:<5}] [system] event=session_import step={} method=native_history inputCount={} outputCount={} jobId={} status={} durationMs={}",
        now.year(), u8::from(now.month()), now.day(), now.hour(), now.minute(), now.second(),
        if status == "failed" { "ERROR" } else { "INFO" }, step, input, output, job_id, status, duration,
    );
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
    ] {
        if let Err(e) = result { warnings.push(format!("{label}: {e}")); }
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
            let mut record = repo::create_session_full(&tx, project_id, None, &s.title, s.kind,
                None, Some(&s.cwd), None, None, None, defaults["args"].as_str(),
                defaults["permissionMode"].as_str(), None, None, None)?;
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

    #[cfg(unix)]
    #[test]
    fn matching_resolves_symlink_without_traversing_history_links() {
        let temp = Temp::new();
        let link = temp.0.join("alias");
        std::os::unix::fs::symlink(&temp.0, &link).unwrap();
        assert!(same_dir(&temp.0, &link));
        assert!(files(&temp.0, "jsonl", true).unwrap().is_empty());
    }
}
