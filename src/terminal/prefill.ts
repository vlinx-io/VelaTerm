//! Text handed from the conversation view to the agent's terminal interface, typed but never submitted.
//!
//! A command the conversation view cannot run is passed to the terminal view instead. The text lands in
//! the agent's prompt and waits there: the person presses Enter, so nothing runs that they did not see.
//! That promise only holds if the text cannot carry its own Enter, which is why every control character
//! is taken out before anything reaches the PTY.

import { useTermStore } from "../store/termStore";
import type { SessionId } from "../types";

/**
 * Make text safe to type into a prompt: every C0 control character (carriage return, line feed, tab,
 * escape among them), DEL and every C1 control character becomes a space, and the ends are trimmed.
 */
export function sanitizePrefill(text: string): string {
  return text.replace(/[\u0000-\u001f\u007f-\u009f]/g, " ").trim();
}

/** Read and clear the text waiting for this session's terminal, so it is typed at most once. */
export function takePrefill(sessionId: SessionId): string | undefined {
  const store = useTermStore.getState();
  const text = store.runtimes[sessionId]?.agentPrefill;
  if (text === undefined) return undefined;
  store.setRuntime(sessionId, { agentPrefill: undefined });
  return text ? sanitizePrefill(text) : undefined;
}
