//! The OpenCode server API used by the chat engine.
//!
//! OpenCode has no stdio protocol of its own: its terminal interface is a client of a local HTTP server,
//! and so is this engine. The engine starts `opencode serve` on a loopback port that nothing else knows,
//! sends requests to it, and follows the server's event stream. This module owns that wire — the requests,
//! their bodies, and the parsing of what comes back — while process lifecycle and timeline projection stay
//! in `engine`.
//!
//! The server is protected by a per-process password: OpenCode honors `OPENCODE_SERVER_PASSWORD` with
//! basic authentication, and a server that would run every tool the agent can run must not be driven by
//! whichever other process finds the port.

use std::io::Read;
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use base64::Engine as _;
use serde::Serialize;
use serde_json::{json, Value};

use super::codex_protocol::CollaborationModePreset;
use super::protocol::ChatImage;

/// Requests other than the event stream must answer within this long.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Requests that run a whole turn — a command, a shell line, a compaction — answer when the turn ends.
const TURN_TIMEOUT: Duration = Duration::from_secs(60 * 60);

/// The user name OpenCode expects with the password; it is not configurable on the client side.
const USERNAME: &str = "opencode";

/// One running `opencode serve`, addressed from the engine.
#[derive(Debug)]
pub struct Server {
    base: String,
    authorization: String,
    /// The project directory every request names, so the server answers for this session's project
    /// even when it also hosts another one.
    directory: Option<String>,
}

impl Server {
    pub fn new(port: u16, password: &str, directory: Option<&str>) -> Self {
        let credentials = base64::engine::general_purpose::STANDARD.encode(format!("{USERNAME}:{password}"));
        Server {
            base: format!("http://127.0.0.1:{port}"),
            authorization: format!("Basic {credentials}"),
            directory: directory.map(str::to_string),
        }
    }

    fn url(&self, path: &str) -> String {
        match &self.directory {
            Some(directory) => {
                let separator = if path.contains('?') { '&' } else { '?' };
                format!("{}{path}{separator}directory={}", self.base, encode_query(directory))
            }
            None => format!("{}{path}", self.base),
        }
    }

    fn read_response(result: Result<ureq::Response, ureq::Error>) -> Result<Value, String> {
        match result {
            Ok(response) => {
                let body = response
                    .into_string()
                    .map_err(|e| format!("Failed to read OpenCode's answer: {e}"))?;
                if body.trim().is_empty() {
                    return Ok(Value::Null);
                }
                Ok(serde_json::from_str(&body).unwrap_or(Value::String(body)))
            }
            Err(ureq::Error::Status(code, response)) => {
                let raw = response.into_string().unwrap_or_default();
                Err(error_text(code, &raw))
            }
            Err(error) => Err(format!("OpenCode did not answer: {error}")),
        }
    }

    pub fn get(&self, path: &str) -> Result<Value, String> {
        Self::read_response(
            ureq::get(&self.url(path))
                .set("Authorization", &self.authorization)
                .timeout(REQUEST_TIMEOUT)
                .call(),
        )
    }

    pub fn post(&self, path: &str, body: Option<&Value>) -> Result<Value, String> {
        self.post_with_timeout(path, body, REQUEST_TIMEOUT)
    }

    /// A request that returns only when a whole turn has run.
    pub fn post_turn(&self, path: &str, body: Option<&Value>) -> Result<Value, String> {
        self.post_with_timeout(path, body, TURN_TIMEOUT)
    }

    fn post_with_timeout(&self, path: &str, body: Option<&Value>, timeout: Duration) -> Result<Value, String> {
        let request = ureq::post(&self.url(path))
            .set("Authorization", &self.authorization)
            .set("Content-Type", "application/json")
            .timeout(timeout);
        // Sent as a string: the crate's JSON helpers sit behind a feature this build does not enable.
        let payload = body.map(Value::to_string).unwrap_or_else(|| "{}".to_string());
        Self::read_response(request.send_string(&payload))
    }

