//! Cross-platform Rust implementation of the hidden `vlx-term --spawn` and `--view` subcommands.
//!
//! Thin PATH shims (`vspawn`, `vspawn-tree`, and `vopen`; see spawn_cli.rs) invoke these commands
//! through sh on Unix or .cmd on Windows. They read injected `VLX_SPAWN_URL`, `VLX_SESSION_ID`, and
//! `VLX_TOKEN`, build JSON, and POST to the local hook service's `/spawn` or `/view` endpoints.
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

    let body = build_spawn_body(&sid, &parsed.prompt, &parsed);
    let endpoint = format!("{url}/spawn?t={token}");
    match post_json(&endpoint, &body) {
        Some(code) if (200..300).contains(&code) => {
            let wt = if parsed.worktree {
                "isolated worktree"
            } else {
                "current dir"
            };
            let kind = parsed.kind.as_deref().unwrap_or("inherit current");
            let mut launch = String::new();
            if let Some(m) = parsed.model.as_deref() {
                launch.push_str(&format!(", model {m}"));
            }
            if let Some(e) = parsed.effort.as_deref() {
                launch.push_str(&format!(", effort {e}"));
            }
            println!("spawned sub-session ({kind}{launch}, {wt}): {}", parsed.prompt);
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

const SPAWN_USAGE: &str = "usage: vspawn [--worktree] [--claude|--codex|--copilot|--kiro] [--model <model>] [--effort <level>] [--name <name>] [--agent-args \"<raw args>\"] <task description...>\n\
    --model/--effort persist on the child session and map to agent-specific flags at launch\n\
    --name sets the child session name (otherwise derived from the task)\n\
    --agent-args replaces the per-agent default launch arguments";
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
    kind: Option<String>,
    prompt: String,
    model: Option<String>,
    effort: Option<String>,
    name: Option<String>,
    agent_args: Option<String>,
}

enum SpawnParse {
    Ok(SpawnArgs),
    Help,
    Err(String),
}

/// Parse arguments after `--spawn`. `--worktree/--wt` enables a worktree, `--no-worktree/--nowt`
/// disables it, agent flags select kind, help flags show usage, and everything after `--` is prompt
/// text even when prefixed by `-`. Other options are errors; remaining words join into a required prompt.
fn parse_spawn_args(rest: &[String]) -> SpawnParse {
    let mut worktree = false;
    let mut kind: Option<String> = None;
    let mut model: Option<String> = None;
    let mut effort: Option<String> = None;
    let mut name: Option<String> = None;
    let mut agent_args: Option<String> = None;
    let mut words: Vec<&str> = Vec::new();
    let mut i = 0;
    // Reads a value-taking option's required value; accepts values beginning with `-`.
    fn take_value<'a>(rest: &'a [String], i: &mut usize, flag: &str) -> Result<&'a str, String> {
        *i += 1;
        rest.get(*i)
            .map(|s| s.as_str())
            .ok_or_else(|| format!("vspawn: {flag} requires a value"))
    }
    while i < rest.len() {
        let a = rest[i].as_str();
        match a {
            "--worktree" | "--wt" => worktree = true,
            "--no-worktree" | "--nowt" => worktree = false,
            "--claude" => kind = Some("claude".to_string()),
            "--codex" => kind = Some("codex".to_string()),
            "--copilot" => kind = Some("copilot".to_string()),
            "--kiro" => kind = Some("kiro".to_string()),
            "--model" => match take_value(rest, &mut i, a) {
                Ok(v) => model = Some(v.to_string()),
                Err(e) => return SpawnParse::Err(e),
            },
            "--effort" => match take_value(rest, &mut i, a) {
                Ok(v) => effort = Some(v.to_string()),
                Err(e) => return SpawnParse::Err(e),
            },
            "--name" => match take_value(rest, &mut i, a) {
                Ok(v) => name = Some(v.to_string()),
                Err(e) => return SpawnParse::Err(e),
            },
            "--agent-args" => match take_value(rest, &mut i, a) {
                Ok(v) => agent_args = Some(v.to_string()),
                Err(e) => return SpawnParse::Err(e),
            },
            "-h" | "--help" => return SpawnParse::Help,
            "--" => {
                for w in &rest[i + 1..] {
                    words.push(w.as_str());
                }
                break;
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
        kind,
        prompt,
        model,
        effort,
        name,
        agent_args,
    })
}

