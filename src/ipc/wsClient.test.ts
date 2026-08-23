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
