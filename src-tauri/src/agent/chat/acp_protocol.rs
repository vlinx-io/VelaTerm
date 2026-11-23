//! Agent Client Protocol (ACP): the wire format Copilot, Cursor, Kimi and Grok share, and its timeline.
//!
//! ACP is JSON-RPC 2.0 with one message per line on the agent's stdin and stdout. VelaTerm is the client:
//! it opens a session (`session/new`, or `session/load` to continue one), sends each turn as a
//! `session/prompt` request whose response ends the turn, and draws the `session/update` notifications
//! that stream in between. The agent can ask the client for permission (`session/request_permission`).
//! File system and terminal capabilities are not advertised, so the agent uses its own tools for both.
//!
//! **Why one mapper for live and replayed updates.** `session/load` replays a conversation as the same
//! notifications the live turn produced. Feeding both through `Mapper` gives a replay the same rows, with
//! the same ids, that the live session drew, so a resumed view replaces its rows without visible change.
//!
//! **History.** Grok writes its native `updates.jsonl` in exactly this notification format, so it is read
//! directly. Copilot, Cursor and Kimi keep theirs in private stores; for them the engine keeps a journal of
//! the notifications it received (plus the prompts it sent, written as `user_message_chunk`), which is what
//! a conversation shows before its agent is started again. Once started, `session/load` is authoritative:
//! it also covers turns taken in the terminal view, and the journal is rewritten from it.
//!
//! Verified against Copilot 1.0.63 (complete turns and `session/load`), and the initialize handshake of
//! Cursor 2026.07.01, Kimi 2.1.1 and Grok 1.0.46.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::engine::ChatRow;
use super::protocol::ChatImage;
use crate::models::SessionKind;

pub const PROTOCOL_VERSION: u64 = 1;

/// Reasoning levels Copilot's `--effort` accepts (Copilot CLI 1.0.63).
pub const COPILOT_EFFORTS: &[&str] = &["none", "low", "medium", "high", "xhigh", "max"];

/// The JSON-RPC error code ACP agents use for "sign in first".
const AUTH_REQUIRED_CODE: i64 = -32000;

/// How many tool calls the mapper remembers. A late update for an older call is dropped rather than
/// letting a long conversation grow the table without limit.
const TOOL_MEMORY: usize = 512;

/// Agents whose conversation view runs over ACP.
pub fn supported(kind: SessionKind) -> bool {
    kind.uses_acp()
}

/// The product name shown in permission cards and messages.
pub fn label(kind: SessionKind) -> &'static str {
    match kind {
        SessionKind::Antigravity => "Antigravity",
        SessionKind::Copilot => "Copilot",
        SessionKind::Cursor => "Cursor",
        SessionKind::Kimi => "Kimi",
        SessionKind::Grok => "Grok",
        _ => "Agent",
    }
}

/// Command-line arguments that start the agent's ACP server.
///
/// Only Copilot takes model and effort at launch: its ACP sessions publish no model selector, while the
/// others publish one and are switched over the protocol once the session is open. Copilot's options are
/// global, so a session's own launch arguments still apply; the others' arguments belong to their terminal
/// interface and would be misread by the server subcommand.
///
/// Antigravity is the exception that keeps safe arguments: `agy acp` accepts flags such as `--add-dir`
/// and `--sandbox`, while model, effort and the conversation itself are owned by the protocol.
pub fn launch_args(kind: SessionKind, model: Option<&str>, effort: Option<&str>, extra: &[String]) -> Result<Vec<String>, String> {
    let mut args: Vec<String> = match kind {
        SessionKind::Antigravity => vec!["acp".into()],
        SessionKind::Copilot => vec!["--acp".into()],
        SessionKind::Cursor => vec!["acp".into()],
        SessionKind::Kimi => vec!["acp".into()],
        SessionKind::Grok => vec!["agent".into(), "stdio".into()],
        other => return Err(format!("The chat engine does not support {} sessions", other.as_str())),
    };
    if kind == SessionKind::Copilot {
        for arg in extra {
            let flag = arg.split('=').next().unwrap_or(arg);
            if matches!(flag, "--acp" | "-p" | "--prompt" | "-i" | "--interactive" | "-r" | "--resume" | "--continue") {
                return Err(format!("Copilot conversation view manages {flag}; remove it from custom arguments"));
            }
        }
        if let Some(model) = model.filter(|model| !model.trim().is_empty()) {
            args.extend(["--model".into(), model.into()]);
        }
        if let Some(effort) = effort.filter(|effort| !effort.trim().is_empty()) {
            args.extend(["--effort".into(), effort.into()]);
        }
        args.extend_from_slice(extra);
    }
    if kind == SessionKind::Antigravity {
        let mut skip_value = false;
        for arg in extra {
            if skip_value { skip_value = false; continue; }
            let flag = arg.split('=').next().unwrap_or(arg);
            // Model, effort, mode, and skip flags are managed over the protocol or terminal launch, so
            // terminal-era arguments for them must not reach `agy acp`.
            if flag == "--dangerously-skip-permissions" {
                continue;
            }
            if matches!(flag, "--model" | "--effort" | "--mode") {
                skip_value = !arg.contains('=');
                continue;
            }
            if matches!(flag,
                "--input-format" | "--output-format" | "--conversation" | "--continue" | "-c" |
                "--print" | "-p" | "--prompt" | "--prompt-interactive" | "-i" | "--remote-control" |
                "--remote-agent" | "--project" | "--new-project" | "--login" | "--worker" | "-h" | "--help")
            {
                return Err(format!("Antigravity conversation view manages {flag}; remove it from custom arguments"));
            }
            args.push(arg.clone());
        }
    }
    Ok(args)
}

/// Whether a message can go out in this conversation.
///
/// ACP itself has no standard steer; Antigravity's server accepts another prompt while a turn runs and
/// delivers it at the next invocation, so only that kind advertises `steering_supported`.
pub fn validate(behavior: &str, image_count: usize, images_supported: bool, steering_supported: bool) -> Result<(), String> {
    if behavior == "steer" && !steering_supported {
        return Err("This agent does not support steering; queue the message instead".into());
    }
    if image_count > 0 && !images_supported {
        return Err("This agent does not accept images in the conversation view".into());
    }
    Ok(())
}

// ─────────────────────────── Requests ───────────────────────────

pub fn request(id: &str, method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
}

pub fn initialize(id: &str) -> Value {
    request(id, "initialize", json!({
        "protocolVersion": PROTOCOL_VERSION,
        "clientCapabilities": {
            "fs": {"readTextFile": false, "writeTextFile": false},
            "terminal": false,
            "elicitation": {"form": {}},
        },
        "clientInfo": {"name": "velaterm", "title": "VelaTerm", "version": env!("CARGO_PKG_VERSION")},
    }))
}

pub fn session_new(id: &str, cwd: &str) -> Value {
    request(id, "session/new", json!({"cwd": cwd, "mcpServers": []}))
}

pub fn session_load(id: &str, session: &str, cwd: &str) -> Value {
    request(id, "session/load", json!({"sessionId": session, "cwd": cwd, "mcpServers": []}))
}

/// Reopen a conversation without replaying it, for agents that offer `session/resume` but not `session/load`.
pub fn session_resume(id: &str, session: &str, cwd: &str) -> Value {
    request(id, "session/resume", json!({"sessionId": session, "cwd": cwd, "mcpServers": []}))
}

/// List conversations to probe whether the server is authenticated without creating a session.
pub fn session_list(id: &str) -> Value {
    request(id, "session/list", json!({}))
}

/// Clear the ACP agent's saved credentials when the server advertises `agentCapabilities.auth.logout`.
pub fn logout(id: &str) -> Value {
    request(id, "logout", json!({}))
}

/// A turn: the text block first, then any images.
pub fn prompt(id: &str, session: &str, text: &str, images: &[super::protocol::ChatImage]) -> Value {
    let mut blocks = Vec::new();
    if !text.is_empty() || images.is_empty() {
        blocks.push(json!({"type":"text","text":text}));
    }
    for image in images {
        blocks.push(json!({"type":"image","mimeType":image.mime_type,"data":image.data}));
    }
    request(id, "session/prompt", json!({"sessionId": session, "prompt": blocks}))
}

/// `session/cancel` is a notification: the running prompt answers with `stopReason: cancelled`.
pub fn cancel(session: &str) -> Value {
    json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":session}})
}

pub fn set_mode(id: &str, session: &str, mode: &str) -> Value {
    request(id, "session/set_mode", json!({"sessionId": session, "modeId": mode}))
}

pub fn set_model(id: &str, session: &str, model: &str) -> Value {
    request(id, "session/set_model", json!({"sessionId": session, "modelId": model}))
}

pub fn set_config_option(id: &str, session: &str, option: &str, value: &str) -> Value {
    request(id, "session/set_config_option", json!({"sessionId": session, "configId": option, "value": value}))
}

pub fn response(id: &Value, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}

pub fn error_response(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

// ─────────────────────────── Incoming ───────────────────────────

/// One line from the agent, sorted by what the engine has to do with it.
#[derive(Debug, PartialEq)]
pub enum Incoming {
    /// The answer to one of our requests.
    Response { id: String, result: Option<Value>, error: Option<Value> },
    /// A request the agent sent us; `id` is kept as sent so the answer echoes it exactly.
    Request { id: Value, method: String, params: Value },
    Notification { method: String, params: Value },
}

pub fn parse_line(line: &str) -> Option<Incoming> {
    let value: Value = serde_json::from_str(line).ok()?;
    let method = value.get("method").and_then(Value::as_str).map(str::to_string);
    match (method, value.get("id")) {
        (Some(method), Some(id)) if !id.is_null() => Some(Incoming::Request {
            id: id.clone(), method, params: value.get("params").cloned().unwrap_or(Value::Null),
        }),
        (Some(method), _) => Some(Incoming::Notification { method, params: value.get("params").cloned().unwrap_or(Value::Null) }),
        (None, Some(id)) => Some(Incoming::Response {
            id: id_key(id), result: value.get("result").cloned(), error: value.get("error").cloned(),
        }),
        (None, None) => None,
    }
}

/// Stable map key for a JSON-RPC id, which may be a string or a number.
pub fn id_key(id: &Value) -> String {
    id.as_str().map(str::to_string).unwrap_or_else(|| id.to_string())
}

/// Whether an error answer means the agent wants its user to sign in first.
pub fn auth_required(error: &Value) -> bool {
    let code = error.get("code").and_then(Value::as_i64);
    let message = error.get("message").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
    code == Some(AUTH_REQUIRED_CODE) && (message.contains("auth") || message.contains("login") || message.contains("sign in"))
}

/// Whether an error says the agent has no API key to call its model with.
pub fn api_key_missing(error: &Value) -> bool {
    error_text(error).to_ascii_lowercase().contains("no api key")
}

/// Whether an error message means Antigravity's in-memory model catalogue has not finished loading
/// right after `agy acp` booted (e.g., `FetchAvailableModels` is still in flight).
pub fn antigravity_model_not_ready_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if !lower.contains("failed to construct executor") {
        return false;
    }
    lower.contains("unknown model key")
        || lower.contains("plan model not specified")
        || (lower.contains("model not found") && lower.contains("model_placeholder_"))
}

/// Whether a JSON-RPC prompt error means Antigravity's in-memory model catalogue has not finished
/// loading right after `agy acp` booted.
pub fn antigravity_model_not_ready(error: &Value) -> bool {
    antigravity_model_not_ready_text(&error_text(error))
}

/// Whether a cold-start Antigravity error message failed specifically because the new session has
/// no active plan model assigned yet.
pub fn antigravity_plan_model_missing_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("failed to construct executor") && lower.contains("plan model not specified")
}

/// Whether a cold-start Antigravity prompt failed specifically because the new session has no
/// active plan model assigned yet.
pub fn antigravity_plan_model_missing(error: &Value) -> bool {
    antigravity_plan_model_missing_text(&error_text(error))
}

/// Whether an ACP JSON-RPC error reports an Antigravity Google OAuth 401 credential failure.
pub fn antigravity_401_error(error: &Value) -> bool {
    super::antigravity_protocol::is_oauth_401_error(&error_text(error))
}

/// Whether an ACP JSON-RPC error reports a transient Antigravity upstream API or transport error.
pub fn antigravity_transient_error(error: &Value) -> bool {
    super::antigravity_protocol::is_transient_upstream_error(&error_text(error))
}

