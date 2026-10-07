//! Keeps xterm's hidden helper textarea caret at the end of its value while no composition is active.
//!
//! xterm's CompositionHelper records `start = textarea.value.length` at compositionstart and later slices the
//! committed text with `value.substring(start)`. That only holds when the IME inserts at the end of the value.
//! The textarea keeps the characters already sent to the PTY, and xterm does not cancel the arrow keys, so ←/→
//! also move the textarea's own caret. A composition started after that is inserted mid-value, and the slice
//! returns the old trailing text instead of the composed text: with `abcdef` and the caret after `abc`,
//! committing `你好` emitted `ef`.
//!
//! The textarea is invisible and its caret means nothing to the terminal, so the caret is moved back to the end
//! whenever it changes outside a composition. During a composition it is left alone: ←/→ inside the pre-edit
//! text are real edits (see imeCaret.ts).

export interface ImeAnchor {
  dispose: () => void;
}

/**
 * @param container Element holding the opened xterm instance; composition events are captured here.
 * @param textarea xterm's helper textarea (`term.textarea`, available once the terminal is opened).
 */
export function installImeAnchor(container: HTMLElement, textarea: HTMLTextAreaElement): ImeAnchor {
  let composing = false;

  const toEnd = () => {
    if (composing) return;
    const end = textarea.value.length;
    if (textarea.selectionStart !== end || textarea.selectionEnd !== end) textarea.setSelectionRange(end, end);
  };

  // Capture phase, so the caret is repaired before xterm's own compositionstart records its offset and before
  // the IME inserts the first pre-edit character.
  const onStart = () => {
    toEnd();
    composing = true;
  };
  const onEnd = () => {
    composing = false;
  };
  const onSelectionChange = () => {
    if (document.activeElement === textarea) toEnd();
  };

  container.addEventListener("compositionstart", onStart, true);
  container.addEventListener("compositionend", onEnd, true);
  document.addEventListener("selectionchange", onSelectionChange);

  return {
    dispose: () => {
      container.removeEventListener("compositionstart", onStart, true);
      container.removeEventListener("compositionend", onEnd, true);
      document.removeEventListener("selectionchange", onSelectionChange);
    },
  };
}
