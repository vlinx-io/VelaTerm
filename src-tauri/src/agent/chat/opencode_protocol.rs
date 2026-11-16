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
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
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
    version: AtomicU8,
    initialized: AtomicBool,
    initialization: std::sync::Mutex<()>,
}

impl Server {
    pub fn new(port: u16, password: &str, directory: Option<&str>) -> Self {
        let credentials = base64::engine::general_purpose::STANDARD.encode(format!("{USERNAME}:{password}"));
        Server {
            base: format!("http://127.0.0.1:{port}"),
            authorization: format!("Basic {credentials}"),
            directory: directory.map(str::to_string),
            version: AtomicU8::new(1),
            initialized: AtomicBool::new(false),
            initialization: std::sync::Mutex::new(()),
        }
    }

    fn url(&self, path: &str) -> String {
        match &self.directory {
            Some(directory) => {
                let separator = if path.contains('?') { '&' } else { '?' };
                let key = if self.is_v2() { "location%5Bdirectory%5D" } else { "directory" };
                format!("{}{path}{separator}{key}={}", self.base, encode_query(directory))
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
                serde_json::from_str(&body).map_err(|_| "OpenCode returned an unexpected non-JSON response".to_string())
            }
            Err(ureq::Error::Status(code, response)) => {
                let raw = response.into_string().unwrap_or_default();
                Err(error_text(code, &raw))
            }
            Err(error) => Err(format!("OpenCode did not answer: {error}")),
        }
    }

    pub fn get(&self, path: &str) -> Result<Value, String> {
        self.initialize()?;
        if self.is_v2() && path == "/provider" {
            let default = self.get_raw("/api/model/default")?;
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let models = loop {
                let models = self.get_raw("/api/model")?;
                if !models.get("data").and_then(Value::as_array).is_some_and(Vec::is_empty)
                    || std::time::Instant::now() >= deadline { break models; }
                // Current v2 publishes its model catalogue asynchronously after activation.
                std::thread::sleep(Duration::from_millis(150));
            };
            let providers = self.get_raw("/api/provider")?;
            return Ok(v2_providers(&models, &default, &providers));
        }
        if self.is_v2() && path.starts_with("/session/") && path.ends_with("/message") {
            return self.v2_messages(path);
        }
        let value = self.get_raw(&self.route(path)?)?;
        if self.is_v2() {
            let mut value = value.get("data").cloned().unwrap_or(value);
            if path.starts_with("/session/") {
                if let Some(directory) = value.pointer("/location/directory").cloned() { value["directory"] = directory; }
            }
            Ok(value)
        } else { Ok(value) }
    }

    fn get_raw(&self, path: &str) -> Result<Value, String> {
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
        self.initialize()?;
        let body = if self.is_v2() { self.v2_body(path, body)? } else { body.cloned().unwrap_or_else(||json!({})) };
        let request = ureq::post(&self.url(&self.route(path)?))
            .set("Authorization", &self.authorization)
            .set("Content-Type", "application/json")
            .timeout(timeout);
        // Sent as a string: the crate's JSON helpers sit behind a feature this build does not enable.
        let value = Self::read_response(request.send_string(&body.to_string()))?;
        Ok(if self.is_v2() { value.get("data").cloned().unwrap_or(value) } else { value })
    }

    pub fn delete(&self, path: &str) -> Result<Value, String> {
        Self::read_response(
            ureq::delete(&self.url(&self.route(path)?))
                .set("Authorization", &self.authorization)
                .timeout(REQUEST_TIMEOUT)
                .call(),
        )
    }

    pub fn is_v2(&self) -> bool {
        self.version.load(Ordering::Relaxed) == 2
    }

    /// Newer previews activate agents and provider configuration lazily. Health alone does not mean
    /// those catalogues are ready; use the native wait endpoint only when the peer advertises it.
    pub fn initialize(&self) -> Result<(),String> {
        if !self.is_v2() || self.initialized.load(Ordering::Acquire){return Ok(());}
        let _guard=self.initialization.lock().map_err(|e|e.to_string())?;
        if self.initialized.load(Ordering::Acquire){return Ok(());}
        let spec=self.get_raw("/openapi.json")?;
        if spec.pointer("/paths/~1api~1plugin~1await-activation/post").is_some() {
            Self::read_response(ureq::post(&self.url("/api/plugin/await-activation"))
                .set("Authorization",&self.authorization).set("Content-Type","application/json")
                .timeout(REQUEST_TIMEOUT).send_string("{}"))?;
        } else if spec.pointer("/paths/~1api~1model~1default/get").is_some() {
            // Current v2 resolves configuration lazily; resolve the default before listing models.
            self.get_raw("/api/model/default")?;
        }
        self.initialized.store(true,Ordering::Release);
        Ok(())
    }

