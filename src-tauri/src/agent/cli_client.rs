//! Cross-platform Rust implementation of the hidden `vlx-term` subcommands `--spawn`, `--view`,
//! `--refer`, and `--search`.
//!
//! Thin PATH shims (`vspawn`, `vspawn-tree`, `vopen`, `vrefer`, and `vsearch`; see spawn_cli.rs) invoke
//! these commands through sh on Unix or .cmd on Windows. They read injected `VLX_SPAWN_URL`,
//! `VLX_SESSION_ID`, and `VLX_TOKEN`, build JSON, and POST to the local hook service.
//!
//! `--spawn` and `--view` only need to know whether the request succeeded. `--refer` and `--search` are
//! reads: they consume the response body and render it, either as text for a model or as raw JSON.
//!
//! Removing the Bash dependency enables Windows and consolidates all platforms in one implementation.

use std::io::{Read, Write};
use std::net::TcpStream;

/// Session environment variables injected at launch; see pty/manager.rs.
const ENV_URL: &str = "VLX_SPAWN_URL";
const ENV_SID: &str = "VLX_SESSION_ID";
const ENV_TOKEN: &str = "VLX_TOKEN";

/// `vlx-term --spawn [...]` entry point: parse arguments, POST `/spawn`, and return an exit code.
/// `args` is the full argv; the vspawn-tree shim also passes `--worktree`.
pub fn run_spawn(args: &[String]) -> ! {
    let rest = &args[args.len().min(2)..];
    let parsed = match parse_spawn_args(rest) {
        SpawnParse::Help => {
            println!("{SPAWN_USAGE}");
            std::process::exit(0);
        }
        SpawnParse::Err(msg) => {
            eprintln!("{msg}");
            eprintln!("{SPAWN_USAGE}");
            std::process::exit(2);
        }
        SpawnParse::Ok(p) => p,
    };

    let (url, sid, token) = match session_env() {
        Ok(v) => v,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(1);
        }
    };

    // Capture the command's real cwd rather than relying on the parent session's persisted directory. A
    // collection has no project root, and an interactive shell or agent may have moved since it launched.
    let cwd = match parsed.cwd.as_deref() {
        Some(path) if std::path::Path::new(path).is_absolute() => path.to_string(),
        Some(path) => std::env::current_dir()
            .map(|base| base.join(path).to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string()),
        None => std::env::current_dir()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default(),
    };
    let body = build_spawn_body(&sid, &parsed, &cwd);
    let endpoint = format!("{url}/spawn?t={token}");
    match post_json(&endpoint, &body) {
        Some(code) if (200..300).contains(&code) => {
            let wt = if parsed.worktree {
                "isolated worktree"
            } else {
                "current dir"
            };
            let kind = parsed.kind.as_deref().unwrap_or("inherit current");
            // Echo the model and effort when given, so a typo in either is visible right away.
            let mut about = format!("{kind}, {wt}");
            if let Some(m) = parsed.model.as_deref() {
                about.push_str(&format!(", model {m}"));
            }
            if let Some(e) = parsed.effort.as_deref() {
                about.push_str(&format!(", effort {e}"));
            }
            println!("spawned sub-session ({about}): {}", parsed.prompt);
            std::process::exit(0);
        }
        Some(code) => {
            eprintln!("vspawn: spawn failed (service returned {code})");
            std::process::exit(1);
        }
        None => {
            eprintln!("vspawn: cannot connect to VelaTerm ({url})");
            std::process::exit(1);
        }
    }
}

/// `vlx-term --view <file|URL>...` entry point: POST each item to `/view`; any failure returns code 1.
pub fn run_view(args: &[String]) -> ! {
    let rest = &args[args.len().min(2)..];
    if let Some(first) = rest.first() {
        if first == "-h" || first == "--help" {
            println!("{VIEW_USAGE}");
            std::process::exit(0);
        }
    }
    if rest.is_empty() {
        eprintln!("{VIEW_USAGE}");
        std::process::exit(2);
    }

    let (url, sid, token) = match session_env() {
        Ok(v) => v,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(1);
        }
    };
    // The server resolves relative paths against cwd, replacing the script's `$PWD` handling.
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let endpoint = format!("{url}/view?t={token}");

    let mut exit = 0;
    for f in rest {
        let body = build_view_body(&sid, f, &cwd);
        match post_json(&endpoint, &body) {
            Some(code) if (200..300).contains(&code) => {
                println!("opened in VelaTerm: {f}");
            }
            _ => {
                // `/view` validation returns 404 for missing/nonregular files; connection failure returns None.
                eprintln!("vopen: open failed (file missing or not a regular file): {f}");
                exit = 1;
            }
        }
    }
    std::process::exit(exit);
}

const SPAWN_USAGE: &str =
    "usage: vspawn [--worktree] [--cwd <path>] [--yes] [--claude|--codex|--copilot|--kiro] [--model <name>] [--effort <level>] <task description...>\n\
    --cwd <path>      child working directory and repository used to create a worktree\n\
    --yes             skip the confirmation dialog and start the child session with default settings\n\
    --model <name>    model for the child session, such as opus or gpt-5.5; names are agent specific\n\
    --effort <level>  reasoning effort for agents that offer one, such as low, medium, or high";
const VIEW_USAGE: &str = "usage: vopen <file|url>...   (relative paths resolve against the current dir; opens multiple at once)\n\
    opens by type: markdown editor / image viewer / code editor (syntax highlight)\n\
    http/https urls open in an in-app browser tab (desktop only)";

/// Read the three session environment variables and return a user-facing error if any is missing.
fn session_env() -> Result<(String, String, String), String> {
    Ok((
        require_env(ENV_URL)?,
        require_env(ENV_SID)?,
        require_env(ENV_TOKEN)?,
    ))
}

fn require_env(name: &str) -> Result<String, String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => Err(format!("not inside a VelaTerm session (missing {name})")),
    }
}

/// Spawn argument parsing result.
struct SpawnArgs {
    worktree: bool,
    /// Explicit child/repository directory; absent means the command's current directory.
    cwd: Option<String>,
    kind: Option<String>,
    /// Skip the confirmation dialog and start the child session with default settings.
    no_confirm: bool,
    /// Model for the child session; the frontend turns it into the agent's own model flag.
    model: Option<String>,
    /// Reasoning effort for the child session; ignored for agents whose CLI has no effort flag.
    effort: Option<String>,
    prompt: String,
}

enum SpawnParse {
    Ok(SpawnArgs),
    Help,
    Err(String),
}

/// Parse arguments after `--spawn`. `--worktree/--wt` enables a worktree, `--no-worktree/--nowt`
/// disables it, `--yes/-y/--no-confirm` skips the confirmation dialog, agent flags select kind,
/// `--cwd`/`--model`/`--effort` take a value in either `--cwd x` or `--cwd=x` form, help flags show usage,
/// and everything after `--` is prompt text even when prefixed by `-`. Other options are errors;
/// remaining words join into a required prompt.
fn parse_spawn_args(rest: &[String]) -> SpawnParse {
    let mut worktree = false;
    let mut cwd: Option<String> = None;
    let mut kind: Option<String> = None;
    let mut no_confirm = false;
    let mut model: Option<String> = None;
    let mut effort: Option<String> = None;
    let mut words: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        let a = rest[i].as_str();
        match a {
            "--worktree" | "--wt" => worktree = true,
            "--no-worktree" | "--nowt" => worktree = false,
            "--yes" | "-y" | "--no-confirm" => no_confirm = true,
            "--claude" => kind = Some("claude".to_string()),
            "--codex" => kind = Some("codex".to_string()),
            "--copilot" => kind = Some("copilot".to_string()),
            "--kiro" => kind = Some("kiro".to_string()),
            "-h" | "--help" => return SpawnParse::Help,
            "--" => {
                for w in &rest[i + 1..] {
                    words.push(w.as_str());
                }
                break;
            }
            // Value options. Both spellings are accepted because agents write one and people the other.
            "--cwd" | "--model" | "--effort" => {
                let Some(v) = rest
                    .get(i + 1)
                    .map(|s| s.as_str())
                    .filter(|v| !v.is_empty())
                else {
                    return SpawnParse::Err(format!("vspawn: {a} needs a value"));
                };
                match a {
                    "--cwd" => cwd = Some(v.to_string()),
                    "--model" => model = Some(v.to_string()),
                    _ => effort = Some(v.to_string()),
                }
                i += 1;
            }
            _ if a.starts_with("--cwd=")
                || a.starts_with("--model=")
                || a.starts_with("--effort=") =>
            {
                // The prefix test guarantees a separator, so the split cannot fail.
                let (flag, v) = a.split_once('=').unwrap_or((a, ""));
                if v.is_empty() {
                    return SpawnParse::Err(format!("vspawn: {flag} needs a value"));
                }
                match flag {
                    "--cwd" => cwd = Some(v.to_string()),
                    "--model" => model = Some(v.to_string()),
                    _ => effort = Some(v.to_string()),
                }
            }
            _ if a.starts_with('-') => {
                return SpawnParse::Err(format!("vspawn: unknown option {a}"));
            }
            _ => words.push(a),
        }
        i += 1;
    }
    let prompt = words.join(" ");
    if prompt.trim().is_empty() {
        return SpawnParse::Err("vspawn: missing task description".to_string());
    }
    SpawnParse::Ok(SpawnArgs {
        worktree,
        cwd,
        kind,
        no_confirm,
        model,
        effort,
        prompt,
    })
}

