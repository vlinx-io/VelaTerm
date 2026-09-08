//! The wire format between VelaTerm and an agent CLI running in streaming JSON mode.
//!
//! Two channels share one pair of pipes. **Messages** flow one way: we write a user turn, the agent writes
//! back what it is doing. **Control requests** flow both ways and expect an answer — we ask it to interrupt
//! or change model, it asks us whether a tool may run. Both are single JSON objects, one per line.
//!
//! Verified against claude 2.1.251. Fields we do not use are ignored rather than rejected, so a newer CLI
//! that adds to a message still parses.

use serde_json::{json, Value};

/// Permission modes the agent's command line accepts. `skip` — what a terminal-driven session stores when
/// the user turns confirmations off — is not among them, so it is translated rather than passed through.
const CLI_PERMISSION_MODES: &[&str] = &["plan", "default", "acceptEdits", "auto", "bypassPermissions"];

/// Translate a session's stored permission mode into one the agent accepts, or None to let it decide.
///
/// The stored value predates the chat engine: it is `skip` or nothing, and means "stop asking me". Passing
/// `skip` through would abort the launch, since the agent's own vocabulary calls that `bypassPermissions`.
pub fn cli_permission_mode(stored: Option<&str>) -> Option<&str> {
    match stored.map(str::trim) {
        Some("skip") => Some("bypassPermissions"),
        Some(mode) if CLI_PERMISSION_MODES.contains(&mode) => Some(mode),
        _ => None,
    }
}

/// Command line that starts an agent in streaming JSON mode.
///
/// Every flag here is load-bearing:
/// - `--print` with the two stream formats is what makes the process a protocol peer instead of a TUI;
///   `--verbose` is required by the CLI whenever `--print` prints stream JSON.
/// - `--include-partial-messages` adds the per-token events the view needs to show an answer as it lands.
/// - `--permission-prompt-tool stdio` routes permission questions to us as control requests. Without it the
///   CLI has no one to ask and quietly denies anything not already allowed.
/// - `--thinking adaptive` together with `--thinking-display summarized` is what puts the reasoning text in
///   the stream. Neither works alone: with the first flag only, the thinking blocks and their deltas still
///   arrive, but every `thinking` field is an empty string, so the view can tell that the agent thought and
///   can show nothing of it. Measured on claude 2.1.259; the same pair is recorded in the protocol-engine
///   design document.
pub fn launch_args(
    resume: Option<&str>,
    model: Option<&str>,
    effort: Option<&str>,
    permission_mode: Option<&str>,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "--print".into(),
        "--verbose".into(),
        "--input-format".into(),
        "stream-json".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--include-partial-messages".into(),
        "--permission-prompt-tool".into(),
        "stdio".into(),
        "--thinking".into(),
        "adaptive".into(),
        "--thinking-display".into(),
        "summarized".into(),
    ];
    if let Some(id) = resume {
        args.push("--resume".into());
        args.push(id.into());
    }
    if let Some(model) = model {
        args.push("--model".into());
        args.push(model.into());
    }
    // "off" is not a level Claude knows: it switches thinking off through a control request once the
    // process is up, and the launch carries no `--effort` at all.
    if let Some(effort) = effort.filter(|level| *level != THINKING_OFF) {
        args.push("--effort".into());
        args.push(effort.into());
    }
    if let Some(mode) = permission_mode {
        args.push("--permission-mode".into());
        args.push(mode.into());
    }
    args
}

/// One line the agent wrote to stdout, reduced to the cases we act on.
pub enum Incoming {
    /// Session start: carries the agent's own session id, which is also the name of its recording file.
    Init { session_id: String, model: Option<String> },
    /// A per-token fragment of the message being written right now.
    Delta(Delta),
    /// A complete assistant message. Authoritative: it replaces whatever the deltas built.
    ///
    /// `parent` is the tool call this message belongs to. It is set only on the frames a subagent
    /// produces — the agent runs one inside a `Task` call and reports its work under that call's id — and
    /// is what keeps a subagent's chatter out of the main conversation.
    Assistant { message: Value, parent: Option<String> },
    /// What a slash command the CLI answered itself printed, such as `/effort` or `/context`.
    ///
    /// The CLI reports it through an assistant frame, but nothing was inferred behind it: the frame
    /// names `<synthetic>` as its model, exactly as a refused turn does. Only the `local_command_source`
    /// field tells the two apart, so the parse splits them here rather than leaving the engine to guess.
    LocalCommand { id: String, text: String },
    /// A user-role line. In practice these carry tool results, not user speech.
    User { message: Value, parent: Option<String> },
    /// The agent started summarizing the conversation to make room in its context window.
    CompactionStarted,
    /// It finished. `trigger` is `manual` when a person asked for it, `auto` when the window filled up.
    CompactionFinished { trigger: Option<String>, pre_tokens: Option<u64> },
    /// A Claude task-protocol lifecycle frame.
    ///
    /// These frames describe subagents that may not emit any sidechain messages at all, especially once
    /// backgrounded. Keep the complete object so the engine can merge identity, status, and usage without
    /// making this wire parser mirror every optional field Claude adds to the task protocol.
    Task { subtype: String, message: Value },
    /// A remark the agent made about its own run: a fallback model, a denied tool, a warning.
    Notice { id: String, message: String },
    /// A failed API call about to be retried. Transient: the next frame supersedes it.
    ApiRetry { attempt: u64, max_retries: u64, delay_ms: u64, message: String },
    /// A loop-side notification the interactive REPL would show in its queue.
    Notification { text: String, priority: String, timeout_ms: Option<u64> },
    /// The subscription rate-limit windows as last reported.
    RateLimit(Value),
    /// Every live background task after a change. Replace semantics.
    BackgroundTasks(Vec<Value>),
    /// A turn finished, successfully or not. `model_usage` is the per-model cost and context window.
    Result {
        subtype: String,
        duration_ms: Option<u64>,
        total_cost_usd: Option<f64>,
        model_usage: Value,
    },
    /// The agent is asking us something and is waiting for an answer.
    ControlRequest { request_id: String, request: Value },
    /// The answer to something we asked.
    ControlResponse { request_id: String, response: Value, error: Option<String> },
    /// A line we have no use for.
    Other,
}

