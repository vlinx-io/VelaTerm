//! Coverage for how a session's death announcement is read.
//!
//! `pty://killed` used to carry nothing, so a client could only guess why the session died — and
//! guessing "gone" meant restarting a session closed its tab on every other client. The payload now says
//! who asked and whether the session is coming back; these tests pin how a missing or unknown value is
//! read, because that is what an older backend sends.

import { describe, expect, it, vi } from "vitest";

const { listen } = vi.hoisted(() => ({ listen: vi.fn() }));
vi.mock("./transport", () => ({
  listen,
  listenNative: vi.fn(),
  isRemoteWindow: false,
}));

import { onPtyKilled } from "./events";

/** Register a listener, then hand it `payload` the way the transport would. */
function deliver(payload: unknown) {
  const seen: { source: string; reason: string }[] = [];
  listen.mockImplementation((_name: string, cb: (p: unknown) => void) => {
    cb(payload);
    return Promise.resolve(() => {});
  });
  void onPtyKilled("s1", (ev) => seen.push(ev));
  return seen[0];
}

describe("reading a kill announcement", () => {
  it("passes through who asked and why", () => {
    expect(deliver({ source: "ws-3", reason: "restart" })).toEqual({
      source: "ws-3",
      reason: "restart",
    });
  });

  it("reads an empty announcement as a close", () => {
    // Keeping a pane for a session that never comes back leaves a dead terminal on screen, which is
    // worse than closing one the user meant to restart. An older backend sends exactly this.
    expect(deliver(null)).toEqual({ source: "", reason: "close" });
  });

  it("reads an unknown reason as a close", () => {
    expect(deliver({ source: "desktop", reason: "who-knows" }).reason).toBe("close");
  });
});
