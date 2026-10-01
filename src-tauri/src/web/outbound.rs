//! Per-connection outbound queue for the WebSocket bridge: two lanes, bounded, drained by one writer.
//!
//! A remote client on a slow link receives less than the server produces while agents stream. A single
//! unbounded FIFO then made every reply wait behind minutes of queued events and terminal output, and the
//! heartbeat ping waited there too. This queue keeps the single writer (and with it the E2EE framing) but
//! changes what the writer takes next:
//!
//! - **Control lane**, always drained first: `hello`, `invoke` replies (ok and error) and the heartbeat
//!   `ping`. Replies match by id, not order, so they may overtake queued events; the chat state they race
//!   (rows, queue, mirror layout) is revisioned on the client and drops anything older than a snapshot.
//! - **Bulk lane**, FIFO: forwarded events, binary PTY frames, `pty-spawn` replies and `pty-resync`
//!   markers. `pty-spawn` replies stay here because the client's replay gate expects the replay bytes of an
//!   attach before its reply.
//!
//! Bounds (accounted in plaintext bytes; E2EE's base64 adds about a third on the wire):
//! - Latest-state events (`chat://event/{sid}` of type `extras`, `tree://changed`,
//!   `presets://changed`) coalesce latest-wins while an older copy is still unsent: the old entry is
//!   tombstoned and the new one appended, so it still lands after everything queued before it.
//! - Each session's unsent terminal output is capped at [`PTY_RESYNC_BYTES`]. Beyond that its unsent frames
//!   are dropped, a `{"t":"pty-resync","sid":..}` marker is queued in the bulk lane, and the sink reports
//!   failure so the PTY fan-out removes it; the client resets the terminal and reattaches (replay). Terminal
//!   bytes are therefore either delivered in order or explicitly replaced, never silently lost.
//! - Everything else is capped per lane ([`EVENT_BACKLOG_LIMIT`], [`CONTROL_BACKLOG_LIMIT`]). Exceeding a cap
//!   drops the backlog and closes the connection with code 1013; the client reconnects and resyncs through
//!   its existing path. Producers never block: a PTY sink runs under the output lock shared with every other
//!   subscriber, so blocking here would stall the desktop and all other clients.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::ws::{CloseFrame, Message};
use futures_util::{Sink, SinkExt};
use serde_json::{json, Map, Value};
use tokio::sync::Notify;

use super::e2ee::Cipher;

/// Unsent terminal output per session before it is replaced by a resync. Twice the replay ring: past that,
/// a replay costs less than late delivery, and a fresh replay alone (at most one ring plus the mode prelude,
/// queued from zero) can never trip it, so a resync cannot loop.
pub(crate) const PTY_RESYNC_BYTES: usize = 2 * crate::pty::manager::SCROLLBACK_CAP;

/// Unsent non-PTY bulk bytes (events, `pty-spawn` replies, resync markers). At the 250-460 KB/s measured
/// over a slow remote link this is 20-35 s of backlog, the scale at which the client already treats a
/// snapshot request as failed; reconnecting then is cheaper than streaming stale state for minutes.
pub(crate) const EVENT_BACKLOG_LIMIT: usize = 8 * 1024 * 1024;

/// Unsent control bytes. A safety net only: every control frame answers exactly one client request.
pub(crate) const CONTROL_BACKLOG_LIMIT: usize = 32 * 1024 * 1024;

/// Close code for a connection whose backlog exceeded a cap ("try again later"). Not 1008, which the share
/// client reads as a revoked grant.
pub(crate) const CLOSE_BACKLOG_LIMIT: u16 = 1013;

/// Traffic classes reported by the `ws_outbound` log line. The diagnostics field filter admits exactly these
/// names, so keep both in step through this constant.
pub(crate) const CLASSES: [&str; 14] = [
    "reply",
    "ping",
    "hello",
    "ptyOutput",
    "ptyReply",
    "ptyResync",
    "chatRows",
    "chatExtras",
    "chatQueue",
    "chatOther",
    "ptyEvent",
    "sessionState",
    "tree",
    "otherEvent",
];

/// Index into [`CLASSES`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Reply,
    Ping,
    Hello,
    PtyOutput,
    PtyReply,
    PtyResync,
    ChatRows,
    ChatExtras,
    ChatQueue,
    ChatOther,
    PtyEvent,
    SessionState,
    Tree,
    OtherEvent,
}

/// Result of queueing one terminal chunk.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PtyPush {
    /// Queued behind everything already in the bulk lane.
    Queued,
    /// The session's backlog passed [`PTY_RESYNC_BYTES`]: its unsent frames were dropped and a resync marker
    /// was queued. The sink must stop delivering.
    Resync,
    /// The connection is closing or overflowed.
    Closed,
}

/// What the writer takes next.
#[derive(Debug)]
pub(crate) enum Next {
    Frame(Message, usize, Class),
    /// A lane exceeded its cap: send a 1013 close and stop.
    Overflow,
}

/// What a bulk entry counts against.
enum Account {
    Event,
    Pty(String),
}

struct Entry {
    /// `None` once coalesced away or dropped by a resync; skipped when popped.
    msg: Option<Message>,
    bytes: usize,
    class: Class,
    account: Account,
    key: Option<String>,
}

#[derive(Default)]
struct Stats {
    peak_bytes: usize,
    sent_bytes: u64,
    sent_frames: u64,
    coalesced: u64,
    resyncs: u64,
    classes: [(u64, u64); CLASSES.len()],
}

