//! CLI adapters. Captured output is bounded and never written to logs.
use super::{digest, heartbeat, schema, Job};
use crate::agent::headless::ARG_PROMPT_LIMIT;
use crate::host::AppCtx;
use crate::models::SessionKind;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::time::{Duration, Instant};

/// How the prompt reaches the CLI.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Delivery {
    /// Written to stdin, which is then closed. Takes a prompt of any size.
    Stdin,
    /// Passed as an argument, so the whole command line has to stay under the operating system's cap.
    Arg,
    /// Written into the job's work directory and named by a flag. Also unbounded.
    File,
}

/// How the one JSON answer is recovered from stdout.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    /// A single JSON object whose `structured_output` or `result` holds the answer.
    ClaudeJson,
    /// Codex event stream: the answer is the `agent_message` item.
    CodexEvents,
    /// Grok prints the constrained JSON on its own.
    Plain,
    /// OpenCode event stream: concatenate the `text` events' parts.
    OpencodeEvents,
    /// Pi and OMP event stream: the `turn_end` message carries the final content parts.
    PiEvents,
}

/// How one agent CLI runs an organizer step.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub kind: SessionKind,
    pub delivery: Delivery,
    pub shape: Shape,
    /// The CLI pins the model to `schema()` itself, so the prompt does not need to restate it and the
    /// reply parses directly. Without it the schema travels in the prompt and `decode` has to recover
    /// the JSON from ordinary reply text, which occasionally fails and is retried by the caller.
    pub constrains_schema: bool,
}

/// Agents the organizer can run, or None for a kind it has no verified invocation for.
///
/// Flags come from each CLI's own `--help`. Every entry disables tools, session persistence and
/// plugin discovery: organizing is a bounded text transformation that must not touch the filesystem
/// or the network, and a process that stops to ask for approval would just hang until it is killed.
pub fn spec(agent: &str) -> Option<Spec> {
    let kind = SessionKind::from_db(agent);
    let (delivery, shape, constrains_schema) = match kind {
        SessionKind::Claude => (Delivery::Stdin, Shape::ClaudeJson, true),
        SessionKind::Codex => (Delivery::Stdin, Shape::CodexEvents, true),
        SessionKind::Grok => (Delivery::File, Shape::Plain, true),
        SessionKind::Opencode => (Delivery::Arg, Shape::OpencodeEvents, false),
        SessionKind::Pi | SessionKind::Omp => (Delivery::Arg, Shape::PiEvents, false),
        _ => return None,
    };
    Some(Spec {
        kind,
        delivery,
        shape,
        constrains_schema,
    })
}

/// Agent ids offered by the organizer, in the order the dialog lists them.
pub const AGENTS: &[&str] = &["claude", "codex", "grok", "opencode", "pi", "omp"];

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

/// Locate the CLI for one organizer agent.
///
/// Resolution is the shared one used by launches and model listings, so an agent configured in
/// Settings resolves the same way everywhere. A configured path that no longer points at a program
/// still counts as unavailable here: the dialog needs that answer before it offers the agent, not a
/// spawn failure several minutes into a job.
pub fn executable(app: &AppCtx, agent: &str) -> Result<String, String> {
    let Some(spec) = spec(agent) else {
        return Err("memory_invalid:agent".into());
    };
    crate::agent::executable::resolve(app, spec.kind, None)
        .filter(|path| is_executable(Path::new(path)))
        .or_else(|| {
            crate::agent::executable::find_on_path(crate::agent::executable::command_name(spec.kind))
        })
        .ok_or_else(|| "memory_agent_unavailable".to_string())
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

/// Build the process for one organizer step, placing the prompt where this CLI expects it.
///
/// Returns the prompt again when it has to be written to stdin, so the caller does not have to know
/// which agents read stdin and which were already given their prompt here.
pub fn command<'a>(
    bin: &str,
    job: &Job,
    dir: &WorkDir,
    prompt: &'a str,
) -> Result<(Command, Option<&'a str>), String> {
    let mut spec = spec(&job.agent).ok_or("memory_invalid:agent")?;
    if spec.kind == SessionKind::Opencode && crate::agent::opencode::is_v2(bin) {
        spec.delivery = Delivery::Stdin;
    }
    if spec.delivery == Delivery::Arg && prompt.len() > ARG_PROMPT_LIMIT {
        return Err("memory_context_too_large".into());
    }
    let mut cmd = build(bin, job, dir, spec, prompt)?;
    crate::agent::executable::prepare_command(&mut cmd, bin);
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
    Ok((
        cmd,
        (spec.delivery == Delivery::Stdin).then_some(prompt),
    ))
}

