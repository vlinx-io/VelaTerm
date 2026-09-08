//! Provider-owned completion catalogues and native skill invocation.

use std::borrow::Cow;
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::{codex_protocol, protocol};
use crate::models::SessionKind;

pub fn list_request(id: &str, cwd: Option<&str>) -> Value {
    codex_protocol::request(id, "skills/list", json!({
        "cwds": cwd.into_iter().collect::<Vec<_>>(), "forceReload": true
    }))
}

pub fn codex_commands(result: &Value) -> Result<Vec<Value>, String> {
    let groups = result.get("data").and_then(Value::as_array)
        .ok_or("Codex returned an invalid skill catalogue")?;
    let mut commands = Vec::new();
    let mut names = HashSet::new();
    for group in groups {
        if group.get("errors").and_then(Value::as_array).is_some_and(|errors| !errors.is_empty()) {
            return Err("Codex could not load all skills. Check the skill files and reopen the completion menu.".into());
        }
        let skills = group.get("skills").and_then(Value::as_array)
            .ok_or("Codex returned an invalid skill catalogue")?;
        for skill in skills {
            if skill.get("enabled").and_then(Value::as_bool) == Some(false) { continue; }
            let name = skill.get("name").and_then(Value::as_str).filter(|s| !s.is_empty())
                .ok_or("Codex returned a skill without a name")?;
            let path = skill.get("path").and_then(Value::as_str).filter(|s| !s.is_empty())
                .ok_or("Codex returned a skill without a path")?;
            if !names.insert(name) { continue; }
            commands.push(json!({
                "name": name,
                "description": skill.pointer("/interface/shortDescription")
                    .or_else(|| skill.get("description")).and_then(Value::as_str).unwrap_or(""),
                "invocation": "$", "skillPath": path
            }));
        }
    }
    Ok(commands)
}

pub fn claude_commands(response: &Value) -> Result<Vec<Value>, String> {
    let commands = response.get("commands").and_then(Value::as_array)
        .ok_or("Claude returned an invalid command catalogue")?;
    Ok(commands.iter().filter(|c| c.get("name").and_then(Value::as_str).is_some()).cloned()
        .map(|mut c| { c["invocation"] = json!("/"); c }).collect())
}

/// Normalize only a leading, known alias. Ordinary text and unknown names stay untouched.
pub fn needs_alias_lookup(kind: SessionKind, text: &str) -> bool {
    let text = text.trim_start();
    match kind {
        SessionKind::Claude => text.starts_with('$'),
        SessionKind::Codex => text.strip_prefix('/').is_some_and(|rest| {
            !is_local_command(rest.split_whitespace().next().unwrap_or(""))
        }),
        _ => false,
    }
}

fn is_local_command(name: &str) -> bool {
    matches!(name, "clear" | "new" | "rewind" | "compact" | "review")
}

pub fn native_text<'a>(text: &'a str, commands: &[Value]) -> Cow<'a, str> {
    let trimmed = text.trim_start();
    let Some(prefix) = trimmed.chars().next().filter(|c| matches!(c, '/' | '$')) else {
        return Cow::Borrowed(text);
    };
    let name = trimmed[1..].split_whitespace().next().unwrap_or("");
    // These belong to the surrounding conversation UI. `$` still reaches a same-named skill.
    if prefix == '/' && is_local_command(name) {
        return Cow::Borrowed(text);
    }
    let native = commands.iter().find(|c| c["name"].as_str() == Some(name))
        .and_then(|c| c["invocation"].as_str());
    match native {
        Some(native @ ("/" | "$")) if !trimmed.starts_with(native) => {
            let start = text.len() - trimmed.len();
            Cow::Owned(format!("{}{}{}", &text[..start], native, &trimmed[1..]))
        }
        _ => Cow::Borrowed(text),
    }
}

/// Attach resolved paths from the backend catalogue to both new turns and steering requests.
pub fn with_inputs(mut request: Value, text: &str, commands: &[Value]) -> Value {
    if let Some(input) = request.pointer_mut("/params/input").and_then(Value::as_array_mut) {
        for command in commands {
            let (Some(name), Some(path)) = (command["name"].as_str(), command["skillPath"].as_str()) else { continue; };
            let marker = format!("${name}");
            let mentioned = text.match_indices(&marker).any(|(at, _)| {
                (at == 0 || text[..at].chars().next_back().is_some_and(char::is_whitespace))
                    && text[at + marker.len()..].chars().next().is_none_or(|c|
                        !c.is_alphanumeric() && !matches!(c, '_' | '-' | ':' | '.' | '/'))
            });
            if mentioned { input.push(json!({"type":"skill", "name":name, "path":path})); }
        }
    }
    request
}