struct State {
    control: VecDeque<(Message, usize, Class)>,
    control_bytes: usize,
    ping_queued: bool,
    bulk: VecDeque<Entry>,
    /// Sequence number of `bulk[0]`; entries are only ever taken from the front, so an entry's index is its
    /// sequence number minus this.
    front_seq: u64,
    event_bytes: usize,
    pty_bytes: HashMap<String, usize>,
    /// Coalescing key -> sequence number of its unsent entry.
    latest: HashMap<String, u64>,
    /// Live (not tombstoned) frames in both lanes.
    frames: usize,
    /// The writer has taken a frame and not finished sending it.
    in_flight: bool,
    /// Last completed send, or the moment the backlog formed after an idle period.
    last_progress: Instant,
    closed: bool,
    overflow: bool,
    stats: Stats,
}

impl State {
    fn queued_bytes(&self) -> usize {
        self.control_bytes + self.event_bytes + self.pty_bytes.values().sum::<usize>()
    }

    fn backlogged(&self) -> bool {
        self.in_flight || self.frames > 0
    }

    /// Restart the stall clock when work arrives at an idle writer, so an old last send does not count
    /// as a stall of a backlog that has only just formed.
    fn before_push(&mut self) {
        if !self.backlogged() {
            self.last_progress = Instant::now();
        }
    }

    fn after_push(&mut self) {
        self.frames += 1;
        let queued = self.queued_bytes();
        if queued > self.stats.peak_bytes {
            self.stats.peak_bytes = queued;
        }
    }

    fn push_bulk(&mut self, msg: Message, class: Class, account: Account, key: Option<String>) {
        self.before_push();
        let bytes = message_bytes(&msg);
        match &account {
            Account::Event => self.event_bytes += bytes,
            Account::Pty(sid) => *self.pty_bytes.entry(sid.clone()).or_default() += bytes,
        }
        let seq = self.front_seq + self.bulk.len() as u64;
        if let Some(key) = &key {
            if let Some(old) = self.latest.insert(key.clone(), seq) {
                let index = old.saturating_sub(self.front_seq) as usize;
                if self.tombstone(index) {
                    self.stats.coalesced += 1;
                }
            }
        }
        self.bulk.push_back(Entry { msg: Some(msg), bytes, class, account, key });
        self.after_push();
    }

    /// Drop one unsent bulk entry in place. Returns whether it was still live.
    fn tombstone(&mut self, index: usize) -> bool {
        let Some(entry) = self.bulk.get_mut(index) else { return false };
        if entry.msg.take().is_none() {
            return false;
        }
        let bytes = entry.bytes;
        let pty_sid = match &entry.account {
            Account::Event => None,
            Account::Pty(sid) => Some(sid.clone()),
        };
        match pty_sid {
            None => self.event_bytes -= bytes,
            Some(sid) => self.release_pty(&sid, bytes),
        }
        self.frames -= 1;
        true
    }

    fn release_pty(&mut self, sid: &str, bytes: usize) {
        if let Some(pending) = self.pty_bytes.get_mut(sid) {
            *pending -= bytes;
            if *pending == 0 {
                self.pty_bytes.remove(sid);
            }
        }
    }

    /// Mark the connection overflowed and free the backlog at once; the writer only sends the close.
    fn overflow(&mut self) {
        self.overflow = true;
        self.closed = true;
        self.control.clear();
        self.bulk.clear();
        self.latest.clear();
        self.pty_bytes.clear();
        self.control_bytes = 0;
        self.event_bytes = 0;
        self.frames = 0;
    }

    fn pop(&mut self) -> Option<(Message, usize, Class)> {
        if let Some((msg, bytes, class)) = self.control.pop_front() {
            self.control_bytes -= bytes;
            if class == Class::Ping {
                self.ping_queued = false;
            }
            self.frames -= 1;
            return Some((msg, bytes, class));
        }
        while let Some(entry) = self.bulk.pop_front() {
            let seq = self.front_seq;
            self.front_seq += 1;
            if let Some(key) = &entry.key {
                if self.latest.get(key) == Some(&seq) {
                    self.latest.remove(key);
                }
            }
            let Some(msg) = entry.msg else { continue };
            match &entry.account {
                Account::Event => self.event_bytes -= entry.bytes,
                Account::Pty(sid) => self.release_pty(sid, entry.bytes),
            }
            self.frames -= 1;
            return Some((msg, entry.bytes, entry.class));
        }
        None
    }
}

struct Inner {
    client_id: String,
    state: Mutex<State>,
    notify: Notify,
}

/// Shared handle to one connection's outbound queue. Producers push from any thread without blocking;
/// exactly one writer ([`run_writer`]) drains it.
#[derive(Clone)]
pub(crate) struct Outbound(Arc<Inner>);

