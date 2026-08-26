//! Runtime state for one PTY session.

use std::collections::{BTreeMap, VecDeque};
use std::fs::File;
use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{ChildKiller, MasterPty};

/// Terminal recording appends raw PTY bytes **from session launch onward** for complete read-only archive replay,
/// not merely a scrollback snapshot.
///
/// A size limit stops writing after one truncation marker so spinner redraws cannot exhaust disk. Logs live under
/// `<data_dir>/recordings/<session-id>.log`; respawning the same session appends to the same file.
pub struct Recorder {
    file: File,
    /// Bytes written including preexisting length, used to enforce the limit.
    written: u64,
    /// Byte limit.
    cap: u64,
    /// Whether the limit stopped writes and the one-time marker was emitted.
    stopped: bool,
}

impl Recorder {
    pub fn new(file: File, existing_len: u64, cap: u64) -> Self {
        Self {
            file,
            written: existing_len,
            cap,
            stopped: false,
        }
    }

    /// Appends raw output. Silently drops after the limit and ignores write failures so terminal operation continues.
    pub fn write(&mut self, data: &[u8]) {
        if self.stopped {
            return;
        }
        if self.written >= self.cap {
            self.stopped = true;
            let _ = self
                .file
                .write_all(b"\r\n--- [VelaTerm] recording truncated: size limit reached ---\r\n");
            let _ = self.file.flush();
            return;
        }
        if self.file.write_all(data).is_ok() {
            self.written += data.len() as u64;
        }
    }
}

/// Output subscriber closure delivering raw bytes to one frontend channel.
///
/// Returns true while delivery succeeds, false after a browser page/Tauri frontend disconnect so the reader removes it.
///
/// Desktop wraps Tauri Channel binary send; browser enqueues bytes for the WS writer.
///
/// A closure keeps the PTY subsystem independent of concrete Tauri/Tokio types.
pub type OutputSink = Box<dyn Fn(&[u8]) -> bool + Send>;

/// Registered output subscriber with a stable ID and sink.
///
/// ID enables one browser tab to detach its route without affecting others or killing the session.
pub struct Subscriber {
    pub id: u64,
    pub sink: OutputSink,
}

/// Replay ring buffer retaining the latest raw terminal output.
///
/// New subscribers replay the snapshot before live output, immediately seeing the current screen. Full-screen TUIs
/// repair partial state on their next redraw; shells get recent history. Overflow drops from the front and may split
/// an escape sequence, which xterm tolerates and subsequent live output repairs.
pub struct ScrollbackBuffer {
    buf: VecDeque<u8>,
    cap: usize,
}

impl ScrollbackBuffer {
    pub fn new(cap: usize) -> Self {
        Self {
            buf: VecDeque::new(),
            cap,
        }
    }

    /// Appends output, dropping oldest bytes above capacity.
    pub fn push(&mut self, data: &[u8]) {
        self.buf.extend(data.iter().copied());
        if self.buf.len() > self.cap {
            let excess = self.buf.len() - self.cap;
            self.buf.drain(0..excess);
        }
    }

    /// Contiguous current snapshot for attachment replay.
    pub fn snapshot(&self) -> Vec<u8> {
        self.buf.iter().copied().collect()
    }
}

// Terminal-mode tracking for replay prelude.

/// DEC private modes that must survive attachment (`ESC [ ? Pm h|l`).
///
/// These define attached xterm rendering/input: alternate screen, mouse reporting/encodings, bracketed paste,
/// application cursor/keypad, focus reporting, cursor visibility, and more. TUIs often set them only at startup,
/// long before a 512 KB replay snapshot. Missing them makes alternate-screen redraws accumulate in normal scrollback
/// and input use wrong encodings.
///
/// `ModeTracker` therefore emits current settings before replay. Mode 2026 is deliberately excluded because its
/// synchronized-update pairs are frame-local and injecting it could defer rendering indefinitely.
const TRACKED_MODES: [u16; 21] = [
    1,    // DECCKM application cursor keys.
    5,    // DECSCNM reverse video.
    6,    // DECOM origin mode.
    7,    // DECAWM auto-wrap.
    12,   // Cursor blinking.
    25,   // DECTCEM cursor visibility.
    45,   // Reverse wraparound.
    47,   // Legacy alternate screen.
    66,   // DECNKM application keypad.
    1000, // Mouse button reporting.
    1002, // Mouse button and drag reporting.
    1003, // All mouse motion reporting.
    1004, // Focus reporting.
    1005, // UTF-8 mouse encoding.
    1006, // SGR mouse encoding.
    1015, // urxvt mouse encoding.
    1016, // SGR-Pixels mouse encoding.
    1047, // Alternate screen without saved cursor.
    1048, // Save/restore cursor.
    1049, // Alternate screen with cursor save/restore and clear.
    2004, // Bracketed paste.
];