/// Build the `/spawn` JSON body, omitting kind so the frontend can inherit it when absent. serde_json
/// safely escapes quotes, newlines, and backslashes. The supplied cwd is the resolved invocation
/// directory. `noConfirm`, `model`, and `effort` are only
/// written when set, keeping the body identical to previous builds for ordinary spawns.
fn build_spawn_body(sid: &str, args: &SpawnArgs, cwd: &str) -> String {
    let mut obj = serde_json::Map::new();
    obj.insert("parentSessionId".into(), serde_json::json!(sid));
    obj.insert("prompt".into(), serde_json::json!(args.prompt));
    obj.insert("worktree".into(), serde_json::json!(args.worktree));
    if !cwd.is_empty() {
        obj.insert("cwd".into(), serde_json::json!(cwd));
    }
    if args.no_confirm {
        obj.insert("noConfirm".into(), serde_json::json!(true));
    }
    if let Some(k) = args.kind.as_deref() {
        obj.insert("kind".into(), serde_json::json!(k));
    }
    if let Some(m) = args.model.as_deref() {
        obj.insert("model".into(), serde_json::json!(m));
    }
    if let Some(e) = args.effort.as_deref() {
        obj.insert("effort".into(), serde_json::json!(e));
    }
    serde_json::Value::Object(obj).to_string()
}

/// Build the `/view` JSON body.
fn build_view_body(sid: &str, path: &str, cwd: &str) -> String {
    serde_json::json!({ "sessionId": sid, "path": path, "cwd": cwd }).to_string()
}

/// Send an HTTP/1.1 POST to the local hook service and return its status code, or None on connection
/// or status-line failure. Reuse server::split_http_url and a raw TcpStream without an HTTP client.
fn post_json(url: &str, body: &str) -> Option<u16> {
    let (host, port, path) = crate::agent::server::split_http_url(url)?;
    let mut stream = TcpStream::connect((host.as_str(), port)).ok()?;
    let req = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: {host}:{port}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {len}\r\n\
         Connection: close\r\n\r\n\
         {body}",
        len = body.len()
    );
    stream.write_all(req.as_bytes()).ok()?;
    stream.flush().ok()?;
    let mut resp = Vec::new();
    let _ = stream.read_to_end(&mut resp);
    // Status lines have the form `HTTP/1.1 200 OK`.
    String::from_utf8_lossy(&resp)
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
}

const REFER_USAGE: &str = "usage: vrefer <session> [--last N] [--range A:B] [--json]\n\
           vrefer --list [--json]\n\
    reads another vlx-term session's conversation, the whole thing by default. <session> is a full\n\
    session id, an id prefix of at least 8 characters, or a session name (exact, else a unique substring).\n\
    --last N     only the last N messages\n\
    --range A:B  only this message index range, zero-based, A included and B excluded\n\
    --ask TEXT   have an agent read the transcript and answer this instead of printing it\n\
    --with KIND  which agent answers (claude|codex|cursor|copilot|grok); default picks one\n\
    --timeout N  seconds the answering agent may take (default 120)\n\
    --list       list the sessions available to read\n\
    --json       machine-readable output";

const SEARCH_USAGE: &str = "usage: vsearch <words...> [--all|--archived] [--limit N] [--json]\n\
    full-text search across every vlx-term session. Multiple words are an implicit AND and word order\n\
    does not matter.\n\
    --all        include archived sessions (default: live sessions only)\n\
    --archived   search archived sessions only\n\
    --limit N    maximum number of matching sessions (default 10)\n\
    --json       machine-readable output";

/// `vlx-term --refer [...]` entry point used by the `vrefer` shim: read another session's transcript.
pub fn run_refer(args: &[String]) -> ! {
    let rest = &args[args.len().min(2)..];
    let parsed = match parse_refer_args(rest) {
        ReferParse::Help => {
            println!("{REFER_USAGE}");
            std::process::exit(0);
        }
        ReferParse::Err(msg) => {
            eprintln!("{msg}");
            eprintln!("{REFER_USAGE}");
            std::process::exit(2);
        }
        ReferParse::Ok(p) => p,
    };

    let (url, sid, token) = match session_env() {
        Ok(v) => v,
        Err(msg) => {
            eprintln!("vrefer: {msg}");
            std::process::exit(1);
        }
    };
    let body = build_refer_body(&sid, &parsed);
    let endpoint = format!("{url}/refer?t={token}");
    let (code, payload) = match post_json_read(&endpoint, &body) {
        Some(v) => v,
        None => {
            eprintln!("vrefer: cannot read a complete response from VelaTerm ({url})");
            std::process::exit(1);
        }
    };
    let value = read_value("vrefer", code, &payload);
    if code == 200 {
        if parsed.json {
            println!("{payload}");
        } else if parsed.list {
            println!("{}", render_refer_list(&value));
        } else if value.get("answer").is_some() {
            println!("{}", render_answer(&value));
        } else {
            // Asking may have failed; the transcript still came back, so say what happened on stderr
            // and print it. Silently handing back a transcript when an answer was requested would look
            // like the question was ignored.
            if let Some(reason) = value.get("askFailed").and_then(|v| v.as_str()) {
                eprintln!("vrefer: could not answer ({reason}); printing the transcript instead");
            }
            println!("{}", render_refer(&value));
        }
        std::process::exit(0);
    }
    report_read_failure("vrefer", code, &payload, &value)
}

/// `vlx-term --search [...]` entry point used by the `vsearch` shim: search across every session.
pub fn run_search(args: &[String]) -> ! {
    let rest = &args[args.len().min(2)..];
    let parsed = match parse_search_args(rest) {
        SearchParse::Help => {
            println!("{SEARCH_USAGE}");
            std::process::exit(0);
        }
        SearchParse::Err(msg) => {
            eprintln!("{msg}");
            eprintln!("{SEARCH_USAGE}");
            std::process::exit(2);
        }
        SearchParse::Ok(p) => p,
    };

    let (url, sid, token) = match session_env() {
        Ok(v) => v,
        Err(msg) => {
            eprintln!("vsearch: {msg}");
            std::process::exit(1);
        }
    };
    let body = build_search_body(&sid, &parsed);
    let endpoint = format!("{url}/search?t={token}");
    let (code, payload) = match post_json_read(&endpoint, &body) {
        Some(v) => v,
        None => {
            eprintln!("vsearch: cannot read a complete response from VelaTerm ({url})");
            std::process::exit(1);
        }
    };
    let value = read_value("vsearch", code, &payload);
    if code == 200 {
        if parsed.json {
            println!("{payload}");
        } else {
            println!("{}", render_search(&value));
        }
        // No match is an ordinary outcome, not a failure, so it must not look like an error.
        std::process::exit(0);
    }
    report_read_failure("vsearch", code, &payload, &value)
}

const ORCH_USAGE: &str = "usage: vorch < proposal.json   (the proposal is read from stdin)\n\
    starts several sessions at once under the current one. Nothing is created until you confirm the\n\
    dialog that appears; you can edit or drop any entry there first.\n\
    \n\
    the JSON body:\n\
    {\n\
      \"title\": \"what this run is for\",\n\
      \"worktreeMode\": \"none\" | \"shared\" | \"each\",      // default: each\n\
      \"defaults\": { \"kind\": \"claude\", \"model\": \"...\", \"effort\": \"...\" },\n\
      \"agents\": [\n\
        { \"name\": \"short label\", \"prompt\": \"self-contained task\" },\n\
        { \"name\": \"...\", \"prompt\": \"...\", \"kind\": \"codex\", \"worktree\": false }\n\
      ]\n\
    }\n\
    \n\
    omit an agent's kind/model/effort/worktree to follow defaults. Each prompt must stand on its own:\n\
    a child session is a fresh conversation and sees nothing of this one.";

/// `vlx-term --orch` entry point used by the `vorch` shim: propose several sessions at once.
///
/// The proposal arrives on stdin rather than in arguments because each agent's prompt is multi-line
/// prose; quoting that through a shell reliably is not something to ask of a caller.
pub fn run_orch(args: &[String]) -> ! {
    let rest = &args[args.len().min(2)..];
    if rest.iter().any(|a| a == "-h" || a == "--help") {
        println!("{ORCH_USAGE}");
        std::process::exit(0);
    }
    if let Some(unknown) = rest.iter().find(|a| a.starts_with('-')) {
        eprintln!("vorch: unknown option {unknown}");
        eprintln!("{ORCH_USAGE}");
        std::process::exit(2);
    }

    let mut body = String::new();
    if std::io::stdin().read_to_string(&mut body).is_err() || body.trim().is_empty() {
        eprintln!("vorch: no proposal on stdin");
        eprintln!("{ORCH_USAGE}");
        std::process::exit(2);
    }

    let (url, sid, token) = match session_env() {
        Ok(v) => v,
        Err(msg) => {
            eprintln!("vorch: {msg}");
            std::process::exit(1);
        }
    };
    // The caller writes the proposal; the session it belongs to is not theirs to claim, so it is filled
    // in here from the injected environment.
    let body = match stamp_session_id(&body, &sid) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("vorch: {e}");
            std::process::exit(2);
        }
    };

    let endpoint = format!("{url}/orch?t={token}");
    let (code, payload) = match post_json_read(&endpoint, &body) {
        Some(v) => v,
        None => {
            eprintln!("vorch: cannot read a complete response from VelaTerm ({url})");
            std::process::exit(1);
        }
    };
    let value = read_value("vorch", code, &payload);
    if code == 200 {
        let agents = value.get("agents").and_then(|v| v.as_u64()).unwrap_or(0);
        println!(
            "proposed {agents} agents; confirm the dialog in VelaTerm to start them\n\
             nothing runs until you do, and you can edit or drop entries there"
        );
        std::process::exit(0);
    }
    report_read_failure("vorch", code, &payload, &value)
}

