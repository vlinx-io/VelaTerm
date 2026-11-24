//! Composer registry giving callers outside a chat pane a way to focus its text box, the chat-engine
//! counterpart of `focusTerminal` in terminal/registry. ChatPane registers a getter by sessionId rather than
//! the element itself, so a composer that is not rendered yet simply has nothing to focus.

const composers = new Map<string, () => HTMLTextAreaElement | null>();

/** Register a session's composer; the returned cleanup removes only this registration, so a remount's
 * replacement survives the old instance's teardown. */
export function registerComposer(id: string, get: () => HTMLTextAreaElement | null): () => void {
  composers.set(id, get);
  return () => {
    if (composers.get(id) === get) composers.delete(id);
  };
}

export function focusComposer(id: string) {
  composers.get(id)?.()?.focus();
}
