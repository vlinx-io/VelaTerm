//! One observer for the agent half of the sidebar's activity record instead of a hook in every writer. The
//! store's runtime map is written from four places (setRuntime, applyStatusSignal, applyScreenDetection,
//! applySessionStates); subscribing to the store once covers them all without touching any of them. The user
//! half is recorded by the two input paths (terminal input, chat submission) directly. Focus is deliberately
//! not a signal: opening a session, switching to its tab or clicking into it is reading, and a session the
//! user only looks at must keep its place in the order.

import type { AgentState, SessionRuntime } from "../types";
import { useTermStore } from "./termStore";

const ACTIVE_STATES: ReadonlySet<AgentState> = new Set<AgentState>(["working", "asking", "waiting"]);

/**
 * Whether the change from `prev` to `next` is agent activity worth recording: the state must become
 * working, asking or waiting and differ from before. A change out of "no state" only counts for working:
 * after startup the backend replays its records, and a session that arrives as asking or waiting from
 * nothing is the old state being restored, not something that just happened. A replayed working record is
 * accepted as activity, which is the honest reading of "the agent is working right now".
 */
export function isActivityTransition(
  prev: SessionRuntime | undefined,
  next: SessionRuntime | undefined,
): boolean {
  const a = next?.agentState ?? null;
  const b = prev?.agentState ?? null;
  if (a === null || !ACTIVE_STATES.has(a) || a === b) return false;
  return b !== null || a === "working";
}

/**
 * Subscribes to the store and records activity when a session's agent state makes an activity transition.
 * Returns the unsubscribe function. The recording itself (burst coalescing, ignoring drafts, the persisted
 * write) lives in `noteSessionActivity`.
 */
export function startActivityWatch(): () => void {
  return useTermStore.subscribe((next, prev) => {
    if (next.runtimes === prev.runtimes) return;
    for (const id of Object.keys(next.runtimes)) {
      const after = next.runtimes[id];
      const before = prev.runtimes[id];
      if (after !== before && isActivityTransition(before, after)) next.noteSessionActivity(id);
    }
  });
}
