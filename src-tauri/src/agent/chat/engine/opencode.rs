//! OpenCode inside the chat engine: a local server, its event stream, and the timeline they produce.
//!
//! OpenCode's terminal interface is a client of `opencode serve`; this engine is another one. The engine
//! starts a server for the session on a private loopback port, creates or resumes the conversation over
//! HTTP, and follows `GET /event` for everything that happens — text as it streams, tool calls as they
//! run, permission and question prompts, subagents working in sessions of their own. Answers, stops, and
//! rewinds go back as requests.
//!
//! What OpenCode calls an *agent* (build, plan, or one the user defined) is the same axis the composer
//! shows for Codex as the collaboration style, so it travels through the same fields and the same chip.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::{
    emit, emit_state, fail_codex_start as fail_start, handle_compaction, handle_turn_end, now_ms,
    start_waiting_message, ChatProcess, ChatRow, RowPath,
};
use crate::agent::chat::history::RewindTarget;
use crate::agent::chat::opencode_protocol::{self as wire, Server};
use crate::agent::chat::opencode_timeline as timeline;
use crate::agent::chat::protocol::ChatImage;
use crate::agent::opencode_store::OpencodeMessage;
use crate::host::AppCtx;
use crate::models::SessionKind;
use crate::pty::AgentState;

/// How long the server may take to answer its first health check.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
/// Attempts to reopen the event stream while the server process is still alive.
const EVENT_RECONNECT_ATTEMPTS: u32 = 5;
/// Bounds on child-session events kept while the Task card that owns them is not known yet.
const MAX_PENDING_CHILDREN: usize = 32;
const MAX_PENDING_CHILD_EVENTS: usize = 128;

/// Everything the engine keeps about the OpenCode side of one session, behind one lock.
#[derive(Default)]
pub(super) struct OpencodeState {
    /// The server, once it answers.
    pub(super) server: Option<Arc<Server>>,
    /// Message id to role. Parts do not say whose message they belong to.
    roles: HashMap<String, String>,
    /// Reply models and their text parts, including parts received before message metadata.
    models: HashMap<String, String>,
    message_parts: HashMap<String, std::collections::HashSet<String>>,
    /// Assistant messages that are compaction summaries; their prose is not an answer.
    summaries: std::collections::HashSet<String>,
    /// Tool call id to the path and row of the call it drew, so a permission naming the call finds it.
    calls: HashMap<String, (RowPath, String)>,
    /// Message ids this engine chose for its own turns, to the row already showing them.
    user_messages: HashMap<String, String>,
    /// The model the server would use on its own, for requests that must name one.
    pub(super) default_model: Option<String>,
    /// Both idle event forms describe the same transition; only a preceding busy state ends a turn.
    busy: bool,
}

fn server(proc: &ChatProcess) -> Result<Arc<Server>, String> {
    proc.opencode
        .lock()
        .unwrap()
        .server
        .clone()
        .ok_or_else(|| "OpenCode has not started yet".to_string())
}

fn root_session(proc: &ChatProcess) -> Result<String, String> {
    proc.agent_session_id
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "OpenCode has not opened its session yet".to_string())
}

/// The TUI has no variant flag. Its native model endpoint persists both fields without a fake prompt.
pub(super) fn prepare_terminal(proc: &ChatProcess, selection: &crate::agent::session_settings::Selection) -> Result<(), String> {
    let server = server(proc)?;
    let id = root_session(proc)?;
    let model = selection.model.clone()
        .or_else(|| proc.model.lock().unwrap().clone())
        .or_else(|| proc.opencode.lock().unwrap().default_model.clone())
        .ok_or("OpenCode has no model available for the terminal")?;
    let (provider, model) = wire::split_model(&model).ok_or("The OpenCode model must include its provider")?;
    let native = server.get(&format!("/session/{}", wire::encode_query(&id)))?;
    // A fresh session needs an agent for the native TUI to restore its saved model and variant.
    if native.get("agent").and_then(Value::as_str).is_none() {
        let config = server.get("/config")?;
        let modes = proc.collaboration_modes.lock().unwrap();
        let agent = config.get("default_agent").and_then(Value::as_str)
            .or_else(|| modes.iter().find(|m| m.mode == "build").map(|m| m.mode.as_str()))
            .or_else(|| modes.first().map(|m| m.mode.as_str()))
            .ok_or("OpenCode has no primary agent available for the terminal")?;
        server.post(&format!("/api/session/{}/agent", wire::encode_query(&id)), Some(&json!({"agent":agent})))?;
    }
    let choice = json!({"id":model,"providerID":provider,"variant":selection.effort.as_deref().unwrap_or("default")});
    server.post(&format!("/api/session/{}/model", wire::encode_query(&id)), Some(&json!({"model":choice})))?;
    let confirmed = server.get(&format!("/session/{}", wire::encode_query(&id)))?;
    if confirmed.get("model") != Some(&choice) {
        return Err("OpenCode did not retain the model and effort selected for the terminal".into());
    }
    Ok(())
}

// ─────────────────────────── Startup ───────────────────────────