    pub fn delete(&self, path: &str) -> Result<Value, String> {
        Self::read_response(
            ureq::delete(&self.url(path))
                .set("Authorization", &self.authorization)
                .timeout(REQUEST_TIMEOUT)
                .call(),
        )
    }

    /// Whether the server answers at all. Used while it starts, so failures are expected and cheap.
    pub fn healthy(&self) -> bool {
        ureq::get(&format!("{}/global/health", self.base))
            .set("Authorization", &self.authorization)
            .timeout(Duration::from_secs(2))
            .call()
            .is_ok()
    }

    /// Open the event stream. The reader stays open for as long as the server sends events.
    pub fn open_events(&self) -> Result<Box<dyn Read + Send>, String> {
        let response = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(45))
            .build()
            .get(&self.url("/event"))
            .set("Authorization", &self.authorization)
            .set("Accept", "text/event-stream")
            .call()
            .map_err(|e| format!("Failed to follow OpenCode's events: {e}"))?;
        Ok(response.into_reader())
    }
}

fn error_text(code: u16, raw: &str) -> String {
    let detail = serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|v| {
            v.pointer("/data/message")
                .or_else(|| v.get("message"))
                .or_else(|| v.get("error"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| raw.trim().to_string());
    if detail.is_empty() {
        format!("OpenCode returned HTTP {code}")
    } else {
        format!("OpenCode returned HTTP {code}: {detail}")
    }
}

/// Whether an error string came from the server saying it has no such session.
pub fn is_not_found(error: &str) -> bool {
    error.contains("HTTP 404") || error.to_ascii_lowercase().contains("not found")
}

/// Percent-encode a query value: only the characters that would end or split the value.
pub(crate) fn encode_query(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Keep each server's port in backend configuration and change it only when occupied. Holding the
/// database lock also prevents concurrent launches from choosing another session's reserved port.
pub fn configured_port(app: &crate::host::AppCtx, scope: &str) -> Result<u16, String> {
    let conn = app.db().conn.lock().unwrap();
    let settings = crate::db::repo::get_app_settings(&conn)?;
    let key = format!("opencode.port.{scope}");
    if let Some(port) = settings.get(&key).and_then(|value| value.parse::<u16>().ok()) {
        if (10000..=49151).contains(&port) && TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Ok(port);
        }
    }
    let reserved: std::collections::HashSet<u16> = settings.iter()
        .filter(|(key, _)| key.starts_with("opencode.port."))
        .filter_map(|(_, value)| value.parse().ok())
        .collect();
    for _ in 0..128 {
        let port = 10000 + (uuid::Uuid::new_v4().as_u128() % 39152) as u16;
        if reserved.contains(&port) {
            continue;
        }
        let Ok(_listener) = TcpListener::bind(("127.0.0.1", port)) else { continue };
        crate::db::repo::set_app_settings(&conn, &std::collections::HashMap::from([(key, port.to_string())]))?;
        return Ok(port);
    }
    Err("No available OpenCode port could be reserved in 10000–49151".to_string())
}

pub fn random_password() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

static MESSAGE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A message id in OpenCode's own shape, chosen before the message is sent.
///
/// OpenCode accepts a caller-supplied id, and knowing it in advance is what lets the row shown while the
/// message is still on its way be the same row the server later reports — and what a rewind names.
pub fn new_message_id() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let counter = MESSAGE_COUNTER.fetch_add(1, Ordering::Relaxed) & 0xfff;
    let ascending = format!("{:012x}", (now << 12) | counter);
    let random = uuid::Uuid::new_v4().simple().to_string();
    format!("msg_{}{}", &ascending[ascending.len() - 12..], &random[..14])
}

/// `provider/model` into its two halves. Provider ids never contain a slash; model ids may.
pub fn split_model(id: &str) -> Option<(&str, &str)> {
    let (provider, model) = id.split_once('/')?;
    if provider.is_empty() || model.is_empty() {
        return None;
    }
    Some((provider, model))
}

fn model_value(model: Option<&str>) -> Option<Value> {
    let (provider, model) = split_model(model?)?;
    Some(json!({"providerID":provider,"modelID":model}))
}

fn data_url(image: &ChatImage) -> String {
    format!("data:{};base64,{}", image.mime_type, image.data)
}

fn extension(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        _ => "bin",
    }
}

