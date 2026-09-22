//! Pure ordering for the sidebar's optional "Sort by activity" mode. No React, no store: the component hands
//! in the tree arrays and the live activity map and gets back newly ordered arrays. With the mode off the
//! component does not call this at all, so the manual order renders exactly as the backend delivered it.
//!
//! Ranks are derived, never stored: a session ranks by the newest stamp in its own subtree (itself and its
//! child sessions), a group by the newest stamp of everything inside it including nested groups, a project by
//! the newest stamp of any of its sessions. Items without any activity keep their manual relative order and
//! come after items with activity; ties fall back to the manual order as well.

import type { Group, Project, Session, SessionId } from "../../types";

/** Last activity per session id in milliseconds; absent means never active. */
export type ActivityMap = Record<SessionId, number>;

/** The subset of a node the comparator needs, satisfied by Project, Group and Session alike. */
interface Ranked {
  sortOrder: number;
}

/**
 * Orders two nodes given their activity ranks: newer activity first, an active item before an inactive one,
 * and the manual `sortOrder` as the deterministic tie-breaker so the result does not depend on input order.
 */
export function compareByActivity(a: Ranked, b: Ranked, rankA: number | undefined, rankB: number | undefined): number {
  if (rankA !== undefined && rankB !== undefined && rankA !== rankB) return rankB - rankA;
  if (rankA !== undefined && rankB === undefined) return -1;
  if (rankA === undefined && rankB !== undefined) return 1;
  return a.sortOrder - b.sortOrder;
}

/** Newest stamp of `own` and `other`, or undefined when neither has one. */
function newest(own: number | undefined, other: number | undefined): number | undefined {
  if (own === undefined) return other;
  if (other === undefined) return own;
  return Math.max(own, other);
}

/** Rank of every session including its descendants, computed once per call in a single bottom-up pass. */
function sessionRanks(sessions: Session[], activity: ActivityMap): Map<SessionId, number> {
  const ranks = new Map<SessionId, number>();
  const byId = new Map(sessions.map((s) => [s.id, s]));
  // Seed each session with its own stamp, then push every stamp up the parent chain so a parent ranks like a
  // group over its children. Cycles cannot occur in a valid tree, but the visited guard keeps a corrupt one
  // from looping forever.
  for (const s of sessions) {
    const own = activity[s.id];
    if (own === undefined) continue;
    let cur: Session | undefined = s;
    const visited = new Set<SessionId>();
    while (cur && !visited.has(cur.id)) {
      visited.add(cur.id);
      ranks.set(cur.id, newest(ranks.get(cur.id), own) as number);
      cur = cur.parentSessionId ? byId.get(cur.parentSessionId) : undefined;
    }
  }
  return ranks;
}

/** Rank of every group: newest stamp among the sessions inside it (child sessions count for the group their
 *  top-level ancestor sits in) and inside its nested groups. */
function groupRanks(groups: Group[], sessions: Session[], activity: ActivityMap): Map<string, number> {
  const ranks = new Map<string, number>();
  const groupById = new Map(groups.map((g) => [g.id, g]));
  const sessionById = new Map(sessions.map((s) => [s.id, s]));
  for (const s of sessions) {
    const own = activity[s.id];
    if (own === undefined) continue;
    let cur: Group | undefined = groupById.get(effectiveGroupId(s, sessionById) ?? "");
    const visited = new Set<string>();
    while (cur && !visited.has(cur.id)) {
      visited.add(cur.id);
      ranks.set(cur.id, newest(ranks.get(cur.id), own) as number);
      cur = cur.parentGroupId ? groupById.get(cur.parentGroupId) : undefined;
    }
  }
  return ranks;
}

/** Rank of every project: newest stamp among all of its sessions. */
function projectRanks(sessions: Session[], activity: ActivityMap): Map<string, number> {
  const ranks = new Map<string, number>();
  for (const s of sessions) {
    const own = activity[s.id];
    if (own === undefined) continue;
    ranks.set(s.projectId, newest(ranks.get(s.projectId), own) as number);
  }
  return ranks;
}

/** Child sessions live under a parent, not a group; their group membership is the parent's. */
function effectiveGroupId(s: Session, byId: Map<SessionId, Session>): string | null | undefined {
  let cur: Session | undefined = s;
  const visited = new Set<SessionId>();
  while (cur?.parentSessionId && !visited.has(cur.id)) {
    visited.add(cur.id);
    cur = byId.get(cur.parentSessionId);
  }
  return cur?.groupId;
}

/**
 * Returns the three tree arrays ordered for the activity mode. Each array is sorted globally with its own
 * rank comparator; because the sidebar flattens siblings with order-preserving `filter` calls, this orders
 * every sibling set at every level (project list, groups among their siblings, sessions at the project root,
 * inside each group, and under each parent). The inputs are not mutated and `sortOrder` values stay untouched.
 */
export function orderTreeByActivity(
  projects: Project[],
  groups: Group[],
  sessions: Session[],
  activity: ActivityMap,
): { projects: Project[]; groups: Group[]; sessions: Session[] } {
  const sRanks = sessionRanks(sessions, activity);
  const gRanks = groupRanks(groups, sessions, activity);
  const pRanks = projectRanks(sessions, activity);
  return {
    projects: [...projects].sort((a, b) => compareByActivity(a, b, pRanks.get(a.id), pRanks.get(b.id))),
    groups: [...groups].sort((a, b) => compareByActivity(a, b, gRanks.get(a.id), gRanks.get(b.id))),
    sessions: [...sessions].sort((a, b) => compareByActivity(a, b, sRanks.get(a.id), sRanks.get(b.id))),
  };
}

/** Drop zones the sidebar computes from the pointer position over a row. */
export type DropZone = "top" | "center" | "bottom";

/**
 * Whether a drop in `zone` may proceed while the activity order is `sortByActivity`. Edge zones reorder
 * siblings by rewriting `sortOrder`, which the activity order would hide, so they are refused while the mode
 * is on; the center zone moves a node into a group, project or under a session and always stays allowed.
 */
export function isDropZoneAllowed(zone: DropZone, sortByActivity: boolean): boolean {
  return zone === "center" || !sortByActivity;
}
