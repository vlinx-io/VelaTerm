//! WebSocket bridge that multiplexes invokes, events, and PTY streams between browser and backend.
//!
//! Protocol (kept in sync with frontend `src/ipc/wsClient.ts`):
//! - Text frames are JSON control messages; binary PTY output is `[1-byte sid length][sid UTF-8][raw bytes]`.
//! - Client: {t:"invoke",id,cmd,args} / {t:"pty-spawn",id,sid,args} / {t:"pty-detach",sid,subId}
//!   / {t:"pong"} (heartbeat response)
//! - Server: {t:"hello",source:"ws-N"} is the **first frame** after connection and identifies the source so
//!   the frontend can compare itself with Resized/SpawnResult.owner; followed by {t:"reply",id,ok,result|error},
//!   {t:"event",name,payload}, {t:"ping"} every 30 seconds, and binary PTY frames. A connection with no inbound
//!   traffic for 2.5 heartbeat intervals is treated as half-open and closed.
//!
//! Event forwarding uses `AppCtx::listen(event_name, ..)`, backed by Tauri events on desktop and the in-process
//! bus in headless mode. Global events are registered at connection time; per-session
//! `pty://status|exit|killed/{sid}` events are registered on that session's first pty-spawn. Closing a connection
//! unregisters listeners and detaches all of its PTY subscriptions, but never kills shared sessions.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{RawQuery, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;

use super::dispatch::dispatch;
use super::e2ee::{self, Cipher};
use super::{Ctx, ServeMode};
use crate::db::repo;
use crate::host::{AppCtx, ListenerId};
use crate::models::SessionKind;
use crate::pty::manager::SpawnResult;
use crate::pty::session::OutputSink;

/// `/ws` endpoint that upgrades to WebSocket. Authentication has two paths (see [`handle_socket`]): E2EE clients
/// authenticate a device token and password during the handshake; plaintext clients use a login session token
/// in `?token=`. This layer does not return 401 directly; it passes validation to [`handle_socket`] while the
/// frontend continues to detect expired credentials through `/api/me`.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(ctx): State<Ctx>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Authentication accepts only `?token=` because browser WebSocket APIs cannot set custom headers and the
    // cookie mechanism has been removed (see auth.rs). Preserve missing/invalid/valid state in a fingerprint
    // used by rejection logs and error frames, distinguishing absent credentials from expired sessions.
    let _ = &headers; // Retained only for the axum extractor signature; E2EE authentication bypasses it.
    let query_token = token_from_query(query.as_deref());
    let query_authed = query_token
        .as_deref()
        .map(|t| ctx.auth.token_valid(t))
        .unwrap_or(false);
    let auth_fp = format!(
        "token={}",
        match (query_token.is_some(), query_authed) {
            (false, _) => "absent",
            (true, false) => "invalid",
            (true, true) => "valid",
        },
    );
    ws.on_upgrade(move |socket| handle_socket(socket, ctx, query_authed, auth_fp))
}

/// Extracts the `token` value from a raw query string such as `a=b&token=xxx&c=d`.
fn token_from_query(query: Option<&str>) -> Option<String> {
    let q = query?;
    for pair in q.split('&') {
        if let Some(v) = pair.strip_prefix("token=") {
            return Some(v.to_string());
        }
    }
    None
}

/// Global WS connection sequence used to create source IDs for PTY input/resize ownership arbitration.
static NEXT_CONN_ID: AtomicU64 = AtomicU64::new(1);

/// Heartbeat interval for application-level `{"t":"ping"}` text frames. Browser JavaScript cannot observe
/// protocol-level Ping/Pong frames, so text frames let clients perform their own idle detection and reply with
/// `{"t":"pong"}` (see wsClient.ts).
const HEARTBEAT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

/// Maximum inbound silence before declaring a connection dead. Healthy clients return a pong every 30 seconds;
/// this 2.5-interval timeout reclaims half-open connections caused by NAT or sleep.
const LIVENESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(75);