/// The body of a user turn: text and attached images, with the agent, model, and variant chosen here.
pub fn prompt_body(
    message_id: &str,
    text: &str,
    images: &[ChatImage],
    agent: Option<&str>,
    model: Option<&str>,
    variant: Option<&str>,
) -> Value {
    let mut parts: Vec<Value> = Vec::new();
    if !text.is_empty() {
        parts.push(json!({"type":"text","text":text}));
    }
    for (index, image) in images.iter().enumerate() {
        parts.push(json!({
            "type":"file",
            "mime":image.mime_type,
            "filename":format!("attachment-{}.{}", index + 1, extension(&image.mime_type)),
            "url":data_url(image)
        }));
    }
    let mut body = json!({"messageID":message_id,"parts":parts});
    if let Some(agent) = agent {
        body["agent"] = json!(agent);
    }
    if let Some(model) = model_value(model) {
        body["model"] = model;
    }
    if let Some(variant) = variant {
        body["variant"] = json!(variant);
    }
    body
}

/// The body of a catalogue command such as `/init` or a user-defined `/review`.
pub fn command_body(
    message_id: &str,
    command: &str,
    arguments: &str,
    agent: Option<&str>,
    model: Option<&str>,
    variant: Option<&str>,
) -> Value {
    let mut body = json!({"messageID":message_id,"command":command,"arguments":arguments});
    if let Some(agent) = agent {
        body["agent"] = json!(agent);
    }
    // This endpoint takes the model as one string, unlike the prompt endpoint.
    if let Some(model) = model {
        if split_model(model).is_some() {
            body["model"] = json!(model);
        }
    }
    if let Some(variant) = variant {
        body["variant"] = json!(variant);
    }
    body
}

/// The body of a `!` shell line run inside the conversation.
pub fn shell_body(message_id: &str, command: &str, agent: &str, model: Option<&str>) -> Value {
    let mut body = json!({"messageID":message_id,"agent":agent,"command":command});
    if let Some(model) = model_value(model) {
        body["model"] = model;
    }
    body
}

/// The primary agents a person can pick, as the same presets the composer's chip already understands.
///
/// Subagents and OpenCode's internal helpers (title, summary, compaction) are not conversation styles.
pub fn parse_agents(value: &Value) -> Vec<CollaborationModePreset> {
    let mut agents: Vec<CollaborationModePreset> = value
        .as_array()
        .into_iter()
        .flatten()
        .filter(|agent| {
            matches!(agent.get("mode").and_then(Value::as_str), Some("primary") | Some("all"))
                && agent.get("hidden").and_then(Value::as_bool) != Some(true)
        })
        .filter_map(|agent| {
            let name = agent.get("name").and_then(Value::as_str)?;
            Some(CollaborationModePreset {
                name: name.to_string(),
                mode: name.to_string(),
                reasoning_effort: agent.get("variant").and_then(Value::as_str).map(str::to_string),
                description: agent
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|d| !d.is_empty())
                    .map(str::to_string),
            })
        })
        .collect();
    // OpenCode's own order: build first, plan second, then everything the user defined.
    agents.sort_by_key(|agent| match agent.mode.as_str() {
        "build" => 0,
        "plan" => 1,
        _ => 2,
    });
    agents
}

/// The slash commands the server offers — its own, the user's, and installed skills — for `/` completion.
pub fn parse_commands(value: &Value) -> Vec<Value> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|command| {
            let name = command.get("name").and_then(Value::as_str)?;
            let hints = command
                .get("hints")
                .and_then(Value::as_array)
                .map(|hints| hints.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "))
                .unwrap_or_default();
            let mut out = json!({"name":name});
            if let Some(description) = command.get("description").and_then(Value::as_str) {
                if !description.trim().is_empty() {
                    out["description"] = json!(description.trim());
                }
            }
            if !hints.is_empty() {
                out["argumentHint"] = json!(hints);
            }
            Some(out)
        })
        .collect()
}

