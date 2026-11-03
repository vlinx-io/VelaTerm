//! Agent Client Protocol (ACP): the wire format Copilot, Cursor, Kimi, Grok and DeepSeek Harness share,
//! and its timeline.
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
//! **DeepSeek Harness** (`dsh --profile acp`) continues a conversation with `session/resume`, which replays
//! nothing; its own session log is the history (`deepseek_history`). It tags message chunks with the
//! assistant message's id and names each tool call after the dsh tool, so for it the mapper takes row ids
//! from those message ids and draws calls by tool name, which is what lets a live turn and its log replay
//! to the same rows.
//!
//! Verified against Copilot 1.0.63 (complete turns and `session/load`), DeepSeek Harness 0.2.1-rc.2 and
//! rc.3 (complete turns, permission requests, cancellation, `session/resume`), and the initialize handshake
//! of Cursor 2026.07.01, Kimi 2.1.1 and Grok 1.0.46.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::engine::ChatRow;
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
        SessionKind::Copilot => "Copilot",
        SessionKind::Cursor => "Cursor",
        SessionKind::Kimi => "Kimi",
        SessionKind::Grok => "Grok",
        SessionKind::Deepseek => "DeepSeek",
        _ => "Agent",
    }
}

/// Command-line arguments that start the agent's ACP server.
///
/// Only Copilot takes model and effort at launch: its ACP sessions publish no model selector, while the
/// others publish one and are switched over the protocol once the session is open. Copilot's options are
/// global, so a session's own launch arguments still apply; the others' arguments belong to their terminal
/// interface and would be misread by the server subcommand.
pub fn launch_args(kind: SessionKind, model: Option<&str>, effort: Option<&str>, extra: &[String]) -> Result<Vec<String>, String> {
    let mut args: Vec<String> = match kind {
        SessionKind::Copilot => vec!["--acp".into()],
        SessionKind::Cursor => vec!["acp".into()],
        SessionKind::Kimi => vec!["acp".into()],
        SessionKind::Grok => vec!["agent".into(), "stdio".into()],
        SessionKind::Deepseek => vec!["--profile".into(), "acp".into()],
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
    // dsh's launcher options (`--patch <overlay>` and the like) apply to the ACP profile as they would to any
    // other; the profile is VelaTerm's to choose. Model and reasoning are switched over the protocol.
    if kind == SessionKind::Deepseek {
        for arg in extra {
            let flag = arg.split('=').next().unwrap_or(arg);
            if matches!(flag, "--profile" | "--from-default-profile" | "--dump-config" | "--dump-config-schema" | "--dump-default-config") {
                return Err(format!("The DeepSeek conversation view manages {flag}; remove it from custom arguments"));
            }
        }
        args.extend_from_slice(extra);
    }
    Ok(args)
}

/// Whether a message can go out in this conversation. ACP has no way into a running turn.
pub fn validate(behavior: &str, image_count: usize, images_supported: bool) -> Result<(), String> {
    if behavior == "steer" {
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
        "clientCapabilities": {"fs": {"readTextFile": false, "writeTextFile": false}, "terminal": false},
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

/// Whether an error says the agent has no API key to call its model with. DeepSeek Harness reports it as
/// an internal error whose text names the missing key (0.2.1-rc.3: `no API key for provider route`).
pub fn api_key_missing(error: &Value) -> bool {
    error_text(error).to_ascii_lowercase().contains("no api key")
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

/// The model name a reply is credited to. DeepSeek Harness names a model by its provider route and model id
/// as a JSON pair (`["deepseek-official","deepseek-v4-flash"]`); the reply shows the model id, as its log
/// records it. Other agents' model values are already names.
pub fn reply_model(kind: SessionKind, value: &str) -> String {
    if kind == SessionKind::Deepseek {
        if let Ok(Value::Array(parts)) = serde_json::from_str::<Value>(value) {
            if let Some(model) = parts.last().and_then(Value::as_str).filter(|model| !model.is_empty()) { return model.to_string(); }
        }
    }
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
            let listed: Vec<Choice> = distinct(models.get("availableModels").and_then(Value::as_array).into_iter().flatten()
                .filter_map(|model| choice(model, "modelId")).collect());
            if !listed.is_empty() {
                self.models = listed;
                self.models_from_state = true;
            }
            self.model = models.get("currentModelId").and_then(Value::as_str).map(str::to_string).or(self.model.take());
        }
        if let Some(options) = result.get("configOptions").and_then(Value::as_array) {
            self.absorb_options(options);
        }
    }

    /// Fold in a `config_option_update`, which always carries the complete set.
    pub fn absorb_options(&mut self, options: &[Value]) {
        for option in options {
            if option.get("type").and_then(Value::as_str) != Some("select") { continue; }
            let Some(id) = option.get("id").and_then(Value::as_str) else { continue };
            let choices = distinct(flatten_options(option.get("options")));
            let current = option.get("currentValue").and_then(Value::as_str).map(str::to_string);
            match option.get("category").and_then(Value::as_str) {
                Some("mode") => {
                    self.mode_option = Some(id.into());
                    if !choices.is_empty() { self.modes = choices; }
                    if current.is_some() { self.mode = current; }
                }
                Some("model") => {
                    self.model_option = Some(id.into());
                    if !choices.is_empty() { self.models = choices; self.models_from_state = false; }
                    if current.is_some() { self.model = current; }
                }
                Some("thought_level") => {
                    self.effort_option = Some(id.into());
                    self.efforts = choices;
                    self.effort = current;
                }
                _ => {}
            }
        }
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
/// A request may name only the call (DeepSeek Harness sends just its `toolCallId`), so what the mapper
/// already knows about that call is the base the request's own fields are laid over.
pub fn permission_payload(kind: SessionKind, request_id: &str, params: &Value, mapper: Option<&Mapper>) -> Value {
    let call = params.get("toolCall").cloned().unwrap_or(Value::Null);
    let known = call.get("toolCallId").and_then(Value::as_str)
        .and_then(|id| mapper.and_then(|mapper| mapper.tools.get(id)).cloned());
    let mut tool = known.unwrap_or_default();
    tool.merge(&call);
    let (name, input) = tool.card(kind == SessionKind::Deepseek);
    let options = params.get("options").cloned().unwrap_or_else(|| json!([]));
    let mut payload = json!({
        "id": request_id,
        "tool_name": name,
        "display_name": label(kind),
        "input": input,
        "_acpOptions": options,
    });
    // DeepSeek Harness asks when a command must leave its sandbox, and the model states why in the call.
    let reason = tool.raw_input.as_ref().and_then(|input| input.get("justification")).and_then(Value::as_str)
        .filter(|reason| kind == SessionKind::Deepseek && !reason.trim().is_empty());
    if let Some(reason) = reason {
        payload["description"] = json!(reason);
    } else if let Some(title) = tool.title.as_deref().filter(|title| !title.is_empty() && kind != SessionKind::Deepseek) {
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
        if let Some(input) = update.get("rawInput").filter(|input| !input.is_null()) { self.raw_input = Some(input.clone()); }
        if let Some(output) = update.get("rawOutput").filter(|output| !output.is_null()) { self.raw_output = Some(output.clone()); }
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
    fn card(&self, by_name: bool) -> (String, Value) {
        let raw = self.raw_input.clone().unwrap_or_else(|| json!({}));
        // DeepSeek Harness reports every call as kind `other`, titled with the tool's own name; the name is
        // what tells its tools apart, mapped the same way its log replay maps them.
        if by_name {
            if let Some(title) = self.title.as_deref().filter(|title| !title.is_empty()) {
                return super::deepseek_history::tool_card(title, &raw);
            }
        }
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
                if let Some(path) = &path { object.entry("path").or_insert(json!(path)); }
                short_title(&title, "Tool")
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
    tools: HashMap<String, ToolState>,
    tool_order: Vec<String>,
    turn: u64,
    /// True while replaying a recording, where the user's own messages arrive as chunks. Live, the engine
    /// draws a prompt itself when it sends one, and an agent's echo would draw it a second time.
    pub replaying: bool,
    /// Timestamp applied to rows created from the next updates, from a journal line.
    pub at: Option<i64>,
    /// Take row ids from each chunk's `messageId` and draw tool calls by name, as DeepSeek Harness needs.
    pub native_ids: bool,
}

impl Mapper {
    /// A mapper for a recorded conversation, which draws the user's own messages too.
    pub fn replay() -> Self { Mapper { replaying: true, ..Mapper::default() } }

    /// A live mapper for one agent's conversation.
    pub fn for_kind(kind: SessionKind) -> Self { Mapper { native_ids: kind == SessionKind::Deepseek, ..Mapper::default() } }

    /// A new prompt goes out: whatever was streaming is finished, and the next plan belongs to this turn.
    pub fn begin_turn(&mut self) {
        self.open = None;
        self.turn += 1;
    }

    /// How many prompts this mapper has seen, which addresses the rows that belong to one turn.
    pub fn turn(&self) -> u64 { self.turn }

    /// Close the streaming block and return its final row, so the view stops drawing it as live.
    pub fn finish(&mut self) -> Option<ChatRow> {
        let (block, id, text, at) = self.open.take()?;
        Some(text_row(block, id, text, false, at))
    }

    /// Tool calls the agent left running when its turn ended, marked so they stop spinning.
    pub fn settle_running(&mut self, canceled: bool) -> Vec<ChatRow> {
        let mut rows = Vec::new();
        for id in self.tool_order.clone() {
            let Some(tool) = self.tools.get_mut(&id) else { continue };
            if tool.status() != "running" { continue; }
            tool.status = Some(if canceled { "canceled".into() } else { "completed".into() });
            let tool = tool.clone();
            rows.push(tool_row(&id, &tool, self.native_ids));
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
                // A replayed user message starts a turn, as a live prompt does.
                if !matches!(self.open, Some((Block::User, _, _, _))) {
                    rows.extend(self.finish());
                    self.begin_turn();
                }
                if let Some(text) = content_text(update.get("content")) { rows.push(self.append(Block::User, &text, None)); }
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
                self.remember(id, tool.clone());
                rows.push(tool_row(id, &tool, self.native_ids));
            }
            "tool_call_update" => {
                let Some(id) = update.get("toolCallId").and_then(Value::as_str) else { return rows };
                // An update for a call never announced is an internal tool the agent keeps to itself.
                let Some(tool) = self.tools.get_mut(id) else { return rows };
                tool.merge(update);
                let tool = tool.clone();
                rows.push(tool_row(id, &tool, self.native_ids));
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

    /// The chunk's own message id, when this agent's ids name rows.
    fn message_id(&self, update: &Value) -> Option<String> {
        if !self.native_ids { return None; }
        update.get("messageId").and_then(Value::as_str).filter(|id| !id.is_empty()).map(str::to_string)
    }

    /// Append to the open block of this kind, closing any other kind first. A block named by a message id
    /// is also closed by a chunk of another message, so two messages in a row never share a row.
    fn append_closing(&mut self, block: Block, text: &str, message: Option<String>) -> Vec<ChatRow> {
        let mut rows = Vec::new();
        let named = message.as_deref().map(|message| block_id(block, message));
        let other = self.open.as_ref().is_some_and(|(open, id, _, _)| *open != block || named.as_ref().is_some_and(|named| named != id));
        if other { rows.extend(self.finish()); }
        rows.push(self.append(block, text, named));
        rows
    }

    fn append(&mut self, block: Block, text: &str, named: Option<String>) -> ChatRow {
        if self.open.as_ref().is_none_or(|(open, _, _, _)| *open != block) {
            self.seq += 1;
            let prefix = match block { Block::User => "u", Block::Assistant => "a", Block::Reasoning => "r" };
            let id = named.unwrap_or_else(|| format!("acp-{prefix}{}", self.seq));
            self.open = Some((block, id, String::new(), self.at));
        }
        let (_, id, buffer, at) = self.open.as_mut().expect("a block was just opened");
        buffer.push_str(text);
        text_row(block, id.clone(), buffer.clone(), true, *at)
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
fn block_id(block: Block, message: &str) -> String {
    match block {
        Block::Reasoning => super::deepseek_history::reasoning_id(message),
        _ => super::deepseek_history::answer_id(message),
    }
}

fn text_row(block: Block, id: String, text: String, streaming: bool, at: Option<i64>) -> ChatRow {
    match block {
        Block::User => ChatRow::User { id, text, images: Vec::new(), at },
        Block::Assistant => ChatRow::Assistant { id, text, streaming, model: None, at, duration_ms: None },
        Block::Reasoning => ChatRow::Reasoning { id, text, streaming },
    }
}

fn tool_row(id: &str, tool: &ToolState, by_name: bool) -> ChatRow {
    let (name, input) = tool.card(by_name);
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
            for row in mapper.finish() { place(row, &mut rows); }
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
    if kind == SessionKind::Deepseek { return super::deepseek_history::replay(native_id); }
    let path = if kind == SessionKind::Grok {
        crate::agent::resume::find_grok_updates(native_id)
    } else {
        data_dir.and_then(|dir| journal_path(dir, kind, native_id)).filter(|path| path.is_file())
    };
    let path = path.ok_or("source_missing: The conversation recording could not be found")?;
    let mut rows = history_rows(&read_recording(&path)?);
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
        let card = |update: Value| { let mut tool = ToolState::default(); tool.merge(&update); tool.card(false) };
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
    fn a_deepseek_request_naming_only_its_call_is_drawn_from_the_call_already_shown() {
        let mut mapper = Mapper::for_kind(SessionKind::Deepseek);
        mapper.update(&json!({"sessionUpdate":"tool_call","toolCallId":"c1","title":"bash","kind":"other","status":"in_progress",
            "rawInput":{"command":"touch /x","justification":"The file is outside the workspace."}}));
        let params = json!({"sessionId":"s","toolCall":{"toolCallId":"c1"},"options":[{"optionId":"allow-once","kind":"allow_once"}]});
        let payload = permission_payload(SessionKind::Deepseek, "acp-0", &params, Some(&mapper));
        assert_eq!((payload["tool_name"].as_str(), payload["input"]["command"].as_str()), (Some("Bash"), Some("touch /x")));
        assert_eq!(payload["description"], "The file is outside the workspace.");
        assert_eq!(payload["display_name"], "DeepSeek");
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
        let mut state = Catalog::default();
        state.absorb_session(&json!({"models":{"availableModels":[{"modelId":"g1","name":"G1"},{"modelId":"g2","name":"G2"}],"currentModelId":"g1"}}));
        assert!(state.model_switchable() && state.models_from_state);
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
        assert_eq!(launch_args(SessionKind::Cursor, None, None, &["--force".into()]).unwrap(), ["acp"]);
        assert_eq!(launch_args(SessionKind::Copilot, Some("gpt-5"), Some("high"), &["--allow-all-paths".into()]).unwrap(),
            ["--acp", "--model", "gpt-5", "--effort", "high", "--allow-all-paths"]);
        assert!(launch_args(SessionKind::Copilot, None, None, &["--resume=abc".into()]).is_err());
        assert_eq!(launch_args(SessionKind::Deepseek, Some("x"), None, &["--patch".into(), "/p.yml".into()]).unwrap(), ["--profile", "acp", "--patch", "/p.yml"]);
        assert!(launch_args(SessionKind::Deepseek, None, None, &["--profile".into(), "web".into()]).is_err());
        assert_eq!(reply_model(SessionKind::Deepseek, r#"["deepseek-official","deepseek-v4-pro"]"#), "deepseek-v4-pro");
        assert_eq!(reply_model(SessionKind::Kimi, "kimi-k2"), "kimi-k2");
        assert!(validate("steer", 0, true).is_err());
        assert!(validate("queue", 1, false).is_err());
        assert!(validate("interrupt", 1, true).is_ok());
    }

    #[test]
    fn journal_paths_reject_names_that_leave_the_directory() {
        let root = Path::new("/data");
        assert_eq!(journal_path(root, SessionKind::Kimi, "a1-b2").unwrap(), root.join("chat-acp/kimi/a1-b2.jsonl"));
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