/// Bring the server up, open the conversation, and start following its events. Runs on its own thread
/// because the server takes a moment to answer and nothing may block the caller meanwhile.
pub(super) fn bootstrap(
    app: AppCtx,
    session_id: String,
    proc: Arc<ChatProcess>,
    port: u16,
    password: String,
    resume: Option<String>,
) {
    std::thread::spawn(move || {
        let server = Arc::new(Server::new(port, &password, proc.cwd.as_deref()));
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        while !server.healthy() {
            if !proc.alive.load(Ordering::Relaxed) {
                return;
            }
            if Instant::now() > deadline {
                emit(
                    &app,
                    &session_id,
                    json!({"type":"error","message":"OpenCode's server did not start in time"}),
                );
                fail_start(&app, &session_id, &proc);
                return;
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        proc.opencode.lock().unwrap().server = Some(server.clone());

        let opened = match &resume {
            Some(id) => match server.get(&format!("/session/{id}")) {
                Ok(session) => Ok(session),
                // A session OpenCode no longer has cannot be resumed; a fresh one loses nothing that
                // could be shown anyway.
                Err(e) if wire::is_not_found(&e) => {
                    eprintln!("chat: OpenCode has no session {id}; starting a new one");
                    server.post("/session", None)
                }
                Err(e) => Err(e),
            },
            None => server.post("/session", None),
        };
        let session = match opened {
            Ok(session) => session,
            Err(message) => {
                emit(&app, &session_id, json!({"type":"error","message":message}));
                fail_start(&app, &session_id, &proc);
                return;
            }
        };
        let Some(native_id) = session.get("id").and_then(Value::as_str).map(str::to_string) else {
            emit(
                &app,
                &session_id,
                json!({"type":"error","message":"OpenCode did not return a session id"}),
            );
            fail_start(&app, &session_id, &proc);
            return;
        };
        remember_session(&app, &session_id, &proc, &native_id, &session);

        // Establish the subscription before ready can release the first queued prompt.
        let reader = match server.open_events() {
            Ok(reader) => reader,
            Err(message) => {
                emit(&app, &session_id, json!({"type":"error","message":message}));
                fail_start(&app, &session_id, &proc);
                return;
            }
        };
        spawn_event_reader(app.clone(), session_id.clone(), proc.clone(), server.clone(), reader);

        // What the server offers: agents for the style chip, commands for `/` completion, and the model
        // it would pick by itself. None of these is fatal when missing; the conversation still works.
        match server.get("/agent") {
            Ok(agents) => announce_agents(&app, &session_id, &proc, &agents, session.get("agent")),
            Err(e) => eprintln!("chat: OpenCode agents unavailable: {e}"),
        }
        match server.get("/command") {
            Ok(commands) => {
                let catalogue = wire::parse_commands(&commands);
                *proc.commands.lock().unwrap() = catalogue.clone();
                emit(&app, &session_id, json!({"type":"commands","commands":catalogue}));
            }
            Err(e) => eprintln!("chat: OpenCode commands unavailable: {e}"),
        }
        if proc.opencode.lock().unwrap().default_model.is_none() {
            if let Ok(providers) = server.get("/provider") {
                let preferred = session
                    .pointer("/model/providerID")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                proc.opencode.lock().unwrap().default_model =
                    wire::default_model(&providers, preferred.as_deref());
            }
        }

        // The store was replayed when the process started; the server's own answer is authoritative and
        // also carries the child sessions' work and any staged revert.
        if resume.is_some() {
            if let Err(e) = rebuild_timeline(&proc, &server, &native_id) {
                eprintln!("chat: OpenCode history not reloaded: {e}");
            }
        }
        proc.ready.store(true, Ordering::Relaxed);
        emit_state(&app, &session_id, AgentState::Waiting);
        start_waiting_message(&app, &session_id, &proc);
    });
}

/// Keep the native session id, persist it for the next resume, and tell the clients about it along with
/// the model and agent the session already carries.
fn remember_session(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, native_id: &str, session: &Value) {
    *proc.agent_session_id.lock().unwrap() = Some(native_id.to_string());
    let stored_model = session.get("model").and_then(|model| {
        let provider = model.get("providerID").and_then(Value::as_str)?;
        let id = model.get("id").and_then(Value::as_str)?;
        Some(format!("{provider}/{id}"))
    });
    let stored_variant = session
        .pointer("/model/variant")
        .and_then(Value::as_str)
        .filter(|v| *v != "default")
        .map(str::to_string);
    let keep_auto = crate::agent::session_settings::keeps_automatic_effort(app, session_id);
    {
        let mut state = proc.opencode.lock().unwrap();
        if state.default_model.is_none() {
            state.default_model = stored_model.clone();
        }
    }
    // What was chosen in this pane wins; the session's own memory fills in only what nobody chose.
    {
        let mut model = proc.model.lock().unwrap();
        if model.is_none() {
            *model = stored_model;
        }
    }
    {
        let mut effort = proc.effort.lock().unwrap();
        if effort.is_none() && !keep_auto {
            *effort = stored_variant;
        }
    }
    let changed = {
        let conn = app.db().conn.lock().unwrap();
        crate::db::repo::set_agent_session_id(&conn, session_id, native_id, SessionKind::Opencode)
            .unwrap_or(false)
    };
    if changed {
        app.emit(crate::host::TREE_CHANGED, ());
    }
    let model = proc.model.lock().unwrap().clone();
    let effort = proc.effort.lock().unwrap().clone();
    let collaboration_mode = proc.collaboration_mode.lock().unwrap().clone();
    emit(
        app,
        session_id,
        json!({
            "type":"session",
            "agentSessionId":native_id,
            "model":model,
            "effort":effort,
            "collaborationMode":collaboration_mode
        }),
    );
}

/// Publish the primary agents as collaboration presets, keeping the stored choice when it still exists.
fn announce_agents(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, agents: &Value, session_agent: Option<&Value>) {
    let presets = wire::parse_agents(agents);
    if presets.is_empty() {
        *proc.collaboration_mode.lock().unwrap() = None;
        *proc.collaboration_modes.lock().unwrap() = Vec::new();
        emit(app, session_id, json!({"type":"collaborationModes","modes":[],"mode":""}));
        return;
    }
    let chosen = proc.collaboration_mode.lock().unwrap().clone();
    let remembered = session_agent.and_then(Value::as_str).map(str::to_string);
    let selected = [chosen, remembered]
        .into_iter()
        .flatten()
        .find(|name| presets.iter().any(|preset| &preset.mode == name))
        .or_else(|| presets.iter().find(|p| p.mode == "build").map(|p| p.mode.clone()))
        .unwrap_or_else(|| presets[0].mode.clone());
    *proc.collaboration_mode.lock().unwrap() = Some(selected.clone());
    *proc.collaboration_modes.lock().unwrap() = presets.clone();
    emit(
        app,
        session_id,
        json!({"type":"collaborationModes","modes":presets,"mode":selected}),
    );
}

// ─────────────────────────── Event stream ───────────────────────────

fn spawn_event_reader(
    app: AppCtx,
    session_id: String,
    proc: Arc<ChatProcess>,
    server: Arc<Server>,
    reader: Box<dyn std::io::Read + Send>,
) {
    std::thread::spawn(move || {
        let mut failures = 0;
        let mut initial = Some(reader);
        while proc.alive.load(Ordering::Relaxed) {
            let reader = match initial.take().map(Ok).unwrap_or_else(|| server.open_events()) {
                Ok(reader) => reader,
                Err(e) => {
                    failures += 1;
                    if failures > EVENT_RECONNECT_ATTEMPTS {
                        emit(&app, &session_id, json!({"type":"error","message":e}));
                        fail_start(&app, &session_id, &proc);
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(300 * failures as u64));
                    continue;
                }
            };
            let mut delivered = false;
            for line in BufReader::new(reader).lines() {
                let Ok(line) = line else { break };
                let Some(event) = wire::parse_sse_line(&line) else { continue };
                delivered = true;
                handle_event(&app, &session_id, &proc, event);
            }
            if delivered {
                failures = 0;
            }
            // The stream ends when the server stops; while the process is still there, reopen it.
            std::thread::sleep(Duration::from_millis(200));
        }
    });
}

/// Where an event belongs: the conversation itself, a subagent's card, or a child not yet linked.
enum Route {
    Root,
    Child(RowPath),
    Unknown(String),
}

fn route_for(proc: &ChatProcess, event_session: Option<&str>) -> Route {
    let root = proc.agent_session_id.lock().unwrap().clone();
    match event_session {
        None => Route::Root,
        Some(id) if root.as_deref() == Some(id) => Route::Root,
        Some(id) => match proc.codex_subagent_ids.lock().unwrap().get(id) {
            Some(path) => Route::Child(path.clone()),
            None => Route::Unknown(id.to_string()),
        },
    }
}

fn event_session_id(event: &Value) -> Option<&str> {
    let props = event.get("properties")?;
    props
        .get("sessionID")
        .or_else(|| props.pointer("/part/sessionID"))
        .or_else(|| props.pointer("/info/sessionID"))
        .and_then(Value::as_str)
}

pub(super) fn handle_event(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, event: Value) {
    let Some(kind) = event.get("type").and_then(Value::as_str) else { return };
    let props = event.get("properties").cloned().unwrap_or(Value::Null);

    // Session records are the one event kind addressed by the session they describe rather than by a
    // `sessionID` field, and the root's own record is worth reading for its title and staged revert.
    if matches!(kind, "session.created" | "session.updated") {
        let info = props.get("info").cloned().unwrap_or(Value::Null);
        let id = info.get("id").and_then(Value::as_str);
        if let Route::Root = route_for(proc, id) {
            root_session_updated(app, session_id, &info);
        }
        return;
    }
    let owner = event_session_id(&event).map(str::to_string);
    let path: RowPath = match route_for(proc, owner.as_deref()) {
        Route::Root => Vec::new(),
        Route::Child(path) => path,
        Route::Unknown(child) => {
            // Only conversation content is worth holding for a card that does not exist yet.
            if kind.starts_with("message.") || kind.starts_with("permission.") || kind.starts_with("question.") {
                buffer_child_event(proc, &child, event);
            }
            return;
        }
    };
    handle_routed(app, session_id, proc, &path, kind, &props);
}

fn buffer_child_event(proc: &ChatProcess, child: &str, event: Value) {
    let mut pending = proc.codex_pending_child_events.lock().unwrap();
    if !pending.contains_key(child) && pending.len() >= MAX_PENDING_CHILDREN {
        return;
    }
    let queue = pending.entry(child.to_string()).or_default();
    if queue.len() >= MAX_PENDING_CHILD_EVENTS {
        queue.remove(0);
    }
    queue.push((String::new(), event));
}

/// A `task` call has named its child session: route the child's events to this card from now on, and
/// deliver what arrived before the link was known.
fn link_child(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, child: &str, path: RowPath) {
    {
        let mut routes = proc.codex_subagent_ids.lock().unwrap();
        if routes.get(child) == Some(&path) {
            return;
        }
        routes.insert(child.to_string(), path.clone());
    }
    let pending = proc.codex_pending_child_events.lock().unwrap().remove(child);
    for (_, event) in pending.into_iter().flatten() {
        let Some(kind) = event.get("type").and_then(Value::as_str) else { continue };
        let props = event.get("properties").cloned().unwrap_or(Value::Null);
        handle_routed(app, session_id, proc, &path, kind, &props);
    }
}

fn handle_routed(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, path: &RowPath, kind: &str, props: &Value) {
    let root = path.is_empty();
    match kind {
        "message.updated" => message_updated(app, session_id, proc, path, props.get("info").unwrap_or(&Value::Null)),
        "message.part.updated" => part_updated(app, session_id, proc, path, props.get("part").unwrap_or(&Value::Null)),
        "message.part.delta" => part_delta(proc, path, props),
        "message.removed" | "message.part.removed" => {
            // Something was taken out of the conversation behind the engine's back; reread rather than
            // guess which rows it was.
            if root {
                if let (Ok(server), Ok(native)) = (server(proc), root_session(proc)) {
                    let _ = rebuild_timeline(proc, &server, &native);
                }
            }
        }
        "permission.asked" => permission_asked(app, session_id, proc, props),
        "permission.replied" => resolve_prompt(app, session_id, proc, props.get("requestID").or_else(|| props.get("id"))),
        "question.asked" => question_asked(app, session_id, proc, props),
        "question.replied" | "question.rejected" => {
            resolve_prompt(app, session_id, proc, props.get("requestID").or_else(|| props.get("id")))
        }
        "session.status" if root => match props.pointer("/status/type").and_then(Value::as_str) {
            Some("idle") => turn_idle(app, session_id, proc),
            Some("busy") => {
                proc.opencode.lock().unwrap().busy = true;
                if proc.turn.lock().unwrap().running {
                    emit_state(app, session_id, AgentState::Working);
                }
            }
            Some("retry") => {
                let attempt = props.pointer("/status/attempt").and_then(Value::as_u64).unwrap_or(0);
                let message = props
                    .pointer("/status/message")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|m| !m.is_empty())
                    .map(|m| format!("The provider is being retried (attempt {attempt}): {m}"))
                    .unwrap_or_else(|| format!("The provider is being retried (attempt {attempt})"));
                proc.timeline.lock().unwrap().upsert(ChatRow::Notice { id: "oc-retry".to_string(), message });
            }
            _ => {}
        },
        "session.idle" if root => turn_idle(app, session_id, proc),
        "session.error" if root => {
            if let Some(message) = props.get("error").and_then(timeline::error_message) {
                proc.timeline.lock().unwrap().upsert(ChatRow::Error {
                    id: format!("e-{}", now_ms()),
                    message,
                });
            }
        }
        "session.compacted" if root => handle_compaction(proc, true, None, None),
        _ => {}
    }
}

/// The turn is over. OpenCode says so with one idle event whatever happened: an answer, a failure that
/// was already reported as an error row, or the abort the user asked for.
fn turn_idle(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    if !std::mem::take(&mut proc.opencode.lock().unwrap().busy) {
        return;
    }
    // A part still marked streaming at this point never got its end mark; it is finished now.
    {
        let mut timeline = proc.timeline.lock().unwrap();
        let open: Vec<String> = timeline
            .rows
            .iter()
            .filter_map(|row| match row {
                ChatRow::Assistant { id, streaming: true, .. } | ChatRow::Reasoning { id, streaming: true, .. } => {
                    Some(id.clone())
                }
                _ => None,
            })
            .collect();
        for id in open {
            let Some(row) = timeline.get(&id).cloned() else { continue };
            let row = match row {
                ChatRow::Assistant { id, text, model, at, duration_ms, .. } => {
                    ChatRow::Assistant { id, text, streaming: false, model, at, duration_ms }
                }
                ChatRow::Reasoning { id, text, .. } => ChatRow::Reasoning { id, text, streaming: false },
                other => other,
            };
            timeline.upsert(row);
        }
    }
    let running = proc.turn.lock().unwrap().running;
    if running {
        handle_turn_end(app, session_id, proc, "success", None);
    } else {
        emit_state(app, session_id, AgentState::Waiting);
    }
}

fn root_session_updated(app: &AppCtx, session_id: &str, info: &Value) {
    // OpenCode names conversations itself after the first exchange. A session still carrying its
    // numbered placeholder takes that name, the way a terminal session takes its first prompt. Until the
    // real title exists OpenCode reports a placeholder of its own, which is no better than ours.
    if let Some(title) = info
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty() && !is_placeholder_title(t))
    {
        let renamed = {
            let conn = app.db().conn.lock().unwrap();
            match crate::db::repo::get_session(&conn, session_id) {
                Ok(Some(session)) if crate::agent::server::is_auto_name(&session.name) && session.name != title => {
                    crate::db::repo::rename_node(&conn, crate::models::NodeKind::Session, session_id, title).is_ok()
                }
                _ => false,
            }
        };
        if renamed {
            app.emit(crate::host::TREE_CHANGED, ());
        }
    }
}

