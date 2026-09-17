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

/**
 * What the line that opens an agent turn says, gathered from the replies in that turn.
 *
 * The line is drawn once, above the turn's first entry, so reasoning and tool calls read as the agent's
 * rather than as rows left over from the prompt. It is filled from whichever reply first reports each
 * fact: the turn's replies are the only rows that carry a model or a timestamp.
 */
export interface TurnHead {
  /** Model reported for the turn's first reply, when the agent reports one per reply. */
  who?: string;
  /** When that reply was written. */
  at?: number;
  /** Wall-clock time for the whole turn; only the last reply of a turn reports it, and only when done. */
  durationMs?: number;
  /** Present when the turn has work besides its answer that the reader can hide. */
  fold?: TurnFold;
}

/** A turn's interim work — reasoning, tool calls, replies before the last — and whether it is hidden. */
export interface TurnFold {
  /** The turn, named by the id of its first entry. */
  id: string;
  /** How many steps the fold covers; a run of tool calls counts each call. */
  steps: number;
  collapsed: boolean;
}

/**
 * One drawable entry: a conversation row, a folded run of tool calls standing in for several, or the
 * author line of a turn whose every entry is hidden.
 */
export type DisplayRow =
  | { kind: "row"; id: string; row: ChatRow; head?: TurnHead }
  | { kind: "run"; id: string; calls: ToolRow[]; running: boolean; head?: TurnHead }
  | { kind: "fold"; id: string; head: TurnHead };

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

/** Whether an entry is work the agent did in a turn, rather than a remark about the conversation itself. */
function isAgentWork(entryRow: DisplayRow): boolean {
  if (entryRow.kind === "run") return true;
  if (entryRow.kind === "fold") return false;
  return entryRow.row.kind === "assistant" || entryRow.row.kind === "reasoning" || entryRow.row.kind === "tool";
}

/**
 * Put the agent's author line on the first entry of each turn.
 *
 * A turn is everything between one prompt and the next. The agent's name, model and duration are carried
 * by its replies, which come after its reasoning and its tool calls; without this pass the work that
 * opens a turn has no author of its own and reads as though the prompt above it had produced it. Marking
 * the first entry lets the view draw one author line above the whole turn instead of one per message.
 */
export function markAgentTurns(entries: readonly DisplayRow[]): DisplayRow[] {
  const out = [...entries];
  let start = -1;
  let head: TurnHead | null = null;
  const close = () => {
    if (start < 0 || !head) return;
    out[start] = { ...out[start], head };
    start = -1;
    head = null;
  };
  for (let index = 0; index < out.length; index += 1) {
    const entryRow = out[index];
    if (entryRow.kind === "row" && entryRow.row.kind === "user") {
      close();
      continue;
    }
    if (!isAgentWork(entryRow)) continue;
    if (start < 0) {
      start = index;
      head = {};
    }
    if (entryRow.kind !== "row" || entryRow.row.kind !== "assistant") continue;
    if (head!.who === undefined && entryRow.row.model) head!.who = entryRow.row.model;
    if (head!.at === undefined && entryRow.row.at !== undefined) head!.at = entryRow.row.at;
    if (entryRow.row.durationMs !== undefined) head!.durationMs = entryRow.row.durationMs;
  }
  close();
  return out;
}

/** The tool call that asks the person something; its answer is part of the conversation, not interim work. */
function isQuestion(entryRow: DisplayRow): boolean {
  if (entryRow.kind !== "row") return false;
  return entryRow.row.kind === "tool" && entryRow.row.name === "AskUserQuestion";
}

/** The result of folding: what the list draws, and which hidden entry belongs to which turn. */
export interface FoldedTurns {
  rows: DisplayRow[];
  /** Every turn that has something to hide, in order. */
  turns: TurnFold[];
  /** Turn id for each entry the fold currently hides, so search can open the turn holding a match. */
  hiddenIn: Map<string, string>;
}

/**
 * Hide the interim work of collapsed turns, leaving each turn's answer.
 *
 * Runs after `markAgentTurns`, whose author lines mark where each turn begins. A turn's answer is its last
 * reply with any text; everything else the agent did in the turn is interim. Errors, notices, commands,
 * compaction and answered questions stay visible either way: they are not the agent working. When the
 * first entry is hidden, the author line moves to the first entry left standing, or stands alone when
 * the turn has nothing left to show.
 */
export function foldAgentTurns(
  entries: readonly DisplayRow[],
  isCollapsed: (turnId: string) => boolean,
): FoldedTurns {
  const rows: DisplayRow[] = [];
  const turns: TurnFold[] = [];
  const hiddenIn = new Map<string, string>();
  let index = 0;
  while (index < entries.length) {
    const first = entries[index];
    if (!first.head) {
      rows.push(first);
      index += 1;
      continue;
    }
    let end = index + 1;
    while (end < entries.length && !entries[end].head && !isTurnStart(entries[end])) end += 1;
    let answer = -1;
    for (let at = end - 1; at >= index && answer < 0; at -= 1) {
      const candidate = entries[at];
      if (candidate.kind === "row" && candidate.row.kind === "assistant" && candidate.row.text.trim() !== "") answer = at;
    }
    const interim = (at: number) => at !== answer && isAgentWork(entries[at]) && !isQuestion(entries[at]);
    let steps = 0;
    for (let at = index; at < end; at += 1) {
      const candidate = entries[at];
      if (interim(at)) steps += candidate.kind === "run" ? candidate.calls.length : 1;
    }
    if (steps === 0) {
      rows.push(...entries.slice(index, end));
      index = end;
      continue;
    }
    const fold: TurnFold = { id: first.id, steps, collapsed: isCollapsed(first.id) };
    turns.push(fold);
    const head: TurnHead = { ...first.head, fold };
    if (!fold.collapsed) {
      rows.push({ ...first, head }, ...entries.slice(index + 1, end));
      index = end;
      continue;
    }
    let headPlaced = false;
    for (let at = index; at < end; at += 1) {
      const candidate = entries[at];
      if (interim(at)) {
        hiddenIn.set(candidate.id, fold.id);
        continue;
      }
      rows.push(headPlaced ? candidate : { ...candidate, head });
      headPlaced = true;
    }
    if (!headPlaced) rows.push({ kind: "fold", id: `${fold.id}:fold`, head });
    index = end;
  }
  return { rows, turns, hiddenIn };
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
  // The turn's author line rides on the first entry, so that entry is taller by the line's own height.
  const head = row.head ? 34 : 0;
  if (row.kind === "fold") return head;
  if (row.kind === "run") return 34 + head;
  switch (row.row.kind) {
    case "tool":
      return 34 + head;
    case "reasoning":
      return 28 + head;
    case "error":
    case "notice":
    case "compaction":
      return 36 + head;
    case "user":
      return textHeight(row.row.text, 46);
    case "assistant":
    case "command":
      return textHeight(row.row.text, 40) + head;
    case "shell":
      return textHeight(row.row.stdout + row.row.stderr, 70) + head;
  }
}

/** Guess how tall a block of prose renders, from its length alone: about 80 characters to a 22px line. */
function textHeight(text: string, chrome: number): number {
  const lines = Math.max(1, Math.ceil(text.length / 80));
  return Math.min(900, chrome + lines * 22);
}
