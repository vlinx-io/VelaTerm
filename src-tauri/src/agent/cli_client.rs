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

    let body = build_spawn_body(
        &sid,
        &parsed.prompt,
        parsed.kind.as_deref(),
        parsed.worktree,
        parsed.no_confirm,
    );
    let endpoint = format!("{url}/spawn?t={token}");
    match post_json(&endpoint, &body) {
        Some(code) if (200..300).contains(&code) => {
            let wt = if parsed.worktree {
                "isolated worktree"
            } else {
                "current dir"
            };
            let kind = parsed.kind.as_deref().unwrap_or("inherit current");
            println!("spawned sub-session ({kind}, {wt}): {}", parsed.prompt);
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
    "usage: vspawn [--worktree] [--yes] [--claude|--codex|--copilot|--kiro] <task description...>\n\
    --yes  skip the confirmation dialog and start the child session with default settings";
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
    /// Skip the confirmation dialog and start the child session with default settings.
    no_confirm: bool,
    prompt: String,
}

enum SpawnParse {
    Ok(SpawnArgs),
    Help,
    Err(String),
}

/// Parse arguments after `--spawn`. `--worktree/--wt` enables a worktree, `--no-worktree/--nowt`
/// disables it, `--yes/-y/--no-confirm` skips the confirmation dialog, agent flags select kind, help
/// flags show usage, and everything after `--` is prompt text even when prefixed by `-`. Other options
/// are errors; remaining words join into a required prompt.
fn parse_spawn_args(rest: &[String]) -> SpawnParse {
    let mut worktree = false;
    let mut kind: Option<String> = None;
    let mut no_confirm = false;
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
        no_confirm,
        prompt,
    })
}

/// Build the `/spawn` JSON body, omitting kind so the frontend can inherit it when absent. serde_json
/// safely escapes quotes, newlines, and backslashes. `noConfirm` is only written when set, keeping the
/// body identical to previous builds for ordinary spawns.
fn build_spawn_body(
    sid: &str,
    prompt: &str,
    kind: Option<&str>,
    worktree: bool,
    no_confirm: bool,
) -> String {
    let mut obj = serde_json::Map::new();
    obj.insert("parentSessionId".into(), serde_json::json!(sid));
    obj.insert("prompt".into(), serde_json::json!(prompt));
    obj.insert("worktree".into(), serde_json::json!(worktree));
    if no_confirm {
        obj.insert("noConfirm".into(), serde_json::json!(true));
    }
    if let Some(k) = kind {
        obj.insert("kind".into(), serde_json::json!(k));
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

    #[test]
    fn build_spawn_body_omits_kind_when_none() {
        let body = build_spawn_body("p1", "fix a bug", None, false, false);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["parentSessionId"], "p1");
        assert_eq!(v["prompt"], "fix a bug");
        assert_eq!(v["worktree"], false);
        assert!(v.get("kind").is_none(), "the field should be omitted when kind is empty");
        assert!(
            v.get("noConfirm").is_none(),
            "the field should be omitted unless --yes was passed"
        );
    }

    #[test]
    fn build_spawn_body_marks_no_confirm() {
        let body = build_spawn_body("p1", "fix a bug", None, false, true);
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
        let body = build_spawn_body("p", "line1\n\"quotes\" \\backslash", Some("claude"), true, false);
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
