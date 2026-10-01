//! WebSocket bridge that multiplexes invokes, events, and PTY streams between browser and backend.
//!
//! Protocol (kept in sync with frontend `src/ipc/wsClient.ts`):
//! - Text frames are JSON control messages; binary PTY output is `[1-byte sid length][sid UTF-8][raw bytes]`.
//! - Client: {t:"invoke",id,cmd,args} / {t:"pty-spawn",id,sid,args} / {t:"pty-detach",sid,subId}
//!   / {t:"pong"} (heartbeat response) / {t:"caps",chat:1} / {t:"watch",name} / {t:"unwatch",name}
//!   / {t:"chat-resync",sid}
//! - Server: {t:"hello",source:"ws-N"} is the **first frame** after connection and identifies the source so
//!   the frontend can compare itself with Resized/SpawnResult.owner; followed by {t:"reply",id,ok,result|error},
//!   {t:"event",name,payload}, {t:"ping"} every 30 seconds, {t:"pty-resync",sid}, {t:"watch-rejected",name},
//!   and binary PTY frames.
//!
//! Chat channels: a client that sends `caps` with the chat codec version it decodes (see `chat_wire.rs`)
//! mirrors its local listeners with `watch`/`unwatch` of `chat://event/{sid}` and `chat://task/{sid}/{taskId}`.
//! The server then forwards a session's chat events exactly while it is watched, encoded as patches against
//! what this connection was sent last, and sends a task's workflow tree only while that task is watched. A
//! watch the server refuses (invalid name, outside a share grant, over the per-connection limit) is answered
//! with `watch-rejected`, so the view waiting for it can recover. A decoder that finds a gap sends
//! `chat-resync`; the server forgets its bases and queues the barrier event `{type:"resync"}` on that session's
//! channel, after which everything is whole again. A client that never
//! sends `caps` keeps the previous behaviour: chat events of every session it started or read through
//! `chat_start`/`chat_snapshot`, byte-identical to the engine's frames, until the connection closes.
//!
//! Outbound frames go through a per-connection [`Outbound`] queue with one writer (see `outbound.rs`): hello,
//! invoke replies and pings take a control lane that is always sent first; events, PTY frames and pty-spawn
//! replies share a bounded bulk lane. When one session's unsent terminal output grows past its cap, the
//! server drops it and sends {t:"pty-resync",sid}; the client resets that terminal and reattaches. A backlog
//! beyond the lane caps closes the connection with code 1013.
//!
//! Liveness: a connection with no inbound traffic for 2.5 heartbeat intervals is treated as half-open and
//! closed, unless its own outbound backlog is still draining (the pong may be stuck behind it on the peer's
//! side); a backlog that made no send progress for as long counts as dead as well.
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

use super::chat_wire::{self, ChatWire, WatchName};
use super::dispatch::{dispatch, CallOrigin};
use super::e2ee::{self, Cipher};
use super::outbound::{self, Class, Outbound, PtyPush};
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

