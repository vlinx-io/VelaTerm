//! Z.ai ZCode (`zcode`): starting its agent, listing its models, and reading its session store.
//!
//! ZCode publishes no command-line package. Its agent is the `glm/zcode.cjs` bundle inside the desktop
//! release, which the desktop app starts with its own Electron acting as Node (`ELECTRON_RUN_AS_NODE=1`) and
//! the provider config and search tools of the release passed by environment. VelaTerm starts it the same
//! way when the executable it resolved is the desktop app's own binary. A `zcode` command, such as the shim
//! VelaTerm's one-click install writes to `~/.local/bin` or ZCode's source-built installer, starts directly.
//!
//! VelaTerm drives only the agent's protocol server (`app-server --stdio`), from the conversation view; ZCode
//! has no terminal session, hooks, or plugin of VelaTerm's.

use std::path::{Path, PathBuf};

/// The desktop release's resource directory beside its main binary: `Contents/Resources` in a macOS app
/// bundle, `resources` in a Windows install. Only one that carries the agent bundle counts.
pub(crate) fn desktop_resources(bin: &Path) -> Option<PathBuf> {
    let dir = bin.parent()?;
    let mac = dir.parent().map(|contents| contents.join("Resources"));
    mac.into_iter()
        .chain([dir.join("resources")])
        .find(|resources| resources.join("glm").join("zcode.cjs").is_file())
}

/// The environment the desktop app gives its agent besides `ELECTRON_RUN_AS_NODE`: the provider config the
/// release ships, which the agent otherwise looks for beside its bundle, and the search tools when present.
fn desktop_env(resources: &Path, win: bool) -> Vec<(&'static str, PathBuf)> {
    let mut env = vec![(
        "ZCODE_BUILTIN_PROVIDER_CONFIG_FILE",
        resources.join("config").join("provider").join("zcode-builtin.json"),
    )];
    let exe = |name: &str| if win { format!("{name}.exe") } else { name.to_string() };
    for (key, dir, name) in [
        ("ZCODE_RG_BINARY", "ripgrep", "rg"),
        ("ZCODE_BFS_BINARY", "bfs", "bfs"),
        ("ZCODE_UGREP_BINARY", "ugrep", "ugrep"),
    ] {
        let tool = resources.join("tools").join(dir).join(exe(name));
        if tool.is_file() {
            env.push((key, tool));
        }
    }
    env
}

/// The built-in provider config the agent at `bin` ships with: the desktop release's, or the copy VelaTerm's
/// one-click install keeps beside the `zcode` shim it writes. Unknown for any other `zcode` command.
fn shipped_config(bin: &Path, resources: Option<&Path>) -> Option<PathBuf> {
    if let Some(resources) = resources {
        return Some(resources.join("config").join("provider").join("zcode-builtin.json"));
    }
    let shim = std::fs::read(bin).ok().filter(|bytes| bytes.len() < 4096)?;
    if !String::from_utf8_lossy(&shim).contains("Installed by VelaTerm") {
        return None;
    }
    let config = zcode_home()?.join("vlx-term").join("agent").join("config").join("provider").join("zcode-builtin.json");
    config.is_file().then_some(config)
}

fn exported(key: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(key).filter(|v| !v.is_empty())
}

