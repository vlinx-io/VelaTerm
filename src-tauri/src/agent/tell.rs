//! Authenticated session messages and the executor's explicit report handoff.

use std::io::{IsTerminal, Read};
use std::time::{Duration, Instant};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{command_core as core, db::repo, host::AppCtx, models::Session};

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS session_tells (
 id TEXT PRIMARY KEY, sender_id TEXT NOT NULL,
 target_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
 fingerprint TEXT NOT NULL, wire TEXT NOT NULL, origin TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS session_tells_target ON session_tells(target_id);
";

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub session_id: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub report: bool,
    /// Join the recipient's running turn instead of waiting for it to end.
    #[serde(default)]
    pub steer: bool,
    #[serde(default)]
    pub round: Option<u32>,
    pub message_id: String,
    pub text: String,
}

pub(super) fn session(app: &AppCtx, id: &str) -> Result<Session, String> {
    repo::get_session(&app.db().conn.lock().unwrap(), id)?
        .filter(|s| s.archived_at.is_none())
        .ok_or_else(|| "The session is missing or archived".into())
}

fn resolve(app: &AppCtx, reference: &str) -> Result<String, String> {
    match repo::resolve_session_ref(&app.db().conn.lock().unwrap(), reference)? {
        repo::SessionRefMatch::One(id) => Ok(id),
        repo::SessionRefMatch::None => Err("Target session not found".into()),
        repo::SessionRefMatch::Ambiguous(candidates) => Err(format!(
            "Ambiguous target; use a session ID:\n{}",
            candidates
                .iter()
                .map(|(id, name)| format!("{id}  {name}"))
                .collect::<Vec<_>>()
                .join("\n")
        )),
    }
}

pub(super) fn validate_message(id: &str, text: &str) -> Result<(), String> {
    if !id.starts_with("msg-") || uuid::Uuid::parse_str(&id[4..]).is_err() {
        return Err("Use a stable msg-UUID message ID for every submission and its retries".into());
    }
    if text.trim().is_empty() || text.len() > 65536 {
        return Err("Submit between 1 and 65536 bytes of message text".into());
    }
    Ok(())
}

pub fn send(app: &AppCtx, req: &Request) -> Result<Value, String> {
    validate_message(&req.message_id, &req.text)?;
    let target = req
        .target
        .as_deref()
        .map(|value| resolve(app, value))
        .transpose()?;
    if req.report && req.steer {
        // Reports are read after the planner's current turn, never inside it.
        return Err("--steer cannot be combined with --report".into());
    }
    if req.report {
        return super::plan_execute::report(app, req, target.as_deref());
    }
    if req.round.is_some() {
        return Err("--round requires --report".into());
    }
    let target = target.ok_or("Specify a target session")?;
    let _guard = super::plan_execute::operation_lock().lock().unwrap();
    let sender = session(app, &req.session_id)?;
    let recipient = session(app, &target)?;
    if sender.id == target {
        return Err("Choose another session as the target".into());
    }
    if recipient.engine != "chat" || !recipient.kind.supports_chat() {
        return Err("The target must be a native chat session".into());
    }
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&json!([sender.id, target, req.text])).unwrap())
    );
    let wire = {
        let conn = app.db().conn.lock().unwrap();
        let workflow_message = conn
            .query_row(
                "SELECT 1 FROM plan_execute_messages WHERE id=?1",
                [&req.message_id],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if workflow_message.is_some() {
            return Err("This message ID already belongs to a workflow submission".into());
        }
        let saved: Option<(String, String)> = conn
            .query_row(
                "SELECT fingerprint,wire FROM session_tells WHERE id=?1",
                [&req.message_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some((hash, wire)) = saved {
            if hash != fingerprint {
                return Err("This message ID already belongs to a different request".into());
            }
            wire
        } else {
            let origin = json!({"sessionId":sender.id,"name":sender.name,"agent":sender.kind,"role":"session"});
            let wire = format!(
                "[VelaTerm message {}]\n{origin}\n\n{}",
                req.message_id, req.text
            )
            .trim_end()
            .to_owned();
            conn.execute("INSERT INTO session_tells(id,sender_id,target_id,fingerprint,wire,origin) VALUES (?1,?2,?3,?4,?5,?6)",
                params![req.message_id, sender.id, target, fingerprint, wire, origin.to_string()]).map_err(|e|e.to_string())?;
            wire
        }
    };
    let delivery = deliver(app, &target, &wire, &req.message_id, behavior(req.steer))?;
    Ok(json!({"delivery":delivery,"messageId":req.message_id,"targetSessionId":target}))
}

/// The submission behavior a request asks for. Steering joins the turn already running; queuing waits
/// for it to end. A steer with no turn to join is sent as an ordinary message rather than refused.
pub(super) fn behavior(steer: bool) -> &'static str {
    if steer { "steer" } else { "queue" }
}

/// Both workflow and ordinary messages use native chat submission receipts and busy-session queues.
pub(super) fn deliver(
    app: &AppCtx,
    target: &str,
    wire: &str,
    id: &str,
    behavior: &str,
) -> Result<&'static str, String> {
    deliver_with_images(app, target, wire, id, vec![], behavior)
}