/// OpenCode's own placeholder before a title has been generated, such as `New session - 2026-09-05T00:24:19.743Z`.
fn is_placeholder_title(title: &str) -> bool {
    title.starts_with("New session")
}

fn message_updated(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, path: &RowPath, info: &Value) {
    let Some(id) = info.get("id").and_then(Value::as_str) else { return };
    let role = info.get("role").and_then(Value::as_str).unwrap_or("assistant");
    {
        let mut state = proc.opencode.lock().unwrap();
        state.roles.insert(id.to_string(), role.to_string());
        if role == "assistant" && timeline::is_summary(info) {
            state.summaries.insert(id.to_string());
        }
    }
    if role != "assistant" {
        return;
    }
    if let Some(model) = timeline::message_model(info) {
        let parts = {
            let mut state = proc.opencode.lock().unwrap();
            state.models.insert(id.to_string(), model.clone());
            state.message_parts.get(id).cloned().unwrap_or_default()
        };
        let mut rows = proc.timeline.lock().unwrap();
        for part_id in parts {
            if let Some(mut row @ ChatRow::Assistant { .. }) = rows.find_at(path, &part_id).cloned() {
                if let ChatRow::Assistant { model: name, .. } = &mut row {
                    *name = Some(model.clone());
                }
                rows.upsert_at(path, row);
            }
        }
    }
    if !path.is_empty() {
        // A child's answers report the tokens and model its card shows.
        let tokens = info.get("tokens").and_then(|t| t.get("total").and_then(Value::as_u64).filter(|n| *n > 0));
        let model = info.get("modelID").and_then(Value::as_str).map(str::to_string);
        proc.timeline.lock().unwrap().update_subagent_at(
            path,
            |facts| {
                if tokens.is_some() {
                    facts.total_tokens = tokens;
                }
                if facts.model.is_none() {
                    facts.model = model;
                }
            },
            None,
            None,
        );
    }
    if let Some(message) = info.get("error").and_then(timeline::error_message) {
        proc.timeline.lock().unwrap().upsert_at(
            path,
            ChatRow::Error { id: timeline::error_row_id(id), message },
        );
    }
    let _ = (app, session_id);
}