/// A command that runs ZCode from `bin`, before its subcommand. The desktop app's binary runs the bundled
/// agent as Node; anything else is the `zcode` command itself. A variable the user exported already wins.
///
/// Both provider config files are pinned when the shipped one is known. Given both, ZCode reads them as they
/// are instead of a copy it refreshes on its own, so the account VelaTerm reports can name the revision the
/// server loaded (see `zcode_account`); the release's config is the one its agent was built with.
pub fn command(bin: &str) -> std::process::Command {
    use crate::agent::zcode_account::{BUILTIN_CONFIG, PERSONAL_CONFIG};
    let mut cmd = crate::host::command(bin);
    let resources = desktop_resources(Path::new(bin));
    if let Some(resources) = &resources {
        cmd.arg(resources.join("glm").join("zcode.cjs"));
        cmd.env("ELECTRON_RUN_AS_NODE", "1");
        for (key, value) in desktop_env(resources, cfg!(windows)) {
            if exported(key).is_none() {
                cmd.env(key, value);
            }
        }
    }
    let builtin = exported(BUILTIN_CONFIG).map(PathBuf::from).or_else(|| shipped_config(Path::new(bin), resources.as_deref()));
    let base = exported("ZCODE_DATA_BASE_DIR").map(PathBuf::from).or_else(crate::host::home_dir);
    if let (Some(builtin), None, Some(base)) = (builtin, exported(PERSONAL_CONFIG), base) {
        cmd.env(BUILTIN_CONFIG, builtin);
        cmd.env(PERSONAL_CONFIG, base.join(".zcode").join("v2").join("provider_config.json"));
    }
    cmd
}

/// ZCode resolves every default path from the OS home directory; there is no ZCODE_HOME override.
fn zcode_home() -> Option<PathBuf> {
    crate::host::home_dir().map(|home| home.join(".zcode"))
}

/// The session database, honoring the same overrides ZCode reads (`ZCODE_SESSION_DB_PATH`, then
/// `ZCODE_SESSION_DB`) when VelaTerm itself inherits them.
pub fn session_db_path() -> Option<PathBuf> {
    for key in ["ZCODE_SESSION_DB_PATH", "ZCODE_SESSION_DB"] {
        if let Some(path) = std::env::var_os(key).filter(|p| !p.is_empty()) {
            return Some(PathBuf::from(path));
        }
    }
    zcode_home().map(|home| home.join("cli").join("db").join("db.sqlite"))
}

/// Whether a ZCode session is **confirmed** absent: the database opens and holds no live row for `id`.
/// An unreadable or missing database cannot prove anything, so it returns false and resume is attempted.
pub fn session_missing(id: &str) -> bool {
    session_db_path().is_some_and(|db| session_missing_in(&db, id))
}

fn session_missing_in(db: &Path, id: &str) -> bool {
    use rusqlite::{Connection, OpenFlags, OptionalExtension};
    if !db.is_file() {
        return false;
    }
    let Ok(conn) = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return false;
    };
    // ZCode writes the same WAL database while it runs; wait briefly rather than misread a busy lock.
    let _ = conn.busy_timeout(std::time::Duration::from_secs(2));
    // `time_deleted` marks a soft-deleted session, which `--resume` reports as not found.
    match conn
        .query_row("SELECT time_deleted FROM session WHERE id = ?1", [id], |row| {
            row.get::<_, Option<i64>>(0)
        })
        .optional()
    {
        Ok(None) => true,
        Ok(Some(deleted)) => deleted.is_some(),
        // A schema change or lock error is not evidence of absence.
        Err(_) => false,
    }
}