/// Incremental DECSET/DECRST tracker that preserves parser state across chunks and records whitelisted mode values.
///
/// Like OutputScanner, skips complete OSC/DCS string payloads that may contain arbitrary bytes and ignores unrelated escapes.
#[derive(Default)]
pub struct ModeTracker {
    state: ModeScanState,
    /// Whitelisted mode to latest value. BTreeMap guarantees deterministic ascending prelude output.
    modes: BTreeMap<u16, bool>,
}

#[derive(Default)]
enum ModeScanState {
    #[default]
    Ground,
    /// Saw ESC; await the next byte to classify the sequence.
    Esc,
    /// Inside a `?`-prefixed private CSI, accumulating parameters.
    CsiPrivate { params: Vec<u16>, cur: Option<u32> },
    /// Inside a non-private or malformed CSI; skip through the final byte.
    CsiSkip,
    /// Inside OSC/DCS/APC/PM/SOS, awaiting BEL or ESC-backslash termination.
    InString { esc_pending: bool },
}

impl ModeTracker {
    /// Feeds output bytes and updates tracked mode values.
    pub fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.state = match std::mem::take(&mut self.state) {
                ModeScanState::Ground => match b {
                    0x1b => ModeScanState::Esc,
                    _ => ModeScanState::Ground,
                },
                ModeScanState::Esc => match b {
                    b'[' => ModeScanState::CsiPrivate {
                        params: Vec::new(),
                        cur: None,
                    },
                    // Enter and skip an OSC/DCS/APC/PM/SOS string payload.
                    b']' | b'P' | b'_' | b'^' | b'X' => {
                        ModeScanState::InString { esc_pending: false }
                    }
                    0x1b => ModeScanState::Esc,
                    _ => ModeScanState::Ground,
                },
                ModeScanState::CsiPrivate { mut params, cur } => {
                    // A CSI whose first byte is not `?` is not a private-mode sequence.
                    if params.is_empty() && cur.is_none() {
                        if b != b'?' {
                            // This byte may itself be final, as in `ESC [ m`; consume under skip rules.
                            match b {
                                0x40..=0x7e => ModeScanState::Ground,
                                0x1b => ModeScanState::Esc,
                                _ => ModeScanState::CsiSkip,
                            }
                        } else {
                            ModeScanState::CsiPrivate {
                                params,
                                cur: Some(0),
                            }
                        }
                    } else {
                        match b {
                            b'0'..=b'9' => {
                                // Saturate overflow at a large value that cannot match the u16 whitelist.
                                let v = cur
                                    .unwrap_or(0)
                                    .saturating_mul(10)
                                    .saturating_add(u32::from(b - b'0'));
                                ModeScanState::CsiPrivate {
                                    params,
                                    cur: Some(v),
                                }
                            }
                            b';' => {
                                params.push(u16::try_from(cur.unwrap_or(0)).unwrap_or(u16::MAX));
                                ModeScanState::CsiPrivate {
                                    params,
                                    cur: Some(0),
                                }
                            }
                            b'h' | b'l' => {
                                params.push(u16::try_from(cur.unwrap_or(0)).unwrap_or(u16::MAX));
                                let set = b == b'h';
                                for p in params {
                                    if TRACKED_MODES.contains(&p) {
                                        self.modes.insert(p, set);
                                    }
                                }
                                ModeScanState::Ground
                            }
                            // Ignore private sequences ending in finals other than h/l, such as DECRQM `$p`.
                            0x40..=0x7e => ModeScanState::Ground,
                            0x1b => ModeScanState::Esc,
                            // Intermediate bytes such as `$` switch to skipping through final.
                            _ => ModeScanState::CsiSkip,
                        }
                    }
                }
                ModeScanState::CsiSkip => match b {
                    0x40..=0x7e => ModeScanState::Ground,
                    0x1b => ModeScanState::Esc,
                    _ => ModeScanState::CsiSkip,
                },
                ModeScanState::InString { esc_pending } => {
                    if esc_pending {
                        // ESC-backslash terminates; any other byte after ESC remains inside the string.
                        if b == b'\\' {
                            ModeScanState::Ground
                        } else {
                            ModeScanState::InString {
                                esc_pending: b == 0x1b,
                            }
                        }
                    } else if b == 0x07 {
                        // BEL terminates OSC and is safely accepted for other strings in xterm practice.
                        ModeScanState::Ground
                    } else {
                        ModeScanState::InString {
                            esc_pending: b == 0x1b,
                        }
                    }
                }
            };
        }
    }

    /// Generates an ascending DECSET/DECRST replay prelude for every mode touched by the application; empty for ordinary shells.
    pub fn prelude(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for (mode, set) in &self.modes {
            out.extend_from_slice(
                format!("\x1b[?{}{}", mode, if *set { 'h' } else { 'l' }).as_bytes(),
            );
        }
        out
    }
}

