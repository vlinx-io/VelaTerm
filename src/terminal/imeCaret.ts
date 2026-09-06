//! Composition caret for the IME pre-edit overlay.
//!
//! xterm mirrors the pre-edit string into `.composition-view` as plain text and draws no caret there. The
//! overlay is opaque and covers the terminal's own cursor, and the hidden textarea's native caret is
//! transparent (see the `.xterm-helper-textarea` rule in index.css), so while composing nothing marks the
//! caret and ←/→ inside the pre-edit text give no visual feedback (issue #59).
//!
//! This module inserts a zero-width caret marker into the overlay at the composition caret. The offset comes
//! from the textarea: while composing, the browser keeps the textarea's selection at the caret inside the
//! composition, so `selectionStart` minus the value length recorded at compositionstart is the caret's offset
//! into the pre-edit text. The marker is empty, so the overlay's `textContent` still equals the pre-edit
//! string and xterm's own handling is unaffected; xterm replaces the overlay content on every
//! compositionupdate, and the marker is re-inserted after it.

/** Class of the caret marker; styled in index.css. */
export const IME_CARET_CLASS = "vlx-ime-caret";

export interface ImeCaret {
  dispose: () => void;
}

/**
 * Installs the composition caret on one terminal.
 * @param container Element holding the opened xterm instance (`.composition-view` and the helper textarea
 *   are looked up lazily inside it).
 */
export function installImeCaret(container: HTMLElement): ImeCaret {
  let composing = false;
  // Textarea value length when the composition began; the pre-edit text follows it.
  let start = 0;
  let pending: ReturnType<typeof setTimeout> | undefined;
  let lastText = "";
  let lastOffset = -1;
  const caret = document.createElement("span");
  caret.className = IME_CARET_CLASS;

  const textarea = () => container.querySelector<HTMLTextAreaElement>(".xterm-helper-textarea");
  const overlay = () => container.querySelector<HTMLElement>(".composition-view");

  const render = () => {
    pending = undefined;
    if (!composing) return;
    const ta = textarea();
    const view = overlay();
    if (!ta || !view) return;
    const text = view.textContent ?? "";
    // A selection outside the composition (or unavailable) places the caret at the end.
    let offset = ta.selectionStart - start;
    if (!Number.isFinite(offset) || offset < 0 || offset > text.length) offset = text.length;
    if (text === lastText && offset === lastOffset && caret.parentNode === view) return;
    lastText = text;
    lastOffset = offset;
    view.replaceChildren(
      document.createTextNode(text.slice(0, offset)),
      caret,
      document.createTextNode(text.slice(offset)),
    );
  };

  // The textarea's value and selection update after the composition event has been dispatched, so read
  // them on the next task, after xterm's own compositionupdate listener has refreshed the overlay text.
  const schedule = () => {
    if (pending !== undefined) return;
    pending = setTimeout(render, 0);
  };

  const onStart = () => {
    composing = true;
    start = textarea()?.value.length ?? 0;
    lastText = "";
    lastOffset = -1;
    schedule();
  };
  const onUpdate = () => schedule();
  const onEnd = () => {
    composing = false;
    if (pending !== undefined) {
      clearTimeout(pending);
      pending = undefined;
    }
    caret.remove();
  };
  // Caret movement inside the composition (←/→) may arrive without a compositionupdate; the selection
  // change is the reliable signal. Only this terminal's focused textarea is of interest.
  const onSelectionChange = () => {
    if (!composing) return;
    if (document.activeElement !== textarea()) return;
    schedule();
  };

  // compositionstart is captured so `start` is read before anything reacts to the event; compositionupdate
  // is observed in the bubble phase so it runs after xterm's target listener replaced the overlay text.
  container.addEventListener("compositionstart", onStart, true);
  container.addEventListener("compositionupdate", onUpdate);
  container.addEventListener("compositionend", onEnd, true);
  document.addEventListener("selectionchange", onSelectionChange);

  return {
    dispose: () => {
      onEnd();
      container.removeEventListener("compositionstart", onStart, true);
      container.removeEventListener("compositionupdate", onUpdate);
      container.removeEventListener("compositionend", onEnd, true);
      document.removeEventListener("selectionchange", onSelectionChange);
    },
  };
}