/// The model catalogue a short-lived `zcode app-server` reports, for a conversation with no running process.
///
/// A draft `session/create` answers with the catalogue and is closed straight away; ZCode persists a session
/// only once it receives a message, so the lookup leaves nothing behind.
pub fn list_models(bin: &str, cwd: &str) -> Result<Vec<serde_json::Value>, String> {
    use crate::agent::chat::zcode_protocol::{self as wire, Incoming};
    use std::io::{BufRead, BufReader, Write};
    use std::process::Stdio;

    let mut cmd = command(bin);
    cmd.arg("app-server").arg("--stdio").current_dir(cwd);
    crate::login_env::refresh_command(&mut cmd);
    crate::agent::executable::prepare_command(&mut cmd, bin);
    for key in crate::pty::manager::AGENT_HARNESS_MARKERS {
        cmd.env_remove(key);
    }
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let account = crate::agent::zcode_account::Account::for_command(&cmd).and_then(|account| account.config());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start ZCode {bin:?} to read its model catalogue: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("ZCode has no input stream")?;
    let stdout = child.stdout.take().ok_or("ZCode has no output stream")?;
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let send = |stdin: &mut std::process::ChildStdin, value: &serde_json::Value| {
        let _ = stdin.write_all(format!("{value}\n").as_bytes()).and_then(|_| stdin.flush());
    };
    // The catalogue lists only the plans the account holds, and the server learns those from the client.
    // It answers in order, so the account is in place before the draft is created.
    if let Some(params) = account {
        send(&mut stdin, &wire::request("vlx-account", "provider/updateAccountConfig", params));
    }
    send(&mut stdin, &wire::request("vlx-catalog", "session/create",
        serde_json::json!({"workspace": {"workspacePath": cwd, "workspaceKey": cwd}})));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut found = None;
    while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
        let Ok(line) = rx.recv_timeout(left) else { break };
        match wire::parse_line(&line) {
            Some(Incoming::Request { id, method, .. }) => {
                let answer = if method == "session/requestRuntimePreferences" {
                    wire::reply(&id, serde_json::json!({"nativeSearchEnhancementsEnabled": false}))
                } else {
                    wire::reply_unsupported(&id)
                };
                send(&mut stdin, &answer);
            }
            Some(Incoming::Response { id, result, error }) if id == "vlx-catalog" => {
                if error.is_none() {
                    if let Some(draft) = result.pointer("/session/sessionId").and_then(serde_json::Value::as_str) {
                        send(&mut stdin, &wire::request("vlx-close", "session/close", serde_json::json!({"sessionId": draft})));
                    }
                    found = result.pointer("/settings/model/available").and_then(serde_json::Value::as_array).cloned();
                }
                break;
            }
            _ => {}
        }
    }
    // Closing stdin lets ZCode shut down on its own; the kill is the bound if it does not.
    drop(stdin);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let _ = child.kill();
    let _ = child.wait();
    let available = found.ok_or("ZCode did not report its model catalogue")?;
    Ok(wire::to_chat_models(&available))
}

/// A ZCode conversation read back from its session store, oldest first, for replay and export.
///
/// Messages and their parts live in the `message` and `part` tables as JSON. User messages ZCode itself
/// injected (environment reminders, tool listings) are marked as not visible and left out, as ZCode's own
/// views do. Nothing here writes.
pub(crate) fn history_events(id: &str) -> Result<Vec<crate::agent::export::Event>, String> {
    let db = session_db_path().ok_or("ZCode's session store could not be located")?;
    history_events_in(&db, id)
}

