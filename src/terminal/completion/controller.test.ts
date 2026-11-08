// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Terminal } from "@xterm/xterm";
import { installCompletion } from "./controller";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), config: { mode: "auto", tabBehavior: "complete", debounceMs: 350 }, configChanged: () => {} }));
vi.mock("../../ipc/transport", () => ({ invoke: mocks.invoke }));
vi.mock("../../i18n", () => ({ t: (key: string) => key }));
vi.mock("./config", () => ({
  getCompletionConfig: () => mocks.config,
  reloadCompletionConfig: vi.fn(),
  watchCompletionConfig: (fn: () => void) => { mocks.configChanged = fn; return () => {}; },
}));

const tips = vi.hoisted(() => ({ current: null as string | null }));
vi.mock("./tip", () => ({ setCompletionTip: (_session: string, tip: string | null) => { tips.current = tip; } }));

const state = { configured: true, supported: true, ready: true, pending: false, revision: 5, inputVersion: 1,
  query: "中文", items: [{ index: 0, label: "中文 path", description: "a directory", matches: [[0, 2]] }] };
let controller: ReturnType<typeof installCompletion>;
let textarea: HTMLTextAreaElement;
let write = vi.fn<(data: string) => Promise<unknown>>();
let terminal: Terminal;
let reportHint: (data: string) => boolean = () => true;

beforeEach(async () => {
  vi.useFakeTimers(); mocks.invoke.mockReset(); mocks.config.mode = "auto"; mocks.config.tabBehavior = "complete"; tips.current = null;
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
    parser: { registerOscHandler: (_code: number, handler: (data: string) => boolean) => { reportHint = handler; return { dispose() {} }; } },
    focus: () => textarea.focus(),
  } as unknown as Terminal;
  write = vi.fn<(data: string) => Promise<unknown>>().mockResolvedValue(undefined);
  controller = installCompletion(terminal, write, "session");
  await vi.advanceTimersByTimeAsync(0);
  mocks.invoke.mockClear();
});
afterEach(() => { controller.dispose(); document.body.replaceChildren(); vi.useRealTimers(); });