/// Plain text carried by a `session/update` chunk notification, if any.
pub fn update_text(update: &Value) -> Option<String> {
    content_text(update.get("content"))
}

/// Extract the underlying error message when `agy acp` delivers a cold-start executor error as a
/// warning-prefixed `agent_message_chunk` (`\n⚠ failed to construct executor: ...\n`) instead of a
/// JSON-RPC error response.
pub fn antigravity_cold_model_chunk_error(text: &str) -> Option<String> {
    let body = text.trim().strip_prefix('⚠')?.trim();
    let mut lines = body.lines().map(str::trim).filter(|line| !line.is_empty());
    let first = lines.next()?;
    if !first.to_ascii_lowercase().starts_with("failed to construct executor") {
        return None;
    }
    if !antigravity_model_not_ready_text(first) {
        return None;
    }
    if lines.any(|line| !line.to_ascii_lowercase().starts_with("error id:")) {
        return None;
    }
    Some(first.to_string())
}

/// The readable part of a JSON-RPC error, preferring the detail an agent puts in `data`.
pub fn error_text(error: &Value) -> String {
    let data = error.get("data");
    data.and_then(|data| data.get("message")).and_then(Value::as_str)
        .or_else(|| data.and_then(Value::as_str))
        .or_else(|| error.get("message").and_then(Value::as_str))
        .unwrap_or("The agent reported an error")
        .to_string()
}

/// What a `stopReason` means for the turn: `None` for a normal end, otherwise the message to show.
pub fn stop_reason_error(reason: &str) -> Option<&'static str> {
    match reason {
        "end_turn" | "cancelled" | "" => None,
        "max_tokens" => Some("The agent stopped because the response reached its token limit."),
        "max_turn_requests" => Some("The agent stopped because the turn reached its request limit."),
        "refusal" => Some("The agent declined to continue this turn."),
        _ => Some("The agent ended the turn unexpectedly."),
    }
}

/// The model name a reply is credited to. ACP agents already report it as a name.
pub fn reply_model(_kind: SessionKind, value: &str) -> String {
    value.to_string()
}

// ─────────────────────────── Session catalogue ───────────────────────────

/// Modes, models and reasoning levels an ACP session offers, as the session reported them.
///
/// Agents describe these in two ways: the older `modes` / `models` states, switched with `session/set_mode`
/// and `session/set_model`, and the newer `configOptions`, switched with `session/set_config_option`. A
/// config option, when present for a category, is what the agent itself treats as current.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub modes: Vec<Choice>,
    pub mode: Option<String>,
    pub mode_option: Option<String>,
    pub models: Vec<Choice>,
    pub model: Option<String>,
    pub model_option: Option<String>,
    /// Whether the models came from the `models` state, which `session/set_model` switches.
    pub models_from_state: bool,
    /// Raw `models.availableModels` reported on session open, kept even when `configOptions` replaces `models`.
    pub state_models: Vec<Choice>,
    /// Reasoning levels each model states for itself, by model id. Grok declares a different ladder per
    /// model under `_meta.reasoningEfforts`; the flat `efforts` below can only name the active one's.
    /// Entries filled from a live `thought_level` option are the same idea from the other source.
    pub model_efforts: HashMap<String, Vec<String>>,
    pub efforts: Vec<Choice>,
    pub effort: Option<String>,
    pub effort_option: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

impl Catalog {
    /// Read the catalogue from a `session/new` or `session/load` result.
    pub fn absorb_session(&mut self, result: &Value) {
        if let Some(modes) = result.get("modes").filter(|modes| modes.is_object()) {
            self.modes = modes.get("availableModes").and_then(Value::as_array).into_iter().flatten()
                .filter_map(|mode| choice(mode, "id")).collect();
            self.mode = modes.get("currentModeId").and_then(Value::as_str).map(str::to_string);
        }
        if let Some(models) = result.get("models").filter(|models| models.is_object()) {
            let available = models.get("availableModels").and_then(Value::as_array);
            let listed: Vec<Choice> = distinct(available.into_iter().flatten()
                .filter_map(|model| choice(model, "modelId")).collect());
            if !listed.is_empty() {
                self.state_models = listed.clone();
                self.models = listed;
                self.models_from_state = true;
            }
            // Keep each model's own ladder, so the picker offers the levels of the model actually chosen
            // instead of one borrowed from whatever the session started with.
            for model in available.into_iter().flatten() {
                let Some(id) = model.get("modelId").and_then(Value::as_str) else { continue };
                let levels = distinct_levels(model.pointer("/_meta/reasoningEfforts").and_then(Value::as_array)
                    .into_iter().flatten()
                    .filter_map(|level| level.get("id").or_else(|| level.get("value")).and_then(Value::as_str))
                    .map(str::to_string).collect());
                if !levels.is_empty() { self.model_efforts.insert(id.to_string(), levels); }
            }
            self.model = models.get("currentModelId").and_then(Value::as_str).map(str::to_string).or(self.model.take());
        }
        if let Some(options) = result.get("configOptions").and_then(Value::as_array) {
            self.absorb_options(options);
        }
    }

    /// Fold in a `config_option_update`, which always carries the complete set.
    pub fn absorb_options(&mut self, options: &[Value]) -> bool {
        // A model without thinking support omits `thought_level`, so stale levels from the previous
        // model must be cleared rather than left active.
        self.effort_option = None;
        self.efforts.clear();
        self.effort = None;
        let (mut saw_mode, mut saw_model, mut saw_effort) = (false, false, false);
        for option in options {
            if option.get("type").and_then(Value::as_str) != Some("select") { continue; }
            let Some(id) = option.get("id").and_then(Value::as_str) else { continue };
            let choices = distinct(flatten_options(option.get("options")));
            let current = option.get("currentValue").and_then(Value::as_str).map(str::to_string);
            match option.get("category").and_then(Value::as_str) {
                Some("mode") => {
                    self.mode_option = Some(id.into());
                    if !choices.is_empty() { self.modes = choices; }
                    if current.is_some() {
                        saw_mode = true;
                        self.mode = current;
                    }
                }
                Some("model") => {
                    saw_model = true;
                    self.model_option = Some(id.into());
                    if !choices.is_empty() { self.models = choices; self.models_from_state = false; }
                    if current.is_some() { self.model = current; }
                }
                Some("thought_level") => {
                    saw_effort = true;
                    self.effort_option = Some(id.into());
                    self.efforts = choices;
                    self.effort = current;
                }
                _ => {}
            }
        }
        if saw_model || saw_effort {
            if let Some(model) = &self.model {
                let levels: Vec<String> = self.efforts.iter().map(|c| c.id.clone()).collect();
                // A ladder the model stated under `_meta` survives an option that names no levels: that
                // means this model offers none to switch here, not that its ladder is unknown.
                if !levels.is_empty() || !self.model_efforts.contains_key(model) {
                    self.model_efforts.insert(model.clone(), levels);
                }
            }
        }
        saw_mode
    }

    /// Whether the running session can change its model without a restart.
    ///
    /// A selector offering fewer than two models is not a choice. Copilot 1.0.63 reports one after
    /// `session/load` that lists only `auto` (twice) whatever model it was launched with; treating it as a
    /// selector would replace the launched model with that placeholder.
    pub fn model_switchable(&self) -> bool {
        (self.model_option.is_some() || self.models_from_state) && self.models.len() >= 2
    }

    pub fn effort_switchable(&self) -> bool {
        self.effort_option.is_some()
    }
}

/// Keep the first of each id: agents may list one model twice, as Copilot does with `auto`.
fn distinct(choices: Vec<Choice>) -> Vec<Choice> {
    let mut out: Vec<Choice> = Vec::new();
    for choice in choices {
        if !out.iter().any(|seen| seen.id == choice.id) { out.push(choice); }
    }
    out
}

/// Keep the first of each reasoning level, in the order the model listed them.
fn distinct_levels(levels: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for level in levels {
        if !out.contains(&level) { out.push(level); }
    }
    out
}

fn choice(value: &Value, id_key: &str) -> Option<Choice> {
    let id = value.get(id_key).and_then(Value::as_str).filter(|id| !id.is_empty())?;
    Some(Choice {
        id: id.into(),
        name: value.get("name").and_then(Value::as_str).filter(|name| !name.is_empty()).unwrap_or(id).into(),
        description: value.get("description").and_then(Value::as_str).filter(|text| !text.is_empty()).map(str::to_string),
    })
}

/// A select option's choices, with grouped options flattened in order.
fn flatten_options(options: Option<&Value>) -> Vec<Choice> {
    let mut out = Vec::new();
    for option in options.and_then(Value::as_array).into_iter().flatten() {
        if let Some(group) = option.get("options").and_then(Value::as_array) {
            out.extend(group.iter().filter_map(|value| choice(value, "value")));
        } else if let Some(value) = choice(option, "value") {
            out.push(value);
        }
    }
    out
}

/// Commands from `available_commands_update`, in the shape the composer completes.
pub fn commands(update: &Value) -> Vec<Value> {
    update.get("availableCommands").and_then(Value::as_array).into_iter().flatten().filter_map(|command| {
        let name = command.get("name").and_then(Value::as_str).filter(|name| !name.is_empty())?;
        Some(json!({
            "name": name,
            "description": command.get("description").cloned().unwrap_or(Value::Null),
            "argumentHint": command.pointer("/input/hint").cloned().unwrap_or(Value::Null),
            "invocation": "/",
        }))
    }).collect()
}

// ─────────────────────────── Permissions ───────────────────────────

/// A `session/request_permission` as the view's permission card.
///
/// The tool call it asks about is drawn with the same mapping as the timeline, so the card reads like the
/// tool card that follows it. An `allow_always` option becomes the card's one standing rule.
///
/// A request may name only the call, so what the mapper already knows about that call is the base the
/// request's own fields are laid over.
pub fn permission_payload(kind: SessionKind, request_id: &str, params: &Value, mapper: Option<&Mapper>) -> Value {
    let call = params.get("toolCall").cloned().unwrap_or(Value::Null);
    let known = call.get("toolCallId").and_then(Value::as_str)
        .and_then(|id| mapper.and_then(|mapper| mapper.tools.get(id)).cloned());
    let mut tool = known.unwrap_or_default();
    tool.merge(&call);
    let (name, input) = tool.card();
    let options = params.get("options").cloned().unwrap_or_else(|| json!([]));
    let mut payload = json!({
        "id": request_id,
        "tool_name": name,
        "display_name": label(kind),
        "input": input,
        "_acpOptions": options,
    });
    if let Some(title) = tool.title.as_deref().filter(|title| !title.is_empty()) {
        payload["description"] = json!(title);
    }
    if let Some(option) = option_of(params.get("options"), &["allow_always"]) {
        payload["permission_suggestions"] = json!([{
            "type": "acpAllowAlways",
            "optionId": option,
            "subject": tool.title.clone().unwrap_or_else(|| name.clone()),
        }]);
    }
    payload
}

/// The option an answer selects: the standing rule when one was taken, otherwise a one-time choice.
///
/// Falls back to the persistent variant when an agent offers only that, so "allow" is never silently
/// turned into "deny" because one spelling is missing.
pub fn choose_option(options: Option<&Value>, allow: bool, always: Option<&str>) -> Option<String> {
    if allow {
        if let Some(always) = always {
            let offered = options.and_then(Value::as_array).into_iter().flatten()
                .any(|option| option.get("optionId").and_then(Value::as_str) == Some(always));
            if offered { return Some(always.to_string()); }
        }
        option_of(options, &["allow_once", "allow_always"])
    } else {
        option_of(options, &["reject_once", "reject_always"])
    }
}

fn option_of(options: Option<&Value>, kinds: &[&str]) -> Option<String> {
    let options = options.and_then(Value::as_array)?;
    kinds.iter().find_map(|kind| options.iter().find(|option| option.get("kind").and_then(Value::as_str) == Some(kind)))
        .and_then(|option| option.get("optionId").and_then(Value::as_str)).map(str::to_string)
}

pub fn permission_outcome(id: &Value, option: Option<&str>) -> Value {
    response(id, match option {
        Some(option) => json!({"outcome":{"outcome":"selected","optionId":option}}),
        None => json!({"outcome":{"outcome":"cancelled"}}),
    })
}