    /// Negotiate using health or server-info JSON, never the frontend's unknown-route HTML fallback.
    ///
    /// A v1 server's only health endpoint is `/global/health`; 1.19.0 happens to answer `/api/health`
    /// too, without a version field. Trusting that alias would flip a v1 server onto the v2 `/api`
    /// routes the moment a single `/global/health` probe is slow enough to time out, and every later
    /// request would then be answered by the frontend's HTML fallback. So v2 is decided by
    /// `/api/info`, or by an `/api/health` answer that names a version, and never by health alone.
    pub fn healthy(&self) -> bool {
        let probe = |path: &str| ureq::get(&format!("{}{path}", self.base))
            .set("Authorization", &self.authorization).timeout(Duration::from_secs(2)).call();
        if let Ok(value) = Self::read_response(probe("/global/health")) {
            if value.get("healthy").and_then(Value::as_bool) == Some(true) {
                self.version.store(1,Ordering::Relaxed);
                return true;
            }
        }
        if let Ok(value) = Self::read_response(probe("/api/info")) {
            if value.get("version").and_then(Value::as_str).is_some_and(|v| !v.is_empty())
                && value.get("pid").and_then(Value::as_u64).is_some_and(|pid| pid > 0)
                && value.get("capabilities").is_some_and(Value::is_object) {
                self.version.store(2,Ordering::Relaxed);
                return true;
            }
        }
        if let Ok(value) = Self::read_response(probe("/api/health")) {
            if value.get("healthy").and_then(Value::as_bool) == Some(true) {
                // Only a preview's numbered answer says v2; a bare one is the v1 alias, and so is a
                // stable 1.x version in case a later v1 build starts naming itself here.
                let v2 = crate::agent::opencode::is_v2_version(value.get("version").and_then(Value::as_str));
                self.version.store(if v2 {2} else {1},Ordering::Relaxed);
                return true;
            }
        }
        false
    }

