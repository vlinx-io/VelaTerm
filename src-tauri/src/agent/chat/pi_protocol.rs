//! The wire format between VelaTerm and Pi or OMP running in headless mode.
//!
//! Both CLIs are the same family: strict LF-delimited JSONL on stdin and stdout. A command is a JSON object
//! with an optional `id`; the answer is `{"type":"response","command":...,"success":...,"data":...}`, and
//! everything else on the stream is an event. The two diverge in a handful of places, captured by
//! [`PiVariant`]: the resume flag, the command catalogue request, and how a model describes its thinking
//! levels.
//!
//! Verified against pi 0.80.10 (`--mode rpc`, `--session`, `get_commands`) and omp 18.1.17 (`--mode rpc`,
//! `--resume`, `negotiate_protocol`, `get_available_commands`, `--approval-mode`).

use serde_json::{json, Value};

use crate::agent::chat::protocol::ChatImage;
use crate::models::SessionKind;

/// Which member of the Pi family a conversation is running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PiVariant {
    Pi,
    Omp,
}

impl PiVariant {
    /// The variant behind a session kind, or None for anything else.
    pub fn of(kind: SessionKind) -> Option<Self> {
        match kind {
            SessionKind::Pi => Some(PiVariant::Pi),
            SessionKind::Omp => Some(PiVariant::Omp),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PiVariant::Pi => "pi",
            PiVariant::Omp => "omp",
        }
    }

    /// The command that asks for the slash-command catalogue. OMP pushes it through
    /// `available_commands_update` as well, but the request is still valid and makes the handshake
    /// deterministic.
    pub fn commands_command(self) -> &'static str {
        match self {
            PiVariant::Pi => "get_commands",
            PiVariant::Omp => "get_available_commands",
        }
    }
}

// ─────────────────────────── Launch ───────────────────────────

/// Arguments that turn the CLI into a headless protocol peer, plus the optional native session to resume,
/// the model to start with, and — for OMP — forcing every tool through without prompting. With `fork`, the
/// session is copied into a new one through the native `--fork <id>` instead of being resumed in place.
pub fn launch_args(
    variant: PiVariant,
    resume: Option<&str>,
    fork: bool,
    model: Option<&str>,
    bypass_approvals: bool,
) -> Vec<String> {
    let mut args: Vec<String> = vec!["--mode".into(), "rpc".into()];
    // A leading `-` would make the value read as a flag of its own.
    if let Some(id) = resume.filter(|id| !id.trim().is_empty() && !id.trim_start().starts_with('-')) {
        match variant {
            _ if fork => args.push("--fork".into()),
            PiVariant::Pi => args.push("--session".into()),
            PiVariant::Omp => args.push("--resume".into()),
        }
        args.push(id.to_string());
    }
    if let Some(model) = model.filter(|model| !model.trim().is_empty()) {
        args.push("--model".into());
        args.push(model.to_string());
    }
    if variant == PiVariant::Omp && bypass_approvals {
        args.push("--approval-mode=yolo".into());
    }
    args
}

// ─────────────────────────── Commands ───────────────────────────

/// A user turn, with any images attached. Pi and OMP use the short image block on this wire.
pub fn prompt(id: &str, text: &str, images: &[ChatImage]) -> Value {
    let mut value = json!({"id": id, "type": "prompt", "message": text});
    if !images.is_empty() {
        value["images"] = images_value(images);
    }
    value
}

/// Write into the running turn without ending it.
pub fn steer(id: &str, text: &str, images: &[ChatImage]) -> Value {
    let mut value = json!({"id": id, "type": "steer", "message": text});
    if !images.is_empty() {
        value["images"] = images_value(images);
    }
    value
}

fn images_value(images: &[ChatImage]) -> Value {
    Value::Array(
        images
            .iter()
            .map(|image| {
                json!({"type":"image","data":image.data,"mimeType":image.mime_type})
            })
            .collect(),
    )
}

pub fn abort(id: &str) -> Value {
    json!({"id": id, "type": "abort"})
}

pub fn compact(id: &str) -> Value {
    json!({"id": id, "type": "compact"})
}

pub fn get_state(id: &str) -> Value {
    json!({"id": id, "type": "get_state"})
}