/// Insert the caller's own session id into the proposal, replacing anything they put there.
///
/// Returns a message rather than a panic when the body is not a JSON object, because the caller composed
/// it by hand and deserves to be told which part is wrong.
fn stamp_session_id(body: &str, sid: &str) -> Result<String, String> {
    let mut value: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("proposal is not valid JSON: {e}"))?;
    let obj = value
        .as_object_mut()
        .ok_or_else(|| "proposal must be a JSON object".to_string())?;
    obj.insert("sessionId".into(), serde_json::json!(sid));
    Ok(serde_json::Value::Object(obj.clone()).to_string())
}

const STAT_USAGE: &str = "usage: vstat [<orch-id>] [--wait] [--follow] [--timeout N] [--json]\n\
    reports which sessions are working, asking, or waiting.\n\
    <orch-id>    limit to one orchestration's agents, or `latest` for this session's newest run;\n\
                 omit for every session\n\
    --wait       block until something changes, instead of answering at once\n\
    --follow     keep watching, printing each change, until every agent is idle\n\
    --timeout N  seconds one wait may block (default 60, max 300)\n\
    --json       machine-readable output";

/// `vlx-term --stat` entry point used by the `vstat` shim.
///
/// Three shapes: answer now, block for one change, or follow until the work is done. The last is what an
/// orchestration's coordinator session runs — it costs nothing between events, because the waiting
/// happens in the service rather than in a sleep loop here.
pub fn run_stat(args: &[String]) -> ! {
    let rest = &args[args.len().min(2)..];
    let parsed = match parse_stat_args(rest) {
        StatParse::Help => {
            println!("{STAT_USAGE}");
            std::process::exit(0);
        }
        StatParse::Err(msg) => {
            eprintln!("{msg}");
            eprintln!("{STAT_USAGE}");
            std::process::exit(2);
        }
        StatParse::Ok(p) => p,
    };

    let (url, sid, token) = match session_env() {
        Ok(v) => v,
        Err(msg) => {
            eprintln!("vstat: {msg}");
            std::process::exit(1);
        }
    };
    let endpoint = format!("{url}/stat?t={token}");

    // The socket must outlive the longest wait the service may hold, or a quiet stretch would read as a
    // dead service. Plain snapshots keep the ordinary read timeout.
    let wait_secs = u64::from(parsed.timeout.unwrap_or(60)).clamp(1, 300);
    let socket_timeout = if parsed.wait {
        std::time::Duration::from_secs(wait_secs + 15)
    } else {
        READ_TIMEOUT
    };

    // `--follow` starts from a snapshot so the current picture prints at once, then walks the version
    // forward one change at a time. A bare `--wait` blocks first: the caller wants the next change.
    let mut since: Option<u64> = if parsed.wait && !parsed.follow {
        Some(0)
    } else {
        None
    };
    // The service wakes waiters on any session's change, not only this run's. Remember what was last
    // printed so an unrelated session's activity does not repeat an identical table.
    let mut last_printed: Option<Vec<(String, String)>> = None;
    loop {
        let body = build_stat_body(&sid, &parsed, since);
        let (code, payload) = match post_json_read_with(&endpoint, &body, socket_timeout) {
            Some(v) => v,
            None => {
                eprintln!("vstat: cannot read a complete response from VelaTerm ({url})");
                std::process::exit(1);
            }
        };
        let value = read_value("vstat", code, &payload);
        if code != 200 {
            report_read_failure("vstat", code, &payload, &value)
        }

        let states = session_states(&value);
        let changed = last_printed.as_ref() != Some(&states);
        if changed || !parsed.follow {
            if parsed.json {
                println!("{payload}");
            } else {
                println!("{}", render_stat(&value));
            }
            last_printed = Some(states);
        }
        if !parsed.follow {
            std::process::exit(0);
        }
        // Following ends when nothing is left to wait for. The summary goes to the screen, and the same
        // line goes out as a terminal notification so the finish is noticed from another tab: a plain
        // terminal's busy-to-idle transition is deliberately silent, so nothing else would announce it.
        if all_idle(&value) {
            let summary = render_stat_summary(&value);
            println!("\n{summary}");
            print!("{}", osc_notification("VelaTerm", &summary));
            let _ = std::io::stdout().flush();
            std::process::exit(0);
        }
        since = value.get("version").and_then(|v| v.as_u64()).or(since);
    }
}

/// `(session id, state)` pairs of a status answer, the part of it worth reprinting when it changes.
fn session_states(value: &serde_json::Value) -> Vec<(String, String)> {
    let empty = Vec::new();
    value
        .get("sessions")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty)
        .iter()
        .map(|s| {
            (
                s.get("sessionId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                s.get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
            )
        })
        .collect()
}

/// OSC 777 `notify` sequence, which the PTY monitor turns into a session notification. Semicolons in the
/// text would be read as field separators, so they are replaced.
fn osc_notification(title: &str, body: &str) -> String {
    let clean = |s: &str| s.replace(';', ",").replace(['\x1b', '\x07'], "");
    format!("\x1b]777;notify;{};{}\x07", clean(title), clean(body))
}

struct StatArgs {
    orch_id: Option<String>,
    wait: bool,
    follow: bool,
    timeout: Option<u32>,
    json: bool,
}

enum StatParse {
    Ok(StatArgs),
    Help,
    Err(String),
}

fn parse_stat_args(rest: &[String]) -> StatParse {
    let mut out = StatArgs {
        orch_id: None,
        wait: false,
        follow: false,
        timeout: None,
        json: false,
    };
    let mut words: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        let a = rest[i].as_str();
        match a {
            "-h" | "--help" => return StatParse::Help,
            "--json" => out.json = true,
            "--wait" => out.wait = true,
            // Following is a repeated wait, so it implies one.
            "--follow" => {
                out.follow = true;
                out.wait = true;
            }
            "--timeout" => {
                let Some(value) = rest.get(i + 1) else {
                    return StatParse::Err("vstat: --timeout needs a value".to_string());
                };
                i += 1;
                match value.parse::<u32>() {
                    Ok(n) if n > 0 => out.timeout = Some(n),
                    _ => {
                        return StatParse::Err(format!(
                            "vstat: --timeout needs a positive number of seconds, got {value}"
                        ))
                    }
                }
            }
            _ if a.starts_with('-') => return StatParse::Err(format!("vstat: unknown option {a}")),
            _ => words.push(a),
        }
        i += 1;
    }
    match words.len() {
        0 => StatParse::Ok(out),
        1 => {
            out.orch_id = Some(words[0].to_string());
            StatParse::Ok(out)
        }
        _ => StatParse::Err("vstat: expected at most one orchestration id".to_string()),
    }
}

fn build_stat_body(sid: &str, args: &StatArgs, since: Option<u64>) -> String {
    let mut obj = serde_json::Map::new();
    obj.insert("sessionId".into(), serde_json::json!(sid));
    if let Some(orch) = &args.orch_id {
        obj.insert("orchId".into(), serde_json::json!(orch));
    }
    if let Some(since) = since {
        obj.insert("since".into(), serde_json::json!(since));
    }
    if let Some(t) = args.timeout {
        obj.insert("timeout".into(), serde_json::json!(t));
    }
    serde_json::Value::Object(obj).to_string()
}

/// Whether every reported session has stopped: nothing working, nothing waiting on a person.
///
/// A session that never reported a state counts as not idle. Its process may still be starting, and
/// calling the run finished while one agent never even began would be the worst kind of wrong answer.
fn all_idle(value: &serde_json::Value) -> bool {
    let empty = Vec::new();
    let sessions = value
        .get("sessions")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);
    !sessions.is_empty()
        && sessions
            .iter()
            .all(|s| s.get("state").and_then(|v| v.as_str()) == Some("waiting"))
}

/// One line per session: id, state, name, and how long it has been in that state.
fn render_stat(value: &serde_json::Value) -> String {
    let empty = Vec::new();
    let sessions = value
        .get("sessions")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);
    if sessions.is_empty() {
        return "no sessions".to_string();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut lines: Vec<String> = Vec::new();
    if let Some(title) = value.get("title").and_then(|v| v.as_str()) {
        lines.push(format!("=== {title} ==="));
    }
    for s in sessions {
        let state = s.get("state").and_then(|v| v.as_str()).unwrap_or("unknown");
        let age = s
            .get("updatedAt")
            .and_then(|v| v.as_i64())
            .map(|t| ago(now - t))
            .unwrap_or_else(|| "-".to_string());
        lines.push(
            format!(
                "{}  {:<8}  {:<10}  {}  {}",
                short_id(s.get("sessionId").and_then(|v| v.as_str()).unwrap_or("")),
                state,
                s.get("kind").and_then(|v| v.as_str()).unwrap_or(""),
                s.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                age
            )
            .trim_end()
            .to_string(),
        );
    }
    lines.join("\n")
}