fn part_updated(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, path: &RowPath, part: &Value) {
    let Some(part_id) = part.get("id").and_then(Value::as_str) else { return };
    let message_id = part.get("messageID").and_then(Value::as_str).unwrap_or("");
    let (role, summary, own_row, model) = {
        let mut state = proc.opencode.lock().unwrap();
        if part.get("type").and_then(Value::as_str) == Some("text") {
            state.message_parts.entry(message_id.to_string()).or_default().insert(part_id.to_string());
        }
        (
            state.roles.get(message_id).cloned().unwrap_or_else(|| "assistant".to_string()),
            state.summaries.contains(message_id),
            state.user_messages.get(message_id).cloned(),
            state.models.get(message_id).cloned(),
        )
    };
    let finished = part.pointer("/time/end").is_some();
    match part.get("type").and_then(Value::as_str) {
        Some("text") if role == "user" => {
            // The engine drew its own turns when it sent them; only turns started elsewhere — a shell
            // line or a command answered by the server — need a row now.
            if own_row.is_some() {
                return;
            }
            let text = timeline::user_text(std::slice::from_ref(part));
            if text.trim().is_empty() {
                return;
            }
            proc.timeline.lock().unwrap().upsert_at(
                path,
                ChatRow::User {
                    id: timeline::user_row_id(message_id),
                    text,
                    images: Vec::new(),
                    at: Some(now_ms() as i64),
                },
            );
        }
        Some("text") => {
            if summary {
                return;
            }
            let mut text = part.get("text").and_then(Value::as_str).unwrap_or("").to_string();
            let mut buffers = proc.buffers.lock().unwrap();
            if text.is_empty() {
                text = buffers.get(part_id).cloned().unwrap_or_default();
            } else {
                buffers.insert(part_id.to_string(), text.clone());
            }
            drop(buffers);
            if text.trim().is_empty() && finished {
                return;
            }
            let mut rows = proc.timeline.lock().unwrap();
            let model = model.or_else(|| match rows.find_at(path, part_id) {
                Some(ChatRow::Assistant { model, .. }) => model.clone(),
                _ => None,
            });
            rows.upsert_at(
                path,
                ChatRow::Assistant {
                    id: part_id.to_string(),
                    text,
                    streaming: !finished,
                    model,
                    at: Some(part.pointer("/time/end").and_then(Value::as_i64).unwrap_or(now_ms() as i64)),
                    duration_ms: None,
                },
            );
        }
        Some("reasoning") => {
            let mut text = part.get("text").and_then(Value::as_str).unwrap_or("").to_string();
            let mut buffers = proc.buffers.lock().unwrap();
            if text.is_empty() {
                text = buffers.get(part_id).cloned().unwrap_or_default();
            } else {
                buffers.insert(part_id.to_string(), text.clone());
            }
            drop(buffers);
            if text.trim().is_empty() && finished {
                return;
            }
            proc.timeline.lock().unwrap().upsert_at(
                path,
                ChatRow::Reasoning { id: part_id.to_string(), text, streaming: !finished },
            );
        }
        Some("tool") => {
            let Some(mut row) = timeline::tool_row(part) else { return };
            // The card may already hold a child timeline and usage; a state update must not drop them.
            if let Some(ChatRow::Tool { children, subagent, .. }) = proc.timeline.lock().unwrap().find_at(path, part_id) {
                if let ChatRow::Tool { children: slot, subagent: facts, .. } = &mut row {
                    *slot = children.clone();
                    if let (Some(previous), Some(next)) = (subagent, facts) {
                        if next.total_tokens.is_none() {
                            next.total_tokens = previous.total_tokens;
                        }
                        if next.model.is_none() {
                            next.model = previous.model.clone();
                        }
                    }
                }
            }
            if let Some(call_id) = part.get("callID").and_then(Value::as_str) {
                proc.opencode
                    .lock()
                    .unwrap()
                    .calls
                    .insert(call_id.to_string(), (path.clone(), part_id.to_string()));
            }
            proc.timeline.lock().unwrap().upsert_at(path, row);
            if let Some(child) = timeline::child_session_of(part) {
                let mut child_path = path.clone();
                child_path.push(part_id.to_string());
                link_child(app, session_id, proc, &child, child_path);
            }
        }
        Some("compaction") if path.is_empty() => {
            let trigger = if part.get("auto").and_then(Value::as_bool).unwrap_or(false) { "auto" } else { "manual" };
            handle_compaction(proc, false, Some(trigger.to_string()), None);
        }
        _ => {}
    }
}

