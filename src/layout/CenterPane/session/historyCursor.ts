//! Shared handling of history cursors that no longer resolve.
//!
//! The server rebuilds its per-conversation index while an agent keeps appending, and it reclaims an
//! idle reader; a reader id, row anchor, or branch revision the client kept from that index then fails
//! every retry that sends it again. Views detect that case here and start over from the newest rows.

/** A history cursor the server can no longer resolve; repeating it would fail the same way. */
export const STALE_CURSOR = /^(source_changed|cursor_expired):/;

/** The backend's error text, without the wrapper a thrown `Error` adds. */
export function failureText(error: unknown): string {
  return String(error).replace(/^Error: /, "");
}
