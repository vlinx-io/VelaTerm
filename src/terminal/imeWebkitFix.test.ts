//! Regression tests for the WebKit IME fallback, especially reversed-event duplicate output.
//!
//! WebKit may deliver full-width punctuation input before its own keydown. If that late keydown
//! creates a keyup fallback marker, keyup sends the key's half-width character again, producing
//! duplicates such as `”"` or `，,`. REORDERED_KEYDOWN_MS recognizes and ignores that late keydown;
//! see imeWebkitFix.ts.
//!
//! Tests avoid relying on jsdom's InputEvent/KeyboardEvent field support. Plain Events receive
//! `data`, `inputType`, `keyCode`, and related properties through defineProperty.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Terminal } from "@xterm/xterm";

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
  return {
    textarea,
    sent,
    fix,
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
  init: { code: string; key: string; keyCode?: number; isComposing?: boolean },
) {
  const ev = new KeyboardEvent(type, {
    bubbles: true,
    code: init.code,
    key: init.key,
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

  it("does not send both halves of one slash keystroke, which is how a typed / appeared as //", () => {
    // The `/` key is the one symbol that passes both gates: `keyup` reports its half-width source, which is a
    // pass-through symbol, and the IME's conversion arrives later through `input` as a full-width `／`, which is
    // full-width punctuation. Each channel committed separately, so one keystroke wrote two characters.
    const { textarea, sent, fix, cleanup } = setup();
    fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229 });
    fireKey(textarea, "keyup", { code: "Slash", key: "/" });
    expect(sent).toEqual(["/"]);
    // The late input carries the full-width form; it is the same keystroke and must be consumed.
    expect(fix.shouldSwallow("／")).toBe(true);
    cleanup();
  });

  it("still deduplicates a duplicate that arrives after a long pause", () => {
    // The duplicate lands with the next keystroke, which may be much later than any fixed time window. The
    // former five-second TTL expired while the user sat idle, so the next keystroke's arrival released the
    // duplicate and the character was sent twice.
    const { textarea, sent, fix, cleanup } = setup();
    fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229 });
    fireKey(textarea, "keyup", { code: "Slash", key: "/" });
    now += 30_000;
    fireKey(textarea, "keydown", { code: "KeyS", key: "Process", keyCode: 229 });
    expect(sent).toEqual(["/"]);
    expect(fix.shouldSwallow("/")).toBe(true);
    cleanup();
  });

  it("keeps two deliberately typed slashes and never swallows later ordinary input", () => {
    const { textarea, sent, fix, cleanup } = setup();
    for (let i = 0; i < 2; i++) {
      fireKey(textarea, "keydown", { code: "Slash", key: "Process", keyCode: 229 });
      fireKey(textarea, "keyup", { code: "Slash", key: "/" });
    }
    expect(sent).toEqual(["/", "/"]); // Both keystrokes belong in the PTY.
    // A registration is consumed and the table then empties, so unrelated later input is untouched.
    expect(fix.shouldSwallow("/")).toBe(true);
    expect(fix.shouldSwallow("/")).toBe(true);
    expect(fix.shouldSwallow("/")).toBe(false);
    expect(fix.shouldSwallow("a")).toBe(false);
    cleanup();
  });
});
