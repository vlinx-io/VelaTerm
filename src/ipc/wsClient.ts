//! Browser-side WebSocket client that maps the desktop Tauri IPC surface
//! (invoke, listen, and PTY channels) onto a **single** connection to the Rust web service.
//! This is used only in the browser; the desktop client never touches it.
//!
//! Protocol (kept in sync with `src-tauri/src/web`):
//! - Text frames carry JSON control messages.
//! - Binary frames carry PTY output: `[1-byte sid length][sid UTF-8][raw bytes]`.
//!
//! Client -> server (JSON):
//!   { t:"invoke",    id, cmd, args }      Generic commands (list_tree, pty_write, git, etc.)
//!   { t:"pty-spawn", id, sid, args }      Spawn or attach to a PTY; returns { pid, launch, subId }
//!   { t:"pty-detach", sid, subId }        Leave the session locally (detach; never kill it)
//! Server -> client:
//!   { t:"reply", id, ok, result | error } Command response
//!   { t:"event", name, payload }          Forwarded Tauri events (pty://status|exit, spawn://, notify://)
//!   Binary frame                            PTY output routed to subscribers by sid
//!
//! Reconnection: after a disconnect, reconnect automatically with exponential backoff from
//! 1s to 30s. Before each retry, probe `GET /api/me` to distinguish failures. A 401 means
//! the session credential is stale (typically because a server restart cleared the in-memory
//! token set), so retrying the `/ws` upgrade would always fail. Notify `onAuthLost` subscribers
//! (LoginGate) to return to the login page instead. Network failures continue backing off.
//! Once reconnected, reissue pty-spawn for every registered PTY. The server attaches to a
//! running session and replays its current screen, making recovery automatic. `onConnState`
//! broadcasts connection state for the disconnect banner.
//!
//! Half-open connections are normally detected by server heartbeats, with a 75s idle check as
//! a fallback. Because a half-open socket never fires `onclose`, waiting for that check can leave
//! the UI visible but frozen for up to ~75s. Browser `online` and window `focus`/
//! `visibilitychange` events therefore trigger proactive recovery. `online` always reconnects;
//! focus and visibility changes reconnect only after a prolonged lack of inbound traffic (see
//! `wakeReconnect`). A half-open socket still reports OPEN, so close it first; otherwise
//! `reconnectNow` would incorrectly return early.

import type { UnlistenFn } from "@tauri-apps/api/event";
import nacl from "tweetnacl";

import { t } from "../i18n";
import { handshakeFailureReason, mapBackendError, type HandshakeFailure } from "./backendError";
import { recordRequestError } from "./reqLog";
import type { PtySpawnArgs, PtySpawnResult } from "./transport";

type EventCb = (payload: unknown) => void;
type ByteCb = (bytes: Uint8Array) => void;

/** Connection state: online means connected; offline includes the consecutive retry count. */
export type ConnState = "online" | "offline";

/** Session-token key in window-scoped sessionStorage; see the sessionToken field. */
const TOKEN_STORAGE_KEY = "vlx-token";

/** Reconnect backoff starts at 1s, doubles each time, and is capped at 30s. */
const RECONNECT_BASE_MS = 1000;
const RECONNECT_MAX_MS = 30_000;

/**
 * The server sends an application-level `{"t":"ping"}` text-frame heartbeat every 30s because
 * browser JavaScript cannot observe protocol-level Ping/Pong frames. A healthy connection should
 * therefore never be quiet longer than that. After 75s (2.5 heartbeat intervals) without inbound
 * traffic, treat the socket as half-open due to NAT or sleep: it may report OPEN even though it is
 * dead and will never fire onclose. Closing it explicitly triggers automatic reconnection.
 */
const IDLE_TIMEOUT_MS = 75_000;
const IDLE_CHECK_MS = 15_000;

/**
 * Silence threshold used to flag a suspicious connection when the window or network wakes
 * (returning to the window or waking a laptop). If no inbound traffic arrives for longer than
 * one heartbeat interval, assume the connection may be half-open and reconnect proactively.
 * This applies to focus/visibility wakeups; `online` is an explicit recovery signal and ignores it.
 */
const WAKE_STALE_MS = 35_000;

interface Pending {
  resolve: (v: unknown) => void;
  reject: (e: unknown) => void;
}