/// Whether a connection counts as dead: silent inbound for [`LIVENESS_TIMEOUT`], and either nothing waiting
/// to be sent or no send progress for as long. A draining backlog keeps it alive, because the peer's pong
/// can only be as prompt as our ping, which may sit behind a slow link's backlog on the way out.
fn peer_presumed_dead(inbound_silence: Duration, backlogged: bool, since_progress: Duration) -> bool {
    inbound_silence >= LIVENESS_TIMEOUT && (!backlogged || since_progress >= LIVENESS_TIMEOUT)
}

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

    let outbound = Outbound::new(conn_source.clone());
    // Source ID for this connection. Each browser page has an independent WS, distinct from desktop.
    diagnostic.step("read");


    // The hello message is the **first application frame** and gives the frontend its source ID for comparing
    // against Resized/SpawnResult.owner in fit and mirror decisions. It is the first control-lane frame, and
    // the writer drains that lane first, guaranteeing delivery before replies, events, or PTY replay.
    outbound.push_control(
        Message::Text(json!({"t": "hello", "source": conn_source}).to_string()),
        Class::Hello,
    );

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

    // Writer task: serialize every outbound reply, event, and PTY binary frame to the socket, control lane
    // first. Encryption (E2EE) happens there; encryption or send failure, or an overflowed backlog, ends it.
    let mut writer = tokio::spawn(outbound::run_writer(outbound.clone(), ws_tx, cipher.clone()));

    let mut event_ids: Vec<ListenerId> = Vec::new();
    // PTY subscriptions owned by this connection: sid -> subscription ID, used for detach on disconnect.
    let mut pty_subs: HashMap<String, u64> = HashMap::new();
    // Sessions with status/exit forwarding already registered, preventing duplicates.
    let mut listened: HashSet<String> = HashSet::new();
    let mut chat = ChatLinks::default();

    // Register session-independent global forwarding. Share connections receive only the events the shared
    // session view consumes: tree refreshes, settings sync, and session state dots. Spawn/orchestration
    // requests, mirror layout, presence, clone progress, knowledge and usage stay host-private.
    let share_events: [&str; 3] = [
        crate::host::TREE_CHANGED,
        crate::host::SETTINGS_CHANGED,
        crate::session_state::STATE_EVENT,
    ];
    let full_events: [&str; 15] = [
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
        // UI mirror: the shared layout and the on/off switch both reach every client this way.
        crate::web::mirror::LAYOUT_EVENT,
        crate::web::mirror::MODE_EVENT,
        // How many clients are attached; a follower shows it next to its own mirror badge.
        crate::web::presence::CLIENTS_EVENT,
        // Account usage is polled once per machine; every client is told when that copy changes.
        crate::agent::usage_store::USAGE_EVENT,
        crate::agent::remote_model_catalog::EVENT,
    ];
    let global_events: &[&str] = if share.is_some() {
        &share_events
    } else {
        &full_events
    };
    for name in global_events {
        event_ids.push(listen_forward(&ctx.app, name, outbound.clone()));
    }

    // Process the plaintext application frame retained during first-frame inspection.
    if let Some(first) = pending_first.take() {
        handle_text(
            &ctx,
            &first,
            &conn_source,
            &outbound,
            &mut pty_subs,
            &mut listened,
            &mut chat,
            &mut event_ids,
            share.clone(),
        );
    }

    // Read loop and heartbeat timer. Any inbound message, including pong, refreshes liveness. Send a ping each
    // interval and close a connection silent beyond LIVENESS_TIMEOUT as half-open (see `peer_presumed_dead`).
    // The writer ending (send failure or overflow) ends the loop too. Normal cleanup then removes forwarding
    // listeners and detaches every subscription.
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
                            &outbound,
                            &mut pty_subs,
                            &mut listened,
                            &mut chat,
                            &mut event_ids,
                            share.clone(),
                        );
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            _ = &mut writer => break,
            _ = heartbeat.tick() => {
                let (backlogged, since_progress) = outbound.progress();
                if peer_presumed_dead(last_inbound.elapsed(), backlogged, since_progress) {
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
                outbound.log_stats();
                if !outbound.push_ping() {
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
    for (_, id) in chat.forwarders.drain().chain(chat.status.drain()) {
        ctx.app.unlisten(id);
    }
    // Deregister before detaching so the host stops showing this client the moment the socket is gone,
    // whether it closed cleanly, timed out as half-open, or was revoked mid-connection.
    ctx.app.emit(
        crate::web::presence::CLIENTS_EVENT,
        crate::web::presence::leave(&conn_source),
    );
    let mgr = ctx.app.pty();
    for (sid, sub) in pty_subs {
        mgr.detach(&ctx.app, &sid, sub, &conn_source);
    }
    outbound.close();
    outbound.log_stats();
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
    // The RAII guard covers the whole credential check; a cancelled handshake future (client
    // disconnect during the Argon2 await) releases the reservation on drop instead of leaking it.
    let Some(attempt) = ctx.limiter.allow(ip) else {
        if let Some(ct) = cipher.encrypt_text(&e2ee::err_msg("rate_limited")) {
            let _ = ws_tx.send(Message::Text(ct)).await;
        }
        return None;
    };
    let token_ok = ctx.auth.validate_pairing_token(&token);
    // Only a holder of the current pairing token may trigger the expensive Argon2 verify; the token check
    // is constant-time and cheap, so unauthenticated strangers cost the server nothing memory-hard.
    let pw_ok = if token_ok {
        match password.as_deref() {
            Some(p) => ctx.auth.verify_password_async(p).await,
            None => false,
        }
    } else {
        false
    };
    // Reject revoked devices even with a valid pairing token and password, blocking reconnect and re-pairing.
    // Without a self-reported device_id, device-level revocation is unavailable, matching registry placeholder behavior.
    let blocked = device_id
        .as_deref()
        .map(|id| ctx.auth.is_blocked(id))
        .unwrap_or(false);
    if token_ok && pw_ok && !blocked {
        attempt.success();
        // After both credentials pass and the device is not revoked, register its self-reported display identity.
        ctx.auth
            .register_device(device_id.as_deref(), device_name.as_deref());
        let ct = cipher.encrypt_text(e2ee::MSG_AUTHENTICATED)?;
        ws_tx.send(Message::Text(ct)).await.ok()?;
        // Return device_id so the main loop can detect revocation during later heartbeats, and the name
        // so the presence registry can show who just attached.
        Some((cipher, device_id, device_name, false))
    } else {
        attempt.failure();
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
    outbound: &Outbound,
    pty_subs: &mut HashMap<String, u64>,
    listened: &mut HashSet<String>,
    // Sessions whose chat channel this connection forwards, and its chat codec. Separate from `listened`: a
    // session can have both a PTY and a chat engine, registered at different moments.
    chat: &mut ChatLinks,
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
            // only register sessions their grant covers, so no other session's events reach them. A client that
            // negotiated the chat codec registers through `watch` instead.
            if matches!(cmd.as_str(), "chat_start" | "chat_snapshot") && !chat.negotiated() {
                if let Some(sid) = args.get("sessionId").and_then(Value::as_str) {
                    let covered = share
                        .as_ref()
                        .is_none_or(|scope| scope.covers_session(&ctx.app, sid));
                    // Bounded like watches: nothing ever unregisters these before the connection closes.
                    let room = chat.forwarders.len() < chat_wire::MAX_WATCHED_NAMES;
                    if covered && room && !chat.forwarders.contains_key(sid) {
                        chat.forwarders.insert(sid.to_string(), listen_forward(
                            &ctx.app,
                            &crate::agent::chat::engine::event_name(sid),
                            outbound.clone(),
                        ));
                        // Work state reaches the sidebar through the channel PTY sessions use, so a session
                        // that never spawns a PTY still needs it registered.
                        if listened.insert(sid.to_string()) {
                            event_ids.push(listen_forward(
                                &ctx.app,
                                &format!("pty://status/{sid}"),
                                outbound.clone(),
                            ));
                        }
                    }
                }
            }
            if matches!(cmd.as_str(), "pty_write" | "pty_resize") {
                // Keep frequent, lightweight keyboard and resize operations synchronous in the read loop for responsiveness.
                let reply =
                    match dispatch_scoped(&ctx.app, share.as_ref(), &cmd, &args, conn_source, origin) {
                        Ok(result) => json!({"t":"reply","id":id,"ok":true,"result":result}),
                        Err(e) => json!({"t":"reply","id":id,"ok":false,"error":e}),
                    };
                outbound.push_control(Message::Text(reply.to_string()), Class::Reply);
            } else {
                // Other commands commonly block on SQLite's global lock, subprocesses, or file reads. Move them
                // to spawn_blocking so one slow request cannot head-of-line block later messages or occupy a Tokio
                // worker and affect other connections. Replies return asynchronously and match by ID, not order.
                let ctx = ctx.clone();
                let outbound = outbound.clone();
                let conn_source = conn_source.to_string();
                let share = share.clone();
                let queued = std::time::Instant::now();
                tokio::task::spawn_blocking(move || {
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
                    let encoded = reply.to_string();
                    if cmd == "chat_snapshot" {
                        crate::diagnostics::record("INFO","chat_sync",serde_json::json!({"requestId":trace_request,"prepareMs":prepare_ms as u64,"encodeMs":encoding.elapsed().as_millis() as u64,"bytes":encoded.len(),"success":reply["ok"]}));
                    }
                    outbound.push_control(Message::Text(encoded), Class::Reply);
                });
            }
        }
        "pty-spawn" => {
            let id = msg.get("id").cloned().unwrap_or(Value::Null);
            if share.is_some() {
                // Share visitors never get a terminal, so no PTY may be started for them.
                outbound.push_bulk(
                    Message::Text(
                        json!({"t":"reply","id":id,"ok":false,"error":"Terminals are unavailable through a public share"}).to_string(),
                    ),
                    Class::PtyReply,
                );
                return;
            }
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
                // A chat watch of this session may already forward its status; the connection owns it now.
                let status = chat
                    .status
                    .remove(&sid)
                    .unwrap_or_else(|| listen_forward(&ctx.app, &format!("pty://status/{sid}"), outbound.clone()));
                event_ids.push(status);
                event_ids.push(listen_forward(
                    &ctx.app,
                    &format!("pty://exit/{sid}"),
                    outbound.clone(),
                ));
                // Notify this view when another client explicitly kills the session by closing or restarting it.
                event_ids.push(listen_forward(
                    &ctx.app,
                    &format!("pty://killed/{sid}"),
                    outbound.clone(),
                ));
            }
            // Serialize SpawnResult directly in camelCase to avoid maintaining parallel handwritten JSON. This
            // automatically includes cols/rows/owner, the initial-size channel used by mirror mode.
            let reply =
                match web_pty_spawn(ctx, &sid, &args, conn_source, attach_only, outbound.clone()) {
                    Ok(res) => {
                        pty_subs.insert(sid.clone(), res.sub_id);
                        let result = serde_json::to_value(&res).unwrap_or(Value::Null);
                        json!({"t":"reply","id":id,"ok":true,"result":result})
                    }
                    Err(e) => json!({"t":"reply","id":id,"ok":false,"error":e}),
                };
            // Bulk lane, not control: the reply must follow the attach's replay bytes (the client's replay gate).
            outbound.push_bulk(Message::Text(reply.to_string()), Class::PtyReply);
        }
        "pty-detach" => {
            let sid = msg.get("sid").and_then(Value::as_str).unwrap_or("");
            if let Some(sub) = msg.get("subId").and_then(Value::as_u64) {
                ctx.app.pty().detach(&ctx.app, sid, sub, conn_source);
            }
            pty_subs.remove(sid);
        }
        "caps" => {
            if msg.get("chat").and_then(Value::as_u64) == Some(chat_wire::PROTOCOL_VERSION) {
                chat.lock().enable();
            }
        }
        "watch" | "unwatch" if chat.negotiated() => {
            let watch = msg["t"] == "watch";
            let raw = msg.get("name").and_then(Value::as_str).unwrap_or("");
            let Some(name) = chat_wire::parse_name(raw) else {
                if chat.first_rejection() {
                    crate::diagnostic_warn!("[ws] ignored a watch frame with an invalid name");
                }
                if watch {
                    reject_watch(outbound, raw);
                }
                return;
            };
            // Share clients only watch sessions their grant covers; leaving needs no check.
            if watch && share.as_ref().is_some_and(|scope| !scope.covers_session(&ctx.app, name.session())) {
                reject_watch(outbound, raw);
                return;
            }
            if watch && chat.lock().over_limit(&name) {
                if chat.first_rejection() {
                    crate::diagnostic_warn!("[ws] ignored a watch beyond the per-connection limit");
                }
                reject_watch(outbound, raw);
                return;
            }
            match (watch, name) {
                (true, WatchName::Session(sid)) => watch_session(ctx, &sid, outbound, listened, chat),
                (false, WatchName::Session(sid)) => {
                    chat.lock().unwatch_session(&sid);
                    for id in [chat.forwarders.remove(&sid), chat.status.remove(&sid)].into_iter().flatten() {
                        ctx.app.unlisten(id);
                    }
                }
                (true, WatchName::Task(sid, task)) => {
                    let sent = {
                        let mut wire = chat.lock();
                        if !wire.watch_task(&sid, &task) {
                            return;
                        }
                        wire.extras_sent(&sid)
                    };
                    // Send the tree to this connection alone. Publishing through the engine would reach every
                    // listener of the session, so a watch/unwatch loop could make one client flood the others.
                    // Read outside the codec lock: the engine emits while holding its own locks, and its
                    // forwarders take the codec lock. An extras frame encoded in between already carried the
                    // tree, and this older read must not overwrite it.
                    let Some(extras) = ctx.app.chat().published_extras(&sid) else { return };
                    let mut wire = chat.lock();
                    if sent.is_some() && wire.extras_sent(&sid) == sent {
                        if let Some(encoded) = wire.encode(&sid, json!({"type":"extras","extras":extras})) {
                            outbound.push_event_exact(&crate::agent::chat::engine::event_name(&sid), encoded);
                        }
                    }
                }
                (false, WatchName::Task(sid, task)) => {
                    chat.lock().unwatch_task(&sid, &task);
                }
            }
        }
        "chat-resync" if chat.negotiated() => {
            let sid = msg.get("sid").and_then(Value::as_str).unwrap_or("");
            if !chat_wire::valid_id(sid) {
                return;
            }
            // Under the codec lock, so the barrier lands after every frame encoded against the old bases
            // and before any whole frame encoded after them.
            let mut wire = chat.lock();
            if wire.resync(sid) {
                outbound.push_event_exact(&crate::agent::chat::engine::event_name(sid), json!({"type":"resync"}));
            }
        }
        _ => {}
    }
}