async fn handle_socket(socket: WebSocket, ctx: Ctx, token_authed: bool, auth_fp: String) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // ---- E2EE negotiation prelude: inspect the first frame ----
    // An e2ee_hello first frame starts E2EE device-token and password authentication. Otherwise treat the
    // client as plaintext, require a validated `?token=`, and retain the frame as its first application message.
    let mut cipher: Option<Cipher> = None;
    // Self-reported device ID available only from E2EE; heartbeats use it to detect mid-connection revocation.
    let mut device_id: Option<String> = None;
    let mut pending_first: Option<String> = None;
    match ws_rx.next().await {
        Some(Ok(Message::Text(first))) => {
            if let Some(client_pub) = e2ee::parse_hello(&first) {
                match handshake(&ctx, &mut ws_tx, &mut ws_rx, &client_pub).await {
                    Some((c, did)) => {
                        cipher = Some(c);
                        device_id = did;
                    }
                    None => return, // End the connection after handshake/authentication failure or revocation.
                }
            } else {
                // LanTls exposes remote browser access and therefore requires paired encryption. Reject any
                // first frame other than e2ee_hello, with no plaintext-token fallback that could enable a
                // downgrade. Loopback and plaintext LAN modes retain the `?token=` path.
                //
                // Make rejection explicit: send a {"t":"error",...} reason, log it, then close. A silent return
                // previously left clients with an unexplained disconnect and made initial tree-load failures
                // difficult to diagnose (see the closed SSH reconnect issue under docs/issues).
                if ctx.mode == ServeMode::LanTls {
                    eprintln!("[ws] rejected: pairing (E2EE) required in LanTls mode, got plaintext first frame");
                    let _ = ws_tx
                        .send(Message::Text(
                            json!({"t":"error","code":"pairing_required","message":"WebSocket rejected: this server requires pairing (E2EE); plaintext connections are not accepted"}).to_string(),
                        ))
                        .await;
                    return;
                }
                // Plaintext mode requires a valid `?token=`; otherwise send an error, log, and close.
                if !token_authed {
                    eprintln!("[ws] rejected: unauthenticated connection ({auth_fp})");
                    let _ = ws_tx
                        .send(Message::Text(
                            json!({"t":"error","code":"unauthorized","message":format!("WebSocket rejected: not authenticated ({auth_fp})")}).to_string(),
                        ))
                        .await;
                    return;
                }
                pending_first = Some(first);
            }
        }
        _ => return, // End on close, a binary first frame, or an error.
    }

    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Message>();
    // Source ID for this connection. Each browser page has an independent WS, distinct from desktop.
    let conn_source = format!("ws-{}", NEXT_CONN_ID.fetch_add(1, Ordering::SeqCst));

    // The hello message is the **first application frame** and gives the frontend its source ID for comparing
    // against Resized/SpawnResult.owner in fit and mirror decisions. The writer encrypts it in E2EE mode and
    // consumes out_tx serially, guaranteeing delivery before replies, events, or PTY replay.
    let _ = out_tx.send(Message::Text(
        json!({"t": "hello", "source": conn_source}).to_string(),
    ));

    // Writer task: serialize every outbound reply, event, and PTY binary frame to the socket. In E2EE mode,
    // text becomes base64 ciphertext and binary becomes raw ciphertext; control frames such as Close/Ping/Pong
    // pass through unchanged. Encryption failure terminates the writer and closes the connection.
    let cipher_w = cipher.clone();
    let writer = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let msg = match &cipher_w {
                Some(c) => match msg {
                    Message::Text(t) => match c.encrypt_text(&t) {
                        Some(ct) => Message::Text(ct),
                        None => break,
                    },
                    Message::Binary(b) => match c.encrypt_bytes(&b) {
                        Some(cb) => Message::Binary(cb),
                        None => break,
                    },
                    other => other,
                },
                None => msg,
            };
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    let mut event_ids: Vec<ListenerId> = Vec::new();
    // PTY subscriptions owned by this connection: sid -> subscription ID, used for detach on disconnect.
    let mut pty_subs: HashMap<String, u64> = HashMap::new();
    // Sessions with status/exit forwarding already registered, preventing duplicates.
    let mut listened: HashSet<String> = HashSet::new();

    // Register session-independent global forwarding for spawn, document-open, tree, and clone events.
    for name in [
        "spawn://request",
        "view://request",
        crate::host::TREE_CHANGED,
        crate::git::CLONE_PROGRESS_EVENT,
    ] {
        event_ids.push(listen_forward(&ctx.app, name, out_tx.clone()));
    }

    // Process the plaintext application frame retained during first-frame inspection.
    if let Some(first) = pending_first.take() {
        handle_text(
            &ctx,
            &first,
            &conn_source,
            &out_tx,
            &mut pty_subs,
            &mut listened,
            &mut event_ids,
        );
    }

    // Read loop and heartbeat timer. Any inbound message, including pong, refreshes liveness. Send a ping each
    // interval and close a connection silent beyond LIVENESS_TIMEOUT as half-open. Normal cleanup then removes
    // forwarding listeners and detaches every subscription.
    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    let mut last_inbound = std::time::Instant::now();
    loop {
        tokio::select! {
            msg = ws_rx.next() => {
                let Some(Ok(msg)) = msg else { break };
                last_inbound = std::time::Instant::now();
                match msg {
                    Message::Text(text) => {
                        // In E2EE mode, decrypt inbound text to JSON; close on decryption failure.
                        let plain = match &cipher {
                            Some(c) => match c
                                .decrypt_text(&text)
                                .and_then(|b| String::from_utf8(b).ok())
                            {
                                Some(s) => s,
                                None => break,
                            },
                            None => text,
                        };
                        handle_text(
                            &ctx,
                            &plain,
                            &conn_source,
                            &out_tx,
                            &mut pty_subs,
                            &mut listened,
                            &mut event_ids,
                        );
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            _ = heartbeat.tick() => {
                if last_inbound.elapsed() >= LIVENESS_TIMEOUT {
                    break;
                }
                // Disconnect a device revoked during this connection within one heartbeat; other devices remain unaffected.
                if let Some(id) = device_id.as_deref() {
                    if ctx.auth.is_blocked(id) {
                        break;
                    }
                }
                if out_tx.send(Message::Text(json!({"t":"ping"}).to_string())).is_err() {
                    break;
                }
            }
        }
    }

    // Cleanup unregisters listeners and detaches this connection's PTY subscriptions without killing sessions.
    // Detach includes the source ID so a connection that owns terminal sizing releases it and broadcasts
    // Resized{owner:None}; remaining clients can then reclaim sizing after a page closes or network drops.
    for id in event_ids {
        ctx.app.unlisten(id);
    }
    let mgr = ctx.app.pty();
    for (sid, sub) in pty_subs {
        mgr.detach(&ctx.app, &sid, sub, &conn_source);
    }
    writer.abort();
}

/// E2EE handshake after receiving the client public key: derive the shared key, return plaintext `e2ee_ready`,
/// await encrypted `e2ee_auth`, validate it, then return encrypted `e2ee_authenticated`. Success returns a
/// [`Cipher`]; decryption/authentication failure or disconnect returns None and ends the connection. Both a
/// valid device token **and** the correct password are required.
async fn handshake(
    ctx: &Ctx,
    ws_tx: &mut SplitSink<WebSocket, Message>,
    ws_rx: &mut SplitStream<WebSocket>,
    client_pub_b64: &str,
) -> Option<(Cipher, Option<String>)> {
    let cipher = ctx.e2ee_keys.derive(client_pub_b64).ok()?;
    // Send ready in plaintext so the client knows the shared key is available for encrypted authentication.
    ws_tx
        .send(Message::Text(e2ee::MSG_READY.to_string()))
        .await
        .ok()?;
    // Await encrypted authentication.
    let auth_raw = match ws_rx.next().await {
        Some(Ok(Message::Text(t))) => t,
        _ => return None,
    };
    let (token, password, device_id, device_name) = cipher
        .decrypt_text(&auth_raw)
        .and_then(|p| e2ee::parse_auth(&p))?;
    let token_ok = ctx.auth.validate_pairing_token(&token);
    let pw_ok = password
        .as_deref()
        .map(|p| ctx.auth.verify_password(p))
        .unwrap_or(false);
    // Reject revoked devices even with a valid pairing token and password, blocking reconnect and re-pairing.
    // Without a self-reported device_id, device-level revocation is unavailable, matching registry placeholder behavior.
    let blocked = device_id
        .as_deref()
        .map(|id| ctx.auth.is_blocked(id))
        .unwrap_or(false);
    if token_ok && pw_ok && !blocked {
        // After both credentials pass and the device is not revoked, register its self-reported display identity.
        ctx.auth
            .register_device(device_id.as_deref(), device_name.as_deref());
        let ct = cipher.encrypt_text(e2ee::MSG_AUTHENTICATED)?;
        ws_tx.send(Message::Text(ct)).await.ok()?;
        // Return device_id so the main loop can detect revocation during later heartbeats.
        Some((cipher, device_id))
    } else {
        // Return an encrypted error, proving key exchange succeeded but identity failed, then close.
        if let Some(ct) = cipher.encrypt_text(&e2ee::err_msg("unauthorized")) {
            let _ = ws_tx.send(Message::Text(ct)).await;
        }
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_text(
    ctx: &Ctx,
    text: &str,
    conn_source: &str,
    out_tx: &mpsc::UnboundedSender<Message>,
    pty_subs: &mut HashMap<String, u64>,
    listened: &mut HashSet<String>,
    event_ids: &mut Vec<ListenerId>,
) {
    let msg: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    match msg.get("t").and_then(Value::as_str).unwrap_or("") {
        "invoke" => {
            let id = msg.get("id").cloned().unwrap_or(Value::Null);
            let cmd = msg
                .get("cmd")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let args = msg.get("args").cloned().unwrap_or(Value::Null);
            if matches!(cmd.as_str(), "pty_write" | "pty_resize") {
                // Keep frequent, lightweight keyboard and resize operations synchronous in the read loop for responsiveness.
                let reply = match dispatch(&ctx.app, &cmd, &args, conn_source) {
                    Ok(result) => json!({"t":"reply","id":id,"ok":true,"result":result}),
                    Err(e) => json!({"t":"reply","id":id,"ok":false,"error":e}),
                };
                let _ = out_tx.send(Message::Text(reply.to_string()));
            } else {
                // Other commands commonly block on SQLite's global lock, subprocesses, or file reads. Move them
                // to spawn_blocking so one slow request cannot head-of-line block later messages or occupy a Tokio
                // worker and affect other connections. Replies return asynchronously and match by ID, not order.
                let ctx = ctx.clone();
                let out_tx = out_tx.clone();
                let conn_source = conn_source.to_string();
                tokio::task::spawn_blocking(move || {
                    let reply = match dispatch(&ctx.app, &cmd, &args, &conn_source) {
                        Ok(result) => json!({"t":"reply","id":id,"ok":true,"result":result}),
                        Err(e) => json!({"t":"reply","id":id,"ok":false,"error":e}),
                    };
                    let _ = out_tx.send(Message::Text(reply.to_string()));
                });
            }
        }
        "pty-spawn" => {
            let id = msg.get("id").cloned().unwrap_or(Value::Null);
            let sid = msg
                .get("sid")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let args = msg.get("args").cloned().unwrap_or(Value::Null);
            // Reconnect attachment never starts a process. If another client closed the session while offline,
            // return an error so this client closes its view instead of resurrecting an empty shell.
            let attach_only = msg
                .get("attachOnly")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            // Register status/exit forwarding once per session before spawn. Attach synchronously emits a state
            // snapshot during spawn, and a new agent emits its marker there as well, so later registration would
            // miss them. A listener left after failed spawn is harmless and is removed when the connection closes.
            if listened.insert(sid.clone()) {
                event_ids.push(listen_forward(
                    &ctx.app,
                    &format!("pty://status/{sid}"),
                    out_tx.clone(),
                ));
                event_ids.push(listen_forward(
                    &ctx.app,
                    &format!("pty://exit/{sid}"),
                    out_tx.clone(),
                ));
                // Notify this view when another client explicitly kills the session by closing or restarting it.
                event_ids.push(listen_forward(
                    &ctx.app,
                    &format!("pty://killed/{sid}"),
                    out_tx.clone(),
                ));
            }
            // Serialize SpawnResult directly in camelCase to avoid maintaining parallel handwritten JSON. This
            // automatically includes cols/rows/owner, the initial-size channel used by mirror mode.
            let reply =
                match web_pty_spawn(ctx, &sid, &args, conn_source, attach_only, out_tx.clone()) {
                    Ok(res) => {
                        pty_subs.insert(sid.clone(), res.sub_id);
                        let result = serde_json::to_value(&res).unwrap_or(Value::Null);
                        json!({"t":"reply","id":id,"ok":true,"result":result})
                    }
                    Err(e) => json!({"t":"reply","id":id,"ok":false,"error":e}),
                };
            let _ = out_tx.send(Message::Text(reply.to_string()));
        }
        "pty-detach" => {
            let sid = msg.get("sid").and_then(Value::as_str).unwrap_or("");
            if let Some(sub) = msg.get("subId").and_then(Value::as_u64) {
                ctx.app.pty().detach(&ctx.app, sid, sub, conn_source);
            }
            pty_subs.remove(sid);
        }
        _ => {}
    }
}

/// Registers forwarding for one event name, sending `{t:"event",name,payload}` through the outbound channel.
fn listen_forward(app: &AppCtx, name: &str, out_tx: mpsc::UnboundedSender<Message>) -> ListenerId {
    let name_owned = name.to_string();
    app.listen(name, move |payload| {
        let payload: Value = serde_json::from_str(payload).unwrap_or(Value::Null);
        let msg = json!({ "t": "event", "name": name_owned, "payload": payload });
        let _ = out_tx.send(Message::Text(msg.to_string()));
    })
}

/// WS-side PTY spawn-or-attach. Builds a sink that wraps bytes in binary frames for the outbound channel,
/// mirrors the reconnect/remote logic of `pty_spawn`, then calls `PtyManager::spawn`, which attaches and replays
/// when the session already exists. `conn_source` identifies this connection and becomes sizing owner for a new
/// process. `attach_only` permits attachment without starting, as used during reconnect.
fn web_pty_spawn(
    ctx: &Ctx,
    sid: &str,
    args: &Value,
    conn_source: &str,
    attach_only: bool,
    out_tx: mpsc::UnboundedSender<Message>,
) -> Result<SpawnResult, String> {
    let kind: SessionKind =
        serde_json::from_value(args.get("kind").cloned().unwrap_or(Value::Null))
            .map_err(|e| format!("Invalid kind: {e}"))?;
    let shell = args
        .get("shell")
        .and_then(Value::as_str)
        .map(str::to_string);
    let cwd = args.get("cwd").and_then(Value::as_str).map(str::to_string);
    // Current theme brightness, used to set COLORFGBG for TUIs that do not query OSC 11.
    let dark = args.get("dark").and_then(Value::as_bool);
    let cols = args
        .get("cols")
        .and_then(Value::as_u64)
        .and_then(|v| u16::try_from(v).ok())
        .unwrap_or(80);
    let rows = args
        .get("rows")
        .and_then(Value::as_u64)
        .and_then(|v| u16::try_from(v).ok())
        .unwrap_or(24);
    let init_prompt = args
        .get("initialPrompt")
        .and_then(Value::as_str)
        .map(str::to_string);

    let app = &ctx.app;
    let (in_db, mut resume_id, fork, agent_args, perm, created_at) = {
        let conn = app.db().conn.lock().unwrap();
        let session = repo::get_session(&conn, sid)?;
        let in_db = session.is_some();
        let resume = session.as_ref().and_then(|s| s.agent_session_id.clone());
        let created_at = session.as_ref().map(|s| s.created_at).unwrap_or(0);
        // Merge custom launch arguments with model/effort and permission-mode flags, matching desktop behavior.
        let args = repo::get_agent_args(&conn, sid)?;
        let (model, effort) = repo::get_model_effort(&conn, sid)?;
        let args = crate::agent::inject::merge_model_effort_flags(
            kind,
            model.as_deref(),
            effort.as_deref(),
            args.as_deref(),
        );
        let perm = repo::get_permission_mode(&conn, sid)?;
        let args =
            crate::agent::inject::merge_permission_flag(kind, perm.as_deref(), args.as_deref());
        (
            in_db,
            resume,
            repo::get_fork_pending(&conn, sid)?,
            args,
            perm,
            created_at,
        )
    };
    // As in commands.rs::pty_spawn, reject spawning a session deleted by another client so a stale tree cannot
    // create an orphan process. Temporary split sessions with the eph- prefix are not stored and remain allowed.
    if !in_db && !sid.starts_with("eph-") {
        return Err("Session has been deleted".to_string());
    }
    resume_id = resume_id.filter(|id| !crate::agent::resume::confirmed_missing(kind, id));
    if kind == SessionKind::Pi && resume_id.is_none() && !fork && in_db {
        if let Some(cwd_for_repair) = cwd.as_deref() {
            let created = created_at.max(0) as u64;
            let since = UNIX_EPOCH + Duration::from_secs(created.saturating_sub(10));
            for agent_id in crate::agent::resume::capture_pi_session_candidates_oldest_first_since(
                Some(cwd_for_repair),
                since,
            ) {
                let changed = {
                    let conn = app.db().conn.lock().unwrap();
                    repo::claim_agent_session_id(&conn, sid, &agent_id, SessionKind::Pi)?
                };
                if changed {
                    app.emit(crate::host::TREE_CHANGED, ());
                    resume_id = Some(agent_id);
                    break;
                }
            }
        }
    }
    let hook = app.hooks().endpoint();

    // WS output sink wraps bytes as [len][sid][payload]. Once connection closure drops out_rx, send fails and
    // returns false so the reader thread removes this destination.
    let sid_bytes = sid.as_bytes().to_vec();
    let sink: OutputSink = Box::new(move |bytes: &[u8]| {
        let mut frame = Vec::with_capacity(1 + sid_bytes.len() + bytes.len());
        frame.push(sid_bytes.len() as u8);
        frame.extend_from_slice(&sid_bytes);
        frame.extend_from_slice(bytes);
        out_tx.send(Message::Binary(frame)).is_ok()
    });

    let mgr = app.pty();
    mgr.spawn(
        app.clone(),
        sid.to_string(),
        kind,
        shell,
        cwd,
        dark,
        cols,
        rows,
        hook,
        resume_id,
        fork,
        init_prompt,
        agent_args,
        perm,
        None, // Browser clients do not currently send theme brightness; the workaround is desktop-only.
        conn_source,
        attach_only,
        sink,
    )
}