/**
 * Expected transport errors include connection failures, disconnects, and handshake/auth failures.
 * The disconnect banner, automatic reconnect logic, or LoginGate already handles these recoverable
 * conditions, so they must not reach the global uncaught-error fallback in main.tsx, which is for
 * genuinely unexpected bugs. The `expected` marker tells the global unhandledrejection handler to
 * call preventDefault and suppress the uncaught-error overlay.
 */
class TransportError extends Error {
  readonly expected = true;
  constructor(message: string) {
    super(message);
    this.name = "TransportError";
  }
}

const textDecoder = new TextDecoder();
const textEncoder = new TextEncoder();

/** Decode base64 into bytes. */
function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Encode bytes as base64. */
export function bytesToB64(bytes: Uint8Array): string {
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}

/** E2EE pairing data for one connection: device token and 32-byte server public key. */
interface Pairing {
  token: string;
  serverPub: Uint8Array;
}

/**
 * Read pairing data from the URL fragment (`#pair=<base64url(JSON{t,k})>`) and cache it in
 * sessionStorage so refreshes still work. Fragments are not sent to the server or included in
 * logs/Referer headers. Return null when no pairing data exists, selecting the token flow.
 */
function readPairing(): Pairing | null {
  try {
    const hash = location.hash.startsWith("#")
      ? location.hash.slice(1)
      : location.hash;
    const code = new URLSearchParams(hash).get("pair");
    let json: string | null = null;
    if (code) {
      const b64 = code.replace(/-/g, "+").replace(/_/g, "/");
      json = atob(b64);
      sessionStorage.setItem("vlx-pair", json);
    } else {
      json = sessionStorage.getItem("vlx-pair");
    }
    if (!json) return null;
    const o = JSON.parse(json) as { t?: string; k?: string };
    if (!o.t || !o.k) return null;
    return { token: o.t, serverPub: b64ToBytes(o.k) };
  } catch {
    return null;
  }
}

/** Stable device ID, generated once and stored in localStorage for server registration. */
function getDeviceId(): string {
  try {
    let id = localStorage.getItem("vlx-device-id");
    if (!id) {
      id =
        typeof crypto !== "undefined" && crypto.randomUUID
          ? crypto.randomUUID()
          : Math.random().toString(36).slice(2) + Date.now().toString(36);
      localStorage.setItem("vlx-device-id", id);
    }
    return id;
  } catch {
    return "browser";
  }
}

/** Display-only device name (`OS · Browser`) sent during registration; not a credential. */
function getDeviceName(): string {
  try {
    const ua = navigator.userAgent;
    const os = /Mac/.test(ua)
      ? "macOS"
      : /Win/.test(ua)
        ? "Windows"
        : /Android/.test(ua)
          ? "Android"
          : /iPhone|iPad|iPod/.test(ua)
            ? "iOS"
            : /Linux/.test(ua)
              ? "Linux"
              : "Device";
    const br = /Edg\//.test(ua)
      ? "Edge"
      : /Chrome\//.test(ua)
        ? "Chrome"
        : /Firefox\//.test(ua)
          ? "Firefox"
          : /Safari\//.test(ua)
            ? "Safari"
            : "Browser";
    return `${os} · ${br}`;
  } catch {
    return "Browser";
  }
}

