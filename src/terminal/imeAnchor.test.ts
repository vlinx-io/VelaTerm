//! Regression test for committing IME text after the helper textarea caret was moved; see imeAnchor.ts.
//!
//! Runs against a real xterm instance, because the fault is in how its CompositionHelper slices the textarea
//! value, and a stand-in would only prove the stand-in. jsdom lacks `matchMedia`, which xterm needs to open.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Terminal } from "@xterm/xterm";

import { installImeAnchor } from "./imeAnchor";

if (!window.matchMedia) {
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: () => ({
      matches: false,
      addEventListener() {},
      removeEventListener() {},
      addListener() {},
      removeListener() {},
    }),
  });
}

const cleanups: (() => void)[] = [];
beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  cleanups.splice(0).forEach((fn) => fn());
  vi.useRealTimers();
});

function setup() {
  const host = document.createElement("div");
  document.body.appendChild(host);
  const term = new Terminal();
  term.open(host);
  const textarea = term.textarea!;
  textarea.focus(); // The anchor only reacts to selection changes of the focused textarea.
  const anchor = installImeAnchor(host, textarea);
  const emitted: string[] = [];
  term.onData((d) => emitted.push(d));
  cleanups.push(() => {
    anchor.dispose();
    term.dispose();
    host.remove();
  });
  return { textarea, emitted };
}

/** What a browser does for an IME: the pre-edit text is inserted at the textarea caret. */
function compose(textarea: HTMLTextAreaElement, text: string) {
  textarea.dispatchEvent(new CompositionEvent("compositionstart", { data: "" }));
  const at = textarea.selectionStart;
  textarea.value = textarea.value.slice(0, at) + text + textarea.value.slice(at);
  textarea.setSelectionRange(at + text.length, at + text.length);
  textarea.dispatchEvent(new CompositionEvent("compositionupdate", { data: text }));
  vi.runAllTimers();
  textarea.dispatchEvent(new CompositionEvent("compositionend", { data: text }));
  vi.runAllTimers(); // xterm emits the commit from a zero-delay timer after compositionend
}

/** Left/Right moving the textarea's own caret, which xterm does not cancel. */
function moveCaret(textarea: HTMLTextAreaElement, to: number) {
  textarea.setSelectionRange(to, to);
  document.dispatchEvent(new Event("selectionchange"));
}

describe("installImeAnchor", () => {
  it("emits the composed text after the caret was moved inside earlier input", () => {
    const { textarea, emitted } = setup();
    textarea.value = "abcdef";
    moveCaret(textarea, 3);
    compose(textarea, "你好");
    expect(emitted).toEqual(["你好"]);
  });

  it("emits only the typed character for a non-composed IME keystroke after the caret moved", () => {
    // Pass-through symbols reach xterm as keydown 229 followed by a plain insert, with no composition events;
    // xterm diffs the textarea value, so a mid-value insert used to be emitted as the whole shifted tail.
    const { textarea, emitted } = setup();
    textarea.value = "abcdef";
    moveCaret(textarea, 3);
    textarea.dispatchEvent(Object.assign(new Event("keydown", { bubbles: true }), { keyCode: 229, key: "Process" }));
    const at = textarea.selectionStart;
    textarea.value = textarea.value.slice(0, at) + "/" + textarea.value.slice(at);
    textarea.setSelectionRange(at + 1, at + 1);
    vi.runAllTimers();
    expect(emitted).toEqual(["/"]);
  });

  it("repairs a caret moved with no selectionchange event, at compositionstart", () => {
    const { textarea, emitted } = setup();
    textarea.value = "abcdef";
    textarea.setSelectionRange(0, 0);
    compose(textarea, "你好");
    expect(emitted).toEqual(["你好"]);
  });

  it("leaves the caret alone while composing, so arrows edit the pre-edit text", () => {
    const { textarea } = setup();
    textarea.value = "ab你好";
    textarea.setSelectionRange(4, 4);
    textarea.dispatchEvent(new CompositionEvent("compositionstart", { data: "" }));
    moveCaret(textarea, 3);
    expect(textarea.selectionStart).toBe(3);
  });
});