pub(super) fn deliver_with_images(
    app: &AppCtx, target: &str, wire: &str, id: &str,
    images: Vec<super::chat::protocol::ChatImage>, behavior: &str,
) -> Result<&'static str, String> {
    let result = (|| {
        if !app.chat().is_alive(target) {
            core::chat_start(app, target, None, None, false)?;
        }
        core::chat_send(app, target, wire, images, Some(behavior), Some(id))
    })();
    result.map_err(|error| format!("Delivery to {target} failed: {error}. Message {id} is retained; retry the same command with this message ID and unchanged text."))
}

/// Only the target's exact persisted wire message can acquire a sender identity.
pub fn decorate(app: &AppCtx, target: &str, value: &mut Value) {
    if let Some(items) = value.as_array_mut() {
        for item in items {
            decorate(app, target, item);
        }
        return;
    }
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    if let Some(text) = obj.get("text").and_then(Value::as_str) {
        if let Some(id) = text
            .strip_prefix("[VelaTerm message ")
            .and_then(|s| s.split_once("]\n"))
            .map(|(id, _)| id)
        {
            let found: Option<String> = app.db().conn.lock().unwrap().query_row(
                "SELECT origin FROM session_tells WHERE id=?1 AND target_id=?2 AND wire=?3 UNION ALL SELECT origin FROM plan_execute_messages WHERE id=?1 AND target_id=?2 AND wire=?3 LIMIT 1",
                params![id,target,text], |r|r.get(0)).optional().ok().flatten();
            if let Some(origin) = found.and_then(|s| serde_json::from_str::<Value>(&s).ok()) {
                let body = text
                    .split_once("\n\n")
                    .map(|(_, s)| s.to_owned())
                    .unwrap_or_default();
                if origin["role"] != "user" {
                    obj.insert("origin".into(), origin);
                }
                obj.insert("text".into(), Value::String(body));
            }
        }
    }
    for key in ["rows", "items", "queue", "children", "messages"] {
        if let Some(child) = obj.get_mut(key) {
            decorate(app, target, child);
        }
    }
}

pub fn handle(app: AppCtx, mut request: crate::diagnostics::HttpRequest, token: String) {
    let started = Instant::now();
    let request_id = request.id().to_owned();
    let authorized = request
        .headers()
        .iter()
        .any(|h| h.field.equiv("Authorization") && h.value.as_str() == format!("Bearer {token}"));
    let mut body = String::new();
    let result = if !authorized {
        Err((401, "Authentication required".to_owned()))
    } else if request.method() != &tiny_http::Method::Post {
        Err((405, "Use POST".to_owned()))
    } else if request
        .as_reader()
        .take(524289)
        .read_to_string(&mut body)
        .is_err()
        || body.len() > 524288
    {
        Err((413, "Request too large".to_owned()))
    } else {
        serde_json::from_str::<Request>(&body)
            .map_err(|e| e.to_string())
            .and_then(|req| send(&app, &req))
            .map_err(|e| (400, e))
    };
    let (status, value) = match result {
        Ok(result) => (200, json!({"result":result})),
        Err((status, error)) => (status, json!({"error":error})),
    };
    let _ = request.respond(
        tiny_http::Response::from_string(value.to_string())
            .with_status_code(status)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
            ),
    );
    crate::diagnostics::record(
        if status == 200 { "INFO" } else { "WARN" },
        "session_tell",
        json!({"requestId":request_id,"statusCode":status,"durationMs":started.elapsed().as_millis() as u64}),
    );
}

