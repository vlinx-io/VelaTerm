//! Diagnostic trail for split panes: who created one, and when.
//!
//! Split panes are only ever produced by `splitNew` (keyboard shortcut, menu bar, pane-header button) or
//! carried in wholesale by a peer's mirror layout. Both are silent: a new pane shows a bare prompt, and the
//! pane tree is persisted, so a split created without notice looks like it appeared on its own days later.
//! This records each event so the source can be identified after the fact.
//!
//! Entries live in memory only and are also mirrored to the console. Read them from devtools via
//! `window.__vlxSplitLog` (a plain array, newest last).

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
}

/** Cap on retained entries; old ones drop off the front. */
const LIMIT = 200;

const entries: SplitTraceEntry[] = [];

/** Record a split event and echo it to the console. */
export function traceSplit(source: SplitSource, detail: string): void {
  const entry: SplitTraceEntry = {
    at: new Date().toISOString(),
    source,
    detail,
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
}

/** Recorded events, oldest first. */
export function splitTrace(): SplitTraceEntry[] {
  return entries;
}
