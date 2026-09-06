//! Diagnostic trail for split panes: who created one, and when.
//!
//! Split panes are only ever produced by `splitNew` (keyboard shortcut, menu bar, pane-header button) or
//! carried in wholesale by a peer's mirror layout. Both are silent: a new pane shows a bare prompt, and the
//! pane tree is persisted, so a split created without notice looks like it appeared on its own days later.
//! This records each event so the source can be identified after the fact.
//!
//! Entries are retained in memory and persisted by the backend to logs/split.log. Native remote
//! windows use their local host so the file retains the originating window label. Plain browsers
//! use their connected server. Read the recent entries via `window.__vlxSplitLog` (newest last).

import { invokeNative } from "../ipc/transport";

/** Where a split came from. */
export type SplitSource =
  | "shortcut"
  | "menu"
  | "pane-button"
  | "mirror"
  | "unknown";

/** One recorded split event. */
export interface SplitTraceEntry {
  at: string;
  source: SplitSource;
  detail: string;
  persistence: "pending" | "saved" | "disabled" | "failed";
}

/** Structural identifiers only; never send terminal contents, session names, or connection URLs. */
export interface SplitTraceContext {
  sessionIds: string[];
  parentSessionId?: string;
  tabId?: string;
  direction?: "horizontal" | "vertical";
}

/** Cap on retained entries; old ones drop off the front. */
const LIMIT = 200;

const entries: SplitTraceEntry[] = [];
let pendingWrite: Promise<void> = Promise.resolve();

/** Record a split event and echo it to the console. */
export function traceSplit(source: SplitSource, detail: string, context: SplitTraceContext): void {
  const now = new Date();
  const entry: SplitTraceEntry = {
    at: now.toISOString(),
    source,
    detail,
    persistence: "pending",
  };
  entries.push(entry);
  if (entries.length > LIMIT) entries.shift();
  console.info("[split]", entry.at, source, detail);
  try {
    (window as unknown as { __vlxSplitLog?: SplitTraceEntry[] }).__vlxSplitLog =
      entries;
  } catch {
    /* Ignore environments without a window, such as unit tests. */
  }
  const payload = { source, clientAtMs: now.getTime(), ...context };
  // Preserve event order within a window. A failed write must not prevent later events or splitting.
  pendingWrite = pendingWrite.then(async () => {
    try {
      const written = await invokeNative<boolean>("record_split_trace", { entry: payload });
      entry.persistence = written ? "saved" : "disabled";
    } catch {
      entry.persistence = "failed";
      console.warn("[split] Failed to save the diagnostic event; the in-memory entry is still available.");
    }
  });
}

/** Recorded events, oldest first. */
export function splitTrace(): SplitTraceEntry[] {
  return entries;
}
