//! CLI adapters. Prompts go through stdin; captured output is bounded and never written to logs.
use super::{digest, heartbeat, schema, Job};
use crate::host::AppCtx;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::time::{Duration, Instant};

pub struct WorkDir(pub PathBuf);
impl WorkDir {
    pub fn new(app: &AppCtx, id: &str) -> Result<Self, String> {
        let path = app.data_dir()?.join("memory-tasks").join(id);
        std::fs::create_dir_all(&path).map_err(|_| "memory_process_failed")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| "memory_process_failed")?;
        }
        let dir = Self(path);
        std::fs::write(dir.0.join("schema.json"), schema().to_string())
            .map_err(|_| "memory_process_failed")?;
        Ok(dir)
    }
}
impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn executable(app: &AppCtx, agent: &str) -> Result<String, String> {
    if !["claude", "codex"].contains(&agent) {
        return Err("memory_invalid:agent".into());
    }
    let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
    let settings = crate::db::repo::get_app_settings(&conn)?;
    let configured = settings
        .get("vlx-settings")
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .and_then(|v| {
            v.get("agentDefaults")?
                .get(agent)?
                .get("path")?
                .as_str()
                .map(str::to_string)
        });
    let configured = configured.filter(|s| !s.trim().is_empty());
    if let Some(path) = configured {
        let p = path.trim();
        let expanded = if let Some(rest) = p.strip_prefix("~/").or_else(|| p.strip_prefix("~\\")) {
            crate::host::home_dir()
                .ok_or("memory_agent_unavailable")?
                .join(rest)
        } else {
            PathBuf::from(p)
        };
        if is_executable(&expanded) {
            return Ok(expanded.to_string_lossy().into_owned());
        }
        return Err("memory_agent_unavailable".into());
    }
    if let Some(path) = crate::agent::install::locate_installed_bin(agent) {
        return Ok(path);
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            for suffix in if cfg!(windows) {
                vec![".exe", ".cmd", ".bat", ""]
            } else {
                vec![""]
            } {
                let p = dir.join(format!("{agent}{suffix}"));
                if is_executable(&p) {
                    return Ok(p.to_string_lossy().into_owned());
                }
            }
        }
    }
    Err("memory_agent_unavailable".into())
}
fn is_executable(p: &Path) -> bool {
    if !p.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return p
            .metadata()
            .is_ok_and(|m| m.permissions().mode() & 0o111 != 0);
    }
    #[cfg(not(unix))]
    {
        true
    }
}

pub fn command(bin: &str, job: &Job, dir: &WorkDir) -> Command {
    let mut cmd = crate::host::command(bin);
    if job.agent == "claude" {
        cmd.args([
            "--print",
            "--output-format",
            "json",
            "--tools",
            "",
            "--strict-mcp-config",
            "--disable-slash-commands",
            "--no-session-persistence",
            "--settings",
            "{\"disableAllHooks\":true}",
            "--json-schema",
        ])
        .arg(schema().to_string());
        if !job.model.is_empty() {
            cmd.args(["--model", &job.model]);
        }
        cmd.env_remove("CLAUDECODE");
    } else {
        cmd.args([
            "exec",
            "--skip-git-repo-check",
            "--ephemeral",
            "--sandbox",
            "read-only",
            "--color",
            "never",
            "--json",
            "--disable",
            "shell_tool",
            "--disable",
            "apps",
            "--disable",
            "multi_agent",
            "-c",
            "project_doc_max_bytes=0",
            "-c",
            "approval_policy=\"never\"",
            "-c",
            "web_search=\"disabled\"",
            "--output-schema",
        ])
        .arg(dir.0.join("schema.json"));
        if !job.model.is_empty() {
            cmd.args(["--model", &job.model]);
        }
        let config_home = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .or_else(|| crate::host::home_dir().map(|home| home.join(".codex")));
        if let Some(config) =
            config_home.and_then(|home| std::fs::read_to_string(home.join("config.toml")).ok())
        {
            if let Ok(doc) = config.parse::<toml_edit::DocumentMut>() {
                for section in ["mcp_servers", "plugins"] {
                    if let Some(table) = doc.get(section).and_then(toml_edit::Item::as_table_like) {
                        for (name, _) in table.iter() {
                            if let Ok(quoted) = serde_json::to_string(name) {
                                cmd.arg("-c")
                                    .arg(format!("{section}.{quoted}.enabled=false"));
                            }
                        }
                    }
                }
            }
        }
        cmd.arg("-");
    }
    // Do not let inherited session hooks associate a compiler invocation with its source session.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("VLX_") || key == "CODEX_THREAD_ID" {
            cmd.env_remove(key);
        }
    }
    cmd.current_dir(&dir.0)
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd
}