/// One connection's chat subscriptions.
#[derive(Default)]
struct ChatLinks {
    /// The chat codec, shared with this connection's forwarders.
    wire: Arc<Mutex<ChatWire>>,
    /// sid -> forwarder of `chat://event/{sid}`.
    forwarders: HashMap<String, ListenerId>,
    /// sid -> forwarder of `pty://status/{sid}` that a watch registered; it ends with the watch.
    status: HashMap<String, ListenerId>,
    /// An invalid or excess watch was already logged for this connection.
    warned: bool,
}

impl ChatLinks {
    fn lock(&self) -> std::sync::MutexGuard<'_, ChatWire> {
        self.wire.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn negotiated(&self) -> bool {
        self.lock().enabled()
    }

    /// Whether this is the first rejected watch of the connection; only that one is logged.
    fn first_rejection(&mut self) -> bool {
        !std::mem::replace(&mut self.warned, true)
    }
}

/// Tell the client a watch will not be served (invalid name, outside a share grant, or over the limit), so a
/// view waiting for that name can recover instead of waiting forever. Names too long to be valid are not
/// echoed back.
fn reject_watch(outbound: &Outbound, name: &str) {
    if name.len() <= chat_wire::MAX_NAME_LEN {
        outbound.push_control(Message::Text(json!({"t":"watch-rejected","name":name}).to_string()), Class::Reply);
    }
}

