//! Cross-platform implementation of the hidden `vlx-term --agent-ctl` subcommand behind the
//! `vagent` PATH shim (see spawn_cli.rs). It parses one subcommand, POSTs JSON to the local hook
//! service's `/agent/<op>` endpoint, prints the JSON response, and exits. Output is JSON because
//! the primary consumer is a lead agent orchestrating child sessions.

use std::io::{Read, Write};
use std::net::TcpStream;

const ENV_URL: &str = "VLX_SPAWN_URL";
const ENV_SID: &str = "VLX_SESSION_ID";
const ENV_TOKEN: &str = "VLX_TOKEN";

const USAGE: &str = "usage:
  vagent spawn  [--profile <p>] [--agent <kind>] [--model <m>] [--effort <e>] [--name <n>] [--agent-args \"<raw>\"] [--permission-mode default|skip] [--worktree|--no-worktree] [--allow-unknown-launch-values] [--timeout <secs>] <task...>
  vagent spawn-status <requestId>
  vagent list
  vagent config
  vagent status <id|name>
  vagent wait   <id|name>... [--any|--all] [--timeout <secs>]
  vagent read   <id|name> [--full]
  vagent prompt <id|name> <text...>
  vagent cancel <id|name>|--all
  vagent diff   <id|name>
  vagent merge  <id|name> [--delete-worktree]
  vagent cleanup [--confirm]

All output is JSON. Sessions are addressed by id or unique name and must be
descendants of the current session. wait blocks until the target is no longer
working.

spawn blocks until the child exists. It returns {\"pending\":true} with a
requestId instead when the user has a confirmation card open, or when the
timeout passes. Keep working and collect the child later with
`vagent spawn-status <requestId>`, which returns the session row once the user
answers, 409 if they declined, and pending while the card is still open.

wait returns a \"blocked\" array naming targets stopped at a permission prompt.
Those are settled but not finished: tell the user, because only the user can
answer the prompt. An empty array means every target finished its turn.

wait also returns a \"failed\" array with session names and provider error text. Do not treat a
failed target as finished work or use it as a dependency. status and wait session rows include
lastTurnOutcome (ok, error, or unknown) and lastTurnError. read returns provider error text when no
assistant reply exists.

config prints the available profiles, their routing descriptions, the spawn
limits, and the current child counts. A profile supplies the agent, model,
effort, and worktree choice; an explicit flag overrides the profile. spawn returns 429 when a limit is reached,
so wait for a child to finish and try again. cancel --all interrupts every
running descendant.

Unknown model and effort values are advisory warnings on the confirmation card. The installed CLI
is authoritative, so newer values remain launchable. Use --allow-unknown-launch-values to suppress
the warning when the value is already verified.

--permission-mode sets the child's approval behavior: default keeps the agent's
own confirmations, skip passes its bypass flag. Without the flag the child
inherits the parent's mode, which is nothing when the parent has none. Set it
explicitly for a worker in its own worktree; otherwise the worker stalls on
approvals with nobody watching its pane.

cleanup lists the worktrees of finished children that hold no uncommitted
changes; --confirm removes those worktrees. A running child, or one whose
worktree has uncommitted changes, is reported as blocked and never touched.";

/// `vlx-term --agent-ctl ...` entry point behind the `vagent` shim.
pub fn run_agent_ctl(args: &[String]) -> ! {
    let rest = &args[args.len().min(2)..];
    let parsed = match parse_ctl_args(rest) {
        CtlParse::Help => {
            println!("{USAGE}");
            std::process::exit(0);
        }
        CtlParse::Err(msg) => {
            eprintln!("{msg}");
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
        CtlParse::Ok(cmd) => cmd,
    };

    let (url, sid, token) = match session_env() {
        Ok(v) => v,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(1);
        }
    };
    let mut body = parsed.body;
    body.insert("parentSessionId".into(), serde_json::json!(sid));
    let endpoint = format!("{url}/agent/{}?t={token}", parsed.op);
    match post_json(&endpoint, &serde_json::Value::Object(body).to_string()) {
        Some((status, resp)) if (200..300).contains(&status) => {
            println!("{resp}");
            std::process::exit(0);
        }
        Some((status, resp)) => {
            eprintln!("{}", server_error_envelope(&parsed.op, status, &resp));
            std::process::exit(1);
        }
        None => {
            eprintln!("vagent: cannot connect to VelaTerm ({url})");
            std::process::exit(1);
        }
    }
}

/// One parsed subcommand: the `/agent/<op>` path segment and its JSON body (before
/// parentSessionId is added from the environment).
struct CtlCmd {
    op: String,
    body: serde_json::Map<String, serde_json::Value>,
}

enum CtlParse {
    Ok(CtlCmd),
    Help,
    Err(String),
}

fn parse_ctl_args(rest: &[String]) -> CtlParse {
    let Some(op) = rest.first().map(String::as_str) else {
        return CtlParse::Err("vagent: missing subcommand".to_string());
    };
    if op == "-h" || op == "--help" {
        return CtlParse::Help;
    }
    let rest = &rest[1..];
    let mut body = serde_json::Map::new();
    let mut words: Vec<String> = Vec::new();
    let mut any = false;
    let mut all = false;
    let mut full = false;
    let mut confirm = false;
    let mut delete_worktree = false;
    let mut worktree: Option<bool> = None;
    let mut i = 0;
    // Reads a value-taking option's required value; accepts values beginning with `-`.
    fn take_value(rest: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
        *i += 1;
        rest.get(*i)
            .cloned()
            .ok_or_else(|| format!("vagent: {flag} requires a value"))
    }
    while i < rest.len() {
        let a = rest[i].as_str();
        let string_flags = [
            ("--agent", "kind"),
            ("--model", "model"),
            ("--effort", "effort"),
            ("--name", "name"),
            ("--agent-args", "agentArgs"),
            ("--profile", "profile"),
            ("--permission-mode", "permissionMode"),
        ];
        if let Some((_, key)) = string_flags.iter().find(|(f, _)| *f == a) {
            match take_value(rest, &mut i, a) {
                Ok(v) => {
                    body.insert((*key).to_string(), serde_json::json!(v));
                }
                Err(e) => return CtlParse::Err(e),
            }
        } else {
            match a {
                "--timeout" => match take_value(rest, &mut i, a) {
                    Ok(v) => match v.parse::<u64>() {
                        Ok(n) => {
                            body.insert("timeoutSecs".into(), serde_json::json!(n));
                        }
                        Err(_) => {
                            return CtlParse::Err("vagent: --timeout requires seconds".to_string())
                        }
                    },
                    Err(e) => return CtlParse::Err(e),
                },
                "--worktree" | "--wt" => worktree = Some(true),
                "--no-worktree" | "--nowt" => worktree = Some(false),
                "--any" => any = true,
                "--all" => all = true,
                "--full" => full = true,
                "--confirm" => confirm = true,
                "--delete-worktree" => delete_worktree = true,
                "--force" => {
                    body.insert("force".into(), serde_json::json!(true));
                }
                "--allow-unknown-launch-values" => {
                    body.insert("allowUnknownLaunchValues".into(), serde_json::json!(true));
                }
                "-h" | "--help" => return CtlParse::Help,
                "--" => {
                    words.extend(rest[i + 1..].iter().cloned());
                    break;
                }
                _ if a.starts_with('-') => {
                    return CtlParse::Err(format!("vagent: unknown option {a}"));
                }
                _ => words.push(a.to_string()),
            }
        }
        i += 1;
    }
    if any && all {
        return CtlParse::Err("vagent: --any and --all are mutually exclusive".to_string());
    }

    match op {
        "spawn" => {
            let prompt = words.join(" ");
            if prompt.trim().is_empty() {
                return CtlParse::Err("vagent: spawn needs a task description".to_string());
            }
            body.insert("prompt".into(), serde_json::json!(prompt));
            if let Some(w) = worktree {
                body.insert("worktree".into(), serde_json::json!(w));
            }
        }
        "list" | "config" => {}
        "spawn-status" => {
            let [request_id] = words.as_slice() else {
                return CtlParse::Err(
                    "vagent: spawn-status needs exactly one <requestId>".to_string(),
                );
            };
            body.insert("requestId".into(), serde_json::json!(request_id));
        }
        "cleanup" => {
            if !words.is_empty() {
                return CtlParse::Err("vagent: cleanup takes no target".to_string());
            }
            if confirm {
                body.insert("confirm".into(), serde_json::json!(true));
            }
        }
        "status" | "diff" | "merge" => {
            let [target] = words.as_slice() else {
                return CtlParse::Err(format!(
                    "vagent: {op} needs exactly one <id|name>"
                ));
            };
            body.insert("target".into(), serde_json::json!(target));
            if op == "merge" && delete_worktree {
                body.insert("deleteWorktree".into(), serde_json::json!(true));
            }
        }
        "cancel" => {
            if all {
                if !words.is_empty() {
                    return CtlParse::Err(
                        "vagent: cancel takes a target or --all, not both".to_string(),
                    );
                }
                body.insert("all".into(), serde_json::json!(true));
            } else {
                let [target] = words.as_slice() else {
                    return CtlParse::Err(
                        "vagent: cancel needs exactly one <id|name>, or --all".to_string(),
                    );
                };
                body.insert("target".into(), serde_json::json!(target));
            }
        }
        "read" => {
            let [target] = words.as_slice() else {
                return CtlParse::Err("vagent: read needs exactly one <id|name>".to_string());
            };
            body.insert("target".into(), serde_json::json!(target));
            if full {
                body.insert("full".into(), serde_json::json!(true));
            }
        }
        "wait" => {
            if words.is_empty() {
                return CtlParse::Err("vagent: wait needs at least one <id|name>".to_string());
            }
            body.insert("targets".into(), serde_json::json!(words));
            if any {
                body.insert("mode".into(), serde_json::json!("any"));
            }
        }
        "prompt" => {
            let [target, text @ ..] = words.as_slice() else {
                return CtlParse::Err("vagent: prompt needs <id|name> and text".to_string());
            };
            let text = text.join(" ");
            if text.trim().is_empty() {
                return CtlParse::Err("vagent: prompt needs text after the target".to_string());
            }
            body.insert("target".into(), serde_json::json!(target));
            body.insert("text".into(), serde_json::json!(text));
        }
        _ => return CtlParse::Err(format!("vagent: unknown subcommand {op}")),
    }
    CtlParse::Ok(CtlCmd {
        op: op.to_string(),
        body,
    })
}

fn session_env() -> Result<(String, String, String), String> {
    let get = |name: &str| match std::env::var(name) {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => Err(format!("not inside a VelaTerm session (missing {name})")),
    };
    Ok((get(ENV_URL)?, get(ENV_SID)?, get(ENV_TOKEN)?))
}

fn server_error_envelope(op: &str, status: u16, body: &str) -> String {
    let error = serde_json::from_str::<serde_json::Value>(body)
        .unwrap_or_else(|_| serde_json::Value::String(body.to_string()));
    serde_json::json!({ "op": op, "status": status, "error": error }).to_string()
}

/// POST JSON and return (status, body). None on connection failure.
pub(crate) fn post_json(url: &str, body: &str) -> Option<(u16, String)> {
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
    let text = String::from_utf8_lossy(&resp);
    let status: u16 = text.split_whitespace().nth(1)?.parse().ok()?;
    let payload = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.trim().to_string())
        .unwrap_or_default();
    Some((status, payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(rest: &[&str]) -> CtlParse {
        let v: Vec<String> = rest.iter().map(|s| s.to_string()).collect();
        parse_ctl_args(&v)
    }

    fn ok(rest: &[&str]) -> CtlCmd {
        match parse(rest) {
            CtlParse::Ok(c) => c,
            _ => panic!("parsing should succeed for {rest:?}"),
        }
    }

    #[test]
    fn parse_spawn_builds_full_body() {
        let c = ok(&[
            "spawn",
            "--agent",
            "claude",
            "--model",
            "fable",
            "--effort",
            "high",
            "--name",
            "critical-auth",
            "--worktree",
            "--timeout",
            "60",
            "implement",
            "auth",
        ]);
        assert_eq!(c.op, "spawn");
        assert_eq!(c.body["kind"], "claude");
        assert_eq!(c.body["model"], "fable");
        assert_eq!(c.body["effort"], "high");
        assert_eq!(c.body["name"], "critical-auth");
        assert_eq!(c.body["worktree"], true);
        assert_eq!(c.body["timeoutSecs"], 60);
        assert_eq!(c.body["prompt"], "implement auth");
    }

    #[test]
    fn parse_spawn_profile_and_conditional_worktree() {
        let c = ok(&["spawn", "--profile", "critical", "fix", "the", "parser"]);
        assert_eq!(c.body["profile"], "critical");
        assert!(!c.body.contains_key("worktree"));

        let c = ok(&["spawn", "--profile", "critical", "--no-worktree", "task"]);
        assert_eq!(c.body["worktree"], false);
        let c = ok(&["spawn", "--wt", "task"]);
        assert_eq!(c.body["worktree"], true);
        let c = ok(&["spawn", "--nowt", "task"]);
        assert_eq!(c.body["worktree"], false);
    }

    #[test]
    fn parse_config_and_cancel_all() {
        let c = ok(&["config"]);
        assert_eq!(c.op, "config");
        assert!(c.body.is_empty());

        let c = ok(&["cancel", "--all"]);
        assert_eq!(c.op, "cancel");
        assert_eq!(c.body["all"], true);
        assert!(!c.body.contains_key("target"));

        assert!(matches!(
            parse(&["cancel", "worker", "--all"]),
            CtlParse::Err(_)
        ));
        assert!(matches!(parse(&["cancel"]), CtlParse::Err(_)));

        let c = ok(&["cleanup"]);
        assert_eq!(c.op, "cleanup");
        assert!(c.body.is_empty());
        let c = ok(&["cleanup", "--confirm"]);
        assert_eq!(c.body["confirm"], true);
        assert!(matches!(parse(&["cleanup", "worker"]), CtlParse::Err(_)));
        assert!(matches!(parse(&["spawn", "--profile"]), CtlParse::Err(_)));
    }

    #[test]
    fn parse_target_subcommands() {
        let c = ok(&["status", "critical-auth"]);
        assert_eq!(c.body["target"], "critical-auth");

        let c = ok(&["wait", "a", "b", "--any", "--timeout", "30"]);
        assert_eq!(c.body["targets"], serde_json::json!(["a", "b"]));
        assert_eq!(c.body["mode"], "any");
        assert_eq!(c.body["timeoutSecs"], 30);

        let c = ok(&["read", "worker", "--full"]);
        assert_eq!(c.body["target"], "worker");
        assert_eq!(c.body["full"], true);

        let c = ok(&["prompt", "worker", "check", "the", "tests"]);
        assert_eq!(c.body["target"], "worker");
        assert_eq!(c.body["text"], "check the tests");

        let c = ok(&["cancel", "worker"]);
        assert_eq!(c.op, "cancel");
        assert_eq!(c.body["target"], "worker");

        let c = ok(&["diff", "worker"]);
        assert_eq!(c.op, "diff");
        assert_eq!(c.body["target"], "worker");

        let c = ok(&["merge", "worker", "--delete-worktree"]);
        assert_eq!(c.op, "merge");
        assert_eq!(c.body["target"], "worker");
        assert_eq!(c.body["deleteWorktree"], true);
    }

    #[test]
    fn parse_rejects_bad_input() {
        assert!(matches!(parse(&[]), CtlParse::Err(_)));
        assert!(matches!(parse(&["bogus"]), CtlParse::Err(_)));
        assert!(matches!(parse(&["spawn"]), CtlParse::Err(_)));
        assert!(matches!(parse(&["status"]), CtlParse::Err(_)));
        assert!(matches!(parse(&["status", "a", "b"]), CtlParse::Err(_)));
        assert!(matches!(parse(&["diff"]), CtlParse::Err(_)));
        assert!(matches!(parse(&["merge", "a", "b"]), CtlParse::Err(_)));
        assert!(matches!(parse(&["prompt", "only-target"]), CtlParse::Err(_)));
        assert!(matches!(
            parse(&["wait", "a", "--any", "--all"]),
            CtlParse::Err(_)
        ));
        assert!(matches!(parse(&["spawn", "--model"]), CtlParse::Err(_)));
        assert!(matches!(parse(&["-h"]), CtlParse::Help));
    }

    #[test]
    fn server_error_envelope_preserves_context_and_body_type() {
        let json_body = server_error_envelope(
            "merge",
            409,
            r#"{"error":"merge conflict","conflicts":["a.txt"]}"#,
        );
        let json_value: serde_json::Value = serde_json::from_str(&json_body).unwrap();
        assert_eq!(json_value["op"], "merge");
        assert_eq!(json_value["status"], 409);
        assert_eq!(json_value["error"]["error"], "merge conflict");
        assert_eq!(json_value["error"]["conflicts"][0], "a.txt");

        let text_body = server_error_envelope("spawn", 400, "invalid request");
        let text_value: serde_json::Value = serde_json::from_str(&text_body).unwrap();
        assert_eq!(text_value["op"], "spawn");
        assert_eq!(text_value["status"], 400);
        assert_eq!(text_value["error"], "invalid request");
    }
}