/// Whether the agent reports that the conversation it was asked to reopen is gone — its directory, its
/// recording, or the session itself no longer exists there.
///
/// Deliberately narrow: a protocol error, a crashed server, or a missing binary all mean something else,
/// and treating those as "gone" would silently replace a conversation the agent still has.
pub fn conversation_missing(error: &Value) -> bool {
    let code = error.pointer("/data/code").and_then(Value::as_str).unwrap_or_default().to_ascii_uppercase();
    if ["NOT_FOUND", "ENOENT", "NO_SUCH"].iter().any(|token| code.contains(token)) { return true; }
    let text = error_text(error).to_ascii_lowercase();
    ["path not found", "no such file", "session not found", "conversation not found",
        "session does not exist", "unknown session", "session is not known"]
        .iter().any(|token| text.contains(token))
}

// Grok asks the client for its dialogs through `_x.ai/...` requests, which its own terminal draws and
// answers in a shape of its own. Each request is kept with the method that sent it, so the answer can
// take that method's form.

/// The plan Grok wants approved before it leaves plan mode.
pub const EXIT_PLAN_MODE: &str = "_x.ai/exit_plan_mode";
/// Questions the model asked through its `ask_user_question` tool.
pub const ASK_USER_QUESTION: &str = "_x.ai/ask_user_question";
/// An MCP server asking for input while one of its tools runs.
pub const MCP_ELICIT: &str = "_x.ai/mcp/elicit";

/// The card one of Grok's dialog requests shows, or `None` when the method is not one of them.
pub fn grok_dialog(kind: SessionKind, card: &str, method: &str, params: &Value) -> Option<Value> {
    match method {
        EXIT_PLAN_MODE => Some(json!({
            "id": card,
            "tool_name": "ExitPlanMode",
            "display_name": label(kind),
            "input": {"plan": params.get("planContent").and_then(Value::as_str).unwrap_or("")},
        })),
        ASK_USER_QUESTION => Some(json!({
            "id": card,
            "tool_name": "AskUserQuestion",
            "display_name": label(kind),
            "input": {"questions": grok_questions(params)},
        })),
        MCP_ELICIT => Some(elicitation_payload(kind, card, params)),
        _ => None,
    }
}

/// Grok's questions in the shape the question form reads: a header per question and an options array,
/// both of which Grok omits for a plain free-text question.
fn grok_questions(params: &Value) -> Vec<Value> {
    params.get("questions").and_then(Value::as_array).into_iter().flatten().enumerate()
        .map(|(index, question)| {
            let mut question = question.clone();
            if question.get("header").and_then(Value::as_str).is_none_or(str::is_empty) {
                question["header"] = json!(format!("Q{}", index + 1));
            }
            if !question.get("options").is_some_and(Value::is_array) {
                question["options"] = json!([]);
            }
            question
        })
        .collect()
}

/// The answer to one of Grok's dialogs, in that method's own shape.
///
/// A plan is `approved` (Grok leaves plan mode by itself) or `rejected`. Questions are `accepted` with
/// the answers keyed by question text, or `skip_interview` when the person dismissed them. An MCP form is
/// `accept` with its content, `decline`, or `cancel`.
pub fn grok_dialog_answer(
    id: &Value,
    method: &str,
    allow: bool,
    updated_input: Option<&Value>,
    message: Option<&str>,
) -> Value {
    response(id, match method {
        EXIT_PLAN_MODE => json!({"outcome": if allow { "approved" } else { "rejected" }}),
        ASK_USER_QUESTION if allow => json!({"outcome": "accepted",
            "answers": updated_input.and_then(|input| input.get("answers")).cloned().unwrap_or_else(|| json!({}))}),
        ASK_USER_QUESTION => json!({"outcome": "skip_interview"}),
        _ if !allow => json!({"outcome": if message == Some("cancel") { "cancel" } else { "decline" }}),
        // A URL elicitation asks the person to finish something in the browser and say so; there is no
        // content to send back, only the acknowledgement.
        _ if updated_input.is_none() => json!({"outcome": "accept"}),
        _ => json!({"outcome": "accept", "content": updated_input.cloned().unwrap_or_else(|| json!({}))}),
    })
}

/// The answer to one of Grok's dialogs left open when its turn ended: nothing approved or answered.
pub fn grok_cancelled_outcome(id: &Value, method: &str) -> Value {
    response(id, match method {
        EXIT_PLAN_MODE => json!({"outcome": "rejected"}),
        ASK_USER_QUESTION => json!({"outcome": "skip_interview"}),
        _ => json!({"outcome": "cancel"}),
    })
}

/// Convert an ACP `elicitation/create` request into a VelaTerm interactive card.
///
/// Antigravity sends `ask_question` prompts as form elicitations whose schema uses `q1`, `q1_other`,
/// ... fields. Converting that shape into `AskUserQuestion` reuses the dedicated question card; any
/// other schema or URL elicitation falls back to the shared MCP `elicitation` form card.
pub fn elicitation_payload(kind: SessionKind, request_id: &str, params: &Value) -> Value {
    let mode = match params.get("mode").and_then(Value::as_str) {
        Some("url") => "url",
        _ => "form",
    };
    if mode != "url" {
        if let Some(questions) = parse_ask_question_schema(params) {
            let message = params.get("message").and_then(Value::as_str).unwrap_or("");
            let mut input = json!({
                "questions": questions,
                "_acpAskQuestion": true,
            });
            if !message.is_empty() {
                input["prompt"] = json!(message);
            }
            return json!({
                "id": request_id,
                "tool_name": "AskUserQuestion",
                "display_name": label(kind),
                "description": if message.is_empty() { Value::Null } else { json!(message) },
                "input": input,
            });
        }
    }
    let server = params.get("serverName").or_else(|| params.get("server")).cloned().unwrap_or(Value::Null);
    json!({
        "id": request_id,
        "subtype": "elicitation",
        "tool_name": "MCP",
        "display_name": label(kind),
        "mcp_server_name": server,
        "message": params.get("message").cloned().unwrap_or(Value::Null),
        "mode": mode,
        "url": params.get("url").cloned().unwrap_or(Value::Null),
        "requested_schema": params.get("requestedSchema").cloned().unwrap_or(Value::Null),
        "input": {
            "server": server,
            "mode": params.get("mode").cloned().unwrap_or(Value::Null),
        },
    })
}

/// Build the JSON-RPC response for an ACP `elicitation/create` request.
pub fn elicitation_outcome(
    id: &Value,
    request_card: &Value,
    params: &Value,
    allow: bool,
    updated_input: Option<&Value>,
    message: Option<&str>,
) -> Value {
    if !allow {
        let action = if message == Some("cancel") { "cancel" } else { "decline" };
        return response(id, json!({"action": action}));
    }
    let is_url = params.get("mode").and_then(Value::as_str) == Some("url");
    if is_url {
        return response(id, json!({"action": "accept"}));
    }
    let is_ask = request_card.pointer("/input/_acpAskQuestion").and_then(Value::as_bool) == Some(true);
    let content = if is_ask {
        build_ask_question_content(request_card, params, updated_input)
    } else {
        updated_input.cloned().unwrap_or_else(|| json!({}))
    };
    response(id, json!({
        "action": "accept",
        "content": content,
    }))
}

fn parse_ask_question_schema(params: &Value) -> Option<Vec<Value>> {
    let schema = params.get("requestedSchema")?.as_object()?;
    let props = schema.get("properties")?.as_object()?;
    if props.is_empty() {
        return None;
    }
    let mut grouped: std::collections::BTreeMap<usize, (bool, bool)> = std::collections::BTreeMap::new();
    for key in props.keys() {
        let Some(rest) = key.strip_prefix('q') else { return None; };
        if let Some(base_num) = rest.strip_suffix("_other") {
            if !base_num.is_empty() && base_num.chars().all(|c| c.is_ascii_digit()) {
                let num = base_num.parse::<usize>().ok()?;
                grouped.entry(num).or_insert((false, false)).1 = true;
                continue;
            }
            return None;
        }
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
            let num = rest.parse::<usize>().ok()?;
            grouped.entry(num).or_insert((false, false)).0 = true;
            continue;
        }
        return None;
    }
    if grouped.is_empty() {
        return None;
    }
    let mut keys: Vec<(usize, String, bool, bool)> = grouped.into_iter().map(|(num, (has_main, has_other))| {
        let key = if has_main { format!("q{num}") } else { format!("q{num}_other") };
        (num, key, has_other, !has_main && has_other)
    }).collect();
    if let Some(order) = schema.get("propertyOrdering").or_else(|| schema.get("propertyOrder")).and_then(Value::as_array) {
        let positions: std::collections::HashMap<&str, usize> = order.iter().enumerate()
            .filter_map(|(idx, val)| val.as_str().map(|s| (s, idx)))
            .collect();
        keys.sort_by_key(|(num, key, _, _)| (positions.get(key.as_str()).copied().unwrap_or(usize::MAX), *num));
    } else {
        keys.sort_by_key(|(num, _, _, _)| *num);
    }

    let fallback_msg = params.get("message").and_then(Value::as_str).unwrap_or("");
    let questions = keys.into_iter().map(|(num, key, has_other, free_text_only)| {
        let prop = props.get(&key).and_then(Value::as_object);
        let multi = !free_text_only && prop.and_then(|p| p.get("type")).and_then(Value::as_str) == Some("array");
        let title = prop.and_then(|p| p.get("title").or_else(|| p.get("description")))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or(if !fallback_msg.is_empty() { fallback_msg } else { key.as_str() });
        let other_key = if free_text_only { Some(key.clone()) } else { has_other.then(|| format!("{key}_other")) };
        let choices_source = if free_text_only {
            None
        } else if multi {
            prop.and_then(|p| p.get("items"))
        } else {
            props.get(&key)
        };
        let raw_options = extract_schema_options(choices_source);
        let options: Vec<Value> = raw_options.iter().map(|(val, label, desc)| {
            let mut item = json!({"label": label});
            if let Some(desc) = desc {
                item["description"] = json!(desc);
            }
            let _ = val;
            item
        }).collect();
        let option_values: Vec<String> = raw_options.into_iter().map(|(val, _, _)| val).collect();
        let mut q = json!({
            "header": format!("Q{num}"),
            "question": title,
            "multiSelect": multi,
            "allowOther": has_other || options.is_empty(),
            "options": options,
            "_key": key,
            "_optionValues": option_values,
        });
        if let Some(ok) = other_key {
            q["_otherKey"] = json!(ok);
        }
        if free_text_only {
            q["_freeTextOnly"] = json!(true);
        }
        q
    }).collect();
    Some(questions)
}

fn extract_schema_options(schema: Option<&Value>) -> Vec<(String, String, Option<String>)> {
    let Some(obj) = schema.and_then(Value::as_object) else { return Vec::new(); };
    if let Some(arr) = obj.get("oneOf").or_else(|| obj.get("anyOf")).and_then(Value::as_array) {
        let mut out = Vec::new();
        for entry in arr {
            let Some(entry_obj) = entry.as_object() else { continue; };
            let Some(val) = entry_obj.get("const").and_then(scalar_to_string) else { continue; };
            let label = entry_obj.get("title").and_then(Value::as_str).filter(|s| !s.is_empty())
                .unwrap_or(&val).to_string();
            let desc = entry_obj.get("description").and_then(Value::as_str)
                .filter(|s| !s.is_empty()).map(str::to_string);
            out.push((val, label, desc));
        }
        return out;
    }
    if let Some(arr) = obj.get("enum").and_then(Value::as_array) {
        let names = obj.get("enumNames").and_then(Value::as_array);
        return arr.iter().enumerate().filter_map(|(idx, v)| {
            let val = scalar_to_string(v)?;
            let label = names.and_then(|n| n.get(idx)).and_then(Value::as_str)
                .filter(|s| !s.is_empty()).unwrap_or(&val).to_string();
            Some((val, label, None))
        }).collect();
    }
    Vec::new()
}