fn build(
    bin: &str,
    job: &Job,
    dir: &WorkDir,
    spec: Spec,
    prompt: &str,
) -> Result<Command, String> {
    let mut cmd = crate::host::command(bin);
    if spec.kind == SessionKind::Claude {
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
        if !job.effort.is_empty() { cmd.args(["--effort", &job.effort]); }
        cmd.env_remove("CLAUDECODE");
    } else if spec.kind == SessionKind::Grok {
        // grok 0.2.118: `--json-schema` constrains the reply and implies JSON output. `--prompt-file`
        // keeps a full segment off the command line, which `-p` could not hold.
        let path = dir.0.join("prompt.txt");
        std::fs::write(&path, prompt).map_err(|_| "memory_process_failed")?;
        cmd.args([
            "--permission-mode",
            "plan",
            "--tools",
            "",
            "--no-memory",
            "--no-plan",
            "--no-subagents",
            "--no-web",
            "--output-format",
            "json",
            "--json-schema",
        ])
        .arg(schema().to_string())
        .arg("--prompt-file")
        .arg(path);
        if !job.model.is_empty() {
            cmd.args(["--model", &job.model]);
        }
        if !job.effort.is_empty() {
            cmd.args(["--reasoning-effort", &job.effort]);
        }
    } else if spec.kind == SessionKind::Opencode {
        // OpenCode v1 isolates with `--pure` and passes the prompt as an argument; v2 uses
        // `--standalone`, folds the variant into `--model <model>#<effort>`, and reads stdin.
        let v2 = spec.delivery == Delivery::Stdin;
        cmd.args(crate::agent::opencode::run_args(v2));
        cmd.args(crate::agent::opencode::run_selection_args(
            v2,
            Some(job.model.as_str()),
            Some(job.effort.as_str()),
        ));
        if !v2 {
            cmd.arg(prompt);
        }
    } else if matches!(spec.kind, SessionKind::Pi | SessionKind::Omp) {
        // Pi v0.24 and OMP v18: `-p` is print mode and `--mode json` gives the event stream `decode`
        // reads. `--no-tools` leaves the model nothing to call, so the run cannot reach the filesystem
        // or the network, and `--no-session` keeps it out of the history. OMP spells its long options
        // with `=` and has two switches Pi does not.
        let pi = spec.kind == SessionKind::Pi;
        cmd.arg("-p");
        let option = |name: &str, value: &str| {
            if pi {
                vec![format!("--{name}"), value.to_string()]
            } else {
                vec![format!("--{name}={value}")]
            }
        };
        cmd.args(option("mode", "json"));
        cmd.args([
            "--no-session",
            "--no-tools",
            "--no-skills",
            "--no-extensions",
        ]);
        if pi {
            cmd.arg("--no-context-files");
        } else {
            cmd.args(["--no-rules", "--no-title"]);
        }
        if !job.model.is_empty() {
            cmd.args(option("model", &job.model));
        }
        if !job.effort.is_empty() {
            cmd.args(option("thinking", &job.effort));
        }
        cmd.arg(prompt);
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
        if !job.effort.is_empty() {
            cmd.arg("-c").arg(format!("model_reasoning_effort={}", serde_json::to_string(&job.effort).unwrap()));
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
                            // Codex parses override paths directly; quotes become part of the key.
                            cmd.arg("-c")
                                .arg(format!("{section}.{name}.enabled=false"));
                        }
                    }
                }
            }
        }
        cmd.arg("-");
    }
    Ok(cmd)
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

