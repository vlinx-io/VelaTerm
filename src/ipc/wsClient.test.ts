//! Pins the two production glue points that route backend gating errors into the UI (they were only
//! covered as pure functions before, so removing the wiring would not fail any test):
//! - the E2EE handshake handler forwards the server's `e2ee_error` code as the onAuthLost reason
//!   (rate_limited vs. credential failure), and
//! - a reply rejection maps stable `remote_*_forbidden:` codes through mapBackendError before the
//!   invoke promise rejects.
//! The tests drive the real private onMessage handler with real tweetnacl E2EE frames; only i18n
//! (echoing keys for locale independence) and the request-error log are mocked.

import nacl from "tweetnacl";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../i18n", () => ({
  // Echo key and argument so assertions are locale-independent.
  t: (key: string, arg?: string) => (arg === undefined ? key : `${key}|${arg}`),
}));
vi.mock("./reqLog", () => ({ recordRequestError: vi.fn() }));

import { bytesToB64, wsClient } from "./wsClient";

// The handler and its state are private by TypeScript convention only; the tests reach through on
// purpose to drive the real code path without a live WebSocket server.
type WsClientInternals = {
  pairing: { token: string; serverPub: Uint8Array } | null;
  sharedKey: Uint8Array | null;
  e2eeReady: boolean;
  pending: Map<number, { resolve: (v: unknown) => void; reject: (e: Error) => void }>;
  onMessage: (ev: { data: unknown }) => void;
};
const internals = wsClient as unknown as WsClientInternals;

/** Encrypt a handshake frame exactly like the server does: nonce + box.after ciphertext, base64. */
function encryptFrame(sharedKey: Uint8Array, payload: object): string {
  const nonce = nacl.randomBytes(nacl.box.nonceLength);
  // Copy into a fresh Uint8Array: jsdom's TextEncoder returns a Uint8Array from another realm,
  // which tweetnacl's instanceof check rejects.
  const plaintext = new Uint8Array(new TextEncoder().encode(JSON.stringify(payload)));
  const ct = nacl.box.after(plaintext, nonce, sharedKey);
  const frame = new Uint8Array(nonce.length + ct.length);
  frame.set(nonce);
  frame.set(ct, nonce.length);
  return bytesToB64(frame);
}

afterEach(() => {
  internals.pairing = null;
  internals.sharedKey = null;
  internals.e2eeReady = false;
  internals.pending.clear();
});

describe("wsClient E2EE handshake failure wiring", () => {
  function setupHandshake(): Uint8Array {
    const sharedKey = nacl.randomBytes(nacl.box.sharedKeyLength);
    internals.pairing = { token: "device-token", serverPub: new Uint8Array(32) };
    internals.sharedKey = sharedKey;
    internals.e2eeReady = false;
    return sharedKey;
  }

  it("forwards a rate_limited e2ee_error to onAuthLost callbacks as the reason", () => {
    const sharedKey = setupHandshake();
    const reasons: Array<string | undefined> = [];
    const unsubscribe = wsClient.onAuthLost((reason) => reasons.push(reason));
    internals.onMessage({
      data: encryptFrame(sharedKey, { type: "e2ee_error", code: "rate_limited" }),
    });
    unsubscribe();
    expect(reasons).toEqual(["rate_limited"]);
  });

  it("reports an e2ee_error without a rate-limit code as unauthorized", () => {
    const sharedKey = setupHandshake();
    const reasons: Array<string | undefined> = [];
    const unsubscribe = wsClient.onAuthLost((reason) => reasons.push(reason));
    internals.onMessage({
      data: encryptFrame(sharedKey, { type: "e2ee_error", code: "invalid_credentials" }),
    });
    unsubscribe();
    expect(reasons).toEqual(["unauthorized"]);
  });

  it("does NOT report auth loss for an undecryptable handshake frame", () => {
    setupHandshake();
    const reasons: Array<string | undefined> = [];
    const unsubscribe = wsClient.onAuthLost((reason) => reasons.push(reason));
    // Valid base64, but random bytes that no key decrypts (decryptText returns null). This says
    // nothing about the credentials, so it must close and retry rather than strand the window on
    // the terminal "wrong password or invalid link" page.
    internals.onMessage({ data: bytesToB64(nacl.randomBytes(64)) });
    unsubscribe();
    expect(reasons).toEqual([]);
  });

  it("swallows a handshake frame with INVALID base64 instead of throwing or reporting auth loss", () => {
    setupHandshake();
    const reasons: Array<string | undefined> = [];
    const unsubscribe = wsClient.onAuthLost((reason) => reasons.push(reason));
    // atob throws on this input; decryptText must catch it and degrade to the same close-and-retry
    // path — never an uncaught exception out of onMessage, never a credential verdict.
    expect(() => internals.onMessage({ data: "%%%not-base64%%%" })).not.toThrow();
    unsubscribe();
    expect(reasons).toEqual([]);
  });
});