impl Outbound {
    pub(crate) fn new(client_id: impl Into<String>) -> Self {
        Self(Arc::new(Inner {
            client_id: client_id.into(),
            state: Mutex::new(State {
                control: VecDeque::new(),
                control_bytes: 0,
                ping_queued: false,
                bulk: VecDeque::new(),
                front_seq: 0,
                event_bytes: 0,
                pty_bytes: HashMap::new(),
                latest: HashMap::new(),
                frames: 0,
                in_flight: false,
                last_progress: Instant::now(),
                closed: false,
                overflow: false,
                stats: Stats::default(),
            }),
            notify: Notify::new(),
        }))
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Queue a frame on the control lane. Returns false once the connection is closing.
    pub(crate) fn push_control(&self, msg: Message, class: Class) -> bool {
        let overflowed = {
            let mut state = self.state();
            if state.closed {
                return false;
            }
            Self::push_control_locked(&mut state, msg, class)
        };
        self.after_push(overflowed)
    }

    /// Queue the heartbeat ping, unless one is still unsent.
    pub(crate) fn push_ping(&self) -> bool {
        let overflowed = {
            let mut state = self.state();
            if state.closed {
                return false;
            }
            if state.ping_queued {
                return true;
            }
            state.ping_queued = true;
            Self::push_control_locked(&mut state, Message::Text(json!({"t":"ping"}).to_string()), Class::Ping)
        };
        self.after_push(overflowed)
    }

    fn push_control_locked(state: &mut State, msg: Message, class: Class) -> Option<usize> {
        state.before_push();
        let bytes = message_bytes(&msg);
        state.control_bytes += bytes;
        state.control.push_back((msg, bytes, class));
        state.after_push();
        (state.control_bytes > CONTROL_BACKLOG_LIMIT).then(|| {
            let queued = state.control_bytes;
            state.overflow();
            queued
        })
    }

    /// Queue a forwarded event on the bulk lane, coalescing latest-state events.
    pub(crate) fn push_event(&self, name: &str, payload: Value) -> bool {
        let (class, key) = classify(name, &payload);
        let msg = Message::Text(json!({ "t": "event", "name": name, "payload": payload }).to_string());
        self.push_bulk_msg(msg, class, key)
    }

    /// Queue a frame on the bulk lane behind everything already there (used for `pty-spawn` replies).
    pub(crate) fn push_bulk(&self, msg: Message, class: Class) -> bool {
        self.push_bulk_msg(msg, class, None)
    }

    fn push_bulk_msg(&self, msg: Message, class: Class, key: Option<String>) -> bool {
        let overflowed = {
            let mut state = self.state();
            if state.closed {
                return false;
            }
            state.push_bulk(msg, class, Account::Event, key);
            if state.event_bytes > EVENT_BACKLOG_LIMIT {
                let queued = state.event_bytes;
                state.overflow();
                Some(queued)
            } else {
                None
            }
        };
        self.after_push(overflowed)
    }

    /// Queue one binary PTY frame for `sid`, or replace the session's backlog with a resync marker when it
    /// would exceed [`PTY_RESYNC_BYTES`]. A chunk arriving at an empty backlog is always queued.
    pub(crate) fn push_pty(&self, sid: &str, frame: Vec<u8>) -> PtyPush {
        let dropped = {
            let mut state = self.state();
            if state.closed {
                return PtyPush::Closed;
            }
            let pending = state.pty_bytes.get(sid).copied().unwrap_or(0);
            if pending == 0 || pending + frame.len() <= PTY_RESYNC_BYTES {
                state.push_bulk(Message::Binary(frame), Class::PtyOutput, Account::Pty(sid.to_string()), None);
                None
            } else {
                let doomed: Vec<usize> = state
                    .bulk
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.msg.is_some() && matches!(&e.account, Account::Pty(s) if s == sid))
                    .map(|(i, _)| i)
                    .collect();
                for index in doomed {
                    state.tombstone(index);
                }
                state.stats.resyncs += 1;
                let marker = Message::Text(json!({"t":"pty-resync","sid":sid}).to_string());
                state.push_bulk(marker, Class::PtyResync, Account::Event, None);
                Some(pending + frame.len())
            }
        };
        self.0.notify.notify_one();
        match dropped {
            None => PtyPush::Queued,
            Some(bytes) => {
                crate::diagnostics::record(
                    "INFO",
                    "pty_resync",
                    json!({"clientId":self.0.client_id,"sessionId":sid,"bytes":bytes}),
                );
                PtyPush::Resync
            }
        }
    }

    fn after_push(&self, overflowed: Option<usize>) -> bool {
        self.0.notify.notify_one();
        match overflowed {
            None => true,
            Some(bytes) => {
                crate::diagnostics::record(
                    "WARN",
                    "ws_slow_consumer",
                    json!({"clientId":self.0.client_id,"bytes":bytes}),
                );
                false
            }
        }
    }

    /// Stop accepting frames; PTY sinks report failure from now on and the writer ends.
    pub(crate) fn close(&self) {
        self.state().closed = true;
        self.0.notify.notify_one();
    }

    /// Whether frames are waiting or being sent, and how long since the writer last made progress.
    pub(crate) fn progress(&self) -> (bool, Duration) {
        let state = self.state();
        (state.backlogged(), state.last_progress.elapsed())
    }

    /// Wait for the next frame, control lane first. `None` once the connection is closed.
    pub(crate) async fn next(&self) -> Option<Next> {
        loop {
            {
                let mut state = self.state();
                if state.overflow {
                    return Some(Next::Overflow);
                }
                if let Some((msg, bytes, class)) = state.pop() {
                    state.in_flight = true;
                    return Some(Next::Frame(msg, bytes, class));
                }
                if state.closed {
                    return None;
                }
            }
            // A push between the check above and this await leaves a permit, so no wakeup is lost.
            self.0.notify.notified().await;
        }
    }

