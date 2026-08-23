//! Codex hook discovery and user-controlled state updates through app-server.
//!
//! VelaTerm's own lifecycle hooks reach Codex as session flags (`-c hooks=...`), not as configuration file
//! entries. Codex still refuses to run them until their hashes are recorded in `hooks.state`, so this module
//! both reads hooks for the settings panel and records that trust before a session launches.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::codex_app_server::CodexAppServer;

/// Codex's `source` value for hooks supplied through `-c hooks=...`. VelaTerm is the only component that
/// passes that flag, so every hook reported with this source belongs to VelaTerm.
const SESSION_FLAGS_SOURCE: &str = "sessionFlags";

/// The `source` reported to the settings panel for VelaTerm's own hooks.
const VELATERM_SOURCE: &str = "velaterm";

const VELATERM_SOURCE_PATH: &str = "VelaTerm session hooks";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexHookError {
    pub message: String,
    pub path: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexHook {
    pub key: String,
    pub event_name: String,
    pub handler_type: String,
    pub matcher: Option<String>,
    pub command: Option<String>,
    pub timeout_sec: u64,
    pub status_message: Option<String>,
    pub source_path: String,
    pub source: String,
    pub plugin_id: Option<String>,
    pub display_order: i64,
    pub enabled: bool,
    pub current_hash: String,
    pub trust_status: String,
    pub is_managed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexHookListEntry {
    pub cwd: String,
    pub hooks: Vec<CodexHook>,
    pub warnings: Vec<String>,
    pub errors: Vec<CodexHookError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodexHooksResponse {
    pub data: Vec<CodexHookListEntry>,
}

pub fn list(bin_path: Option<&str>, cwds: Vec<String>) -> Result<CodexHooksResponse, String> {
    let mut server = start_server(bin_path)?;
    let mut response = request_list(&mut server, &cwds)?;
    if let Some(states) = missing_session_hook_states(&response) {
        write_hook_states(&mut server, states)?;
        response = request_list(&mut server, &cwds)?;
    }
    relabel_session_hooks(&mut response);
    Ok(response)
}

/// Records trust for VelaTerm's session hooks so Codex runs them without an interactive `/hooks` prompt.
///
/// Codex derives each hash from the hook text, so the recorded hashes stay valid until Codex or
/// `build_codex_hooks` changes. Callers may cache on that pair rather than repeating this on every spawn.
pub fn ensure_session_hook_trust(bin_path: Option<&str>) -> Result<(), String> {
    let mut server = start_server(bin_path)?;
    let response = request_list(&mut server, &[])?;
    if let Some(states) = missing_session_hook_states(&response) {
        write_hook_states(&mut server, states)?;
    }
    Ok(())
}

pub fn update(
    bin_path: Option<&str>,
    cwds: Vec<String>,
    key: &str,
    enabled: Option<bool>,
    trusted_hash: Option<&str>,
) -> Result<CodexHooksResponse, String> {
    if key.trim().is_empty() || key.len() > 8192 {
        return Err("Invalid Codex hook key".to_string());
    }
    if enabled.is_none() && trusted_hash.is_none() {
        return Err("Codex hook update has no state change".to_string());
    }
    if trusted_hash.is_some_and(|hash| hash.trim().is_empty() || hash.len() > 256) {
        return Err("Invalid Codex hook hash".to_string());
    }

    let mut server = start_server(bin_path)?;
    // The panel disables these controls, but the same command is reachable from paired remote clients.
    // Reject anything Codex did not just report, and anything VelaTerm or an administrator controls.
    let current = request_list(&mut server, &cwds)?;
    let target = find_hook(&current, key)
        .ok_or_else(|| format!("Unknown Codex hook key {key}"))?
        .clone();
    if target.is_managed || target.source == SESSION_FLAGS_SOURCE {
        return Err("This Codex hook is managed and cannot be changed".to_string());
    }

    let mut state = serde_json::Map::new();
    if let Some(enabled) = enabled {
        state.insert("enabled".to_string(), serde_json::json!(enabled));
    }
    if let Some(trusted_hash) = trusted_hash {
        state.insert("trusted_hash".to_string(), serde_json::json!(trusted_hash));
    }
    let mut hook_states = serde_json::Map::new();
    hook_states.insert(key.to_string(), serde_json::Value::Object(state));

    write_hook_states(&mut server, hook_states)?;
    let mut response = request_list(&mut server, &cwds)?;
    relabel_session_hooks(&mut response);
    Ok(response)
}

fn start_server(bin_path: Option<&str>) -> Result<CodexAppServer, String> {
    CodexAppServer::start(bin_path, &super::inject::codex_hook_config_overrides())
}

/// An empty `cwds` makes Codex substitute its own working directory, which for a spawned child is VelaTerm's
/// and carries no project hooks. Anchor on the home directory so the result does not depend on where the app
/// was started.
fn request_list(server: &mut CodexAppServer, cwds: &[String]) -> Result<CodexHooksResponse, String> {
    let fallback = crate::host::home_dir().map(|home| home.to_string_lossy().to_string());
    let cwds: Vec<String> = if cwds.is_empty() {
        fallback.into_iter().collect()
    } else {
        cwds.to_vec()
    };
    let result = server.request(
        "hooks/list",
        serde_json::json!({ "cwds": cwds }),
        Duration::from_secs(8),
    )?;
    serde_json::from_value(result)
        .map_err(|error| format!("invalid Codex hooks/list response: {error}"))
}

fn write_hook_states(
    server: &mut CodexAppServer,
    states: serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    server.request(
        "config/batchWrite",
        serde_json::json!({
            "edits": [{
                "keyPath": "hooks.state",
                "value": serde_json::Value::Object(states),
                // Upsert merges per hook key, so writing `enabled` alone preserves an existing `trusted_hash`.
                "mergeStrategy": "upsert"
            }],
            "reloadUserConfig": true
        }),
        Duration::from_secs(8),
    )?;
    Ok(())
}

fn session_hooks(response: &CodexHooksResponse) -> impl Iterator<Item = &CodexHook> {
    response
        .data
        .iter()
        .flat_map(|entry| entry.hooks.iter())
        .filter(|hook| hook.source == SESSION_FLAGS_SOURCE)
}

fn find_hook<'a>(response: &'a CodexHooksResponse, key: &str) -> Option<&'a CodexHook> {
    response
        .data
        .iter()
        .flat_map(|entry| entry.hooks.iter())
        .find(|hook| hook.key == key)
}

/// The `hooks.state` entries needed to make every VelaTerm session hook trusted and enabled, or `None` when
/// they already are.
fn missing_session_hook_states(
    response: &CodexHooksResponse,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    let mut states = serde_json::Map::new();
    for hook in session_hooks(response) {
        if hook.trust_status == "trusted" && hook.enabled {
            continue;
        }
        states.insert(
            hook.key.clone(),
            serde_json::json!({ "enabled": true, "trusted_hash": hook.current_hash }),
        );
    }
    (!states.is_empty()).then_some(states)
}

/// Presents VelaTerm's session hooks under its own name and locks them, so the panel does not offer controls
/// for definitions the user cannot edit.
fn relabel_session_hooks(response: &mut CodexHooksResponse) {
    for entry in &mut response.data {
        for hook in &mut entry.hooks {
            if hook.source != SESSION_FLAGS_SOURCE {
                continue;
            }
            hook.source = VELATERM_SOURCE.to_string();
            hook.source_path = VELATERM_SOURCE_PATH.to_string();
            hook.is_managed = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hook(key: &str, source: &str, trust_status: &str, enabled: bool) -> CodexHook {
        CodexHook {
            key: key.to_string(),
            event_name: "sessionStart".to_string(),
            handler_type: "command".to_string(),
            matcher: None,
            command: Some("\"$VLX_EXE\" --codex-hook ready".to_string()),
            timeout_sec: 5,
            status_message: None,
            source_path: "/<session-flags>/config.toml".to_string(),
            source: source.to_string(),
            plugin_id: None,
            display_order: 0,
            enabled,
            current_hash: format!("sha256:{key}"),
            trust_status: trust_status.to_string(),
            is_managed: false,
        }
    }

    fn response(hooks: Vec<CodexHook>) -> CodexHooksResponse {
        CodexHooksResponse {
            data: vec![CodexHookListEntry {
                cwd: "/work".to_string(),
                hooks,
                warnings: Vec::new(),
                errors: Vec::new(),
            }],
        }
    }

    #[test]
    fn hook_response_accepts_the_public_app_server_shape() {
        let response: CodexHooksResponse = serde_json::from_value(serde_json::json!({
            "data": [{
                "cwd": "/work",
                "hooks": [{
                    "key": "/home/me/.codex/hooks.json:post_tool_use:0:0",
                    "eventName": "postToolUse",
                    "handlerType": "command",
                    "matcher": "Edit|Write",
                    "command": "python3 format.py",
                    "timeoutSec": 5,
                    "statusMessage": null,
                    "additionalContextLimit": null,
                    "sourcePath": "/home/me/.codex/hooks.json",
                    "source": "user",
                    "pluginId": null,
                    "displayOrder": 0,
                    "enabled": true,
                    "currentHash": "sha256:abc",
                    "trustStatus": "untrusted",
                    "isManaged": false
                }],
                "warnings": [],
                "errors": []
            }]
        }))
        .expect("public hooks/list response should parse");

        assert_eq!(response.data[0].hooks[0].event_name, "postToolUse");
        assert_eq!(response.data[0].hooks[0].trust_status, "untrusted");
    }

    #[test]
    fn untrusted_session_hooks_are_queued_for_trust() {
        let response = response(vec![
            hook("session_start", SESSION_FLAGS_SOURCE, "untrusted", true),
            hook("stop", SESSION_FLAGS_SOURCE, "trusted", false),
            hook("user_hook", "user", "untrusted", true),
        ]);

        let states = missing_session_hook_states(&response).expect("two hooks need repair");

        assert_eq!(states.len(), 2);
        assert_eq!(states["session_start"]["enabled"], serde_json::json!(true));
        assert_eq!(
            states["session_start"]["trusted_hash"],
            serde_json::json!("sha256:session_start")
        );
        // A user hook stays untouched: only VelaTerm's own hooks are trusted automatically.
        assert!(!states.contains_key("user_hook"));
    }

    #[test]
    fn trusted_session_hooks_need_no_write() {
        let response = response(vec![
            hook("session_start", SESSION_FLAGS_SOURCE, "trusted", true),
            hook("user_hook", "user", "untrusted", true),
        ]);

        assert!(missing_session_hook_states(&response).is_none());
    }

    #[test]
    fn session_hooks_are_presented_as_locked_velaterm_hooks() {
        let mut response = response(vec![
            hook("session_start", SESSION_FLAGS_SOURCE, "trusted", true),
            hook("user_hook", "user", "trusted", true),
        ]);

        relabel_session_hooks(&mut response);

        let hooks = &response.data[0].hooks;
        assert_eq!(hooks[0].source, VELATERM_SOURCE);
        assert_eq!(hooks[0].source_path, VELATERM_SOURCE_PATH);
        assert!(hooks[0].is_managed);
        assert_eq!(hooks[1].source, "user");
        assert!(!hooks[1].is_managed);
    }

    #[test]
    #[ignore = "depends on the locally installed Codex app-server"]
    fn installed_codex_hooks_list_smoke() {
        let cwd = std::env::current_dir()
            .expect("current directory")
            .to_string_lossy()
            .to_string();
        let response = list(None, vec![cwd.clone()]).expect("installed Codex should list hooks");
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].cwd, cwd);

        let session_hooks: Vec<_> = response.data[0]
            .hooks
            .iter()
            .filter(|hook| hook.source == VELATERM_SOURCE)
            .collect();
        assert_eq!(session_hooks.len(), 6, "VelaTerm injects six lifecycle hooks");
        assert!(session_hooks
            .iter()
            .all(|hook| hook.trust_status == "trusted" && hook.enabled));
    }

    #[test]
    #[ignore = "depends on the locally installed Codex app-server"]
    fn installed_codex_rejects_unknown_and_managed_updates() {
        let unknown = update(None, vec![], "no/such/hook:0:0", Some(false), None)
            .expect_err("an unknown key must be rejected");
        assert!(unknown.contains("Unknown Codex hook key"), "{unknown}");

        let managed = update(
            None,
            vec![],
            "/<session-flags>/config.toml:stop:0:0",
            Some(false),
            None,
        )
        .expect_err("a VelaTerm session hook must be rejected");
        assert!(managed.contains("managed"), "{managed}");
    }
}