fn scalar_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn build_ask_question_content(request_card: &Value, params: &Value, updated_input: Option<&Value>) -> Value {
    let mut content = serde_json::Map::new();
    let questions = request_card.pointer("/input/questions").and_then(Value::as_array)
        .or_else(|| updated_input.and_then(|u| u.get("questions")).and_then(Value::as_array));
    let Some(questions) = questions else { return Value::Object(content); };
    let answers = updated_input.and_then(|u| u.get("answers")).and_then(Value::as_object);
    let answer_list = updated_input.and_then(|u| u.get("_answerList")).and_then(Value::as_array);
    let selected_indices = updated_input.and_then(|u| u.get("_selectedIndices")).and_then(Value::as_object);
    let other_texts = updated_input.and_then(|u| u.get("_otherTexts")).and_then(Value::as_object);
    let skipped_indices: std::collections::HashSet<usize> = updated_input
        .and_then(|u| u.get("_skippedIndices"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .map(|n| n as usize)
        .collect();
    let has_structured = selected_indices.is_some() || other_texts.is_some() || !skipped_indices.is_empty();
    let required: std::collections::HashSet<&str> = params.pointer("/requestedSchema/required")
        .and_then(Value::as_array).into_iter().flatten()
        .filter_map(Value::as_str).collect();

    for (idx, q) in questions.iter().enumerate() {
        let Some(key) = q.get("_key").and_then(Value::as_str) else { continue; };
        let other_key = q.get("_otherKey").and_then(Value::as_str);
        let free_text_only = q.get("_freeTextOnly") == Some(&Value::Bool(true));
        let multi = !free_text_only && q.get("multiSelect").and_then(Value::as_bool) == Some(true);
        let question_text = q.get("question").and_then(Value::as_str).unwrap_or("");
        let opt_values: Vec<String> = q.get("_optionValues").and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();
        let opt_labels: Vec<String> = q.get("options").and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(|o| o.get("label").and_then(Value::as_str).map(str::to_string)).collect())
            .unwrap_or_default();

        if skipped_indices.contains(&idx) {
            if multi && required.contains(key) {
                content.insert(key.to_string(), json!(Vec::<String>::new()));
            }
            continue;
        }

        let idx_key = idx.to_string();
        if has_structured {
            let picked = selected_indices.and_then(|m| m.get(&idx_key).or_else(|| m.get(key))).and_then(Value::as_array);
            let custom = other_texts.and_then(|m| m.get(&idx_key).or_else(|| m.get(key)))
                .and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
            if picked.is_some() || custom.is_some() {
                let vals: Vec<String> = picked.into_iter().flatten().filter_map(Value::as_u64).filter_map(|i| {
                    let i = i as usize;
                    opt_values.get(i).cloned().or_else(|| Some((i + 1).to_string()))
                }).collect();
                if free_text_only {
                    if let Some(text) = custom {
                        content.insert(key.to_string(), json!(text));
                    }
                } else if multi {
                    if !vals.is_empty() || required.contains(key) {
                        content.insert(key.to_string(), json!(vals));
                    }
                    if let Some(text) = custom {
                        if let Some(ok) = other_key {
                            content.insert(ok.to_string(), json!(text));
                        } else if vals.is_empty() {
                            content.insert(key.to_string(), json!([text]));
                        }
                    }
                } else if let Some(first) = vals.first() {
                    content.insert(key.to_string(), json!(first));
                } else if let Some(text) = custom {
                    content.insert(other_key.unwrap_or(key).to_string(), json!(text));
                }
                continue;
            }
        }

        let Some(raw_ans) = answer_list.and_then(|arr| arr.get(idx)).and_then(Value::as_str)
            .or_else(|| answers.and_then(|a| a.get(question_text)).and_then(Value::as_str))
            .map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        if free_text_only {
            content.insert(key.to_string(), json!(raw_ans));
        } else if multi {
            let (vals, custom) = match_multi_answer(raw_ans, &opt_labels, &opt_values);
            if !vals.is_empty() || required.contains(key) {
                content.insert(key.to_string(), json!(vals));
            }
            if let Some(text) = custom {
                if let Some(ok) = other_key {
                    content.insert(ok.to_string(), json!(text));
                } else if vals.is_empty() {
                    content.insert(key.to_string(), json!([text]));
                }
            }
        } else if let Some(pos) = opt_labels.iter().position(|label| label == raw_ans) {
            let val = opt_values.get(pos).cloned().unwrap_or_else(|| (pos + 1).to_string());
            content.insert(key.to_string(), json!(val));
        } else if let Some(ok) = other_key {
            content.insert(ok.to_string(), json!(raw_ans));
        } else {
            content.insert(key.to_string(), json!(raw_ans));
        }
    }
    Value::Object(content)
}

fn match_multi_answer(raw: &str, labels: &[String], values: &[String]) -> (Vec<String>, Option<String>) {
    // Match full known labels left-to-right at `, ` boundaries before treating leftovers as custom text,
    // so option labels containing commas are preserved intact.
    let mut selected = Vec::new();
    let mut custom_parts = Vec::new();
    let mut rest = raw.trim();
    while !rest.is_empty() {
        let matched = labels.iter().enumerate().find(|(_, label)| {
            if !rest.starts_with(label.as_str()) {
                return false;
            }
            let after = &rest[label.len()..];
            after.is_empty() || after.starts_with(", ")
        });
        if let Some((pos, label)) = matched {
            let val = values.get(pos).cloned().unwrap_or_else(|| (pos + 1).to_string());
            if !selected.contains(&val) {
                selected.push(val);
            }
            rest = rest[label.len()..].strip_prefix(", ").unwrap_or("").trim_start();
        } else if let Some(comma) = rest.find(", ") {
            let part = rest[..comma].trim();
            if !part.is_empty() {
                custom_parts.push(part);
            }
            rest = rest[comma + 2..].trim_start();
        } else {
            if !rest.trim().is_empty() {
                custom_parts.push(rest.trim());
            }
            break;
        }
    }
    let custom = if custom_parts.is_empty() { None } else { Some(custom_parts.join(", ")) };
    (selected, custom)
}

// ─────────────────────────── Timeline ───────────────────────────

/// What one tool call has said about itself so far. Updates carry only the fields that changed.
#[derive(Clone, Debug, Default)]
struct ToolState {
    title: Option<String>,
    kind: Option<String>,
    status: Option<String>,
    raw_input: Option<Value>,
    raw_output: Option<Value>,
    content: Vec<Value>,
    locations: Vec<Value>,
}

impl ToolState {
    fn merge(&mut self, update: &Value) {
        if let Some(title) = update.get("title").and_then(Value::as_str) { self.title = Some(title.into()); }
        if let Some(kind) = update.get("kind").and_then(Value::as_str) { self.kind = Some(kind.into()); }
        if let Some(status) = update.get("status").and_then(Value::as_str) { self.status = Some(status.into()); }
        if let Some(input) = update.get("rawInput").filter(|input| !input.is_null()) {
            let mut next = input.clone();
            // Preserve host-recorded AskUserQuestion questions and answers if a trailing tool_call_update
            // resends a partial or empty rawInput.
            if let (Some(prev), Some(obj)) = (self.raw_input.as_ref(), next.as_object_mut()) {
                for key in ["answers", "_answerList"] {
                    if !obj.contains_key(key) {
                        if let Some(val) = prev.get(key) {
                            obj.insert(key.into(), val.clone());
                        }
                    }
                }
                if !obj.get("questions").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
                    for key in ["questions", "_acpAskQuestion"] {
                        if let Some(val) = prev.get(key) {
                            obj.insert(key.into(), val.clone());
                        }
                    }
                }
            }
            self.raw_input = Some(next);
        }
        if let Some(output) = update.get("rawOutput").filter(|output| !output.is_null()) {
            let keep_dismissed = self.raw_output.as_ref().and_then(Value::as_str) == Some("Dismissed by user")
                && (output.as_str() == Some("") || output.as_object().is_some_and(|o| o.is_empty()));
            if !keep_dismissed {
                self.raw_output = Some(output.clone());
            }
        }
        // Content and locations replace the previous collection when present, as the protocol specifies.
        if let Some(content) = update.get("content").and_then(Value::as_array) { self.content = content.clone(); }
        if let Some(locations) = update.get("locations").and_then(Value::as_array) { self.locations = locations.clone(); }
    }

    fn path(&self) -> Option<String> {
        let input = self.raw_input.as_ref();
        input.and_then(|input| ["path", "file_path", "filePath", "file"].iter().find_map(|key| input.get(*key).and_then(Value::as_str)))
            .or_else(|| self.locations.first().and_then(|location| location.get("path")).and_then(Value::as_str))
            .or_else(|| self.diffs().first().and_then(|diff| diff.get("path")).and_then(Value::as_str))
            .map(str::to_string)
    }

    fn diffs(&self) -> Vec<&Value> {
        self.content.iter().filter(|block| block.get("type").and_then(Value::as_str) == Some("diff")).collect()
    }

    /// The tool card this call is drawn as: one of the view's known tools where the call is one, so it
    /// gets that tool's summary and body, or its own title with the raw input otherwise.
    fn card(&self) -> (String, Value) {
        let raw = self.raw_input.clone().unwrap_or_else(|| json!({}));
        let mut input = if raw.is_object() { raw.clone() } else { json!({"input": raw}) };
        let object = input.as_object_mut().expect("tool input is an object");
        let title = self.title.clone().unwrap_or_default();
        let path = self.path();
        let name = match self.kind.as_deref() {
            Some("read") => {
                if let Some(path) = &path { object.insert("file_path".into(), json!(path)); }
                "Read".to_string()
            }
            Some("edit") | Some("delete") | Some("move") => {
                let diffs = self.diffs();
                match diffs.as_slice() {
                    [diff] => {
                        let file = diff.get("path").and_then(Value::as_str).map(str::to_string).or(path.clone());
                        if let Some(file) = &file { object.insert("file_path".into(), json!(file)); }
                        let new_text = diff.get("newText").and_then(Value::as_str).unwrap_or("");
                        match diff.get("oldText").and_then(Value::as_str) {
                            Some(old_text) => {
                                object.insert("old_string".into(), json!(old_text));
                                object.insert("new_string".into(), json!(new_text));
                                "Edit".to_string()
                            }
                            None => {
                                object.insert("content".into(), json!(new_text));
                                "Write".to_string()
                            }
                        }
                    }
                    [] => {
                        if let Some(path) = &path { object.insert("file_path".into(), json!(path)); }
                        if self.kind.as_deref() == Some("edit") { "Edit".to_string() } else { short_title(&title, "Change files") }
                    }
                    many => {
                        let changes: Vec<Value> = many.iter().map(|diff| json!({
                            "path": diff.get("path").cloned().unwrap_or(Value::Null),
                            "diff": unified(diff.get("oldText").and_then(Value::as_str), diff.get("newText").and_then(Value::as_str).unwrap_or("")),
                        })).collect();
                        object.insert("changes".into(), json!(changes));
                        "FileChange".to_string()
                    }
                }
            }
            Some("execute") => {
                let command = raw.get("command").and_then(command_text)
                    .or_else(|| raw.get("cmd").and_then(command_text))
                    .unwrap_or_else(|| title.clone());
                object.insert("command".into(), json!(command));
                if !title.is_empty() && !object.contains_key("description") { object.insert("description".into(), json!(title)); }
                "Bash".to_string()
            }
            Some("search") => {
                let pattern = ["pattern", "query", "regex", "glob"].iter().find_map(|key| raw.get(*key).and_then(Value::as_str))
                    .map(str::to_string).unwrap_or_else(|| title.clone());
                object.insert("pattern".into(), json!(pattern));
                "Grep".to_string()
            }
            Some("fetch") => {
                if let Some(url) = raw.get("url").and_then(Value::as_str) {
                    object.insert("url".into(), json!(url));
                    "WebFetch".to_string()
                } else {
                    let query = raw.get("query").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| title.clone());
                    object.insert("query".into(), json!(query));
                    "WebSearch".to_string()
                }
            }
            _ => {
                if raw.get("questions").and_then(Value::as_array).is_some()
                    || title.eq_ignore_ascii_case("ask_question")
                    || title.eq_ignore_ascii_case("ask question")
                {
                    input = super::antigravity_protocol::normalize_ask_question_input(&input);
                    "AskUserQuestion".to_string()
                } else {
                    if let Some(path) = &path { object.entry("path").or_insert(json!(path)); }
                    short_title(&title, "Tool")
                }
            }
        };
        (name, input)
    }

    fn status(&self) -> &'static str {
        match self.status.as_deref() {
            Some("completed") => "completed",
            Some("failed") => "failed",
            Some("canceled") => "canceled",
            _ => "running",
        }
    }

    /// What the call produced, from its content blocks, or its raw output when it has none.
    fn output(&self) -> Option<String> {
        let mut parts = Vec::new();
        for block in &self.content {
            match block.get("type").and_then(Value::as_str) {
                Some("content") => if let Some(text) = content_text(block.get("content")) { parts.push(text); },
                Some("terminal") => {}
                _ => {}
            }
        }
        if !parts.is_empty() { return Some(parts.join("\n")); }
        let raw = self.raw_output.as_ref()?;
        if let Some(text) = raw.as_str() { return Some(text.to_string()); }
        for key in ["content", "output", "stdout", "result", "text"] {
            if let Some(text) = raw.get(key).and_then(Value::as_str) { return Some(text.to_string()); }
        }
        if let Some(text) = content_text(raw.get("content")) { return Some(text); }
        (!raw.is_null()).then(|| serde_json::to_string_pretty(raw).unwrap_or_default()).filter(|text| text != "{}")
    }
}