    /// Record a completed send.
    fn sent(&self, bytes: usize, class: Class) {
        let mut state = self.state();
        state.in_flight = false;
        state.last_progress = Instant::now();
        state.stats.sent_bytes += bytes as u64;
        state.stats.sent_frames += 1;
        let slot = &mut state.stats.classes[class as usize];
        slot.0 += 1;
        slot.1 += bytes as u64;
    }

    /// Current queue depth plus the counters since the previous call, which resets them.
    pub(crate) fn take_stats(&self) -> Value {
        let mut state = self.state();
        let queued = state.queued_bytes();
        let stats = std::mem::take(&mut state.stats);
        let classes: Map<String, Value> = CLASSES
            .iter()
            .zip(stats.classes.iter())
            .filter(|(_, (frames, _))| *frames > 0)
            .map(|(name, (frames, bytes))| (name.to_string(), json!({"frames":frames,"bytes":bytes})))
            .collect();
        json!({
            "clientId": self.0.client_id,
            "queuedBytes": queued,
            "queuedFrames": state.frames,
            "peakQueuedBytes": stats.peak_bytes.max(queued),
            "sentBytes": stats.sent_bytes,
            "sentFrames": stats.sent_frames,
            "coalescedCount": stats.coalesced,
            "resyncCount": stats.resyncs,
            "classes": classes,
        })
    }

    /// Emit the periodic `ws_outbound` line (counters since the previous line).
    pub(crate) fn log_stats(&self) {
        crate::diagnostics::record("INFO", "ws_outbound", self.take_stats());
    }
}

