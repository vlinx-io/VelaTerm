//! Global guard against WKWebView inserting legacy macOS arrow control characters (U+001C–U+001F,
//! plus AppKit's U+F700–U+F703 function-key codes) via native `beforeinput` when an arrow key is
//! pressed past an editable boundary.

const ARROW_CONTROL_TEXT_RE = /^[\u001C-\u001F\uF700-\uF703]+$/;

function resolveElement(event: Event): HTMLElement | null {
  const raw = typeof event.composedPath === "function" ? event.composedPath()[0] ?? event.target : event.target;
  if (raw instanceof HTMLElement) return raw;
  if (raw instanceof Node) return raw.parentElement;
  return null;
}

function isContentEditableElement(el: HTMLElement): boolean {
  if (el.isContentEditable) return true;
  // jsdom does not implement HTMLElement.prototype.isContentEditable; fall back to the DOM attribute.
  const host = el.closest<HTMLElement>("[contenteditable]");
  if (!host) return false;
  const value = host.getAttribute("contenteditable")?.toLowerCase();
  return value === "" || value === "true" || value === "plaintext-only";
}

function isGuardedEditable(el: HTMLElement | null): boolean {
  if (!el) return false;
  // Leave xterm's hidden helper textarea untouched; xterm owns its own keyboard and IME pipeline.
  if (el.classList.contains("xterm-helper-textarea")) return false;
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) return true;
  return isContentEditableElement(el);
}

/**
 * Cancels native `beforeinput` events that attempt to insert arrow-key control characters into
 * editable fields. React's synthetic `onBeforeInput` and controlled-value stripping do not reliably
 * prevent WebKit from mutating the DOM at input boundaries, so this attaches a capture-phase native
 * listener on `root` (defaulting to `document`).
 */
export function installWebkitArrowInputGuard(root: EventTarget | null = document): () => void {
  if (!root) return () => {};
  const onBeforeInput = (event: Event) => {
    const input = event as InputEvent;
    // Match only direct insertion of arrow control characters; pastes, IME composition, line
    // breaks, tabs, and normal editing remain native.
    if (input.inputType !== "insertText" || !input.data || !ARROW_CONTROL_TEXT_RE.test(input.data)) return;
    if (!isGuardedEditable(resolveElement(event))) return;
    input.preventDefault();
    input.stopPropagation();
  };
  root.addEventListener("beforeinput", onBeforeInput, true);
  return () => root.removeEventListener("beforeinput", onBeforeInput, true);
}