fn part_delta(proc: &Arc<ChatProcess>, path: &RowPath, props: &Value) {
    if props.get("field").and_then(Value::as_str) != Some("text") {
        return;
    }
    let (Some(part_id), Some(delta)) = (
        props.get("partID").and_then(Value::as_str),
        props.get("delta").and_then(Value::as_str),
    ) else {
        return;
    };
    let text = {
        let mut buffers = proc.buffers.lock().unwrap();
        let entry = buffers.entry(part_id.to_string()).or_default();
        entry.push_str(delta);
        entry.clone()
    };
    let mut timeline = proc.timeline.lock().unwrap();
    let Some(existing) = timeline.find_at(path, part_id).cloned() else { return };
    let row = match existing {
        ChatRow::Assistant { id, model, at, duration_ms, .. } => {
            ChatRow::Assistant { id, text, streaming: true, model, at, duration_ms }
        }
        ChatRow::Reasoning { id, .. } => ChatRow::Reasoning { id, text, streaming: true },
        _ => return,
    };
    timeline.upsert_at(path, row);
}

// ─────────────────────────── Prompts ───────────────────────────

fn permission_asked(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, props: &Value) {
    let Some(id) = props.get("id").and_then(Value::as_str) else { return };
    let tool_input = props
        .pointer("/tool/callID")
        .and_then(Value::as_str)
        .and_then(|call| proc.opencode.lock().unwrap().calls.get(call).cloned())
        .and_then(|(path, row_id)| match proc.timeline.lock().unwrap().find_at(&path, &row_id) {
            Some(ChatRow::Tool { input, .. }) => Some(input.clone()),
            _ => None,
        });
    let payload = wire::permission_payload(props, tool_input);
    // Bypass answers for the person, the way OpenCode's own `--auto` does: everything not denied by
    // configuration runs. Questions are not permissions and are still asked.
    if proc.mode.lock().unwrap().as_str() == "bypassPermissions" {
        if let Ok(server) = server(proc) {
            if server.post(&format!("/permission/{id}/reply"), Some(&json!({"reply":"once"}))).is_ok() {
                return;
            }
        }
    }
    proc.permissions.lock().unwrap().insert(id.to_string(), payload.clone());
    emit(app, session_id, json!({"type":"permission","request":payload}));
    emit_state(app, session_id, AgentState::Asking);
}

fn question_asked(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, props: &Value) {
    let Some(payload) = wire::question_payload(props) else { return };
    let id = payload["id"].as_str().unwrap_or("").to_string();
    proc.permissions.lock().unwrap().insert(id, payload.clone());
    emit(app, session_id, json!({"type":"permission","request":payload}));
    emit_state(app, session_id, AgentState::Asking);
}

/// A prompt was answered — here or, in principle, by another client of the same server.
fn resolve_prompt(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>, id: Option<&Value>) {
    let Some(id) = id.and_then(Value::as_str) else { return };
    if proc.permissions.lock().unwrap().remove(id).is_some() {
        emit(app, session_id, json!({"type":"permissionResolved","id":id}));
        if proc.permissions.lock().unwrap().is_empty() && proc.turn.lock().unwrap().running {
            emit_state(app, session_id, AgentState::Working);
        }
    }
}

