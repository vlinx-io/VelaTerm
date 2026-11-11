//! Cmd/Ctrl+Tab tab cycling: forward/backward order, wrap-around at both ends, and the no-op below two
//! open tabs, plus the shared isTabCycleKey predicate that usePtySession uses to keep the combo from xterm.
//! Tab cannot be a remappable ShortcutAction (shortcutRegistry combos encode single letters
//! only), so this is a fixed shortcut next to Cmd+1–9 and is covered here rather than via the registry.
//!
//! Stub the Tauri-touching modules so termStore loads under jsdom, mirroring the store's own tests.

import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../ipc/tree", () => ({
  listTree: vi.fn().mockResolvedValue({ projects: [], groups: [], sessions: [] }),
}));
vi.mock("../notify", () => ({
  notify: vi.fn(),
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));

import { renderHook } from "@testing-library/react";
import { isTabCycleKey, useKeyboardShortcuts } from "./useKeyboardShortcuts";
import { useTermStore } from "../store/termStore";

/** Dispatch a keydown at the document, where the shortcut listener sits in capture phase. */
function press(
  key: string,
  mods: { ctrl?: boolean; meta?: boolean; shift?: boolean } = {},
): KeyboardEvent {
  const e = new KeyboardEvent("keydown", {
    key,
    ctrlKey: !!mods.ctrl,
    metaKey: !!mods.meta,
    shiftKey: !!mods.shift,
    bubbles: true,
    cancelable: true,
  });
  document.dispatchEvent(e);
  return e;
}

/** Seed the visible tab strip. `liveTabs` stays populated to prove it is not part of the cycle. */
function seed(openTabs: string[], activeTabId: string | null) {
  useTermStore.setState({
    openTabs,
    activeTabId,
    liveTabs: ["bg-1", "bg-2"],
    paneTrees: {},
    docTabs: {},
    browserTabs: {},
    activeSessionId: null,
    focusedPaneId: null,
  });
}

const active = () => useTermStore.getState().activeTabId;

beforeEach(() => {
  renderHook(() => useKeyboardShortcuts());
});

describe("Cmd/Ctrl+Tab tab cycling", () => {
  it("moves to the next tab and consumes the event", () => {
    seed(["a", "b", "c"], "a");
    const e = press("Tab", { ctrl: true });
    expect(active()).toBe("b");
    expect(e.defaultPrevented).toBe(true);
  });

  it("moves to the previous tab with Shift", () => {
    seed(["a", "b", "c"], "c");
    press("Tab", { ctrl: true, shift: true });
    expect(active()).toBe("b");
  });

  it("wraps forward from the last tab to the first", () => {
    seed(["a", "b", "c"], "c");
    press("Tab", { ctrl: true });
    expect(active()).toBe("a");
  });

  it("wraps backward from the first tab to the last", () => {
    seed(["a", "b", "c"], "a");
    press("Tab", { ctrl: true, shift: true });
    expect(active()).toBe("c");
  });

  it("cycles the whole strip and returns to the start", () => {
    seed(["a", "b", "c"], "a");
    const seen: (string | null)[] = [];
    for (let i = 0; i < 3; i++) {
      press("Tab", { ctrl: true });
      seen.push(active());
    }
    expect(seen).toEqual(["b", "c", "a"]);
  });

  it("treats Cmd the same as Ctrl", () => {
    seed(["a", "b"], "a");
    press("Tab", { meta: true });
    expect(active()).toBe("b");
  });

  it("is a no-op with a single open tab", () => {
    seed(["a"], "a");
    const e = press("Tab", { ctrl: true });
    expect(active()).toBe("a");
    expect(e.defaultPrevented).toBe(false);
  });

  it("is a no-op with no open tabs", () => {
    seed([], null);
    const e = press("Tab", { ctrl: true });
    expect(active()).toBeNull();
    expect(e.defaultPrevented).toBe(false);
  });

  it("ignores background liveTabs when cycling", () => {
    seed(["a", "b"], "b");
    press("Tab", { ctrl: true });
    expect(active()).toBe("a"); // Wrapped within openTabs; never reached "bg-1".
  });

  it("leaves an unmodified Tab alone so shell completion still reaches the terminal", () => {
    seed(["a", "b", "c"], "a");
    const plain = press("Tab");
    const shifted = press("Tab", { shift: true });
    expect(active()).toBe("a");
    expect(plain.defaultPrevented).toBe(false);
    expect(shifted.defaultPrevented).toBe(false);
  });
});

// usePtySession's xterm key handler swallows exactly what isTabCycleKey accepts; dispatching at `document`
// cannot reach that handler, so the predicate is checked directly.
describe("isTabCycleKey", () => {
  const key = (k: string, mods: { ctrl?: boolean; meta?: boolean; shift?: boolean } = {}) =>
    new KeyboardEvent("keydown", { key: k, ctrlKey: !!mods.ctrl, metaKey: !!mods.meta, shiftKey: !!mods.shift });

  it("claims Cmd/Ctrl(+Shift)+Tab while there are tabs to cycle", () => {
    seed(["a", "b"], "a");
    expect(isTabCycleKey(key("Tab", { ctrl: true }))).toBe(true);
    expect(isTabCycleKey(key("Tab", { meta: true }))).toBe(true);
    expect(isTabCycleKey(key("Tab", { ctrl: true, shift: true }))).toBe(true);
  });

  it("leaves the combo to the terminal with fewer than two open tabs", () => {
    seed(["a"], "a");
    expect(isTabCycleKey(key("Tab", { ctrl: true }))).toBe(false);
    seed([], null);
    expect(isTabCycleKey(key("Tab", { ctrl: true, shift: true }))).toBe(false);
  });

  it("never claims an unmodified Tab or Shift+Tab", () => {
    seed(["a", "b"], "a");
    expect(isTabCycleKey(key("Tab"))).toBe(false);
    expect(isTabCycleKey(key("Tab", { shift: true }))).toBe(false);
  });
});