/// Closing line for `--follow`: what the run ended up doing.
fn render_stat_summary(value: &serde_json::Value) -> String {
    let empty = Vec::new();
    let sessions = value
        .get("sessions")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);
    let done = sessions.len();
    let title = value.get("title").and_then(|v| v.as_str()).unwrap_or("");
    if title.is_empty() {
        format!("all {done} sessions are idle")
    } else {
        format!("{title}: all {done} agents finished")
    }
}

/// Compact relative time such as `2m` or `just now`.
fn ago(secs: i64) -> String {
    match secs {
        s if s < 5 => "just now".to_string(),
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m", s / 60),
        s => format!("{}h", s / 3600),
    }
}

/// Print a failed read response and exit. 409 means an ambiguous session reference and gets its own
/// exit code so scripts can tell "nothing found" apart from "too many found".
fn report_read_failure(command: &str, code: u16, payload: &str, value: &serde_json::Value) -> ! {
    let reason = value
        .get("error")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("service returned {code}: {payload}"));
    eprintln!("{command}: {reason}");
    if code == 409 {
        if let Some(candidates) = value.get("candidates").and_then(|v| v.as_array()) {
            for c in candidates {
                eprintln!(
                    "  {}  {}",
                    short_id(c.get("sessionId").and_then(|v| v.as_str()).unwrap_or("")),
                    c.get("name").and_then(|v| v.as_str()).unwrap_or("")
                );
            }
        }
        std::process::exit(2);
    }
    std::process::exit(1);
}

/// Parsed `vrefer` arguments.
struct ReferArgs {
    target: String,
    last: Option<u32>,
    start: Option<u32>,
    end: Option<u32>,
    ask: Option<String>,
    with: Option<String>,
    timeout: Option<u32>,
    list: bool,
    json: bool,
}

enum ReferParse {
    Ok(ReferArgs),
    Help,
    Err(String),
}

/// Parse arguments after `--refer`. Exactly one session reference is expected unless `--list` is given.
fn parse_refer_args(rest: &[String]) -> ReferParse {
    let mut out = ReferArgs {
        target: String::new(),
        last: None,
        start: None,
        end: None,
        ask: None,
        with: None,
        timeout: None,
        list: false,
        json: false,
    };
    let mut words: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        let a = rest[i].as_str();
        match a {
            "-h" | "--help" => return ReferParse::Help,
            "--list" => out.list = true,
            "--json" => out.json = true,
            "--ask" | "--with" => {
                let Some(value) = rest.get(i + 1) else {
                    return ReferParse::Err(format!("vrefer: {a} needs a value"));
                };
                i += 1;
                if a == "--ask" {
                    if value.trim().is_empty() {
                        return ReferParse::Err("vrefer: --ask needs a question".to_string());
                    }
                    out.ask = Some(value.clone());
                } else {
                    out.with = Some(value.clone());
                }
            }
            "--timeout" => {
                let Some(value) = rest.get(i + 1) else {
                    return ReferParse::Err("vrefer: --timeout needs a value".to_string());
                };
                i += 1;
                match value.parse::<u32>() {
                    Ok(n) if n > 0 => out.timeout = Some(n),
                    _ => {
                        return ReferParse::Err(format!(
                            "vrefer: --timeout needs a positive number of seconds, got {value}"
                        ))
                    }
                }
            }
            "--last" | "--range" => {
                let Some(value) = rest.get(i + 1) else {
                    return ReferParse::Err(format!("vrefer: {a} needs a value"));
                };
                i += 1;
                if a == "--last" {
                    match value.parse::<u32>() {
                        Ok(n) if n > 0 => out.last = Some(n),
                        _ => {
                            return ReferParse::Err(format!(
                                "vrefer: --last needs a positive number, got {value}"
                            ))
                        }
                    }
                } else {
                    match parse_range(value) {
                        Some((s, e)) => {
                            out.start = Some(s);
                            out.end = Some(e);
                        }
                        None => {
                            return ReferParse::Err(format!(
                                "vrefer: --range needs the form A:B, got {value}"
                            ))
                        }
                    }
                }
            }
            _ if a.starts_with('-') => {
                return ReferParse::Err(format!("vrefer: unknown option {a}"));
            }
            _ => words.push(a),
        }
        i += 1;
    }
    if out.list {
        return ReferParse::Ok(out);
    }
    match words.len() {
        // Names with spaces must be quoted; accepting loose words would silently search for the wrong name.
        0 => ReferParse::Err("vrefer: missing session reference".to_string()),
        1 => {
            out.target = words[0].to_string();
            ReferParse::Ok(out)
        }
        _ => ReferParse::Err(format!(
            "vrefer: expected one session reference, got {}; quote names containing spaces",
            words.len()
        )),
    }
}

/// Parse a zero-based `A:B` message range, where A is included and B excluded.
fn parse_range(value: &str) -> Option<(u32, u32)> {
    let (a, b) = value.split_once(':')?;
    let start: u32 = a.trim().parse().ok()?;
    let end: u32 = b.trim().parse().ok()?;
    Some((start, end))
}

/// Parsed `vsearch` arguments.
struct SearchArgs {
    query: String,
    scope: Option<String>,
    limit: Option<u32>,
    json: bool,
}

enum SearchParse {
    Ok(SearchArgs),
    Help,
    Err(String),
}

/// Parse arguments after `--search`. Remaining words join into the query; everything after `--` is
/// query text even when it starts with `-`.
fn parse_search_args(rest: &[String]) -> SearchParse {
    let mut scope: Option<String> = None;
    let mut limit: Option<u32> = None;
    let mut json = false;
    let mut words: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        let a = rest[i].as_str();
        match a {
            "-h" | "--help" => return SearchParse::Help,
            "--json" => json = true,
            "--all" => scope = Some("all".to_string()),
            "--archived" => scope = Some("archived".to_string()),
            "--limit" => {
                let Some(value) = rest.get(i + 1) else {
                    return SearchParse::Err("vsearch: --limit needs a value".to_string());
                };
                i += 1;
                match value.parse::<u32>() {
                    Ok(n) if n > 0 => limit = Some(n),
                    _ => {
                        return SearchParse::Err(format!(
                            "vsearch: --limit needs a positive number, got {value}"
                        ))
                    }
                }
            }
            "--" => {
                for w in &rest[i + 1..] {
                    words.push(w.as_str());
                }
                break;
            }
            _ if a.starts_with('-') => {
                return SearchParse::Err(format!("vsearch: unknown option {a}"));
            }
            _ => words.push(a),
        }
        i += 1;
    }
    let query = words.join(" ");
    if query.trim().is_empty() {
        return SearchParse::Err("vsearch: missing search words".to_string());
    }
    SearchParse::Ok(SearchArgs {
        query,
        scope,
        limit,
        json,
    })
}

/// Build the `/refer` JSON body, omitting absent window fields so the server applies its defaults.
fn build_refer_body(sid: &str, args: &ReferArgs) -> String {
    let mut obj = serde_json::Map::new();
    obj.insert("sessionId".into(), serde_json::json!(sid));
    obj.insert("target".into(), serde_json::json!(args.target));
    obj.insert("list".into(), serde_json::json!(args.list));
    if let Some(n) = args.last {
        obj.insert("last".into(), serde_json::json!(n));
    }
    if let Some(s) = args.start {
        obj.insert("start".into(), serde_json::json!(s));
    }
    if let Some(e) = args.end {
        obj.insert("end".into(), serde_json::json!(e));
    }
    if let Some(ask) = &args.ask {
        obj.insert("ask".into(), serde_json::json!(ask));
    }
    if let Some(with) = &args.with {
        obj.insert("with".into(), serde_json::json!(with));
    }
    if let Some(t) = args.timeout {
        obj.insert("timeout".into(), serde_json::json!(t));
    }
    serde_json::Value::Object(obj).to_string()
}

/// Build the `/search` JSON body.
fn build_search_body(sid: &str, args: &SearchArgs) -> String {
    let mut obj = serde_json::Map::new();
    obj.insert("sessionId".into(), serde_json::json!(sid));
    obj.insert("query".into(), serde_json::json!(args.query));
    if let Some(scope) = &args.scope {
        obj.insert("scope".into(), serde_json::json!(scope));
    }
    if let Some(n) = args.limit {
        obj.insert("limit".into(), serde_json::json!(n));
    }
    serde_json::Value::Object(obj).to_string()
}

/// Snippets shown per session in text output. The full set is available through `--json`; printing all
/// of them would let one search crowd out the caller's own context.
const TEXT_SNIPPETS_PER_SESSION: usize = 3;

/// Longest snippet printed in text output before it is cut short.
const TEXT_SNIPPET_CHARS: usize = 200;

/// First eight characters of a session id: enough to identify it in output and to pass back to `vrefer`.
fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

/// Shorten an ISO timestamp to `MM-DD HH:MM` for headers, leaving anything unexpected untouched.
///
/// The value is printed as the transcript recorded it, with no timezone conversion.
fn short_time(ts: &str) -> String {
    let bytes = ts.as_bytes();
    if bytes.len() >= 16 && bytes[4] == b'-' && bytes[7] == b'-' && bytes[10] == b'T' {
        return format!("{} {}", &ts[5..10], &ts[11..16]);
    }
    ts.to_string()
}

