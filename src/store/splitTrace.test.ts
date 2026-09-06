import { afterEach, expect, it, vi } from "vitest";

const { invokeNative } = vi.hoisted(() => ({ invokeNative: vi.fn().mockResolvedValue(true) }));
// invokeNative reaches the process hosting this window, so a remote window logs to its local desktop app
// rather than to the server it displays; that routing belongs to the transport adapter.
vi.mock("../ipc/transport", () => ({ invokeNative }));

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  vi.clearAllMocks();
  vi.resetModules();
});

it("persists structural identifiers only, never display text", async () => {
  vi.stubGlobal("window", { __TAURI_INTERNALS__: {}, __VLX_FORCE_BROWSER__: true });
  const { traceSplit, splitTrace } = await import("./splitTrace");
  traceSplit("menu", "display-only context", {
    sessionIds: ["eph-1"], parentSessionId: "session-1", tabId: "tab-1", direction: "horizontal",
  });
  await vi.waitFor(() => expect(splitTrace()[0].persistence).toBe("saved"));
  expect(invokeNative).toHaveBeenCalledExactlyOnceWith("record_split_trace", {
    entry: {
      source: "menu", clientAtMs: expect.any(Number), sessionIds: ["eph-1"],
      parentSessionId: "session-1", tabId: "tab-1", direction: "horizontal",
    },
  });
});

it("retains failed events and continues persisting subsequent events", async () => {
  vi.stubGlobal("window", {});
  vi.spyOn(console, "warn").mockImplementation(() => {});
  invokeNative.mockRejectedValueOnce(new Error("offline"));
  const { traceSplit, splitTrace } = await import("./splitTrace");
  traceSplit("shortcut", "first", { sessionIds: ["eph-1"] });
  traceSplit("mirror", "second", { sessionIds: ["eph-2"] });
  await vi.waitFor(() => expect(splitTrace().map(e => e.persistence)).toEqual(["failed", "saved"]));
  expect(invokeNative).toHaveBeenCalledTimes(2);
  expect(console.warn).toHaveBeenCalledOnce();
});

it("distinguishes disabled logging from a persisted event", async () => {
  vi.stubGlobal("window", {});
  invokeNative.mockResolvedValueOnce(false);
  const { traceSplit, splitTrace } = await import("./splitTrace");
  traceSplit("pane-button", "split", { sessionIds: ["eph-1"] });
  await vi.waitFor(() => expect(splitTrace()[0].persistence).toBe("disabled"));
});
