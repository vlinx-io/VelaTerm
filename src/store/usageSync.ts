//! Keeps this client's copy of the account-usage snapshot aligned with the backend's.
//!
//! Quotas belong to an account, not a session, so the backend polls each provider once for the whole
//! machine and broadcasts the result. A client reads that copy on startup and then follows the broadcast;
//! it never queries Claude, Codex, or Grok itself, which is what stops ten open sessions from becoming ten
//! times the requests against a rate-limited endpoint.

import { onUsageChanged } from "../ipc/events";
import { usageSnapshot } from "../ipc/commands";
import { useTermStore } from "./termStore";

/**
 * Read the stored snapshot once, then follow `usage://changed`. Returns a stop function for unmount.
 *
 * The initial read is a plain memory read on the backend, so it stays cheap even when several windows
 * start at once. A failure is ignored: the panel simply shows nothing until the next broadcast arrives.
 */
export function startUsageSync(): () => void {
  let stopped = false;
  void usageSnapshot()
    .then((snap) => {
      if (!stopped) useTermStore.getState().setUsage(snap);
    })
    .catch(() => {});
  const unlisten = onUsageChanged((snap) => {
    if (!stopped) useTermStore.getState().setUsage(snap);
  });
  return () => {
    stopped = true;
    void unlisten.then((fn) => fn());
  };
}