/// One model the composer can switch to, in the shape every chat engine reports.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpencodeModel {
    pub id: String,
    pub label: String,
    pub description: String,
    pub effort_levels: Vec<String>,
    pub context_window: Option<u64>,
    pub curated: bool,
    pub large_context: bool,
}

/// The effort ladder the composer draws, from least to most; variants outside it are offered last.
const EFFORT_LADDER: [&str; 7] = ["minimal", "low", "medium", "high", "xhigh", "max", "ultra"];

fn ladder_rank(name: &str) -> usize {
    EFFORT_LADDER.iter().position(|level| *level == name).unwrap_or(EFFORT_LADDER.len())
}

/// The models of connected providers, from `GET /provider`.
///
/// Each provider's default model leads its group so the usual choice is at hand; the rest follow by name.
/// A `none` variant means the provider's own unthinking setting and is what an empty effort already
/// sends, so it is not offered twice.
pub fn parse_models(value: &Value) -> Vec<OpencodeModel> {
    let defaults = value.get("default").cloned().unwrap_or(Value::Null);
    let mut out = Vec::new();
    for provider in value.get("all").or_else(|| value.get("providers")).and_then(Value::as_array).into_iter().flatten() {
        let Some(provider_id) = provider.get("id").and_then(Value::as_str) else { continue };
        if !provider_connected(value, provider_id) {
            continue;
        }
        let provider_name = provider.get("name").and_then(Value::as_str).unwrap_or(provider_id);
        let default_model = defaults.get(provider_id).and_then(Value::as_str);
        let Some(models) = provider.get("models").and_then(Value::as_object) else { continue };
        let mut group: Vec<(bool, OpencodeModel)> = models
            .iter()
            .map(|(model_id, model)| {
                let mut variants: Vec<String> = model
                    .get("variants")
                    .and_then(Value::as_object)
                    .map(|v| v.keys().filter(|k| k.as_str() != "none").cloned().collect())
                    .unwrap_or_default();
                variants.sort_by_key(|v| (ladder_rank(v), v.clone()));
                let context = model
                    .pointer("/limit/context")
                    .and_then(Value::as_u64)
                    .filter(|c| *c > 0);
                (
                    default_model == Some(model_id.as_str()),
                    OpencodeModel {
                        id: format!("{provider_id}/{model_id}"),
                        label: model
                            .get("name")
                            .and_then(Value::as_str)
                            .filter(|n| !n.trim().is_empty())
                            .unwrap_or(model_id)
                            .to_string(),
                        description: provider_name.to_string(),
                        effort_levels: variants,
                        context_window: context,
                        curated: true,
                        large_context: false,
                    },
                )
            })
            .collect();
        group.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.label.to_lowercase().cmp(&b.1.label.to_lowercase())));
        out.extend(group.into_iter().map(|(_, model)| model));
    }
    out
}

/// The provider default the server would use on its own, as `provider/model`, when one is configured.
pub fn default_model(value: &Value, connected_first: Option<&str>) -> Option<String> {
    let defaults = value.get("default")?.as_object()?;
    let providers = value.get("all").or_else(|| value.get("providers")).and_then(Value::as_array)?;
    let pick = |provider_id: &str| -> Option<String> {
        if !provider_connected(value, provider_id) {
            return None;
        }
        let model = defaults.get(provider_id)?.as_str()?;
        Some(format!("{provider_id}/{model}"))
    };
    if let Some(preferred) = connected_first {
        if let Some(found) = pick(preferred) {
            return Some(found);
        }
    }
    providers
        .iter()
        .filter_map(|provider| provider.get("id").and_then(Value::as_str))
        .find_map(pick)
}