/// A fragment of the message currently being written.
pub struct Delta {
    /// The tool call this fragment belongs to, set only for a subagent's own output. See `Assistant`.
    pub parent: Option<String>,
    /// Id of the message being built, used to reconcile with the complete message that follows.
    pub message_id: Option<String>,
    /// Position of the content block within that message.
    pub index: usize,
    pub kind: DeltaKind,
}

pub enum DeltaKind {
    /// A new message began. Only this event names the message; the fragments that follow do not, so the
    /// reader has to carry the id forward to address the rows they build.
    MessageStart,
    /// A block began. `block_type` is `text`, `thinking`, or `tool_use`.
    Start { block_type: String, tool_name: Option<String>, tool_use_id: Option<String> },
    /// More prose for the block.
    Text(String),
    /// More reasoning for the block.
    Thinking(String),
    /// The block is complete.
    Stop,
    /// The whole message is complete.
    MessageStop,
}

/// Parse one stdout line. Returns `Other` for anything unrecognized, including malformed JSON, because a
/// single bad line must never take down a running conversation.
pub fn parse_line(line: &str) -> Incoming {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Incoming::Other;
    };
    match v.get("type").and_then(Value::as_str) {
        Some("system") => {
            match v.get("subtype").and_then(Value::as_str) {
                Some("init") => {}
                // The only status worth a timeline row. Every other value — `requesting`, and the null
                // that reports the result — describes a step the reader never asked about.
                Some("status") => {
                    return match v.get("status").and_then(Value::as_str) {
                        Some("compacting") => Incoming::CompactionStarted,
                        _ => Incoming::Other,
                    };
                }
                Some("compact_boundary") => return parse_compact_boundary(v),
                Some(subtype @ ("task_started" | "task_updated" | "task_progress" | "task_notification")) => {
                    return Incoming::Task { subtype: subtype.to_string(), message: v };
                }
                Some("api_retry") => {
                    return Incoming::ApiRetry {
                        attempt: v.get("attempt").and_then(Value::as_u64).unwrap_or(0),
                        max_retries: v.get("max_retries").and_then(Value::as_u64).unwrap_or(0),
                        delay_ms: v.get("retry_delay_ms").and_then(Value::as_u64).unwrap_or(0),
                        message: api_error_text(v.get("error")),
                    };
                }
                Some("api_error") => {
                    return notice(&v, format!("API error: {}", api_error_text(v.get("error"))));
                }
                Some("model_fallback") => {
                    let message = match v.get("content").and_then(Value::as_str) {
                        Some(text) if !text.trim().is_empty() => text.trim().to_string(),
                        _ => format!(
                            "Switched from {} to {} ({})",
                            v.get("original_model").and_then(Value::as_str).unwrap_or("the requested model"),
                            v.get("fallback_model").and_then(Value::as_str).unwrap_or("the fallback model"),
                            v.get("trigger").and_then(Value::as_str).unwrap_or("fallback").replace('_', " "),
                        ),
                    };
                    return notice(&v, message);
                }
                Some("model_refusal_fallback") => {
                    let message = match v.get("direction").and_then(Value::as_str) {
                        Some("retry") => "The model declined to answer; retrying with the fallback model",
                        Some("sticky") => "The model declined to answer; the fallback model takes over for this session",
                        Some("revert") => "Back on the original model after the fallback answered",
                        _ => "The model declined to answer; a fallback model was used",
                    };
                    return notice(&v, message.to_string());
                }
                Some("permission_denied") => {
                    let tool = v.get("tool_name").and_then(Value::as_str).unwrap_or("A tool");
                    let message = match v.get("decision_reason").and_then(Value::as_str) {
                        Some(reason) if !reason.trim().is_empty() => format!("{tool} was denied: {}", reason.trim()),
                        _ => format!("{tool} was denied"),
                    };
                    return notice(&v, message);
                }
                Some("informational") => {
                    // `info` exists for transcript mode and `notice` is drawn in inactive gray in the
                    // terminal; only the two prominent levels are worth a row here.
                    return match (v.get("level").and_then(Value::as_str), v.get("content").and_then(Value::as_str)) {
                        (Some("warning" | "suggestion"), Some(text)) if !text.trim().is_empty() => {
                            notice(&v, text.trim().to_string())
                        }
                        _ => Incoming::Other,
                    };
                }
                Some("notification") => {
                    let Some(text) = v.get("text").and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty()) else {
                        return Incoming::Other;
                    };
                    return Incoming::Notification {
                        text: text.to_string(),
                        priority: v.get("priority").and_then(Value::as_str).unwrap_or("medium").to_string(),
                        timeout_ms: v.get("timeout_ms").and_then(Value::as_u64),
                    };
                }
                Some("background_tasks_changed") => {
                    return Incoming::BackgroundTasks(
                        v.get("tasks").and_then(Value::as_array).cloned().unwrap_or_default(),
                    );
                }
                _ => return Incoming::Other,
            }
            let Some(session_id) = v.get("session_id").and_then(Value::as_str) else {
                return Incoming::Other;
            };
            Incoming::Init {
                session_id: session_id.to_string(),
                model: v.get("model").and_then(Value::as_str).map(str::to_string),
            }
        }
        Some("stream_event") => parse_stream_event(&v),
        Some("assistant") => match v.get("message") {
            Some(m) => match local_command(&v, m) {
                Some(row) => row,
                None => Incoming::Assistant { message: m.clone(), parent: parent_tool_use_id(&v) },
            },
            None => Incoming::Other,
        },
        Some("user") => match v.get("message") {
            Some(m) => Incoming::User { message: m.clone(), parent: parent_tool_use_id(&v) },
            None => Incoming::Other,
        },
        Some("result") => Incoming::Result {
            subtype: v
                .get("subtype")
                .and_then(Value::as_str)
                .unwrap_or("success")
                .to_string(),
            duration_ms: v.get("duration_ms").and_then(Value::as_u64),
            total_cost_usd: v.get("total_cost_usd").and_then(Value::as_f64),
            model_usage: v.get("modelUsage").cloned().unwrap_or(Value::Null),
        },
        Some("rate_limit_event") => match v.get("rate_limit_info") {
            Some(info) if info.is_object() => Incoming::RateLimit(info.clone()),
            _ => Incoming::Other,
        },
        Some("control_request") => {
            let (Some(id), Some(req)) = (
                v.get("request_id").and_then(Value::as_str),
                v.get("request"),
            ) else {
                return Incoming::Other;
            };
            Incoming::ControlRequest { request_id: id.to_string(), request: req.clone() }
        }
        Some("control_response") => {
            let Some(resp) = v.get("response") else {
                return Incoming::Other;
            };
            let Some(id) = resp.get("request_id").and_then(Value::as_str) else {
                return Incoming::Other;
            };
            // A failed control request answers with subtype "error" and a message instead of a payload.
            let error = (resp.get("subtype").and_then(Value::as_str) == Some("error"))
                .then(|| {
                    resp.get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("control request failed")
                        .to_string()
                });
            Incoming::ControlResponse {
                request_id: id.to_string(),
                response: resp.get("response").cloned().unwrap_or(Value::Null),
                error,
            }
        }
        _ => Incoming::Other,
    }
}