class WsClient {
  private ws: WebSocket | null = null;
  private readonly url: string;
  private nextId = 1;
  private readonly pending = new Map<number, Pending>();
  private readonly events = new Map<string, Set<EventCb>>();
  /** sid -> output callback (xterm.write). */
  private readonly ptySinks = new Map<string, ByteCb>();
  /** sid -> original spawn arguments, resent when reconnecting and reattaching. */
  private readonly ptyArgs = new Map<string, PtySpawnArgs>();
  /** sid -> current subscription ID, returned when detaching. */
  private readonly subIds = new Map<string, number>();
  /**
   * sid -> callbacks fired after WebSocket reconnection and delivery of all reattach replay frames.
   * usePtySession uses the latest respawn result to correct its mirrored grid and flush the replay.
   */
  private readonly reattachCbs = new Map<string, Set<(res: PtySpawnResult) => void>>();
  /**
   * sid -> callbacks fired when reattachment **starts**, after reconnection but before resending
   * pty-spawn. usePtySession uses this signal to rearm replay gating and reset the screen. A
   * reattach replay redraws the mode prelude and current-screen snapshot as a unit; without a
   * reset, the entire history would be appended to the existing buffer and duplicate its contents.
   */
  private readonly reattachStartCbs = new Map<string, Set<() => void>>();
  /**
   * Source identifier for this connection, supplied by the server's hello frame as `ws-N`.
   * The client uses it to determine whether `Resized.owner` or `SpawnResult.owner` refers to this
   * connection. Hello is always the first frame, before any reply or event, so it is available
   * whenever a spawn result is processed.
   */
  private source: string | null = null;
  private connectPromise: Promise<void> | null = null;
  /** Consecutive reconnect failures; reset on success and used for backoff and the banner. */
  private reconnectAttempts = 0;
  /** Pending reconnect timer; ensures that only one retry is scheduled at a time. */
  private reconnectTimer: ReturnType<typeof setTimeout> | undefined;
  /** Credential-expiry callbacks; LoginGate returns to login after an /api/me 401. The optional
   *  reason distinguishes a server-side login rate limit from failed credentials on the E2EE path. */
  private readonly authLostCbs = new Set<(reason?: HandshakeFailure) => void>();
  /** Connection-state callbacks used to show or hide the disconnect banner. */
  private readonly connStateCbs = new Set<(state: ConnState, attempts: number) => void>();
  /** Time of the latest inbound message, including heartbeats, for idle detection. */
  private lastInbound = Date.now();
  /** Idle-detection timer, started when a connection opens and stopped when it closes. */
  private idleTimer: ReturnType<typeof setInterval> | undefined;

  // ── E2EE (end-to-end encryption) state ──
  /** Pairing data for this visit (token + server public key); null selects unencrypted token auth. */
  private readonly pairing: Pairing | null = readPairing();
  /** E2EE second-factor password, injected by LoginGate and sent with deviceToken. */
  private password = "";
  /** Ephemeral client key pair, regenerated for every connection. */
  private clientKeys: nacl.BoxKeyPair | null = null;
  /** ECDH-derived shared key (`box.before` result); null until the handshake completes. */
  private sharedKey: Uint8Array | null = null;
  /** Whether the handshake has completed; all subsequent frames are encrypted. */
  private e2eeReady = false;
  /** Resolve/reject handlers for the active connection promise, called after the handshake. */
  private pendingConnect: { resolve: () => void; reject: (e: unknown) => void } | null = null;
  /** Session token returned after LoginGate succeeds. This is the web client's **only credential**;
   *  cookies are no longer used. sessionStorage keeps it private to each window/tab, survives a
   *  refresh, and clears when the window closes. Domain-wide cookies ignore ports and let windows
   *  overwrite one another, which caused new SSH windows to be rejected immediately. WebSocket
   *  connections append `?token=` because browsers cannot set custom WS headers; HTTP probes use
   *  the `Authorization` header. */
  private sessionToken: string | null = null;
  /** Whether goOnline has ever succeeded. wakeReconnect is enabled only afterward: focus/network
   *  wakeups are reconnect triggers and must never initiate the pre-login page's first connection,
   *  which would lack credentials and be rejected. */
  private everConnected = false;
  /** Whether the last connection URL included `?token=`, logged when auth fails for diagnostics. */
  private lastConnectHadToken = false;
  /** Requests waiting for a token. This is the token flow's **gate**: connection requests queue
   *  here until `setSessionToken` releases them, structurally preventing pre-login connections. */
  private tokenWaiters: Array<() => void> = [];

  constructor() {
    const proto = location.protocol === "https:" ? "wss" : "ws";
    this.url = `${proto}://${location.host}/ws`;
    // Restore this window's session token so a refresh does not require another login.
    try {
      this.sessionToken = sessionStorage.getItem(TOKEN_STORAGE_KEY);
    } catch {
      /* Storage is unavailable in rare environments; leave null and use the normal login flow. */
    }
    // Reconnect immediately on network/window wakeups; singleton listeners need no cleanup.
    if (typeof window !== "undefined") {
      // Network recovery is explicit, so reconnect unconditionally (force).
      window.addEventListener("online", () => this.wakeReconnect(true));
      // On window focus, reconnect only when suspicious to avoid disrupting healthy connections.
      window.addEventListener("focus", () => this.wakeReconnect(false));
      if (typeof document !== "undefined") {
        document.addEventListener("visibilitychange", () => {
          if (document.visibilityState === "visible") this.wakeReconnect(false);
        });
      }
    }
  }