const USAGE: &str = "usage: vtell <session> [--steer] [--message-id msg-UUID] [message...]
       vtell [planner-session] --report --round N [--message-id msg-UUID] [message...]
Read UTF-8 text from stdin when no message arguments are provided.
Targets accept a session ID, an ID prefix of at least 8 characters, or an unambiguous name.
--steer joins the turn the recipient is running instead of waiting for it to end. The delivery
receipt reads \"steered\" only when the message actually joined a running turn.
--report submits the executor's result for review; its planner is the default target.
Keep the printed message ID and exact text when retrying an uncertain delivery.";

fn parse_args(args: &[String]) -> Result<Option<Request>, String> {
    let mut req = Request {
        message_id: format!("msg-{}", uuid::Uuid::new_v4()),
        ..Default::default()
    };
    let mut words = Vec::new();
    let mut options = true;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--help" | "-h" if options => return Ok(None),
            "--" if options => options = false,
            "--report" if options => req.report = true,
            "--steer" if options => req.steer = true,
            _ if options
                && (arg == "--round"
                    || arg == "--message-id"
                    || arg.starts_with("--round=")
                    || arg.starts_with("--message-id=")) =>
            {
                let (flag, inline) = arg
                    .split_once('=')
                    .map(|(k, v)| (k, Some(v)))
                    .unwrap_or((arg, None));
                let value = match inline {
                    Some(v) => v,
                    None => {
                        i += 1;
                        args.get(i)
                            .ok_or_else(|| format!("{flag} needs a value"))?
                            .as_str()
                    }
                };
                if flag == "--round" {
                    req.round = Some(value.parse().map_err(|_| "Invalid round")?);
                } else {
                    req.message_id = value.to_owned();
                }
            }
            _ if options && arg.starts_with('-') => return Err(format!("Unknown option {arg}")),
            _ if req.target.is_none() => req.target = Some(arg.to_owned()),
            _ => words.push(arg),
        }
        i += 1;
    }
    if req.report && req.round.is_none() {
        return Err("--report requires --round N; read the current round with vflow status".into());
    }
    if !req.report && req.round.is_some() {
        return Err("--round requires --report".into());
    }
    if req.report && req.steer {
        return Err("--steer cannot be combined with --report".into());
    }
    if !req.report && req.target.is_none() {
        return Err("Specify a target session".into());
    }
    req.text = words.join(" ");
    Ok(Some(req))
}

pub fn run_cli(args: &[String]) -> ! {
    let result = (|| -> Result<Value, String> {
        let Some(mut req) = parse_args(&args[args.len().min(2)..])? else {
            println!("{USAGE}");
            std::process::exit(0);
        };
        req.session_id =
            std::env::var("VLX_SESSION_ID").map_err(|_| "Run inside a VelaTerm session")?;
        if req.text.is_empty() {
            if std::io::stdin().is_terminal() {
                return Err("Provide message text as arguments or on stdin".into());
            }
            std::io::stdin()
                .take(65537)
                .read_to_string(&mut req.text)
                .map_err(|_| "Cannot read message text")?;
        }
        validate_message(&req.message_id, &req.text)?;
        // Print before sending so an interrupted or timed-out command leaves a recoverable submission ID.
        eprintln!("vtell: messageId={}", req.message_id);
        post_local("tell", &serde_json::to_value(req).unwrap())
    })();
    match result {
        Ok(value) => {
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("vtell: {error}");
            std::process::exit(1);
        }
    }
}