fn kill(child: &mut Child) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    #[cfg(windows)]
    {
        let _ = crate::host::command("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn capture<R: Read + Send + 'static>(
    mut stream: R,
    overflow: Arc<AtomicBool>,
) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut out = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            match stream.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if out.len() + n > 8 * 1024 * 1024 {
                        overflow.store(true, Ordering::Relaxed);
                        break;
                    }
                    out.extend_from_slice(&buf[..n]);
                }
            }
        }
        let _ = tx.send(out);
    });
    rx
}

pub fn decode(agent: &str, text: &str) -> Result<(Value, Value), String> {
    if agent == "claude" {
        let output: Value = serde_json::from_str(text).map_err(|_| "memory_invalid_output")?;
        if output.get("is_error").and_then(Value::as_bool) == Some(true) {
            return Err("memory_process_failed".into());
        }
        let result = if let Some(v) = output.get("structured_output") {
            v.clone()
        } else {
            serde_json::from_str(
                output
                    .get("result")
                    .and_then(Value::as_str)
                    .ok_or("memory_invalid_output")?,
            )
            .map_err(|_| "memory_invalid_output")?
        };
        Ok((result, output.get("usage").cloned().unwrap_or(Value::Null)))
    } else {
        let mut result = None;
        let mut usage = Value::Null;
        let mut completed = false;
        for line in text.lines().filter(|s| !s.trim().is_empty()) {
            let event: Value = serde_json::from_str(line).map_err(|_| "memory_invalid_output")?;
            match event.get("type").and_then(Value::as_str) {
                Some("turn.failed") | Some("error") => return Err("memory_process_failed".into()),
                Some("turn.completed") => {
                    completed = true;
                    usage = event.get("usage").cloned().unwrap_or(Value::Null);
                }
                Some("item.completed") if event["item"]["type"] == "agent_message" => {
                    result = Some(
                        serde_json::from_str(
                            event["item"]["text"]
                                .as_str()
                                .ok_or("memory_invalid_output")?,
                        )
                        .map_err(|_| "memory_invalid_output")?,
                    );
                }
                _ => {}
            }
        }
        if !completed {
            return Err("memory_invalid_output".into());
        }
        Ok((result.ok_or("memory_invalid_output")?, usage))
    }
}

