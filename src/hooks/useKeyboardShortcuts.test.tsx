import { cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const state = vi.hoisted(() => ({
  activeSessionId: "session-1" as string | null,
  activeTabId: "session-1" as string | null,
  docTabs: {} as Record<string, unknown>,
  browserTabs: {} as Record<string, unknown>,
  taskTabs: {} as Record<string, unknown>,
  shortcutOverrides: {} as Record<string, string>,
  splitNew: vi.fn(),
  newScratchTab: vi.fn(),
  projects: [], groups: [], sessions: [], inspectTarget: null, selection: [],
}));

const terminalRegistry = vi.hoisted(() => ({ getTerminal: vi.fn(), selectAll: vi.fn() }));
vi.mock("../terminal/registry", () => terminalRegistry);
vi.mock("../platform", () => ({ env: { isBrowser: true, isRemoteWindow: false } }));
vi.mock("../ipc/transport", () => ({ isTauri: false }));
vi.mock("../store/termStore", () => ({ useTermStore: { getState: () => state } }));

beforeEach(async () => {
  vi.stubGlobal("navigator", { platform: "MacIntel" });
  vi.resetModules();
  state.activeSessionId = "session-1";
  state.activeTabId = "session-1";
  state.docTabs = {};
  state.browserTabs = {};
  state.taskTabs = {};
  state.shortcutOverrides = {};
  terminalRegistry.getTerminal.mockReset();
  window.history.replaceState(null, "", "/");
  const { useKeyboardShortcuts } = await import("./useKeyboardShortcuts");
  renderHook(() => useKeyboardShortcuts());
});

afterEach(() => {
  cleanup();
  document.body.replaceChildren();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

function press(key: string, modifiers: KeyboardEventInit, target: HTMLElement = document.body) {
  const event = new KeyboardEvent("keydown", {
    key, code: `Key${key.toUpperCase()}`, bubbles: true, cancelable: true, ...modifiers,
  });
  target.dispatchEvent(event);
  return event;
}

it("splits once in each direction and cancels browser defaults on macOS", () => {
  expect(press("d", { metaKey: true }).defaultPrevented).toBe(true);
  expect(state.splitNew).toHaveBeenNthCalledWith(1, "horizontal", "shortcut");
  expect(press("D", { metaKey: true, shiftKey: true }).defaultPrevented).toBe(true);
  expect(state.splitNew).toHaveBeenNthCalledWith(2, "vertical", "shortcut");
  expect(state.splitNew).toHaveBeenCalledTimes(2);
});

function focusedTerminal() {
  const element = document.createElement("div");
  const textarea = document.createElement("textarea");
  element.append(textarea);
  document.body.append(element);
  terminalRegistry.getTerminal.mockReturnValue({ element });
  textarea.focus();
  return textarea;
}

it("selects the focused terminal without forwarding the shortcut to its input handler", () => {
  const textarea = focusedTerminal();
  const terminalInput = vi.fn();
  textarea.addEventListener("keydown", terminalInput);
  expect(press("A", { ctrlKey: true, shiftKey: true }, textarea).defaultPrevented).toBe(true);
  expect(terminalRegistry.selectAll).toHaveBeenCalledExactlyOnceWith("session-1");
  expect(terminalInput).not.toHaveBeenCalled();

  // Plain Ctrl+A still belongs to the shell until the user explicitly rebinds it.
  expect(press("a", { ctrlKey: true }, textarea).defaultPrevented).toBe(false);
  expect(terminalInput).toHaveBeenCalledOnce();
  state.shortcutOverrides = { selectAllTerminal: "mod+a" };
  terminalInput.mockClear();
  expect(press("a", { ctrlKey: true }, textarea).defaultPrevented).toBe(true);
  expect(terminalRegistry.selectAll).toHaveBeenCalledTimes(2);
  expect(terminalInput).not.toHaveBeenCalled();
});

it.each(["input", "textarea", "contenteditable"])("preserves select all in a focused %s outside the terminal", (kind) => {
  focusedTerminal();
  const field = document.createElement(kind === "contenteditable" ? "div" : kind);
  if (kind === "contenteditable") {
    field.setAttribute("contenteditable", "true");
    field.tabIndex = 0;
  }
  document.body.append(field);
  field.focus();
  state.shortcutOverrides = { selectAllTerminal: "mod+a" };
  expect(press("a", { ctrlKey: true }, field).defaultPrevented).toBe(false);
  expect(terminalRegistry.selectAll).not.toHaveBeenCalled();
});

it("leaves conversation and dormant sessions without a registered terminal alone", () => {
  const field = document.createElement("textarea");
  document.body.append(field);
  field.focus();
  expect(press("A", { ctrlKey: true, shiftKey: true }, field).defaultPrevented).toBe(false);
  expect(terminalRegistry.selectAll).not.toHaveBeenCalled();
});

it.each(["docTabs", "browserTabs", "taskTabs"] as const)("leaves %s selection alone even with a stale active session", (tabs) => {
  const textarea = focusedTerminal();
  state.activeTabId = "other-tab";
  state[tabs] = { "other-tab": {} };
  expect(press("A", { ctrlKey: true, shiftKey: true }, textarea).defaultPrevented).toBe(false);
  expect(terminalRegistry.selectAll).not.toHaveBeenCalled();
});

it("does not select terminal output during composition or after another handler cancels the key", () => {
  const textarea = focusedTerminal();
  for (const extra of [{ isComposing: true }, { keyCode: 229 }]) {
    expect(press("A", { ctrlKey: true, shiftKey: true, ...extra }, textarea).defaultPrevented).toBe(false);
  }
  const canceled = new KeyboardEvent("keydown", {
    key: "A", code: "KeyA", ctrlKey: true, shiftKey: true, bubbles: true, cancelable: true,
  });
  canceled.preventDefault();
  textarea.dispatchEvent(canceled);
  expect(terminalRegistry.selectAll).not.toHaveBeenCalled();
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

it("opens the agent picker without an active session, respects overrides and ignores IME/repeats", () => {
  state.activeSessionId = null;
  state.shortcutOverrides = { newAgentSession: "mod+alt+j" };
  expect(press("n", { ctrlKey: true, altKey: true }).defaultPrevented).toBe(false);
  for (const extra of [{ isComposing: true }, { keyCode: 229 }, { repeat: true }]) {
    expect(press("j", { ctrlKey: true, altKey: true, ...extra }).defaultPrevented).toBe(false);
    expect(window.location.search).toBe("");
  }
  expect(press("j", { ctrlKey: true, altKey: true }).defaultPrevented).toBe(true);
  const url = window.location.href;
  expect(new URL(url).searchParams.has("newAgent")).toBe(true);
  expect(press("j", { ctrlKey: true, altKey: true }).defaultPrevented).toBe(true);
  expect(window.location.href).toBe(url);
});

it("preserves an older action rebound onto the new action's default", () => {
  state.shortcutOverrides = { newTab: "mod+alt+n" };
  expect(press("n", { ctrlKey: true, altKey: true }).defaultPrevented).toBe(true);
  expect(state.newScratchTab).toHaveBeenCalledOnce();
  expect(new URLSearchParams(window.location.search).has("newAgent")).toBe(false);
});

it("switches tabs with mod+digit but leaves mod+Alt+digit to editors", () => {
  const setActiveTab = vi.fn();
  Object.assign(state, { openTabs: ["tab-1", "tab-2"], setActiveTab });
  const digit = (modifiers: KeyboardEventInit) => {
    const event = new KeyboardEvent("keydown", { key: "1", code: "Digit1", bubbles: true, cancelable: true, ...modifiers });
    document.body.dispatchEvent(event);
    return event;
  };
  // This suite runs as a plain browser on macOS, where mod is Ctrl.
  expect(digit({ ctrlKey: true }).defaultPrevented).toBe(true);
  expect(setActiveTab).toHaveBeenCalledWith("tab-1");
  setActiveTab.mockClear();
  expect(digit({ ctrlKey: true, altKey: true }).defaultPrevented).toBe(false);
  expect(setActiveTab).not.toHaveBeenCalled();
});

it("numbers only the visible tabs when tabs are scoped to a project", () => {
  const setActiveTab = vi.fn();
  Object.assign(state, {
    scopeTabsToProject: true,
    activeProjectId: "A",
    openTabs: ["a1", "b1", "a2"],
    paneTrees: {},
    ephemeralSessions: {},
    projects: [{ id: "A" }, { id: "B" }],
    sessions: [
      { id: "a1", projectId: "A" },
      { id: "b1", projectId: "B" },
      { id: "a2", projectId: "A" },
    ],
    setActiveTab,
  });
  try {
    const event = new KeyboardEvent("keydown", { key: "2", code: "Digit2", bubbles: true, cancelable: true, ctrlKey: true });
    document.body.dispatchEvent(event);
    expect(setActiveTab).toHaveBeenCalledWith("a2");
  } finally {
    Object.assign(state, { scopeTabsToProject: false, activeProjectId: null, projects: [], sessions: [] });
  }
});

it("numbers all open tabs when scoping is off", () => {
  const setActiveTab = vi.fn();
  Object.assign(state, {
    scopeTabsToProject: false,
    activeProjectId: "A",
    openTabs: ["a1", "b1", "a2"],
    paneTrees: {},
    ephemeralSessions: {},
    projects: [{ id: "A" }, { id: "B" }],
    sessions: [
      { id: "a1", projectId: "A" },
      { id: "b1", projectId: "B" },
      { id: "a2", projectId: "A" },
    ],
    setActiveTab,
  });
  try {
    document.body.dispatchEvent(new KeyboardEvent("keydown", { key: "2", code: "Digit2", bubbles: true, cancelable: true, ctrlKey: true }));
    expect(setActiveTab).toHaveBeenCalledWith("b1");
  } finally {
    Object.assign(state, { activeProjectId: null, projects: [], sessions: [] });
  }
});