fn provider_connected(value: &Value, id: &str) -> bool {
    value.get("connected").and_then(Value::as_array)
        .map(|providers| providers.iter().any(|provider| provider.as_str() == Some(id)))
        .unwrap_or(value.get("all").is_none())
}

/// A permission question as the composer's card understands it.
///
/// `tool_input` is what the tool call itself proposed, when the engine can find its row; the card draws
/// a command or a file path from it. The `always` patterns become the card's one standing rule: "always
/// allow this", which OpenCode honours for the rest of the session.
pub fn permission_payload(props: &Value, tool_input: Option<Value>) -> Value {
    let id = props.get("id").and_then(Value::as_str).unwrap_or("");
    let permission = props.get("permission").and_then(Value::as_str).unwrap_or("tool");
    let patterns: Vec<&str> = props
        .get("patterns")
        .and_then(Value::as_array)
        .map(|p| p.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let always: Vec<&str> = props
        .get("always")
        .and_then(Value::as_array)
        .map(|p| p.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let metadata = props.get("metadata").cloned().unwrap_or_else(|| json!({}));
    let mut input = tool_input.unwrap_or(Value::Null);
    if input.is_null() {
        input = super::opencode_timeline::normalize_input(&metadata);
        if let Value::Object(map) = &mut input {
            if !patterns.is_empty() && !map.contains_key("command") && permission == "bash" {
                map.insert("command".into(), json!(patterns.join("\n")));
            }
            if !patterns.is_empty() && !map.contains_key("file_path") && matches!(permission, "edit" | "write" | "read") {
                map.insert("file_path".into(), json!(patterns[0]));
            }
        }
    }
    let mut payload = json!({
        "id":id,
        "tool_name":super::opencode_timeline::tool_name(permission),
        "display_name":"OpenCode",
        "input":input,
        "_opencodeKind":"permission"
    });
    if !patterns.is_empty() {
        payload["description"] = json!(patterns.join(", "));
    }
    if !always.is_empty() {
        payload["permission_suggestions"] = json!([{
            "type":"opencodeAlways",
            "subject":always.join(", ")
        }]);
    }
    payload
}

/// A question the agent asked, as the same form card Claude's AskUserQuestion uses.
pub fn question_payload(props: &Value) -> Option<Value> {
    let id = props.get("id").and_then(Value::as_str)?;
    let questions: Vec<Value> = props
        .get("questions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|question| {
            let text = question.get("question").and_then(Value::as_str)?;
            let header = question.get("header").and_then(Value::as_str).unwrap_or(text);
            let options: Vec<Value> = question
                .get("options")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|option| {
                    let label = option.get("label").and_then(Value::as_str)?;
                    let mut out = json!({"label":label});
                    if let Some(description) = option.get("description").and_then(Value::as_str) {
                        out["description"] = json!(description);
                    }
                    Some(out)
                })
                .collect();
            Some(json!({
                "question":text,
                "header":header,
                "options":options,
                "multiSelect":question.get("multiple").and_then(Value::as_bool).unwrap_or(false),
                "allowOther":question.get("custom").and_then(Value::as_bool).unwrap_or(true),
                "allowEmpty":false
            }))
        })
        .collect();
    if questions.is_empty() {
        return None;
    }
    Some(json!({
        "id":id,
        "tool_name":"AskUserQuestion",
        "display_name":"OpenCode",
        "input":{"questions":questions},
        "_opencodeKind":"question"
    }))
}

/// The answers the form card wrote back, in the order OpenCode asked its questions.
///
/// The card keys answers by question text and joins several picks with a comma. OpenCode wants one list
/// of labels per question, so joined picks are split back along the option labels it offered; text that
/// matches no label is the typed answer and travels whole.
pub fn question_answers(request: &Value, updated_input: Option<&Value>) -> Vec<Vec<String>> {
    let questions = request
        .pointer("/input/questions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let answers = updated_input
        .and_then(|input| input.get("answers"))
        .and_then(Value::as_object);
    questions
        .iter()
        .map(|question| {
            let text = question.get("question").and_then(Value::as_str).unwrap_or("");
            let Some(answer) = answers
                .and_then(|a| a.get(text))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|a| !a.is_empty())
            else {
                return Vec::new();
            };
            let labels: Vec<&str> = question
                .get("options")
                .and_then(Value::as_array)
                .map(|o| o.iter().filter_map(|option| option.get("label").and_then(Value::as_str)).collect())
                .unwrap_or_default();
            split_labels(answer, &labels)
        })
        .collect()
}