pub(super) fn post_local(route: &str, body: &Value) -> Result<Value, String> {
    let base = std::env::var("VLX_SPAWN_URL").map_err(|_| "Missing VelaTerm endpoint")?;
    let url = url::Url::parse(&base).map_err(|_| "Invalid VelaTerm endpoint")?;
    if url.scheme() != "http" || !matches!(url.host_str(), Some("127.0.0.1" | "localhost")) {
        return Err("Expected the local VelaTerm endpoint".into());
    }
    let token = super::session_env::token().ok_or("Missing VelaTerm credentials")?;
    let response = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(50))
        .redirects(0)
        .try_proxy_from_env(false)
        .build()
        .post(&format!("{}/{route}", base.trim_end_matches('/')))
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .set("X-Request-Id", &uuid::Uuid::new_v4().to_string())
        .send_string(&body.to_string());
    let response = match response {
        Ok(r) | Err(ureq::Error::Status(_, r)) => r,
        Err(_) => return Err("Cannot reach VelaTerm; retry with the same message ID".into()),
    };
    let mut body = String::new();
    response
        .into_reader()
        .take(1048576)
        .read_to_string(&mut body)
        .map_err(|_| "Cannot read response")?;
    let value: Value = serde_json::from_str(&body).map_err(|_| "Invalid VelaTerm response")?;
    if let Some(error) = value["error"].as_str() {
        return Err(error.into());
    }
    value
        .get("result")
        .cloned()
        .ok_or_else(|| "Invalid VelaTerm response".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).into()).collect()
    }

    #[test]
    fn parse_messages_and_report_destinations() {
        let req = parse_args(&args(&["Planning session", "A message", "with spaces"]))
            .unwrap()
            .unwrap();
        assert_eq!(req.target.as_deref(), Some("Planning session"));
        assert_eq!(req.text, "A message with spaces");
        assert!(!req.report);
        let req = parse_args(&args(&["--report", "--round=2"]))
            .unwrap()
            .unwrap();
        assert!(req.report);
        assert_eq!(req.round, Some(2));
        assert!(req.target.is_none());
        let req = parse_args(&args(&[
            "planner",
            "--report",
            "--round",
            "2",
            "--",
            "--literal report",
        ]))
        .unwrap()
        .unwrap();
        assert_eq!(req.text, "--literal report");
        assert_eq!(req.target.as_deref(), Some("planner"));
    }

    #[test]
    fn parse_reads_the_steer_flag_and_maps_it_to_a_submission_behavior() {
        let req = parse_args(&args(&["executor", "A remark on the work in progress"]))
            .unwrap()
            .unwrap();
        assert!(!req.steer);
        assert_eq!(behavior(req.steer), "queue");
        let req = parse_args(&args(&["executor", "--steer", "A remark on the work in progress"]))
            .unwrap()
            .unwrap();
        assert!(req.steer);
        assert_eq!(behavior(req.steer), "steer");
        assert_eq!(req.text, "A remark on the work in progress");
        let req = parse_args(&args(&["executor", "--", "--steer", "is literal here"]))
            .unwrap()
            .unwrap();
        assert!(!req.steer);
        assert_eq!(req.text, "--steer is literal here");
    }

    #[test]
    fn parse_rejects_missing_or_misplaced_round_and_keeps_retry_id() {
        for values in [
            vec![],
            vec!["--report"],
            vec!["planner", "--round", "1"],
            vec!["--report", "--round", "invalid"],
            vec!["planner", "--unknown"],
            vec!["--report", "--round", "1", "--steer"],
        ] {
            assert!(parse_args(&args(&values)).is_err(), "{values:?}");
        }
        let id = format!("msg-{}", uuid::Uuid::new_v4());
        let req = parse_args(&args(&["planner", "--message-id", &id, "Hello"]))
            .unwrap()
            .unwrap();
        assert_eq!(req.message_id, id);
        validate_message(&req.message_id, &req.text).unwrap();
        assert!(validate_message("forged", "Hello").is_err());
        assert!(validate_message(&id, " ").is_err());
        assert!(validate_message(&id, &"x".repeat(65537)).is_err());
    }

    #[test]
    fn request_does_not_accept_client_supplied_sender_labels() {
        let req = json!({"sessionId":"s","target":"t","messageId":"msg-test","text":"Hi","origin":{"name":"Another session"}});
        assert!(serde_json::from_value::<Request>(req).is_err());
    }
}
