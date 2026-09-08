//! Folding a run of tool calls into one row, and deciding how much of a long conversation stays mounted.
//!
//! An agent doing real work fires tool calls in bursts: eight reads, three greps, a couple of edits. Drawn
//! one card per call, a single turn pushes the answer that follows it off the screen. Consecutive calls are
//! therefore folded into one row that says how many there were and what they did, and only opens when
//! someone wants the detail.
//!
//! The same grouping decides what the view has to draw. Everything here is a pure function over the rows so
//! the rules can be tested without a DOM.

import type { ChatRow } from "../../../ipc/chat";

/** A tool call row, named separately because a fold is a list of exactly these. */
export type ToolRow = Extract<ChatRow, { kind: "tool" }>;

/** One drawable entry: a conversation row, or a folded run of tool calls standing in for several. */
export type DisplayRow =
  | { kind: "row"; id: string; row: ChatRow }
  | { kind: "run"; id: string; calls: ToolRow[]; running: boolean };

/**
 * Fewest consecutive calls worth folding.
 *
 * Two cards read perfectly well on their own, and folding them would trade a line of detail for a line
 * saying there is detail. Three is where the burst starts to crowd out the conversation around it.
 */
export const MIN_RUN = 3;

/** How many recent entries stay really mounted, outside the virtualized part of the list. */
export const MOUNTED_RECENT = 30;

function isRunning(call: ToolRow): boolean {
  return call.status === "running";
}

/** Wrap a plain row so both kinds of entry carry an id the list can key on. */
function entry(row: ChatRow): DisplayRow {
  return { kind: "row", id: row.id, row };
}

/**
 * Fold consecutive tool calls into runs.
 *
 * Calls still running at the end of a burst are left standing on their own: while the agent is working,
 * what it is doing right now is the one thing worth reading, and folding it away the moment the previous
 * call finishes would make the view flicker between "7 tool calls" and a card nobody asked to see.
 */
export function groupToolRuns(rows: readonly ChatRow[], minRun = MIN_RUN): DisplayRow[] {
  const out: DisplayRow[] = [];
  let pending: ToolRow[] = [];

  const flush = () => {
    if (pending.length === 0) return;
    // Split off the trailing calls that have not finished. Several can be running at once, because the
    // agent is allowed to ask for a batch of tools in one go.
    let cut = pending.length;
    while (cut > 0 && isRunning(pending[cut - 1])) cut -= 1;
    const folded = pending.slice(0, cut);
    const live = pending.slice(cut);
    if (folded.length >= minRun) {
      out.push({
        kind: "run",
        id: folded[0].id,
        calls: folded,
        running: folded.some(isRunning),
      });
    } else {
      for (const call of folded) out.push(entry(call));
    }
    for (const call of live) out.push(entry(call));
    pending = [];
  };

  for (const row of rows) {
    if (row.kind === "tool") {
      pending.push(row);
      continue;
    }
    flush();
    out.push(entry(row));
  }
  flush();
  return out;
}

/** How many times each tool was called, in the order the names first appear: `Read ×4 · Grep ×2`. */
export function runSummary(calls: readonly ToolRow[]): { name: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const call of calls) {
    const name = call.name || "";
    counts.set(name, (counts.get(name) ?? 0) + 1);
  }
  return [...counts].map(([name, count]) => ({ name, count }));
}

/** The summary as one line. Tool names are proper nouns and `×N` reads the same everywhere, so it needs no translation. */
export function runSummaryText(calls: readonly ToolRow[]): string {
  return runSummary(calls)
    .map(({ name, count }) => (count > 1 ? `${name} ×${count}` : name))
    .join(" · ");
}

/**
 * Where the really-mounted tail of the list begins.
 *
 * Recent entries are left mounted rather than virtualized because they are the ones still changing: text
 * arrives token by token and a tool card grows when its output lands, and a row whose height moves while a
 * virtualizer is measuring it is how a view ends up jumping under the reader.
 *
 * The boundary then walks back to the start of a turn, so the mounted part never begins halfway through an
 * answer. The walk is bounded: one turn can hold hundreds of rows, and mounting all of them would undo the
 * point of virtualizing at all.
 */
export function mountedStart(rows: readonly DisplayRow[], minMounted = MOUNTED_RECENT): number {
  if (rows.length <= minMounted) return 0;
  let index = rows.length - minMounted;
  const floor = Math.max(rows.length - minMounted * 2, 0);
  while (index > floor && !isTurnStart(rows[index])) index -= 1;
  return index;
}

function isTurnStart(entryRow: DisplayRow | undefined): boolean {
  return entryRow?.kind === "row" && entryRow.row.kind === "user";
}

/** Rough height in pixels, used for the parts of the list that have not been drawn yet. */
export function estimateRowHeight(row: DisplayRow | undefined): number {
  if (!row) return 120;
  if (row.kind === "run") return 34;
  switch (row.row.kind) {
    case "tool":
      return 34;
    case "reasoning":
      return 28;
    case "error":
    case "notice":
    case "compaction":
      return 36;
    case "user":
      return textHeight(row.row.text, 46);
    case "assistant":
    case "command":
      return textHeight(row.row.text, 40);
  }
}

/** Guess how tall a block of prose renders, from its length alone: about 80 characters to a 22px line. */
function textHeight(text: string, chrome: number): number {
  const lines = Math.max(1, Math.ceil(text.length / 80));
  return Math.min(900, chrome + lines * 22);
}
