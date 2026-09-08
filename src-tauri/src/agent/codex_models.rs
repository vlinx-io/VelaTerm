//! Model catalogue reported by Codex app-server.
//!
//! Unlike Claude Code, Codex exposes the account's current catalogue. The list is therefore queried from a
//! short-lived local app-server process instead of being duplicated in frontend or backend constants.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

const MODEL_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexModel {
    pub id: String,
    pub label: String,
    pub description: String,
    pub effort_levels: Vec<String>,
    pub context_window: Option<u64>,
    pub curated: bool,
    pub large_context: bool,
    /// Speeds this model can answer at, such as `fast`; empty when it has only the one.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub service_tiers: Vec<ServiceTier>,
    /// The tier the catalogue runs this model at unless told otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_service_tier: Option<String>,
    /// Whether this model honours the `personality` setting.
    pub supports_personality: bool,
}

/// One speed a Codex model offers, as the catalogue describes it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceTier {
    pub id: String,
    pub label: String,
    pub description: String,
}

pub fn list(bin: &str, extra_args: &[String]) -> Result<Vec<CodexModel>, String> {
    let mut command = Command::new(bin);
    command.arg("app-server").arg("--stdio");
    command.args(app_server_args(extra_args));
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = command.spawn().map_err(|e| format!("Failed to start Codex app-server: {e}"))?;
    let result = query(&mut child);
    let _ = child.kill();
    let _ = child.wait();
    result
}

/// Query one already-started process. Keeping cleanup in `list` means every error after spawn still reaps
/// the child, including a broken pipe during initialization.
fn query(child: &mut Child) -> Result<Vec<CodexModel>, String> {
    let mut stdin = child.stdin.take().ok_or("Codex app-server has no input stream")?;
    let stdout = child.stdout.take().ok_or("Codex app-server has no output stream")?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let init = json!({
        "id":"vlx-model-init",
        "method":"initialize",
        "params":{
            "clientInfo":{"name":"codex_app_server_daemon","title":"VelaTerm","version":env!("CARGO_PKG_VERSION")},
            "capabilities":{"experimentalApi":true}
        }
    });
    writeln!(stdin, "{init}").map_err(|e| format!("Failed to query Codex models: {e}"))?;
    stdin.flush().map_err(|e| format!("Failed to query Codex models: {e}"))?;

    let deadline = Instant::now() + MODEL_TIMEOUT;
    let initialized = receive_response(&rx, deadline, "vlx-model-init")?;
    if let Some(error) = initialized.get("error") {
        return Err(error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Codex initialization failed")
            .to_string());
    }
    for value in [
        json!({"method":"initialized","params":{}}),
        json!({"id":"vlx-model-list","method":"model/list","params":{}}),
    ] {
        writeln!(stdin, "{value}").map_err(|e| format!("Failed to query Codex models: {e}"))?;
    }
    stdin.flush().map_err(|e| format!("Failed to query Codex models: {e}"))?;
    let value = receive_response(&rx, deadline, "vlx-model-list")?;
    if let Some(error) = value.get("error") {
        return Err(error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Codex model query failed")
            .to_string());
    }
    Ok(parse_models(value.get("result").unwrap_or(&Value::Null)))
}

fn receive_response(rx: &Receiver<String>, deadline: Instant, wanted_id: &str) -> Result<Value, String> {
    loop {
        let wait = deadline.saturating_duration_since(Instant::now());
        if wait.is_zero() {
            return Err("Timed out while reading Codex models".to_string());
        }
        let line = match rx.recv_timeout(wait) {
            Ok(line) => line,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err("Timed out while reading Codex models".to_string())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("Codex app-server exited before returning models".to_string())
            }
        };
        let Ok(value) = serde_json::from_str::<Value>(&line) else { continue };
        if value.get("id").and_then(Value::as_str) != Some(wanted_id) {
            continue;
        }
        return Ok(value);
    }
}

/// Only options accepted by `codex app-server` survive; TUI-only flags are represented through protocol
/// fields by the chat engine and must not make the app-server launch fail.
pub fn app_server_args(extra_args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < extra_args.len() {
        let arg = &extra_args[i];
        let takes_value = matches!(arg.as_str(), "-c" | "--config" | "--enable" | "--disable" | "--code-mode-host");
        if takes_value && i + 1 < extra_args.len() {
            out.push(arg.clone());
            out.push(extra_args[i + 1].clone());
            i += 2;
            continue;
        }
        if arg == "--strict-config" || arg.starts_with("--config=") || arg.starts_with("--enable=") || arg.starts_with("--disable=") {
            out.push(arg.clone());
        }
        i += 1;
    }
    out
}

pub fn parse_models(result: &Value) -> Vec<CodexModel> {
    result
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|entry| !entry.get("hidden").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(|entry| {
            let id = entry.get("id").and_then(Value::as_str)?.to_string();
            let efforts = entry
                .get("supportedReasoningEfforts")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|value| value.get("reasoningEffort").and_then(Value::as_str).map(str::to_string))
                .collect();
            let service_tiers = entry
                .get("serviceTiers")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|tier| {
                    let id = tier.get("id").and_then(Value::as_str)?;
                    Some(ServiceTier {
                        id: id.to_string(),
                        label: tier.get("name").and_then(Value::as_str).unwrap_or(id).to_string(),
                        description: tier.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
                    })
                })
                .collect();
            Some(CodexModel {
                label: entry.get("displayName").and_then(Value::as_str).unwrap_or(&id).to_string(),
                description: entry.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
                id,
                effort_levels: efforts,
                context_window: entry.get("contextWindow").and_then(Value::as_u64),
                curated: true,
                large_context: false,
                service_tiers,
                default_service_tier: entry
                    .get("defaultServiceTier")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                supports_personality: entry
                    .get("supportsPersonality")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_visible_native_models_and_their_effort_ladders() {
        let models = parse_models(&json!({"data":[
            {"id":"gpt-5.6-terra","displayName":"GPT-5.6-Terra","description":"Balanced", "hidden":false,
             "supportedReasoningEfforts":[{"reasoningEffort":"low"},{"reasoningEffort":"high"}],
             "serviceTiers":[{"id":"default","name":"Standard","description":"Standard speed"},
                             {"id":"fast","name":"Fast","description":"1.5x speed, increased usage"}],
             "defaultServiceTier":"default","supportsPersonality":true},
            {"id":"hidden","hidden":true,"supportedReasoningEfforts":[]}
        ]}));
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "gpt-5.6-terra");
        assert_eq!(models[0].effort_levels, ["low", "high"]);
        assert_eq!(models[0].service_tiers.len(), 2);
        assert_eq!(models[0].service_tiers[1].id, "fast");
        assert_eq!(models[0].service_tiers[1].label, "Fast");
        assert_eq!(models[0].default_service_tier.as_deref(), Some("default"));
        assert!(models[0].supports_personality);
    }

    #[test]
    fn keeps_app_server_config_and_drops_tui_only_flags() {
        let args = vec![
            "--full-auto".into(),
            "-c".into(),
            "features.foo=true".into(),
            "--enable=bar".into(),
            "--no-alt-screen".into(),
        ];
        assert_eq!(app_server_args(&args), ["-c", "features.foo=true", "--enable=bar"]);
    }
}