/// Answer a permission or question card. `remember` is the standing "always allow" rule the card offered.
pub(super) fn respond(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    request_id: &str,
    request: &Value,
    allow: bool,
    updated_input: Option<&Value>,
    remember: bool,
) -> Result<(), String> {
    let server = server(proc)?;
    if request.get("_opencodeKind").and_then(Value::as_str) == Some("question") {
        if allow {
            let answers = wire::question_answers(request, updated_input);
            server.post(&format!("/question/{request_id}/reply"), Some(&json!({"answers":answers})))?;
        } else {
            server.post(&format!("/question/{request_id}/reject"), None)?;
        }
    } else {
        let reply = if !allow {
            "reject"
        } else if remember {
            "always"
        } else {
            "once"
        };
        server.post(&format!("/permission/{request_id}/reply"), Some(&json!({"reply":reply})))?;
    }
    proc.permissions.lock().unwrap().remove(request_id);
    emit(app, session_id, json!({"type":"permissionResolved","id":request_id}));
    if proc.permissions.lock().unwrap().is_empty() && proc.turn.lock().unwrap().running {
        emit_state(app, session_id, AgentState::Working);
    }
    Ok(())
}

/// Bypass was switched on while prompts were waiting: answer the permissions, keep the questions.
pub(super) fn approve_pending(app: &AppCtx, session_id: &str, proc: &Arc<ChatProcess>) {
    let Ok(server) = server(proc) else { return };
    let pending: Vec<String> = proc
        .permissions
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, request)| request.get("_opencodeKind").and_then(Value::as_str) == Some("permission"))
        .map(|(id, _)| id.clone())
        .collect();
    for id in pending {
        if server.post(&format!("/permission/{id}/reply"), Some(&json!({"reply":"once"}))).is_ok() {
            proc.permissions.lock().unwrap().remove(&id);
            emit(app, session_id, json!({"type":"permissionResolved","id":id}));
        }
    }
}

// ─────────────────────────── Turns ───────────────────────────

/// Send one user turn. Plain text goes out as a prompt; `!` runs a shell line; a leading `/` naming one
/// of the server's commands runs that command; `/compact` summarizes. All of them end with the same idle
/// event, so the turn bookkeeping is shared.
pub(super) fn dispatch(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    row_id: &str,
    text: &str,
    images: &[ChatImage],
) -> Result<(), String> {
    let server = server(proc)?;
    let native = root_session(proc)?;
    let message_id = wire::new_message_id();
    {
        let mut state = proc.opencode.lock().unwrap();
        state.user_messages.insert(message_id.clone(), row_id.to_string());
    }
    proc.user_targets.lock().unwrap().insert(
        row_id.to_string(),
        RewindTarget { message_id: message_id.clone(), turn_id: None, text: text.to_string() },
    );
    let agent = proc.collaboration_mode.lock().unwrap().clone();
    let model = proc.model.lock().unwrap().clone();
    let variant = proc.effort.lock().unwrap().clone();
    let trimmed = text.trim();

    // A command that acts on the conversation rather than starting a turn was typed while the agent was
    // busy and waited in the queue. Running it now would be wrong, and sending it as prose would be worse.
    if is_local_command(trimmed) {
        return Err(format!("Send {trimmed} when the agent is idle"));
    }
    if let Some(command) = trimmed.strip_prefix('!').map(str::trim).filter(|c| !c.is_empty()) {
        let body = wire::shell_body(&message_id, command, agent.as_deref().unwrap_or("build"), model.as_deref());
        run_turn_request(app, session_id, proc, server, format!("/session/{native}/shell"), body);
        return Ok(());
    }
    if trimmed == "/compact" {
        let model = model
            .or_else(|| proc.opencode.lock().unwrap().default_model.clone())
            .ok_or("Choose a model before compacting the conversation")?;
        let (provider, model) = wire::split_model(&model).ok_or("The chosen model has no provider")?;
        let body = json!({"providerID":provider,"modelID":model});
        run_turn_request(app, session_id, proc, server, format!("/session/{native}/summarize"), body);
        return Ok(());
    }
    if let Some(rest) = trimmed.strip_prefix('/') {
        let (name, arguments) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let known = proc
            .commands
            .lock()
            .unwrap()
            .iter()
            .any(|command| command.get("name").and_then(Value::as_str) == Some(name));
        if known {
            let body = wire::command_body(
                &message_id,
                name,
                arguments.trim(),
                agent.as_deref(),
                model.as_deref(),
                variant.as_deref(),
            );
            run_turn_request(app, session_id, proc, server, format!("/session/{native}/command"), body);
            return Ok(());
        }
    }
    let body = wire::prompt_body(&message_id, text, images, agent.as_deref(), model.as_deref(), variant.as_deref());
    server.post(&format!("/session/{native}/prompt_async"), Some(&body))?;
    Ok(())
}

/// OpenCode persists asynchronous prompts while its loop is running and reads them at its next step.
/// Send steering as prose, so shell and slash syntax cannot start a competing command.
pub(super) fn steer(proc: &Arc<ChatProcess>, row_id: &str, text: &str, images: &[ChatImage]) -> Result<(), String> {
    let server = server(proc)?;
    let native = root_session(proc)?;
    let message_id = wire::new_message_id();
    let agent = proc.collaboration_mode.lock().unwrap().clone();
    let model = proc.model.lock().unwrap().clone();
    let variant = proc.effort.lock().unwrap().clone();
    let body = wire::prompt_body(&message_id, text, images, agent.as_deref(), model.as_deref(), variant.as_deref());
    proc.opencode.lock().unwrap().user_messages.insert(message_id.clone(), row_id.to_string());
    proc.user_targets.lock().unwrap().insert(row_id.to_string(), RewindTarget {
        message_id: message_id.clone(), turn_id: None, text: text.to_string(),
    });
    if let Err(error) = server.post(&format!("/session/{native}/prompt_async"), Some(&body)) {
        proc.opencode.lock().unwrap().user_messages.remove(&message_id);
        proc.user_targets.lock().unwrap().remove(row_id);
        return Err(error);
    }
    Ok(())
}

/// A request that answers only when its turn has run. It goes out on its own thread so the caller can
/// return at once; the turn ends through the event stream like any other, and a request the server
/// refuses ends it here, since no idle event will follow one that never started.
fn run_turn_request(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    server: Arc<Server>,
    path: String,
    body: Value,
) {
    let app = app.clone();
    let session_id = session_id.to_string();
    let proc = proc.clone();
    std::thread::spawn(move || {
        if let Err(message) = server.post_turn(&path, Some(&body)) {
            let running = proc.turn.lock().unwrap().running;
            if running {
                proc.timeline.lock().unwrap().upsert(ChatRow::Error {
                    id: format!("e-{}", now_ms()),
                    message,
                });
                handle_turn_end(&app, &session_id, &proc, "request_failed", None);
            } else {
                emit(&app, &session_id, json!({"type":"error","message":message}));
            }
        }
    });
}