fn split_labels(answer: &str, labels: &[&str]) -> Vec<String> {
    if labels.contains(&answer) {
        return vec![answer.to_string()];
    }
    // Walk the joined string, taking the longest label that fits at each step; a label may itself
    // contain the separator, so splitting on it blindly would cut such a label in two.
    let mut rest = answer;
    let mut picked: Vec<String> = Vec::new();
    while !rest.is_empty() {
        let next = labels
            .iter()
            .filter(|label| !label.is_empty() && rest.starts_with(**label))
            .filter(|label| rest.len() == label.len() || rest[label.len()..].starts_with(", "))
            .max_by_key(|label| label.len());
        let Some(label) = next else { return vec![answer.to_string()] };
        picked.push((*label).to_string());
        rest = &rest[label.len()..];
        rest = rest.strip_prefix(", ").unwrap_or(rest);
    }
    if picked.is_empty() { vec![answer.to_string()] } else { picked }
}

/// One `data:` line of the event stream, parsed. Comments, blank lines, and other fields yield nothing.
pub fn parse_sse_line(line: &str) -> Option<Value> {
    let data = line.strip_prefix("data:")?.trim();
    if data.is_empty() {
        return None;
    }
    serde_json::from_str(data).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_name_the_project_directory() {
        let server = Server::new(12345, "pw", Some("/tmp/my project"));
        assert_eq!(server.url("/session"), "http://127.0.0.1:12345/session?directory=/tmp/my%20project");
        assert_eq!(
            server.url("/session?limit=2"),
            "http://127.0.0.1:12345/session?limit=2&directory=/tmp/my%20project"
        );
    }

    #[test]
    fn message_ids_look_like_opencodes_and_never_repeat() {
        let a = new_message_id();
        let b = new_message_id();
        assert!(a.starts_with("msg_") && a.len() == 4 + 12 + 14);
        assert_ne!(a, b);
    }

    #[test]
    fn a_model_id_splits_at_the_provider() {
        assert_eq!(split_model("chutes/zai-org/GLM-5.2"), Some(("chutes", "zai-org/GLM-5.2")));
        assert_eq!(split_model("nothing"), None);
        assert_eq!(split_model("/x"), None);
    }

    #[test]
    fn a_prompt_carries_text_images_and_choices() {
        let body = prompt_body(
            "msg_1",
            "hi",
            &[ChatImage { mime_type: "image/png".into(), data: "AAAA".into() }],
            Some("plan"),
            Some("deepseek/deepseek-v4-pro"),
            Some("max"),
        );
        assert_eq!(body["messageID"], "msg_1");
        assert_eq!(body["parts"][0]["text"], "hi");
        assert_eq!(body["parts"][1]["url"], "data:image/png;base64,AAAA");
        assert_eq!(body["parts"][1]["filename"], "attachment-1.png");
        assert_eq!(body["agent"], "plan");
        assert_eq!(body["model"]["providerID"], "deepseek");
        assert_eq!(body["model"]["modelID"], "deepseek-v4-pro");
        assert_eq!(body["variant"], "max");
    }

    #[test]
    fn only_visible_primary_agents_become_presets() {
        let agents = json!([
            {"name":"plan","mode":"primary","description":"Plan mode."},
            {"name":"build","mode":"primary"},
            {"name":"title","mode":"primary","hidden":true},
            {"name":"explore","mode":"subagent"},
            {"name":"custom","mode":"all","description":"  mine  "}
        ]);
        let presets = parse_agents(&agents);
        let names: Vec<&str> = presets.iter().map(|p| p.mode.as_str()).collect();
        assert_eq!(names, vec!["build", "plan", "custom"]);
        assert_eq!(presets[2].description.as_deref(), Some("mine"));
    }

    #[test]
    fn models_lead_with_the_providers_default_and_sort_their_variants() {
        let value = json!({
            "default":{"p":"b"},
            "providers":[{"id":"p","name":"Provider","models":{
                "a":{"name":"Alpha","limit":{"context":1000},"variants":{"max":{},"low":{},"none":{}}},
                "b":{"name":"Beta","limit":{"context":0}}
            }}]
        });
        let models = parse_models(&value);
        assert_eq!(models[0].id, "p/b");
        assert_eq!(models[1].id, "p/a");
        assert_eq!(models[1].effort_levels, vec!["low", "max"]);
        assert_eq!(models[1].context_window, Some(1000));
        assert_eq!(models[0].context_window, None);
        assert_eq!(default_model(&value, None).as_deref(), Some("p/b"));
    }

    #[test]
    fn disconnected_providers_are_not_offered_or_selected_by_default() {
        let mut value = json!({
            "default":{"offline":"a","online":"b"},
            "all":[
                {"id":"offline","models":{"a":{"name":"Unavailable"}}},
                {"id":"online","models":{"b":{"name":"Available"}}}
            ],
            "connected":["online"]
        });
        let models = parse_models(&value);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "online/b");
        assert_eq!(default_model(&value, Some("offline")).as_deref(), Some("online/b"));
        value["connected"] = json!([]);
        assert!(parse_models(&value).is_empty());
        assert!(default_model(&value, None).is_none());
    }

    #[test]
    fn a_permission_becomes_a_card_with_its_standing_rule() {
        let props = json!({
            "id":"per_1","sessionID":"ses_1","permission":"bash",
            "patterns":["git status"],"always":["git *"],"metadata":{}
        });
        let payload = permission_payload(&props, None);
        assert_eq!(payload["tool_name"], "Bash");
        assert_eq!(payload["input"]["command"], "git status");
        assert_eq!(payload["description"], "git status");
        assert_eq!(payload["permission_suggestions"][0]["type"], "opencodeAlways");
        assert_eq!(payload["permission_suggestions"][0]["subject"], "git *");
    }

    #[test]
    fn a_question_round_trips_through_the_form_card() {
        let props = json!({"id":"que_1","sessionID":"ses_1","questions":[
            {"question":"Which?","header":"Pick","options":[{"label":"A","description":"a"},{"label":"B, C","description":"bc"}],"multiple":true}
        ]});
        let payload = question_payload(&props).unwrap();
        assert_eq!(payload["tool_name"], "AskUserQuestion");
        assert_eq!(payload["input"]["questions"][0]["multiSelect"], true);
        let answered = json!({"answers":{"Which?":"A, B, C"}});
        assert_eq!(question_answers(&payload, Some(&answered)), vec![vec!["A", "B, C"]]);
        let typed = json!({"answers":{"Which?":"something else"}});
        assert_eq!(question_answers(&payload, Some(&typed)), vec![vec!["something else"]]);
        assert_eq!(question_answers(&payload, None), vec![Vec::<String>::new()]);
    }

    #[test]
    fn only_data_lines_of_the_stream_carry_events() {
        assert_eq!(parse_sse_line("data: {\"type\":\"server.connected\"}").unwrap()["type"], "server.connected");
        assert!(parse_sse_line("event: message").is_none());
        assert!(parse_sse_line(": keepalive").is_none());
        assert!(parse_sse_line("").is_none());
    }

    #[test]
    fn server_errors_read_their_message() {
        assert_eq!(
            error_text(404, "{\"name\":\"NotFoundError\",\"data\":{\"message\":\"Session not found: x\"}}"),
            "OpenCode returned HTTP 404: Session not found: x"
        );
        assert!(is_not_found(&error_text(404, "")));
    }
}
