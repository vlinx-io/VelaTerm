import { cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const state = vi.hoisted(() => ({
  activeSessionId: "session-1" as string | null,
  shortcutOverrides: {} as Record<string, string>,
  splitNew: vi.fn(),
  newScratchTab: vi.fn(),
}));

vi.mock("../platform", () => ({ env: { isBrowser: true, isRemoteWindow: false } }));
vi.mock("../ipc/transport", () => ({ isTauri: false }));
vi.mock("../store/termStore", () => ({ useTermStore: { getState: () => state } }));

beforeEach(async () => {
  vi.stubGlobal("navigator", { platform: "MacIntel" });
  vi.resetModules();
  state.activeSessionId = "session-1";
  state.shortcutOverrides = {};
  const { useKeyboardShortcuts } = await import("./useKeyboardShortcuts");
  renderHook(() => useKeyboardShortcuts());
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

function press(key: string, modifiers: KeyboardEventInit) {
  const event = new KeyboardEvent("keydown", {
    key, code: `Key${key.toUpperCase()}`, bubbles: true, cancelable: true, ...modifiers,
  });
  document.body.dispatchEvent(event);
  return event;
}

it("splits once in each direction and cancels browser defaults on macOS", () => {
  expect(press("d", { metaKey: true }).defaultPrevented).toBe(true);
  expect(state.splitNew).toHaveBeenNthCalledWith(1, "horizontal", "shortcut");
  expect(press("D", { metaKey: true, shiftKey: true }).defaultPrevented).toBe(true);
  expect(state.splitNew).toHaveBeenNthCalledWith(2, "vertical", "shortcut");
  expect(state.splitNew).toHaveBeenCalledTimes(2);
});

it("leaves Ctrl+D and unrelated Cmd chords alone while retaining other browser bindings", () => {
  expect(press("d", { ctrlKey: true }).defaultPrevented).toBe(false);
  expect(press("d", { ctrlKey: true, shiftKey: true }).defaultPrevented).toBe(false);
  expect(press("d", { ctrlKey: true, metaKey: true }).defaultPrevented).toBe(false);
  expect(press("t", { metaKey: true }).defaultPrevented).toBe(false);
  expect(state.splitNew).not.toHaveBeenCalled();
  expect(state.newScratchTab).not.toHaveBeenCalled();
  expect(press("t", { ctrlKey: true, altKey: true }).defaultPrevented).toBe(true);
  expect(state.newScratchTab).toHaveBeenCalledOnce();
});

it("honors a saved split binding instead of also triggering the default", () => {
  state.shortcutOverrides = { splitRight: "mod+alt+k" };
  expect(press("d", { metaKey: true }).defaultPrevented).toBe(false);
  expect(state.splitNew).not.toHaveBeenCalled();
  expect(press("k", { ctrlKey: true, altKey: true }).defaultPrevented).toBe(true);
  expect(state.splitNew).toHaveBeenCalledExactlyOnceWith("horizontal", "shortcut");
});

it("does not claim browser commands without an active terminal session", () => {
  state.activeSessionId = null;
  expect(press("d", { metaKey: true }).defaultPrevented).toBe(false);
  expect(press("d", { metaKey: true, shiftKey: true }).defaultPrevented).toBe(false);
  expect(state.splitNew).not.toHaveBeenCalled();
});