/// Ask the server to stop the running turn. The stop arrives as an abort error and an idle event.
pub(super) fn abort(proc: &Arc<ChatProcess>) -> Result<(), String> {
    let server = server(proc)?;
    let native = root_session(proc)?;
    server.post(&format!("/session/{native}/abort"), None)?;
    Ok(())
}

fn is_local_command(text: &str) -> bool {
    matches!(text, "/undo" | "/redo" | "/share" | "/unshare")
}

/// Commands OpenCode's own interface answers without a turn. `None` means the text is not one of them.
pub(super) fn local_command(
    app: &AppCtx,
    session_id: &str,
    proc: &Arc<ChatProcess>,
    text: &str,
) -> Result<Option<&'static str>, String> {
    let command = text.trim();
    if !is_local_command(command) {
        return Ok(None);
    }
    let server = server(proc)?;
    let native = root_session(proc)?;
    match command {
        "/undo" => {
            super::ensure_rewind_idle(proc)?;
            ensure_snapshot_scope(proc, &server, &native)?;
            let last = proc
                .timeline
                .lock()
                .unwrap()
                .user_messages()
                .last()
                .cloned()
                .ok_or("There is no message to undo")?;
            let target = super::rewind_target(proc, &last.0)?;
            server.post(
                &format!("/session/{native}/revert"),
                Some(&json!({"messageID":target.message_id})),
            )?;
            proc.timeline.lock().unwrap().trim_from_user(&last.0);
            proc.timeline.lock().unwrap().upsert(ChatRow::Notice {
                id: format!("n-{}", now_ms()),
                message: "The last message and the file changes it caused were reverted. Use /redo to restore them.".to_string(),
            });
            emit_state(app, session_id, AgentState::Waiting);
        }
        "/redo" => {
            super::ensure_rewind_idle(proc)?;
            ensure_snapshot_scope(proc, &server, &native)?;
            server.post(&format!("/session/{native}/unrevert"), None)?;
            rebuild_timeline(proc, &server, &native)?;
            emit_state(app, session_id, AgentState::Waiting);
        }
        "/share" => {
            let session = server.post(&format!("/session/{native}/share"), None)?;
            let url = session
                .pointer("/share/url")
                .and_then(Value::as_str)
                .ok_or("OpenCode did not return a share link")?;
            proc.timeline.lock().unwrap().upsert(ChatRow::Notice {
                id: format!("n-{}", now_ms()),
                message: format!("This conversation is shared at {url}"),
            });
        }
        "/unshare" => {
            server.delete(&format!("/session/{native}/share"))?;
            proc.timeline.lock().unwrap().upsert(ChatRow::Notice {
                id: format!("n-{}", now_ms()),
                message: "This conversation is no longer shared.".to_string(),
            });
        }
        _ => unreachable!(),
    }
    Ok(Some("sent"))
}

/// Rewind to before one user message. OpenCode's revert restores both the conversation and the files it
/// changed; the caller then trims the timeline it already holds.
pub(super) fn rewind(proc: &Arc<ChatProcess>, target: &RewindTarget) -> Result<(), String> {
    let server = server(proc)?;
    let native = root_session(proc)?;
    ensure_snapshot_scope(proc, &server, &native)?;
    server.post(
        &format!("/session/{native}/revert"),
        Some(&json!({"messageID":target.message_id})),
    )?;
    Ok(())
}

/// OpenCode 1.18.24 updates snapshots relative to the session directory but restores the whole tree.
/// A subdirectory session can therefore restore stale files outside that directory. Check the live
/// repository as well as the server's cached paths before allowing any snapshot mutation.
fn ensure_snapshot_scope(proc: &ChatProcess, server: &Server, native: &str) -> Result<(), String> {
    let paths = server.get("/path")?;
    let session = server.get(&format!("/session/{native}"))?;
    let canonical = |value: Option<&str>| -> Result<std::path::PathBuf, String> {
        let value = value.filter(|value| !value.trim().is_empty())
            .ok_or("OpenCode did not provide a directory for file restoration")?;
        std::fs::canonicalize(value)
            .map_err(|e| format!("Cannot verify the OpenCode file restoration directory: {e}"))
    };
    let cwd = canonical(proc.cwd.as_deref())?;
    let directory = canonical(paths.get("directory").and_then(Value::as_str))?;
    let worktree = canonical(paths.get("worktree").and_then(Value::as_str))?;
    let session_directory = canonical(session.get("directory").and_then(Value::as_str))?;
    let root = std::process::Command::new("git")
        .arg("-C").arg(&cwd).args(["rev-parse", "--show-toplevel"])
        .output().map_err(|e| format!("Cannot verify the Git root before file restoration: {e}"))?;
    if !root.status.success() {
        return Err("OpenCode file restoration requires an accessible Git working tree".to_string());
    }
    let root_text = String::from_utf8_lossy(&root.stdout);
    let git_root = canonical(Some(root_text.trim()))?;
    validate_snapshot_scope(&cwd, &directory, &worktree, &session_directory, &git_root)
}

fn validate_snapshot_scope(
    cwd: &std::path::Path,
    directory: &std::path::Path,
    worktree: &std::path::Path,
    session_directory: &std::path::Path,
    git_root: &std::path::Path,
) -> Result<(), String> {
    if cwd != directory || cwd != worktree || cwd != session_directory || cwd != git_root {
        return Err("OpenCode file restoration is unavailable for subdirectory sessions or changed repository roots. Its snapshot may include stale files outside the session directory. Start a new session at the Git working-tree root.".to_string());
    }
    Ok(())
}

// ─────────────────────────── Timeline rebuild ───────────────────────────

fn read_messages(server: &Server, session: &str) -> Result<Vec<OpencodeMessage>, String> {
    let value = server.get(&format!("/session/{session}/message"))?;
    Ok(value
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| OpencodeMessage {
            info: entry.get("info").cloned().unwrap_or(Value::Null),
            parts: entry.get("parts").and_then(Value::as_array).cloned().unwrap_or_default(),
        })
        .collect())
}

