//! First-alignment gate shared by mirror sync and the initial layout restore.
//!
//! A remote window has two sources for its arrangement: the layout it saved to local storage, and the one a
//! peer publishes through mirror mode. Both used to land independently, so whichever arrived first was
//! replaced by the other. Restoring local storage first mounts every leaf as a real terminal — which is what
//! starts a process — only for the peer's layout to unmount a batch of them moments later. On a session
//! whose process is already gone that mount starts a shell nobody will ever look at, and a browser client's
//! unmount detaches rather than kills, so the shell stays behind.
//!
//! This gate lets the restore wait for the alignment to conclude and skip itself when the peer's layout won.
//! It lives in its own module so neither side has to import the other.

/** Upper bound on the wait. Alignment normally settles in one round trip; the budget covers a backend that
 * is slow or unreachable, where falling back to the locally saved layout beats an empty window. */
const TIMEOUT_MS = 2000;

let settled = false;
let resolveFirst: ((applied: boolean) => void) | null = null;
const first = new Promise<boolean>((resolve) => {
  resolveFirst = resolve;
});

/** Report how the first alignment ended: true when a peer's layout was applied. Later calls are ignored, so
 * a reconnect realignment cannot reopen a decision the restore has already acted on. */
export function settleFirstMirrorAlign(applied: boolean): void {
  if (settled) return;
  settled = true;
  resolveFirst?.(applied);
}

/** Resolve once the first alignment concludes, or false once the budget runs out. */
export function whenFirstMirrorAlign(): Promise<boolean> {
  return Promise.race([
    first,
    new Promise<boolean>((resolve) => setTimeout(() => resolve(false), TIMEOUT_MS)),
  ]);
}
