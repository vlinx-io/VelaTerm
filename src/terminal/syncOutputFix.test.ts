//! Regression tests for the DEC mode 2026 incremental-refresh workaround in syncOutputFix.ts.
//!
//! Opens a real @xterm/xterm Terminal in jsdom so InputHandler, RenderService, RenderDebouncer, and
//! DomRenderer all run end-to-end. Verifies that exiting synchronized output (`CSI ? 2026 l`) only
//! re-renders rows dirtied during the frame rather than calling replaceChildren on all viewport rows.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Terminal } from "@xterm/xterm";

import { installSyncOutputFix } from "./syncOutputFix";

interface RenderRange {
  start: number;
  end: number;
}

const cleanups: (() => void)[] = [];

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("matchMedia", () => ({
    matches: false,
    addListener() {},
    removeListener() {},
  }));
});

afterEach(() => {
  cleanups.splice(0).forEach((fn) => fn());
  vi.clearAllTimers();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

async function feedAndFlush(term: Terminal, data: string): Promise<void> {
  let parsed = false;
  term.write(data, () => {
    parsed = true;
  });
  await vi.advanceTimersByTimeAsync(0);
  if (!parsed) {
    await vi.advanceTimersByTimeAsync(20);
  }
  expect(parsed).toBe(true);
  await vi.advanceTimersByTimeAsync(20);
}

async function feedOnly(term: Terminal, data: string): Promise<void> {
  let parsed = false;
  term.write(data, () => {
    parsed = true;
  });
  await vi.advanceTimersByTimeAsync(0);
  expect(parsed).toBe(true);
}

function setupTerminal(cols = 80, rows = 45) {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const term = new Terminal({ cols, rows, allowProposedApi: true });
  term.open(container);
  const disposeFix = installSyncOutputFix(term);
  const renders: RenderRange[] = [];
  const renderSub = term.onRender((r) => renders.push(r));
  const replies: string[] = [];
  const dataSub = term.onData((d) => replies.push(d));

  cleanups.push(() => {
    dataSub.dispose();
    renderSub.dispose();
    disposeFix();
    term.dispose();
    container.remove();
  });

  const rowsEl = container.querySelector(".xterm-rows") as HTMLElement | null;
  expect(rowsEl).not.toBeNull();

  return { term, container, rowsEl: rowsEl!, renders, replies, disposeFix };
}

describe("syncOutputFix", () => {
  it("only refreshes the dirty prompt row on single-chunk DEC 2026 keystroke echoes", async () => {
    const { term, rowsEl, renders } = setupTerminal(80, 45);

    // Populate rows 0..39 with styled content and park the prompt on row 42 (1-based 43).
    let setupSeq = "";
    for (let y = 0; y < 40; y++) {
      setupSeq += `\x1b[${y + 1};1H\x1b[36mline-${y}\x1b[m styled content #${y}`;
    }
    setupSeq += "\x1b[43;1H\x1b[94m>\x1b[m ";
    await feedAndFlush(term, setupSeq);

    const row0Child = rowsEl.children[0]?.firstChild;
    const row20Child = rowsEl.children[20]?.firstChild;
    const row39Child = rowsEl.children[39]?.firstChild;
    expect(row0Child).toBeTruthy();
    expect(row20Child).toBeTruthy();
    expect(row39Child).toBeTruthy();

    // First, verify an ASCII single-character sync frame (matching agy's exact keystroke echo).
    renders.length = 0;
    await feedAndFlush(term, "\x1b[?2026hh\x1b[?2026l");
    expect(term.modes.synchronizedOutputMode).toBe(false);
    expect(renders).toEqual([{ start: 42, end: 42 }]);
    expect(rowsEl.children[0].firstChild).toBe(row0Child);
    expect(rowsEl.children[20].firstChild).toBe(row20Child);
    expect(rowsEl.children[39].firstChild).toBe(row39Child);

    // Next, verify a CJK single-line prompt redraw wrapped in DEC 2026.
    renders.length = 0;
    await feedAndFlush(
      term,
      "\x1b[?2026h\x1b[?7l\r\x1b[K\x1b[94m>\x1b[m hello\u4f60\u597d\x1b[?7h\x1b[12G\x1b[?2026l",
    );
    expect(term.modes.synchronizedOutputMode).toBe(false);
    expect(renders).toEqual([{ start: 42, end: 42 }]);
    expect(rowsEl.children[0].firstChild).toBe(row0Child);
    expect(rowsEl.children[20].firstChild).toBe(row20Child);
    expect(rowsEl.children[39].firstChild).toBe(row39Child);
  });

  it("accumulates buffered dirty rows across chunks and leaves untouched rows intact", async () => {
    const { term, rowsEl, renders } = setupTerminal(80, 45);

    let init = "";
    for (let y = 0; y < 10; y++) {
      init += `\x1b[${y + 1};1Htop-${y}`;
    }
    // Park cursor on the prompt row before starting the synchronized frame so row 9 is not dirtied
    // solely by cursor movement away from it.
    init += "\x1b[43;1H";
    await feedAndFlush(term, init);
    const row0Child = rowsEl.children[0].firstChild;
    const row9Child = rowsEl.children[9].firstChild;

    renders.length = 0;
    // Chunk 1 opens synchronized output and updates row 10 (1-based 11) and row 42 (1-based 43).
    await feedAndFlush(term, "\x1b[?2026h\x1b[11;1Hheader\x1b[43;1H");
    expect(term.modes.synchronizedOutputMode).toBe(true);
    expect(renders).toEqual([]);

    // Chunk 2 prints prompt text on row 42 and exits synchronized output.
    await feedAndFlush(term, "prompt\x1b[?2026l");
    expect(term.modes.synchronizedOutputMode).toBe(false);
    expect(renders).toEqual([{ start: 10, end: 42 }]);
    expect(rowsEl.children[0].firstChild).toBe(row0Child);
    expect(rowsEl.children[9].firstChild).toBe(row9Child);
  });

  it("flushes buffered dirty rows when the trailing chunk contains only CSI ? 2026 l", async () => {
    const { term, renders } = setupTerminal(80, 45);
    await feedAndFlush(term, "\x1b[43;1H");

    renders.length = 0;
    await feedAndFlush(term, "\x1b[?2026h\x1b[16;1Hmiddle\x1b[43;1H");
    expect(term.modes.synchronizedOutputMode).toBe(true);
    expect(renders).toEqual([]);

    await feedAndFlush(term, "\x1b[?2026l");
    expect(term.modes.synchronizedOutputMode).toBe(false);
    expect(renders).toEqual([{ start: 15, end: 42 }]);
  });

  it("handles compound DECRST sequences alongside mode 2026", async () => {
    const { term, renders, replies } = setupTerminal(80, 45);
    await feedAndFlush(term, "\x1b[?25h\x1b[?2004h\x1b[43;1H");

    // Reset cursor visibility (25), bracketed paste (2004), and synchronized output (2026) together.
    renders.length = 0;
    await feedAndFlush(term, "\x1b[?2026hx\x1b[?25;2004;2026l");
    expect(term.modes.synchronizedOutputMode).toBe(false);
    expect(term.modes.bracketedPasteMode).toBe(false);
    expect(renders).toEqual([{ start: 42, end: 42 }]);

    // Verify mode 25 was actually reset via DECRQM.
    replies.length = 0;
    await feedAndFlush(term, "\x1b[?25$p");
    expect(replies).toContain("\x1b[?25;2$y");

    // Alt-screen exit combined with 2026 reset should still refresh the restored normal screen buffer.
    await feedAndFlush(term, "\x1b[?2026h\x1b[?1049h\x1b[1;1Halt");
    expect(term.buffer.active.type).toBe("alternate");
    renders.length = 0;
    await feedAndFlush(term, "\x1b[?1049;2026l");
    expect(term.buffer.active.type).toBe("normal");
    expect(term.modes.synchronizedOutputMode).toBe(false);
    expect(renders).toEqual([{ start: 0, end: 44 }]);
  });

  it("passes non-2026 DECRST sequences through unchanged", async () => {
    const { term, renders } = setupTerminal(80, 45);
    await feedAndFlush(term, "\x1b[?2004h");
    expect(term.modes.bracketedPasteMode).toBe(true);

    renders.length = 0;
    await feedAndFlush(term, "\x1b[?2004l");
    expect(term.modes.bracketedPasteMode).toBe(false);
  });

  it("preserves xterm's 1000 ms synchronized-output safety timeout", async () => {
    const { term, renders } = setupTerminal(80, 45);
    await feedAndFlush(term, "\x1b[1;1Hready");

    renders.length = 0;
    await feedAndFlush(term, "\x1b[?2026h\x1b[3;1Hstuck");
    expect(term.modes.synchronizedOutputMode).toBe(true);
    expect(renders).toEqual([]);

    await vi.advanceTimersByTimeAsync(1000);
    await vi.advanceTimersByTimeAsync(20);
    expect(term.modes.synchronizedOutputMode).toBe(false);
    expect(renders.length).toBe(1);
  });

  it("survives term.reset(), RIS (ESC c), and DECSTR (CSI ! p) without stale mode references", async () => {
    const { term, renders } = setupTerminal(80, 45);

    // term.reset() clones a new decPrivateModes object via CoreService.reset().
    term.reset();
    await feedAndFlush(term, "\x1b[1;1Hrow-0\x1b[43;1H");
    renders.length = 0;
    await feedAndFlush(term, "\x1b[?2026ha\x1b[?2026l");
    expect(term.modes.synchronizedOutputMode).toBe(false);
    expect(renders).toEqual([{ start: 42, end: 42 }]);

    // RIS (ESC c) triggers full terminal reset.
    await feedAndFlush(term, "\x1bc\x1b[43;1H");
    renders.length = 0;
    await feedAndFlush(term, "\x1b[?2026hb\x1b[?2026l");
    expect(term.modes.synchronizedOutputMode).toBe(false);
    expect(renders).toEqual([{ start: 42, end: 42 }]);

    // DECSTR (CSI ! p) resets CoreService modes without clearing the buffer.
    await feedAndFlush(term, "\x1b[!p\x1b[43;1H");
    renders.length = 0;
    await feedAndFlush(term, "\x1b[?2026hc\x1b[?2026l");
    expect(term.modes.synchronizedOutputMode).toBe(false);
    expect(renders).toEqual([{ start: 42, end: 42 }]);
  });

  it("ignores colon subparameter arrays in compound DECRST sequences", async () => {
    const { term, replies } = setupTerminal(100, 45);
    term.options.windowOptions = { setWinLines: true };
    await feedAndFlush(term, "\x1b[?25h");

    // In Params.toArray(), "25:3;2026" yields [25, [3], 2026]. [3] must not invoke DECCOLM (mode 3).
    await feedAndFlush(term, "\x1b[?2026hx\x1b[?25:3;2026l");
    expect(term.cols).toBe(100);
    expect(term.modes.synchronizedOutputMode).toBe(false);

    replies.length = 0;
    await feedAndFlush(term, "\x1b[?25$p");
    expect(replies).toContain("\x1b[?25;2$y");
  });

  it("restores default CSI ? 2026 l behavior after disposal", async () => {
    const { term, renders, disposeFix } = setupTerminal(80, 45);
    await feedAndFlush(term, "\x1b[43;1H");

    disposeFix();
    renders.length = 0;
    await feedOnly(term, "\x1b[?2026hh\x1b[?2026l");
    await vi.advanceTimersByTimeAsync(20);

    expect(term.modes.synchronizedOutputMode).toBe(false);
    // Built-in xterm 6.0.0 behavior refreshes the entire viewport when the fix is disposed.
    expect(renders).toEqual([{ start: 0, end: 44 }]);
  });
});