describe("wsClient reply rejection error mapping", () => {
  it("maps stable remote_cmd_forbidden codes to the localized message before rejecting", async () => {
    let rejected: Error | undefined;
    const promise = new Promise<unknown>((resolve, reject) => {
      internals.pending.set(7, { resolve, reject });
    }).catch((e: Error) => {
      rejected = e;
    });
    internals.onMessage({
      data: JSON.stringify({ t: "reply", id: 7, ok: false, error: "remote_cmd_forbidden:web_server_start" }),
    });
    await promise;
    expect(rejected?.message).toBe("transport.remoteCmdForbidden|web_server_start");
  });

  it("passes unrecognized backend errors through unchanged", async () => {
    let rejected: Error | undefined;
    const promise = new Promise<unknown>((resolve, reject) => {
      internals.pending.set(8, { resolve, reject });
    }).catch((e: Error) => {
      rejected = e;
    });
    internals.onMessage({
      data: JSON.stringify({ t: "reply", id: 8, ok: false, error: "plain backend failure" }),
    });
    await promise;
    expect(rejected?.message).toBe("plain backend failure");
  });
});

/** Minimal WebSocket stand-in: `close()` only moves to CLOSING, exactly like the browser, so the
 *  overlap window between a superseded socket and its `onclose` is reproducible. */
