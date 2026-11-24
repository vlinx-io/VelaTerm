//! Cross-shell persistence for the one-time conversation view switch hint.
//!
//! Storing dismissal only in localStorage breaks in Electron, where each launch binds the sidecar to a new
//! dynamic loopback port and therefore starts with an empty origin store, and does not share dismissal across
//! Tauri, Electron, and browser sessions. Mirroring the key through `settingsSync` keeps dismissal in SQLite
//! `app_settings` while localStorage remains the synchronous local cache.

import { flushNow, pushSetting } from "./settingsSync";

export const CONVERSATION_VIEW_HINT_KEY = "vlx-conversation-view-hint-0.2.0-dismissed";

const listeners = new Set<() => void>();
let dismissedForRun = false;

export function isConversationViewHintDismissed(): boolean {
  try {
    return dismissedForRun || localStorage.getItem(CONVERSATION_VIEW_HINT_KEY) === "1";
  } catch {
    return dismissedForRun;
  }
}

export function subscribeConversationViewHint(listener: () => void): () => void {
  listeners.add(listener);
  const onStorage = (event: StorageEvent) => {
    if (event.key === CONVERSATION_VIEW_HINT_KEY || event.key === null) listener();
  };
  window.addEventListener("storage", onStorage);
  return () => {
    listeners.delete(listener);
    window.removeEventListener("storage", onStorage);
  };
}

/** Notify subscribers after backend reconciliation updates the cached dismissal state. */
export function notifyConversationViewHintChanged(): void {
  listeners.forEach((listener) => listener());
}

/** Dismiss the hint locally and flush the flag to backend `app_settings` so restarts and other shells stay quiet. */
export function dismissConversationViewHint(): void {
  dismissedForRun = true;
  try {
    localStorage.setItem(CONVERSATION_VIEW_HINT_KEY, "1");
  } catch {
    // Keep the notice closed for this run when browser storage is unavailable.
  }
  pushSetting(CONVERSATION_VIEW_HINT_KEY, "1");
  void flushNow(2000);
  notifyConversationViewHintChanged();
}