  /** Whether URL-fragment pairing data selects E2EE mode; LoginGate then requests a password. */
  isPairingMode(): boolean {
    return this.pairing !== null;
  }

  /** Set the E2EE second-factor password collected by LoginGate for the ensure() handshake. */
  setPairingPassword(pw: string): void {
    this.password = pw;
  }

  /** Set the session token issued after login and persist it in this window's sessionStorage so
   *  refreshes do not require login. Then release queued requests so ensure() can connect with it. */
  setSessionToken(token: string): void {
    this.sessionToken = token;
    try {
      sessionStorage.setItem(TOKEN_STORAGE_KEY, token);
    } catch {
      /* Without storage, keep the token in memory and require login after a refresh. */
    }
    const waiters = this.tokenWaiters;
    this.tokenWaiters = [];
    for (const w of waiters) w();
  }

  /** Current session token, or null; used by LoginGate to probe whether login can be skipped. */
  getSessionToken(): string | null {
    return this.sessionToken;
  }

  /** Authenticated HTTP headers for token mode. A token always becomes `Authorization: Bearer`;
   *  cookies have been removed, so the token is the sole credential. */
  authHeaders(): Record<string, string> {
    return this.sessionToken ? { Authorization: `Bearer ${this.sessionToken}` } : {};
  }

  /** Encrypt an outbound text message and return a base64 frame containing nonce + ciphertext. */
  private encryptText(plaintext: string): string {
    const nonce = nacl.randomBytes(nacl.box.nonceLength);
    const ct = nacl.box.after(textEncoder.encode(plaintext), nonce, this.sharedKey!);
    const frame = new Uint8Array(nonce.length + ct.length);
    frame.set(nonce);
    frame.set(ct, nonce.length);
    return bytesToB64(frame);
  }

  /** Decrypt an inbound base64 text frame; return null on failure — including malformed base64,
   *  where atob throws: an invalid frame must degrade to the caller's null path (failHandshake
   *  during the handshake, frame discard afterwards) instead of throwing out of onMessage. */
  private decryptText(b64: string): string | null {
    let frame: Uint8Array;
    try {
      frame = b64ToBytes(b64);
    } catch {
      return null;
    }
    const bytes = this.decryptBytes(frame);
    return bytes ? textDecoder.decode(bytes) : null;
  }

  /** Decrypt an inbound binary frame containing nonce + ciphertext; return null on failure. */
  private decryptBytes(frame: Uint8Array): Uint8Array | null {
    if (!this.sharedKey || frame.length < nacl.box.nonceLength + nacl.box.overheadLength) {
      return null;
    }
    const nonce = frame.subarray(0, nacl.box.nonceLength);
    const ct = frame.subarray(nacl.box.nonceLength);
    return nacl.box.open.after(ct, nonce, this.sharedKey);
  }