fn history_events_in(db: &Path, id: &str) -> Result<Vec<crate::agent::export::Event>, String> {
    use crate::agent::export::Event;
    use rusqlite::{Connection, OpenFlags};
    use serde_json::Value;

    if !db.is_file() {
        return Err("ZCode has not recorded any conversation on this machine".to_string());
    }
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| format!("Failed to open ZCode's session store: {e}"))?;
    let _ = conn.busy_timeout(std::time::Duration::from_secs(2));
    let read_rows = |sql: &str| -> Result<Vec<(String, String)>, String> {
        let mut stmt = conn.prepare(sql).map_err(|e| format!("Failed to read ZCode's session store: {e}"))?;
        let rows = stmt
            .query_map([id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(|e| format!("Failed to read ZCode's session store: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| format!("Failed to read ZCode's session store: {e}"))
    };
    let messages = read_rows(
        "SELECT id, data FROM message WHERE session_id = ?1 ORDER BY COALESCE(sequence, 0), time_created, id",
    )?;
    if messages.is_empty() {
        return Err(format!("ZCode has no conversation {id}"));
    }
    let parts = read_rows(
        "SELECT message_id, data FROM part WHERE session_id = ?1 ORDER BY COALESCE(sequence, 0), time_created, id",
    )?;
    let mut by_message: std::collections::HashMap<String, Vec<Value>> = std::collections::HashMap::new();
    for (message, data) in parts {
        if let Ok(value) = serde_json::from_str::<Value>(&data) {
            by_message.entry(message).or_default().push(value);
        }
    }
    let ts = |value: &Value| {
        value
            .pointer("/time/created")
            .and_then(Value::as_i64)
            .and_then(crate::agent::chat::opencode_timeline::iso_from_ms)
    };
    let mut events = Vec::new();
    for (message_id, data) in messages {
        let Ok(info) = serde_json::from_str::<Value>(&data) else { continue };
        let parts = by_message.remove(&message_id).unwrap_or_default();
        let text_of = |part: &Value| {
            let hidden = part.get("synthetic").and_then(Value::as_bool) == Some(true)
                || part.get("ignored").and_then(Value::as_bool) == Some(true);
            (!hidden).then(|| part.get("text").and_then(Value::as_str).unwrap_or("").to_string())
        };
        match info.get("role").and_then(Value::as_str) {
            Some("user") => {
                let visible = match info.pointer("/semantics/uiVisibility").and_then(Value::as_str) {
                    Some(visibility) => visibility == "visible",
                    None => {
                        info.get("synthetic").and_then(Value::as_bool) != Some(true)
                            && info.pointer("/metadata/visibility").and_then(Value::as_str) != Some("model-only")
                    }
                };
                if !visible {
                    continue;
                }
                let text: Vec<String> = parts
                    .iter()
                    .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(text_of)
                    .filter(|text| !text.is_empty())
                    .collect();
                if !text.is_empty() {
                    events.push(Event::User { text: text.join("\n\n"), ts: ts(&info) });
                }
            }
            Some("assistant") => {
                for part in &parts {
                    match part.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            if let Some(text) = text_of(part).filter(|t| !t.is_empty()) {
                                events.push(Event::AssistantText { text, ts: ts(&info) });
                            }
                        }
                        Some("reasoning") => {
                            if let Some(text) = part.get("text").and_then(Value::as_str).filter(|t| !t.is_empty()) {
                                events.push(Event::Thinking { text: text.to_string(), ts: ts(&info) });
                            }
                        }
                        Some("tool") => {
                            let call = part.get("callID").and_then(Value::as_str).map(str::to_string);
                            let name = crate::agent::chat::zcode_protocol::tool_name(
                                part.get("tool").and_then(Value::as_str).unwrap_or("tool"),
                            );
                            let state = part.get("state").cloned().unwrap_or(Value::Null);
                            events.push(Event::ToolUse {
                                id: call.clone(),
                                name: name.clone(),
                                input: state.get("input").cloned().unwrap_or_else(|| serde_json::json!({})),
                                ts: ts(&info),
                            });
                            let status = state.get("status").and_then(Value::as_str).unwrap_or("");
                            if matches!(status, "completed" | "error") {
                                let text = state
                                    .get("output")
                                    .and_then(Value::as_str)
                                    .or_else(|| state.get("error").and_then(Value::as_str))
                                    .unwrap_or("")
                                    .to_string();
                                events.push(Event::ToolResult { id: call, name: Some(name), text, is_error: status == "error" });
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vlx-zcode-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp home");
        dir
    }

    #[test]
    fn desktop_binary_runs_the_bundled_agent_as_node() {
        let home = temp_home("desktop");
        let contents = home.join("ZCode.app").join("Contents");
        let bin = contents.join("MacOS").join("ZCode");
        let resources = contents.join("Resources");
        std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
        std::fs::write(&bin, "").unwrap();
        assert_eq!(desktop_resources(&bin), None, "an app without the agent bundle is not ZCode's desktop release");

        let entry = resources.join("glm").join("zcode.cjs");
        let rg = resources.join("tools").join("ripgrep").join(if cfg!(windows) { "rg.exe" } else { "rg" });
        for file in [&entry, &rg] {
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, "").unwrap();
        }
        assert_eq!(desktop_resources(&bin), Some(resources.clone()));
        let cmd = command(bin.to_str().unwrap());
        assert_eq!(cmd.get_args().collect::<Vec<_>>(), [entry.as_os_str()]);
        let env: std::collections::HashMap<_, _> =
            cmd.get_envs().filter_map(|(k, v)| Some((k.to_string_lossy().into_owned(), v?.to_os_string()))).collect();
        assert_eq!(env["ELECTRON_RUN_AS_NODE"], "1");
        assert_eq!(Path::new(&env["ZCODE_BUILTIN_PROVIDER_CONFIG_FILE"]), resources.join("config/provider/zcode-builtin.json"));
        assert_eq!(Path::new(&env["ZCODE_RG_BINARY"]), rg);
        assert!(!env.contains_key("ZCODE_BFS_BINARY"), "only tools the release ships are passed");

        // A Windows install keeps its resources beside the executable.
        let win_bin = home.join("Programs").join("ZCode").join("ZCode.exe");
        let win_entry = win_bin.parent().unwrap().join("resources").join("glm").join("zcode.cjs");
        std::fs::create_dir_all(win_entry.parent().unwrap()).unwrap();
        std::fs::write(&win_entry, "").unwrap();
        assert_eq!(desktop_resources(&win_bin), Some(win_bin.parent().unwrap().join("resources")));

        // Any other executable is the `zcode` command itself.
        let shim = home.join("bin").join("zcode");
        assert_eq!(command(shim.to_str().unwrap()).get_args().count(), 0);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn history_reads_visible_messages_and_tool_parts_in_order() {
        use crate::agent::export::Event;
        let home = temp_home("history");
        let db = home.join("db.sqlite");
        let conn = rusqlite::Connection::open(&db).unwrap();
        // Rows shaped like ZCode 0.17.0's store: an injected reminder, a real prompt, and a tool turn.
        conn.execute_batch(r#"
            CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT, sequence INTEGER);
            CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT, session_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT, sequence INTEGER);
            INSERT INTO message VALUES ('m0','s',1,1,'{"role":"user","time":{"created":1792981439350},"semantics":{"uiVisibility":"visible"}}',0);
            INSERT INTO message VALUES ('m1','s',2,2,'{"role":"user","metadata":{"visibility":"model-only"}}',1);
            INSERT INTO message VALUES ('m2','s',3,3,'{"role":"assistant","time":{"created":1792981439356}}',2);
            INSERT INTO part VALUES ('p0','m0','s',1,1,'{"type":"text","text":"please use a tool"}',0);
            INSERT INTO part VALUES ('p1','m1','s',2,2,'{"type":"text","text":"Available agent types"}',0);
            INSERT INTO part VALUES ('p2','m2','s',3,3,'{"type":"step-start"}',0);
            INSERT INTO part VALUES ('p3','m2','s',3,3,'{"type":"reasoning","text":"Need a tool."}',1);
            INSERT INTO part VALUES ('p4','m2','s',3,3,'{"type":"tool","callID":"call_1","tool":"Bash","state":{"status":"completed","input":{"command":"touch made.txt"},"output":"(Bash completed with no output)"}}',2);
        "#).unwrap();
        drop(conn);
        let events = history_events_in(&db, "s").unwrap();
        assert_eq!(events.len(), 4);
        assert!(matches!(&events[0], Event::User { text, ts: Some(_) } if text == "please use a tool"));
        assert!(matches!(&events[1], Event::Thinking { text, .. } if text == "Need a tool."));
        assert!(matches!(&events[2], Event::ToolUse { name, input, .. } if name == "Bash" && input["command"] == "touch made.txt"));
        assert!(matches!(&events[3], Event::ToolResult { is_error: false, text, .. } if text == "(Bash completed with no output)"));
        assert!(history_events_in(&db, "other").is_err());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn session_missing_only_when_database_proves_it() {
        let home = temp_home("db");
        let db = home.join("db.sqlite");
        assert!(!session_missing_in(&db, "sess_a"), "no database proves nothing");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE session (id TEXT PRIMARY KEY, time_deleted INTEGER);
             INSERT INTO session VALUES ('sess_live', NULL), ('sess_gone', 42);",
        )
        .unwrap();
        drop(conn);
        assert!(!session_missing_in(&db, "sess_live"));
        assert!(session_missing_in(&db, "sess_gone"));
        assert!(session_missing_in(&db, "sess_unknown"));
        let _ = std::fs::remove_dir_all(&home);
    }
}