    /// Open the event stream. The reader stays open for as long as the server sends events.
    pub fn open_events(&self) -> Result<Box<dyn Read + Send>, String> {
        let response = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(45))
            .build()
            .get(&self.url(if self.is_v2() { "/api/event" } else { "/event" }))
            .set("Authorization", &self.authorization)
            .set("Accept", "text/event-stream")
            .call()
            .map_err(|e| format!("Failed to follow OpenCode's events: {e}"))?;
        Ok(response.into_reader())
    }

    fn route(&self, path: &str) -> Result<String, String> {
        if !self.is_v2() || path.starts_with("/api/") { return Ok(path.to_string()); }
        if matches!(path,"/path") {
            return Err("This OpenCode v2 server does not expose the v1 path API".into());
        }
        if path.ends_with("/share") || path.ends_with("/unshare") {
            return Err("This OpenCode v2 server does not support shared conversation links".into());
        }
        let path = if let Some(prefix)=path.strip_suffix("/prompt_async") {format!("{prefix}/prompt")}
            else if let Some(prefix)=path.strip_suffix("/abort") {format!("{prefix}/interrupt")}
            else if let Some(prefix)=path.strip_suffix("/summarize") {format!("{prefix}/compact")}
            else if let Some(prefix)=path.strip_suffix("/unrevert") {format!("{prefix}/revert/clear")}
            else if let Some(prefix)=path.strip_suffix("/revert") {format!("{prefix}/revert/stage")}
            else {path.to_string()};
        Ok(format!("/api{path}"))
    }

    fn v2_body(&self, path: &str, body: Option<&Value>) -> Result<Value, String> {
        let native = body.cloned().unwrap_or_else(||json!({}));
        if path == "/session" {
            return Ok(json!({"location":{"directory":self.directory.as_deref().ok_or("OpenCode v2 requires a project directory")?}}));
        }
        if path.ends_with("/prompt_async") || path.ends_with("/command") {
            let session_path = path.rsplit_once('/').unwrap().0;
            // v2 selections belong to the session and are not accepted as prompt fields.
            if let Some(agent)=native.get("agent") {
                self.post(&format!("{session_path}/agent"),Some(&json!({"agent":agent})))?;
            }
            if let Some(model)=native.get("model") {
                let model = json!({"id":model["modelID"],"providerID":model["providerID"],
                    "variant":native.get("variant").and_then(Value::as_str).unwrap_or("default")});
                self.post(&format!("{session_path}/model"),Some(&json!({"model":model})))?;
            }
            if path.ends_with("/command") {
                return Ok(json!({"id":native["messageID"],"command":native["command"],"arguments":native["arguments"],"delivery":"queue"}));
            }
            let parts = native.get("parts").and_then(Value::as_array).cloned().unwrap_or_default();
            let text = parts.iter().filter(|p|p["type"]=="text").filter_map(|p|p["text"].as_str()).collect::<Vec<_>>().join("\n");
            let files: Vec<Value> = parts.iter().filter(|p|p["type"]=="file").map(|p|json!({"uri":p["url"],"name":p["filename"]})).collect();
            let mut value = json!({"id":native["messageID"],"text":text,"delivery":native.get("_delivery").and_then(Value::as_str).unwrap_or("queue")});
            if !files.is_empty() { value["files"]=json!(files); }
            return Ok(value);
        }
        if path.ends_with("/shell") {
            let id=native["messageID"].as_str().unwrap_or("").replacen("msg_","evt_",1);
            return Ok(json!({"id":id,"command":native["command"]}));
        }
        if path.ends_with("/summarize") { return Ok(json!({"delivery":"queue"})); }
        if path.ends_with("/revert") {
            return Ok(json!({"messageID":native["messageID"],"files":true}));
        }
        Ok(native)
    }

    fn v2_messages(&self, path: &str) -> Result<Value, String> {
        let session = path.strip_prefix("/session/").and_then(|p|p.strip_suffix("/message")).unwrap_or("");
        let mut next = format!("/api{path}?limit=100&order=asc");
        let mut seen = std::collections::HashSet::new();
        let mut messages = Vec::new();
        loop {
            let page = self.get_raw(&next)?;
            let data = page.get("data").and_then(Value::as_array).ok_or("OpenCode v2 returned invalid message history")?;
            messages.extend(data.iter().flat_map(|m|super::opencode_v2::messages(session,m))
                .map(|m|json!({"info":m.info,"parts":m.parts})));
            let Some(cursor) = page.pointer("/cursor/next").and_then(Value::as_str) else {break};
            if !seen.insert(cursor.to_string()) { return Err("OpenCode v2 repeated a history cursor".into()); }
            next = format!("/api{path}?limit=100&cursor={}",encode_query(cursor));
        }
        Ok(json!(messages))
    }

    pub fn permission_reply(&self, session: &str, request: &str, reply: &str) -> Result<Value, String> {
        let path = if self.is_v2() {format!("/api/session/{}/permission/{}/reply",encode_query(session),encode_query(request))}
            else {format!("/permission/{}/reply",encode_query(request))};
        self.post(&path,Some(&json!({"reply":reply})))
    }

    /// Background jobs this conversation and its subagents own. v2 only; older servers have no job list.
    pub fn jobs(&self, session: &str) -> Result<Vec<Value>, String> {
        if !self.is_v2() {
            return Err("This OpenCode server does not report background jobs".into());
        }
        let value = self.get(&format!(
            "/api/experimental/job?sessionID={}&descendants=true",
            encode_query(session)
        ))?;
        let list = value
            .as_array()
            .cloned()
            .or_else(|| value.get("jobs").and_then(Value::as_array).cloned())
            .or_else(|| value.get("items").and_then(Value::as_array).cloned())
            .or_else(|| value.get("data").and_then(Value::as_array).cloned())
            .or_else(|| value.pointer("/data/jobs").and_then(Value::as_array).cloned())
            .or_else(|| value.pointer("/data/items").and_then(Value::as_array).cloned())
            .ok_or("OpenCode returned an invalid background job list")?;
        Ok(list)
    }

    /// The sessions OpenCode is running right now. v2 lists active drains; v1 lists every busy session.
    pub fn active_sessions(&self) -> Result<std::collections::HashSet<String>, String> {
        let value = if self.is_v2() {
            self.get("/api/session/active")?
        } else {
            self.get("/session/status")?
        };
        if let Some(ids) = value.as_array() {
            return Ok(ids.iter().filter_map(Value::as_str).map(str::to_string).collect());
        }
        let map = value.as_object().ok_or("OpenCode returned an invalid session status list")?;
        let active = if self.is_v2() {
            map.keys().cloned().collect()
        } else {
            map.iter()
                .filter(|(_, status)| match status {
                    Value::String(kind) => kind != "idle",
                    _ => status.get("type").and_then(Value::as_str) != Some("idle"),
                })
                .map(|(id, _)| id.clone())
                .collect()
        };
        Ok(active)
    }

    /// Move the foreground work OpenCode is waiting on into the background.
    pub fn background(&self, session: &str) -> Result<Value, String> {
        let path = if self.is_v2() {
            format!("/api/session/{}/background", encode_query(session))
        } else {
            format!("/experimental/session/{}/background", encode_query(session))
        };
        self.post(&path, None)
    }

    /// Stop one session: a subagent task ends, or a whole turn when the root session is named.
    pub fn interrupt_session(&self, session: &str) -> Result<Value, String> {
        self.post(&format!("/session/{}/abort", encode_query(session)), None)
    }

    /// Terminate one shell command OpenCode owns. v2 keeps the command id the job reports.
    pub fn delete_shell(&self, shell: &str) -> Result<Value, String> {
        if !self.is_v2() {
            return Err("This OpenCode server cannot stop a background shell command".into());
        }
        self.delete(&format!("/api/shell/{}", encode_query(shell)))
    }

    /// The output one shell command has written so far, as plain text.
    pub fn shell_output(&self, shell: &str) -> Result<String, String> {
        if !self.is_v2() {
            return Err("This OpenCode server cannot read background shell output".into());
        }
        let value = self.get(&format!("/api/shell/{}/output", encode_query(shell)))?;
        let text = value.as_str().map(str::to_string)
            .or_else(|| value.get("output").and_then(Value::as_str).map(str::to_string))
            .or_else(|| value.get("text").and_then(Value::as_str).map(str::to_string))
            .or_else(|| value.pointer("/data/output").and_then(Value::as_str).map(str::to_string))
            .or_else(|| value.pointer("/data/text").and_then(Value::as_str).map(str::to_string));
        text.ok_or_else(|| "OpenCode returned an invalid shell output page".to_string())
    }
}

