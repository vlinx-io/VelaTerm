//! Covers how clipboard reads behave when the source stalls. A read that went ahead anyway would leave a
//! stuck read inside the app, and a later main-thread copy or paste would then freeze every window.

import { beforeEach, describe, expect, it, vi } from "vitest";
import { ClipboardUnresponsiveError, onClipboardUnresponsive } from "./clipboardStall";

const { invoke, readText, navigatorRead } = vi.hoisted(() => ({
  invoke: vi.fn(),
  readText: vi.fn(),
  navigatorRead: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({ readText }));
vi.mock("./env", () => ({ env: { isTauri: true, hasNativeHost: true, isElectron: false } }));

beforeEach(() => {
  invoke.mockReset();
  readText.mockReset().mockResolvedValue("pasted");
  navigatorRead.mockReset();
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { readText: navigatorRead },
  });
});

async function tauriClipboard() {
  const { tauriPlatform } = await import("./tauri");
  return tauriPlatform.clipboard;
}

describe("clipboard reads on a local window", () => {
  it("read the clipboard once it is ready", async () => {
    invoke.mockResolvedValue(true);
    await expect((await tauriClipboard()).readText()).resolves.toBe("pasted");
    expect(invoke).toHaveBeenCalledWith("clipboard_wait_ready");
  });

  it("fail and notify without touching the clipboard when the source stalls", async () => {
    invoke.mockResolvedValue(false);
    const notified = vi.fn();
    const unsubscribe = onClipboardUnresponsive(notified);
    await expect((await tauriClipboard()).readText()).rejects.toBeInstanceOf(ClipboardUnresponsiveError);
    unsubscribe();
    expect(notified).toHaveBeenCalledTimes(1);
    expect(readText).not.toHaveBeenCalled();
    expect(navigatorRead).not.toHaveBeenCalled();
  });

  it("still read when the readiness check itself is unavailable", async () => {
    invoke.mockRejectedValue(new Error("command not allowed"));
    await expect((await tauriClipboard()).readText()).resolves.toBe("pasted");
  });
});