/// Recover the answer object from reply text that was not constrained to `schema()`.
///
/// An agent without a schema flag writes the JSON as ordinary prose, so it may arrive wrapped in a
/// fenced code block or trailed by a sentence. Parsing the whole string first keeps a clean reply
/// exact; the brace scan is only the fallback and is deliberately shallow, because anything it cannot
/// read is reported as invalid output and the step is retried rather than guessed at.
pub fn recover_json(text: &str) -> Result<Value, String> {
    let trimmed = text.trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Ok(value);
    }
    // Strip one fenced block, with or without a language tag on the opening fence.
    let unfenced = trimmed
        .strip_prefix("```")
        .and_then(|rest| rest.split_once('\n'))
        .map(|(_, body)| body)
        .and_then(|body| body.rsplit_once("```"))
        .map(|(body, _)| body.trim());
    if let Some(value) = unfenced.and_then(|body| serde_json::from_str::<Value>(body).ok()) {
        return Ok(value);
    }
    let body = unfenced.unwrap_or(trimmed);
    let start = body.find('{').ok_or("memory_invalid_output")?;
    let end = body.rfind('}').ok_or("memory_invalid_output")?;
    if end <= start {
        return Err("memory_invalid_output".into());
    }
    serde_json::from_str(&body[start..=end]).map_err(|_| "memory_invalid_output".into())
}

/// Collect the final assistant text from an event stream, ignoring progress and thinking events.
fn stream_text(shape: Shape, text: &str) -> Result<(String, Value), String> {
    let mut answer = String::new();
    let mut usage = Value::Null;
    let mut completed = false;
    for line in text.lines().filter(|s| !s.trim().is_empty()) {
        // A CLI may interleave a log line with its events; skip what does not parse rather than
        // failing the whole step over it.
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match (shape, event.get("type").and_then(Value::as_str)) {
            (Shape::OpencodeEvents, Some("text")) => {
                if let Some(part) = event["part"]["text"].as_str() {
                    answer.push_str(part);
                }
                completed = true;
            }
            (Shape::OpencodeEvents, Some("step_finish")) => {
                usage = event["part"]["tokens"].clone();
            }
            (Shape::PiEvents, Some("turn_end")) => {
                // Pi and OMP report thinking as its own content part; only text is the answer.
                for part in event["message"]["content"].as_array().into_iter().flatten() {
                    if part["type"] == "text" {
                        answer.push_str(part["text"].as_str().unwrap_or_default());
                    }
                }
                usage = event["message"]["usage"].clone();
                completed = true;
            }
            _ => {}
        }
    }
    if !completed || answer.trim().is_empty() {
        return Err("memory_invalid_output".into());
    }
    Ok((answer, usage))
}

pub fn decode(agent: &str, text: &str) -> Result<(Value, Value), String> {
    let shape = spec(agent).ok_or("memory_invalid:agent")?.shape;
    match shape {
        Shape::Plain => return Ok((recover_json(text)?, Value::Null)),
        Shape::OpencodeEvents | Shape::PiEvents => {
            let (answer, usage) = stream_text(shape, text)?;
            return Ok((recover_json(&answer)?, usage));
        }
        Shape::ClaudeJson | Shape::CodexEvents => {}
    }
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
        &json!({"step":step,"method":"AI","model":if job.model.is_empty(){"configured_default"}else{&job.model},"effort":if job.effort.is_empty(){"configured_default"}else{&job.effort},"interface":format!("{} CLI",job.agent),"goal":"compile_thematic_memory","inputType":"text","originalChars":prompt.chars().count(),"sentChars":prompt.chars().count(),"limit":350000,"truncated":false,"imageCount":0,"schema":"memory-wiki-v1","preview":"[source and memory content redacted]","sha256":digest(prompt),"inputCount":1,"outputCount":0,"status":"started","durationMs":0}),
    );
    let bin = executable(app, &job.agent)?;
    let (mut spawner, stdin_prompt) = command(&bin, job, dir, prompt)?;
    let mut child = spawner.spawn().map_err(|_| "memory_process_failed")?;
    let overflow = Arc::new(AtomicBool::new(false));
    let stdout = capture(
        child.stdout.take().ok_or("memory_process_failed")?,
        overflow.clone(),
    );
    let stderr = capture(
        child.stderr.take().ok_or("memory_process_failed")?,
        overflow.clone(),
    );
    // Agents given their prompt on the command line still get stdin closed immediately, so one that
    // decides to read from it sees end of input instead of waiting for a line nobody will type.
    let mut input = child.stdin.take().ok_or("memory_process_failed")?;
    let bytes = stdin_prompt.unwrap_or_default().as_bytes().to_vec();
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
    let _ = app;
    let configured=std::env::var("VLX_MEMORY_LOG_LEVEL").unwrap_or_else(|_|"info".into());
    if !crate::diagnostics::enabled(&configured,level) {return;}
    let mut fields=data.clone(); fields["jobId"]=json!(id); fields["step"]=json!(event);
    crate::diagnostics::record(level,"memory",fields);
}