/// Build the `/spawn` JSON body, omitting kind so the frontend can inherit it when absent. serde_json
/// safely escapes quotes, newlines, and backslashes.
fn build_spawn_body(sid: &str, prompt: &str, args: &SpawnArgs) -> String {
    let mut obj = serde_json::Map::new();
    obj.insert("parentSessionId".into(), serde_json::json!(sid));
    obj.insert("prompt".into(), serde_json::json!(prompt));
    obj.insert("worktree".into(), serde_json::json!(args.worktree));
    // Optional fields are omitted so the frontend can inherit or derive defaults.
    let optional = [
        ("kind", &args.kind),
        ("model", &args.model),
        ("effort", &args.effort),
        ("name", &args.name),
        ("agentArgs", &args.agent_args),
    ];
    for (key, value) in optional {
        if let Some(v) = value {
            obj.insert(key.into(), serde_json::json!(v));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(rest: &[&str]) -> Vec<String> {
        rest.iter().map(|s| s.to_string()).collect()
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

    /// Builds SpawnArgs with all optional fields unset for body tests.
    fn bare_args(worktree: bool, kind: Option<&str>) -> SpawnArgs {
        SpawnArgs {
            worktree,
            kind: kind.map(str::to_string),
            prompt: String::new(),
            model: None,
            effort: None,
            name: None,
            agent_args: None,
        }
    }

    #[test]
    fn parse_spawn_model_effort_name_agent_args() {
        let SpawnParse::Ok(p) = parse_spawn_args(&args(&[
            "--claude",
            "--model",
            "fable",
            "--effort",
            "high",
            "--name",
            "critical-auth",
            "--agent-args",
            "--foo bar",
            "implement",
            "auth",
        ])) else {
            panic!("parsing should succeed");
        };
        assert_eq!(p.kind.as_deref(), Some("claude"));
        assert_eq!(p.model.as_deref(), Some("fable"));
        assert_eq!(p.effort.as_deref(), Some("high"));
        assert_eq!(p.name.as_deref(), Some("critical-auth"));
        // The quoted value arrives as one argv word, even with a leading dash.
        assert_eq!(p.agent_args.as_deref(), Some("--foo bar"));
        assert_eq!(p.prompt, "implement auth");
    }

    #[test]
    fn parse_spawn_value_flags_require_values() {
        for flag in ["--model", "--effort", "--name", "--agent-args"] {
            assert!(
                matches!(parse_spawn_args(&args(&["task", flag])), SpawnParse::Err(_)),
                "{flag} without a value should be an error"
            );
        }
    }

    #[test]
    fn build_spawn_body_omits_kind_when_none() {
        let body = build_spawn_body("p1", "fix a bug", &bare_args(false, None));
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["parentSessionId"], "p1");
        assert_eq!(v["prompt"], "fix a bug");
        assert_eq!(v["worktree"], false);
        assert!(v.get("kind").is_none(), "the field should be omitted when kind is empty");
        // The launch-configuration fields are likewise omitted when unset.
        for key in ["model", "effort", "name", "agentArgs"] {
            assert!(v.get(key).is_none(), "{key} should be omitted when unset");
        }
    }

    #[test]
    fn build_spawn_body_includes_launch_config() {
        let mut a = bare_args(true, Some("codex"));
        a.model = Some("luna".to_string());
        a.effort = Some("xhigh".to_string());
        a.name = Some("update-types".to_string());
        a.agent_args = Some("--foo bar".to_string());
        let body = build_spawn_body("p1", "update types", &a);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["kind"], "codex");
        assert_eq!(v["model"], "luna");
        assert_eq!(v["effort"], "xhigh");
        assert_eq!(v["name"], "update-types");
        assert_eq!(v["agentArgs"], "--foo bar");
    }

    #[test]
    fn build_spawn_body_escapes_special_chars() {
        // serde_json escapes quotes, newlines, and backslashes without handwritten json_escape.
        let body = build_spawn_body(
            "p",
            "line1\n\"quotes\" \\backslash",
            &bare_args(true, Some("claude")),
        );
        let v: serde_json::Value = serde_json::from_str(&body).expect("should be valid JSON");
        assert_eq!(v["prompt"], "line1\n\"quotes\" \\backslash");
        assert_eq!(v["kind"], "claude");
        assert_eq!(v["worktree"], true);
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