/// Collapse a snippet's whitespace onto one line and cut it to a bounded length.
fn one_line(text: &str, max_chars: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        return collapsed;
    }
    let kept: String = collapsed.chars().take(max_chars).collect();
    format!("{kept}…")
}

/// Render a `/refer` transcript response as text for a model to read.
fn render_refer(value: &serde_json::Value) -> String {
    let session_id = value.get("sessionId").and_then(|v| v.as_str()).unwrap_or("");
    let name = value.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let kind = value.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    let archived = value.get("archived").and_then(|v| v.as_bool()).unwrap_or(false);
    let total = value.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
    let start = value.get("range").and_then(|v| v.get(0)).and_then(|v| v.as_u64()).unwrap_or(0);
    let end = value.get("range").and_then(|v| v.get(1)).and_then(|v| v.as_u64()).unwrap_or(0);
    let empty = Vec::new();
    let messages = value
        .get("messages")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);

    let archived_tag = if archived { " · archived" } else { "" };
    let window = if end > start {
        format!("showing #{start}-#{}", end - 1)
    } else {
        "showing none".to_string()
    };
    let mut out = format!(
        "=== Session: {name} ({}) · {kind}{archived_tag} · {total} messages · {window} ===\n",
        short_id(session_id)
    );

    for (offset, message) in messages.iter().enumerate() {
        let index = start as usize + offset;
        let role = message.get("role").and_then(|v| v.as_str()).unwrap_or("");
        let ts = message
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(short_time)
            .unwrap_or_default();
        let tools: Vec<&str> = message
            .get("tools")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|t| t.as_str()).collect())
            .unwrap_or_default();
        let tools = if tools.is_empty() {
            String::new()
        } else {
            format!(" ({})", tools.join(", "))
        };
        let stamp = if ts.is_empty() {
            String::new()
        } else {
            format!(" {ts}")
        };
        out.push_str(&format!("\n[#{index} {role}{stamp}]{tools}\n"));
        out.push_str(message.get("text").and_then(|v| v.as_str()).unwrap_or(""));
        out.push('\n');
    }

    // Only reachable when the caller narrowed the window themselves, since the default is everything.
    // Say how to get the rest so they do not have to remember the range syntax.
    if start > 0 {
        out.push_str(&format!(
            "\n(earlier messages: vrefer {} --range 0:{start})\n",
            short_id(session_id)
        ));
    }
    out
}

