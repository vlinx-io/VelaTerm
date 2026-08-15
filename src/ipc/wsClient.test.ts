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

  it("reports an undecryptable handshake frame without a reason (credential-failure path)", () => {
    setupHandshake();
    const reasons: Array<string | undefined> = [];
    const unsubscribe = wsClient.onAuthLost((reason) => reasons.push(reason));
    // Valid base64, but random bytes that no key decrypts (decryptText returns null).
    internals.onMessage({ data: bytesToB64(nacl.randomBytes(64)) });
    unsubscribe();
    expect(reasons).toEqual([undefined]);
  });

  it("treats a handshake frame with INVALID base64 as a failed handshake instead of throwing", () => {
    setupHandshake();
    const reasons: Array<string | undefined> = [];
    const unsubscribe = wsClient.onAuthLost((reason) => reasons.push(reason));
    // atob throws on this input; decryptText must catch it and degrade to the null path, which the
    // handshake handler maps to failHandshake — never an uncaught exception out of onMessage.
    expect(() => internals.onMessage({ data: "%%%not-base64%%%" })).not.toThrow();
    unsubscribe();
    expect(reasons).toEqual([undefined]);
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