  /** Ensure the socket is connected; concurrent callers share one connection promise. */
  private ensure(): Promise<void> {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) return Promise.resolve();
    if (this.connectPromise) return this.connectPromise;
    // Pairing mode must wait for its second-factor password. An empty-password handshake would be
    // rejected, causing endless reconnects and a false "wrong password" message before the user
    // can type one. Suspend this attempt without connecting, retrying, reporting, or caching it;
    // a later ensure call after setPairingPassword will connect normally.
    if (this.pairing && !this.password) {
      return new Promise<void>(() => {});
    }
    // Token gate: outside pairing mode, **never create a connection** without a token. Queue it
    // until setSessionToken releases it, mirroring the password gate above. This structurally
    // prevents premature pre-login connections that ensure() used to cache, which then failed the
    // entire batch of startup requests after App mounted. Keep a stack-bearing sentinel log so the
    // now-harmless early caller can still be found and fixed at its source.
    if (!this.pairing && !this.sessionToken) {
      const stack = (new Error().stack ?? "")
        .split("\n")
        .slice(1, 6)
        .map((l) => l.trim())
        .join(" <- ");
      recordRequestError(
        "ws:connect-no-token",
        `connection attempt before login was queued (harmless); initiator stack: ${stack}`,
      );
      return new Promise<void>((resolve, reject) => {
        this.tokenWaiters.push(() => {
          this.ensure().then(resolve, reject);
        });
      });
    }
    this.connectPromise = new Promise<void>((resolve, reject) => {
      this.pendingConnect = { resolve, reject };
      // The token is the sole credential, appended as `?token=` because browser WebSockets cannot
      // set custom headers. The gate above guarantees it exists here, except in pairing mode,
      // which authenticates through the E2EE handshake instead.
      const url = this.sessionToken
        ? `${this.url}?token=${encodeURIComponent(this.sessionToken)}`
        : this.url;
      this.lastConnectHadToken = this.sessionToken !== null;
      const ws = new WebSocket(url);
      ws.binaryType = "arraybuffer";
      ws.onopen = () => {
        this.ws = ws;
        if (this.pairing) {
          // E2EE flow: generate an ephemeral client key and send e2ee_hello in plaintext. Delay
          // goOnline until onMessage receives e2ee_authenticated.
          this.e2eeReady = false;
          this.sharedKey = null;
          this.clientKeys = nacl.box.keyPair();
          ws.send(
            JSON.stringify({
              type: "e2ee_hello",
              publicKeyB64: bytesToB64(this.clientKeys.publicKey),
            }),
          );
        } else {
          // Token flow: `?token=` was validated during the upgrade, so go online immediately.
          this.goOnline(ws);
        }
      };
      ws.onmessage = (ev) => this.onMessage(ev);
      ws.onclose = () => {
        this.ws = null;
        this.connectPromise = null;
        // Reset E2EE state so the next connection uses a fresh handshake and ephemeral key.
        this.e2eeReady = false;
        this.sharedKey = null;
        clearInterval(this.idleTimer);
        this.idleTimer = undefined;
        // Reject every pending request.
        for (const p of this.pending.values())
          p.reject(new TransportError(t("transport.wsDisconnected")));
        this.pending.clear();
        // Reconnect automatically with exponential backoff. A failed socket fires onclose after
        // onerror, so scheduling only here prevents duplicate timers.
        this.emitConnState("offline");
        this.scheduleReconnect();
      };
      ws.onerror = () => {
        // onclose normally follows onerror; only reject the initial ensure if it has not opened.
        if (this.connectPromise) {
          this.connectPromise = null;
          reject(new TransportError(t("transport.wsConnectFailed")));
        }
      };
    });
    return this.connectPromise;
  }

  /**
   * Bring the connection online: reset retries, start idle detection, reattach registered PTYs,
   * and resolve the connection promise. Token auth calls this from onopen; E2EE calls it after
   * the handshake receives `e2ee_authenticated`.
   */
  private goOnline(ws: WebSocket) {
    this.connectPromise = null;
    this.reconnectAttempts = 0;
    this.everConnected = true;
    this.emitConnState("online");
    // If no traffic arrives within IDLE_TIMEOUT_MS despite the server's 30s heartbeat, treat the
    // socket as half-open and close it so onclose can trigger automatic recovery.
    this.lastInbound = Date.now();
    clearInterval(this.idleTimer);
    this.idleTimer = setInterval(() => {
      if (Date.now() - this.lastInbound > IDLE_TIMEOUT_MS) {
        ws.close();
      }
    }, IDLE_CHECK_MS);
    // Recover by reattaching every registered PTY. attachOnly never revives a session closed by
    // another client during the outage as an empty shell; the server replays the mode prelude and
    // current screen.
    for (const sid of this.ptySinks.keys()) {
      const args = this.ptyArgs.get(sid);
      if (!args) continue;
      const startCbs = this.reattachStartCbs.get(sid);
      if (startCbs) for (const cb of startCbs) cb();
      void this.sendPtySpawn(sid, args, true)
        .then((res) => {
          const cbs = this.reattachCbs.get(sid);
          if (cbs) for (const cb of cbs) cb(res);
        })
        .catch((err) => {
          // An attach-only failure means another client closed or deleted the session while this
          // one was offline. Finish as a terminated session: remove local registrations and emit
          // an exit event so usePtySession closes the view.
          const msg = String(err instanceof Error ? err.message : err);
          if (msg.includes("is not running") || msg.includes("has been deleted")) {
            this.ptySinks.delete(sid);
            this.ptyArgs.delete(sid);
            this.subIds.delete(sid);
            this.dispatchLocalEvent(`pty://exit/${sid}`, null);
          }
        });
    }
    this.pendingConnect?.resolve();
    this.pendingConnect = null;
  }

  /** On E2EE decryption/auth failure, close the connection and send LoginGate back to login.
   *  `reason` carries the server's handshake error code so the UI can show a rate-limit message
   *  instead of misreporting throttling as a wrong password. */
  private failHandshake(reason?: HandshakeFailure) {
    this.pendingConnect?.reject(new TransportError(t("transport.wsConnectFailed")));
    this.pendingConnect = null;
    for (const cb of this.authLostCbs) cb(reason);
    this.ws?.close();
  }

  /** Broadcast connection state to all subscribers. */
  private emitConnState(state: ConnState) {
    for (const cb of this.connStateCbs) cb(state, this.reconnectAttempts);
  }

  /** Schedule one backoff retry unless a reconnect timer already exists. */
  private scheduleReconnect() {
    if (this.reconnectTimer) return;
    const delay = Math.min(
      RECONNECT_BASE_MS * 2 ** this.reconnectAttempts,
      RECONNECT_MAX_MS,
    );
    this.reconnectAttempts++;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = undefined;
      void this.tryReconnect();
    }, delay);
  }

  /**
   * Probe `/api/me` before each reconnect attempt to identify the failure:
   * - 401/403: credentials are stale, often after a restart clears in-memory tokens. Retrying
   *   `/ws` would always fail, so stop and return LoginGate to login; remounting reconnects the app.
   * - Network error: the server is unreachable, so keep retrying with backoff.
   * - Successful probe: reconnect normally; onclose schedules another round if it fails.
   */
  private async tryReconnect() {
    // Pairing mode has no session token, so /api/me always returns 401 and cannot diagnose it.
    // Reconnect and repeat the handshake directly: a valid in-memory password/token succeeds;
    // an expired token produces e2ee_error and invokes onAuthLost.
    if (this.pairing) {
      void this.ensure().catch(() => {});
      return;
    }
    try {
      const r = await fetch("/api/me", { headers: this.authHeaders() });
      if (r.status === 401 || r.status === 403) {
        for (const cb of this.authLostCbs) cb();
        return;
      }
    } catch {
      this.emitConnState("offline");
      this.scheduleReconnect();
      return;
    }
    void this.ensure().catch(() => {});
  }

  private onMessage(ev: MessageEvent) {
    this.lastInbound = Date.now();

    // ── E2EE handshake: plaintext e2ee_ready; encrypted e2ee_authenticated/e2ee_error ──
    if (this.pairing && !this.e2eeReady) {
      if (!this.sharedKey) {
        // Expect plaintext e2ee_ready, derive the shared key, and return encrypted e2ee_auth.
        let m: { type?: string };
        try {
          m = JSON.parse(ev.data as string);
        } catch {
          return;
        }
        if (m.type === "e2ee_ready" && this.clientKeys) {
          this.sharedKey = nacl.box.before(
            this.pairing.serverPub,
            this.clientKeys.secretKey,
          );
          const auth = JSON.stringify({
            type: "e2ee_auth",
            deviceToken: this.pairing.token,
            password: this.password,
            deviceId: getDeviceId(),
            deviceName: getDeviceName(),
          });
          this.ws?.send(this.encryptText(auth));
        }
      } else {
        // Expect encrypted e2ee_authenticated or e2ee_error.
        const plain =
          typeof ev.data === "string" ? this.decryptText(ev.data) : null;
        if (plain === null) {
          this.failHandshake();
          return;
        }
        let m: { type?: string; code?: string };
        try {
          m = JSON.parse(plain);
        } catch {
          return;
        }
        if (m.type === "e2ee_authenticated") {
          this.e2eeReady = true;
          if (this.ws) this.goOnline(this.ws);
        } else {
          // e2ee_error distinguishes server-side login throttling (code "rate_limited", sent
          // before any credential work) from an invalid device token or password; forward the
          // reason so LoginGate can show a rate-limit message instead of "wrong password".
          this.failHandshake(handshakeFailureReason(m));
        }
      }
      return;
    }

    // Binary frames carry PTY output; decrypt the entire frame in E2EE mode.
    if (ev.data instanceof ArrayBuffer) {
      const frame = new Uint8Array(ev.data);
      const view = this.e2eeReady ? this.decryptBytes(frame) : frame;
      if (!view) return; // Discard frames that cannot be decrypted.
      const sidLen = view[0];
      const sid = textDecoder.decode(view.subarray(1, 1 + sidLen));
      const payload = view.subarray(1 + sidLen);
      this.ptySinks.get(sid)?.(payload);
      return;
    }
    // Text frames carry JSON control messages; decrypt them first in E2EE mode.
    let raw: string;
    if (this.e2eeReady) {
      const dec = this.decryptText(ev.data as string);
      if (dec === null) return;
      raw = dec;
    } else {
      raw = ev.data as string;
    }
    let msg: Record<string, unknown>;
    try {
      msg = JSON.parse(raw);
    } catch {
      return;
    }
    if (msg.t === "ping") {
      // Reply to the server heartbeat so it knows this client is alive.
      this.send({ t: "pong" });
    } else if (msg.t === "hello") {
      // The first frame supplies this connection's source ID; replace it after reconnection.
      this.source = String(msg.source ?? "");
    } else if (msg.t === "reply") {
      const p = this.pending.get(msg.id as number);
      if (!p) return;
      this.pending.delete(msg.id as number);
      if (msg.ok) p.resolve(msg.result);
      // Stable backend gating codes (remote_*_forbidden) are localized here, at the single point
      // where every backend error enters the UI; other errors pass through unchanged.
      else p.reject(new Error(mapBackendError(String(msg.error ?? t("transport.cmdFailed")))));
    } else if (msg.t === "event") {
      const subs = this.events.get(msg.name as string);
      if (subs) for (const cb of subs) cb(msg.payload);
    } else if (msg.t === "error") {
      // The server reports connection-level failures such as rejected WS authentication before
      // closing the socket. Record them in the central error buffer for console/banner visibility;
      // previously the server closed silently. Include whether this URL carried a token so it can
      // be correlated with the token fingerprint in server logs.
      recordRequestError(
        `ws:${String(msg.code ?? "error")}`,
        `${String(msg.message ?? "server rejected the connection")} (client sent token=${this.lastConnectHadToken ? "yes" : "no"})`,
      );
    }
  }

  private send(obj: unknown) {
    const text = JSON.stringify(obj);
    // Encrypt all outbound text after E2EE is ready. Handshake hello/auth frames bypass this path.
    this.ws?.send(this.e2eeReady ? this.encryptText(text) : text);
  }

  /** Dispatch an event locally through the same path as server events, used after reattach failure. */
  private dispatchLocalEvent(name: string, payload: unknown) {
    const subs = this.events.get(name);
    if (subs) for (const cb of subs) cb(payload);
  }

  /** Send an ID-bearing request and return its response promise. */
  private async request(partial: Record<string, unknown>): Promise<unknown> {
    await this.ensure();
    const id = this.nextId++;
    return new Promise<unknown>((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.send({ ...partial, id });
    });
  }

  // ─────────────────────────── Public API ───────────────────────────

  async invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    return (await this.request({ t: "invoke", cmd, args: args ?? {} })) as T;
  }

  async listen<T>(name: string, cb: (payload: T) => void): Promise<UnlistenFn> {
    await this.ensure();
    let set = this.events.get(name);
    if (!set) {
      set = new Set();
      this.events.set(name, set);
    }
    const wrapped: EventCb = (p) => cb(p as T);
    set.add(wrapped);
    return () => {
      set?.delete(wrapped);
    };
  }

  /** Send pty-spawn. `attachOnly` reattaches without starting; see the onopen comments. */
  private async sendPtySpawn(
    sid: string,
    args: PtySpawnArgs,
    attachOnly = false,
  ): Promise<PtySpawnResult> {
    const res = (await this.request(
      attachOnly
        ? { t: "pty-spawn", sid, args, attachOnly: true }
        : { t: "pty-spawn", sid, args },
    )) as {
      pid: number;
      launch: string | null;
      subId: number;
      attached: boolean;
      cols: number;
      rows: number;
      owner: string | null;
    };
    this.subIds.set(sid, res.subId);
    return {
      pid: res.pid,
      launch: res.launch,
      attached: res.attached,
      cols: res.cols,
      rows: res.rows,
      owner: res.owner,
    };
  }

  async spawnPty(args: PtySpawnArgs, onBytes: ByteCb): Promise<PtySpawnResult> {
    // Register the sink before spawning so an immediate initial replay cannot be missed.
    this.ptySinks.set(args.sessionId, onBytes);
    this.ptyArgs.set(args.sessionId, args);
    return this.sendPtySpawn(args.sessionId, args);
  }

  /** Leave the session locally: detach without killing the process, then clear registrations. */
  teardownPty(sid: string) {
    const subId = this.subIds.get(sid);
    this.send({ t: "pty-detach", sid, subId });
    this.ptySinks.delete(sid);
    this.ptyArgs.delete(sid);
    this.subIds.delete(sid);
    this.reattachCbs.delete(sid);
    this.reattachStartCbs.delete(sid);
  }

  /** Register an auth-expiry callback for /api/me 401 responses and E2EE handshake failures; the
   *  optional reason reports server-side rate limiting. Returns an unsubscribe function. */
  onAuthLost(cb: (reason?: HandshakeFailure) => void): () => void {
    this.authLostCbs.add(cb);
    return () => {
      this.authLostCbs.delete(cb);
    };
  }

  /** Register a connection-state callback with retry count; returns an unsubscribe function. */
  onConnState(cb: (state: ConnState, attempts: number) => void): () => void {
    this.connStateCbs.add(cb);
    return () => {
      this.connStateCbs.delete(cb);
    };
  }

  /**
   * Reconnect now: cancel the pending backoff timer and attempt reconnection immediately.
   * Used by the disconnect banner. Do not reset the retry count, so the banner remains visible
   * and future automatic backoff keeps its cadence; this only brings the next attempt forward.
   * Do nothing when already connected.
   */
  reconnectNow(): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) return;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = undefined;
    }
    void this.tryReconnect();
  }

  /**
   * Force an immediate reconnect by closing the current socket even if it still reports OPEN.
   * Use when the underlying channel has been replaced: after rebuilding an SSH tunnel, the old
   * WebSocket TCP connection still traverses the dead tunnel and only appears open. Forced
   * reconnection recovers immediately instead of waiting ~75s for idle detection.
   */
  forceReconnect(): void {
    this.wakeReconnect(true);
  }

  /**
   * Reconnect immediately after a network/window wakeup. `force` (the online event) reconnects
   * unconditionally; focus/visibility changes act only when the socket looks OPEN but has been
   * silent long enough to suggest a half-open connection. Because reconnectNow would return early
   * for an OPEN socket, close it first so onclose performs normal offline scheduling, then cancel
   * that backoff and attempt reconnection immediately.
   */
  private wakeReconnect(force: boolean): void {
    // There is nothing to reconnect before the first successful connection. Focus/network events
    // also fire on the login page (a new window is immediately focused), when credentials are not
    // ready and an early first connection would be rejected. Only a real post-mount request such
    // as invoke may initiate that first connection.
    if (!this.everConnected) return;
    const open = this.ws !== null && this.ws.readyState === WebSocket.OPEN;
    if (open && !force && Date.now() - this.lastInbound < WAKE_STALE_MS) return;
    if (open) this.ws!.close();
    this.reconnectNow();
  }

  /** Source identifier supplied by the hello frame; null before connection. */
  getSource(): string | null {
    return this.source;
  }

  /** Register a callback for reconnect + attach replay completion with the latest respawn result. */
  onReattach(sid: string, cb: (res: PtySpawnResult) => void): () => void {
    let set = this.reattachCbs.get(sid);
    if (!set) {
      set = new Set();
      this.reattachCbs.set(sid, set);
    }
    set.add(cb);
    return () => {
      set?.delete(cb);
    };
  }

  /** Register a callback fired after reconnection and immediately before resending pty-spawn. */
  onReattachStart(sid: string, cb: () => void): () => void {
    let set = this.reattachStartCbs.get(sid);
    if (!set) {
      set = new Set();
      this.reattachStartCbs.set(sid, set);
    }
    set.add(cb);
    return () => {
      set?.delete(cb);
    };
  }
}

/** Browser-side singleton. */
export const wsClient = new WsClient();
