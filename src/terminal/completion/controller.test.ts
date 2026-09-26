// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Terminal } from "@xterm/xterm";
import { installCompletion } from "./controller";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), config: { mode: "auto", debounceMs: 350 }, configChanged: () => {} }));
vi.mock("../../ipc/transport", () => ({ invoke: mocks.invoke }));
vi.mock("../../i18n", () => ({ t: (key: string) => key }));
vi.mock("./config", () => ({
  getCompletionConfig: () => mocks.config,
  reloadCompletionConfig: vi.fn(),
  watchCompletionConfig: (fn: () => void) => { mocks.configChanged = fn; return () => {}; },
}));

const state = { configured: true, supported: true, ready: true, pending: false, revision: 5, inputVersion: 1,
  query: "中文", items: [{ index: 0, label: "中文 path", description: "a directory", matches: [[0, 2]] }] };
let controller: ReturnType<typeof installCompletion>;
let textarea: HTMLTextAreaElement;
let write = vi.fn<(data: string) => Promise<unknown>>();
let terminal: Terminal;

beforeEach(async () => {
  vi.useFakeTimers(); mocks.invoke.mockReset(); mocks.config.mode = "auto";
  mocks.invoke.mockResolvedValue(state);
  Element.prototype.scrollIntoView = vi.fn();
  const element = document.createElement("div");
  element.innerHTML = '<div class="xterm-screen"></div><textarea></textarea>';
  element.getClientRects = () => [new DOMRect(0, 0, 800, 400)] as unknown as DOMRectList;
  textarea = element.querySelector("textarea")!;
  document.body.appendChild(element); textarea.focus();
  terminal = { element, textarea, cols: 80, rows: 24,
    buffer: { active: { type: "normal", cursorX: 5, cursorY: 0, baseY: 0, viewportY: 0 } },
    onCursorMove: () => ({ dispose() {} }), onScroll: () => ({ dispose() {} }),
    onWriteParsed: () => ({ dispose() {} }),
    focus: () => textarea.focus(),
  } as unknown as Terminal;
  write = vi.fn<(data: string) => Promise<unknown>>().mockResolvedValue(undefined);
  controller = installCompletion(terminal, write, "session");
  await vi.advanceTimersByTimeAsync(0);
  mocks.invoke.mockClear();
});
afterEach(() => { controller.dispose(); document.body.replaceChildren(); vi.useRealTimers(); });

