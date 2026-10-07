//! Regression tests for the WebKit IME fallback, especially reversed-event duplicate output.
//!
//! WebKit may deliver full-width punctuation input before its own keydown. If that late keydown
//! creates a keyup fallback marker, keyup sends the key's half-width character again, producing
//! duplicates such as `”"` or `，,`. REORDERED_KEYDOWN_MS recognizes and ignores that late keydown;
//! see imeWebkitFix.ts.
//!
//! Additional cases cover native onData preceding the keyup fallback, composition lifecycle guards,
//! and the real xterm textarea-change/composition timers used before and after cursor movement.
//!
//! Tests avoid relying on jsdom's InputEvent/KeyboardEvent field support. Plain Events receive
//! `data`, `inputType`, `keyCode`, and related properties through defineProperty.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Terminal } from "@xterm/xterm";

import { installWebkitImeFix, type WebkitImeFix } from "./imeWebkitFix";

/** Create a minimal parent/textarea DOM, install the fallback, and collect emitted data. */
function setup() {
  const container = document.createElement("div");
  const textarea = document.createElement("textarea");
  container.appendChild(textarea);
  document.body.appendChild(container);
  const sent: string[] = [];
  const fix: WebkitImeFix = installWebkitImeFix(
    { textarea } as unknown as Terminal,
    (d) => sent.push(d),
  );
  // Mirror usePtySession's onData path so both send orders reach the same PTY collector.
  const nativeData = (data: string) => {
    if (!fix.shouldSwallow(data)) sent.push(data);
  };
  return {
    textarea,
    sent,
    fix,
    nativeData,
    cleanup: () => {
      fix.dispose();
      container.remove();
    },
  };
}

/** Dispatch an input event, defaulting to insertText outside composition. */
function fireInput(
  textarea: HTMLTextAreaElement,
  data: string,
  opts: { isComposing?: boolean; inputType?: string } = {},
) {
  const ev = new Event("input", { bubbles: true });
  Object.defineProperties(ev, {
    data: { value: data },
    inputType: { value: opts.inputType ?? "insertText" },
    isComposing: { value: opts.isComposing ?? false },
  });
  textarea.dispatchEvent(ev);
}

/** Dispatch a keyboard event; define the legacy read-only keyCode separately. */
function fireKey(
  textarea: HTMLTextAreaElement,
  type: "keydown" | "keyup",
  init: { code: string; key: string; keyCode?: number; isComposing?: boolean; ctrlKey?: boolean; altKey?: boolean; metaKey?: boolean },
) {
  const ev = new KeyboardEvent(type, {
    bubbles: true,
    code: init.code,
    key: init.key,
    ctrlKey: init.ctrlKey,
    altKey: init.altKey,
    metaKey: init.metaKey,
  });
  Object.defineProperties(ev, {
    keyCode: { value: init.keyCode ?? 0 },
    isComposing: { value: init.isComposing ?? false },
  });
  textarea.dispatchEvent(ev);
}