/// Render an answered `--ask` response: who answered and from what, then the answer.
///
/// The attribution line matters. What follows is one agent's reading of another session, not that
/// session's own words, and the caller should quote it as such.
fn render_answer(value: &serde_json::Value) -> String {
    let name = value.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let id = short_id(value.get("sessionId").and_then(|v| v.as_str()).unwrap_or(""));
    let total = value.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
    let by = value
        .get("summarizer")
        .and_then(|s| s.get("kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("an agent");
    let answer = value.get("answer").and_then(|v| v.as_str()).unwrap_or("");
    format!("[via {by} · read {total} messages from {name} ({id})]\n\n{answer}")
}

/// Render a `/refer --list` response as one line per session.
fn render_refer_list(value: &serde_json::Value) -> String {
    let empty = Vec::new();
    let sessions = value
        .get("sessions")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);
    if sessions.is_empty() {
        return "no sessions".to_string();
    }
    let mut lines: Vec<String> = Vec::new();
    for s in sessions {
        let archived = s.get("archived").and_then(|v| v.as_bool()).unwrap_or(false);
        let line = format!(
            "{}  {:<8}  {:<10}  {}  {}",
            short_id(s.get("sessionId").and_then(|v| v.as_str()).unwrap_or("")),
            if archived { "archived" } else { "live" },
            s.get("kind").and_then(|v| v.as_str()).unwrap_or(""),
            s.get("name").and_then(|v| v.as_str()).unwrap_or(""),
            s.get("cwd").and_then(|v| v.as_str()).unwrap_or("")
        );
        // Sessions without a working directory would otherwise end in padding nobody can see.
        lines.push(line.trim_end().to_string());
    }
    lines.join("\n")
}

/// Render a `/search` response as text, ending each session with the command that reads its transcript.
fn render_search(value: &serde_json::Value) -> String {
    let query = value.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let scope = value.get("scope").and_then(|v| v.as_str()).unwrap_or("live");
    let empty = Vec::new();
    let hits = value.get("hits").and_then(|v| v.as_array()).unwrap_or(&empty);
    let recording_only = value
        .get("recordingOnlySessions")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if hits.is_empty() {
        return match recording_only {
            0 => "no matches".to_string(),
            n => format!("no matches in conversations ({n} matched terminal output only)"),
        };
    }
    let total = value
        .get("totalSessions")
        .and_then(|v| v.as_u64())
        .unwrap_or(hits.len() as u64) as usize;

    let mut out = format!(
        "=== {} of {total} matching sessions for \"{query}\" ({scope}) ===\n",
        hits.len()
    );
    for (i, hit) in hits.iter().enumerate() {
        let session_id = hit.get("sessionId").and_then(|v| v.as_str()).unwrap_or("");
        let id = short_id(session_id);
        let archived = hit.get("archived").and_then(|v| v.as_bool()).unwrap_or(false);
        let match_count = hit.get("matchCount").and_then(|v| v.as_u64()).unwrap_or(0);
        out.push_str(&format!(
            "\n[{}] {} ({id}) · {}{} · {} · {match_count} matches\n",
            i + 1,
            hit.get("name").and_then(|v| v.as_str()).unwrap_or(""),
            hit.get("kind").and_then(|v| v.as_str()).unwrap_or(""),
            if archived { " · archived" } else { "" },
            hit.get("source").and_then(|v| v.as_str()).unwrap_or("")
        ));

        let matches = hit.get("matches").and_then(|v| v.as_array()).unwrap_or(&empty);
        for m in matches.iter().take(TEXT_SNIPPETS_PER_SESSION) {
            // Every match anchors to a message index, which `vrefer --range` consumes directly. The
            // ordinal is a fallback for the rare row indexed without one.
            let anchor = match m.get("messageIndex").and_then(|v| v.as_u64()) {
                Some(index) => format!("msg {index}"),
                None => format!("hit {}", m.get("ordinal").and_then(|v| v.as_u64()).unwrap_or(0)),
            };
            out.push_str(&format!(
                "      [{anchor}] {}\n",
                one_line(
                    m.get("snippet").and_then(|v| v.as_str()).unwrap_or(""),
                    TEXT_SNIPPET_CHARS
                )
            ));
        }
        if matches.len() > TEXT_SNIPPETS_PER_SESSION {
            out.push_str(&format!(
                "      ... {} more snippets, see --json\n",
                matches.len() - TEXT_SNIPPETS_PER_SESSION
            ));
        }
        out.push_str(&format!("    full transcript: vrefer {id}\n"));
    }
    // Say what was left out, so a smaller result than the desktop panel's is never a silent surprise.
    if recording_only > 0 {
        out.push_str(&format!(
            "\n({recording_only} more matched terminal output only, not conversations; not shown)\n"
        ));
    }
    out.trim_end().to_string()
}

/// POST and return both the status code and the response body.
///
/// Read commands need a complete decoded response, including HTTP chunked transfer encoding.
fn post_json_read(url: &str, body: &str) -> Option<(u16, String)> {
    post_json_read_with(url, body, READ_TIMEOUT)
}

/// Use the existing HTTP client so framing, UTF-8 across chunks, and truncated bodies are handled once.
fn post_json_read_with(
    url: &str,
    body: &str,
    timeout: std::time::Duration,
) -> Option<(u16, String)> {
    let agent = ureq::AgentBuilder::new().timeout(timeout).redirects(0).try_proxy_from_env(false).build();
    let response = match agent.post(url).set("Content-Type", "application/json").send_string(body) {
        Ok(response) | Err(ureq::Error::Status(_, response)) => response,
        Err(ureq::Error::Transport(_)) => return None,
    };
    let status = response.status();
    let mut payload = String::new();
    response.into_reader().read_to_string(&mut payload).ok()?;
    Some((status, payload))
}

/// A successful HTTP status is not evidence of a valid catalogue or transcript.
fn read_value(cli: &str, status: u16, payload: &str) -> serde_json::Value {
    match serde_json::from_str::<serde_json::Value>(payload) {
        Ok(value) if value.is_object() => value,
        _ if status != 200 => serde_json::Value::Null,
        _ => {
            eprintln!("{cli}: VelaTerm returned an invalid JSON response");
            std::process::exit(1);
        }
    }
}

const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[cfg(test)]
mod tests {
    use super::*;

    fn args(rest: &[&str]) -> Vec<String> {
        rest.iter().map(|s| s.to_string()).collect()
    }

    /// Body-building fixture: only the fields a test cares about are set by the caller.
    fn spawn_args(prompt: &str) -> SpawnArgs {
        SpawnArgs {
            worktree: false,
            cwd: None,
            kind: None,
            no_confirm: false,
            model: None,
            effort: None,
            prompt: prompt.to_string(),
        }
    }

    #[test]
    fn parse_spawn_collects_prompt_words() {
        let SpawnParse::Ok(p) = parse_spawn_args(&args(&["fix", "login", "bug"])) else {
            panic!("parsing should succeed");
        };
        assert_eq!(p.prompt, "fix login bug");
        assert!(!p.worktree);
        assert!(p.kind.is_none());
    }

    #[test]
    fn parse_spawn_flags() {
        let SpawnParse::Ok(p) = parse_spawn_args(&args(&["--worktree", "--codex", "do", "something"]))
        else {
            panic!("parsing should succeed");
        };
        assert!(p.worktree);
        assert_eq!(p.kind.as_deref(), Some("codex"));
        assert_eq!(p.prompt, "do something");

        // `--wt` alias followed by `--no-worktree` disables the worktree.
        let SpawnParse::Ok(p) = parse_spawn_args(&args(&["--wt", "--nowt", "x"])) else {
            panic!("parsing should succeed");
        };
        assert!(!p.worktree);
    }

    #[test]
    fn parse_spawn_double_dash_allows_leading_dash_prompt() {
        // Everything after `--` is prompt text, including words beginning with `-`.
        let SpawnParse::Ok(p) = parse_spawn_args(&args(&["--", "--not-a-flag", "here"])) else {
            panic!("parsing should succeed");
        };
        assert_eq!(p.prompt, "--not-a-flag here");
    }

    #[test]
    fn parse_spawn_help_and_errors() {
        assert!(matches!(parse_spawn_args(&args(&["-h"])), SpawnParse::Help));
        assert!(matches!(
            parse_spawn_args(&args(&["--bogus"])),
            SpawnParse::Err(_)
        ));
        // Missing prompt text.
        assert!(matches!(parse_spawn_args(&args(&[])), SpawnParse::Err(_)));
        assert!(matches!(
            parse_spawn_args(&args(&["--worktree"])),
            SpawnParse::Err(_)
        ));
    }

    #[test]
    fn build_spawn_body_omits_kind_when_none() {
        let body = build_spawn_body("p1", &spawn_args("fix a bug"), "/repo");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["parentSessionId"], "p1");
        assert_eq!(v["prompt"], "fix a bug");
        assert_eq!(v["worktree"], false);
        assert_eq!(v["cwd"], "/repo");
        assert!(
            v.get("kind").is_none(),
            "the field should be omitted when kind is empty"
        );
        assert!(
            v.get("noConfirm").is_none(),
            "the field should be omitted unless --yes was passed"
        );
        assert!(
            v.get("model").is_none() && v.get("effort").is_none(),
            "both should be omitted so the frontend keeps inheriting them"
        );
    }

    #[test]
    fn build_spawn_body_marks_no_confirm() {
        let body = build_spawn_body(
            "p1",
            &SpawnArgs {
                no_confirm: true,
                ..spawn_args("fix a bug")
            },
            "/repo",
        );
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["noConfirm"], true);
    }

    #[test]
    fn parse_spawn_yes_flag() {
        for flag in ["--yes", "-y", "--no-confirm"] {
            let SpawnParse::Ok(p) = parse_spawn_args(&args(&[flag, "do", "it"])) else {
                panic!("parsing should succeed");
            };
            assert!(p.no_confirm, "{flag} should skip the confirmation dialog");
            assert_eq!(p.prompt, "do it");
        }
        // Absent by default.
        let SpawnParse::Ok(p) = parse_spawn_args(&args(&["do", "it"])) else {
            panic!("parsing should succeed");
        };
        assert!(!p.no_confirm);
    }

    #[test]
    fn build_spawn_body_escapes_special_chars() {
        // serde_json escapes quotes, newlines, and backslashes without handwritten json_escape.
        let body = build_spawn_body(
            "p",
            &SpawnArgs {
                kind: Some("claude".to_string()),
                worktree: true,
                ..spawn_args("line1\n\"quotes\" \\backslash")
            },
            "/repo",
        );
        let v: serde_json::Value = serde_json::from_str(&body).expect("should be valid JSON");
        assert_eq!(v["prompt"], "line1\n\"quotes\" \\backslash");
        assert_eq!(v["kind"], "claude");
        assert_eq!(v["worktree"], true);
    }

    #[test]
    fn parse_refer_accepts_one_reference_and_window_options() {
        let ReferParse::Ok(p) = parse_refer_args(&args(&["2feead2c", "--last", "50"])) else {
            panic!("parsing should succeed");
        };
        assert_eq!(p.target, "2feead2c");
        assert_eq!(p.last, Some(50));
        assert!(!p.json && !p.list);

        let ReferParse::Ok(p) = parse_refer_args(&args(&["--range", "10:30", "my session", "--json"]))
        else {
            panic!("parsing should succeed");
        };
        assert_eq!(p.target, "my session");
        assert_eq!((p.start, p.end), (Some(10), Some(30)));
        assert!(p.json);

        // `--list` needs no reference.
        let ReferParse::Ok(p) = parse_refer_args(&args(&["--list"])) else {
            panic!("parsing should succeed");
        };
        assert!(p.list && p.target.is_empty());

        assert!(matches!(parse_refer_args(&args(&["-h"])), ReferParse::Help));
    }

    #[test]
    fn parse_refer_rejects_bad_input() {
        // A missing reference, several bare words (an unquoted name), and malformed option values.
        assert!(matches!(parse_refer_args(&args(&[])), ReferParse::Err(_)));
        assert!(matches!(
            parse_refer_args(&args(&["one", "two"])),
            ReferParse::Err(_)
        ));
        assert!(matches!(
            parse_refer_args(&args(&["s", "--last", "0"])),
            ReferParse::Err(_)
        ));
        assert!(matches!(
            parse_refer_args(&args(&["s", "--last"])),
            ReferParse::Err(_)
        ));
        assert!(matches!(
            parse_refer_args(&args(&["s", "--range", "10"])),
            ReferParse::Err(_)
        ));
        assert!(matches!(
            parse_refer_args(&args(&["s", "--bogus"])),
            ReferParse::Err(_)
        ));
    }

    #[test]
    fn parse_search_joins_words_and_reads_scope() {
        let SearchParse::Ok(p) = parse_search_args(&args(&["output", "scheduler", "--all"])) else {
            panic!("parsing should succeed");
        };
        assert_eq!(p.query, "output scheduler");
        assert_eq!(p.scope.as_deref(), Some("all"));
        assert!(p.limit.is_none() && !p.json);

        let SearchParse::Ok(p) =
            parse_search_args(&args(&["--archived", "--limit", "5", "--json", "throttle"]))
        else {
            panic!("parsing should succeed");
        };
        assert_eq!(p.query, "throttle");
        assert_eq!(p.scope.as_deref(), Some("archived"));
        assert_eq!(p.limit, Some(5));
        assert!(p.json);

        // Everything after `--` is query text, even words starting with a dash.
        let SearchParse::Ok(p) = parse_search_args(&args(&["--", "--all", "literally"])) else {
            panic!("parsing should succeed");
        };
        assert_eq!(p.query, "--all literally");
        assert!(p.scope.is_none(), "words after -- are not options");

        assert!(matches!(parse_search_args(&args(&[])), SearchParse::Err(_)));
        assert!(matches!(
            parse_search_args(&args(&["--limit", "x", "q"])),
            SearchParse::Err(_)
        ));
        assert!(matches!(parse_search_args(&args(&["--help"])), SearchParse::Help));
    }

    #[test]
    fn build_read_bodies_omit_absent_options() {
        let ReferParse::Ok(p) = parse_refer_args(&args(&["abc12345"])) else {
            panic!("parsing should succeed");
        };
        let v: serde_json::Value = serde_json::from_str(&build_refer_body("me", &p)).unwrap();
        assert_eq!(v["sessionId"], "me");
        assert_eq!(v["target"], "abc12345");
        assert_eq!(v["list"], false);
        assert!(v.get("last").is_none(), "the server applies its own default window");
        assert!(v.get("start").is_none() && v.get("end").is_none());

        let SearchParse::Ok(p) = parse_search_args(&args(&["needle"])) else {
            panic!("parsing should succeed");
        };
        let v: serde_json::Value = serde_json::from_str(&build_search_body("me", &p)).unwrap();
        assert_eq!(v["query"], "needle");
        assert!(v.get("scope").is_none() && v.get("limit").is_none());
    }

    #[test]
    fn parse_refer_reads_ask_options() {
        let ReferParse::Ok(p) = parse_refer_args(&args(&[
            "cbf83d22",
            "--ask",
            "how did throttling end up",
            "--with",
            "codex",
            "--timeout",
            "30",
        ])) else {
            panic!("parsing should succeed");
        };
        assert_eq!(p.ask.as_deref(), Some("how did throttling end up"));
        assert_eq!(p.with.as_deref(), Some("codex"));
        assert_eq!(p.timeout, Some(30));

        // Missing or empty values are refused rather than sent as a blank question.
        assert!(matches!(parse_refer_args(&args(&["s", "--ask"])), ReferParse::Err(_)));
        assert!(matches!(parse_refer_args(&args(&["s", "--ask", "  "])), ReferParse::Err(_)));
        assert!(matches!(parse_refer_args(&args(&["s", "--with"])), ReferParse::Err(_)));
        assert!(matches!(
            parse_refer_args(&args(&["s", "--timeout", "0"])),
            ReferParse::Err(_)
        ));
        assert!(matches!(
            parse_refer_args(&args(&["s", "--timeout", "soon"])),
            ReferParse::Err(_)
        ));
    }

    #[test]
    fn build_refer_body_carries_ask_options_only_when_set() {
        let ReferParse::Ok(p) = parse_refer_args(&args(&["s", "--ask", "why"])) else {
            panic!("parsing should succeed");
        };
        let v: serde_json::Value = serde_json::from_str(&build_refer_body("me", &p)).unwrap();
        assert_eq!(v["ask"], "why");
        assert!(
            v.get("with").is_none() && v.get("timeout").is_none(),
            "the server applies its own defaults"
        );

        // A plain read sends no ask field at all, so the old path is byte-for-byte unchanged.
        let ReferParse::Ok(p) = parse_refer_args(&args(&["s"])) else {
            panic!("parsing should succeed");
        };
        let v: serde_json::Value = serde_json::from_str(&build_refer_body("me", &p)).unwrap();
        assert!(v.get("ask").is_none());
    }

    #[test]
    fn render_answer_attributes_the_reading() {
        let value = serde_json::json!({
            "sessionId": "cbf83d22-88d5-4edc-ab36-9bcd2bdd517e",
            "name": "Output scheduler throttling",
            "total": 842,
            "question": "how did throttling end up",
            "answer": "Per-tab tiers: foreground uncapped, background on a shared 64KB budget.",
            "summarizer": {"kind": "claude", "elapsedMs": 18400},
            "messages": []
        });
        let out = render_answer(&value);
        assert!(
            out.starts_with("[via claude · read 842 messages from Output scheduler throttling (cbf83d22)]"),
            "the reader and the source must be named: {out}"
        );
        assert!(out.contains("Per-tab tiers"), "got: {out}");
        assert!(!out.contains("[#0"), "the transcript itself should not be reprinted: {out}");
    }

    #[test]
    fn stamp_session_id_overwrites_whatever_the_caller_claimed() {
        // The proposal is composed by a model; which session it belongs to is not its to assert.
        let stamped = stamp_session_id(
            r#"{"title":"t","sessionId":"someone-else","agents":[]}"#,
            "real-session",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&stamped).unwrap();
        assert_eq!(v["sessionId"], "real-session");
        assert_eq!(v["title"], "t");

        // A body without the field gets it added.
        let stamped = stamp_session_id(r#"{"title":"t"}"#, "s1").unwrap();
        let v: serde_json::Value = serde_json::from_str(&stamped).unwrap();
        assert_eq!(v["sessionId"], "s1");

        // Malformed input is reported, not panicked on: a person composed it by hand.
        assert!(stamp_session_id("not json", "s1").is_err());
        assert!(stamp_session_id("[1,2,3]", "s1").is_err(), "must be an object");
    }

    #[test]
    fn parse_stat_args_reads_the_three_shapes() {
        let StatParse::Ok(p) = parse_stat_args(&args(&[])) else {
            panic!("no arguments is a plain snapshot");
        };
        assert!(!p.wait && !p.follow && p.orch_id.is_none());

        let StatParse::Ok(p) = parse_stat_args(&args(&["orch-1", "--wait", "--timeout", "30"]))
        else {
            panic!("parsing should succeed");
        };
        assert_eq!(p.orch_id.as_deref(), Some("orch-1"));
        assert!(p.wait && !p.follow);
        assert_eq!(p.timeout, Some(30));

        // Following is a repeated wait, so it turns waiting on by itself.
        let StatParse::Ok(p) = parse_stat_args(&args(&["--follow"])) else {
            panic!("parsing should succeed");
        };
        assert!(p.follow && p.wait);

        assert!(matches!(parse_stat_args(&args(&["-h"])), StatParse::Help));
        assert!(matches!(parse_stat_args(&args(&["a", "b"])), StatParse::Err(_)));
        assert!(matches!(parse_stat_args(&args(&["--timeout", "0"])), StatParse::Err(_)));
        assert!(matches!(parse_stat_args(&args(&["--bogus"])), StatParse::Err(_)));
    }

    #[test]
    fn all_idle_requires_every_session_to_have_reported_waiting() {
        let mk = |states: &[Option<&str>]| {
            serde_json::json!({
                "sessions": states
                    .iter()
                    .map(|s| serde_json::json!({ "sessionId": "x", "state": s }))
                    .collect::<Vec<_>>()
            })
        };
        assert!(all_idle(&mk(&[Some("waiting"), Some("waiting")])));
        assert!(!all_idle(&mk(&[Some("waiting"), Some("working")])));
        // Asking is not finished: someone is blocked on a person.
        assert!(!all_idle(&mk(&[Some("waiting"), Some("asking")])));
        // A session that never reported may still be starting up; calling the run done would be the
        // worst possible wrong answer.
        assert!(!all_idle(&mk(&[Some("waiting"), None])));
        // Nothing to report is not the same as everything finished.
        assert!(!all_idle(&serde_json::json!({ "sessions": [] })));
    }

    #[test]
    fn render_stat_lists_sessions_with_their_state() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let value = serde_json::json!({
            "version": 7,
            "title": "break up the settings panel",
            "sessions": [
                {"sessionId": "aaaaaaaa-1111", "name": "split", "kind": "claude",
                 "state": "working", "updatedAt": now - 2},
                {"sessionId": "bbbbbbbb-2222", "name": "tests", "kind": "codex",
                 "state": "asking", "updatedAt": now - 130},
                {"sessionId": "cccccccc-3333", "name": "docs", "kind": "claude",
                 "state": null, "updatedAt": null}
            ]
        });
        let out = render_stat(&value);
        assert!(out.contains("=== break up the settings panel ==="), "got: {out}");
        assert!(out.contains("aaaaaaaa  working"), "got: {out}");
        assert!(out.contains("just now"), "got: {out}");
        assert!(out.contains("bbbbbbbb  asking") && out.contains("2m"), "got: {out}");
        // A session with no reported state says so rather than looking idle.
        assert!(out.contains("cccccccc  unknown"), "got: {out}");
        assert!(out.lines().all(|l| l == l.trim_end()), "no trailing padding: {out:?}");

        assert_eq!(render_stat(&serde_json::json!({"sessions": []})), "no sessions");
    }

    #[test]
    fn session_states_and_notification_shape() {
        let value = serde_json::json!({
            "sessions": [
                {"sessionId": "a", "state": "working"},
                {"sessionId": "b", "state": null}
            ]
        });
        // A never-reported state compares as `unknown`, so it still counts as a change when it arrives.
        assert_eq!(
            session_states(&value),
            vec![("a".to_string(), "working".to_string()), ("b".to_string(), "unknown".to_string())]
        );
        // OSC 777 fields are semicolon-separated, so text may not carry one through.
        assert_eq!(
            osc_notification("VelaTerm", "t: all 2 agents; done"),
            "\x1b]777;notify;VelaTerm;t: all 2 agents, done\x07"
        );
    }

    #[test]
    fn ago_reads_naturally_at_each_scale() {
        assert_eq!(ago(0), "just now");
        assert_eq!(ago(4), "just now");
        assert_eq!(ago(40), "40s");
        assert_eq!(ago(125), "2m");
        assert_eq!(ago(7300), "2h");
    }

    #[test]
    fn read_http_decodes_chunked_utf8_and_retains_error_status() {
        use std::net::TcpListener;
        use std::thread;
        // Keep temporary test listeners within the project's permitted non-default port range.
        let listener = (0..100).find_map(|_| {
            let n = u16::from_le_bytes(uuid::Uuid::new_v4().as_bytes()[..2].try_into().unwrap());
            TcpListener::bind(("127.0.0.1", 10000 + n % 39152)).ok()
        }).unwrap();
        let url = format!("http://{}/refer", listener.local_addr().unwrap());
        let payload = r#"{"sessions":[{"name":"会话"}]}"#;
        let expected = payload.to_string();
        let server = thread::spawn(move || {
            for (index, status) in ["200 OK", "404 Not Found", "200 OK"].iter().enumerate() {
                let (mut socket, _) = listener.accept().unwrap();
                socket.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    socket.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                if index == 2 {
                    socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{}").unwrap();
                } else {
                    write!(socket, "HTTP/1.1 {status}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").unwrap();
                    // Split in the middle of Chinese UTF-8 characters, as real network chunks may do.
                    for chunk in payload.as_bytes().chunks(3) {
                        write!(socket, "{:x}\r\n", chunk.len()).unwrap();
                        socket.write_all(chunk).unwrap();
                        socket.write_all(b"\r\n").unwrap();
                    }
                    socket.write_all(b"0\r\n\r\n").unwrap();
                }
            }
        });
        assert_eq!(post_json_read(&url, ""), Some((200, expected.clone())));
        assert_eq!(post_json_read(&url, ""), Some((404, expected)));
        assert!(post_json_read(&url, "").is_none(), "truncated responses must fail");
        server.join().unwrap();
    }

    #[test]
    fn render_refer_shows_a_window_and_how_to_reach_the_rest() {
        let value = serde_json::json!({
            "sessionId": "2feead2c-1111-4444-8888-000000000001",
            "name": "Refactor session views",
            "kind": "claude",
            "archived": false,
            "total": 84,
            "range": [82, 84],
            "messages": [
                {"role": "user", "text": "split the viewers", "timestamp": "2026-08-17T14:02:11Z", "tools": []},
                {"role": "assistant", "text": "done", "timestamp": "2026-08-17T14:03:40Z", "tools": ["Read", "Edit"]}
            ]
        });
        let out = render_refer(&value);
        assert!(out.contains("=== Session: Refactor session views (2feead2c) · claude · 84 messages · showing #82-#83 ==="), "got: {out}");
        assert!(out.contains("[#82 user 08-17 14:02]"), "got: {out}");
        assert!(out.contains("[#83 assistant 08-17 14:03] (Read, Edit)"), "got: {out}");
        assert!(out.contains("split the viewers") && out.contains("done"));
        assert!(
            out.contains("vrefer 2feead2c --range 0:82"),
            "a truncated window should say how to read the earlier messages: {out}"
        );

        // A full transcript has nothing earlier to offer.
        let whole = serde_json::json!({
            "sessionId": "abcdef12", "name": "n", "kind": "codex", "archived": true,
            "total": 1, "range": [0, 1],
            "messages": [{"role": "user", "text": "hi", "timestamp": null, "tools": []}]
        });
        let out = render_refer(&whole);
        assert!(out.contains("· archived ·"), "got: {out}");
        assert!(!out.contains("--range"), "got: {out}");
        assert!(out.contains("[#0 user]"), "a missing timestamp should be skipped: {out}");
    }

    #[test]
    fn render_search_caps_snippets_and_suggests_the_next_command() {
        let value = serde_json::json!({
            "query": "scheduler throttling",
            "scope": "live",
            "totalSessions": 1,
            "hits": [{
                "sessionId": "a9af7b1c-2222-4444-8888-000000000002",
                "name": "Output scheduler throttling",
                "kind": "claude",
                "archived": false,
                "source": "transcript",
                "matchCount": 12,
                "matches": [
                    {"messageIndex": 3, "ordinal": 1, "snippet": "background tabs\nrotate on a  budget"},
                    {"messageIndex": 7, "ordinal": 2, "snippet": "typing window compresses background writes"},
                    {"messageIndex": 9, "ordinal": 3, "snippet": "third"},
                    {"messageIndex": 11, "ordinal": 4, "snippet": "fourth"}
                ]
            }]
        });
        let out = render_search(&value);
        assert!(out.contains("=== 1 of 1 matching sessions for \"scheduler throttling\" (live) ==="), "got: {out}");
        assert!(out.contains("[1] Output scheduler throttling (a9af7b1c) · claude · transcript · 12 matches"), "got: {out}");
        // Snippets collapse to one line and are capped at three per session.
        assert!(out.contains("[msg 3] background tabs rotate on a budget"), "got: {out}");
        assert!(out.contains("[msg 9] third"), "got: {out}");
        assert!(!out.contains("fourth"), "only three snippets belong in text output: {out}");
        assert!(out.contains("... 1 more snippets, see --json"), "got: {out}");
        assert!(out.contains("full transcript: vrefer a9af7b1c"), "got: {out}");

        // A row indexed without a message index falls back to its ordinal.
        let no_anchor = serde_json::json!({
            "query": "q", "scope": "all", "totalSessions": 1,
            "hits": [{
                "sessionId": "98384f2e", "name": "n", "kind": "claude", "archived": true,
                "source": "transcript", "matchCount": 1,
                "matches": [{"messageIndex": null, "ordinal": 4, "snippet": "raw line"}]
            }]
        });
        let out = render_search(&no_anchor);
        assert!(out.contains("[hit 4] raw line"), "got: {out}");
        assert!(out.contains("· archived ·"), "got: {out}");

        // No hits reads as an ordinary outcome rather than an error.
        let empty = serde_json::json!({"query": "q", "scope": "live", "totalSessions": 0, "hits": []});
        assert_eq!(render_search(&empty), "no matches");
    }

    #[test]
    fn render_search_says_when_recording_only_sessions_were_left_out() {
        // Sessions whose only matches came from terminal recordings are dropped by the endpoint. The
        // count must be reported, or a smaller result than the desktop panel's looks like a bug.
        let value = serde_json::json!({
            "query": "q", "scope": "live", "totalSessions": 1, "recordingOnlySessions": 3,
            "hits": [{
                "sessionId": "abcdef12", "name": "n", "kind": "claude", "archived": false,
                "source": "transcript", "matchCount": 1,
                "matches": [{"messageIndex": 2, "ordinal": 1, "snippet": "hello"}]
            }]
        });
        let out = render_search(&value);
        assert!(
            out.contains("3 more matched terminal output only"),
            "got: {out}"
        );

        // The same holds when nothing is left to show at all.
        let all_dropped = serde_json::json!({
            "query": "q", "scope": "live", "totalSessions": 0, "recordingOnlySessions": 2, "hits": []
        });
        assert_eq!(
            render_search(&all_dropped),
            "no matches in conversations (2 matched terminal output only)"
        );
    }

    #[test]
    fn parse_spawn_model_and_effort() {
        // Space-separated form.
        let SpawnParse::Ok(p) =
            parse_spawn_args(&args(&["--model", "opus", "--effort", "high", "do", "it"]))
        else {
            panic!("parsing should succeed");
        };
        assert_eq!(p.model.as_deref(), Some("opus"));
        assert_eq!(p.effort.as_deref(), Some("high"));
        assert_eq!(p.prompt, "do it");

        // `key=value` form, and a model name carrying brackets such as `opus[1m]`.
        let SpawnParse::Ok(p) = parse_spawn_args(&args(&["--model=opus[1m]", "--effort=max", "x"]))
        else {
            panic!("parsing should succeed");
        };
        assert_eq!(p.model.as_deref(), Some("opus[1m]"));
        assert_eq!(p.effort.as_deref(), Some("max"));

        // Absent by default, which is what lets the child inherit the parent's arguments.
        let SpawnParse::Ok(p) = parse_spawn_args(&args(&["x"])) else {
            panic!("parsing should succeed");
        };
        assert!(p.model.is_none() && p.effort.is_none());
    }

    #[test]
    fn parse_spawn_cwd_in_both_forms() {
        let SpawnParse::Ok(spaced) = parse_spawn_args(&args(&["--cwd", "/repo one", "task"]))
        else {
            panic!("space-separated cwd should parse");
        };
        assert_eq!(spaced.cwd.as_deref(), Some("/repo one"));

        let SpawnParse::Ok(joined) = parse_spawn_args(&args(&["--cwd=relative/repo", "task"]))
        else {
            panic!("joined cwd should parse");
        };
        assert_eq!(joined.cwd.as_deref(), Some("relative/repo"));
    }

    #[test]
    fn parse_spawn_model_and_effort_need_values() {
        // A trailing flag has nothing to consume, and an empty value would launch a bare flag.
        for a in [
            vec!["--model"],
            vec!["--effort"],
            vec!["--model=", "x"],
            vec!["--effort=", "x"],
        ] {
            assert!(
                matches!(parse_spawn_args(&args(&a)), SpawnParse::Err(_)),
                "{a:?} should be rejected"
            );
        }
        // A value that looks like a flag is still a value: agents spell models like `-o` nowhere, but
        // taking the next word verbatim keeps the rule simple and predictable.
        let SpawnParse::Ok(p) = parse_spawn_args(&args(&["--model", "--codex", "x"])) else {
            panic!("parsing should succeed");
        };
        assert_eq!(p.model.as_deref(), Some("--codex"));
        assert!(
            p.kind.is_none(),
            "the consumed word must not also select a kind"
        );
    }

    #[test]
    fn render_refer_list_marks_archived_sessions() {
        let value = serde_json::json!({"sessions": [
            {"sessionId": "2feead2c-1111", "name": "live one", "kind": "claude", "archived": false, "cwd": "/work/a"},
            {"sessionId": "98384f2e-2222", "name": "old one", "kind": "codex", "archived": true, "cwd": null}
        ]});
        let out = render_refer_list(&value);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("2feead2c  live"), "got: {}", lines[0]);
        assert!(lines[0].contains("claude") && lines[0].contains("live one") && lines[0].contains("/work/a"));
        assert!(lines[1].contains("archived") && lines[1].contains("old one"), "got: {}", lines[1]);
        assert!(
            lines.iter().all(|l| *l == l.trim_end()),
            "a session without a cwd should not leave trailing padding: {out:?}"
        );

        assert_eq!(render_refer_list(&serde_json::json!({"sessions": []})), "no sessions");
    }

    #[test]
    fn short_time_only_rewrites_iso_timestamps() {
        assert_eq!(short_time("2026-08-17T14:02:11Z"), "08-17 14:02");
        assert_eq!(short_time("2026-08-17T14:02:11+08:00"), "08-17 14:02");
        // Anything that is not an ISO timestamp is printed as recorded.
        assert_eq!(short_time("yesterday"), "yesterday");
        assert_eq!(short_time(""), "");
    }

    #[test]
    fn one_line_collapses_whitespace_and_caps_length() {
        assert_eq!(one_line("  a\n\tb   c ", 100), "a b c");
        // Counting characters rather than bytes keeps multi-byte text from being cut mid-character.
        assert_eq!(one_line("中文内容很长", 3), "中文内…");
    }

    #[test]
    fn build_spawn_body_carries_model_and_effort() {
        let body = build_spawn_body(
            "p1",
            &SpawnArgs {
                model: Some("sonnet".to_string()),
                effort: Some("low".to_string()),
                ..spawn_args("fix a bug")
            },
            "/repo",
        );
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["model"], "sonnet");
        assert_eq!(v["effort"], "low");
    }

    #[test]
    fn build_view_body_shape() {
        let body = build_view_body("s1", "notes.md", "/work/dir");
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["sessionId"], "s1");
        assert_eq!(v["path"], "notes.md");
        assert_eq!(v["cwd"], "/work/dir");
    }
}