/// A locally answered slash command's output, or None for an ordinary assistant frame.
///
/// The frame's own text blocks carry the output already stripped of the `<local-command-stdout>` wrapper
/// the recording keeps, so they are what the row shows.
fn local_command(v: &Value, message: &Value) -> Option<Incoming> {
    v.get("local_command_source").and_then(Value::as_str)?;
    let text = message
        .get("content")
        .and_then(Value::as_array)?
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if text.is_empty() {
        return None;
    }
    // Prefixed so the row can never collide with an assistant row built from message ids.
    let id = message.get("id").and_then(Value::as_str).unwrap_or("local");
    Some(Incoming::LocalCommand { id: format!("lc-{id}"), text })
}

/// The tool call a frame belongs to. Present and non-null only on a subagent's own output.
/// A notice row keyed by the frame's own uuid, so a frame delivered twice lands on one row.
fn notice(v: &Value, message: String) -> Incoming {
    let key = v
        .get("uuid")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("{:x}", text_key(&message)));
    Incoming::Notice { id: format!("n-{key}"), message }
}

/// A stable small hash for notices that arrive without a uuid. Only row identity, nothing else.
fn text_key(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The display string of an `api_error` / `api_retry` error object, or a plain string.
fn api_error_text(error: Option<&Value>) -> String {
    match error {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(object) => ["formatted", "message"]
            .iter()
            .find_map(|key| object.get(*key).and_then(Value::as_str))
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .unwrap_or("unknown error")
            .to_string(),
        None => "unknown error".to_string(),
    }
}

fn parent_tool_use_id(v: &Value) -> Option<String> {
    v.get("parent_tool_use_id")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// The line that closes a compaction, with whatever its metadata reports.
///
/// The field has been spelled three ways across CLI versions and none of them is required, so every
/// spelling is accepted and a boundary carrying no metadata at all still produces the marker: that the
/// context was compacted is the part the reader needs, the token count is a detail.
fn parse_compact_boundary(v: Value) -> Incoming {
    let meta = ["compact_metadata", "compactMetadata", "compactionMetadata"]
        .iter()
        .find_map(|key| v.get(*key));
    let trigger = meta
        .and_then(|m| m.get("trigger"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let pre_tokens = meta.and_then(|m| {
        m.get("pre_tokens")
            .or_else(|| m.get("preTokens"))
            .and_then(Value::as_u64)
    });
    Incoming::CompactionFinished { trigger, pre_tokens }
}

fn parse_stream_event(v: &Value) -> Incoming {
    let Some(ev) = v.get("event") else {
        return Incoming::Other;
    };
    let message_id = ev
        .get("message")
        .and_then(|m| m.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let index = ev.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
    let kind = match ev.get("type").and_then(Value::as_str) {
        Some("message_start") => DeltaKind::MessageStart,
        Some("content_block_start") => {
            let block = ev.get("content_block");
            DeltaKind::Start {
                block_type: block
                    .and_then(|b| b.get("type"))
                    .and_then(Value::as_str)
                    .unwrap_or("text")
                    .to_string(),
                tool_name: block
                    .and_then(|b| b.get("name"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                tool_use_id: block
                    .and_then(|b| b.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
            }
        }
        Some("content_block_delta") => {
            let d = ev.get("delta");
            match d.and_then(|d| d.get("type")).and_then(Value::as_str) {
                Some("text_delta") => DeltaKind::Text(
                    d.and_then(|d| d.get("text")).and_then(Value::as_str).unwrap_or("").to_string(),
                ),
                Some("thinking_delta") => DeltaKind::Thinking(
                    d.and_then(|d| d.get("thinking"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                ),
                // Tool arguments also arrive in fragments. They are not shown while incomplete, so the
                // authoritative message is what fills the card in.
                _ => return Incoming::Other,
            }
        }
        Some("content_block_stop") => DeltaKind::Stop,
        Some("message_stop") => DeltaKind::MessageStop,
        _ => return Incoming::Other,
    };
    Incoming::Delta(Delta { parent: parent_tool_use_id(v), message_id, index, kind })
}

// ─────────────────────────── Outgoing ───────────────────────────

/// One image attached to a user turn.
///
/// The bytes travel as base64 with no data-URL prefix, exactly as the agent's message format wants them.
/// Kept out of `engine` because this is a wire shape: a timeline row carries the same struct only so the
/// picture can be drawn back where it was sent.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatImage {
    /// The image's MIME type, such as `image/png`.
    pub mime_type: String,
    /// Base64 of the file's bytes, without the `data:` prefix.
    pub data: String,
}

/// A user turn, with any images that were attached to it.
///
/// Images use the same block the Anthropic message format uses — `{"type":"image","source":{"type":
/// "base64",...}}`. Verified against claude 2.1.258: the SDK's own shorter `{"type":"image","data",
/// "mimeType"}` is *not* accepted on this wire and ends the turn with `error_during_execution`, so the
/// long form is the only one that works here.
///
/// The text block is left out when there is nothing written, since a caption nobody typed is not worth a
/// block; a message with neither text nor images never gets this far.
pub fn user_message(text: &str, images: &[ChatImage]) -> Value {
    let mut content: Vec<Value> = Vec::with_capacity(images.len() + 1);
    if !text.is_empty() {
        content.push(json!({"type":"text","text":text}));
    }
    for image in images {
        content.push(json!({
            "type": "image",
            "source": {"type":"base64","media_type":image.mime_type,"data":image.data},
        }));
    }
    json!({"type":"user","message":{"role":"user","content":content}})
}

/// Our side of a control request. `id` correlates the answer that comes back.
pub fn control_request(id: &str, request: Value) -> Value {
    json!({"type":"control_request","request_id":id,"request":request})
}

/// The handshake. Its answer carries the agent's slash commands and skills, which the composer offers.
pub fn initialize() -> Value {
    json!({"subtype":"initialize","hooks":{}})
}

pub fn interrupt() -> Value {
    json!({"subtype":"interrupt"})
}

/// Roll the active Claude branch back to just before one native user-message UUID.
///
/// Claude refuses a target that is not its latest user message ("stale target") unless the request also
/// names the latest one the caller has seen. Passing it lets one request drop several turns at once, and
/// turns a view that has fallen behind into an explicit "unseen later turn" refusal instead of a rewind
/// of the wrong depth.
pub fn rewind_conversation(target_message_uuid: &str, last_seen_user_message_uuid: &str) -> Value {
    json!({
        "subtype":"rewind_conversation",
        "target_message_uuid":target_message_uuid,
        "last_seen_user_message_uuid":last_seen_user_message_uuid,
    })
}

/// The effort value that means "no extended thinking at all". Not a level the CLI accepts on its command
/// line; it is applied as a thinking-token cap of zero once the process is up.
pub const THINKING_OFF: &str = "off";

/// Ask for the model catalogue the agent itself offers, with each model's effort ladder and switches.
pub fn list_models() -> Value {
    json!({"subtype":"list_models"})
}

/// Ask how the context window is being spent right now, by category.
pub fn get_context_usage() -> Value {
    json!({"subtype":"get_context_usage"})
}

/// Cap extended thinking. Zero switches it off; `None` restores the model's own behaviour.
pub fn set_max_thinking_tokens(max: Option<u64>) -> Value {
    json!({"subtype":"set_max_thinking_tokens","max_thinking_tokens":max})
}

/// Merge settings into the agent's flag layer for this process only. Fast mode travels this way.
pub fn apply_flag_settings(settings: Value) -> Value {
    json!({"subtype":"apply_flag_settings","settings":settings})
}

pub fn mcp_status() -> Value {
    json!({"subtype":"mcp_status"})
}

pub fn mcp_toggle(server_name: &str, enabled: bool) -> Value {
    json!({"subtype":"mcp_toggle","serverName":server_name,"enabled":enabled})
}

pub fn mcp_reconnect(server_name: &str) -> Value {
    json!({"subtype":"mcp_reconnect","serverName":server_name})
}

pub fn stop_task(task_id: &str) -> Value {
    json!({"subtype":"stop_task","task_id":task_id})
}

/// Move every foreground task (running shell commands and subagents) to the background, so the turn can
/// go on without them. The control-request form of Ctrl+B.
pub fn background_tasks() -> Value {
    json!({"subtype":"background_tasks"})
}

/// Our answer to an MCP server's request for user input.
pub fn elicitation_response(request_id: &str, action: &str, content: Option<Value>) -> Value {
    let mut inner = json!({"action":action});
    if let Some(content) = content.filter(|c| c.is_object()) {
        inner["content"] = content;
    }
    json!({"type":"control_response","response":{"subtype":"success","request_id":request_id,"response":inner}})
}

/// Preview or apply Claude's own file checkpoint for one native user-message UUID.
pub fn rewind_files(user_message_id: &str, dry_run: bool) -> Value {
    json!({"subtype":"rewind_files","user_message_id":user_message_id,"dry_run":dry_run})
}

pub fn set_permission_mode(mode: &str) -> Value {
    json!({"subtype":"set_permission_mode","mode":mode})
}

pub fn set_model(model: Option<&str>) -> Value {
    match model {
        Some(m) => json!({"subtype":"set_model","model":m}),
        None => json!({"subtype":"set_model","model":null}),
    }
}

/// Our answer to the agent's permission question.
///
/// Allowing echoes an input back: the protocol lets a host edit a tool's arguments before it runs, and
/// returning them unchanged is how it says "run exactly what you proposed". A form the user filled in —
/// AskUserQuestion's answers — is the other case: there the caller passes an edited input, and that is
/// what carries the answers to the agent.
///
/// `updated_permissions` is how a "stop asking me about this" answer travels. The agent offers the
/// standing rules it would accept in the question's `permission_suggestions`; handing one back here is
/// what adopts it, and the agent then applies it to the destination the suggestion names. Only sent
/// alongside an allow — a refusal that widened permissions would be a contradiction.
pub fn permission_response(
    request_id: &str,
    allow: bool,
    input: Value,
    message: Option<&str>,
    updated_permissions: Option<Value>,
) -> Value {
    let inner = if allow {
        let mut allowed = json!({"behavior":"allow","updatedInput":input});
        // An empty array says the same thing as no array at all, and the agent need not read it.
        if let Some(updates) = updated_permissions.filter(|u| u.as_array().is_some_and(|a| !a.is_empty())) {
            allowed["updatedPermissions"] = updates;
        }
        allowed
    } else {
        json!({"behavior":"deny","message":message.unwrap_or("Denied by user")})
    };
    json!({"type":"control_response","response":{"subtype":"success","request_id":request_id,"response":inner}})
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stored `skip` becomes the agent's own word for it; anything unrecognized is left to the agent.
    #[test]
    fn stored_permission_mode_is_translated_for_the_command_line() {
        assert_eq!(cli_permission_mode(Some("skip")), Some("bypassPermissions"));
        assert_eq!(cli_permission_mode(Some("plan")), Some("plan"));
        assert_eq!(cli_permission_mode(Some("acceptEdits")), Some("acceptEdits"));
        assert_eq!(cli_permission_mode(None), None);
        assert_eq!(cli_permission_mode(Some("")), None);
        assert_eq!(cli_permission_mode(Some("nonsense")), None);
    }

    #[test]
    fn launch_args_always_ask_for_streaming_and_permission_routing() {
        let args = launch_args(None, None, None, None);
        let joined = args.join(" ");
        assert!(joined.contains("--input-format stream-json"));
        assert!(joined.contains("--output-format stream-json"));
        assert!(joined.contains("--include-partial-messages"));
        assert!(joined.contains("--permission-prompt-tool stdio"));
        assert!(joined.contains("--verbose"), "stream-json output requires it");
        assert!(
            joined.contains("--thinking adaptive") && joined.contains("--thinking-display summarized"),
            "reasoning text arrives only when both are present",
        );
    }

    #[test]
    fn launch_args_append_only_what_was_asked_for() {
        let args = launch_args(Some("sid-1"), Some("opus"), Some("high"), Some("plan")).join(" ");
        assert!(args.contains("--resume sid-1"));
        assert!(args.contains("--model opus"));
        assert!(args.contains("--effort high"));
        assert!(args.contains("--permission-mode plan"));
    }

    #[test]
    fn init_line_yields_the_agent_session_id() {
        let line = r#"{"type":"system","subtype":"init","session_id":"abc","model":"claude-sonnet-5"}"#;
        match parse_line(line) {
            Incoming::Init { session_id, model } => {
                assert_eq!(session_id, "abc");
                assert_eq!(model.as_deref(), Some("claude-sonnet-5"));
            }
            _ => panic!("expected Init"),
        }
    }

    #[test]
    fn task_protocol_frames_keep_their_subtype_and_payload() {
        let line = r#"{"type":"system","subtype":"task_progress","task_id":"task-1","usage":{"total_tokens":42}}"#;
        match parse_line(line) {
            Incoming::Task { subtype, message } => {
                assert_eq!(subtype, "task_progress");
                assert_eq!(message["task_id"], "task-1");
                assert_eq!(message["usage"]["total_tokens"], 42);
            }
            _ => panic!("expected a task protocol frame"),
        }
    }

    #[test]
    fn system_frames_about_the_run_become_notices_or_transient_state() {
        match parse_line(r#"{"type":"system","subtype":"api_retry","attempt":2,"max_retries":5,"retry_delay_ms":1500,"error":{"formatted":"529 overloaded"}}"#) {
            Incoming::ApiRetry { attempt, max_retries, delay_ms, message } => {
                assert_eq!((attempt, max_retries, delay_ms), (2, 5, 1500));
                assert_eq!(message, "529 overloaded");
            }
            _ => panic!("expected ApiRetry"),
        }
        match parse_line(r#"{"type":"system","subtype":"model_fallback","uuid":"f1","trigger":"overloaded","original_model":"opus","fallback_model":"sonnet","content":""}"#) {
            Incoming::Notice { id, message } => {
                assert_eq!(id, "n-f1");
                assert_eq!(message, "Switched from opus to sonnet (overloaded)");
            }
            _ => panic!("expected Notice"),
        }
        match parse_line(r#"{"type":"system","subtype":"permission_denied","tool_name":"Bash","tool_use_id":"t","decision_reason":"rule"}"#) {
            Incoming::Notice { message, .. } => assert_eq!(message, "Bash was denied: rule"),
            _ => panic!("expected Notice"),
        }
        // Only the prominent levels earn a row; `info` is transcript-mode chatter.
        assert!(matches!(
            parse_line(r#"{"type":"system","subtype":"informational","level":"info","content":"x"}"#),
            Incoming::Other
        ));
        assert!(matches!(
            parse_line(r#"{"type":"system","subtype":"informational","level":"warning","content":"slow"}"#),
            Incoming::Notice { .. }
        ));
        match parse_line(r#"{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"a","task_type":"local_bash","description":"npm test"}]}"#) {
            Incoming::BackgroundTasks(tasks) => assert_eq!(tasks.len(), 1),
            _ => panic!("expected BackgroundTasks"),
        }
        assert!(matches!(
            parse_line(r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed_warning","utilization":0.9}}"#),
            Incoming::RateLimit(_)
        ));
        match parse_line(r#"{"type":"result","subtype":"success","duration_ms":5,"total_cost_usd":0.25,"modelUsage":{"claude-sonnet-5":{"contextWindow":1000000}}}"#) {
            Incoming::Result { total_cost_usd, model_usage, .. } => {
                assert_eq!(total_cost_usd, Some(0.25));
                assert_eq!(model_usage.pointer("/claude-sonnet-5/contextWindow"), Some(&json!(1_000_000)));
            }
            _ => panic!("expected Result"),
        }
    }

    #[test]
    fn thinking_off_is_a_cap_rather_than_a_command_line_level() {
        let joined = launch_args(None, None, Some(THINKING_OFF), None).join(" ");
        assert!(!joined.contains("--effort"), "{joined}");
        assert_eq!(
            set_max_thinking_tokens(Some(0)),
            json!({"subtype":"set_max_thinking_tokens","max_thinking_tokens":0})
        );
        assert_eq!(
            set_max_thinking_tokens(None),
            json!({"subtype":"set_max_thinking_tokens","max_thinking_tokens":null})
        );
    }

    #[test]
    fn elicitation_answers_carry_content_only_when_accepting_a_form() {
        let accepted = elicitation_response("r1", "accept", Some(json!({"name":"x"})));
        assert_eq!(accepted.pointer("/response/response"), Some(&json!({"action":"accept","content":{"name":"x"}})));
        let declined = elicitation_response("r1", "decline", Some(json!({"name":"x"})));
        assert_eq!(declined.pointer("/response/response"), Some(&json!({"action":"decline","content":{"name":"x"}})));
        let cancelled = elicitation_response("r1", "cancel", None);
        assert_eq!(cancelled.pointer("/response/response"), Some(&json!({"action":"cancel"})));
    }

    #[test]
    fn result_line_keeps_the_provider_reported_duration() {
        let line = r#"{"type":"result","subtype":"success","duration_ms":78123}"#;
        match parse_line(line) {
            Incoming::Result { subtype, duration_ms, .. } => {
                assert_eq!(subtype, "success");
                assert_eq!(duration_ms, Some(78_123));
            }
            _ => panic!("expected Result"),
        }
    }

    /// Only the opening event names the message, which is why the reader has to remember it.
    #[test]
    fn message_start_names_the_message_and_deltas_do_not() {
        let start = r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"m1"}}}"#;
        match parse_line(start) {
            Incoming::Delta(d) => {
                assert!(matches!(d.kind, DeltaKind::MessageStart));
                assert_eq!(d.message_id.as_deref(), Some("m1"));
            }
            _ => panic!("expected Delta"),
        }
        let delta = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}}"#;
        match parse_line(delta) {
            Incoming::Delta(d) => assert!(d.message_id.is_none()),
            _ => panic!("expected Delta"),
        }
    }

    /// A slash command the CLI answers itself must not be mistaken for a refused turn.
    ///
    /// Both arrive as an assistant frame naming `<synthetic>` as the model, so the field that tells them
    /// apart is `local_command_source`. Without the split, `/effort` reads as a failure.
    #[test]
    fn a_locally_answered_command_is_its_own_frame() {
        let line = r#"{"type":"assistant","message":{"id":"m9","model":"<synthetic>","stop_reason":"end_turn","content":[{"type":"text","text":"Set effort level to high"}]},"is_meta":true,"local_command_source":"<local-command-stdout>Set effort level to high</local-command-stdout>"}"#;
        match parse_line(line) {
            Incoming::LocalCommand { id, text } => {
                assert_eq!(id, "lc-m9", "the id is prefixed so it cannot collide with an assistant row");
                assert_eq!(text, "Set effort level to high");
            }
            _ => panic!("expected LocalCommand"),
        }
        // The same model value on a frame without that field is a refusal, and stays an assistant frame
        // for the engine to turn into an error row.
        let refused = r#"{"type":"assistant","message":{"id":"m9","model":"<synthetic>","stop_reason":"refusal","content":[{"type":"text","text":"API Error"}]}}"#;
        assert!(matches!(parse_line(refused), Incoming::Assistant { .. }));
    }

    #[test]
    fn text_and_thinking_deltas_carry_their_message_and_block() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":2,"delta":{"type":"text_delta","text":"hi"}}}"#;
        match parse_line(line) {
            Incoming::Delta(d) => {
                assert_eq!(d.index, 2);
                assert!(matches!(d.kind, DeltaKind::Text(ref t) if t == "hi"));
            }
            _ => panic!("expected Delta"),
        }
        let thinking = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hmm"}}}"#;
        assert!(matches!(
            parse_line(thinking),
            Incoming::Delta(Delta { kind: DeltaKind::Thinking(ref t), .. }) if t == "hmm"
        ));
    }

    /// A subagent's frames name the tool call they belong to; the main conversation's do not.
    ///
    /// Verified against claude 2.1.258: the agent runs a subagent inside a `Task` call and reports every
    /// line it produces under that call's id, at the top level of the frame rather than inside the message.
    #[test]
    fn a_subagents_frames_name_the_tool_call_they_belong_to() {
        let child = r#"{"type":"assistant","parent_tool_use_id":"toolu_1","message":{"id":"m","content":[]}}"#;
        match parse_line(child) {
            Incoming::Assistant { parent, .. } => assert_eq!(parent.as_deref(), Some("toolu_1")),
            _ => panic!("expected Assistant"),
        }
        let result = r#"{"type":"user","parent_tool_use_id":"toolu_1","message":{"role":"user","content":[]}}"#;
        match parse_line(result) {
            Incoming::User { parent, .. } => assert_eq!(parent.as_deref(), Some("toolu_1")),
            _ => panic!("expected User"),
        }
        // The field is present on every frame and null on the main conversation's, which must read as "no parent"
        // rather than as a parent named "null".
        let main = r#"{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m","content":[]}}"#;
        match parse_line(main) {
            Incoming::Assistant { parent, .. } => assert!(parent.is_none()),
            _ => panic!("expected Assistant"),
        }
    }

    /// Compaction is announced twice: once when it starts, once when the summary replaces the history.
    #[test]
    fn compaction_is_reported_as_a_start_and_a_boundary() {
        let started = r#"{"type":"system","subtype":"status","status":"compacting","session_id":"s"}"#;
        assert!(matches!(parse_line(started), Incoming::CompactionStarted));
        let done = r#"{"type":"system","subtype":"compact_boundary","session_id":"s","compact_metadata":{"trigger":"manual","pre_tokens":31039,"post_tokens":2710}}"#;
        match parse_line(done) {
            Incoming::CompactionFinished { trigger, pre_tokens } => {
                assert_eq!(trigger.as_deref(), Some("manual"));
                assert_eq!(pre_tokens, Some(31039));
            }
            _ => panic!("expected CompactionFinished"),
        }
    }

    /// A boundary without metadata still marks the conversation; only the token count is missing.
    #[test]
    fn a_boundary_without_metadata_still_marks_the_conversation() {
        let bare = r#"{"type":"system","subtype":"compact_boundary","session_id":"s"}"#;
        match parse_line(bare) {
            Incoming::CompactionFinished { trigger, pre_tokens } => {
                assert!(trigger.is_none());
                assert!(pre_tokens.is_none());
            }
            _ => panic!("expected CompactionFinished"),
        }
    }

    /// The other statuses describe steps nobody asked about, and one of them reports a null status.
    #[test]
    fn other_status_lines_are_ignored() {
        assert!(matches!(
            parse_line(r#"{"type":"system","subtype":"status","status":"requesting","session_id":"s"}"#),
            Incoming::Other
        ));
        assert!(matches!(
            parse_line(r#"{"type":"system","subtype":"status","status":null,"compact_result":"success"}"#),
            Incoming::Other
        ));
    }

    #[test]
    fn permission_question_arrives_as_a_control_request() {
        let line = r#"{"type":"control_request","request_id":"r1","request":{"subtype":"can_use_tool","tool_name":"Write","input":{"file_path":"/x"}}}"#;
        match parse_line(line) {
            Incoming::ControlRequest { request_id, request } => {
                assert_eq!(request_id, "r1");
                assert_eq!(request["subtype"], "can_use_tool");
                assert_eq!(request["tool_name"], "Write");
            }
            _ => panic!("expected ControlRequest"),
        }
    }

    #[test]
    fn control_response_reports_failure_separately_from_payload() {
        let ok = r#"{"type":"control_response","response":{"subtype":"success","request_id":"r1","response":{"commands":[]}}}"#;
        match parse_line(ok) {
            Incoming::ControlResponse { request_id, error, .. } => {
                assert_eq!(request_id, "r1");
                assert!(error.is_none());
            }
            _ => panic!("expected ControlResponse"),
        }
        let bad = r#"{"type":"control_response","response":{"subtype":"error","request_id":"r2","error":"nope"}}"#;
        match parse_line(bad) {
            Incoming::ControlResponse { error, .. } => assert_eq!(error.as_deref(), Some("nope")),
            _ => panic!("expected ControlResponse"),
        }
    }

    #[test]
    fn a_malformed_line_is_ignored_rather_than_fatal() {
        assert!(matches!(parse_line("not json"), Incoming::Other));
        assert!(matches!(parse_line("{}"), Incoming::Other));
    }

    #[test]
    fn allowing_a_tool_echoes_its_input_back() {
        let v = permission_response("r1", true, json!({"file_path":"/x"}), None, None);
        assert_eq!(v["response"]["response"]["behavior"], "allow");
        assert_eq!(v["response"]["response"]["updatedInput"]["file_path"], "/x");
        let denied = permission_response("r1", false, Value::Null, Some("no"), None);
        assert_eq!(denied["response"]["response"]["behavior"], "deny");
        assert_eq!(denied["response"]["response"]["message"], "no");
    }

    /// A filled-in form travels as the tool's own arguments, so whatever the caller hands over is what runs.
    #[test]
    fn allowing_carries_an_edited_input_unchanged() {
        let answered = json!({"questions":[{"question":"Which?"}],"answers":{"Which?":"This one"}});
        let v = permission_response("r1", true, answered, None, None);
        assert_eq!(v["response"]["response"]["updatedInput"]["answers"]["Which?"], "This one");
    }

    /// An adopted suggestion rides along with the allow, verbatim: the agent wrote it, and editing it
    /// here would hand back a rule nobody was shown.
    #[test]
    fn an_adopted_suggestion_travels_with_the_allow() {
        let suggestion = json!([{"type":"setMode","mode":"acceptEdits","destination":"session"}]);
        let v = permission_response("r1", true, json!({}), None, Some(suggestion.clone()));
        assert_eq!(v["response"]["response"]["updatedPermissions"], suggestion);
    }

    /// Nothing adopted means no field at all — an empty list would say the same thing at more expense.
    #[test]
    fn no_suggestion_leaves_the_answer_as_it_was() {
        let v = permission_response("r1", true, json!({}), None, Some(json!([])));
        assert!(v["response"]["response"].get("updatedPermissions").is_none());
        let v = permission_response("r1", true, json!({}), None, None);
        assert!(v["response"]["response"].get("updatedPermissions").is_none());
    }

    /// The long block is the one this wire accepts. Verified against claude 2.1.258 with a live process:
    /// the SDK's short `{"type":"image","data","mimeType"}` form fails the turn instead of being read.
    #[test]
    fn an_attached_image_travels_as_a_base64_source_block() {
        let image = ChatImage { mime_type: "image/png".into(), data: "AAAA".into() };
        let v = user_message("what is this", std::slice::from_ref(&image));
        let content = v["message"]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0], json!({"type":"text","text":"what is this"}));
        assert_eq!(content[1]["type"], "image");
        assert_eq!(content[1]["source"]["type"], "base64");
        assert_eq!(content[1]["source"]["media_type"], "image/png");
        assert_eq!(content[1]["source"]["data"], "AAAA");
    }

    /// Images alone are a message too — dropping a screenshot in and pressing send says enough.
    #[test]
    fn images_without_a_caption_send_no_empty_text_block() {
        let images = vec![
            ChatImage { mime_type: "image/png".into(), data: "AAAA".into() },
            ChatImage { mime_type: "image/jpeg".into(), data: "BBBB".into() },
        ];
        let v = user_message("", &images);
        let content = v["message"]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert!(content.iter().all(|b| b["type"] == "image"));
        assert_eq!(content[1]["source"]["media_type"], "image/jpeg");
    }

    #[test]
    fn a_plain_message_is_still_one_text_block() {
        let v = user_message("hello", &[]);
        assert_eq!(v["message"]["content"], json!([{"type":"text","text":"hello"}]));
    }

    /// A refusal never widens permissions, however the caller asks.
    #[test]
    fn a_refusal_carries_no_permission_update() {
        let suggestion = json!([{"type":"setMode","mode":"acceptEdits","destination":"session"}]);
        let v = permission_response("r1", false, Value::Null, Some("no"), Some(suggestion));
        assert!(v["response"]["response"].get("updatedPermissions").is_none());
    }
}
