//! Tests for the composition caret inserted into xterm's pre-edit overlay; see imeCaret.ts.
//!
//! xterm's own compositionupdate handling (writing `ev.data` into the overlay) is simulated by a listener on
//! the textarea, which runs before the module's bubble-phase listener on the container.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { IME_CARET_CLASS, installImeCaret, type ImeCaret } from "./imeCaret";

function setup() {
  const container = document.createElement("div");
  const helpers = document.createElement("div");
  const textarea = document.createElement("textarea");
  textarea.className = "xterm-helper-textarea";
  const view = document.createElement("div");
  view.className = "composition-view";
  helpers.append(textarea, view);
  container.appendChild(helpers);
  document.body.appendChild(container);
  // Stand-in for xterm's CompositionHelper.compositionupdate.
  textarea.addEventListener("compositionupdate", (e) => {
    view.textContent = (e as unknown as { data: string }).data;
  });
  const caret: ImeCaret = installImeCaret(container);
  return {
    textarea,
    view,
    cleanup: () => {
      caret.dispose();
      container.remove();
    },
  };
}

function fireComposition(target: HTMLElement, type: string, data = "") {
  const ev = new Event(type, { bubbles: true });
  Object.defineProperty(ev, "data", { value: data });
  target.dispatchEvent(ev);
}

/** Text nodes and caret marker of the overlay, in order, as a compact string. */
function layout(view: HTMLElement): string {
  return Array.from(view.childNodes)
    .map((n) =>
      n instanceof HTMLElement && n.classList.contains(IME_CARET_CLASS) ? "|" : n.textContent,
    )
    .join("");
}

describe("installImeCaret", () => {
  let s: ReturnType<typeof setup>;
  beforeEach(() => {
    vi.useFakeTimers();
    s = setup();
    s.textarea.focus();
  });
  afterEach(() => {
    s.cleanup();
    vi.useRealTimers();
  });

  it("places the caret at the selection offset inside the pre-edit text", () => {
    s.textarea.value = "ab";
    fireComposition(s.textarea, "compositionstart");
    s.textarea.value = "ab測試中";
    s.textarea.setSelectionRange(3, 3);
    fireComposition(s.textarea, "compositionupdate", "測試中");
    vi.runAllTimers();
    expect(layout(s.view)).toBe("測|試中");
    expect(s.view.textContent).toBe("測試中");
  });

  it("follows caret movement reported through selectionchange", () => {
    fireComposition(s.textarea, "compositionstart");
    s.textarea.value = "測試";
    s.textarea.setSelectionRange(2, 2);
    fireComposition(s.textarea, "compositionupdate", "測試");
    vi.runAllTimers();
    expect(layout(s.view)).toBe("測試|");
    s.textarea.setSelectionRange(0, 0);
    document.dispatchEvent(new Event("selectionchange"));
    vi.runAllTimers();
    expect(layout(s.view)).toBe("|測試");
  });

  it("falls back to the end when the selection lies outside the composition", () => {
    s.textarea.value = "abc";
    fireComposition(s.textarea, "compositionstart");
    s.textarea.value = "abc測";
    s.textarea.setSelectionRange(1, 1);
    fireComposition(s.textarea, "compositionupdate", "測");
    vi.runAllTimers();
    expect(layout(s.view)).toBe("測|");
  });

  it("removes the caret and stops rendering after compositionend", () => {
    fireComposition(s.textarea, "compositionstart");
    s.textarea.value = "測";
    s.textarea.setSelectionRange(1, 1);
    fireComposition(s.textarea, "compositionupdate", "測");
    vi.runAllTimers();
    expect(s.view.querySelector("." + IME_CARET_CLASS)).not.toBeNull();
    fireComposition(s.textarea, "compositionend");
    expect(s.view.querySelector("." + IME_CARET_CLASS)).toBeNull();
    document.dispatchEvent(new Event("selectionchange"));
    vi.runAllTimers();
    expect(s.view.querySelector("." + IME_CARET_CLASS)).toBeNull();
  });
});