// Output stream under one aggregate lock.

/// Shared replay buffer, mode tracker, and subscriber list protected by **one lock**.
///
/// Separate locks would let reader ingestion/fan-out interleave with attach snapshot/replay/register, causing a new
/// subscriber to duplicate a chunk contained in both snapshot and fan-out or miss one contained in neither. One lock
/// makes each path atomic: subscribers receive exactly the snapshot followed by each later chunk once.
///
/// Calling sinks under lock is safe because Tauri Channel send and unbounded Tokio send only enqueue without blocking.
pub struct OutputStream {
    ring: ScrollbackBuffer,
    modes: ModeTracker,
    subscribers: Vec<Subscriber>,
}

impl OutputStream {
    pub fn new(ring_cap: usize) -> Self {
        Self {
            ring: ScrollbackBuffer::new(ring_cap),
            modes: ModeTracker::default(),
            subscribers: Vec::new(),
        }
    }

    /// Reader entry: buffer a chunk, update modes, fan out, and remove disconnected subscribers.
    pub fn ingest(&mut self, chunk: &[u8]) {
        self.ring.push(chunk);
        self.modes.feed(chunk);
        self.subscribers.retain(|s| (s.sink)(chunk));
    }

    /// Attaches a subscriber by replaying mode prelude plus snapshot before registration.
    ///
    /// The prelude restores alternate-screen, mouse, paste, and other modes that likely fell out of the ring; the
    /// snapshot immediately restores the visible screen.
    pub fn attach(&mut self, id: u64, sink: OutputSink) {
        let prelude = self.modes.prelude();
        if !prelude.is_empty() {
            sink(&prelude);
        }
        let replay = self.ring.snapshot();
        if !replay.is_empty() {
            sink(&replay);
        }
        self.subscribers.push(Subscriber { id, sink });
    }

    /// Removes one subscriber on browser detach.
    pub fn detach(&mut self, sub_id: u64) {
        self.subscribers.retain(|s| s.id != sub_id);
    }

    /// Current subscriber count for tests.
    #[cfg(test)]
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }
}

// Output coalescer between the reader and ingest.

/// Coalescing window: chunks arriving soon after the last flush accumulate until a flusher ingests them together.
/// A 12 ms frame reduces bursty bridge messages to one per session/window while sparse echoes/prompts send immediately.
pub const COALESCE_WINDOW: Duration = Duration::from_millis(12);
/// Flush early at this byte limit to avoid oversized delivery. 64 KB matches frontend chunks and parses in milliseconds.
pub const COALESCE_MAX_BYTES: usize = 64 * 1024;

/// PTY input queue capacity in chunks, one per key or paste. A full queue means prolonged unread stdin, where an error
/// is preferable to unbounded backlog.
pub const INPUT_QUEUE_CAP: usize = 1024;

/// Coalescing buffer shared by reader and timed flusher threads.
///
/// Ordering contract: lock the coalescer first and acquire/ingest the stream while still holding it. Both threads use
/// coalescer -> stream order, making pending-to-ingest atomic without reverse acquisition or deadlock.
///
/// Attach need not know about coalescing: snapshots contain ingested bytes and pending data arrives later as live output,
/// preserving exactly-once semantics.
pub struct OutputCoalescer {
    pending: Vec<u8>,
    /// Arrival time of the first unflushed byte, None when empty.
    since: Option<Instant>,
    /// Last flush time, used to send immediately after an idle window.
    last_flush: Instant,
}