pub fn call(
    app: &AppCtx,
    job: &Job,
    dir: &WorkDir,
    prompt: &str,
    step: &str,
) -> Result<Value, String> {
    heartbeat(app, &job.id)?;
    // Refuse oversized contexts explicitly; never silently truncate source or existing knowledge.
    if prompt.chars().count() > 350_000 {
        return Err("memory_context_too_large".into());
    }
    let start = Instant::now();
    audit(
        app,
        &job.id,
        "INFO",
        "ai_request",
        &json!({"step":step,"method":"AI","model":if job.model.is_empty(){"configured_default"}else{&job.model},"interface":format!("{} CLI",job.agent),"goal":"compile_thematic_memory","inputType":"text","originalChars":prompt.chars().count(),"sentChars":prompt.chars().count(),"limit":350000,"truncated":false,"imageCount":0,"schema":"memory-wiki-v1","preview":"[source and memory content redacted]","sha256":digest(prompt),"inputCount":1,"outputCount":0,"status":"started","durationMs":0}),
    );
    let bin = executable(app, &job.agent)?;
    let mut child = command(&bin, job, dir)
        .spawn()
        .map_err(|_| "memory_process_failed")?;
    let overflow = Arc::new(AtomicBool::new(false));
    let stdout = capture(
        child.stdout.take().ok_or("memory_process_failed")?,
        overflow.clone(),
    );
    let stderr = capture(
        child.stderr.take().ok_or("memory_process_failed")?,
        overflow.clone(),
    );
    let mut input = child.stdin.take().ok_or("memory_process_failed")?;
    let bytes = prompt.as_bytes().to_vec();
    let (send, write_done) = mpsc::channel();
    std::thread::spawn(move || {
        let result = input.write_all(&bytes);
        drop(input);
        let _ = send.send(result.is_ok());
    });
    let mut last_heartbeat = Instant::now();
    let status = loop {
        if overflow.load(Ordering::Relaxed) {
            kill(&mut child);
            return Err("memory_output_too_large".into());
        }
        if start.elapsed() > Duration::from_secs(600) {
            kill(&mut child);
            return Err("memory_timeout".into());
        }
        if last_heartbeat.elapsed() > Duration::from_millis(500) {
            if let Err(e) = heartbeat(app, &job.id) {
                kill(&mut child);
                return Err(e);
            }
            last_heartbeat = Instant::now();
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => {
                kill(&mut child);
                return Err("memory_process_failed".into());
            }
        }
    };
    let out = stdout.recv_timeout(Duration::from_secs(3)).map_err(|_| {
        kill(&mut child);
        "memory_process_failed"
    })?;
    let err = stderr.recv_timeout(Duration::from_secs(3)).map_err(|_| {
        kill(&mut child);
        "memory_process_failed"
    })?;
    audit(
        app,
        &job.id,
        "INFO",
        "ai_received",
        &json!({"step":step,"method":"AI","responseBytes":out.len(),"sha256":format!("{:x}",sha2::Sha256::digest(&out)),"exitCode":status.code(),"status":"received","durationMs":start.elapsed().as_millis()}),
    );
    if !status.success() || write_done.recv_timeout(Duration::from_secs(1)) != Ok(true) {
        audit(
            app,
            &job.id,
            "ERROR",
            "ai_failed",
            &json!({"step":step,"method":"AI","exitCode":status.code(),"stderrBytes":err.len(),"stderrSha256":format!("{:x}",sha2::Sha256::digest(&err)),"status":"failed","durationMs":start.elapsed().as_millis()}),
        );
        return Err("memory_process_failed".into());
    }
    let text = String::from_utf8(out).map_err(|_| "memory_invalid_output")?;
    let (result, reported_usage) = decode(&job.agent, &text)?;
    let mut usage = serde_json::Map::new();
    for field in [
        "input_tokens",
        "output_tokens",
        "cached_input_tokens",
        "cache_creation_input_tokens",
        "cache_read_input_tokens",
    ] {
        if let Some(value) = reported_usage.get(field).and_then(Value::as_u64) {
            usage.insert(field.into(), json!(value));
        }
    }
    audit(
        app,
        &job.id,
        "INFO",
        "ai_response",
        &json!({"step":step,"method":"AI","responseBytes":text.len(),"sha256":digest(&text),"usage":usage,"inputCount":1,"outputCount":result["entries"].as_array().map_or(0,Vec::len),"entityType":"memory_entry","status":"completed","durationMs":start.elapsed().as_millis()}),
    );
    heartbeat(app, &job.id)?;
    Ok(result)
}

use sha2::Digest;
pub fn audit(app: &AppCtx, id: &str, level: &str, event: &str, data: &Value) {
    if std::env::var("VLX_MEMORY_LOG_LEVEL").is_ok_and(|v| {
        v.eq_ignore_ascii_case("off") || (v.eq_ignore_ascii_case("error") && level != "ERROR")
    }) {
        return;
    }
    let utc = time::OffsetDateTime::now_utc();
    let date = time::UtcOffset::current_local_offset().map_or(utc, |o| utc.to_offset(o));
    let line = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} [{:<5}] [system] event={} jobId={} {}\n",
        date.year(),
        date.month() as u8,
        date.day(),
        date.hour(),
        date.minute(),
        date.second(),
        level,
        event,
        id,
        data
    );
    eprint!("{line}");
    let dir = std::env::var_os("VLX_MEMORY_LOG_DIR")
        .map(PathBuf::from)
        .or_else(|| app.data_dir().ok().map(|p| p.join("logs")));
    if let Some(dir) = dir {
        if std::fs::create_dir_all(&dir).is_ok() {
            let mut options = std::fs::OpenOptions::new();
            options.create(true).append(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            if let Ok(mut file) = options.open(dir.join("memory.log")) {
                let _ = file.write_all(line.as_bytes());
            }
        }
    }
}
