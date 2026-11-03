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
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, RawQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use super::outbound;

use super::dispatch::{dispatch, CallOrigin};
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
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Authentication accepts only `?token=` because browser WebSocket APIs cannot set custom headers and the
    // cookie mechanism has been removed (see auth.rs). Preserve missing/invalid/valid state in a fingerprint
    // used by rejection logs and error frames, distinguishing absent credentials from expired sessions.
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
    // The share relay tunnel injects the grant and account it authorized for this request. In share mode this
    // is the only accepted admission: the loopback instance has no usable password and no pairing tokens.
    let share = if ctx.mode == ServeMode::ShareTunnel {
        match share_scope_from_headers(&ctx.app, &headers) {
            Some(scope) => Some(scope),
            None => {
                crate::diagnostic_warn!("[ws] rejected: share tunnel request without a valid grant");
                return (StatusCode::FORBIDDEN, "Missing share authorization").into_response();
            }
        }
    } else {
        None
    };
    ws.on_upgrade(move |socket| handle_socket(socket, ctx, query_authed, auth_fp, addr.ip(), share))
}

/// Reads the tunnel-injected grant and account headers and resolves them against the host's local share
/// records. The tunnel gate guarantees the request came through the in-process tunnel client, so a valid
/// local share is sufficient authorization.
fn share_scope_from_headers(app: &AppCtx, headers: &HeaderMap) -> Option<super::share_policy::ShareScope> {
    let share_id = headers.get("x-vlx-share")?.to_str().ok()?;
    let account_id = headers
        .get("x-vlx-account")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    super::public_relay::share_scope_for(app, share_id, account_id)
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

/// Requests one connection may have outstanding. Further frames are not read until a reply is queued, so a
/// client that floods requests is slowed by TCP backpressure instead of holding unbounded replies in memory.
const MAX_PENDING_REPLIES: usize = 32;

/// How long a reply may wait for earlier output to drain before the connection counts as stuck. It stays
/// below the client's 30-second snapshot timeout so a stuck connection closes before the request gives up.
const REPLY_PATIENCE: Duration = Duration::from_secs(20);

/// An input or resize frame that arrived while its session's spawn was still running.
struct DeferredPtyCall {
    id: Value,
    cmd: String,
    args: Value,
}

/// PTY bookkeeping one connection shares between its read loop and spawns finishing on the blocking pool.
#[derive(Default)]
struct PtyState {
    /// sid -> subscription ID, used for detach on disconnect.
    subs: HashMap<String, u64>,
    /// Sessions with a spawn in flight. Their input and resize frames wait here and run in arrival order
    /// once the spawn returns, as they did when spawns ran inside the read loop.
    spawning: HashMap<String, Vec<DeferredPtyCall>>,
    /// Set by connection cleanup; a spawn finishing later detaches at once instead of leaving a subscription.
    closed: bool,
}

type SharedPtyState = Arc<std::sync::Mutex<PtyState>>;

async fn handle_socket(
    socket: WebSocket,
    ctx: Ctx,
    token_authed: bool,
    auth_fp: String,
    ip: std::net::IpAddr,
    mut share: Option<super::share_policy::ShareScope>,
) {
    let conn_source = format!("ws-{}", NEXT_CONN_ID.fetch_add(1, Ordering::SeqCst));
    let mut diagnostic=crate::diagnostics::Span::new("ws_connection",serde_json::json!({"clientId":conn_source}));
    diagnostic.step("handshake");
    let (mut ws_tx, mut ws_rx) = socket.split();

    // ---- E2EE negotiation prelude: inspect the first frame ----
    // An e2ee_hello first frame starts E2EE device-token and password authentication. Otherwise treat the
    // client as plaintext, require a validated `?token=`, and retain the frame as its first application message.
    let mut cipher: Option<Cipher> = None;
    // Self-reported device ID available only from E2EE; heartbeats use it to detect mid-connection revocation.
    let mut device_id: Option<String> = None;
    // Self-reported display name available only from E2EE; the host's badge shows it to name this client.
    let mut device_name: Option<String> = None;
    let mut pending_first: Option<String> = None;
    // Set when a share connection was upgraded to full access: `(grant ID, account ID, device key)`, re-checked
    // on every heartbeat so revoking the device or disabling full access ends the session.
    let mut full_access: Option<(String, String, String)> = None;
    match ws_rx.next().await {
        Some(Ok(Message::Text(first))) => {
            if let Some(client_pub) = e2ee::parse_hello(&first) {
                let device = e2ee::parse_hello_device(&first);
                match handshake(&ctx, &mut ws_tx, &mut ws_rx, &client_pub, ip, share.as_ref(), device.as_ref()).await {
                    Some((c, did, dname, full)) => {
                        cipher = Some(c);
                        device_id = did;
                        device_name = dname;
                        if full {
                            // From here on the connection is served like a paired LAN client: the regular
                            // remote dispatch, terminals and the full event set, still without management commands.
                            let authority = share.take().and_then(|scope| scope.push_authority);
                            let (Some((grant, account)), Some((key, _))) = (authority, device) else { return };
                            crate::diagnostics::record("INFO", "share_full_access", json!({"status":"granted"}));
                            full_access = Some((grant, account, key));
                        }
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
                    crate::diagnostic_warn!("[ws] rejected: pairing (E2EE) required in LanTls mode, got plaintext first frame");
                    let _ = ws_tx
                        .send(Message::Text(
                            json!({"t":"error","code":"pairing_required","message":"WebSocket rejected: this server requires pairing (E2EE); plaintext connections are not accepted"}).to_string(),
                        ))
                        .await;
                    return;
                }
                // Plaintext mode requires a valid `?token=`; otherwise send an error, log, and close.
                if !token_authed {
                    crate::diagnostic_warn!("[ws] rejected: unauthenticated connection ({auth_fp})");
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

    let (out_tx, mut out_rx) = outbound::channel();
    // Source ID for this connection. Each browser page has an independent WS, distinct from desktop.
    diagnostic.step("read");


    // The hello message is the **first application frame** and gives the frontend its source ID for comparing
    // against Resized/SpawnResult.owner in fit and mirror decisions. The writer encrypts it in E2EE mode and
    // consumes out_tx serially, guaranteeing delivery before replies, events, or PTY replay.
    let _ = out_tx.send(Message::Text(
        json!({"t": "hello", "source": conn_source,"historyProtocol":2}).to_string(),
    ));

    // Announce the arrival. On a host this is the only sign that someone else is attached: the desktop talks
    // over IPC and never appears in this list, so a non-empty list means somebody remote is on the other end.
    // The identity travels with it so the host can name who that is; only `ip` is observed rather than
    // self-reported, which is why none of it gates anything.
    // The joining client is not listening yet and reads the list from its own `mirror_get` alignment instead.
    ctx.app.emit(
        crate::web::presence::CLIENTS_EVENT,
        crate::web::presence::join(crate::web::presence::ClientInfo::arriving(
            conn_source.clone(),
            device_name.clone(),
            device_id.clone(),
            ip.to_string(),
        )),
    );

    // Writer task: serialize every outbound reply, event, and PTY binary frame to the socket. In E2EE mode,
    // text becomes base64 ciphertext and binary becomes raw ciphertext; control frames such as Close/Ping/Pong
    // pass through unchanged. Encryption failure terminates the writer and closes the connection.
    let cipher_w = cipher.clone();
    let mut writer = tokio::spawn(async move {
        while let Some(queued) = out_rx.recv().await {
            let msg=queued.message;
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
            if !matches!(tokio::time::timeout(Duration::from_secs(5),ws_tx.send(msg)).await,Ok(Ok(()))) {
                break;
            }
        }
        if out_rx.pressured() {
            crate::diagnostic_warn!("[ws] outbound byte or item budget exceeded; closing slow connection for resynchronization");
            let close=Message::Close(Some(axum::extract::ws::CloseFrame {code:1013,reason:"Outbound budget exceeded; reconnect to resynchronize".into()}));
            let _=tokio::time::timeout(Duration::from_secs(1),ws_tx.send(close)).await;
        }
    });

    let mut event_ids: Vec<ListenerId> = Vec::new();
    // PTY subscriptions owned by this connection and spawns still running for it.
    let pty_state: SharedPtyState = Default::default();
    // Sessions with status/exit forwarding already registered, preventing duplicates.
    let mut listened: HashSet<String> = HashSet::new();
    let mut chat_listened: HashSet<String> = HashSet::new();

    // Register session-independent global forwarding. Share connections receive only the events the shared
    // session view consumes: tree refreshes, settings sync, and session state dots. Spawn/orchestration
    // requests, mirror layout, presence, clone progress, knowledge and usage stay host-private.
    let share_events: [&str; 3] = [
        crate::host::TREE_CHANGED,
        crate::host::SETTINGS_CHANGED,
        crate::session_state::STATE_EVENT,
    ];
    let full_events: [&str; 18] = [
        "spawn://request",
        "spawn://resolved",
        "plan-execute://proposal",
        "view://request",
        "knowledge://changed",
        crate::host::TREE_CHANGED,
        crate::host::PRESETS_CHANGED,
        // Preferences are backend-authoritative; this tells every client to re-read them mid-run.
        crate::host::SETTINGS_CHANGED,
        // Authoritative session facts. Registered here, per connection, rather than on attach: a dot on a
        // session this client has never opened is exactly what the per-attach registration could not do.
        crate::session_state::STATE_EVENT,
        crate::git::CLONE_PROGRESS_EVENT,
        // The Files and Git tabs refresh on this instead of polling where the platform can watch.
        crate::fs_watch::CHANGED_EVENT,
        // UI mirror: the shared layout and the on/off switch both reach every client this way.
        crate::web::mirror::LAYOUT_EVENT,
        crate::web::mirror::MODE_EVENT,
        // How many clients are attached; a follower shows it next to its own mirror badge.
        crate::web::presence::CLIENTS_EVENT,
        // Account usage is polled once per machine; every client is told when that copy changes.
        crate::agent::usage_store::USAGE_EVENT,
        crate::agent::remote_model_catalog::EVENT,
        "translation://settings",
        "translation://defaults",
    ];
    let global_events: &[&str] = if share.is_some() {
        &share_events
    } else {
        &full_events
    };
    for name in global_events {
        event_ids.push(listen_forward(&ctx.app, name, out_tx.clone()));
    }

    let pending_replies = Arc::new(Semaphore::new(MAX_PENDING_REPLIES));
    // Process the plaintext application frame retained during first-frame inspection.
    if let Some(first) = pending_first.take() {
        let Ok(permit) = pending_replies.clone().acquire_owned().await else { return };
        handle_text(
            &ctx,
            &first,
            &conn_source,
            &out_tx,
            permit,
            &pty_state,
            &mut listened,
            &mut chat_listened,
            &mut event_ids,
            share.clone(),
        );
    }

    // Read loop and heartbeat timer. Any inbound message, including pong, refreshes liveness. Send a ping each
    // interval and close a connection silent beyond LIVENESS_TIMEOUT as half-open. Normal cleanup then removes
    // forwarding listeners and detaches every subscription.
    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    let mut last_inbound = std::time::Instant::now();
    loop {
        tokio::select! {
            _ = &mut writer => break,
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
                        let Ok(permit) = pending_replies.clone().acquire_owned().await else { break };
                        handle_text(
                            &ctx,
                            &plain,
                            &conn_source,
                            &out_tx,
                            permit,
                            &pty_state,
                            &mut listened,
                            &mut chat_listened,
                            &mut event_ids,
                            share.clone(),
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
                if let Some((grant, account, key)) = &full_access {
                    if !super::full_access::still_authorized(&ctx.app, grant, account, key) {
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
    // Deregister before detaching so the host stops showing this client the moment the socket is gone,
    // whether it closed cleanly, timed out as half-open, or was revoked mid-connection.
    ctx.app.emit(
        crate::web::presence::CLIENTS_EVENT,
        crate::web::presence::leave(&conn_source),
    );
    let subs = {
        let mut state = pty_state.lock().unwrap();
        state.closed = true;
        state.spawning.clear();
        std::mem::take(&mut state.subs)
    };
    let mgr = ctx.app.pty();
    for (sid, sub) in subs {
        mgr.detach(&ctx.app, &sid, sub, &conn_source);
    }
    writer.abort();
    diagnostic.success();
}

/// E2EE handshake after receiving the client public key: derive the shared key, return plaintext `e2ee_ready`,
/// await encrypted `e2ee_auth`, validate it, then return encrypted `e2ee_authenticated`. Success returns a
/// [`Cipher`] together with the self-reported device ID and name; decryption/authentication failure or
/// disconnect returns None and ends the connection. Both a valid device token **and** the correct password
/// are required.
///
/// This surface is reachable unauthenticated on 0.0.0.0, so it is hardened against Argon2 DoS: a rate-limited
/// IP is rejected before any credential work, the memory-hard password verify only runs for holders of the
/// current pairing token (constant-time check first), and it executes on the blocking pool behind the
/// process-wide semaphore. Every failed handshake — wrong token or wrong password — counts as a failure.
async fn handshake(
    ctx: &Ctx,
    ws_tx: &mut SplitSink<WebSocket, Message>,
    ws_rx: &mut SplitStream<WebSocket>,
    client_pub_b64: &str,
    ip: std::net::IpAddr,
    share: Option<&super::share_policy::ShareScope>,
    device: Option<&(String, String)>,
) -> Option<(Cipher, Option<String>, Option<String>, bool)> {
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
    // Share visitors hold no host password and no pairing token: the relay already authorized this grant, and
    // the tunnel client injected the matching scope, so E2EE only proves key agreement. Device registration
    // and the host blocklist do not apply to them.
    // A visitor presenting an approved device key and a proof bound to this session is upgraded to full
    // access; anything less stays within the share policy.
    if let Some(scope) = share {
        let full = device.is_some_and(|(key, proof)| {
            super::full_access::authorize(&ctx.app, scope, &ctx.e2ee_keys, key, proof, client_pub_b64)
        });
        let reply = if full { e2ee::MSG_AUTHENTICATED_FULL } else { e2ee::MSG_AUTHENTICATED };
        let ct = cipher.encrypt_text(reply)?;
        ws_tx.send(Message::Text(ct)).await.ok()?;
        return Some((cipher, device_id, device_name, full));
    }
    // Rate limit before any credential verification; the explicit encrypted reason lets clients distinguish
    // throttling from a wrong password.
    // The reservation covers the whole credential check and moves into the Argon2 task, so a client
    // disconnecting mid-verify cannot release it before the verdict exists.
    let Some(attempt) = ctx.limiter.allow(ip) else {
        if let Some(ct) = cipher.encrypt_text(&e2ee::err_msg("rate_limited")) {
            let _ = ws_tx.send(Message::Text(ct)).await;
        }
        return None;
    };
    // Only a holder of the current pairing token may trigger the expensive Argon2 verify; the token check
    // is constant-time and cheap, so unauthenticated strangers cost the server nothing memory-hard.
    // `verified` carries the still-open reservation back only when the password matched.
    let verified = match password.as_deref() {
        Some(p) if ctx.auth.validate_pairing_token(&token) => ctx.auth.verify_attempt(p, attempt).await,
        _ => {
            attempt.failure();
            None
        }
    };
    // Reject revoked devices even with a valid pairing token and password, blocking reconnect and re-pairing.
    // Without a self-reported device_id, device-level revocation is unavailable, matching registry placeholder behavior.
    let blocked = device_id
        .as_deref()
        .map(|id| ctx.auth.is_blocked(id))
        .unwrap_or(false);
    match verified {
        Some(attempt) if !blocked => {
            attempt.success();
            // After both credentials pass and the device is not revoked, register its self-reported display identity.
            ctx.auth
                .register_device(device_id.as_deref(), device_name.as_deref());
            let ct = cipher.encrypt_text(e2ee::MSG_AUTHENTICATED)?;
            ws_tx.send(Message::Text(ct)).await.ok()?;
            // Return device_id so the main loop can detect revocation during later heartbeats, and the name
            // so the presence registry can show who just attached.
            Some((cipher, device_id, device_name, false))
        }
        verified => {
            // A revoked device with correct credentials still counts as a failed attempt.
            if let Some(attempt) = verified {
                attempt.failure();
            }
            // Return an encrypted error, proving key exchange succeeded but identity failed, then close.
            if let Some(ct) = cipher.encrypt_text(&e2ee::err_msg("unauthorized")) {
                let _ = ws_tx.send(Message::Text(ct)).await;
            }
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_text(
    ctx: &Ctx,
    text: &str,
    conn_source: &str,
    out_tx: &outbound::Sender,
    // Held until this frame's reply is queued; see MAX_PENDING_REPLIES.
    permit: OwnedSemaphorePermit,
    pty_state: &SharedPtyState,
    listened: &mut HashSet<String>,
    // Sessions whose chat channel this connection forwards. Separate from `listened`: a session can have
    // both a PTY and a chat engine, registered at different moments.
    chat_listened: &mut HashSet<String>,
    event_ids: &mut Vec<ListenerId>,
    // Present on share-tunnel connections; all commands are filtered through the grant's scope.
    share: Option<super::share_policy::ShareScope>,
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
            let trace_request=msg.get("diagnostic").and_then(|v|v.get("requestId")).and_then(Value::as_str)
                .filter(|v|v.len()==36 && uuid::Uuid::parse_str(v).is_ok()).map(str::to_owned);
            let trace_operation=msg.get("diagnostic").and_then(|v|v.get("operationId")).and_then(Value::as_str)
                .filter(|v|v.len()==36 && uuid::Uuid::parse_str(v).is_ok()).map(str::to_owned);
            // The caller's trust classification follows this instance's serve mode: Electron's loopback
            // sidecar clients are local; LAN-exposed and share-tunnel instances serve remote clients.
            let origin = CallOrigin::for_serve_mode(ctx.mode);
            // A chat session has no PTY, so its events have no pty-spawn to hang registration off. Register
            // them when the client first asks about that session — starting the engine or reading its state —
            // and do it before dispatching, since starting emits its first events synchronously. Share clients
            // only register sessions their grant covers, so no other session's events reach them.
            if matches!(cmd.as_str(), "chat_start" | "chat_snapshot" | "translation_session") {
                if let Some(sid) = args.get("sessionId").and_then(Value::as_str) {
                    let covered = share
                        .as_ref()
                        .is_none_or(|scope| scope.covers_session(&ctx.app, sid));
                    if covered && chat_listened.insert(sid.to_string()) {
                        event_ids.push(listen_forward(
                            &ctx.app,
                            &crate::agent::chat::engine::event_name(sid),
                            out_tx.clone(),
                        ));
                        if share.is_none() {
                            event_ids.push(listen_forward(&ctx.app, &format!("translation://session/{sid}"), out_tx.clone()));
                        }
                        // Work state reaches the sidebar through the channel PTY sessions use, so a session
                        // that never spawns a PTY still needs it registered.
                        if listened.insert(sid.to_string()) {
                            event_ids.push(listen_forward(
                                &ctx.app,
                                &format!("pty://status/{sid}"),
                                out_tx.clone(),
                            ));
                        }
                    }
                }
            }
            if matches!(cmd.as_str(), "pty_write" | "pty_resize") {
                // A session still spawning has no process to take the frame yet; it runs when the spawn returns.
                if let Some(sid) = args.get("sessionId").and_then(Value::as_str) {
                    let mut state = pty_state.lock().unwrap();
                    if let Some(deferred) = state.spawning.get_mut(sid) {
                        deferred.push(DeferredPtyCall { id, cmd, args: args.clone() });
                        return;
                    }
                }
                // Keep frequent, lightweight keyboard and resize operations synchronous in the read loop for responsiveness.
                let reply =
                    match dispatch_scoped(&ctx.app, share.as_ref(), &cmd, &args, conn_source, origin) {
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
                let share = share.clone();
                let queued = std::time::Instant::now();
                let reply = tokio::task::spawn_blocking(move || {
                    let _context=crate::diagnostics::Context::enter(trace_request.as_deref());
                    let _operation=crate::diagnostics::Operation::enter(trace_operation.as_deref());
                    crate::diagnostics::record("DEBUG", "rpc_queue", serde_json::json!({"command":cmd,"requestId":trace_request,"queueMs":queued.elapsed().as_millis() as u64}));
                    let started = std::time::Instant::now();
                    let reply = match dispatch_scoped(&ctx.app, share.as_ref(), &cmd, &args, &conn_source, origin) {
                        Ok(result) => json!({"t":"reply","id":id,"ok":true,"result":result}),
                        Err(e) => json!({"t":"reply","id":id,"ok":false,"error":e}),
                    };
                    let prepare_ms = started.elapsed().as_millis();
                    let encoding = std::time::Instant::now();
                    let mut encoded = reply.to_string();
                    encoded.shrink_to_fit();
                    if cmd == "chat_snapshot" {
                        crate::diagnostics::record("INFO","chat_sync",serde_json::json!({"requestId":trace_request,"prepareMs":prepare_ms as u64,"encodeMs":encoding.elapsed().as_millis() as u64,"bytes":encoded.len(),"success":reply["ok"]}));
                    }
                    // Original image reads have their existing explicit five-MiB limit. They use
                    // a single separately charged slot rather than consuming the control reserve.
                    let image=cmd=="chat_attachment" && args["purpose"]!="preview";
                    (Message::Text(encoded),image)
                });
                // Replies wait for room rather than failing: the client asked for them, and the burst a
                // reconnect sends to resynchronize would otherwise close every new connection again.
                tokio::spawn(async move {
                    let Ok((message,image))=reply.await else {return};
                    let _=if image {out_tx.attachment(message)} else {out_tx.reply(message,REPLY_PATIENCE).await};
                    drop(permit);
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
            let mut args = msg.get("args").cloned().unwrap_or(Value::Null);
            if let Some(scope) = share.as_ref() {
                // Only the host's own account gets a terminal through a share, and only a terminal-view agent
                // inside the grant. The launch then uses host records: the client's kind, shell, directory and
                // initial prompt are discarded, so a visitor page cannot choose what runs or where.
                match super::share_policy::owner_terminal_session(&ctx.app, scope, &sid) {
                    Ok(session) => args = owner_spawn_args(&ctx.app, &session, &args),
                    Err(_) => {
                        let _ = out_tx.send(Message::Text(
                            json!({"t":"reply","id":id,"ok":false,"error":"Terminals are unavailable through a public share"}).to_string(),
                        ));
                        return;
                    }
                }
            }
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
            // Starting a shell reads profiles and can take seconds. Run it on the blocking pool so a workspace
            // restoring several terminals does not hold every other request on this connection behind them.
            pty_state.lock().unwrap().spawning.entry(sid.clone()).or_default();
            let ctx = ctx.clone();
            let out_tx = out_tx.clone();
            let conn_source = conn_source.to_string();
            let pty_state = pty_state.clone();
            let origin = CallOrigin::for_serve_mode(ctx.mode);
            tokio::spawn(async move {
                // An attach replays the session's whole scrollback. Reserve room for it before attaching, so a
                // reconnect reattaching many busy terminals queues their replays instead of exceeding the budget.
                let Ok(ticket) = out_tx.replay_ticket(REPLY_PATIENCE).await else {
                    pty_state.lock().unwrap().spawning.remove(&sid);
                    drop(permit);
                    return;
                };
                let _ = tokio::task::spawn_blocking(move || {
                    let spawned = web_pty_spawn(&ctx, &sid, &args, &conn_source, attach_only, out_tx.clone(), ticket);
                    // Serialize SpawnResult directly in camelCase to avoid maintaining parallel handwritten JSON. This
                    // automatically includes cols/rows/owner, the initial-size channel used by mirror mode.
                    let reply = match &spawned {
                        Ok(res) => {
                            let result = serde_json::to_value(res).unwrap_or(Value::Null);
                            json!({"t":"reply","id":id,"ok":true,"result":result})
                        }
                        Err(e) => json!({"t":"reply","id":id,"ok":false,"error":e}),
                    };
                    let mut state = pty_state.lock().unwrap();
                    if state.closed {
                        drop(state);
                        if let Ok(res) = spawned { ctx.app.pty().detach(&ctx.app, &sid, res.sub_id, &conn_source); }
                        drop(permit);
                        return;
                    }
                    if let Ok(res) = &spawned { state.subs.insert(sid.clone(), res.sub_id); }
                    let _ = out_tx.send(Message::Text(reply.to_string()));
                    // Replay held frames under the lock, so a frame read meanwhile cannot overtake them.
                    for call in state.spawning.remove(&sid).unwrap_or_default() {
                        let reply = match dispatch_scoped(&ctx.app, share.as_ref(), &call.cmd, &call.args, &conn_source, origin) {
                            Ok(result) => json!({"t":"reply","id":call.id,"ok":true,"result":result}),
                            Err(e) => json!({"t":"reply","id":call.id,"ok":false,"error":e}),
                        };
                        let _ = out_tx.send(Message::Text(reply.to_string()));
                    }
                    drop(permit);
                }).await;
            });
        }
        "pty-detach" => {
            let sid = msg.get("sid").and_then(Value::as_str).unwrap_or("");
            if let Some(sub) = msg.get("subId").and_then(Value::as_u64) {
                ctx.app.pty().detach(&ctx.app, sid, sub, conn_source);
            }
            pty_state.lock().unwrap().subs.remove(sid);
        }
        _ => {}
    }
}

/// Routes a command through the share policy when the connection belongs to a share tunnel, and through the
/// regular dispatch otherwise. Keeping one entry point means share connections can never reach an arm that
/// bypasses their scope.
fn dispatch_scoped(
    app: &AppCtx,
    share: Option<&super::share_policy::ShareScope>,
    cmd: &str,
    args: &Value,
    who: &str,
    origin: CallOrigin,
) -> Result<Value, String> {
    match share {
        Some(scope) => super::share_policy::dispatch_shared(app, scope, cmd, args, who, origin),
        None => dispatch(app, cmd, args, who, origin),
    }
}

/// Registers forwarding for one event name, sending `{t:"event",name,payload}` through the outbound channel.
fn listen_forward(app: &AppCtx, name: &str, out_tx: outbound::Sender) -> ListenerId {
    let name_owned = name.to_string();
    app.listen(name, move |payload| {
        let payload: Value = serde_json::from_str(payload).unwrap_or(Value::Null);
        let presentation=name_owned.starts_with("chat://") && matches!(payload["type"].as_str(),Some("rows"|"replaceRows"));
        let msg = json!({ "t": "event", "name": name_owned, "payload": payload });
        let message=Message::Text(msg.to_string());
        let _=if presentation {out_tx.presentation(message)} else {out_tx.send(message)};
    })
}

/// Spawn arguments for a share owner's terminal, rebuilt from the session record. Only the grid size, theme
/// brightness and diagnostic identifiers come from the client. A session without its own directory runs in
/// its project's, as the host frontend launches it.
fn owner_spawn_args(app: &AppCtx, session: &crate::models::Session, client: &Value) -> Value {
    let cwd = session.cwd.clone().filter(|c| !c.trim().is_empty()).or_else(|| {
        let conn = app.db().conn.lock().ok()?;
        repo::get_project_root(&conn, &session.project_id).ok().flatten()
    });
    let mut args = json!({"kind":session.kind,"shell":session.shell,"cwd":cwd});
    for key in ["cols", "rows", "dark", "diagnosticRequestId", "diagnosticOperationId"] {
        if let Some(value) = client.get(key) {
            args[key] = value.clone();
        }
    }
    args
}

/// Gives back a replay reservation the attach did not spend, on every return path of the spawn.
struct ReleaseTicket(Arc<Mutex<Option<outbound::ReplayTicket>>>);
impl Drop for ReleaseTicket {
    fn drop(&mut self) {
        if let Ok(mut ticket) = self.0.lock() {
            ticket.take();
        }
    }
}

/// WS-side PTY spawn-or-attach. Builds a sink that wraps bytes in binary frames for the outbound channel,
/// mirrors the reconnect/remote logic of `pty_spawn`, then calls `PtyManager::spawn`, which attaches and replays
/// when the session already exists. `conn_source` identifies this connection and becomes sizing owner for a new
/// process. `attach_only` permits attachment without starting, as used during reconnect. `ticket` is the room
/// reserved for the attach replay; it is released once the spawn returns if no replay used it.
fn web_pty_spawn(
    ctx: &Ctx,
    sid: &str,
    args: &Value,
    conn_source: &str,
    attach_only: bool,
    out_tx: outbound::Sender,
    ticket: outbound::ReplayTicket,
) -> Result<SpawnResult, String> {
    let _context=crate::diagnostics::Context::enter(args.get("diagnosticRequestId").and_then(Value::as_str));
    let _operation=crate::diagnostics::Operation::enter(args.get("diagnosticOperationId").and_then(Value::as_str));
    let mut diagnostic=crate::diagnostics::Span::new("pty_prepare",serde_json::json!({"sessionId":sid}));
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
        // Merge custom launch arguments with the agent-specific permission-mode flag, matching desktop behavior.
        let args = repo::get_agent_args(&conn, sid)?;
        let perm = repo::get_permission_mode(&conn, sid)?;
        let perm = crate::agent::permission_catalog::effective(&conn, kind, perm.as_deref())?;
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
    // Codex validation runs in PtyManager after the existing-process attach path.
    if kind != SessionKind::Codex {
        resume_id = crate::agent::resume::checked_resume_id(kind, resume_id)?;
    }
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
    // returns false so the reader thread removes this destination. The first chunk of an attach is the whole
    // replay and spends the reserved ticket; later chunks are live output under the presentation quota.
    let sid_bytes = sid.as_bytes().to_vec();
    let ticket = Arc::new(Mutex::new(Some(ticket)));
    let sink_ticket = ticket.clone();
    let sink: OutputSink = Box::new(move |bytes: &[u8]| {
        let mut frame = Vec::with_capacity(1 + sid_bytes.len() + bytes.len());
        frame.push(sid_bytes.len() as u8);
        frame.extend_from_slice(&sid_bytes);
        frame.extend_from_slice(bytes);
        let message = Message::Binary(frame);
        match sink_ticket.lock().unwrap().take() {
            Some(ticket) => out_tx.replay(message, ticket).is_ok(),
            None => out_tx.send(message).is_ok(),
        }
    });
    // A new process or an empty session has no replay; give the reserved room back once the spawn returns.
    let _release = ReleaseTicket(ticket);

    diagnostic.success();
    drop(diagnostic);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::HeadlessHost;
    use std::sync::Arc;

    /// Minimal Ctx around an isolated headless host — no socket is ever bound (tests must never bind
    /// 0.0.0.0); the WebServer inside stays stopped, matching the dispatch test fixture.
    fn test_ctx(mode: ServeMode) -> Ctx {
        let data_dir = std::env::temp_dir().join(format!("vlx-ws-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).expect("failed to create the temporary directory");
        let db = crate::db::Db::open(&data_dir.join("test.db"))
            .expect("failed to open the test database");
        let app = AppCtx::Headless(Arc::new(HeadlessHost::new(data_dir.clone(), db)));
        Ctx {
            app,
            // The verifier PHC is never exercised here; handle_text runs after authentication.
            auth: Arc::new(
                super::super::auth::AuthState::load_or_create("unused-phc", &data_dir)
                    .expect("failed to open the test pairing store"),
            ),
            e2ee_keys: Arc::new(e2ee::ServerKeys::load_or_create(&data_dir)
                .expect("failed to create test server keys")),
            mode,
            limiter: Arc::new(super::super::rate_limit::LoginRateLimiter::new()),
            tunnel_secret: None,
        }
    }

    /// Send one invoke frame through handle_text and await the reply. Non-PTY commands answer from
    /// spawn_blocking, so the reply arrives asynchronously on the outbound channel.
    async fn invoke_reply(ctx: &Ctx, cmd: &str) -> Value {
        let (out_tx, mut out_rx) = outbound::channel();
        let pty_state = SharedPtyState::default();
        let mut listened = HashSet::new();
        let mut chat_listened = HashSet::new();
        let mut event_ids = Vec::new();
        let frame = json!({"t": "invoke", "id": 1, "cmd": cmd, "args": {}}).to_string();
        handle_text(
            ctx,
            &frame,
            "ws-test",
            &out_tx,
            Arc::new(Semaphore::new(1)).try_acquire_owned().unwrap(),
            &pty_state,
            &mut listened,
            &mut chat_listened,
            &mut event_ids,
            None,
        );
        match out_rx.recv().await.expect("expected a reply frame").message {
            Message::Text(t) => serde_json::from_str(&t).expect("reply must be JSON"),
            other => panic!("expected a text reply, got: {other:?}"),
        }
    }

    /// The seam the dispatch tests cannot see: handle_text itself derives the origin from the serve mode.
    /// If it regressed to a hard-coded CallOrigin::Local, every dispatch-level gating test would stay
    /// green while remote WS clients regained the management plane. LanHttp must yield the gating error;
    /// LoopbackHttp (Electron sidecar) must pass the gate and fail only with the server's own
    /// not-started error.
    #[tokio::test]
    async fn handle_text_gates_management_commands_by_serve_mode() {
        let remote = test_ctx(ServeMode::LanHttp);
        let reply = invoke_reply(&remote, "web_pairing_create").await;
        assert_eq!(reply["ok"], json!(false));
        assert_eq!(
            reply["error"],
            json!("remote_cmd_forbidden:web_pairing_create"),
            "a LAN-exposed instance must gate management commands in handle_text"
        );

        let local = test_ctx(ServeMode::LoopbackHttp);
        let reply = invoke_reply(&local, "web_pairing_create").await;
        assert_eq!(reply["ok"], json!(false));
        assert_eq!(
            reply["error"],
            json!("Web server not started"),
            "the Electron loopback lane must keep the full management plane"
        );
    }

    #[tokio::test]
    async fn translation_session_subscribes_once_before_returning_its_snapshot() {
        let ctx=test_ctx(ServeMode::LoopbackHttp);
        let (out_tx,mut out_rx)=outbound::channel();
        let pty_state=SharedPtyState::default();let mut listened=HashSet::new();let mut chat_listened=HashSet::new();let mut event_ids=Vec::new();
        let sid={let conn=ctx.app.db().conn.lock().unwrap();let project=crate::db::repo::create_virtual_project(&conn,"Translation event fixture").unwrap();
            let session=crate::db::repo::create_session(&conn,&project.id,None,"Translation",SessionKind::Codex,None,None,None,None,None).unwrap();
            crate::db::repo::set_session_engine(&conn,&session.id,"chat").unwrap();session.id};
        for id in [1,2] {
            let frame=json!({"t":"invoke","id":id,"cmd":"translation_session","args":{"sessionId":sid}}).to_string();
            handle_text(&ctx,&frame,"translation-test",&out_tx,Arc::new(Semaphore::new(1)).try_acquire_owned().unwrap(),&pty_state,&mut listened,&mut chat_listened,&mut event_ids,None);
            let message=tokio::time::timeout(std::time::Duration::from_secs(5),out_rx.recv()).await.unwrap().unwrap().message;
            let Message::Text(text)=message else {panic!("expected snapshot reply")};
            let value:Value=serde_json::from_str(&text).unwrap();assert_eq!(value["ok"],true);
        }
        ctx.app.emit(&format!("translation://session/{sid}"),json!({}));
        let Message::Text(text)=out_rx.recv().await.unwrap().message else {panic!("expected translation event")};
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap()["name"],format!("translation://session/{sid}"));
        assert!(tokio::time::timeout(std::time::Duration::from_millis(200),out_rx.recv()).await.is_err(),"the event is delivered once");
        for id in event_ids {ctx.app.unlisten(id);}
    }

    async fn next_reply(out_rx:&mut outbound::Receiver) -> Value {
        let message=tokio::time::timeout(std::time::Duration::from_secs(5),out_rx.recv()).await.expect("expected a reply").unwrap().message;
        let Message::Text(text)=message else {panic!("expected a text reply")};
        serde_json::from_str(&text).unwrap()
    }

    /// Input for a session whose spawn is still running waits for that spawn instead of failing on a session
    /// that does not exist yet.
    #[tokio::test]
    async fn pty_input_waits_for_a_running_spawn() {
        let ctx=test_ctx(ServeMode::LoopbackHttp);
        let (out_tx,mut out_rx)=outbound::channel();
        let pty_state=SharedPtyState::default();let mut listened=HashSet::new();let mut chat_listened=HashSet::new();let mut event_ids=Vec::new();
        pty_state.lock().unwrap().spawning.insert("spawning-session".into(),Vec::new());
        let frame=json!({"t":"invoke","id":7,"cmd":"pty_write","args":{"sessionId":"spawning-session","data":"ls\r"}}).to_string();
        handle_text(&ctx,&frame,"ws-test",&out_tx,Arc::new(Semaphore::new(1)).try_acquire_owned().unwrap(),&pty_state,&mut listened,&mut chat_listened,&mut event_ids,None);
        assert!(tokio::time::timeout(std::time::Duration::from_millis(100),out_rx.recv()).await.is_err(),"no reply before the spawn returns");
        let state=pty_state.lock().unwrap();
        let deferred=&state.spawning["spawning-session"];
        assert_eq!(deferred.len(),1);
        assert_eq!(deferred[0].id,json!(7));
        assert_eq!(deferred[0].cmd,"pty_write");
    }

    /// The spawn runs off the read loop, yet its reply still precedes input sent right after it, and a finished
    /// spawn leaves nothing held for the session.
    #[tokio::test]
    async fn pty_spawn_replies_before_input_sent_after_it() {
        let ctx=test_ctx(ServeMode::LoopbackHttp);
        let (out_tx,mut out_rx)=outbound::channel();
        let pty_state=SharedPtyState::default();let mut listened=HashSet::new();let mut chat_listened=HashSet::new();let mut event_ids=Vec::new();
        let frames=[
            json!({"t":"pty-spawn","id":1,"sid":"deleted-session","args":{"kind":"terminal"}}),
            json!({"t":"invoke","id":2,"cmd":"pty_write","args":{"sessionId":"deleted-session","data":"x"}}),
        ];
        for frame in frames {
            handle_text(&ctx,&frame.to_string(),"ws-test",&out_tx,Arc::new(Semaphore::new(1)).try_acquire_owned().unwrap(),&pty_state,&mut listened,&mut chat_listened,&mut event_ids,None);
        }
        let spawn=next_reply(&mut out_rx).await;
        assert_eq!(spawn["id"],1);
        assert_eq!(spawn["error"],"Session has been deleted");
        assert_eq!(next_reply(&mut out_rx).await["id"],2);
        let state=pty_state.lock().unwrap();
        assert!(state.spawning.is_empty() && state.subs.is_empty());
        drop(state);
        for id in event_ids {ctx.app.unlisten(id);}
    }

}