pub fn get_available_models(id: &str) -> Value {
    json!({"id": id, "type": "get_available_models"})
}

pub fn get_commands(id: &str, variant: PiVariant) -> Value {
    json!({"id": id, "type": variant.commands_command()})
}

pub fn set_model(id: &str, provider: &str, model_id: &str) -> Value {
    json!({"id": id, "type": "set_model", "provider": provider, "modelId": model_id})
}

pub fn set_thinking_level(id: &str, level: &str) -> Value {
    json!({"id": id, "type": "set_thinking_level", "level": level})
}

/// OMP negotiates its frame protocol on startup and answers the version it will speak.
pub fn negotiate(id: &str) -> Value {
    json!({"id": id, "type": "negotiate_protocol", "protocolVersion": 2})
}

// ─────────────────────────── Incoming ───────────────────────────

/// One decoded line. Unknown shapes are ignored rather than rejected so a newer CLI still works.
#[derive(Debug)]
pub enum PiIncoming {
    /// An answer to one of our commands.
    Response {
        command: String,
        success: bool,
        data: Value,
        error: Option<String>,
    },
    /// OMP's opening frame.
    Ready,
    /// A freshly pushed slash-command catalogue (OMP `available_commands_update`).
    Commands(Vec<Value>),
    /// An extension asking for user input, or a fire-and-forget status update.
    ExtensionUi(Value),
    /// Any other frame is a session event.
    Event(Value),
}

pub fn parse_line(line: &str) -> Option<PiIncoming> {
    let value: Value = serde_json::from_str(line).ok()?;
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "response" => Some(PiIncoming::Response {
            command: value.get("command").and_then(Value::as_str).unwrap_or("").to_string(),
            success: value.get("success").and_then(Value::as_bool).unwrap_or(false),
            data: value.get("data").cloned().unwrap_or(Value::Null),
            error: value.get("error").and_then(Value::as_str).map(str::to_string),
        }),
        "ready" => Some(PiIncoming::Ready),
        "available_commands_update" => Some(PiIncoming::Commands(
            value
                .get("commands")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        )),
        "extension_ui_request" => Some(PiIncoming::ExtensionUi(value)),
        _ => Some(PiIncoming::Event(value)),
    }
}

// ─────────────────────────── Models ───────────────────────────

/// Convert a model catalogue into the composer's `ChatModel` shape.
pub fn to_chat_models(variant: PiVariant, models: &[Value]) -> Vec<Value> {
    models
        .iter()
        .filter_map(|model| {
            let id = model.get("id").and_then(Value::as_str)?;
            if id.is_empty() {
                return None;
            }
            let label = model
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty())
                .unwrap_or(id);
            let provider = model
                .get("provider")
                .and_then(Value::as_str)
                .unwrap_or("");
            let context_window = model
                .get("contextWindow")
                .and_then(Value::as_u64)
                .filter(|window| *window > 0);
            Some(json!({
                "id": id,
                "label": label,
                "description": provider,
                "effortLevels": thinking_levels(variant, model),
                "contextWindow": context_window,
                "curated": true,
                "largeContext": false,
            }))
        })
        .collect()
}

/// The thinking levels a model offers. Pi lists an override map while OMP lists the efforts directly, so the
/// derivation branches on the variant; a model without reasoning has only `off`.
pub fn thinking_levels(variant: PiVariant, model: &Value) -> Vec<String> {
    if model.get("reasoning").and_then(Value::as_bool) != Some(true) {
        return vec!["off".to_string()];
    }
    let mut levels = vec!["off".to_string()];
    match variant {
        PiVariant::Omp => {
            if let Some(efforts) = model
                .pointer("/thinking/efforts")
                .and_then(Value::as_array)
            {
                for effort in efforts.iter().filter_map(Value::as_str) {
                    push_unique(&mut levels, effort);
                }
            }
            if levels.len() == 1 {
                for level in ["minimal", "low", "medium", "high"] {
                    push_unique(&mut levels, level);
                }
            }
        }
        PiVariant::Pi => {
            for level in ["minimal", "low", "medium", "high"] {
                push_unique(&mut levels, level);
            }
            if let Some(map) = model.get("thinkingLevelMap").and_then(Value::as_object) {
                for level in ["xhigh", "max"] {
                    if map.get(level).is_some_and(|value| !value.is_null()) {
                        push_unique(&mut levels, level);
                    }
                }
            }
        }
    }
    levels
}