describe("native completion interaction", () => {
  it("opens distinct popups without crypto.randomUUID on HTTP pages", async () => {
    vi.stubGlobal("crypto", {});
    let second: ReturnType<typeof installCompletion> | undefined;
    try {
      second = installCompletion(terminal, write, "second-session");
      await vi.advanceTimersByTimeAsync(0);
      const popups = [...document.querySelectorAll<HTMLElement>('[role="listbox"]')];
      expect(popups).toHaveLength(2);
      expect(new Set(popups.map(popup => popup.id)).size).toBe(2);
      second.input("中"); await vi.advanceTimersByTimeAsync(350);
      expect(popups[1].hidden).toBe(false);
      expect(textarea.getAttribute("aria-controls")).toBe(popups[1].id);
    } finally {
      second?.dispose();
      vi.unstubAllGlobals();
    }
  });

  it("highlights native matches and keeps the list mounted during further input", async () => {
    controller.input("中"); await vi.advanceTimersByTimeAsync(350);
    const popup = document.querySelector<HTMLElement>('[role="listbox"]')!;
    expect(popup.querySelector('mark')?.textContent).toBe("中文");
    controller.input("文");
    expect(popup.hidden).toBe(false);
    expect(popup.getAttribute("aria-busy")).toBe("true");
    await vi.advanceTimersByTimeAsync(350);
    expect(popup.hidden).toBe(false);
    expect(popup.hasAttribute("aria-busy")).toBe(false);
  });

  it("waits for a fresh revision when selection is requested during an update", async () => {
    controller.input("中"); await vi.advanceTimersByTimeAsync(350);
    controller.input("文");
    mocks.invoke.mockResolvedValue({ ...state, revision: 6 });
    controller.key(new KeyboardEvent("keydown", { key: "Tab" }));
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.invoke).toHaveBeenCalledWith("pty_completion", {
      sessionId: "session", action: "accept", revision: 6, index: 0,
    });
    expect(write).not.toHaveBeenCalled();
  });

  it("keeps the selected candidate across refreshed results and uses its native index", async () => {
    const second = { index: 1, label: "another", description: "description", matches: [] };
    mocks.invoke.mockResolvedValue({ ...state, items: [...state.items, second] });
    controller.input("a"); await vi.advanceTimersByTimeAsync(350);
    controller.key(new KeyboardEvent("keydown", { key: "ArrowDown" }));
    controller.input("n"); await vi.advanceTimersByTimeAsync(350);
    expect(document.querySelector('[aria-selected="true"]')?.textContent).toContain("another");
    controller.key(new KeyboardEvent("keydown", { key: "Tab" }));
    expect(mocks.invoke).toHaveBeenCalledWith("pty_completion", {
      sessionId: "session", action: "accept", revision: 5, index: 1,
    });
  });

  it("leaves shell history walking to the shell instead of requesting between presses", async () => {
    controller.input("g"); await vi.advanceTimersByTimeAsync(350);
    const popup = document.querySelector<HTMLElement>('[role="listbox"]')!;
    expect(popup.hidden).toBe(false);
    mocks.invoke.mockClear();
    // Zsh ends its continuing history search once another widget runs, so an automatic request here
    // would stop Up after the first recalled entry.
    controller.input("\x1b[A"); await vi.advanceTimersByTimeAsync(1000);
    expect(mocks.invoke).not.toHaveBeenCalled();
    expect(popup.hidden).toBe(true);
    controller.input("i"); await vi.advanceTimersByTimeAsync(350);
    expect(popup.hidden).toBe(false);
  });

  it("hands an arrow back to the shell when the list cannot move further", async () => {
    const second = { index: 1, label: "token", description: "", matches: [] };
    mocks.invoke.mockResolvedValue({ ...state, items: [state.items[0], second] });
    controller.input("t"); await vi.advanceTimersByTimeAsync(350);
    const popup = document.querySelector<HTMLElement>('[role="listbox"]')!;
    const up = new KeyboardEvent("keydown", { key: "ArrowUp", cancelable: true });
    expect(controller.key(up)).toBe(true);
    expect(up.defaultPrevented).toBe(false);
    expect(popup.hidden).toBe(true);
    controller.input("t"); await vi.advanceTimersByTimeAsync(350);
    expect(controller.key(new KeyboardEvent("keydown", { key: "ArrowDown", cancelable: true }))).toBe(false);
    expect(popup.querySelector('[aria-selected="true"]')?.textContent).toContain("token");
    const down = new KeyboardEvent("keydown", { key: "ArrowDown", cancelable: true });
    expect(controller.key(down)).toBe(true);
    expect(down.defaultPrevented).toBe(false);
    expect(popup.hidden).toBe(true);
  });

  it("opens native suggestions with Ctrl+Space", async () => {
    expect(controller.key(new KeyboardEvent("keydown", { key: " ", code: "Space", ctrlKey: true }))).toBe(false);
    await vi.advanceTimersByTimeAsync(0);
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(false);
  });

  it("does not lose repeated manual requests while IPC is in flight", async () => {
    mocks.config.mode = "tab";
    let resolve!: (value: typeof state) => void;
    mocks.invoke.mockImplementationOnce(() => new Promise(r => { resolve = r; }));
    controller.key(new KeyboardEvent("keydown", { key: "Tab" }));
    controller.key(new KeyboardEvent("keydown", { key: "Tab" }));
    resolve(state); await vi.advanceTimersByTimeAsync(1);
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(false);
  });

  it.each(["auto", "tab"])("reopens accepted directory suggestions only on demand in %s mode", async mode => {
    mocks.config.mode = mode;
    mocks.invoke.mockResolvedValue({ ...state, items: [{ ...state.items[0], label: "folder/" }] });
    controller.input("f");
    if (mode === "tab") controller.key(new KeyboardEvent("keydown", { key: "Tab" }));
    await vi.advanceTimersByTimeAsync(350);
    controller.key(new KeyboardEvent("keydown", { key: "Tab" }));
    await vi.advanceTimersByTimeAsync(1000);
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(true);
    expect(mocks.invoke.mock.calls.filter(([, args]) => args.action === "query")).toHaveLength(1);
    controller.key(new KeyboardEvent("keydown", { key: "Tab" }));
    await vi.advanceTimersByTimeAsync(1000);
    expect(write).not.toHaveBeenCalled();
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(false);
    expect(mocks.invoke.mock.calls.filter(([, args]) => args.action === "query")).toHaveLength(2);
  });

  it("coalesces continuous input without postponing the request and inserts on Tab", async () => {
    controller.input("g"); await vi.advanceTimersByTimeAsync(200);
    controller.input("i"); await vi.advanceTimersByTimeAsync(149);
    expect(mocks.invoke).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(document.querySelector('[role="option"]')?.textContent).toContain("中文 path");
    const tab = new KeyboardEvent("keydown", { key: "Tab", cancelable: true });
    expect(controller.key(tab)).toBe(false);
    expect(mocks.invoke).toHaveBeenCalledWith("pty_completion", {
      sessionId: "session", action: "accept", revision: 5, index: 0,
    });
    expect(write).not.toHaveBeenCalled();
  });

  it.each([false, true])("passes Enter to the shell without accepting, pending=%s", async pending => {
    controller.input("ls -lh"); await vi.advanceTimersByTimeAsync(350);
    if (pending) controller.input("A");
    const enter = new KeyboardEvent("keydown", { key: "Enter", cancelable: true });
    expect(controller.key(enter)).toBe(true);
    expect(enter.defaultPrevented).toBe(false);
    await vi.advanceTimersByTimeAsync(1000);
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(true);
    expect(mocks.invoke.mock.calls.some(([, args]) => args.action === "accept")).toBe(false);
    expect(write).not.toHaveBeenCalled();
  });

  it("accepts a keyboard-selected candidate on Enter like Tab", async () => {
    const second = { index: 1, label: "another", description: "", matches: [] };
    mocks.invoke.mockResolvedValue({ ...state, items: [...state.items, second] });
    controller.input("a"); await vi.advanceTimersByTimeAsync(350);
    controller.key(new KeyboardEvent("keydown", { key: "ArrowDown", cancelable: true }));
    const enter = new KeyboardEvent("keydown", { key: "Enter", cancelable: true });
    expect(controller.key(enter)).toBe(false);
    expect(enter.defaultPrevented).toBe(true);
    expect(mocks.invoke).toHaveBeenCalledWith("pty_completion", {
      sessionId: "session", action: "accept", revision: 5, index: 1,
    });
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(true);
    expect(write).not.toHaveBeenCalled();
  });

  it("returns Enter to the shell once input follows a keyboard selection", async () => {
    const second = { index: 1, label: "another", description: "", matches: [] };
    mocks.invoke.mockResolvedValue({ ...state, items: [...state.items, second] });
    controller.input("a"); await vi.advanceTimersByTimeAsync(350);
    controller.key(new KeyboardEvent("keydown", { key: "ArrowDown", cancelable: true }));
    controller.input("n"); await vi.advanceTimersByTimeAsync(350);
    expect(document.querySelector('[aria-selected="true"]')?.textContent).toContain("another");
    const enter = new KeyboardEvent("keydown", { key: "Enter", cancelable: true });
    expect(controller.key(enter)).toBe(true);
    expect(enter.defaultPrevented).toBe(false);
    expect(mocks.invoke.mock.calls.some(([, args]) => args.action === "accept")).toBe(false);
  });

  it.each(["push", "-lh", "中文"])("keeps automatic exact matches visible for %s during input", async token => {
    const candidate = { index: 1, label: token, description: "", matches: [] };
    mocks.invoke.mockResolvedValue({ ...state, query: token, items: [
      { ...candidate, index: 0, label: token + "x" }, candidate,
    ] });
    controller.input(token); await vi.advanceTimersByTimeAsync(350);
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(false);
    mocks.invoke.mockResolvedValue({ ...state, query: token + " ", items: [candidate] });
    controller.input(" "); await vi.advanceTimersByTimeAsync(350);
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(false);
  });

  it("keeps exact basename candidates visible for path tokens", async () => {
    mocks.invoke.mockResolvedValue({ ...state, query: "/tmp/file", items: [
      { index: 0, label: "file", description: "", matches: [] },
    ] });
    controller.input("e"); await vi.advanceTimersByTimeAsync(350);
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(false);
  });

  it("finishes an in-flight query and immediately refreshes for the latest input", async () => {
    let finish!: (value: typeof state) => void;
    mocks.invoke.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    controller.input("g"); await vi.advanceTimersByTimeAsync(350);
    controller.input("i"); controller.input("t");
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
    finish(state); await vi.advanceTimersByTimeAsync(1);
    expect(mocks.invoke.mock.calls.filter(([, args]) => args.action === "query")).toHaveLength(2);
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(false);
  });

  it("waits for native pending completion before querying changed input", async () => {
    mocks.invoke.mockResolvedValueOnce({ ...state, pending: true });
    controller.input("g"); await vi.advanceTimersByTimeAsync(350);
    controller.input("i");
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(17);
    expect(mocks.invoke.mock.calls.map(([, args]) => args.action)).toEqual(["query", "state", "query"]);
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(false);
  });

  it("closes suggestions when disabled, preserves native keys and resumes when enabled", async () => {
    controller.input("g"); await vi.advanceTimersByTimeAsync(350);
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(false);
    mocks.config.mode = "off"; mocks.configChanged(); mocks.invoke.mockClear();
    controller.input("i"); await vi.advanceTimersByTimeAsync(1000);
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(true);
    for (const event of [new KeyboardEvent("keydown", { key: "Tab", cancelable: true }),
      new KeyboardEvent("keydown", { key: " ", code: "Space", ctrlKey: true, cancelable: true })]) {
      expect(controller.key(event)).toBe(true);
      expect(event.defaultPrevented).toBe(false);
    }
    expect(mocks.invoke).not.toHaveBeenCalled();
    mocks.config.mode = "auto"; mocks.configChanged();
    controller.input("t"); await vi.advanceTimersByTimeAsync(350);
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(false);
  });

  it("discards pending candidates after suggestions are disabled", async () => {
    let finish!: (value: typeof state) => void;
    mocks.invoke.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    controller.input("g"); await vi.advanceTimersByTimeAsync(350);
    mocks.config.mode = "off"; mocks.configChanged();
    finish(state); await vi.advanceTimersByTimeAsync(1000);
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(true);
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
  });

  it("only requests on Tab when configured", async () => {
    mocks.config.mode = "tab";
    controller.input("g"); await vi.advanceTimersByTimeAsync(1000);
    expect(mocks.invoke).not.toHaveBeenCalled();
    expect(controller.key(new KeyboardEvent("keydown", { key: "Tab" }))).toBe(false);
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.invoke).toHaveBeenCalledWith("pty_completion", { sessionId: "session", action: "query" });
  });

  it("discards a response received after further input", async () => {
    let resolve!: (value: typeof state) => void;
    mocks.invoke.mockImplementationOnce(() => new Promise(r => { resolve = r; }));
    controller.input("g"); await vi.advanceTimersByTimeAsync(350);
    controller.input("\r"); resolve(state); await vi.advanceTimersByTimeAsync(0);
    expect(document.querySelector<HTMLElement>('[role="listbox"]')!.hidden).toBe(true);
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
  });

  it("preserves native Tab for unsupported shells and alternate-screen programs", async () => {
    mocks.invoke.mockResolvedValue({ ...state, supported: false, ready: false, items: [] });
    controller.key(new KeyboardEvent("keydown", { key: "Tab" }));
    await vi.advanceTimersByTimeAsync(0);
    expect(write).toHaveBeenCalledWith("\t");
    mocks.invoke.mockClear();
    expect(controller.key(new KeyboardEvent("keydown", { key: "Tab" }))).toBe(true);
    expect(mocks.invoke).not.toHaveBeenCalled();
    Object.assign(terminal.buffer.active, { type: "alternate" });
    expect(controller.key(new KeyboardEvent("keydown", { key: "Tab" }))).toBe(true);
  });

  it("does not request while composing text or searching shell history", async () => {
    textarea.dispatchEvent(new CompositionEvent("compositionstart"));
    controller.input("中"); await vi.advanceTimersByTimeAsync(500);
    expect(mocks.invoke).not.toHaveBeenCalled();
    textarea.dispatchEvent(new CompositionEvent("compositionend"));
    controller.input("\x12"); controller.input("git");
    await vi.advanceTimersByTimeAsync(500);
    expect(mocks.invoke).not.toHaveBeenCalled();
  });
});