describe("imeWebkitFix", () => {
  let nowSpy: ReturnType<typeof vi.spyOn>;
  let now = 100_000;

  beforeEach(() => {
    now = 100_000;
    nowSpy = vi.spyOn(Date, "now").mockImplementation(() => now);
  });

  afterEach(() => {
    nowSpy.mockRestore();
  });

  it("out-of-order double-send regression, quote: input commits `”` first, and the late keydown + keyup must not also send a halfwidth `\"`", () => {
    const { textarea, sent, fix, cleanup } = setup();
    // Observed WebKit order: input commits first, followed by that key's keydown(229).
    fireInput(textarea, "”");
    fireKey(textarea, "keydown", { code: "Quote", key: "”", keyCode: 229 });
    fireKey(textarea, "keyup", { code: "Quote", key: '"' }); // keyup may contain the half-width original.
    expect(sent).toEqual(["”"]); // Emit only the full-width quote.
    // Swallow the same sequence when it later arrives from xterm.
    expect(fix.shouldSwallow("”")).toBe(true);
    cleanup();
  });

  it("out-of-order double-send regression, comma: input commits `，` first, and the late keydown + keyup must not also send a halfwidth `,`", () => {
    const { textarea, sent, cleanup } = setup();
    fireInput(textarea, "，");
    fireKey(textarea, "keydown", { code: "Comma", key: "，", keyCode: 229 });
    fireKey(textarea, "keyup", { code: "Comma", key: "," });
    expect(sent).toEqual(["，"]);
    cleanup();
  });

  it("the normal order (keydown → input → keyup) does not double-send either: input clears the flag so keyup adds nothing", () => {
    const { textarea, sent, cleanup } = setup();
    fireKey(textarea, "keydown", { code: "Comma", key: "，", keyCode: 229 });
    fireInput(textarea, "，");
    fireKey(textarea, "keyup", { code: "Comma", key: "," });
    expect(sent).toEqual(["，"]);
    cleanup();
  });

  it("pass-through halfwidth symbols such as % are still emitted by keyup immediately, unaffected by the out-of-order guard", () => {
    const { textarea, sent, cleanup } = setup();
    // WebKit delays passthrough-symbol input until the next key, leaving only the keydown marker at keyup.
    fireKey(textarea, "keydown", { code: "Digit5", key: "Process", keyCode: 229 });
    fireKey(textarea, "keyup", { code: "Digit5", key: "%" });
    expect(sent).toEqual(["%"]);
    cleanup();
  });

  it("the detection window blocks only one batch of events: once a fullwidth punctuation commit falls outside it, the next pass-through symbol is emitted as usual", () => {
    const { textarea, sent, cleanup } = setup();
    fireInput(textarea, "，"); // Commit immediately and record the time.
    now += 200; // Exceed REORDERED_KEYDOWN_MS to simulate a subsequent keystroke.
    fireKey(textarea, "keydown", { code: "Digit5", key: "Process", keyCode: 229 });
    fireKey(textarea, "keyup", { code: "Digit5", key: "%" });
    expect(sent).toEqual(["，", "%"]);
    cleanup();
  });

  it("leaves composing input alone entirely, such as typing pinyin and choosing a candidate", () => {
    const { textarea, sent, cleanup } = setup();
    fireInput(textarea, "ceshi", { isComposing: true, inputType: "insertCompositionText" });
    fireKey(textarea, "keydown", { code: "KeyC", key: "Process", keyCode: 229 });
    fireKey(textarea, "keyup", { code: "KeyC", key: "c" }); // Letters are excluded by isPassthroughSymbol.
    expect(sent).toEqual([]);
    cleanup();
  });

  it("does not resend a slash that xterm emitted before keyup", () => {
    const { textarea, sent, nativeData, cleanup } = setup();
    fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229 });
    nativeData("/");
    fireKey(textarea, "keyup", { code: "Slash", key: "/" });
    expect(sent).toEqual(["/"]);
    cleanup();
  });

  it("still sends a delayed slash immediately and consumes xterm's later copy", () => {
    const { textarea, sent, nativeData, cleanup } = setup();
    fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229 });
    fireKey(textarea, "keyup", { code: "Slash", key: "/" });
    expect(sent).toEqual(["/"]);
    nativeData("/");
    expect(sent).toEqual(["/"]);
    cleanup();
  });

  it("preserves two intentional slashes when xterm sends each before keyup", () => {
    const { textarea, sent, nativeData, cleanup } = setup();
    for (let i = 0; i < 2; i++) {
      fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229 });
      nativeData("/");
      fireKey(textarea, "keyup", { code: "Slash", key: "/" });
      now += 100;
    }
    expect(sent.join("")).toBe("//");
    cleanup();
  });

  it("does not mistake the previous slash's late copy for the current key's native send", () => {
    const { textarea, sent, nativeData, cleanup } = setup();
    fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229 });
    fireKey(textarea, "keyup", { code: "Slash", key: "/" });
    now += 100;
    fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229 });
    nativeData("/"); // This is the previous key's duplicate, not the current slash.
    fireKey(textarea, "keyup", { code: "Slash", key: "/" });
    nativeData("/");
    expect(sent.join("")).toBe("//");
    cleanup();
  });

  it("leaves slash keyups during composition to xterm even when isComposing is false", () => {
    const { textarea, sent, nativeData, cleanup } = setup();
    textarea.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
    fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229 });
    fireKey(textarea, "keyup", { code: "Slash", key: "/", isComposing: false });
    expect(sent).toEqual([]);
    textarea.dispatchEvent(new CompositionEvent("compositionend", { bubbles: true, data: "测试/" }));
    nativeData("测试/");
    expect(sent).toEqual(["测试/"]);
    cleanup();
  });

  it("disarms the slash fallback at compositionstart and preserves Chinese after cursor movement", () => {
    const { textarea, sent, nativeData, cleanup } = setup();
    fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229 });
    textarea.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
    textarea.dispatchEvent(new CompositionEvent("compositionend", { bubbles: true, data: "测试" }));
    fireKey(textarea, "keyup", { code: "Slash", key: "/" });
    nativeData("测试");
    fireKey(textarea, "keydown", { code: "ArrowLeft", key: "ArrowLeft", keyCode: 37 });
    nativeData("\x1b[D");
    expect(sent).toEqual(["测试", "\x1b[D"]);
    cleanup();
  });

  it("does not keep an IME slash marker across a non-IME keydown or focus loss", () => {
    const { textarea, sent, cleanup } = setup();
    fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229 });
    fireKey(textarea, "keydown", { code: "ArrowLeft", key: "ArrowLeft", keyCode: 37 });
    fireKey(textarea, "keyup", { code: "Slash", key: "/" });
    fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229 });
    textarea.dispatchEvent(new Event("blur"));
    fireKey(textarea, "keyup", { code: "Slash", key: "/" });
    expect(sent).toEqual([]);
    cleanup();
  });

  it("keeps the delayed-symbol fallback when keydown alone reports isComposing", () => {
    const { textarea, sent, cleanup } = setup();
    fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229, isComposing: true });
    fireKey(textarea, "keyup", { code: "Slash", key: "/" });
    expect(sent).toEqual(["/"]);
    cleanup();
  });

  it.each(["ctrlKey", "altKey", "metaKey"] as const)("does not insert a slash for a %s shortcut", modifier => {
    const { textarea, sent, nativeData, cleanup } = setup();
    fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229, [modifier]: true });
    nativeData("\x1b/");
    fireKey(textarea, "keyup", { code: "Slash", key: "/", [modifier]: true });
    expect(sent).toEqual(["\x1b/"]);
    cleanup();
  });

  it("keeps ordinary English slashes and Chinese commits outside the fallback", () => {
    const { textarea, sent, nativeData, cleanup } = setup();
    fireKey(textarea, "keydown", { code: "Slash", key: "/", keyCode: 191 });
    nativeData("/");
    fireKey(textarea, "keyup", { code: "Slash", key: "/" });
    nativeData("中文");
    expect(sent).toEqual(["/", "中文"]);
    cleanup();
  });
});