fn push_unique(levels: &mut Vec<String>, level: &str) {
    if !levels.iter().any(|existing| existing == level) {
        levels.push(level.to_string());
    }
}

/// Resolve a stored model id to the provider and native id a `set_model` command needs, using the live
/// catalogue. A `provider/id` spelling is split directly when the catalogue cannot name it.
pub fn resolve_model(models: &[Value], model: &str) -> (String, String) {
    if let Some(found) = models.iter().find(|entry| {
        entry.get("id").and_then(Value::as_str) == Some(model)
    }) {
        let provider = found
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        return (provider, model.to_string());
    }
    match model.split_once('/') {
        Some((provider, id)) => (provider.to_string(), id.to_string()),
        None => (String::new(), model.to_string()),
    }
}

// ─────────────────────────── Extension UI ───────────────────────────

/// Methods that ask the person something and block until answered. The rest are fire-and-forget.
pub fn extension_is_dialog(method: &str) -> bool {
    matches!(method, "select" | "confirm" | "input" | "editor")
}

/// Build the permission-card payload for an extension dialog.
pub fn extension_payload(variant: PiVariant, request: &Value) -> Value {
    let id = request.get("id").and_then(Value::as_str).unwrap_or("");
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let title = request.get("title").and_then(Value::as_str).unwrap_or("");
    let mut input = json!({"prompt": title});
    match method {
        "select" => {
            if let Some(options) = request.get("options") {
                input["options"] = options.clone();
            }
        }
        "confirm" => {
            if let Some(message) = request.get("message") {
                input["message"] = message.clone();
            }
        }
        "input" | "editor" => {
            if let Some(placeholder) = request.get("placeholder") {
                input["placeholder"] = placeholder.clone();
            }
            if let Some(prefill) = request.get("prefill") {
                input["prefill"] = prefill.clone();
            }
        }
        _ => {}
    }
    json!({
        "id": id,
        "tool_name": "ToolApproval",
        "display_name": variant.as_str(),
        "input": input,
        "_piKind": method,
        "_piRequestId": id,
        "_piOptions": request.get("options").cloned().unwrap_or(Value::Null),
    })
}