impl Default for OutputCoalescer {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputCoalescer {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            since: None,
            // Treat initialization as long-idle so the first startup output sends immediately.
            last_flush: Instant::now()
                .checked_sub(COALESCE_WINDOW)
                .unwrap_or_else(Instant::now),
        }
    }

    /// Appends a chunk and returns whether to flush immediately due to capacity or elapsed window. False means
    /// accumulation; a newly created deadline should wake the flusher.
    pub fn push(&mut self, chunk: &[u8], now: Instant) -> bool {
        self.pending.extend_from_slice(chunk);
        if self.pending.len() >= COALESCE_MAX_BYTES
            || now.duration_since(self.last_flush) >= COALESCE_WINDOW
        {
            true
        } else {
            if self.since.is_none() {
                self.since = Some(now);
            }
            false
        }
    }

    /// Takes all pending bytes and advances last_flush, returning empty when none.
    pub fn take(&mut self, now: Instant) -> Vec<u8> {
        self.since = None;
        self.last_flush = now;
        std::mem::take(&mut self.pending)
    }

    /// Current pending deadline, or None.
    pub fn deadline(&self) -> Option<Instant> {
        self.since.map(|t| t + COALESCE_WINDOW)
    }
}

/// Handles owned by a running PTY session.
///
/// `master` resizes; `input` sends to the dedicated fd-writer queue; `killer` terminates. The Child itself lives in
/// a dedicated waiter thread blocked on wait (see manager.rs).
///
/// PtyManager::spawn starts a detached reader that continuously reads the master and fans out through `stream`.
pub struct PtySession {
    #[allow(dead_code)]
    pub master: Box<dyn MasterPty + Send>,
    /// Input queue makes `pty_write` return after enqueue without touching the fd. A dedicated FIFO writer isolates
    /// blocking write_all from the global sessions lock and other sessions. Removing the session drops Sender and
    /// naturally ends the writer's recv loop.
    pub input: std::sync::mpsc::SyncSender<Vec<u8>>,
    pub killer: Box<dyn ChildKiller + Send + Sync>,
    /// Child PID used to query runtime cwd.
    pub pid: u32,
    /// Whether termination was intentional, such as restart or closing a split. Shared with the reader so it suppresses
    /// natural-exit events that would otherwise close the pane.
    pub intentional: Arc<AtomicBool>,
    /// Who asked for the termination and why, filled in by `kill` and read by the reader when it announces
    /// the death. Without it the announcement is bare, and every other client has to guess whether the
    /// session is coming back — which is why restarting one used to close its tab everywhere.
    pub kill_info: Arc<Mutex<Option<KillInfo>>>,
    /// Shared single-lock OutputStream; the reader ingests each chunk into buffering, tracking, and fan-out.
    pub stream: Arc<Mutex<OutputStream>>,
    /// Authoritative `(cols, rows)`, initialized at spawn and updated after resize. Attached mirror xterms match it
    /// before replay to prevent layout corruption.
    pub size: Mutex<(u16, u16)>,
    /// Sizing owner (`desktop` or `ws-N`). Only owner or explicit takeover may resize; other clients mirror. None
    /// means the previous owner detached and the next resize claims ownership.
    pub size_owner: Mutex<Option<String>>,
}

/// Why a session was deliberately terminated, and by whom.
///
/// `reason` separates the two cases that look identical from the outside: `restart` means the same
/// session is about to come back with a new process, so every client keeps its pane and waits; `close`
/// means it is going away, so other clients close their view. `source` is the requesting client's
/// connection ID (`desktop` or `ws-N`), which lets the requester recognise and ignore its own echo
/// instead of inferring it from a timing window.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KillInfo {
    pub source: String,
    pub reason: KillReason,
}

/// Whether a deliberate termination ends the session or precedes a new process for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KillReason {
    /// The session is going away: closing a tab, closing a split, deleting the session.
    Close,
    /// A new process for this same session follows immediately.
    Restart,
}