/// A catalogue probe opens no conversation and sends no model turn. This also serves an empty composer.
pub fn lookup(kind: SessionKind, bin: &str, cwd: Option<&str>, args: &[String]) -> Result<Vec<Value>, String> {
    let mut command = Command::new(bin);
    match kind {
        SessionKind::Codex => { command.arg("app-server").args(crate::agent::codex_models::app_server_args(args)); }
        SessionKind::Claude => {
            command.args(protocol::launch_args(None, None, None, None)).args(args);
        }
        _ => return Ok(Vec::new()),
    }
    if let Some(cwd) = cwd { command.current_dir(cwd); }
    for key in crate::pty::manager::AGENT_HARNESS_MARKERS { command.env_remove(key); }
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = command.spawn().map_err(|e| format!("Failed to read the skill catalogue: {e}"))?;
    let result = (|| {
        let mut stdin = child.stdin.take().ok_or("Skill probe has no input stream")?;
        let stdout = child.stdout.take().ok_or("Skill probe has no output stream")?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() { break; }
            }
        });
        let init = if kind == SessionKind::Codex { codex_protocol::initialize("skills_init") }
            else { protocol::control_request("skills_init", protocol::initialize()) };
        writeln!(stdin, "{init}").and_then(|_| stdin.flush()).map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let line = rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .map_err(|_| "The agent did not return its skill catalogue. Reopen the completion menu to retry.".to_string())?;
            let Ok(value) = serde_json::from_str::<Value>(&line) else { continue; };
            if kind == SessionKind::Claude {
                if value.pointer("/response/request_id").and_then(Value::as_str) == Some("skills_init") {
                    return claude_commands(&value["response"]["response"]);
                }
            } else if let Some(id) = value["id"].as_str() {
                if value.get("error").is_some() {
                    return Err("Codex could not read its skill catalogue. Reopen the completion menu to retry.".into());
                }
                if id == "skills_init" {
                    for req in [codex_protocol::initialized(), list_request("skills_list", cwd)] {
                        writeln!(stdin, "{req}").and_then(|_| stdin.flush()).map_err(|e| e.to_string())?;
                    }
                } else if id == "skills_list" { return codex_commands(&value["result"]); }
            }
        }
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue() -> Vec<Value> {
        codex_commands(&json!({"data":[{"skills":[
            {"name":"vspawn","description":"Spawn a session","path":"/skills/vspawn/SKILL.md","enabled":true},
            {"name":"compact","path":"/skills/compact/SKILL.md","enabled":true},
            {"name":"plugin:review","path":"/skills/review/SKILL.md","enabled":true},
            {"name":"disabled","path":"/skills/disabled/SKILL.md","enabled":false}
        ],"errors":[]}]})).unwrap()
    }

    #[test]
    fn catalogue_uses_provider_paths_and_excludes_disabled_skills() {
        let commands = catalogue();
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[0]["invocation"], "$");
        assert_eq!(commands[0]["skillPath"], "/skills/vspawn/SKILL.md");
        assert!(codex_commands(&json!({})).is_err());
        assert!(codex_commands(&json!({"data":[{"skills":[],"errors":[{"message":"invalid file"}]}]})).is_err());
        assert!(codex_commands(&json!({"data":[{"skills":[],"errors":[]}]})).unwrap().is_empty());
    }

    #[test]
    fn aliases_preserve_arguments_and_leave_commands_and_prose_alone() {
        let commands = catalogue();
        assert_eq!(native_text("/vspawn --cwd '/a b' task", &commands), "$vspawn --cwd '/a b' task");
        assert_eq!(native_text("$vspawn task", &commands), "$vspawn task");
        for text in ["/compact", "/review changes", "/unknown task", "explain /vspawn", "$disabled task"] {
            assert_eq!(native_text(text, &commands), text);
        }
        let claude = claude_commands(&json!({"commands":[{"name":"vspawn","argumentHint":"task"}]})).unwrap();
        assert_eq!(native_text("$vspawn task", &claude), "/vspawn task");
        assert_eq!(native_text("/vspawn task", &claude), "/vspawn task");
        assert_eq!(native_text("explain $vspawn", &claude), "explain $vspawn");
        assert!(needs_alias_lookup(SessionKind::Claude, "$vspawn task"));
        assert!(needs_alias_lookup(SessionKind::Codex, "/vspawn task"));
        assert!(!needs_alias_lookup(SessionKind::Codex, "$vspawn task"));
        assert!(!needs_alias_lookup(SessionKind::Codex, "/compact"));
        assert!(!needs_alias_lookup(SessionKind::Opencode, "$vspawn task"));
    }

    #[test]
    fn turn_and_steer_include_exact_skills_once_without_losing_other_input() {
        for method in ["turn/start", "turn/steer"] {
            let text = "$vspawn task $vspawn and $plugin:review";
            let request = json!({"method":method,"params":{"input":[
                {"type":"text","text":text},{"type":"image","url":"data:image/png;base64,AAAA"}
            ]}});
            let sent = with_inputs(request, text, &catalogue());
            assert_eq!(sent["params"]["input"].as_array().unwrap().len(), 4);
            assert_eq!(sent["params"]["input"][1]["type"], "image");
            assert_eq!(sent["params"]["input"][2], json!({"type":"skill","name":"vspawn","path":"/skills/vspawn/SKILL.md"}));
            assert_eq!(sent["params"]["input"][3]["name"], "plugin:review");
        }
        let sent = with_inputs(json!({"params":{"input":[]}}), "cost$vspawn $vspawn-tree $vspawn/path $disabled", &catalogue());
        assert!(sent["params"]["input"].as_array().unwrap().is_empty());
    }
}