fn command_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() { return Some(text.to_string()); }
    let parts: Vec<&str> = value.as_array()?.iter().filter_map(Value::as_str).collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// A title short enough for a card header; the full title stays in the card's description.
fn short_title(title: &str, fallback: &str) -> String {
    let title = title.lines().next().unwrap_or("").trim();
    if title.is_empty() { return fallback.to_string(); }
    let mut out: String = title.chars().take(80).collect();
    if out.len() < title.len() { out.push('…'); }
    out
}

/// Plain unified-style text for a file change, enough for the view's diff highlighting.
fn unified(old_text: Option<&str>, new_text: &str) -> String {
    let mut out = String::new();
    for line in old_text.unwrap_or("").lines() { out.push('-'); out.push_str(line); out.push('\n'); }
    for line in new_text.lines() { out.push('+'); out.push_str(line); out.push('\n'); }
    out
}

/// Text of a content block: text itself, a resource's text, or a link written as Markdown.
fn content_text(block: Option<&Value>) -> Option<String> {
    let block = block?;
    match block.get("type").and_then(Value::as_str) {
        Some("text") => block.get("text").and_then(Value::as_str).map(str::to_string),
        Some("resource_link") => {
            let uri = block.get("uri").and_then(Value::as_str)?;
            let name = block.get("name").and_then(Value::as_str).unwrap_or(uri);
            Some(format!("[{name}]({uri})"))
        }
        Some("resource") => block.pointer("/resource/text").and_then(Value::as_str).map(str::to_string),
        _ => None,
    }
}

fn content_image(block: Option<&Value>) -> Option<ChatImage> {
    let block = block?;
    if block.get("type").and_then(Value::as_str) != Some("image") {
        return None;
    }
    let mime = block.get("mimeType").or_else(|| block.get("mime_type")).and_then(Value::as_str)
        .filter(|m| m.starts_with("image/"))?;
    let data = block.get("data").and_then(Value::as_str).filter(|d| !d.is_empty())?;
    Some(ChatImage { mime_type: mime.to_string(), data: data.to_string() })
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Block { User, Assistant, Reasoning }

/// Turns `session/update` notifications into timeline rows.
///
/// Message chunks carry no message id, so the mapper decides where one message ends: a chunk of a different
/// kind, a tool call, a plan or a new prompt closes the open block, and the next chunk starts a new row.
#[derive(Default)]
pub struct Mapper {
    seq: u64,
    /// The streaming block: its kind, row id, text so far, and the time it began.
    open: Option<(Block, String, String, Option<i64>)>,
    /// Images attached to an open `Block::User` block during `session/load` replay.
    open_user_images: Vec<ChatImage>,
    tools: HashMap<String, ToolState>,
    tool_order: Vec<String>,
    last_ask_question_id: Option<String>,
    turn: u64,
    /// True while replaying a recording, where the user's own messages arrive as chunks. Live, the engine
    /// draws a prompt itself when it sends one, and an agent's echo would draw it a second time.
    pub replaying: bool,
    /// Timestamp applied to rows created from the next updates, from a journal line.
    pub at: Option<i64>,
}

impl Mapper {
    /// A mapper for a recorded conversation, which draws the user's own messages too.
    pub fn replay() -> Self { Mapper { replaying: true, ..Mapper::default() } }

    /// A live mapper for one agent's conversation.
    pub fn for_kind(_kind: SessionKind) -> Self { Mapper::default() }

    /// A new prompt goes out: whatever was streaming is finished, and the next plan belongs to this turn.
    pub fn begin_turn(&mut self) {
        self.open = None;
        self.open_user_images.clear();
        self.last_ask_question_id = None;
        self.turn += 1;
    }

    /// How many prompts this mapper has seen, which addresses the rows that belong to one turn.
    pub fn turn(&self) -> u64 { self.turn }

    /// Close the streaming block and return its final row, so the view stops drawing it as live.
    pub fn finish(&mut self) -> Option<ChatRow> {
        let (block, id, text, at) = self.open.take()?;
        let images = if block == Block::User {
            std::mem::take(&mut self.open_user_images)
        } else {
            self.open_user_images.clear();
            Vec::new()
        };
        Some(text_row(block, id, text, images, false, at))
    }

    /// Write answered or dismissed `AskUserQuestion` state onto the active ACP tool row so the live
    /// timeline renders a `<QuestionAnswerCard>` rather than an empty collapsed row.
    pub fn record_ask_question_answer(&mut self, updated_input: Option<&Value>, allow: bool) -> Option<ChatRow> {
        let id = self.last_ask_question_id.clone()?;
        let tool = self.tools.get_mut(&id)?;
        if tool.card().0 != "AskUserQuestion" {
            return None;
        }
        let mut raw = tool.raw_input.clone().unwrap_or_else(|| json!({}));
        if !raw.get("questions").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) {
            if let Some(qs) = updated_input.and_then(|u| u.get("questions")) {
                if let Some(obj) = raw.as_object_mut() {
                    obj.insert("questions".into(), qs.clone());
                } else {
                    raw = json!({"questions": qs});
                }
            }
        }
        let mut normalized = super::antigravity_protocol::normalize_ask_question_input(&raw);
        if allow {
            if let Some(obj) = normalized.as_object_mut() {
                if let Some(answers) = updated_input.and_then(|u| u.get("answers")) {
                    obj.insert("answers".into(), answers.clone());
                }
                if let Some(list) = updated_input.and_then(|u| u.get("_answerList")) {
                    obj.insert("_answerList".into(), list.clone());
                }
            }
        } else {
            tool.raw_output = Some(json!("Dismissed by user"));
        }
        tool.raw_input = Some(normalized);
        Some(tool_row(&id, tool))
    }

    /// Tool calls the agent left running when its turn ended, marked so they stop spinning.
    pub fn settle_running(&mut self, canceled: bool) -> Vec<ChatRow> {
        let mut rows = Vec::new();
        for id in self.tool_order.clone() {
            let Some(tool) = self.tools.get_mut(&id) else { continue };
            if tool.status() != "running" { continue; }
            tool.status = Some(if canceled { "canceled".into() } else { "completed".into() });
            let tool = tool.clone();
            rows.push(tool_row(&id, &tool));
        }
        rows
    }

    /// Apply one update and return every row it created or changed.
    pub fn update(&mut self, update: &Value) -> Vec<ChatRow> {
        let mut rows = Vec::new();
        match update.get("sessionUpdate").and_then(Value::as_str).unwrap_or("") {
            "user_message_chunk" => {
                if !self.replaying {
                    rows.extend(self.finish());
                    return rows;
                }
                if let Some(text) = content_text(update.get("content")) {
                    if super::antigravity_protocol::is_internal_recovery_prompt(text.trim()) {
                        return rows;
                    }
                    // A replayed user message starts a turn, as a live prompt does.
                    if !matches!(self.open, Some((Block::User, _, _, _))) {
                        rows.extend(self.finish());
                        self.begin_turn();
                    }
                    rows.push(self.append(Block::User, &text, None));
                } else if let Some(image) = content_image(update.get("content")) {
                    if !matches!(self.open, Some((Block::User, _, _, _))) {
                        rows.extend(self.finish());
                        self.begin_turn();
                    }
                    rows.push(self.append_user_image(image));
                }
            }
            "agent_message_chunk" => {
                let message = self.message_id(update);
                if let Some(text) = content_text(update.get("content")) { rows.extend(self.append_closing(Block::Assistant, &text, message)); }
            }
            "agent_thought_chunk" => {
                let message = self.message_id(update);
                if let Some(text) = content_text(update.get("content")) { rows.extend(self.append_closing(Block::Reasoning, &text, message)); }
            }
            "tool_call" => {
                rows.extend(self.finish());
                let Some(id) = update.get("toolCallId").and_then(Value::as_str).filter(|id| !id.is_empty()) else { return rows };
                let mut tool = ToolState::default();
                tool.merge(update);
                // Copilot logs its intent through a tool of its own. A live turn never announces it, but
                // `session/load` replays it; hiding it keeps both views alike, and its updates then fall
                // through as unannounced.
                if tool.title.as_deref() == Some("report_intent") { return rows; }
                if tool.card().0 == "AskUserQuestion" {
                    self.last_ask_question_id = Some(id.to_string());
                }
                self.remember(id, tool.clone());
                rows.push(tool_row(id, &tool));
            }
            "tool_call_update" => {
                let Some(id) = update.get("toolCallId").and_then(Value::as_str) else { return rows };
                // An update for a call never announced is an internal tool the agent keeps to itself.
                let Some(tool) = self.tools.get_mut(id) else { return rows };
                tool.merge(update);
                if tool.card().0 == "AskUserQuestion" {
                    self.last_ask_question_id = Some(id.to_string());
                }
                let tool = tool.clone();
                rows.push(tool_row(id, &tool));
            }
            "plan" => {
                rows.extend(self.finish());
                let todos: Vec<Value> = update.get("entries").and_then(Value::as_array).into_iter().flatten().map(|entry| json!({
                    "content": entry.get("content").and_then(Value::as_str).unwrap_or(""),
                    "status": entry.get("status").and_then(Value::as_str).unwrap_or("pending"),
                })).collect();
                rows.push(ChatRow::Tool {
                    id: format!("acp-plan-{}", self.turn),
                    name: "TodoWrite".into(),
                    input: json!({"todos": todos}),
                    output: None,
                    is_error: false,
                    status: "completed",
                    subagent: None,
                    children: Vec::new(),
                });
            }
            _ => {}
        }
        rows
    }

    /// ACP does not give chunks message ids that could name rows.
    fn message_id(&self, _update: &Value) -> Option<String> {
        None
    }

    /// Append to the open block of this kind, closing any other kind first, so two messages in a row
    /// never share a row.
    fn append_closing(&mut self, block: Block, text: &str, _message: Option<String>) -> Vec<ChatRow> {
        let mut rows = Vec::new();
        if self.open.as_ref().is_some_and(|(open, _, _, _)| *open != block) { rows.extend(self.finish()); }
        rows.push(self.append(block, text, None));
        rows
    }

    fn append(&mut self, block: Block, text: &str, named: Option<String>) -> ChatRow {
        if self.open.as_ref().is_none_or(|(open, _, _, _)| *open != block) {
            self.seq += 1;
            let prefix = match block { Block::User => "u", Block::Assistant => "a", Block::Reasoning => "r" };
            let id = named.unwrap_or_else(|| format!("acp-{prefix}{}", self.seq));
            self.open = Some((block, id, String::new(), self.at));
            if block != Block::User {
                self.open_user_images.clear();
            }
        }
        let (_, id, buffer, at) = self.open.as_mut().expect("a block was just opened");
        buffer.push_str(text);
        let images = if block == Block::User { self.open_user_images.clone() } else { Vec::new() };
        text_row(block, id.clone(), buffer.clone(), images, true, *at)
    }

    fn append_user_image(&mut self, image: ChatImage) -> ChatRow {
        if self.open.as_ref().is_none_or(|(open, _, _, _)| *open != Block::User) {
            self.seq += 1;
            let id = format!("acp-u{}", self.seq);
            self.open = Some((Block::User, id, String::new(), self.at));
            self.open_user_images.clear();
        }
        if self.open_user_images.len() < crate::command_core::MAX_IMAGES_PER_MESSAGE {
            self.open_user_images.push(image);
        }
        let (_, id, buffer, at) = self.open.as_ref().expect("a user block was just opened");
        text_row(Block::User, id.clone(), buffer.clone(), self.open_user_images.clone(), true, *at)
    }

    fn remember(&mut self, id: &str, tool: ToolState) {
        if self.tools.insert(id.to_string(), tool).is_none() {
            self.tool_order.push(id.to_string());
        }
        while self.tool_order.len() > TOOL_MEMORY {
            let old = self.tool_order.remove(0);
            self.tools.remove(&old);
        }
    }
}