/// The answer to an extension dialog. `select` returns the chosen option, `confirm` a boolean, and the text
/// methods a value. `allow` picks the affirmative answer; a deny cancels.
pub fn extension_response(request: &Value, allow: bool) -> Value {
    let id = request.get("id").and_then(Value::as_str).unwrap_or("");
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    match method {
        "select" => {
            let options: Vec<&str> = request
                .get("options")
                .and_then(Value::as_array)
                .map(|entries| entries.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let value = if allow {
                options.first().copied().unwrap_or("Approve")
            } else {
                options
                    .iter()
                    .find(|option| option.eq_ignore_ascii_case("deny"))
                    .copied()
                    .unwrap_or("Deny")
            };
            json!({"type":"extension_ui_response","id":id,"value":value})
        }
        "confirm" => json!({"type":"extension_ui_response","id":id,"confirmed":allow}),
        _ => json!({"type":"extension_ui_response","id":id,"cancelled":true}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pi_thinking_levels_follow_the_override_map() {
        let model = json!({
            "id": "gpt-5.6-sol",
            "reasoning": true,
            "thinkingLevelMap": {"xhigh":"xhigh","max":"max","minimal":"low"},
        });
        let levels = thinking_levels(PiVariant::Pi, &model);
        assert!(levels.contains(&"medium".to_string()));
        assert!(levels.contains(&"xhigh".to_string()));
        assert!(levels.contains(&"max".to_string()));
        assert_eq!(levels[0], "off");
    }

    #[test]
    fn omp_thinking_levels_follow_the_model_efforts() {
        let model = json!({
            "id": "anthropic.claude-opus-4-6-v1",
            "reasoning": true,
            "thinking": {"mode":"anthropic-adaptive","efforts":["low","medium","high","max"]},
        });
        assert_eq!(
            thinking_levels(PiVariant::Omp, &model),
            vec!["off", "low", "medium", "high", "max"]
        );
    }

    #[test]
    fn a_model_without_reasoning_has_only_off() {
        let model = json!({"id":"x","reasoning":false});
        assert_eq!(thinking_levels(PiVariant::Pi, &model), vec!["off"]);
        assert_eq!(thinking_levels(PiVariant::Omp, &model), vec!["off"]);
    }

    #[test]
    fn catalog_entries_carry_the_composer_shape() {
        let models = vec![json!({
            "id": "gpt-5.6-sol",
            "name": "GPT-5.6 Sol",
            "provider": "openai-codex",
            "reasoning": true,
            "contextWindow": 272000,
        })];
        let chat = to_chat_models(PiVariant::Pi, &models);
        assert_eq!(chat.len(), 1);
        assert_eq!(chat[0]["id"], "gpt-5.6-sol");
        assert_eq!(chat[0]["label"], "GPT-5.6 Sol");
        assert_eq!(chat[0]["description"], "openai-codex");
        assert_eq!(chat[0]["contextWindow"], 272000);
        assert_eq!(chat[0]["curated"], true);
    }

    #[test]
    fn resolve_model_prefers_the_live_catalog_then_splits() {
        let models = vec![json!({"id":"deepseek-v4-pro","provider":"deepseek"})];
        assert_eq!(
            resolve_model(&models, "deepseek-v4-pro"),
            ("deepseek".to_string(), "deepseek-v4-pro".to_string())
        );
        assert_eq!(
            resolve_model(&models, "openai/gpt-5.6"),
            ("openai".to_string(), "gpt-5.6".to_string())
        );
    }

    #[test]
    fn extension_dialogs_become_cards_and_replies() {
        let request = json!({
            "type":"extension_ui_request","id":"abc","method":"select",
            "title":"Allow tool: bash\nCommand: echo hi","options":["Approve","Deny"],
        });
        let payload = extension_payload(PiVariant::Omp, &request);
        assert_eq!(payload["_piKind"], "select");
        assert_eq!(payload["tool_name"], "ToolApproval");
        assert!(extension_is_dialog("select"));
        assert!(!extension_is_dialog("notify"));
        assert_eq!(extension_response(&request, true)["value"], "Approve");
        assert_eq!(extension_response(&request, false)["value"], "Deny");
        let confirm = json!({"id":"c","method":"confirm"});
        assert_eq!(extension_response(&confirm, true)["confirmed"], true);
        assert_eq!(extension_response(&confirm, false)["confirmed"], false);
    }

    #[test]
    fn parse_line_classifies_frames() {
        assert!(matches!(
            parse_line(r#"{"type":"ready","protocolVersion":1}"#),
            Some(PiIncoming::Ready)
        ));
        let response = parse_line(
            r#"{"id":"1","type":"response","command":"get_state","success":true,"data":{"sessionId":"s"}}"#,
        );
        match response {
            Some(PiIncoming::Response { command, success, data, .. }) => {
                assert_eq!(command, "get_state");
                assert!(success);
                assert_eq!(data["sessionId"], "s");
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(
            parse_line(r#"{"type":"agent_start"}"#),
            Some(PiIncoming::Event(_))
        ));
    }

    #[test]
    fn launch_args_resume_per_variant() {
        assert_eq!(
            launch_args(PiVariant::Pi, Some("abc"), false, Some("m"), false),
            vec!["--mode", "rpc", "--session", "abc", "--model", "m"]
        );
        assert_eq!(
            launch_args(PiVariant::Omp, Some("abc"), false, None, true),
            vec!["--mode", "rpc", "--resume", "abc", "--approval-mode=yolo"]
        );
        // A value with a leading dash would read as a flag of its own.
        assert_eq!(launch_args(PiVariant::Omp, Some("--yolo"), false, None, false), vec!["--mode", "rpc"]);
    }

    #[test]
    fn launch_args_fork_uses_the_native_fork_flag() {
        for variant in [PiVariant::Pi, PiVariant::Omp] {
            assert_eq!(launch_args(variant, Some("abc"), true, None, false), vec!["--mode", "rpc", "--fork", "abc"]);
        }
    }
}