class FakeWebSocket {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  binaryType = "";
  readyState: number = FakeWebSocket.CONNECTING;
  closeCalls = 0;
  sent: unknown[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  constructor(readonly url: string) {
    created.push(this);
  }
  send(data: unknown) {
    this.sent.push(data);
  }
  close() {
    this.closeCalls++;
    this.readyState = FakeWebSocket.CLOSING;
  }
}
let created: FakeWebSocket[] = [];

type ReconnectInternals = WsClientInternals & {
  ws: FakeWebSocket | null;
  password: string;
  everConnected: boolean;
  connectPromise: Promise<void> | null;
  lastInbound: number;
  clientKeys: { publicKey: Uint8Array; secretKey: Uint8Array } | null;
  ensure: () => Promise<void>;
};
const reconnect = wsClient as unknown as ReconnectInternals;

describe("wsClient superseded-socket isolation", () => {
  const realWebSocket = globalThis.WebSocket;

  afterEach(() => {
    globalThis.WebSocket = realWebSocket;
    created = [];
    reconnect.ws = null;
    reconnect.password = "";
    reconnect.everConnected = false;
    reconnect.connectPromise = null;
    reconnect.clientKeys = null;
  });

  /** Drive a pairing-mode connection to the state a forced reconnect starts from: socket open, its
   *  connect promise already settled, and inbound traffic stale enough to look half-open. */
  function connectFirstSocket(): FakeWebSocket {
    globalThis.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    reconnect.pairing = { token: "device-token", serverPub: new Uint8Array(32) };
    reconnect.password = "pw";
    void reconnect.ensure();
    const a = created[0];
    a.readyState = FakeWebSocket.OPEN;
    a.onopen?.();
    // goOnline's observable effects for this test; the handshake itself is covered above.
    reconnect.connectPromise = null;
    reconnect.everConnected = true;
    reconnect.lastInbound = 0;
    return a;
  }

  it("hands ownership to the replacement socket and ignores the old one's late callbacks", () => {
    const a = connectFirstSocket();
    // A forced reconnect closes A and immediately builds B, while A is still CLOSING and can still
    // deliver queued frames — the overlap that used to corrupt the shared E2EE keys.
    wsClient.forceReconnect();
    const b = created[1];
    expect(b).toBeDefined();
    expect(a.closeCalls).toBe(1);
    expect(reconnect.ws).toBe(b);

    // B negotiates its own key pair; a late frame on A must not consume or overwrite it.
    b.readyState = FakeWebSocket.OPEN;
    b.onopen?.();
    const keysB = reconnect.clientKeys;
    expect(keysB).not.toBeNull();
    a.onmessage?.({ data: JSON.stringify({ type: "e2ee_ready" }) });
    expect(reconnect.sharedKey).toBeNull();
    expect(reconnect.clientKeys).toBe(keysB);

    // A's close arrives last and must not disown B or reset its handshake state.
    a.readyState = FakeWebSocket.CLOSED;
    a.onclose?.();
    expect(reconnect.ws).toBe(b);
  });

  it("fails the superseded socket's in-flight requests instead of leaving them pending", async () => {
    connectFirstSocket();
    let rejected: Error | undefined;
    const inflight = new Promise<unknown>((resolve, reject) => {
      reconnect.pending.set(11, { resolve, reject });
    }).catch((e: Error) => {
      rejected = e;
    });
    wsClient.forceReconnect();
    await inflight;
    expect(rejected).toBeDefined();
    expect(reconnect.pending.size).toBe(0);
  });
});

type PtyInternals = ReconnectInternals & {
  ptySinks: Map<string, (bytes: Uint8Array) => void>;
  ptyArgs: Map<string, unknown>;
  subIds: Map<string, number>;
  reattachCbs: Map<string, Set<unknown>>;
  reattachStartCbs: Map<string, Set<unknown>>;
  idleTimer: ReturnType<typeof setInterval> | undefined;
  goOnline: (ws: FakeWebSocket) => void;
};
const ptyInternals = wsClient as unknown as PtyInternals;

describe("wsClient pty-resync", () => {
  const realWebSocket = globalThis.WebSocket;
  let socket: FakeWebSocket;

  /** An open plaintext connection with one registered terminal, as usePtySession leaves it. */
  function openWithTerminal(sid: string) {
    globalThis.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    socket = new FakeWebSocket("ws://test/ws");
    socket.readyState = FakeWebSocket.OPEN;
    ptyInternals.ws = socket;
    ptyInternals.ptySinks.set(sid, () => {});
    ptyInternals.ptyArgs.set(sid, { sessionId: sid, kind: "shell", cols: 80, rows: 24 });
  }

  /** The pty-spawn frames the client has sent so far. */
  function sentSpawns(): Array<Record<string, unknown>> {
    return socket.sent
      .map((raw) => JSON.parse(String(raw)) as Record<string, unknown>)
      .filter((msg) => msg.t === "pty-spawn");
  }

  afterEach(() => {
    clearInterval(ptyInternals.idleTimer);
    ptyInternals.idleTimer = undefined;
    globalThis.WebSocket = realWebSocket;
    created = [];
    ptyInternals.ws = null;
    ptyInternals.ptySinks.clear();
    ptyInternals.ptyArgs.clear();
    ptyInternals.subIds.clear();
    ptyInternals.reattachCbs.clear();
    ptyInternals.reattachStartCbs.clear();
    ptyInternals.pending.clear();
  });

  it("resets the terminal and reattaches it attach-only when the server resyncs it", async () => {
    openWithTerminal("s1");
    const events: string[] = [];
    wsClient.onReattachStart("s1", () => events.push("start"));
    wsClient.onReattach("s1", (res) => events.push(`reattached:${res.pid}`));

    internals.onMessage({ data: JSON.stringify({ t: "pty-resync", sid: "s1" }) });
    expect(events).toEqual(["start"]);
    await vi.waitFor(() => expect(sentSpawns()).toHaveLength(1));
    const spawn = sentSpawns()[0];
    expect(spawn.sid).toBe("s1");
    expect(spawn.attachOnly).toBe(true);

    internals.onMessage({
      data: JSON.stringify({
        t: "reply",
        id: spawn.id,
        ok: true,
        result: { pid: 9, launch: null, subId: 4, attached: true, cols: 80, rows: 24, owner: null },
      }),
    });
    await vi.waitFor(() => expect(events).toEqual(["start", "reattached:9"]));
    expect(ptyInternals.subIds.get("s1")).toBe(4);
  });

  it("ignores a resync for a terminal this client no longer has", async () => {
    openWithTerminal("s1");
    const starts: string[] = [];
    wsClient.onReattachStart("s1", () => starts.push("s1"));
    internals.onMessage({ data: JSON.stringify({ t: "pty-resync", sid: "gone" }) });
    await Promise.resolve();
    expect(starts).toEqual([]);
    expect(sentSpawns()).toEqual([]);
  });

  it("still reattaches every registered terminal when the connection comes online", async () => {
    openWithTerminal("s1");
    const starts: string[] = [];
    wsClient.onReattachStart("s1", () => starts.push("s1"));
    ptyInternals.goOnline(socket);
    expect(starts).toEqual(["s1"]);
    await vi.waitFor(() => expect(sentSpawns()).toHaveLength(1));
    expect(sentSpawns()[0]).toMatchObject({ t: "pty-spawn", sid: "s1", attachOnly: true });
  });
});

type ChatInternals = PtyInternals & {
  events: Map<string, Set<unknown>>;
  chatResyncing: Set<string>;
};
const chatInternals = wsClient as unknown as ChatInternals;

describe("wsClient chat channels", () => {
  const realWebSocket = globalThis.WebSocket;
  let socket: FakeWebSocket;

  function open() {
    globalThis.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    socket = new FakeWebSocket("ws://test/ws");
    socket.readyState = FakeWebSocket.OPEN;
    chatInternals.ws = socket;
  }
  const sent = () => socket.sent.map((raw) => JSON.parse(String(raw)) as Record<string, unknown>);
  const event = (name: string, payload: unknown) =>
    internals.onMessage({ data: JSON.stringify({ t: "event", name, payload }) });

  afterEach(() => {
    clearInterval(chatInternals.idleTimer);
    chatInternals.idleTimer = undefined;
    globalThis.WebSocket = realWebSocket;
    chatInternals.ws = null;
    chatInternals.events.clear();
    chatInternals.chatResyncing.clear();
    internals.onMessage({ data: JSON.stringify({ t: "hello", source: "ws-reset" }) });
  });

  it("mirrors the first and last local listener of a chat name with watch and unwatch", async () => {
    open();
    const offA = await wsClient.listen("chat://event/s", () => {});
    const offB = await wsClient.listen("chat://event/s", () => {});
    const offTree = await wsClient.listen("tree://changed", () => {});
    expect(sent()).toEqual([{ t: "watch", name: "chat://event/s" }]);
    offA();
    offA();
    expect(sent()).toHaveLength(1);
    offB();
    offTree();
    expect(sent()).toEqual([{ t: "watch", name: "chat://event/s" }, { t: "unwatch", name: "chat://event/s" }]);
    expect(chatInternals.events.has("chat://event/s")).toBe(false);
  });

  it("announces the codec first and watches the listened chat names again when it comes online", async () => {
    open();
    await wsClient.listen("chat://event/s", () => {});
    await wsClient.listen("chat://task/s/t1", () => {});
    const off = await wsClient.listen("chat://event/gone", () => {});
    off();
    await wsClient.listen("spawn://request", () => {});
    socket.sent = [];
    chatInternals.goOnline(socket);
    expect(sent()).toEqual([
      { t: "caps", chat: 1 },
      { t: "watch", name: "chat://event/s" },
      { t: "watch", name: "chat://task/s/t1" },
    ]);
  });

  it("hands listeners the decoded payload, not the patch", async () => {
    open();
    const seen: unknown[] = [];
    await wsClient.listen("chat://event/s", (payload) => seen.push(payload));
    event("chat://event/s", { type: "rows", epoch: 1, retain: ["r"], rows: [{ kind: "assistant", id: "r", text: "Hel", streaming: true }] });
    event("chat://event/s", { type: "rows", epoch: 1, retain: ["r"], rows: [{ id: "r", patch: { app: { text: "lo" }, len: { text: 3 } } }] });
    expect(seen[1]).toEqual({ type: "rows", epoch: 1, rows: [{ kind: "assistant", id: "r", text: "Hello", streaming: true }] });
  });

  it("drops a session's events after a gap until the resync barrier, which reaches the listeners", async () => {
    open();
    const seen: Array<{ type?: string }> = [];
    await wsClient.listen("chat://event/s", (payload) => seen.push(payload as { type?: string }));
    const other: unknown[] = [];
    await wsClient.listen("chat://event/o", (payload) => other.push(payload));
    socket.sent = [];
    event("chat://event/s", { type: "rows", epoch: 1, retain: ["r"], rows: [{ kind: "reasoning", id: "r", text: "abc", streaming: true }] });
    // A tampered base length: the decoder must not guess.
    event("chat://event/s", { type: "rows", epoch: 1, retain: ["r"], rows: [{ id: "r", patch: { app: { text: "d" }, len: { text: 2 } } }] });
    expect(sent()).toEqual([{ t: "chat-resync", sid: "s" }]);
    event("chat://event/s", { type: "queued", items: [{ id: "q", text: "late" }] });
    event("chat://event/s", { type: "rows", epoch: 1, rows: [{ kind: "user", id: "u", text: "full" }] });
    event("chat://event/o", { type: "queued", items: [] });
    expect(seen.map((payload) => payload.type)).toEqual(["rows"]);
    expect(other, "other sessions are unaffected").toHaveLength(1);
    event("chat://event/s", { type: "resync" });
    event("chat://event/s", { type: "rows", epoch: 1, retain: ["r"], rows: [{ kind: "reasoning", id: "r", text: "abcd", streaming: true }] });
    expect(seen.map((payload) => payload.type)).toEqual(["rows", "resync", "rows"]);
    expect(sent(), "one resync request per gap").toHaveLength(1);
  });

  it("drops an undecodable event silently when nothing listens to the session", () => {
    open();
    event("chat://event/s", { type: "extras", patch: { set: { fastMode: true } } });
    expect(sent()).toEqual([]);
  });
});

describe("wsClient E2EE online gate", () => {
  const realWebSocket = globalThis.WebSocket;
  let socket: FakeWebSocket;
  // Stand-ins for the cipher: this suite pins which frames are sent and whether they are encrypted, not the
  // cryptography, which the handshake tests above drive for real.
  type Cipher = { encryptText: (text: string) => string; decryptText: (b64: string) => string | null };
  const cipher = wsClient as unknown as Cipher;
  const real: Cipher = { encryptText: cipher.encryptText, decryptText: cipher.decryptText };

  /** A pairing connection whose socket is open but whose handshake has only sent `e2ee_hello`. */
  function openHandshaking() {
    globalThis.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    cipher.encryptText = (text) => `enc:${text}`;
    cipher.decryptText = (b64) => (b64.startsWith("enc:") ? b64.slice(4) : null);
    chatInternals.pairing = { token: "device-token", serverPub: nacl.box.keyPair().publicKey };
    chatInternals.password = "pw";
    void chatInternals.ensure();
    socket = created[created.length - 1];
    socket.readyState = FakeWebSocket.OPEN;
    socket.onopen?.();
    socket.sent = [];
  }

  /** Every frame sent so far: encrypted ones decoded, plaintext ones flagged. */
  const frames = () => socket.sent.map((raw) => {
    const text = String(raw);
    return text.startsWith("enc:") ? (JSON.parse(text.slice(4)) as Record<string, unknown>) : { plaintext: text };
  });

  afterEach(() => {
    cipher.encryptText = real.encryptText;
    cipher.decryptText = real.decryptText;
    clearInterval(chatInternals.idleTimer);
    chatInternals.idleTimer = undefined;
    globalThis.WebSocket = realWebSocket;
    created = [];
    chatInternals.ws = null;
    chatInternals.password = "";
    chatInternals.connectPromise = null;
    chatInternals.clientKeys = null;
    chatInternals.events.clear();
  });

  it("sends no application frame while the E2EE handshake runs, and everything encrypted once online", async () => {
    openHandshaking();
    // What the UI does at any moment, a phone unlock included: a view listens, one stops listening, a
    // terminal is left, a command is invoked. None of it may reach the socket before the handshake ends.
    let listening = false;
    const listened = wsClient.listen("chat://task/s/t1", () => {}).then(() => { listening = true; });
    (wsClient as unknown as { send: (frame: unknown) => void }).send({ t: "unwatch", name: "chat://event/old" });
    wsClient.teardownPty("pty-1");
    const invoked = wsClient.invoke("list_tree").catch(() => {});
    await Promise.resolve();
    await Promise.resolve();
    expect(socket.sent, "nothing leaves during the handshake").toEqual([]);
    expect(listening, "listen waits for the handshake").toBe(false);

    socket.onmessage?.({ data: JSON.stringify({ type: "e2ee_ready" }) });
    expect(frames().map((frame) => frame.type)).toEqual(["e2ee_auth"]);
    socket.onmessage?.({ data: `enc:${JSON.stringify({ type: "e2ee_authenticated" })}` });
    await listened;
    await vi.waitFor(() => expect(frames().some((frame) => frame.t === "invoke")).toBe(true));
    const sent = frames();
    expect(sent.some((frame) => "plaintext" in frame), "every frame is encrypted").toBe(false);
    expect(sent.map((frame) => frame.t ?? frame.type)).toEqual(["e2ee_auth", "caps", "watch", "invoke"]);
    expect(sent[2]).toEqual({ t: "watch", name: "chat://task/s/t1" });
    // The pending invoke is answered by nobody here; fail it so the promise settles.
    chatInternals.pending.forEach((p) => p.reject(new Error("done")));
    await invoked;
  });
});

describe("wsClient watch rejection", () => {
  const realWebSocket = globalThis.WebSocket;

  afterEach(() => {
    globalThis.WebSocket = realWebSocket;
    chatInternals.ws = null;
    chatInternals.events.clear();
  });

  it("hands a rejected watch to the listeners of exactly that name", async () => {
    globalThis.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
    const socket = new FakeWebSocket("ws://test/ws");
    socket.readyState = FakeWebSocket.OPEN;
    chatInternals.ws = socket;
    const task: unknown[] = [];
    const other: unknown[] = [];
    await wsClient.listen("chat://task/s/t1", (payload) => task.push(payload));
    await wsClient.listen("chat://task/s/t2", (payload) => other.push(payload));
    internals.onMessage({ data: JSON.stringify({ t: "watch-rejected", name: "chat://task/s/t1" }) });
    expect(task).toEqual([{ type: "watchRejected" }]);
    expect(other).toEqual([]);
  });
});