const popup = () => document.querySelector<HTMLElement>('[role="listbox"]')!;
const ctrlSpace = () => new KeyboardEvent("keydown", { key: " ", code: "Space", ctrlKey: true, cancelable: true });
const tab = () => new KeyboardEvent("keydown", { key: "Tab", cancelable: true });
const actions = () => mocks.invoke.mock.calls.map(([, args]) => args.action);
async function open(target = controller) { target.key(ctrlSpace()); await vi.advanceTimersByTimeAsync(0); }

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
      await open(second);
      expect(popups[1].hidden).toBe(false);
      expect(textarea.getAttribute("aria-controls")).toBe(popups[1].id);
    } finally {
      second?.dispose();
      vi.unstubAllGlobals();
    }
  });

  it.each(["auto", "tab"])("never opens the list while typing in %s mode", async mode => {
    mocks.config.mode = mode;
    for (const data of ["g", "i", "t", " ", "c"]) controller.input(data);
    await vi.advanceTimersByTimeAsync(1000);
    expect(mocks.invoke).not.toHaveBeenCalled();
    expect(popup().hidden).toBe(true);
  });

  it.each(["auto", "tab"])("requests on Tab in %s mode", async mode => {
    mocks.config.mode = mode;
    expect(controller.key(tab())).toBe(false);
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.invoke).toHaveBeenCalledWith("pty_completion", { sessionId: "session", action: "query", apply: true });
  });

  it("leaves nothing more to do when the backend already applied Tab", async () => {
    mocks.invoke.mockResolvedValue({ ...state, items: [], applied: true });
    controller.key(tab()); await vi.advanceTimersByTimeAsync(0);
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
    expect(popup().hidden).toBe(true);
    expect(write).not.toHaveBeenCalled();
  });

  it("applies a single candidate on Tab without opening the list", async () => {
    mocks.invoke.mockResolvedValue({ ...state, items: [{ ...state.items[0], index: 3, label: "folder/" }] });
    controller.key(tab()); await vi.advanceTimersByTimeAsync(0);
    expect(mocks.invoke).toHaveBeenCalledWith("pty_completion", {
      sessionId: "session", action: "accept", revision: 5, index: 3,
    });
    expect(popup().hidden).toBe(true);
    expect(write).not.toHaveBeenCalled();
  });

  it("inserts the shared prefix on Tab before listing ambiguous candidates", async () => {
    const items = [{ index: 0, label: "Documents/", description: "", matches: [] },
      { index: 1, label: "Downloads/", description: "", matches: [] }];
    mocks.invoke.mockResolvedValue({ ...state, query: "D", items, extendable: true });
    controller.key(tab()); await vi.advanceTimersByTimeAsync(0);
    expect(mocks.invoke).toHaveBeenCalledWith("pty_completion", { sessionId: "session", action: "extend", revision: 5 });
    expect(popup().hidden).toBe(true);
    mocks.invoke.mockClear();
    mocks.invoke.mockResolvedValue({ ...state, query: "Do", items, extendable: false });
    controller.key(tab()); await vi.advanceTimersByTimeAsync(0);
    expect(actions()).toEqual(["query"]);
    expect(popup().hidden).toBe(false);
    controller.key(tab());
    expect(mocks.invoke).toHaveBeenCalledWith("pty_completion", {
      sessionId: "session", action: "accept", revision: 5, index: 0,
    });
    expect(write).not.toHaveBeenCalled();
  });

  it("highlights native matches and keeps the list mounted during further input", async () => {
    await open();
    expect(popup().querySelector('mark')?.textContent).toBe("中文");
    controller.input("文");
    expect(popup().hidden).toBe(false);
    expect(popup().getAttribute("aria-busy")).toBe("true");
    await vi.advanceTimersByTimeAsync(350);
    expect(popup().hidden).toBe(false);
    expect(popup().hasAttribute("aria-busy")).toBe(false);
  });

  it("closes an open list on whitespace instead of listing the next argument", async () => {
    await open(); mocks.invoke.mockClear();
    controller.input(" "); await vi.advanceTimersByTimeAsync(1000);
    expect(popup().hidden).toBe(true);
    expect(mocks.invoke).not.toHaveBeenCalled();
  });

  it("waits for a fresh revision when selection is requested during an update", async () => {
    await open();
    controller.input("文");
    mocks.invoke.mockResolvedValue({ ...state, revision: 6 });
    controller.key(tab());
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.invoke).toHaveBeenCalledWith("pty_completion", {
      sessionId: "session", action: "accept", revision: 6, index: 0,
    });
    expect(write).not.toHaveBeenCalled();
  });

  it("keeps the selected candidate across refreshed results and uses its native index", async () => {
    const second = { index: 1, label: "another", description: "description", matches: [] };
    mocks.invoke.mockResolvedValue({ ...state, items: [...state.items, second] });
    await open();
    controller.key(new KeyboardEvent("keydown", { key: "ArrowDown" }));
    controller.input("n"); await vi.advanceTimersByTimeAsync(350);
    expect(document.querySelector('[aria-selected="true"]')?.textContent).toContain("another");
    controller.key(tab());
    expect(mocks.invoke).toHaveBeenCalledWith("pty_completion", {
      sessionId: "session", action: "accept", revision: 5, index: 1,
    });
  });

  it("leaves shell history walking to the shell instead of requesting between presses", async () => {
    await open();
    expect(popup().hidden).toBe(false);
    mocks.invoke.mockClear();
    // Zsh ends its continuing history search once another widget runs, so a request here would stop Up
    // after the first recalled entry.
    controller.input("\x1b[A"); await vi.advanceTimersByTimeAsync(1000);
    expect(mocks.invoke).not.toHaveBeenCalled();
    expect(popup().hidden).toBe(true);
  });

  it("hands an arrow back to the shell when the list cannot move further", async () => {
    const second = { index: 1, label: "token", description: "", matches: [] };
    mocks.invoke.mockResolvedValue({ ...state, items: [state.items[0], second] });
    await open();
    const up = new KeyboardEvent("keydown", { key: "ArrowUp", cancelable: true });
    expect(controller.key(up)).toBe(true);
    expect(up.defaultPrevented).toBe(false);
    expect(popup().hidden).toBe(true);
    await open();
    expect(controller.key(new KeyboardEvent("keydown", { key: "ArrowDown", cancelable: true }))).toBe(false);
    expect(popup().querySelector('[aria-selected="true"]')?.textContent).toContain("token");
    const down = new KeyboardEvent("keydown", { key: "ArrowDown", cancelable: true });
    expect(controller.key(down)).toBe(true);
    expect(down.defaultPrevented).toBe(false);
    expect(popup().hidden).toBe(true);
  });

  it("opens native suggestions with Ctrl+Space, even for a single candidate", async () => {
    expect(controller.key(ctrlSpace())).toBe(false);
    await vi.advanceTimersByTimeAsync(0);
    expect(popup().hidden).toBe(false);
    expect(actions()).toEqual(["query"]);
  });

  it("does not lose repeated manual requests while IPC is in flight", async () => {
    const items = [state.items[0], { index: 1, label: "another", description: "", matches: [] }];
    let resolve!: (value: typeof state) => void;
    mocks.invoke.mockImplementationOnce(() => new Promise(r => { resolve = r; }));
    mocks.invoke.mockResolvedValue({ ...state, items });
    controller.key(tab());
    controller.key(tab());
    resolve({ ...state, items }); await vi.advanceTimersByTimeAsync(1);
    expect(popup().hidden).toBe(false);
  });

  it.each([false, true])("passes Enter to the shell without accepting, pending=%s", async pending => {
    await open();
    if (pending) controller.input("A");
    const enter = new KeyboardEvent("keydown", { key: "Enter", cancelable: true });
    expect(controller.key(enter)).toBe(true);
    expect(enter.defaultPrevented).toBe(false);
    await vi.advanceTimersByTimeAsync(1000);
    expect(popup().hidden).toBe(true);
    expect(mocks.invoke.mock.calls.some(([, args]) => args.action === "accept")).toBe(false);
    expect(write).not.toHaveBeenCalled();
  });

  it("accepts a keyboard-selected candidate on Enter like Tab", async () => {
    const second = { index: 1, label: "another", description: "", matches: [] };
    mocks.invoke.mockResolvedValue({ ...state, items: [...state.items, second] });
    await open();
    controller.key(new KeyboardEvent("keydown", { key: "ArrowDown", cancelable: true }));
    const enter = new KeyboardEvent("keydown", { key: "Enter", cancelable: true });
    expect(controller.key(enter)).toBe(false);
    expect(enter.defaultPrevented).toBe(true);
    expect(mocks.invoke).toHaveBeenCalledWith("pty_completion", {
      sessionId: "session", action: "accept", revision: 5, index: 1,
    });
    expect(popup().hidden).toBe(true);
    expect(write).not.toHaveBeenCalled();
  });

  it("returns Enter to the shell once input follows a keyboard selection", async () => {
    const second = { index: 1, label: "another", description: "", matches: [] };
    mocks.invoke.mockResolvedValue({ ...state, items: [...state.items, second] });
    await open();
    controller.key(new KeyboardEvent("keydown", { key: "ArrowDown", cancelable: true }));
    controller.input("n"); await vi.advanceTimersByTimeAsync(350);
    expect(document.querySelector('[aria-selected="true"]')?.textContent).toContain("another");
    const enter = new KeyboardEvent("keydown", { key: "Enter", cancelable: true });
    expect(controller.key(enter)).toBe(true);
    expect(enter.defaultPrevented).toBe(false);
    expect(mocks.invoke.mock.calls.some(([, args]) => args.action === "accept")).toBe(false);
  });

  it.each(["push", "-lh", "中文"])("keeps exact matches visible for %s while refreshing", async token => {
    const candidate = { index: 1, label: token, description: "", matches: [] };
    mocks.invoke.mockResolvedValue({ ...state, query: token, items: [
      { ...candidate, index: 0, label: token + "x" }, candidate,
    ] });
    await open();
    expect(popup().hidden).toBe(false);
    controller.input("x"); await vi.advanceTimersByTimeAsync(350);
    expect(popup().hidden).toBe(false);
  });

  it("keeps exact basename candidates visible for path tokens", async () => {
    mocks.invoke.mockResolvedValue({ ...state, query: "/tmp/file", items: [
      { index: 0, label: "file", description: "", matches: [] },
    ] });
    await open();
    expect(popup().hidden).toBe(false);
  });

  it("finishes an in-flight refresh and immediately refreshes for the latest input", async () => {
    await open(); mocks.invoke.mockClear();
    let finish!: (value: typeof state) => void;
    mocks.invoke.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    controller.input("g"); await vi.advanceTimersByTimeAsync(350);
    controller.input("i"); controller.input("t");
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
    finish(state); await vi.advanceTimersByTimeAsync(1);
    expect(actions()).toEqual(["query", "query"]);
    expect(popup().hidden).toBe(false);
  });

  it("waits for native pending completion before querying changed input", async () => {
    await open(); mocks.invoke.mockClear();
    mocks.invoke.mockResolvedValueOnce({ ...state, pending: true });
    controller.input("g"); await vi.advanceTimersByTimeAsync(350);
    controller.input("i");
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(17);
    expect(actions()).toEqual(["query", "state", "query"]);
    expect(popup().hidden).toBe(false);
  });

  it("closes suggestions when disabled, preserves native keys and resumes when enabled", async () => {
    await open();
    expect(popup().hidden).toBe(false);
    mocks.config.mode = "off"; mocks.configChanged(); mocks.invoke.mockClear();
    controller.input("i"); await vi.advanceTimersByTimeAsync(1000);
    expect(popup().hidden).toBe(true);
    for (const event of [tab(), ctrlSpace()]) {
      expect(controller.key(event)).toBe(true);
      expect(event.defaultPrevented).toBe(false);
    }
    expect(mocks.invoke).not.toHaveBeenCalled();
    mocks.config.mode = "auto"; mocks.configChanged();
    await open();
    expect(popup().hidden).toBe(false);
  });

  it("discards pending candidates after suggestions are disabled", async () => {
    let finish!: (value: typeof state) => void;
    mocks.invoke.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
    await open();
    mocks.config.mode = "off"; mocks.configChanged();
    finish(state); await vi.advanceTimersByTimeAsync(1000);
    expect(popup().hidden).toBe(true);
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
  });

  it("discards a response received after further input", async () => {
    let resolve!: (value: typeof state) => void;
    mocks.invoke.mockImplementationOnce(() => new Promise(r => { resolve = r; }));
    await open();
    controller.input("\r"); resolve(state); await vi.advanceTimersByTimeAsync(0);
    expect(popup().hidden).toBe(true);
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
  });

  it("preserves native Tab for unsupported shells and alternate-screen programs", async () => {
    mocks.invoke.mockResolvedValue({ ...state, supported: false, ready: false, items: [] });
    controller.key(tab());
    await vi.advanceTimersByTimeAsync(0);
    expect(write).toHaveBeenCalledWith("\t");
    mocks.invoke.mockClear();
    expect(controller.key(tab())).toBe(true);
    expect(mocks.invoke).not.toHaveBeenCalled();
    Object.assign(terminal.buffer.active, { type: "alternate" });
    expect(controller.key(tab())).toBe(true);
  });

  it("does not request while composing text or searching shell history", async () => {
    textarea.dispatchEvent(new CompositionEvent("compositionstart"));
    controller.input("中"); await vi.advanceTimersByTimeAsync(500);
    expect(mocks.invoke).not.toHaveBeenCalled();
    textarea.dispatchEvent(new CompositionEvent("compositionend"));
    controller.input("\x12"); controller.input("git");
    expect(controller.key(tab())).toBe(true);
    await vi.advanceTimersByTimeAsync(500);
    expect(mocks.invoke).not.toHaveBeenCalled();
  });
  describe("Tab and the inline hint", () => {
    const accepted = { ...state, items: [], hintAccepted: true, revision: 6 };
    const settle = () => vi.advanceTimersByTime(250);

    it("names the key only while a hint shows and the terminal has focus", () => {
      mocks.config.tabBehavior = "accept";
      reportHint("1");
      settle();
      expect(tips.current).toBe("accept-tab");
      textarea.blur();
      expect(tips.current).toBeNull();
      textarea.focus();
      settle();
      expect(tips.current).toBe("accept-tab");
      reportHint("0");
      expect(tips.current).toBeNull();
    });

    it("leaves the header alone when a hint disappears before the delay ends", () => {
      reportHint("1");
      vi.advanceTimersByTime(249);
      expect(tips.current).toBeNull();
      reportHint("0");
      settle();
      expect(tips.current).toBeNull();
    });

    it("names the arrow key when Tab stays with completion, and nothing without inline hints", () => {
      reportHint("1");
      settle();
      expect(tips.current).toBe("accept-arrow");
      reportHint("0");
      mocks.config.mode = "tab";
      reportHint("1");
      settle();
      expect(tips.current).toBeNull();
    });

    it("drops the key label once the command runs", () => {
      reportHint("1");
      settle();
      controller.input("\r");
      expect(tips.current).toBeNull();
    });

    it("asks the shell to take the hint, then offers the undo", async () => {
      mocks.config.tabBehavior = "accept";
      reportHint("1");
      mocks.invoke.mockResolvedValueOnce(accepted);
      expect(controller.key(tab())).toBe(false);
      await vi.advanceTimersByTimeAsync(0);
      expect(actions()).toEqual(["tab"]);
      expect(write).not.toHaveBeenCalled();
      expect(popup().hidden).toBe(true);
      reportHint("0");
      settle();
      expect(tips.current).toBe("undo");
    });

    it("restores the typed text and opens the list on the second Tab", async () => {
      mocks.config.tabBehavior = "accept";
      mocks.invoke.mockResolvedValueOnce(accepted);
      controller.key(tab());
      await vi.advanceTimersByTimeAsync(0);
      mocks.invoke.mockClear();
      expect(controller.key(tab())).toBe(false);
      await vi.advanceTimersByTimeAsync(0);
      expect(mocks.invoke.mock.calls[0][1]).toMatchObject({ action: "undo", revision: 6 });
      expect(actions()).toEqual(["undo", "query"]);
      expect(popup().hidden).toBe(false);
    });

    it("forgets the undo as soon as the user types", async () => {
      mocks.config.tabBehavior = "accept";
      mocks.invoke.mockResolvedValueOnce(accepted);
      controller.key(tab());
      await vi.advanceTimersByTimeAsync(0);
      controller.input("x");
      expect(tips.current).toBeNull();
      mocks.invoke.mockClear();
      controller.key(tab());
      await vi.advanceTimersByTimeAsync(0);
      expect(actions()[0]).toBe("tab");
    });

    it("hides the label while the list is open, since Tab then takes the highlighted item", async () => {
      reportHint("1");
      settle();
      await controller.key(ctrlSpace());
      await vi.advanceTimersByTimeAsync(0);
      expect(popup().hidden).toBe(false);
      expect(tips.current).toBeNull();
    });

    it("keeps Tab on completion when that behavior is chosen", async () => {
      reportHint("1");
      controller.key(tab());
      await vi.advanceTimersByTimeAsync(0);
      expect(actions()[0]).toBe("query");
    });
  });
});