/// Replace the timeline with what the server holds, child sessions included, and relearn which Task card
/// each child session belongs to.
pub(super) fn rebuild_timeline(proc: &Arc<ChatProcess>, server: &Arc<Server>, native: &str) -> Result<(), String> {
    let mut messages = read_messages(server, native)?;
    if let Ok(session) = server.get(&format!("/session/{native}")) {
        if let Some(cut) = session.get("revert").and_then(crate::agent::opencode_store::reverted_from) {
            if let Some(index) = messages.iter().position(|m| m.info.get("id").and_then(Value::as_str) == Some(cut)) {
                messages.truncate(index);
            }
        }
    }
    let cache: std::sync::Mutex<HashMap<String, Option<Vec<OpencodeMessage>>>> = Default::default();
    let children = |id: &str| -> Option<Vec<OpencodeMessage>> {
        if let Some(found) = cache.lock().unwrap().get(id) {
            return found.clone();
        }
        let read = read_messages(server, id).ok();
        cache.lock().unwrap().insert(id.to_string(), read.clone());
        read
    };
    let rows = timeline::rows(&messages, &children);
    let mut routes: HashMap<String, RowPath> = HashMap::new();
    let mut restored = OpencodeState::default();
    collect_routes(&messages, &children, &Vec::new(), 0, &mut routes, &mut restored);
    {
        let mut timeline = proc.timeline.lock().unwrap();
        timeline.replace_all(rows);
    }
    *proc.codex_subagent_ids.lock().unwrap() = routes;
    {
        let mut state = proc.opencode.lock().unwrap();
        state.roles.extend(restored.roles);
        state.calls.extend(restored.calls);
        state.models.extend(restored.models);
        state.message_parts.extend(restored.message_parts);
        state.summaries.extend(
            messages
                .iter()
                .filter(|m| timeline::is_summary(&m.info))
                .filter_map(|m| m.info.get("id").and_then(Value::as_str).map(str::to_string)),
        );
    }
    Ok(())
}

fn collect_routes(
    messages: &[OpencodeMessage],
    children: &dyn Fn(&str) -> Option<Vec<OpencodeMessage>>,
    path: &RowPath,
    depth: usize,
    routes: &mut HashMap<String, RowPath>,
    state: &mut OpencodeState,
) {
    for message in messages {
        if let (Some(id), Some(role)) = (
            message.info.get("id").and_then(Value::as_str),
            message.info.get("role").and_then(Value::as_str),
        ) {
            state.roles.insert(id.to_string(), role.to_string());
            if role == "assistant" {
                if let Some(model) = timeline::message_model(&message.info) {
                    state.models.insert(id.to_string(), model);
                }
                state.message_parts.insert(id.to_string(), message.parts.iter()
                    .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(|part| part.get("id").and_then(Value::as_str).map(str::to_string))
                    .collect());
            }
        }
        for part in &message.parts {
            let Some(part_id) = part.get("id").and_then(Value::as_str) else { continue };
            if let Some(call) = part.get("callID").and_then(Value::as_str) {
                state.calls.insert(call.to_string(), (path.clone(), part_id.to_string()));
            }
            let Some(child) = timeline::child_session_of(part) else { continue };
            let mut child_path = path.clone();
            child_path.push(part_id.to_string());
            routes.insert(child.clone(), child_path.clone());
            if depth < 4 {
                if let Some(child_messages) = children(&child) {
                    collect_routes(&child_messages, children, &child_path, depth + 1, routes, state);
                }
            }
        }
    }
}

// ─────────────────────────── Shared helpers used by the engine ───────────────────────────

/// The permission mode this engine runs OpenCode in, from what the session stored.
pub(super) fn initial_mode(stored: Option<&str>) -> &'static str {
    match stored.map(str::trim) {
        Some("skip") | Some("bypassPermissions") => "bypassPermissions",
        _ => "default",
    }
}

#[cfg(test)]
mod tests {
    use super::validate_snapshot_scope;
    use std::path::Path;

    #[test]
    fn chat_title_uses_opencode_native_updates_and_preserves_custom_names() {
        let app = super::super::tests::ctx("title-opencode");
        {
            let conn = app.db().conn.lock().unwrap();
            conn.execute("INSERT INTO projects(id,name,root_path,created_at) VALUES ('p','test','/tmp',0)", []).unwrap();
            conn.execute("INSERT INTO sessions(id,project_id,name,kind,permission_mode,created_at) VALUES ('s','p','OpenCode 1','opencode','default',0)", []).unwrap();
        }
        let name = || {
            let conn = app.db().conn.lock().unwrap();
            crate::db::repo::get_session_name(&conn, "s").unwrap().unwrap()
        };
        super::root_session_updated(&app, "s", &serde_json::json!({"title":"New session - 2026-09-07"}));
        assert_eq!(name(), "OpenCode 1");
        super::root_session_updated(&app, "s", &serde_json::json!({"title":"原生生成的标题"}));
        assert_eq!(name(), "原生生成的标题");
        {
            let conn = app.db().conn.lock().unwrap();
            crate::db::repo::rename_node(&conn, crate::models::NodeKind::Session, "s", "自定义标题").unwrap();
        }
        super::root_session_updated(&app, "s", &serde_json::json!({"title":"后续原生标题"}));
        assert_eq!(name(), "自定义标题");
    }

    #[test]
    fn opencode_snapshot_accepts_one_consistent_root() {
        let root = Path::new("/repo");
        assert!(validate_snapshot_scope(root, root, root, root, root).is_ok());
    }

    #[test]
    fn opencode_snapshot_rejects_a_subdirectory_session() {
        let root = Path::new("/repo");
        let child = Path::new("/repo/child");
        assert!(validate_snapshot_scope(child, child, root, child, root).is_err());
    }

    #[test]
    fn opencode_snapshot_rejects_changed_or_resumed_roots() {
        let root = Path::new("/repo");
        let other = Path::new("/other");
        assert!(validate_snapshot_scope(root, root, root, root, other).is_err());
        assert!(validate_snapshot_scope(root, root, root, other, root).is_err());
        assert!(validate_snapshot_scope(root, other, root, root, root).is_err());
    }
}