/// The single writer: drain `outbound` into `sink`, control lane first. In E2EE mode text becomes base64
/// ciphertext and binary becomes raw ciphertext; Close/Ping/Pong pass through unchanged. Encryption or send
/// failure ends the writer; an overflowed queue sends a 1013 close and ends it.
pub(crate) async fn run_writer<S>(outbound: Outbound, mut sink: S, cipher: Option<Cipher>)
where
    S: Sink<Message> + Unpin,
{
    while let Some(next) = outbound.next().await {
        let (msg, bytes, class) = match next {
            Next::Frame(msg, bytes, class) => (msg, bytes, class),
            Next::Overflow => {
                let _ = sink
                    .send(Message::Close(Some(CloseFrame {
                        code: CLOSE_BACKLOG_LIMIT,
                        reason: "outbound backlog limit".into(),
                    })))
                    .await;
                break;
            }
        };
        let msg = match &cipher {
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
        if sink.send(msg).await.is_err() {
            break;
        }
        outbound.sent(bytes, class);
    }
    outbound.close();
}

fn message_bytes(msg: &Message) -> usize {
    match msg {
        Message::Text(t) => t.len(),
        Message::Binary(b) | Message::Ping(b) | Message::Pong(b) => b.len(),
        Message::Close(_) => 0,
    }
}

/// Traffic class and, for latest-state events, the coalescing key. Only events whose payload is a complete
/// state (or a bare signal) may coalesce; deltas such as chat rows, session state or presence may not.
fn classify(name: &str, payload: &Value) -> (Class, Option<String>) {
    if name.starts_with("chat://event/") {
        return match payload.get("type").and_then(Value::as_str) {
            Some("rows" | "replaceRows" | "reset") => (Class::ChatRows, None),
            Some("extras") => (Class::ChatExtras, Some(format!("{name}#extras"))),
            // Never coalesced: the client acknowledges its pending submissions by the ids each queue list
            // names, so dropping an older list could hide an id forever (a ghost "Queued" message).
            Some("queued") => (Class::ChatQueue, None),
            _ => (Class::ChatOther, None),
        };
    }
    if name == crate::host::TREE_CHANGED || name == crate::host::PRESETS_CHANGED {
        return (Class::Tree, Some(name.to_string()));
    }
    if ["pty://status/", "pty://exit/", "pty://killed/"].iter().any(|p| name.starts_with(p)) {
        return (Class::PtyEvent, None);
    }
    if name == crate::session_state::STATE_EVENT {
        return (Class::SessionState, None);
    }
    (Class::OtherEvent, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB: usize = 1024 * 1024;

    fn json_of(msg: &Message) -> Value {
        match msg {
            Message::Text(t) => serde_json::from_str(t).expect("text frames are JSON"),
            other => panic!("expected a text frame, got {other:?}"),
        }
    }

    fn reply(id: u64) -> Message {
        Message::Text(json!({"t":"reply","id":id,"ok":true,"result":null}).to_string())
    }

    fn pty_frame(sid: &str, payload: &[u8]) -> Vec<u8> {
        let mut frame = vec![sid.len() as u8];
        frame.extend_from_slice(sid.as_bytes());
        frame.extend_from_slice(payload);
        frame
    }

    /// Payload bytes of a binary frame for `sid`, or None for any other frame.
    fn pty_payload<'a>(msg: &'a Message, sid: &str) -> Option<&'a [u8]> {
        match msg {
            Message::Binary(b) if b.len() > sid.len() && &b[1..1 + sid.len()] == sid.as_bytes() => {
                Some(&b[1 + sid.len()..])
            }
            _ => None,
        }
    }

    /// Everything still queued, in the order the writer would send it.
    async fn drain(outbound: &Outbound) -> Vec<(Message, Class)> {
        outbound.close();
        let mut frames = Vec::new();
        while let Some(next) = outbound.next().await {
            match next {
                Next::Frame(msg, bytes, class) => {
                    outbound.sent(bytes, class);
                    frames.push((msg, class));
                }
                Next::Overflow => panic!("unexpected overflow"),
            }
        }
        frames
    }

    /// A real sink the writer can drive; it records what reaches the socket.
    fn recording_sink(
        out: Arc<Mutex<Vec<Message>>>,
    ) -> std::pin::Pin<Box<impl Sink<Message, Error = ()>>> {
        Box::pin(futures_util::sink::unfold(out, |out, msg: Message| async move {
            out.lock().unwrap().push(msg);
            Ok::<_, ()>(out)
        }))
    }

    fn rows_event(outbound: &Outbound, sid: &str, n: usize, filler: &str) {
        outbound.push_event(
            &format!("chat://event/{sid}"),
            json!({"type":"rows","n":n,"rows":[filler]}),
        );
    }

    /// T1: a reply queued behind megabytes of events and terminal output is the first frame after hello,
    /// and the bulk lane still leaves in exactly the order it was queued.
    #[tokio::test]
    async fn a_reply_overtakes_megabytes_of_queued_bulk() {
        let outbound = Outbound::new("ws-1");
        outbound.push_control(Message::Text(json!({"t":"hello","source":"ws-1"}).to_string()), Class::Hello);
        let filler = "x".repeat(64 * 1024);
        for n in 0..64 {
            rows_event(&outbound, "s", n, &filler); // 4 MiB of chat rows
        }
        for n in 0..14u8 {
            assert_eq!(outbound.push_pty("p", pty_frame("p", &[n; 64 * 1024])), PtyPush::Queued); // ~0.9 MiB
        }
        outbound.push_control(reply(42), Class::Reply);
        outbound.close();

        let sent = Arc::new(Mutex::new(Vec::new()));
        run_writer(outbound.clone(), recording_sink(sent.clone()), None).await;
        let sent = sent.lock().unwrap();
        assert_eq!(json_of(&sent[0])["t"], "hello");
        assert_eq!(json_of(&sent[1])["id"], 42, "the reply must not wait behind the bulk lane");
        let rows: Vec<u64> = sent[2..66].iter().map(|m| json_of(m)["payload"]["n"].as_u64().unwrap()).collect();
        assert_eq!(rows, (0..64).collect::<Vec<_>>());
        let pty: Vec<u8> = sent[66..].iter().map(|m| pty_payload(m, "p").unwrap()[0]).collect();
        assert_eq!(pty, (0..14).collect::<Vec<u8>>());
    }

    /// T9: hello leaves first even when events were queued before it and before the writer started.
    #[tokio::test]
    async fn hello_is_the_first_frame_even_behind_earlier_events() {
        let outbound = Outbound::new("ws-9");
        outbound.push_event(crate::session_state::STATE_EVENT, json!({"a":1}));
        outbound.push_control(Message::Text(json!({"t":"hello","source":"ws-9"}).to_string()), Class::Hello);
        let frames = drain(&outbound).await;
        assert_eq!(json_of(&frames[0].0)["t"], "hello");
        assert_eq!(frames[1].1, Class::SessionState);
    }

    /// T2: with nothing draining the socket, queued event bytes never exceed the cap plus one frame; past
    /// it the connection refuses everything and the writer sends only a 1013 close.
    #[tokio::test]
    async fn a_stalled_writer_keeps_the_backlog_bounded_and_closes_with_1013() {
        let outbound = Outbound::new("ws-2");
        let filler = "y".repeat(256 * 1024);
        let mut accepted = 0;
        for n in 0..100 {
            let before = outbound.take_stats()["queuedBytes"].as_u64().unwrap() as usize;
            assert!(before <= EVENT_BACKLOG_LIMIT, "queued {before} bytes");
            let ok = outbound.push_event("knowledge://changed", json!({"n":n,"f":filler}));
            if !ok {
                break;
            }
            accepted += 1;
        }
        assert!(accepted < 100, "the cap must trip before 25 MiB are queued");
        let stats = outbound.take_stats();
        assert_eq!(stats["queuedBytes"], 0, "an overflow frees the backlog at once");
        assert!(!outbound.push_event("knowledge://changed", json!({})));
        assert!(!outbound.push_control(reply(1), Class::Reply));
        assert_eq!(outbound.push_pty("p", pty_frame("p", b"x")), PtyPush::Closed);

        let sent = Arc::new(Mutex::new(Vec::new()));
        run_writer(outbound.clone(), recording_sink(sent.clone()), None).await;
        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 1, "nothing of the backlog is sent after an overflow");
        match &sent[0] {
            Message::Close(Some(frame)) => assert_eq!(frame.code, CLOSE_BACKLOG_LIMIT),
            other => panic!("expected a 1013 close, got {other:?}"),
        }
    }

    /// T2 (peak): the peak the log reports stays within the cap plus one frame.
    #[test]
    fn peak_event_backlog_stays_within_the_cap_plus_one_frame() {
        let outbound = Outbound::new("ws-2b");
        let filler = "z".repeat(100 * 1024);
        while outbound.push_event("knowledge://changed", json!({"f":filler})) {}
        let peak = outbound.take_stats()["peakQueuedBytes"].as_u64().unwrap() as usize;
        assert!(peak > EVENT_BACKLOG_LIMIT && peak <= EVENT_BACKLOG_LIMIT + filler.len() + 128, "peak {peak}");
    }

    /// T2 (control): the control lane has its own cap.
    #[test]
    fn the_control_lane_is_capped_too() {
        let outbound = Outbound::new("ws-2c");
        let big = Message::Text("r".repeat(MIB));
        let mut accepted = 0;
        while outbound.push_control(big.clone(), Class::Reply) {
            accepted += 1;
            assert!(accepted <= 32, "the control cap must trip");
        }
        assert_eq!(accepted, 32);
    }

    /// T3: repeated extras for one session collapse to the latest, which lands after everything queued
    /// before it; other sessions and chat rows are untouched and in order.
    #[tokio::test]
    async fn extras_coalesce_latest_wins_behind_earlier_rows() {
        let outbound = Outbound::new("ws-3");
        for n in 0..100 {
            rows_event(&outbound, "s", n, "r");
            outbound.push_event("chat://event/s", json!({"type":"extras","extras":{"n":n}}));
            outbound.push_event("chat://event/t", json!({"type":"extras","extras":{"n":n}}));
        }
        let frames = drain(&outbound).await;
        assert_eq!(frames.len(), 102);
        let rows: Vec<u64> = frames[..100].iter().map(|(m, _)| json_of(m)["payload"]["n"].as_u64().unwrap()).collect();
        assert_eq!(rows, (0..100).collect::<Vec<_>>());
        let s = json_of(&frames[100].0);
        assert_eq!((s["name"].as_str(), s["payload"]["extras"]["n"].as_u64()), (Some("chat://event/s"), Some(99)));
        let t = json_of(&frames[101].0);
        assert_eq!((t["name"].as_str(), t["payload"]["extras"]["n"].as_u64()), (Some("chat://event/t"), Some(99)));
    }

    /// T3: a coalesced extras moves to the tail, so it can never land before a `reset` that clears it.
    #[tokio::test]
    async fn coalesced_extras_land_after_an_intervening_reset() {
        let outbound = Outbound::new("ws-3b");
        outbound.push_event("chat://event/s", json!({"type":"extras","extras":{"n":1}}));
        outbound.push_event("chat://event/s", json!({"type":"reset","rows":[]}));
        outbound.push_event("chat://event/s", json!({"type":"extras","extras":{"n":2}}));
        let kinds: Vec<(String, Option<u64>)> = drain(&outbound)
            .await
            .iter()
            .map(|(m, _)| {
                let v = json_of(m);
                (v["payload"]["type"].as_str().unwrap().to_string(), v["payload"]["extras"]["n"].as_u64())
            })
            .collect();
        assert_eq!(kinds, vec![("reset".to_string(), None), ("extras".to_string(), Some(2))]);
    }

    /// T3: tree/preset signals coalesce; queue lists (acknowledged per id) and deltas (session state,
    /// presence) never do.
    #[tokio::test]
    async fn only_latest_state_events_coalesce() {
        let outbound = Outbound::new("ws-3c");
        for n in 0..3 {
            outbound.push_event("chat://event/s", json!({"type":"queued","items":[n],"revision":n}));
            outbound.push_event(crate::host::TREE_CHANGED, Value::Null);
            outbound.push_event(crate::host::PRESETS_CHANGED, Value::Null);
            outbound.push_event(crate::session_state::STATE_EVENT, json!({"n":n}));
            outbound.push_event(crate::web::presence::CLIENTS_EVENT, json!({"n":n}));
        }
        let frames = drain(&outbound).await;
        let count = |class: Class| frames.iter().filter(|(_, c)| *c == class).count();
        assert_eq!(count(Class::ChatQueue), 3, "every queue list reaches the client");
        assert_eq!(count(Class::Tree), 2, "one tree and one presets signal");
        assert_eq!(count(Class::SessionState), 3);
        assert_eq!(count(Class::OtherEvent), 3);
        let revisions: Vec<u64> = frames
            .iter()
            .filter(|(_, c)| *c == Class::ChatQueue)
            .map(|(m, _)| json_of(m)["payload"]["revision"].as_u64().unwrap())
            .collect();
        assert_eq!(revisions, vec![0, 1, 2]);
        let states: Vec<u64> = frames
            .iter()
            .filter(|(_, c)| *c == Class::SessionState)
            .map(|(m, _)| json_of(m)["payload"]["n"].as_u64().unwrap())
            .collect();
        assert_eq!(states, vec![0, 1, 2]);
    }

    /// T5: below the threshold every terminal byte arrives, in order, per session.
    #[tokio::test]
    async fn pty_output_below_the_threshold_is_delivered_complete_and_in_order() {
        let outbound = Outbound::new("ws-5");
        let mut expected_a = Vec::new();
        let mut expected_b = Vec::new();
        for n in 0..200u32 {
            let a = format!("a{n};").into_bytes();
            let b = format!("b{n};").into_bytes();
            expected_a.extend_from_slice(&a);
            expected_b.extend_from_slice(&b);
            assert_eq!(outbound.push_pty("sa", pty_frame("sa", &a)), PtyPush::Queued);
            assert_eq!(outbound.push_pty("sb", pty_frame("sb", &b)), PtyPush::Queued);
        }
        let frames = drain(&outbound).await;
        let got = |sid: &str| frames.iter().filter_map(|(m, _)| pty_payload(m, sid)).flatten().copied().collect::<Vec<u8>>();
        assert_eq!(got("sa"), expected_a);
        assert_eq!(got("sb"), expected_b);
    }

    /// T5: past the threshold the session's unsent frames are dropped as a whole and replaced by one resync
    /// marker at the tail; other sessions keep every byte in order, and nothing of the session follows it.
    #[tokio::test]
    async fn pty_backlog_past_the_threshold_is_replaced_by_an_explicit_resync() {
        let outbound = Outbound::new("ws-5b");
        let chunk = vec![b'a'; 64 * 1024];
        let mut queued_a = 0;
        let mut expected_b = Vec::new();
        let result = loop {
            let r = outbound.push_pty("sa", pty_frame("sa", &chunk));
            if r != PtyPush::Queued {
                break r;
            }
            queued_a += 1;
            let b = format!("b{queued_a};").into_bytes();
            expected_b.extend_from_slice(&b);
            outbound.push_pty("sb", pty_frame("sb", &b));
        };
        assert_eq!(result, PtyPush::Resync);
        assert!(queued_a * (64 * 1024) <= PTY_RESYNC_BYTES && (queued_a + 1) * (64 * 1024) > PTY_RESYNC_BYTES - 64);
        assert_eq!(outbound.take_stats()["resyncCount"], 1);
        let frames = drain(&outbound).await;
        assert!(frames.iter().all(|(m, _)| pty_payload(m, "sa").is_none()), "no stale frame of the session survives");
        let got_b: Vec<u8> = frames.iter().filter_map(|(m, _)| pty_payload(m, "sb")).flatten().copied().collect();
        assert_eq!(got_b, expected_b);
        let (marker, class) = frames.last().unwrap();
        assert_eq!(*class, Class::PtyResync);
        assert_eq!(json_of(marker), json!({"t":"pty-resync","sid":"sa"}));
    }

    /// T5: a pty-spawn reply queued behind its replay leaves after it (the client's replay gate).
    #[tokio::test]
    async fn a_pty_spawn_reply_follows_its_replay() {
        let outbound = Outbound::new("ws-5c");
        outbound.push_pty("sa", pty_frame("sa", b"replay"));
        outbound.push_bulk(reply(7), Class::PtyReply);
        outbound.push_control(reply(8), Class::Reply);
        let frames = drain(&outbound).await;
        let classes: Vec<Class> = frames.iter().map(|(_, c)| *c).collect();
        assert_eq!(classes, vec![Class::Reply, Class::PtyOutput, Class::PtyReply]);
    }

    /// T6: the heartbeat ping overtakes the backlog, and a ping still unsent is not queued twice.
    #[tokio::test]
    async fn the_ping_overtakes_the_backlog_and_is_not_duplicated() {
        let outbound = Outbound::new("ws-6");
        let filler = "p".repeat(64 * 1024);
        for n in 0..64 {
            rows_event(&outbound, "s", n, &filler);
        }
        assert!(outbound.push_ping());
        assert!(outbound.push_ping());
        let frames = drain(&outbound).await;
        assert_eq!(json_of(&frames[0].0)["t"], "ping");
        assert_eq!(frames.iter().filter(|(_, c)| *c == Class::Ping).count(), 1);
    }

    /// T6: a writer stuck on one send reports a backlog without progress; once it sends, progress resets.
    #[tokio::test]
    async fn progress_tracks_the_writer() {
        let outbound = Outbound::new("ws-6b");
        assert!(!outbound.progress().0, "an empty queue is not a backlog");
        outbound.push_event("knowledge://changed", json!({}));
        let (backlogged, since) = outbound.progress();
        assert!(backlogged);
        assert!(since < Duration::from_secs(5), "a backlog that just formed has not stalled");

        let stuck = Box::pin(futures_util::sink::unfold((), |_, _msg: Message| {
            futures_util::future::pending::<Result<(), ()>>()
        }));
        let writer = tokio::spawn(run_writer(outbound.clone(), stuck, None));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(outbound.progress().0, "a frame in flight still counts as backlog");
        writer.abort();

        let sent = Arc::new(Mutex::new(Vec::new()));
        let fresh = Outbound::new("ws-6c");
        fresh.push_event("knowledge://changed", json!({}));
        fresh.close();
        run_writer(fresh.clone(), recording_sink(sent), None).await;
        let (backlogged, since) = fresh.progress();
        assert!(!backlogged);
        assert!(since < Duration::from_secs(5));
    }

    /// T7: the stats line counts frames and bytes per class, coalescing and resyncs, then resets.
    #[tokio::test]
    async fn stats_count_sent_frames_per_class() {
        let outbound = Outbound::new("ws-7");
        outbound.push_control(reply(1), Class::Reply);
        outbound.push_event("chat://event/s", json!({"type":"extras","extras":{}}));
        outbound.push_event("chat://event/s", json!({"type":"extras","extras":{}}));
        outbound.push_pty("sa", pty_frame("sa", b"abc"));
        let frames = drain(&outbound).await;
        assert_eq!(frames.len(), 3);
        let stats = outbound.take_stats();
        assert_eq!(stats["clientId"], "ws-7");
        assert_eq!(stats["queuedBytes"], 0);
        assert_eq!(stats["queuedFrames"], 0);
        assert_eq!(stats["sentFrames"], 3);
        assert_eq!(stats["coalescedCount"], 1);
        assert_eq!(stats["resyncCount"], 0);
        assert_eq!(stats["classes"]["reply"]["frames"], 1);
        assert_eq!(stats["classes"]["chatExtras"]["frames"], 1);
        assert_eq!(stats["classes"]["ptyOutput"]["bytes"], 6);
        let total: u64 = stats["classes"].as_object().unwrap().values().map(|v| v["bytes"].as_u64().unwrap()).sum();
        assert_eq!(stats["sentBytes"], total);
        assert_eq!(crate::diagnostics::safe_fields(&stats), stats, "the log line survives the field filter");
        let again = outbound.take_stats();
        assert_eq!(again["sentFrames"], 0, "counters restart after each line");
    }

    /// T3 after a partial drain: coalescing still finds the unsent copy once the writer has already taken
    /// entries off the front, and a copy the writer already took is never touched (the new one is just sent).
    #[tokio::test]
    async fn coalescing_stays_correct_while_the_writer_drains() {
        let outbound = Outbound::new("ws-3d");
        let extras = |n: u64| json!({"type":"extras","extras":{"n":n}});
        rows_event(&outbound, "s", 0, "r");
        outbound.push_event("chat://event/s", extras(1));
        rows_event(&outbound, "s", 1, "r");
        // The writer takes rows 0; the unsent extras 1 now sits at the front.
        match outbound.next().await {
            Some(Next::Frame(msg, bytes, class)) => {
                assert_eq!(json_of(&msg)["payload"]["n"], 0);
                outbound.sent(bytes, class);
            }
            other => panic!("expected rows 0, got {other:?}"),
        }
        outbound.push_event("chat://event/s", extras(2));
        rows_event(&outbound, "s", 2, "r");
        // Take rows 1 (extras 1 is a tombstone and skipped), then extras 2 itself: it is now in flight.
        let mut taken = Vec::new();
        for _ in 0..2 {
            match outbound.next().await {
                Some(Next::Frame(msg, bytes, class)) => {
                    outbound.sent(bytes, class);
                    taken.push(json_of(&msg)["payload"].clone());
                }
                other => panic!("expected a frame, got {other:?}"),
            }
        }
        assert_eq!(taken[0]["n"], 1, "rows 1 follows rows 0; the stale extras 1 was dropped");
        assert_eq!(taken[1]["extras"]["n"], 2, "extras 2 took extras 1's place at the tail");
        // A new extras after the writer took extras 2 must not tombstone anything live (rows 2).
        outbound.push_event("chat://event/s", extras(3));
        let rest: Vec<Value> = drain(&outbound).await.iter().map(|(m, _)| json_of(m)["payload"].clone()).collect();
        assert_eq!(rest.len(), 2);
        assert_eq!(rest[0]["n"], 2);
        assert_eq!(rest[1]["extras"]["n"], 3);
        let stats = outbound.take_stats();
        assert_eq!(stats["coalescedCount"], 1);
        assert_eq!(stats["queuedBytes"], 0, "byte accounting returns to zero");
        assert_eq!(stats["queuedFrames"], 0, "frame accounting returns to zero");
    }

    /// AC1: the E2EE framing moved into `run_writer` unchanged: text becomes base64 ciphertext of the same
    /// JSON, binary becomes raw ciphertext of the same frame, both in lane order, and an overflow close is sent
    /// in plain (control frames pass through).
    #[tokio::test]
    async fn the_writer_encrypts_text_and_binary_frames_in_lane_order() {
        let dir = std::env::temp_dir().join(format!("vlx-outbound-e2ee-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let keys = super::super::e2ee::ServerKeys::load_or_create(&dir).unwrap();
        // Deriving against our own public key yields a cipher that opens its own frames.
        let cipher = keys.derive(keys.public_key_b64()).unwrap();

        let outbound = Outbound::new("ws-e2ee");
        rows_event(&outbound, "s", 0, "r");
        outbound.push_pty("p", pty_frame("p", b"term"));
        outbound.push_control(reply(5), Class::Reply);
        outbound.close();
        let sent = Arc::new(Mutex::new(Vec::new()));
        run_writer(outbound.clone(), recording_sink(sent.clone()), Some(cipher.clone())).await;
        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 3);
        let open_text = |m: &Message| match m {
            Message::Text(t) => {
                assert!(serde_json::from_str::<Value>(t).is_err(), "no plaintext JSON on the wire");
                serde_json::from_slice::<Value>(&cipher.decrypt_text(t).expect("decrypts")).unwrap()
            }
            other => panic!("expected an encrypted text frame, got {other:?}"),
        };
        assert_eq!(open_text(&sent[0])["id"], 5, "the reply still leaves first");
        assert_eq!(open_text(&sent[1])["payload"]["n"], 0);
        match &sent[2] {
            Message::Binary(b) => {
                assert_ne!(b.as_slice(), pty_frame("p", b"term").as_slice());
                assert_eq!(cipher.decrypt_bytes(b).expect("decrypts"), pty_frame("p", b"term"));
            }
            other => panic!("expected an encrypted binary frame, got {other:?}"),
        }

        let overflowed = Outbound::new("ws-e2ee-2");
        while overflowed.push_event("knowledge://changed", json!({"f":"o".repeat(MIB)})) {}
        let sent = Arc::new(Mutex::new(Vec::new()));
        run_writer(overflowed, recording_sink(sent.clone()), Some(cipher)).await;
        let sent = sent.lock().unwrap();
        assert!(matches!(&sent[..], [Message::Close(Some(f))] if f.code == CLOSE_BACKLOG_LIMIT));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