/// Start forwarding one session's chat channel through the codec. A repeated watch is a no-op; the caller has
/// checked the per-connection limit.
fn watch_session(ctx: &Ctx, sid: &str, outbound: &Outbound, listened: &HashSet<String>, chat: &mut ChatLinks) {
    let Some(generation) = chat.lock().watch_session(sid) else { return };
    // A forwarder registered before the client negotiated sends whole frames; the codec one replaces it.
    if let Some(old) = chat.forwarders.remove(sid) {
        ctx.app.unlisten(old);
    }
    chat.forwarders.insert(sid.to_string(), chat_forward(&ctx.app, sid, generation, chat.wire.clone(), outbound.clone()));
    // Work state reaches the conversation view through the channel PTY sessions use, so a session that never
    // spawns a PTY still needs it registered. It belongs to this watch and ends with it, so a watch/unwatch
    // loop cannot pile up listeners; a session whose terminal this connection attached already has one.
    if !listened.contains(sid) && !chat.status.contains_key(sid) {
        chat.status.insert(sid.to_string(), listen_forward(&ctx.app, &format!("pty://status/{sid}"), outbound.clone()));
    }
}

/// Registers a session's chat forwarding through the codec: each event is encoded and queued under the codec
/// lock, so patches leave in the order they were computed, and never coalesced. An event of a watch that has
/// since ended (the bus may still be running this handler on another thread) is dropped.
fn chat_forward(app: &AppCtx, sid: &str, generation: u64, wire: Arc<Mutex<ChatWire>>, outbound: Outbound) -> ListenerId {
    let name = crate::agent::chat::engine::event_name(sid);
    let sid = sid.to_string();
    app.listen(&name.clone(), move |payload| {
        let payload: Value = serde_json::from_str(payload).unwrap_or(Value::Null);
        let mut wire = wire.lock().unwrap_or_else(|e| e.into_inner());
        if !wire.is_current(&sid, generation) {
            return;
        }
        if let Some(encoded) = wire.encode(&sid, payload) {
            outbound.push_event_exact(&name, encoded);
        }
    })
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

/// Registers forwarding for one event name, queueing `{t:"event",name,payload}` on the bulk lane.
fn listen_forward(app: &AppCtx, name: &str, outbound: Outbound) -> ListenerId {
    let name_owned = name.to_string();
    app.listen(name, move |payload| {
        let payload: Value = serde_json::from_str(payload).unwrap_or(Value::Null);
        outbound.push_event(&name_owned, payload);
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
    outbound: Outbound,
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

    let sink = pty_sink(outbound, sid);

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

/// WS output sink that wraps bytes as [len][sid][payload] binary frames on the bulk lane. It returns false,
/// and keeps returning false, once the connection closes or the session's backlog was replaced by a resync,
/// so the PTY fan-out removes it; the client reattaches with a fresh sink after the resync marker.
fn pty_sink(outbound: Outbound, sid: &str) -> OutputSink {
    let sid_owned = sid.to_string();
    let sid_bytes = sid.as_bytes().to_vec();
    let stopped = std::sync::atomic::AtomicBool::new(false);
    Box::new(move |bytes: &[u8]| {
        if stopped.load(Ordering::Relaxed) {
            return false;
        }
        let mut frame = Vec::with_capacity(1 + sid_bytes.len() + bytes.len());
        frame.push(sid_bytes.len() as u8);
        frame.extend_from_slice(&sid_bytes);
        frame.extend_from_slice(bytes);
        match outbound.push_pty(&sid_owned, frame) {
            PtyPush::Queued => true,
            PtyPush::Resync | PtyPush::Closed => {
                stopped.store(true, Ordering::Relaxed);
                false
            }
        }
    })
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
        let outbound = Outbound::new("ws-test");
        let mut pty_subs = HashMap::new();
        let mut listened = HashSet::new();
        let mut chat = ChatLinks::default();
        let mut event_ids = Vec::new();
        let frame = json!({"t": "invoke", "id": 1, "cmd": cmd, "args": {}}).to_string();
        handle_text(
            ctx,
            &frame,
            "ws-test",
            &outbound,
            &mut pty_subs,
            &mut listened,
            &mut chat,
            &mut event_ids,
            None,
        );
        match outbound.next().await.expect("expected a reply frame") {
            outbound::Next::Frame(Message::Text(t), _, Class::Reply) => {
                serde_json::from_str(&t).expect("reply must be JSON")
            }
            other => panic!("expected a text reply on the control lane, got: {other:?}"),
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

    /// T6: silence alone kills a connection only when our own backlog is not still draining.
    #[test]
    fn liveness_spares_a_connection_whose_backlog_is_draining() {
        let s = Duration::from_secs;
        assert!(!peer_presumed_dead(s(80), true, s(2)), "a draining backlog keeps the connection");
        assert!(peer_presumed_dead(s(80), true, s(80)), "a backlog without progress is a dead peer");
        assert!(peer_presumed_dead(s(80), false, s(0)), "silence without backlog is a dead peer, as before");
        assert!(!peer_presumed_dead(s(10), true, s(80)), "recent inbound traffic always keeps it");
        assert!(!peer_presumed_dead(s(10), false, s(0)));
    }

    /// T5 against the real PTY fan-out: once one session's unsent output passes the threshold, the WS sink
    /// refuses the chunk and the output stream drops it (without a detach), a resync marker is queued, and a
    /// fresh attach replaying a full ring does not trip it again.
    #[tokio::test]
    async fn a_flooding_terminal_is_resynced_and_a_fresh_attach_replays_without_looping() {
        use crate::pty::session::OutputStream;
        let outbound = Outbound::new("ws-test");
        let mut stream = OutputStream::new(crate::pty::manager::SCROLLBACK_CAP);
        stream.attach(1, pty_sink(outbound.clone(), "sa"));
        let chunk = vec![b'x'; 64 * 1024];
        let mut ingested = 0;
        while stream.subscriber_count() == 1 {
            stream.ingest(&chunk);
            ingested += chunk.len();
            assert!(ingested <= 2 * outbound::PTY_RESYNC_BYTES, "the sink must refuse eventually");
        }
        // Frames carry a small sid header, so the backlog passes the threshold at about this many payload bytes.
        assert!(ingested >= outbound::PTY_RESYNC_BYTES - chunk.len());
        assert_eq!(outbound.take_stats()["resyncCount"], 1);

        // The client's reattach: a new sink replays the prelude and the full ring.
        stream.attach(2, pty_sink(outbound.clone(), "sa"));
        stream.ingest(b"after");
        assert_eq!(stream.subscriber_count(), 1, "a replay of one ring must not trigger another resync");
        assert_eq!(outbound.take_stats()["resyncCount"], 0);

        outbound.close();
        let mut kinds = Vec::new();
        while let Some(outbound::Next::Frame(msg, _, class)) = outbound.next().await {
            kinds.push(class);
            if class == Class::PtyResync {
                assert_eq!(msg, Message::Text(json!({"t":"pty-resync","sid":"sa"}).to_string()));
            }
        }
        let marker = kinds.iter().position(|c| *c == Class::PtyResync).expect("a resync marker");
        assert_eq!(kinds.iter().filter(|c| **c == Class::PtyResync).count(), 1);
        assert_eq!(kinds[..marker].iter().filter(|c| **c == Class::PtyOutput).count(), 0, "stale output was dropped");
        assert!(kinds[marker + 1..].iter().all(|c| *c == Class::PtyOutput), "only the replay follows the marker");
        assert!(kinds.len() > marker + 1);
    }
    /// One simulated connection: the real frame handler, forwarders, codec and outbound queue.
    struct TestConn {
        outbound: Outbound,
        pty_subs: HashMap<String, u64>,
        listened: HashSet<String>,
        chat: ChatLinks,
        event_ids: Vec<ListenerId>,
        share: Option<super::super::share_policy::ShareScope>,
        sent: Arc<std::sync::Mutex<Vec<Message>>>,
        writer: Option<tokio::task::JoinHandle<()>>,
    }

    impl TestConn {
        fn new(id: &str) -> Self {
            Self {
                outbound: Outbound::new(id),
                pty_subs: HashMap::new(),
                listened: HashSet::new(),
                chat: ChatLinks::default(),
                event_ids: Vec::new(),
                share: None,
                sent: Arc::new(std::sync::Mutex::new(Vec::new())),
                writer: None,
            }
        }

        /// Run the real writer into a recording sink from now on, as a live socket drains the queue.
        fn start_writer(&mut self) {
            let sink = Box::pin(futures_util::sink::unfold(self.sent.clone(), |out, msg: Message| async move {
                out.lock().unwrap().push(msg);
                Ok::<_, ()>(out)
            }));
            self.writer = Some(tokio::spawn(outbound::run_writer(self.outbound.clone(), sink, None)));
        }

        /// A client that negotiated the codec and watches `names`.
        fn negotiated(ctx: &Ctx, id: &str, names: &[&str]) -> Self {
            let mut conn = Self::new(id);
            conn.send(ctx, json!({"t":"caps","chat":chat_wire::PROTOCOL_VERSION}));
            for name in names {
                conn.send(ctx, json!({"t":"watch","name":name}));
            }
            conn
        }

        fn send(&mut self, ctx: &Ctx, frame: Value) {
            handle_text(
                ctx,
                &frame.to_string(),
                "ws-test",
                &self.outbound,
                &mut self.pty_subs,
                &mut self.listened,
                &mut self.chat,
                &mut self.event_ids,
                self.share.clone(),
            );
        }

        fn queued_frames(&self) -> u64 {
            self.outbound.take_stats()["queuedFrames"].as_u64().unwrap()
        }

        /// Close the queue and run the real writer into a recording sink. Returns the text frames sent and
        /// the counters (`chatRows`, `chatExtras`, ...) the `ws_outbound` line would report.
        async fn drain(mut self) -> (Vec<String>, Value) {
            if self.writer.is_none() {
                self.start_writer();
            }
            self.outbound.close();
            self.writer.take().unwrap().await.unwrap();
            let texts = self.sent
                .lock()
                .unwrap()
                .iter()
                .filter_map(|m| match m {
                    Message::Text(t) => Some(t.clone()),
                    _ => None,
                })
                .collect();
            (texts, self.outbound.take_stats())
        }
    }

    fn class(stats: &Value, name: &str, field: &str) -> u64 {
        stats["classes"][name][field].as_u64().unwrap_or(0)
    }

    /// Chat event payloads of `sid` among the sent frames, in order.
    fn chat_payloads(texts: &[String], sid: &str) -> Vec<Value> {
        let name = crate::agent::chat::engine::event_name(sid);
        texts
            .iter()
            .map(|t| serde_json::from_str::<Value>(t).unwrap())
            .filter(|v| v["t"] == "event" && v["name"] == name.as_str())
            .map(|v| v["payload"].clone())
            .collect()
    }

    struct StreamRun {
        negotiated: u64,
        legacy: u64,
        flushes: usize,
    }

    /// Stream an answer of `total` bytes through the real engine path to one negotiated and one legacy
    /// connection, and check that the negotiated client decodes exactly the streamed text.
    async fn stream_through_both(total: usize) -> StreamRun {
        let ctx = test_ctx(ServeMode::LoopbackHttp);
        let mut negotiated = TestConn::negotiated(&ctx, "ws-new", &["chat://event/s"]);
        let mut legacy = TestConn::new("ws-old");
        legacy.send(&ctx, json!({"t":"invoke","id":1,"cmd":"chat_snapshot","args":{"sessionId":"s"}}));
        // Both sockets drain while the answer streams, so the quadratic legacy backlog stays under its cap.
        negotiated.start_writer();
        legacy.start_writer();
        let app = ctx.app.clone();
        let (text, flushes) = tokio::task::spawn_blocking(move || {
            crate::agent::chat::engine::test_support::stream_answer(&app, "s", total, 64, 8)
        })
        .await
        .unwrap();
        let (texts, stats) = negotiated.drain().await;
        let mut decoder = chat_wire::reference::Decoder::default();
        let mut last = String::new();
        for payload in chat_payloads(&texts, "s") {
            let decoded = decoder.decode(&payload).expect("every frame decodes");
            if let Some(row) = decoded["rows"].as_array().and_then(|rows| rows.last()) {
                last = row["text"].as_str().unwrap().to_string();
            }
        }
        assert_eq!(last, text, "the decoded answer is the streamed answer");
        let (_, legacy_stats) = legacy.drain().await;
        StreamRun {
            negotiated: class(&stats, "chatRows", "bytes"),
            legacy: class(&legacy_stats, "chatRows", "bytes"),
            flushes,
        }
    }

    /// AC1: streaming an answer costs O(N) chatRows bytes on a negotiated connection, while the legacy
    /// connection in the same run keeps the quadratic cost.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ac1_a_streamed_answer_costs_linear_bytes_on_a_negotiated_connection() {
        let n = 64 * 1024;
        let small = stream_through_both(n).await;
        assert!(
            small.negotiated as usize <= 2 * n + 256 * small.flushes,
            "negotiated: {} bytes for {n} bytes in {} flushes",
            small.negotiated,
            small.flushes
        );
        assert!(small.legacy >= 10 * small.negotiated, "legacy {} vs negotiated {}", small.legacy, small.negotiated);
        let large = stream_through_both(2 * n).await;
        eprintln!(
            "chatRows bytes: N={n}: negotiated {} legacy {} ({} flushes); N={}: negotiated {} legacy {} ({} flushes)",
            small.negotiated, small.legacy, small.flushes, 2 * n, large.negotiated, large.legacy, large.flushes
        );
        assert!(
            large.negotiated as f64 <= 2.2 * small.negotiated as f64,
            "doubling N took {} -> {} bytes",
            small.negotiated,
            large.negotiated
        );
        assert!(
            large.legacy as f64 >= 3.5 * small.legacy as f64,
            "the legacy path stays quadratic: {} -> {}",
            small.legacy,
            large.legacy
        );
    }

    fn workflow_extras(workflows: usize, agents: usize) -> Value {
        let tasks = crate::agent::chat::engine::test_support::workflow_tasks(workflows, agents);
        json!({"type":"extras","extras":{"fastMode":false,"backgroundTasks":tasks}})
    }

    /// AC2: after unwatch, rows and extras of that session produce no frame at all on the connection.
    #[tokio::test]
    async fn ac2_an_unwatched_session_costs_nothing() {
        let ctx = test_ctx(ServeMode::LoopbackHttp);
        let mut conn = TestConn::negotiated(&ctx, "ws-2", &["chat://event/S"]);
        let name = crate::agent::chat::engine::event_name("S");
        let row = |i: usize| json!({"type":"rows","epoch":1,"revision":i,"positions":{},"rows":[{"kind":"assistant","id":"r","text":"x".repeat(i + 1),"streaming":true}]});
        ctx.app.emit(&name, row(0));
        assert_eq!(conn.queued_frames(), 1, "a watched session is forwarded");
        conn.send(&ctx, json!({"t":"unwatch","name":"chat://event/S"}));
        assert!(conn.chat.forwarders.is_empty(), "the forwarder is gone");
        let extras = workflow_extras(6, 20);
        for i in 0..100 {
            ctx.app.emit(&name, row(i + 1));
            ctx.app.emit(&name, extras.clone());
        }
        assert_eq!(conn.queued_frames(), 1, "nothing was queued after unwatch");
        let (_, stats) = conn.drain().await;
        assert_eq!(class(&stats, "chatRows", "frames"), 1, "only the frame from before the unwatch");
        assert_eq!(class(&stats, "chatExtras", "frames"), 0);
    }

    /// AC3/AC4: six running workflows travel as a compact status, clock-only and identical publishes send
    /// nothing, a token change is a small patch, and opening one task sends its tree once, then changes.
    #[tokio::test]
    async fn ac3_background_workflows_travel_as_a_compact_status_with_details_on_demand() {
        use crate::agent::chat::engine::test_support::{flush_extras, publish_extras, workflow_tasks};
        let ctx = test_ctx(ServeMode::LoopbackHttp);
        ctx.app.chat().insert_test_process("S");
        ctx.app.chat().update_test_extras("S", |extras| extras.background_tasks = workflow_tasks(6, 20));
        let mut conn = TestConn::negotiated(&ctx, "ws-3", &["chat://event/S"]);
        publish_extras(&ctx.app, "S");
        assert_eq!(conn.queued_frames(), 1);
        for _ in 0..200 {
            publish_extras(&ctx.app, "S");
        }
        assert_eq!(conn.queued_frames(), 1, "clock-only or identical publishes send nothing");
        ctx.app.chat().update_test_extras("S", |extras| {
            extras.background_tasks[2].usage = Some(json!({"total_tokens": 6100, "tool_uses": 13}));
        });
        publish_extras(&ctx.app, "S");
        assert_eq!(conn.queued_frames(), 2);

        conn.send(&ctx, json!({"t":"watch","name":"chat://task/S/task3"}));
        assert_eq!(conn.queued_frames(), 3, "exactly one frame with the tree");
        assert!(!flush_extras(&ctx.app, "S"), "the watch published nothing to the session's other listeners");
        ctx.app.chat().update_test_extras("S", |extras| {
            let tree = extras.background_tasks[3].workflow_progress.as_mut().unwrap();
            tree[6]["tokens"] = json!(99_999);
        });
        publish_extras(&ctx.app, "S");

        let (texts, _) = conn.drain().await;
        let frames: Vec<&String> = texts.iter().filter(|t| t.contains("\"type\":\"extras\"")).collect();
        assert_eq!(frames.len(), 4);
        let payloads = chat_payloads(&texts, "S");
        let first = &payloads[0]["extras"]["backgroundTasks"];
        assert_eq!(first.as_array().unwrap().len(), 6);
        for task in first.as_array().unwrap() {
            assert!(task.get("workflow_progress").is_none());
            assert_eq!(task["detail_omitted"], true);
        }
        assert!(!frames[0].contains("workflow_progress"));
        assert!(frames[1].len() <= 512, "a token change cost {} bytes: {}", frames[1].len(), frames[1]);
        let detail = &payloads[2]["patch"]["arr"]["backgroundTasks"]["items"]["task3"];
        assert_eq!(detail["set"]["workflow_progress"].as_array().unwrap().len(), 21, "task3's whole tree");
        assert_eq!(detail["del"], json!(["detail_omitted"]));
        let items = payloads[2]["patch"]["arr"]["backgroundTasks"]["items"].as_object().unwrap();
        for (task, patch) in items {
            assert!(task == "task3" || chat_wire::is_clock_only(patch), "{task}: {patch}");
        }
        let tree = &payloads[3]["patch"]["arr"]["backgroundTasks"]["items"]["task3"]["arr"]["workflow_progress"];
        assert!(tree.get("order").is_none());
        for (entry, patch) in tree["items"].as_object().unwrap() {
            if entry == "w3a5" {
                assert_eq!(patch["set"]["tokens"], 99_999);
            } else {
                assert!(chat_wire::is_clock_only(patch), "{entry} changed: {patch}");
            }
        }
        let mut decoder = chat_wire::reference::Decoder::default();
        let decoded: Vec<Value> = payloads.iter().map(|p| decoder.decode(p).unwrap()).collect();
        assert_eq!(decoded[3]["extras"]["backgroundTasks"][3]["workflow_progress"][6]["tokens"], 99_999);
    }

    /// AC4: an extras message republished unchanged produces no frame.
    #[tokio::test]
    async fn ac4_an_unchanged_republish_is_silent() {
        let ctx = test_ctx(ServeMode::LoopbackHttp);
        let conn = TestConn::negotiated(&ctx, "ws-4", &["chat://event/S"]);
        let name = crate::agent::chat::engine::event_name("S");
        let extras = workflow_extras(6, 20);
        for _ in 0..50 {
            ctx.app.emit(&name, extras.clone());
        }
        let (_, stats) = conn.drain().await;
        assert_eq!(class(&stats, "chatExtras", "frames"), 1);
    }

    /// AC6: a connection that never negotiates receives byte-identical frames to the baseline forwarding,
    /// extras coalescing included; a negotiated one that does not watch receives no chat events at all.
    #[tokio::test]
    async fn ac6_an_unnegotiated_connection_gets_the_baseline_frames() {
        let ctx = test_ctx(ServeMode::LoopbackHttp);
        let mut legacy = TestConn::new("ws-old");
        legacy.send(&ctx, json!({"t":"invoke","id":1,"cmd":"chat_start","args":{"sessionId":"s"}}));
        let mut quiet = TestConn::negotiated(&ctx, "ws-new", &[]);
        quiet.send(&ctx, json!({"t":"invoke","id":2,"cmd":"chat_snapshot","args":{"sessionId":"s"}}));
        let baseline = Outbound::new("ws-baseline");
        let name = crate::agent::chat::engine::event_name("s");
        let sequence = vec![
            json!({"type":"rows","epoch":1,"revision":1,"positions":{"r":0},"rows":[{"kind":"assistant","id":"r","text":"a","streaming":true}]}),
            json!({"type":"rows","epoch":1,"revision":2,"positions":{"r":0},"rows":[{"kind":"assistant","id":"r","text":"ab","streaming":true}]}),
            workflow_extras(2, 3),
            workflow_extras(3, 3),
            json!({"type":"queued","items":[],"revision":1,"epoch":1}),
            json!({"type":"reset","epoch":2,"revision":1,"rows":[]}),
            workflow_extras(1, 1),
            workflow_extras(1, 2),
        ];
        for payload in &sequence {
            ctx.app.emit(&name, payload.clone());
            // The baseline path: listen_forward parses the bus payload and queues it with push_event.
            baseline.push_event(&name, serde_json::from_str(&payload.to_string()).unwrap());
        }
        let chat = |texts: Vec<String>| -> Vec<String> { texts.into_iter().filter(|t| t.contains("chat://event/s")).collect() };
        let listened = legacy.listened.clone();
        let (legacy_texts, legacy_stats) = legacy.drain().await;
        let (baseline_texts, baseline_stats) = TestConn { outbound: baseline, ..TestConn::new("unused") }.drain().await;
        assert_eq!(chat(legacy_texts), chat(baseline_texts));
        assert_eq!(legacy_stats["coalescedCount"], baseline_stats["coalescedCount"], "extras coalesce as before");
        assert!(legacy_stats["coalescedCount"].as_u64().unwrap() >= 2);
        let (quiet_texts, _) = quiet.drain().await;
        assert!(chat(quiet_texts).is_empty(), "a negotiated client registers through watch only");
        assert!(listened.contains("s"), "legacy registration still forwards the session's work state");
    }

    /// A task watch sends the tree to the watching connection alone: a watch/unwatch loop costs the session's
    /// other listeners (desktop IPC, legacy and negotiated connections) nothing.
    #[tokio::test]
    async fn a_task_watch_loop_cannot_flood_other_listeners() {
        use crate::agent::chat::engine::test_support::{flush_extras, workflow_tasks};
        let ctx = test_ctx(ServeMode::LoopbackHttp);
        ctx.app.chat().insert_test_process("S");
        ctx.app.chat().update_test_extras("S", |extras| extras.background_tasks = workflow_tasks(2, 3));
        let bus = Arc::new(std::sync::Mutex::new(0));
        let sink = bus.clone();
        let desktop = ctx.app.listen(&crate::agent::chat::engine::event_name("S"), move |_| *sink.lock().unwrap() += 1);
        let other = TestConn::negotiated(&ctx, "ws-other", &["chat://event/S"]);
        let mut conn = TestConn::negotiated(&ctx, "ws-loop", &["chat://event/S"]);
        for _ in 0..50 {
            conn.send(&ctx, json!({"t":"watch","name":"chat://task/S/task1"}));
            assert!(!flush_extras(&ctx.app, "S"), "nothing is left for the engine to publish");
            conn.send(&ctx, json!({"t":"unwatch","name":"chat://task/S/task1"}));
        }
        ctx.app.unlisten(desktop);
        assert_eq!(*bus.lock().unwrap(), 0, "no event reached the session's bus");
        assert_eq!(other.queued_frames(), 0);
        let (texts, _) = conn.drain().await;
        let payloads = chat_payloads(&texts, "S");
        // The first watch sends the tree; repeating it changes nothing this connection was sent, so it costs nothing.
        assert_eq!(payloads.len(), 1, "only the watching connection receives the tree, once");
        let mut decoder = chat_wire::reference::Decoder::default();
        let first = decoder.decode(&payloads[0]).unwrap();
        assert_eq!(first["extras"]["backgroundTasks"][1]["workflow_progress"].as_array().unwrap().len(), 4, "task1's tree");
        assert_eq!(first["extras"]["backgroundTasks"][0]["detail_omitted"], true);
    }

    /// A watch the server refuses is answered, so the client can stop waiting for it; accepted and repeated
    /// ones are not, and names too long to be valid are not echoed (share refusals: see the grant test).
    #[tokio::test]
    async fn a_refused_watch_is_answered() {
        let ctx = test_ctx(ServeMode::LoopbackHttp);
        let mut conn = TestConn::negotiated(&ctx, "ws-refused", &["chat://event/s0"]);
        conn.send(&ctx, json!({"t":"watch","name":"chat://task/s0/bad id"}));
        conn.send(&ctx, json!({"t":"unwatch","name":"chat://task/s0/bad id"}));
        conn.send(&ctx, json!({"t":"watch","name":format!("chat://task/s0/{}", "x".repeat(chat_wire::MAX_NAME_LEN))}));
        for i in 1..chat_wire::MAX_WATCHED_NAMES {
            conn.send(&ctx, json!({"t":"watch","name":format!("chat://task/s0/t{i}")}));
        }
        conn.send(&ctx, json!({"t":"watch","name":"chat://task/s0/over"}));
        conn.send(&ctx, json!({"t":"watch","name":"chat://event/over"}));
        conn.send(&ctx, json!({"t":"watch","name":"chat://event/s0"}));
        for (_, id) in conn.chat.forwarders.drain().chain(conn.chat.status.drain()) {
            ctx.app.unlisten(id);
        }
        let (texts, _) = conn.drain().await;
        let rejected: Vec<Value> = texts
            .iter()
            .map(|t| serde_json::from_str::<Value>(t).unwrap())
            .filter(|v| v["t"] == "watch-rejected")
            .map(|v| v["name"].clone())
            .collect();
        assert_eq!(rejected, vec![json!("chat://task/s0/bad id"), json!("chat://task/s0/over"), json!("chat://event/over")]);
    }

    /// The status forwarder a watch registers ends with the watch, so a watch/unwatch loop over ever new
    /// sessions keeps the connection's bus listeners bounded; a terminal attach takes it over.
    #[test]
    fn watch_status_forwarders_end_with_the_watch() {
        let ctx = test_ctx(ServeMode::LoopbackHttp);
        let mut conn = TestConn::negotiated(&ctx, "ws-status", &[]);
        for i in 0..1000 {
            conn.send(&ctx, json!({"t":"watch","name":format!("chat://event/s{i}")}));
            conn.send(&ctx, json!({"t":"unwatch","name":format!("chat://event/s{i}")}));
        }
        assert!(conn.chat.status.is_empty() && conn.chat.forwarders.is_empty() && conn.event_ids.is_empty());
        ctx.app.emit("pty://status/s7", json!({"kind":"agent"}));
        assert_eq!(conn.queued_frames(), 0, "an ended watch forwards no status");

        conn.send(&ctx, json!({"t":"watch","name":"chat://event/live"}));
        assert_eq!(conn.chat.status.len(), 1);
        ctx.app.emit("pty://status/live", json!({"kind":"agent"}));
        assert_eq!(conn.queued_frames(), 1, "a watched session forwards its status once");
        let before = conn.event_ids.len();
        conn.send(&ctx, json!({"t":"pty-spawn","id":1,"sid":"live","args":{},"attachOnly":true}));
        assert!(conn.chat.status.is_empty(), "the attach owns the status forwarder now");
        assert_eq!(conn.event_ids.len(), before + 3, "status (taken over), exit and killed");
        conn.send(&ctx, json!({"t":"unwatch","name":"chat://event/live"}));
        let queued = conn.queued_frames();
        ctx.app.emit("pty://status/live", json!({"kind":"agent"}));
        assert_eq!(conn.queued_frames(), queued + 1, "the attached terminal keeps its status, exactly once");
        for id in conn.event_ids.drain(..) {
            ctx.app.unlisten(id);
        }
    }

    /// A share client cannot watch a session outside its grant, and invalid names are ignored.
    #[tokio::test]
    async fn watch_is_scoped_to_the_share_grant_and_validated() {
        let ctx = test_ctx(ServeMode::ShareTunnel);
        let mut conn = TestConn::new("ws-share");
        conn.share = Some(super::super::share_policy::ShareScope {
            scope: "session".into(),
            target_id: Some("granted".into()),
            push_authority: None,
        });
        conn.send(&ctx, json!({"t":"caps","chat":1}));
        conn.send(&ctx, json!({"t":"watch","name":"chat://event/other"}));
        conn.send(&ctx, json!({"t":"watch","name":"chat://task/other/t1"}));
        conn.send(&ctx, json!({"t":"watch","name":"chat://event/../etc"}));
        assert!(conn.chat.forwarders.is_empty());
        assert_eq!(conn.chat.lock().watched_count(), 0);
        ctx.app.emit(&crate::agent::chat::engine::event_name("other"), json!({"type":"queued","items":[]}));
        assert_eq!(conn.queued_frames(), 3, "only the three refusals");
        let (texts, _) = conn.drain().await;
        let refused = ["chat://event/other", "chat://task/other/t1", "chat://event/../etc"];
        assert_eq!(texts, refused.map(|name| json!({"t":"watch-rejected","name":name}).to_string()));
    }

    /// A connection holds at most MAX_WATCHED_NAMES watches; caps with an unknown version negotiates nothing.
    #[test]
    fn watches_are_bounded_per_connection() {
        let ctx = test_ctx(ServeMode::LoopbackHttp);
        let mut conn = TestConn::new("ws-many");
        conn.send(&ctx, json!({"t":"caps","chat":99}));
        conn.send(&ctx, json!({"t":"watch","name":"chat://event/a"}));
        assert!(!conn.chat.negotiated() && conn.chat.forwarders.is_empty(), "an unknown codec version is ignored");
        conn.send(&ctx, json!({"t":"caps","chat":1}));
        for i in 0..chat_wire::MAX_WATCHED_NAMES + 20 {
            conn.send(&ctx, json!({"t":"watch","name":format!("chat://event/s{i}")}));
            conn.send(&ctx, json!({"t":"watch","name":format!("chat://task/s{i}/t")}));
        }
        assert_eq!(conn.chat.lock().watched_count(), chat_wire::MAX_WATCHED_NAMES);
        for (_, id) in conn.chat.forwarders.drain() {
            ctx.app.unlisten(id);
        }
    }

    /// AC7 (server side): a resync request queues the barrier behind every frame encoded before it, and
    /// everything after it is whole again.
    #[tokio::test]
    async fn a_resync_barrier_follows_earlier_frames_and_precedes_whole_ones() {
        let ctx = test_ctx(ServeMode::LoopbackHttp);
        let mut conn = TestConn::negotiated(&ctx, "ws-7", &["chat://event/S"]);
        let name = crate::agent::chat::engine::event_name("S");
        let row = |text: &str| json!({"type":"rows","epoch":1,"revision":1,"positions":{},"rows":[{"kind":"assistant","id":"r","text":text,"streaming":true}]});
        ctx.app.emit(&name, row("a"));
        ctx.app.emit(&name, row("ab"));
        conn.send(&ctx, json!({"t":"chat-resync","sid":"S"}));
        conn.send(&ctx, json!({"t":"chat-resync","sid":"not watched"}));
        ctx.app.emit(&name, row("abc"));
        let (texts, _) = conn.drain().await;
        let payloads = chat_payloads(&texts, "S");
        assert_eq!(payloads.len(), 4);
        assert!(payloads[1]["rows"][0].get("patch").is_some());
        assert_eq!(payloads[2], json!({"type":"resync"}));
        assert_eq!(payloads[3]["rows"][0]["text"], "abc", "after the barrier the row is whole");
    }
}