/// The row id of a block named by its message, matching what the agent's log replays to.
fn text_row(block: Block, id: String, text: String, images: Vec<ChatImage>, streaming: bool, at: Option<i64>) -> ChatRow {
    match block {
        Block::User => ChatRow::User { id, text, images, at },
        Block::Assistant => ChatRow::Assistant { id, text, streaming, model: None, at, duration_ms: None },
        Block::Reasoning => ChatRow::Reasoning { id, text, streaming },
    }
}

fn tool_row(id: &str, tool: &ToolState) -> ChatRow {
    let (name, input) = tool.card();
    let status = tool.status();
    ChatRow::Tool {
        id: format!("acp-t-{id}"),
        name,
        input,
        output: tool.output(),
        is_error: status == "failed",
        status,
        subagent: None,
        children: Vec::new(),
    }
}

/// Rows of a recorded notification stream: Grok's native `updates.jsonl`, or the engine's journal.
///
/// Rows the stream left open are closed, and tool calls it left running belong to a process that is gone.
pub fn history_rows(content: &str) -> Vec<ChatRow> {
    let mut mapper = Mapper::replay();
    let mut rows: Vec<ChatRow> = Vec::new();
    let mut at: HashMap<String, usize> = HashMap::new();
    let mut place = |row: ChatRow, rows: &mut Vec<ChatRow>| {
        match at.get(row.id()) {
            Some(&index) => rows[index] = row,
            None => { at.insert(row.id().to_string(), rows.len()); rows.push(row); }
        }
    };
    for line in content.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
        if value.get("method").and_then(Value::as_str) == Some("_vlx/turn_end") {
            if let Some(row) = mapper.finish() { place(row, &mut rows); }
            if let Some(duration) = value.get("durationMs").and_then(Value::as_u64) { attach_duration(&mut rows, duration); }
            continue;
        }
        if value.get("method").and_then(Value::as_str) != Some("session/update") { continue; }
        let Some(update) = value.pointer("/params/update") else { continue };
        mapper.at = line_time(&value);
        for row in mapper.update(update) { place(row, &mut rows); }
    }
    for row in mapper.finish().into_iter().chain(mapper.settle_running(true)) { place(row, &mut rows); }
    rows
}

/// A recorded line's time in milliseconds: the journal's own field, or Grok's native timestamp.
fn line_time(value: &Value) -> Option<i64> {
    if let Some(at) = value.get("_vlxAt").and_then(Value::as_i64) { return Some(at); }
    // Grok writes whole seconds; a value this large is already in milliseconds.
    let number = value.get("timestamp")?.as_i64()?;
    Some(if number < 100_000_000_000 { number * 1000 } else { number })
}

// ─────────────────────────── Journal ───────────────────────────

/// Where the engine keeps a conversation's notifications for agents without a readable recording.
pub fn journal_path(data_dir: &Path, kind: SessionKind, native_id: &str) -> Option<PathBuf> {
    // Antigravity writes its own transcript, which also keeps attached media paths; a journal would only
    // duplicate it and lose those images.
    if kind == SessionKind::Antigravity {
        return None;
    }
    if native_id.is_empty() || !native_id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')) || native_id.starts_with('.') {
        return None;
    }
    Some(data_dir.join("chat-acp").join(kind.as_str()).join(format!("{native_id}.jsonl")))
}

/// One journal line: the update as the agent sent it, with the time it arrived.
pub fn journal_line(update: &Value, at: i64) -> String {
    json!({"method":"session/update","params":{"update":update},"_vlxAt":at}).to_string()
}

/// A prompt the engine sent, journaled the way `session/load` replays it.
pub fn journal_prompt(text: &str, at: i64) -> String {
    journal_line(&json!({"sessionUpdate":"user_message_chunk","content":{"type":"text","text":text}}), at)
}

/// The end of a turn and how long it took. ACP reports neither in its replay, so the journal records it
/// and a history read attaches it to the turn's last answer, as a live turn does.
pub fn journal_turn_end(duration_ms: u64, at: i64) -> String {
    json!({"method":"_vlx/turn_end","durationMs":duration_ms,"_vlxAt":at}).to_string()
}

/// Attach a turn's duration to its last top-level answer, the row the live view puts it on.
pub fn attach_duration(rows: &mut [ChatRow], duration: u64) {
    let start = rows.iter().rposition(|row| matches!(row, ChatRow::User { .. } | ChatRow::Shell { .. })).map_or(0, |index| index + 1);
    if let Some(ChatRow::Assistant { duration_ms, .. }) = rows[start..].iter_mut().rev().find(|row| matches!(row, ChatRow::Assistant { .. })) {
        *duration_ms = Some(duration);
    }
}

pub fn append_journal(path: &Path, lines: &[String]) -> std::io::Result<()> {
    if lines.is_empty() { return Ok(()); }
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    let mut buffer = String::new();
    for line in lines { buffer.push_str(line); buffer.push('\n'); }
    file.write_all(buffer.as_bytes())
}

/// Replace the journal with a complete replay, through a temporary file so a crash never leaves half of one.
pub fn rewrite_journal(path: &Path, lines: &[String]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    let temp = path.with_extension("jsonl.tmp");
    {
        let mut file = std::fs::File::create(&temp)?;
        let mut buffer = String::new();
        for line in lines { buffer.push_str(line); buffer.push('\n'); }
        file.write_all(buffer.as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&temp, path)
}

/// Read a recorded stream within the history byte budget.
pub fn read_recording(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|_| "source_missing: The conversation recording could not be found".to_string())?;
    let limit = crate::agent::history_store::limits::PAGE_BYTES as u64;
    if file.metadata().map(|meta| meta.len()).unwrap_or(0) > limit * 64 {
        return Err("detail_requires_paging: The conversation recording is too large to show at once".into());
    }
    let mut content = String::new();
    for line in std::io::BufReader::new(file).lines() {
        let Ok(line) = line else { break };
        content.push_str(&line);
        content.push('\n');
    }
    Ok(content)
}