impl KillReason {
    /// Parse the wire value, defaulting to `Close`.
    ///
    /// An unknown or missing reason must read as `Close`: keeping a pane for a session that never comes
    /// back leaves a dead terminal on screen, which is worse than closing one the user meant to restart.
    pub fn parse(raw: Option<&str>) -> Self {
        match raw {
            Some("restart") => KillReason::Restart,
            _ => KillReason::Close,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        KillReason, ModeTracker, OutputCoalescer, OutputStream, ScrollbackBuffer, COALESCE_MAX_BYTES,
        COALESCE_WINDOW,
    };
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    /// An unknown or missing reason has to read as `close`. Keeping a pane for a session that never
    /// comes back leaves a dead terminal on screen, which is worse than closing one meant to restart —
    /// and an older client that sends no reason at all must keep behaving the way it always did.
    #[test]
    fn an_unknown_kill_reason_reads_as_close() {
        assert_eq!(KillReason::parse(Some("restart")), KillReason::Restart);
        assert_eq!(KillReason::parse(Some("close")), KillReason::Close);
        assert_eq!(KillReason::parse(None), KillReason::Close);
        assert_eq!(KillReason::parse(Some("something-else")), KillReason::Close);
    }

    /// The wire form is what other clients branch on, so it must stay exactly these two words.
    #[test]
    fn kill_reason_serializes_to_its_wire_form() {
        assert_eq!(serde_json::to_string(&KillReason::Restart).unwrap(), "\"restart\"");
        assert_eq!(serde_json::to_string(&KillReason::Close).unwrap(), "\"close\"");
    }

    /// Coalescer sends sparse output immediately and accumulates dense output without reordering.
    #[test]
    fn coalescer_sparse_flush_now_dense_accumulates() {
        let mut c = OutputCoalescer::new();
        let t0 = Instant::now();
        // First chunk after an idle window sends immediately.
        assert!(c.push(b"echo", t0));
        assert_eq!(c.take(t0).as_slice(), b"echo");
        // Closely following small chunks accumulate until the first pending chunk plus the window.
        assert!(!c.push(b"a", t0 + Duration::from_millis(1)));
        assert!(!c.push(b"b", t0 + Duration::from_millis(2)));
        assert_eq!(
            c.deadline(),
            Some(t0 + Duration::from_millis(1) + COALESCE_WINDOW)
        );
        // At deadline, take and deliver in order, clearing pending state.
        assert_eq!(c.take(t0 + Duration::from_millis(13)).as_slice(), b"ab");
        assert_eq!(c.deadline(), None);
        // A chunk after another idle window sends immediately again.
        assert!(c.push(b"c", t0 + Duration::from_millis(13) + COALESCE_WINDOW));
    }

    /// Coalescer flushes early when byte capacity is reached before the deadline.
    #[test]
    fn coalescer_cap_forces_early_flush() {
        let mut c = OutputCoalescer::new();
        let t0 = Instant::now();
        assert!(c.push(b"x", t0)); // Immediate send advances last_flush.
        c.take(t0);
        let big = vec![b'y'; COALESCE_MAX_BYTES];
        assert!(c.push(&big, t0 + Duration::from_millis(1)));
        assert_eq!(
            c.take(t0 + Duration::from_millis(1)).len(),
            COALESCE_MAX_BYTES
        );
    }

    #[test]
    fn scrollback_keeps_recent_within_cap() {
        let mut b = ScrollbackBuffer::new(5);
        b.push(b"abc");
        assert_eq!(b.snapshot(), b"abc");
        // Overflow drops from the front, retaining only the latest capacity bytes.
        b.push(b"defg");
        assert_eq!(b.snapshot(), b"cdefg");
        // A single oversized push retains its final capacity bytes.
        b.push(b"0123456789");
        assert_eq!(b.snapshot(), b"56789");
    }

    /// Mode tracking records latest whitelisted DECSET/DECRST and emits an ascending prelude.
    #[test]
    fn mode_tracker_basic_set_reset_and_prelude() {
        let mut t = ModeTracker::default();
        // Typical Claude startup: alternate screen, application cursor, mouse modes, bracketed paste, hidden cursor.
        t.feed(b"\x1b[?1049h\x1b[?1h\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?2004h\x1b[?25l");
        assert_eq!(
            t.prelude(),
            b"\x1b[?1h\x1b[?25l\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?1049h\x1b[?2004h"
        );
        // Latest reset value wins.
        t.feed(b"\x1b[?1049l\x1b[?25h");
        let p = String::from_utf8(t.prelude()).unwrap();
        assert!(p.contains("\x1b[?1049l"));
        assert!(p.contains("\x1b[?25h"));
    }

    /// Handles cross-chunk sequences and multi-parameter sequences while ignoring non-whitelisted modes.
    #[test]
    fn mode_tracker_split_chunks_and_multi_params() {
        let mut t = ModeTracker::default();
        // Split `ESC [ ? 10` and `49h` across chunks.
        t.feed(b"\x1b[?10");
        t.feed(b"49h");
        // Set multiple modes in one common application sequence.
        t.feed(b"\x1b[?1000;1006;9999h");
        let p = String::from_utf8(t.prelude()).unwrap();
        assert_eq!(p, "\x1b[?1000h\x1b[?1006h\x1b[?1049h");
        // Mode 9999 is not whitelisted and must not appear.
        assert!(!p.contains("9999"));
    }

    /// OSC/DCS payloads are not misparsed; non-private CSI and DECRQM do not affect tracking.
    #[test]
    fn mode_tracker_ignores_strings_and_non_mode_sequences() {
        let mut t = ModeTracker::default();
        // Fake DECSET text in an OSC title validates string skipping.
        t.feed(b"\x1b]0;title ?1049h fake\x07");
        // DCS payload terminated by ST.
        t.feed(b"\x1bPpayload?25l\x1b\\");
        // Ordinary SGR CSI and DECRQM query whose final is not h/l.
        t.feed(b"\x1b[31m\x1b[?2026$p");
        assert!(t.prelude().is_empty());
        // Normal sequences remain trackable after string termination.
        t.feed(b"\x1b[?2004h");
        assert_eq!(t.prelude(), b"\x1b[?2004h");
    }

    /// Attachment receives mode prelude plus current snapshot, then all subsequent chunks.
    #[test]
    fn output_stream_attach_replays_prelude_then_snapshot() {
        let mut s = OutputStream::new(64);
        s.ingest(b"\x1b[?1049h\x1b[?1000h");
        s.ingest(b"hello");

        let got: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
        let got_cb = Arc::clone(&got);
        s.attach(
            1,
            Box::new(move |b| {
                got_cb.lock().unwrap().push(b.to_vec());
                true
            }),
        );
        s.ingest(b" world");

        let chunks = got.lock().unwrap().clone();
        assert_eq!(chunks.len(), 3, "expected a prelude, a snapshot and a live chunk");
        // Prelude is sorted by mode number regardless of ingest order.
        assert_eq!(chunks[0], b"\x1b[?1000h\x1b[?1049h");
        assert_eq!(chunks[1], b"\x1b[?1049h\x1b[?1000hhello");
        assert_eq!(chunks[2], b" world");
    }

    /// Concurrency stress: single-lock ingest and attach are atomic, so every subscriber receives a **contiguous
    /// suffix** of the canonical stream after the prelude, with no missing or duplicate chunks.
    #[test]
    fn output_stream_concurrent_attach_no_dup_no_loss() {
        let stream = Arc::new(Mutex::new(OutputStream::new(1 << 20)));
        // Canonical stream of 128 numbered chunks with enough ring capacity to retain the head.
        let canonical: Vec<Vec<u8>> = (0..128u32)
            .map(|i| format!("<{i:04}>").into_bytes())
            .collect();
        let full: Vec<u8> = canonical.concat();

        let writer = {
            let stream = Arc::clone(&stream);
            let canonical = canonical.clone();
            std::thread::spawn(move || {
                for c in &canonical {
                    stream.lock().unwrap().ingest(c);
                    std::thread::yield_now();
                }
            })
        };

        let mut sinks = Vec::new();
        for i in 0..32u64 {
            let got: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
            let got_cb = Arc::clone(&got);
            stream.lock().unwrap().attach(
                100 + i,
                Box::new(move |b| {
                    got_cb.lock().unwrap().extend_from_slice(b);
                    true
                }),
            );
            sinks.push(got);
            std::thread::yield_now();
        }
        writer.join().unwrap();

        for got in sinks {
            let bytes = got.lock().unwrap().clone();
            // With no mode prelude, received bytes must be a contiguous suffix ending at the canonical stream end.
            let pos = full
                .windows(bytes.len().max(1))
                .position(|w| w == bytes.as_slice());
            assert!(
                !bytes.is_empty() && pos == Some(full.len() - bytes.len()),
                "a subscriber's byte stream should be a contiguous suffix of the full stream, losing and repeating nothing; actual length {}",
                bytes.len()
            );
        }
    }
}