fn v2_providers(models: &Value, default: &Value, providers: &Value) -> Value {
    let mut groups: std::collections::BTreeMap<String, Value> = Default::default();
    for provider in providers.get("data").and_then(Value::as_array).into_iter().flatten() {
        let Some(id)=provider.get("id").and_then(Value::as_str) else {continue};
        groups.insert(id.to_string(),json!({"id":id,"name":provider.get("name").and_then(Value::as_str).unwrap_or(id),"models":{}}));
    }
    for model in models.get("data").and_then(Value::as_array).into_iter().flatten() {
        if model["enabled"]==false {continue;}
        let (Some(provider),Some(id))=(model["providerID"].as_str(),model["id"].as_str()) else {continue};
        let group=groups.entry(provider.to_string()).or_insert_with(||json!({"id":provider,"name":provider,"models":{}}));
        let mut value=model.clone();
        let variants: serde_json::Map<String,Value>=model.get("variants").and_then(Value::as_array).into_iter().flatten()
            .filter_map(|v|Some((v.get("id")?.as_str()?.to_string(),v.clone()))).collect();
        value["variants"]=json!(variants);
        group["models"][id]=value;
    }
    let connected: Vec<String>=groups.iter().filter(|(_,v)|v["models"].as_object().is_some_and(|m|!m.is_empty())).map(|(id,_)|id.clone()).collect();
    let mut defaults=serde_json::Map::new();
    if let (Some(provider),Some(id))=(default.pointer("/data/providerID").and_then(Value::as_str),default.pointer("/data/id").and_then(Value::as_str)) {
        defaults.insert(provider.into(),json!(id));
    }
    json!({"all":groups.into_values().collect::<Vec<_>>(),"connected":connected,"default":defaults})
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
                mode: agent.get("id").and_then(Value::as_str).unwrap_or(name).to_string(),
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
pub fn permission_payload(props: &Value, tool_input: Option<Value>, display_name: &str) -> Value {
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
        "display_name":display_name,
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
pub fn question_payload(props: &Value, display_name: &str) -> Option<Value> {
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
        "display_name":display_name,
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

    fn mock(responses: Vec<(&'static str,&'static str,Value,&'static str)>) -> (Server,std::thread::JoinHandle<()>) {
        let listener=loop {
            let port=10000+(uuid::Uuid::new_v4().as_u128()%39152) as u16;
            if let Ok(listener)=TcpListener::bind(("127.0.0.1",port)){break listener;}
        };
        let port=listener.local_addr().unwrap().port();
        let server=tiny_http::Server::from_listener(listener,None).unwrap();
        let handle=std::thread::spawn(move || {
            for (method,path,expected,response) in responses {
                let mut request=server.recv_timeout(Duration::from_secs(5)).unwrap().expect("expected request");
                assert_eq!(request.method().as_str(),method);
                assert!(request.url().starts_with(path),"unexpected route: {}",request.url());
                if !expected.is_null() {
                    let mut body=String::new();request.as_reader().read_to_string(&mut body).unwrap();
                    assert_eq!(serde_json::from_str::<Value>(&body).unwrap(),expected);
                }
                request.respond(tiny_http::Response::from_string(response)).unwrap();
            }
        });
        (Server::new(port,"fixture",Some("/tmp/my project")),handle)
    }

    #[test]
    fn current_v2_uses_server_info_and_retries_the_starting_model_catalogue() {
        let (server,worker)=mock(vec![
            ("GET","/global/health",Value::Null,"<!DOCTYPE html><html>OpenCode</html>"),
            ("GET","/api/info",Value::Null,"{\"version\":\"2.0.26\",\"pid\":123,\"capabilities\":{}}"),
            ("GET","/openapi.json",Value::Null,"{\"paths\":{\"/api/model/default\":{\"get\":{}}}}"),
            ("GET","/api/model/default?location%5Bdirectory%5D=",Value::Null,"{\"data\":{\"id\":\"test\",\"providerID\":\"fixture\"}}"),
            ("GET","/api/agent",Value::Null,"{\"data\":[{\"id\":\"build\",\"name\":\"Build\",\"mode\":\"primary\"}]}"),
            ("GET","/api/model/default",Value::Null,"{\"data\":{\"id\":\"test\",\"providerID\":\"fixture\"}}"),
            ("GET","/api/model",Value::Null,"{\"data\":[]}"),
            ("GET","/api/model",Value::Null,"{\"data\":[{\"id\":\"test\",\"providerID\":\"fixture\",\"name\":\"Test\",\"enabled\":true,\"variants\":[{\"id\":\"high\"}]}]}"),
            ("GET","/api/provider",Value::Null,"{\"data\":[{\"id\":\"fixture\",\"name\":\"Fixture\"}]}"),
            ("POST","/api/session/ses_1/model",json!({"model":{"id":"test","providerID":"fixture","variant":"high"}}),"null"),
        ]);
        assert!(server.healthy());assert!(server.is_v2());
        assert_eq!(parse_agents(&server.get("/agent").unwrap())[0].mode,"build");
        let providers=server.get("/provider").unwrap();
        let models=parse_models(&providers);
        assert_eq!(models.len(),1);
        assert_eq!(models[0].id,"fixture/test");
        assert_eq!(models[0].effort_levels,vec!["high"]);
        assert_eq!(default_model(&providers,None).as_deref(),Some("fixture/test"));
        server.post("/api/session/ses_1/model",Some(&json!({"model":{"id":"test","providerID":"fixture","variant":"high"}}))).unwrap();
        worker.join().unwrap();
    }

    #[test]
    fn a_missed_global_health_answer_does_not_flip_a_v1_server_onto_v2_routes() {
        // 1.19.0 answers /api/health too, while /api/info and /openapi.json are frontend HTML. One
        // slow /global/health must not be read as proof of v2: every later request would then be
        // answered by the HTML fallback and fail the conversation before it starts. A stable 1.x
        // version in the same answer must not change that, however a later v1 build spells it.
        for health in ["{\"healthy\":true}","{\"healthy\":true,\"version\":\"1.19.5\"}"] {
            let (server,worker)=mock(vec![
                ("GET","/global/health",Value::Null,"<!DOCTYPE html><html>OpenCode</html>"),
                ("GET","/api/info",Value::Null,"<!DOCTYPE html><html>OpenCode</html>"),
                ("GET","/api/health",Value::Null,health),
                ("GET","/session/status",Value::Null,"{}"),
            ]);
            assert!(server.healthy());
            assert!(!server.is_v2());
            assert_eq!(server.get("/session/status").unwrap(),json!({}));
            worker.join().unwrap();
        }
    }

    #[test]
    fn server_info_requires_a_real_server_response() {
        for info in ["<!DOCTYPE html><html>OpenCode</html>","{\"version\":\"2.0.26\"}","{\"version\":\"2.0.26\",\"pid\":0,\"capabilities\":{}}"] {
            let (server,worker)=mock(vec![
                ("GET","/global/health",Value::Null,"{}"),
                ("GET","/api/info",Value::Null,info),
                ("GET","/api/health",Value::Null,"{}"),
            ]);
            assert!(!server.healthy());assert!(!server.is_v2());
            worker.join().unwrap();
        }
    }

    #[test]
    fn v2_health_rejects_the_html_fallback_and_adapts_native_requests() {
        let (server,worker)=mock(vec![
            ("GET","/global/health",Value::Null,"<!DOCTYPE html><html>OpenCode</html>"),
            ("GET","/api/info",Value::Null,"<!DOCTYPE html><html>OpenCode</html>"),
            ("GET","/api/health",Value::Null,"{\"healthy\":true,\"version\":\"0.0.0-beta-17823\"}"),
            ("GET","/openapi.json",Value::Null,"{\"paths\":{}}"),
            ("POST","/api/session?location%5Bdirectory%5D=",json!({"location":{"directory":"/tmp/my project"}}),"{\"data\":{\"id\":\"ses_1\"}}"),
            ("POST","/api/session/ses_1/agent",json!({"agent":"build"}),"null"),
            ("POST","/api/session/ses_1/model",json!({"model":{"id":"test","providerID":"fixture","variant":"high"}}),"null"),
            ("POST","/api/session/ses_1/prompt",json!({"id":"msg_1","text":"Hello","delivery":"steer"}),"{\"data\":{\"id\":\"msg_1\"}}"),
            ("POST","/api/session/ses_child/permission/per_1/reply",json!({"reply":"always"}),"null"),
            ("POST","/api/session/ses_1/interrupt",json!({}),"null"),
            ("POST","/api/session/ses_1/revert/stage",json!({"messageID":"msg_1","files":true}),"null"),
            ("POST","/api/session/ses_1/revert/clear",json!({}),"null"),
        ]);
        assert!(server.healthy());assert!(server.is_v2());
        assert_eq!(server.post("/session",None).unwrap()["id"],"ses_1");
        let mut body=prompt_body("msg_1","Hello",&[],Some("build"),Some("fixture/test"),Some("high"));
        body["_delivery"]=json!("steer");
        server.post("/session/ses_1/prompt_async",Some(&body)).unwrap();
        server.permission_reply("ses_child","per_1","always").unwrap();
        server.post("/session/ses_1/abort",None).unwrap();
        server.post("/session/ses_1/revert",Some(&json!({"messageID":"msg_1"}))).unwrap();
        server.post("/session/ses_1/unrevert",None).unwrap();
        assert!(server.post("/session/ses_1/share",None).unwrap_err().contains("does not support"));
        worker.join().unwrap();
    }

    #[test]
    fn v1_health_keeps_the_existing_route_and_v2_history_follows_every_page() {
        let (v1,worker)=mock(vec![("GET","/global/health",Value::Null,"{\"healthy\":true,\"version\":\"1.18.31\"}")]);
        assert!(v1.healthy());assert!(!v1.is_v2());worker.join().unwrap();
        let (v2,worker)=mock(vec![
            ("GET","/api/session/ses_1/message?limit=100&order=asc",Value::Null,"{\"data\":[{\"id\":\"msg_1\",\"type\":\"user\",\"text\":\"first\"}],\"cursor\":{\"next\":\"next+page\"}}"),
            ("GET","/api/session/ses_1/message?limit=100&cursor=next%2Bpage",Value::Null,"{\"data\":[{\"id\":\"msg_2\",\"type\":\"assistant\",\"content\":[{\"type\":\"text\",\"text\":\"last\"}]}],\"cursor\":{}}"),
        ]);
        v2.version.store(2,Ordering::Relaxed);
        v2.initialized.store(true,Ordering::Relaxed);
        let messages=v2.get("/session/ses_1/message").unwrap();
        assert_eq!(messages.as_array().unwrap().len(),2);
        assert_eq!(messages[1].pointer("/parts/0/text"),Some(&json!("last")));
        worker.join().unwrap();
    }

    #[test]
    fn v2_background_jobs_use_the_native_routes() {
        let (server,worker)=mock(vec![
            ("GET","/api/experimental/job?sessionID=ses_1&descendants=true&location%5Bdirectory%5D=",Value::Null,
                "{\"data\":[{\"id\":\"job-1\",\"kind\":\"shell\",\"status\":\"running\",\"shellID\":\"sh_1\",\"command\":\"npm test\",\"time\":{\"started\":1000}}]}"),
            ("GET","/api/session/active?location%5Bdirectory%5D=",Value::Null,"{\"data\":{\"ses_1\":{},\"ses_child\":{}}}"),
            ("POST","/api/session/ses_1/background",json!({}),"null"),
            ("POST","/api/session/ses_child/interrupt",json!({}),"null"),
            ("DELETE","/api/shell/sh_1",Value::Null,""),
            ("GET","/api/shell/sh_1/output?location%5Bdirectory%5D=",Value::Null,"{\"data\":{\"output\":\"running tests\\n\"}}"),
        ]);
        server.version.store(2,Ordering::Relaxed);
        server.initialized.store(true,Ordering::Relaxed);
        let jobs=server.jobs("ses_1").unwrap();
        assert_eq!(jobs[0]["shellID"],json!("sh_1"));
        let active=server.active_sessions().unwrap();
        assert!(active.contains("ses_child"));
        server.background("ses_1").unwrap();
        server.interrupt_session("ses_child").unwrap();
        server.delete_shell("sh_1").unwrap();
        assert_eq!(server.shell_output("sh_1").unwrap(),"running tests\n");
        worker.join().unwrap();
    }

    #[test]
    fn newer_v2_waits_for_native_plugin_activation_before_reading_agents() {
        let (server,worker)=mock(vec![
            ("GET","/openapi.json",Value::Null,"{\"paths\":{\"/api/plugin/await-activation\":{\"post\":{}}}}"),
            ("POST","/api/plugin/await-activation",json!({}),""),
            ("GET","/api/agent",Value::Null,"{\"data\":[{\"id\":\"build\",\"name\":\"Build\",\"mode\":\"primary\"}]}"),
        ]);
        server.version.store(2,Ordering::Relaxed);
        assert_eq!(parse_agents(&server.get("/agent").unwrap())[0].mode,"build");
        worker.join().unwrap();
    }

    /// Test peers must use isolated native stores and the local fixture model, with no external API.
    #[test]
    #[ignore = "requires isolated real v1/v2 API peers and a local model fixture"]
    fn real_opencode_v1_v2_protocol_round_trip() {
        let file=std::env::var("VLX_OPENCODE_TEST_PEERS").expect("isolated test peers required");
        let peers:Value=serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        for name in ["v1","v2"] {
            let peer=&peers[name];
            let server=Server::new(peer["port"].as_u64().unwrap() as u16,peer["password"].as_str().unwrap(),peer["directory"].as_str());
            assert!(server.healthy());assert_eq!(server.is_v2(),name=="v2");
            assert!(parse_agents(&server.get("/agent").unwrap()).iter().any(|a|a.mode=="build"));
            let providers=server.get("/provider").unwrap();
            let models=parse_models(&providers);
            assert!(models.iter().any(|m|m.id=="vlx-test/smoke" && m.effort_levels.iter().any(|e|e=="high")));
            let native=server.post("/session",None).unwrap();
            let id=native["id"].as_str().unwrap();
            let reader=server.open_events().unwrap();
            let (tx,rx)=std::sync::mpsc::channel();
            let owner=id.to_string();
            std::thread::spawn(move || {
                use std::io::BufRead;
                let mut events=Vec::new();
                for line in std::io::BufReader::new(reader).lines().map_while(Result::ok) {
                    let Some(event)=parse_sse_line(&line) else {continue};
                    let session=event.pointer("/data/sessionID").or_else(||event.pointer("/properties/sessionID"))
                        .or_else(||event.pointer("/properties/part/sessionID")).or_else(||event.pointer("/properties/info/sessionID"));
                    if session.and_then(Value::as_str)!=Some(&owner){continue;}
                    let done=matches!(event["type"].as_str(),Some("session.execution.succeeded"|"session.execution.failed"|"session.idle"))
                        || event.pointer("/properties/status/type").and_then(Value::as_str)==Some("idle");
                    events.push(event);
                    if done{let _=tx.send(events);return;}
                }
            });
            let message_id=new_message_id();
            server.post(&format!("/session/{id}/prompt_async"),Some(&prompt_body(&message_id,"Protocol fixture",&[],Some("build"),Some("vlx-test/smoke"),Some("high")))).unwrap();
            let events=rx.recv_timeout(Duration::from_secs(30)).expect("native turn should finish");
            if server.is_v2(){assert!(events.iter().any(|e|e["type"]=="session.text.delta"));}
            else {assert!(events.iter().any(|e|e["type"]=="message.part.updated" || e["type"]=="message.part.delta"));}
            let history=server.get(&format!("/session/{id}/message")).unwrap();
            assert!(history.as_array().unwrap().iter().any(|m|m.pointer("/info/id").and_then(Value::as_str)==Some(&message_id)));
            assert!(history.as_array().unwrap().iter().flat_map(|m|m["parts"].as_array().into_iter().flatten()).any(|p|p["text"]=="Protocol fixture reply"));
            let choice=json!({"id":"smoke","providerID":"vlx-test","variant":"high"});
            server.post(&format!("/api/session/{id}/model"),Some(&json!({"model":choice}))).unwrap();
            assert_eq!(server.get(&format!("/session/{id}")).unwrap()["model"],choice);
            server.post(&format!("/session/{id}/abort"),None).unwrap();
            if server.is_v2() {
                let reader=server.open_events().unwrap();
                let (tx,rx)=std::sync::mpsc::channel();
                let owner=id.to_string();
                std::thread::spawn(move || {
                    use std::io::BufRead;
                    for line in std::io::BufReader::new(reader).lines().map_while(Result::ok) {
                        let Some(event)=parse_sse_line(&line) else {continue};
                        if event.pointer("/data/sessionID").and_then(Value::as_str)!=Some(&owner){continue;}
                        if matches!(event["type"].as_str(),Some("session.shell.started"|"session.shell.ended")) {
                            let done=event["type"]=="session.shell.ended";
                            let _=tx.send(event);
                            if done{return;}
                        }
                    }
                });
                let message=new_message_id();
                std::thread::scope(|scope| {
                    let shell=scope.spawn(||server.post_turn(&format!("/session/{id}/shell"),Some(&shell_body(&message,"printf fixture; sleep 3; printf late","build",None))));
                    let started=rx.recv_timeout(Duration::from_secs(10)).unwrap();
                    let shell_id=started.pointer("/data/shell/id").and_then(Value::as_str).unwrap();
                    server.delete(&format!("/api/shell/{}",encode_query(shell_id))).unwrap();
                    let ended=rx.recv_timeout(Duration::from_secs(10)).unwrap();
                    assert_eq!(ended["type"],"session.shell.ended");
                    assert!(!ended.pointer("/data/output/output").and_then(Value::as_str).unwrap_or("").contains("late"));
                    shell.join().unwrap().unwrap();
                });
            }
        }
    }

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
        let payload = permission_payload(&props, None, "OpenCode");
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
        let payload = question_payload(&props, "OpenCode").unwrap();
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