/// History rows of an ACP conversation that has no running process.
pub fn replay(data_dir: Option<&Path>, kind: SessionKind, native_id: &str) -> Result<Vec<ChatRow>, String> {
    let path = if kind == SessionKind::Grok {
        crate::agent::resume::find_grok_updates(native_id)
    } else if kind == SessionKind::Antigravity {
        crate::agent::antigravity::transcript_path(native_id)
    } else {
        data_dir.and_then(|dir| journal_path(dir, kind, native_id)).filter(|path| path.is_file())
    };
    let path = path.ok_or("source_missing: The conversation recording could not be found")?;
    let content = read_recording(&path)?;
    // Antigravity's transcript is its own step format, not ACP notifications.
    let mut rows = if kind == SessionKind::Antigravity {
        super::antigravity_protocol::history_rows(&content)
    } else {
        history_rows(&content)
    };
    super::shell::map_replayed_rows(&mut rows);
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(value: Value) -> Value { value }

    fn texts(rows: &[ChatRow]) -> Vec<(String, String)> {
        rows.iter().map(|row| match row {
            ChatRow::User { id, text, .. } => (id.clone(), format!("user:{text}")),
            ChatRow::Assistant { id, text, .. } => (id.clone(), format!("assistant:{text}")),
            ChatRow::Reasoning { id, text, .. } => (id.clone(), format!("reasoning:{text}")),
            ChatRow::Tool { id, name, status, .. } => (id.clone(), format!("tool:{name}:{status}")),
            other => (other.id().to_string(), "other".into()),
        }).collect()
    }

    /// The Copilot 1.0.63 turn recorded on 2026-10-19: prose, a thought, a read, then the answer.
    fn copilot_turn() -> Vec<Value> {
        vec![
            json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Reading"}}),
            json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":" sample.txt."}}),
            json!({"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"Reading file"}}),
            json!({"sessionUpdate":"tool_call","toolCallId":"call_M2U","title":"Viewing /w/sample.txt","kind":"read","status":"pending",
                "rawInput":{"path":"/w/sample.txt"},"locations":[{"path":"/w/sample.txt"}]}),
            json!({"sessionUpdate":"tool_call_update","toolCallId":"call_H5f","status":"completed",
                "content":[{"type":"content","content":{"type":"text","text":"Intent logged"}}]}),
            json!({"sessionUpdate":"tool_call_update","toolCallId":"call_M2U","status":"completed",
                "rawOutput":{"content":"1. alpha line\n2. beta line\n3. ","detailedContent":"diff"}}),
            json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"beta"}}),
            json!({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":" line"}}),
        ]
    }

    #[test]
    fn a_live_turn_splits_messages_at_thoughts_and_tools_and_ignores_unannounced_calls() {
        let mut mapper = Mapper::default();
        mapper.begin_turn();
        let mut last: Vec<(String, String)> = Vec::new();
        for value in copilot_turn() {
            for row in mapper.update(&update(value)) {
                let entry = texts(&[row]).remove(0);
                match last.iter().position(|(id, _)| *id == entry.0) {
                    Some(index) => last[index] = entry,
                    None => last.push(entry),
                }
            }
        }
        if let Some(row) = mapper.finish() {
            let entry = texts(&[row]).remove(0);
            let index = last.iter().position(|(id, _)| *id == entry.0).unwrap();
            last[index] = entry;
        }
        assert_eq!(last, vec![
            ("acp-a1".into(), "assistant:Reading sample.txt.".into()),
            ("acp-r2".into(), "reasoning:Reading file".into()),
            ("acp-t-call_M2U".into(), "tool:Read:completed".into()),
            ("acp-a3".into(), "assistant:beta line".into()),
        ]);
    }

    #[test]
    fn a_read_call_becomes_the_read_card_with_its_output() {
        let mut mapper = Mapper::default();
        let mut rows = Vec::new();
        for value in copilot_turn() { rows.extend(mapper.update(&value)); }
        let read = rows.iter().rev().find(|row| row.id() == "acp-t-call_M2U").unwrap();
        let ChatRow::Tool { name, input, output, is_error, .. } = read else { panic!("not a tool row") };
        assert_eq!(name, "Read");
        assert_eq!(input["file_path"], "/w/sample.txt");
        assert_eq!(output.as_deref(), Some("1. alpha line\n2. beta line\n3. "));
        assert!(!is_error);
    }

    #[test]
    fn tool_kinds_map_to_the_view_cards() {
        let card = |update: Value| { let mut tool = ToolState::default(); tool.merge(&update); tool.card() };
        let (name, input) = card(json!({"kind":"edit","title":"Edit a.rs","content":[{"type":"diff","path":"/w/a.rs","oldText":"x","newText":"y"}]}));
        assert_eq!((name.as_str(), &input["file_path"], &input["old_string"], &input["new_string"]), ("Edit", &json!("/w/a.rs"), &json!("x"), &json!("y")));
        let (name, input) = card(json!({"kind":"edit","content":[{"type":"diff","path":"/w/new.rs","newText":"fn main() {}"}]}));
        assert_eq!((name.as_str(), &input["content"]), ("Write", &json!("fn main() {}")));
        let (name, input) = card(json!({"kind":"edit","content":[
            {"type":"diff","path":"/w/a","oldText":"1","newText":"2"},{"type":"diff","path":"/w/b","newText":"3"}]}));
        assert_eq!(name, "FileChange");
        assert_eq!(input["changes"][1], json!({"path":"/w/b","diff":"+3\n"}));
        let (name, input) = card(json!({"kind":"execute","title":"Run tests","rawInput":{"command":["cargo","test"]}}));
        assert_eq!((name.as_str(), &input["command"], &input["description"]), ("Bash", &json!("cargo test"), &json!("Run tests")));
        let (name, input) = card(json!({"kind":"search","title":"Search","rawInput":{"query":"TODO"}}));
        assert_eq!((name.as_str(), &input["pattern"]), ("Grep", &json!("TODO")));
        let (name, input) = card(json!({"kind":"fetch","rawInput":{"url":"https://example.com"}}));
        assert_eq!((name.as_str(), &input["url"]), ("WebFetch", &json!("https://example.com")));
        let (name, input) = card(json!({"kind":"other","title":"Ask the user\nsecond line","rawInput":"plain"}));
        assert_eq!((name.as_str(), &input["input"]), ("Ask the user", &json!("plain")));
    }

    #[test]
    fn a_plan_is_one_todo_card_per_turn() {
        let mut mapper = Mapper::default();
        mapper.begin_turn();
        let plan = |status: &str| json!({"sessionUpdate":"plan","entries":[{"content":"Read","priority":"high","status":status}]});
        let first = mapper.update(&plan("in_progress"));
        let second = mapper.update(&plan("completed"));
        assert_eq!(first[0].id(), second[0].id());
        let ChatRow::Tool { name, input, .. } = &second[0] else { panic!() };
        assert_eq!(name, "TodoWrite");
        assert_eq!(input["todos"][0], json!({"content":"Read","status":"completed"}));
        mapper.begin_turn();
        assert_ne!(mapper.update(&plan("pending"))[0].id(), first[0].id());
    }

    #[test]
    fn live_user_chunks_are_not_drawn_twice_but_replayed_ones_are() {
        let chunk = json!({"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"hi"}});
        assert!(Mapper::default().update(&chunk).is_empty());
        let mut replay = Mapper::replay();
        assert_eq!(texts(&replay.update(&chunk)), vec![("acp-u1".into(), "user:hi".into())]);
    }

    #[test]
    fn a_journal_replays_to_the_rows_the_live_turn_drew() {
        let mut lines = vec![journal_prompt("Read sample.txt", 1_000)];
        lines.extend(copilot_turn().iter().enumerate().map(|(index, value)| journal_line(value, 2_000 + index as i64)));
        lines.push(journal_turn_end(8_000, 8_500));
        lines.push(journal_prompt("Thanks", 9_000));
        let rows = history_rows(&lines.join("\n"));
        assert_eq!(texts(&rows), vec![
            ("acp-u1".into(), "user:Read sample.txt".into()),
            ("acp-a2".into(), "assistant:Reading sample.txt.".into()),
            ("acp-r3".into(), "reasoning:Reading file".into()),
            ("acp-t-call_M2U".into(), "tool:Read:completed".into()),
            ("acp-a4".into(), "assistant:beta line".into()),
            ("acp-u5".into(), "user:Thanks".into()),
        ]);
        let ChatRow::User { at, .. } = &rows[0] else { panic!() };
        assert_eq!(*at, Some(1_000));
        let ChatRow::Assistant { streaming, duration_ms, .. } = &rows[4] else { panic!() };
        assert!(!streaming);
        assert_eq!(*duration_ms, Some(8_000));
        assert!(matches!(&rows[1], ChatRow::Assistant { duration_ms: None, .. }));
    }

    #[test]
    fn replayed_calls_left_running_are_canceled() {
        let line = journal_line(&json!({"sessionUpdate":"tool_call","toolCallId":"c1","kind":"execute","status":"in_progress","rawInput":{"command":"sleep 9"}}), 1);
        let rows = history_rows(&line);
        let ChatRow::Tool { status, .. } = &rows[0] else { panic!() };
        assert_eq!(*status, "canceled");
    }

    #[test]
    fn permission_cards_offer_the_standing_rule_and_answers_pick_the_right_option() {
        let params = json!({"sessionId":"s","toolCall":{"toolCallId":"c1","title":"Run cargo test","kind":"execute","rawInput":{"command":"cargo test"}},
            "options":[{"optionId":"yes","name":"Allow","kind":"allow_once"},{"optionId":"always","name":"Always","kind":"allow_always"},
                {"optionId":"no","name":"Reject","kind":"reject_once"}]});
        let payload = permission_payload(SessionKind::Copilot, "7", &params, None);
        assert_eq!(payload["tool_name"], "Bash");
        assert_eq!(payload["input"]["command"], "cargo test");
        assert_eq!(payload["display_name"], "Copilot");
        assert_eq!(payload["permission_suggestions"][0]["optionId"], "always");
        let options = params.get("options");
        assert_eq!(choose_option(options, true, None).as_deref(), Some("yes"));
        assert_eq!(choose_option(options, true, Some("always")).as_deref(), Some("always"));
        assert_eq!(choose_option(options, true, Some("forged")).as_deref(), Some("yes"));
        assert_eq!(choose_option(options, false, None).as_deref(), Some("no"));
        let only_always = json!([{"optionId":"a","kind":"allow_always"}]);
        assert_eq!(choose_option(Some(&only_always), true, None).as_deref(), Some("a"));
        assert_eq!(choose_option(Some(&only_always), false, None), None);
        assert_eq!(permission_outcome(&json!(7), None)["result"]["outcome"]["outcome"], "cancelled");
    }

    #[test]
    fn a_selector_of_one_model_is_not_a_choice_and_internal_intent_calls_stay_hidden() {
        let mut catalog = Catalog::default();
        catalog.absorb_session(&json!({"models":{"availableModels":[{"modelId":"auto","name":"Auto"},{"modelId":"auto","name":"Auto"}],"currentModelId":"auto"},
            "configOptions":[{"type":"select","id":"model","category":"model","currentValue":"auto","options":[{"value":"auto","name":"Auto"}]}]}));
        assert_eq!(catalog.models.len(), 1);
        assert!(!catalog.model_switchable());
        let mut mapper = Mapper::replay();
        assert!(mapper.update(&json!({"sessionUpdate":"tool_call","toolCallId":"c9","title":"report_intent","kind":"other","rawInput":{"intent":"Count"}})).is_empty());
        assert!(mapper.update(&json!({"sessionUpdate":"tool_call_update","toolCallId":"c9","status":"completed"})).is_empty());
    }

    #[test]
    fn only_a_missing_conversation_counts_as_one() {
        // Grok reports the lost session directory this way.
        assert!(conversation_missing(&json!({"code":-32603,"message":"Path not found.","data":{"code":"FS_NOT_FOUND"}})));
        assert!(conversation_missing(&json!({"code":-32000,"message":"Session not found"})));
        assert!(conversation_missing(&json!({"code":-32603,"message":"no such file or directory","data":{"code":"ENOENT"}})));
        // Everything else is a real failure, not a conversation to replace.
        assert!(!conversation_missing(&json!({"code":-32602,"message":"Invalid params"})));
        assert!(!conversation_missing(&json!({"code":-32603,"message":"Authentication required"})));
        assert!(!conversation_missing(&json!({"code":-32000,"message":"command not found"})));
        assert!(!conversation_missing(&json!({"code":-32000,"message":"model not found"})));
    }

    #[test]
    fn antigravity_cold_model_errors_are_recognized_narrowly() {
        assert!(antigravity_model_not_ready(&json!({
            "code": -32603,
            "message": "failed to construct executor: unknown model key MODEL_PLACEHOLDER_M532: model not found"
        })));
        assert!(antigravity_model_not_ready(&json!({
            "code": -32603,
            "message": "failed to construct executor: unknown model key MODEL_PLACEHOLDER_M528: model not found"
        })));
        let plan_missing = json!({
            "code": -32603,
            "message": "failed to construct executor: plan model not specified; you must specify a valid model"
        });
        assert!(antigravity_model_not_ready(&plan_missing));
        assert!(antigravity_plan_model_missing(&plan_missing));
        assert!(!antigravity_plan_model_missing(&json!({
            "code": -32603,
            "message": "failed to construct executor: unknown model key MODEL_PLACEHOLDER_M532: model not found"
        })));
        assert!(!antigravity_model_not_ready(&json!({"code": -32602, "message": "Unknown model ID: gemini-99"})));
        assert!(!antigravity_model_not_ready(&json!({"code": -32000, "message": "Authentication required"})));
        assert!(!antigravity_model_not_ready(&json!({"code": -32603, "message": "failed to construct executor: permission denied"})));

        assert_eq!(
            antigravity_cold_model_chunk_error("\n⚠ failed to construct executor: plan model not specified; you must specify a valid model\n").as_deref(),
            Some("failed to construct executor: plan model not specified; you must specify a valid model"),
        );
        assert_eq!(
            antigravity_cold_model_chunk_error("\n⚠ failed to construct executor: unknown model key MODEL_PLACEHOLDER_M532: model not found\nError ID: edc0f379-1\n").as_deref(),
            Some("failed to construct executor: unknown model key MODEL_PLACEHOLDER_M532: model not found"),
        );
        assert!(antigravity_cold_model_chunk_error("failed to construct executor: plan model not specified; you must specify a valid model").is_none());
        assert!(antigravity_cold_model_chunk_error("The error was ⚠ failed to construct executor: plan model not specified").is_none());
        assert!(antigravity_cold_model_chunk_error("⚠ failed to construct executor: plan model not specified\nHere is why it happened.").is_none());
        assert!(antigravity_cold_model_chunk_error("⚠ failed to construct executor: permission denied").is_none());

        assert!(antigravity_transient_error(&json!({
            "code": -32603,
            "message": "generating and executing: NOT_FOUND (code 404): Requested entity was not found."
        })));
        assert!(antigravity_transient_error(&json!({
            "code": -32603,
            "message": "modelAPI stream error: INTERNAL (code 500): Internal error encountered."
        })));
        assert!(!antigravity_transient_error(&json!({
            "code": -32603,
            "message": "RESOURCE_EXHAUSTED (code 429): Resource has been exhausted."
        })));
        assert!(!antigravity_transient_error(&json!({
            "code": -32603,
            "message": "UNAUTHENTICATED (code 401): Request had invalid authentication credentials."
        })));
    }

    #[test]
    fn each_model_keeps_its_own_reasoning_ladder() {
        let mut catalog = Catalog::default();
        catalog.absorb_session(&json!({
            "models":{"currentModelId":"a","availableModels":[
                {"modelId":"a","name":"A","_meta":{"reasoningEfforts":[{"id":"none"},{"id":"low"},{"id":"high"},{"id":"max"}]}},
                {"modelId":"b","name":"B","_meta":{"reasoningEfforts":[{"id":"high"},{"id":"medium"},{"id":"low"},{"id":"high"}]}},
                {"modelId":"c","name":"C"}
            ]},
            "configOptions":[{"type":"select","id":"reasoning_effort","category":"thought_level","currentValue":"low",
                "options":[{"value":"none","name":"None"},{"value":"low","name":"Low"}]}]
        }));
        assert_eq!(catalog.model_efforts["b"], ["high", "medium", "low"]);
        assert!(!catalog.model_efforts.contains_key("c"));
        // The opening `thought_level` option is what this session can actually switch for its own model,
        // so it wins over the model's wider `_meta` ladder.
        assert_eq!(catalog.model_efforts["a"], ["none", "low"]);
        // Switching models without a `thought_level` option leaves the stated ladders alone.
        catalog.absorb_options(&[json!({"type":"select","id":"model","category":"model","currentValue":"b",
            "options":[{"value":"a","name":"A"},{"value":"b","name":"B"}]})]);
        assert_eq!(catalog.model_efforts["b"], ["high", "medium", "low"]);
    }

    #[test]
    fn the_catalog_reads_states_and_config_options() {
        let mut catalog = Catalog::default();
        catalog.absorb_session(&json!({
            "modes":{"availableModes":[{"id":"agent","name":"Agent"},{"id":"plan","name":"Plan","description":"Plan first"}],"currentModeId":"agent"},
            "configOptions":[{"type":"select","id":"allow_all","category":"permissions","currentValue":"off","options":[]}],
        }));
        assert_eq!(catalog.modes.len(), 2);
        assert_eq!(catalog.mode.as_deref(), Some("agent"));
        assert!(!catalog.model_switchable());
        catalog.absorb_options(&[json!({"type":"select","id":"model","category":"model","currentValue":"m2","options":[
            {"group":"A","name":"A","options":[{"value":"m1","name":"One"}]},{"value":"m2","name":"Two"}]}),
            json!({"type":"select","id":"effort","category":"thought_level","currentValue":"high","options":[{"value":"low","name":"Low"},{"value":"high","name":"High"}]})]);
        assert_eq!(catalog.models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["m1", "m2"]);
        assert_eq!(catalog.model.as_deref(), Some("m2"));
        assert!(catalog.model_switchable() && catalog.effort_switchable());
        assert_eq!(catalog.effort.as_deref(), Some("high"));
        assert_eq!(catalog.model_efforts.get("m2").map(Vec::as_slice), Some(&["low".into(), "high".into()][..]));
        // Switching to a model whose options omit `thought_level` clears stale effort state.
        catalog.absorb_options(&[json!({"type":"select","id":"model","category":"model","currentValue":"m1","options":[
            {"value":"m1","name":"One"},{"value":"m2","name":"Two"}]})]);
        assert!(!catalog.effort_switchable());
        assert!(catalog.efforts.is_empty() && catalog.effort.is_none());
        assert_eq!(catalog.model_efforts.get("m1").map(Vec::as_slice), Some(&[][..]));
        let mut state = Catalog::default();
        state.absorb_session(&json!({
            "models":{"availableModels":[{"modelId":"g1-low","name":"G1 (Low)"},{"modelId":"g1-high","name":"G1 (High)"}],"currentModelId":"g1-low"},
            "configOptions":[{"type":"select","id":"model","category":"model","currentValue":"g1","options":[{"value":"g1","name":"G1"}]}]
        }));
        assert_eq!(state.state_models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["g1-low", "g1-high"]);
        assert!(!state.models_from_state);
        let mut legacy = Catalog::default();
        legacy.absorb_session(&json!({"models":{"availableModels":[{"modelId":"g1","name":"G1"},{"modelId":"g2","name":"G2"}],"currentModelId":"g1"}}));
        assert!(legacy.model_switchable() && legacy.models_from_state);
    }

    #[test]
    fn incoming_lines_are_sorted_into_responses_requests_and_notifications() {
        assert_eq!(parse_line(r#"{"jsonrpc":"2.0","id":"vlx-prompt-3","result":{"stopReason":"end_turn"}}"#),
            Some(Incoming::Response { id: "vlx-prompt-3".into(), result: Some(json!({"stopReason":"end_turn"})), error: None }));
        assert_eq!(parse_line(r#"{"jsonrpc":"2.0","id":4,"method":"session/request_permission","params":{}}"#),
            Some(Incoming::Request { id: json!(4), method: "session/request_permission".into(), params: json!({}) }));
        assert!(matches!(parse_line(r#"{"jsonrpc":"2.0","method":"session/update","params":{}}"#), Some(Incoming::Notification { .. })));
        assert_eq!(parse_line("not json"), None);
    }

    #[test]
    fn sign_in_errors_are_recognized_from_each_agent() {
        assert!(auth_required(&json!({"code":-32000,"message":"Authentication required","data":{"message":"Please run 'agent login' first"}})));
        assert!(auth_required(&json!({"code":-32000,"message":"Authentication required","data":"no auth method id provided"})));
        assert!(!auth_required(&json!({"code":-32000,"message":"Session not found"})));
        assert_eq!(error_text(&json!({"code":-32000,"message":"Authentication required","data":{"message":"Run login"}})), "Run login");
    }

    #[test]
    fn launch_arguments_follow_each_agent() {
        assert_eq!(launch_args(SessionKind::Grok, Some("grok-4.6"), None, &[]).unwrap(), ["agent", "stdio"]);
        assert_eq!(launch_args(SessionKind::Antigravity, Some("gemini-4-argon"), Some("high"),
            &[
                "--dangerously-skip-permissions".into(),
                "--add-dir=/w".into(),
                "--model".into(),
                "gemini".into(),
                "--mode".into(),
                "plan".into(),
                "--effort=high".into(),
            ]).unwrap(),
            ["acp", "--add-dir=/w"]);
        assert!(launch_args(SessionKind::Antigravity, None, None, &["--conversation=old".into()]).is_err());
        assert_eq!(launch_args(SessionKind::Cursor, None, None, &["--force".into()]).unwrap(), ["acp"]);
        assert_eq!(launch_args(SessionKind::Copilot, Some("gpt-5"), Some("high"), &["--allow-all-paths".into()]).unwrap(),
            ["--acp", "--model", "gpt-5", "--effort", "high", "--allow-all-paths"]);
        assert!(launch_args(SessionKind::Copilot, None, None, &["--resume=abc".into()]).is_err());
        assert_eq!(reply_model(SessionKind::Kimi, "kimi-k2"), "kimi-k2");
        assert!(validate("steer", 0, true, false).is_err());
        assert!(validate("steer", 0, true, true).is_ok());
        assert!(validate("queue", 1, false, true).is_err());
        assert!(validate("interrupt", 1, true, false).is_ok());
    }

    #[test]
    fn initialize_advertises_form_elicitation_capability() {
        let req = initialize("init-1");
        assert_eq!(req.pointer("/params/clientCapabilities/elicitation"), Some(&json!({"form": {}})));
    }

    #[test]
    fn ask_question_elicitation_converts_and_round_trips_answers() {
        let params = json!({
            "mode": "form",
            "toolCallId": "tc-1",
            "message": "Need your preferences",
            "requestedSchema": {
                "type": "object",
                "properties": {
                    "q10": { "type": "string", "title": "Final note" },
                    "q1": {
                        "type": "string",
                        "title": "Choose deploy target",
                        "oneOf": [
                            {"const": "1", "title": "Staging"},
                            {"const": "2", "title": "Production"}
                        ]
                    },
                    "q1_other": { "type": "string", "title": "Other" },
                    "q2": {
                        "type": "array",
                        "title": "Enable features",
                        "items": {
                            "anyOf": [
                                {"const": "1", "title": "Metrics, Tracing"},
                                {"const": "2", "title": "Alerts"}
                            ]
                        }
                    },
                    "q2_other": { "type": "string", "title": "Other" }
                },
                "required": ["q1", "q2"]
            }
        });
        let card = elicitation_payload(SessionKind::Antigravity, "acp-5", &params);
        assert_eq!(card["tool_name"], "AskUserQuestion");
        assert_eq!(card["input"]["_acpAskQuestion"], true);
        let qs = card["input"]["questions"].as_array().unwrap();
        assert_eq!(qs.iter().map(|q| q["_key"].as_str().unwrap()).collect::<Vec<_>>(), ["q1", "q2", "q10"]);
        assert_eq!(qs[0]["multiSelect"], false);
        assert_eq!(qs[0]["allowOther"], true);
        assert_eq!(qs[1]["multiSelect"], true);
        assert_eq!(qs[2]["allowOther"], true);

        let via_ui = json!({
            "answers": {
                "Choose deploy target": "custom cluster",
                "Enable features": "Metrics, Tracing, custom flag",
                "Final note": "looks good"
            }
        });
        let ok = elicitation_outcome(&json!(5), &card, &params, true, Some(&via_ui), None);
        assert_eq!(ok["result"], json!({
            "action": "accept",
            "content": {
                "q1_other": "custom cluster",
                "q2": ["1"],
                "q2_other": "custom flag",
                "q10": "looks good"
            }
        }));

        let via_indices = json!({
            "_selectedIndices": {"0": [1], "1": [0, 1]},
            "_otherTexts": {"1": "extra"}
        });
        let ok2 = elicitation_outcome(&json!(5), &card, &params, true, Some(&via_indices), None);
        assert_eq!(ok2["result"], json!({
            "action": "accept",
            "content": {
                "q1": "2",
                "q2": ["1", "2"],
                "q2_other": "extra"
            }
        }));

        let declined = elicitation_outcome(&json!(5), &card, &params, false, None, Some("decline"));
        assert_eq!(declined["result"], json!({"action": "decline"}));
        let cancelled = elicitation_outcome(&json!(5), &card, &params, false, None, Some("cancel"));
        assert_eq!(cancelled["result"], json!({"action": "cancel"}));

        let skipped = json!({
            "_selectedIndices": {"0": [0]},
            "_otherTexts": {"2": "notes"},
            "_skippedIndices": [1]
        });
        let ok_skip = elicitation_outcome(&json!(5), &card, &params, true, Some(&skipped), None);
        assert_eq!(ok_skip["result"], json!({
            "action": "accept",
            "content": {
                "q1": "1",
                "q2": [],
                "q10": "notes"
            }
        }));
    }

    #[test]
    fn optionsless_q_other_elicitation_and_timeline_writeback_work() {
        let params = json!({
            "mode": "form",
            "toolCallId": "tc-free",
            "message": "The agent has a question.",
            "requestedSchema": {
                "type": "object",
                "properties": {
                    "q1_other": { "type": "string", "title": "What port?" }
                }
            }
        });
        let card = elicitation_payload(SessionKind::Antigravity, "acp-12", &params);
        assert_eq!(card["tool_name"], "AskUserQuestion");
        assert_eq!(card["input"]["questions"][0]["options"], json!([]));
        assert_eq!(card["input"]["questions"][0]["allowOther"], true);
        let ok = elicitation_outcome(&json!(12), &card, &params, true, Some(&json!({"_otherTexts": {"0": "8080"}})), None);
        assert_eq!(ok["result"], json!({"action": "accept", "content": {"q1_other": "8080"}}));

        let mut mapper = Mapper::default();
        let rows = mapper.update(&json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "tc-free",
            "title": "ask_question",
            "kind": "other",
            "status": "in_progress",
            "rawInput": {"questions": [{"question": "What port?", "is_multi_select": false, "options": []}]}
        }));
        assert!(matches!(&rows[0], ChatRow::Tool { name, .. } if name == "AskUserQuestion"));
        let answered = mapper.record_ask_question_answer(Some(&json!({
            "answers": {"What port?": "8080"},
            "_answerList": ["8080"]
        })), true).expect("recorded row");
        let ChatRow::Tool { name, input, .. } = answered else { panic!() };
        assert_eq!(name, "AskUserQuestion");
        assert_eq!(input["answers"], json!({"What port?": "8080"}));
        assert_eq!(input["_answerList"], json!(["8080"]));

        let trailing = mapper.update(&json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": "tc-free",
            "status": "completed",
            "rawInput": {}
        }));
        let ChatRow::Tool { input: after_update, .. } = &trailing[0] else { panic!() };
        assert_eq!(after_update["answers"], json!({"What port?": "8080"}));
        assert_eq!(after_update["questions"][0]["header"], "Q1");

        let mut replay = Mapper::replay();
        let _ = replay.update(&json!({
            "sessionUpdate": "user_message_chunk",
            "content": {"type": "image", "mimeType": "image/png", "data": "UE5H"}
        }));
        let closed = replay.finish().expect("closed user row");
        let ChatRow::User { text, images, .. } = closed else { panic!() };
        assert_eq!(text, "");
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].data, "UE5H");
    }

    #[test]
    fn generic_elicitation_preserves_mcp_card_and_tool_card_normalizes_ask_question() {
        let params = json!({
            "mode": "form",
            "serverName": "github",
            "message": "Sign in",
            "requestedSchema": {"type": "object", "properties": {"token": {"type": "string"}}}
        });
        let card = elicitation_payload(SessionKind::Antigravity, "acp-9", &params);
        assert_eq!(card["subtype"], "elicitation");
        assert_eq!(card["tool_name"], "MCP");
        let resp = elicitation_outcome(&json!(9), &card, &params, true, Some(&json!({"token": "abc"})), None);
        assert_eq!(resp["result"], json!({"action": "accept", "content": {"token": "abc"}}));

        let mut state = ToolState::default();
        state.merge(&json!({
            "title": "ask_question",
            "kind": "other",
            "rawInput": {"questions": [{"question": "Pick?", "multi_select": true, "allow_other": false}]}
        }));
        let (name, input) = state.card();
        assert_eq!(name, "AskUserQuestion");
        assert_eq!(input["questions"][0]["multiSelect"], true);
        assert_eq!(input["questions"][0]["allowOther"], false);
    }

    #[test]
    fn journal_paths_reject_names_that_leave_the_directory() {
        let root = Path::new("/data");
        assert_eq!(journal_path(root, SessionKind::Kimi, "a1-b2").unwrap(), root.join("chat-acp/kimi/a1-b2.jsonl"));
        assert!(journal_path(root, SessionKind::Antigravity, "a1-b2").is_none());
        for bad in ["", "../x", "a/b", ".hidden", "a\\b"] { assert!(journal_path(root, SessionKind::Kimi, bad).is_none()); }
    }

    #[test]
    fn a_journal_rewrite_replaces_and_appends_extend() {
        let dir = std::env::temp_dir().join(format!("acp-journal-{}", uuid::Uuid::new_v4()));
        let path = dir.join("kimi").join("s.jsonl");
        append_journal(&path, &["one".into()]).unwrap();
        append_journal(&path, &["two".into()]).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\ntwo\n");
        rewrite_journal(&path, &["three".into()]).unwrap();
        append_journal(&path, &["four".into()]).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "three\nfour\n");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