describe("imeWebkitFix with xterm's native input handlers", () => {
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
    cleanups.splice(0).forEach(cleanup => cleanup());
    vi.clearAllTimers();
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  function setupNative() {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const term = new Terminal({ cols: 40, rows: 10 });
    term.open(container);
    const sent: string[] = [];
    const fix = installWebkitImeFix(term, data => sent.push(data));
    const sub = term.onData(data => {
      if (!fix.shouldSwallow(data)) sent.push(data);
    });
    cleanups.push(() => { sub.dispose(); fix.dispose(); term.dispose(); container.remove(); });
    return { textarea: term.textarea!, sent };
  }

  it("sends one slash when xterm's textarea-change timer runs before keyup", () => {
    const { textarea, sent } = setupNative();
    fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229 });
    textarea.value = "/";
    vi.advanceTimersByTime(1);
    expect(sent).toEqual(["/"]);
    fireKey(textarea, "keyup", { code: "Slash", key: "/" });
    expect(sent).toEqual(["/"]);
  });

  it("preserves native Chinese composition after a delayed slash and an arrow key", () => {
    const { textarea, sent } = setupNative();
    fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229 });
    fireKey(textarea, "keyup", { code: "Slash", key: "/" });
    textarea.value = "/";
    vi.advanceTimersByTime(1);
    fireKey(textarea, "keydown", { code: "ArrowLeft", key: "ArrowLeft", keyCode: 37 });
    fireKey(textarea, "keyup", { code: "ArrowLeft", key: "ArrowLeft", keyCode: 37 });
    textarea.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
    textarea.dispatchEvent(new CompositionEvent("compositionupdate", { bubbles: true, data: "ceshi" }));
    textarea.value = "/ceshi";
    vi.advanceTimersByTime(1);
    textarea.dispatchEvent(new CompositionEvent("compositionend", { bubbles: true, data: "测试" }));
    textarea.value = "/测试";
    vi.advanceTimersByTime(1);
    expect(sent).toEqual(["/", "\x1b[D", "测试"]);
  });
  it("consumes every duplicate after a burst longer than the old deduplication cap, so no character is sent twice", () => {
    // A burst of pass-through symbols within the reorder window used to exceed the 16-entry cap; the oldest
    // registrations were dropped and their late duplicates reached the PTY, so a typed `/` arrived as `//`.
    const symbols = ["`", "~", "!", "@", "#", "$", "%", "^", "&", "*", "(", ")", "-", "_", "=", "+", "[", "{", "]", "}", ";", ":", "'", '"'];
    const { textarea, sent, fix, cleanup } = setup();
    for (const symbol of symbols) {
      fireKey(textarea, "keydown", { code: "KeyX", key: "Process", keyCode: 229 });
      fireKey(textarea, "keyup", { code: "KeyX", key: symbol });
    }
    expect(sent).toEqual(symbols); // Each symbol committed exactly once, in order.
    // xterm delivers each duplicate afterwards, oldest first; every one must be swallowed.
    expect(symbols.map((symbol) => fix.shouldSwallow(symbol))).toEqual(symbols.map(() => true));
    cleanup();
  });
});
