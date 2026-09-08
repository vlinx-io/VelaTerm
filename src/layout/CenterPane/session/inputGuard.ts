/**
 * WKWebView can insert U+001C–U+001F when an arrow key reaches an input boundary.
 * Cancel the native edit before it changes the DOM; React's synthetic beforeinput
 * handling and setting an unchanged controlled value do not reliably prevent it.
 * Delegate within the chat pane so dynamically mounted queue and question fields
 * receive the same protection as the composer.
 */
export function attachChatInputGuard(root: HTMLDivElement | null) {
  if (!root) return;
  const onBeforeInput = (event: Event) => {
    const input = event as InputEvent;
    if (!(input.target instanceof HTMLInputElement || input.target instanceof HTMLTextAreaElement)) return;
    // Match only direct insertion of arrow control characters. Pasted content,
    // normal composition, line breaks, tabs, and navigation remain native.
    if (input.inputType === "insertText" && input.data && /^[\u001C-\u001F]+$/.test(input.data)) {
      input.preventDefault();
    }
  };
  root.addEventListener("beforeinput", onBeforeInput, true);
  return () => root.removeEventListener("beforeinput", onBeforeInput, true);
}
