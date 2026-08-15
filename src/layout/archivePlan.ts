//! Worker worktrees a successful archive deletes, listed before the tree hides the sessions.

import type { Group, Session } from "../types";

/** Paths of the worker worktrees that archiving `rootIds` deletes, without duplicates. */
export function archivedWorktreePaths(
  sessions: Session[],
  groups: Group[],
  rootIds: string[],
): string[] {
  const children = new Map<string, Session[]>();
  for (const session of sessions) {
    const parent = session.parentSessionId;
    if (!parent) continue;
    children.set(parent, [...(children.get(parent) ?? []), session]);
  }
  const shared = new Set(
    groups.map((group) => group.worktreePath?.trim()).filter(Boolean),
  );
  const byId = new Map(sessions.map((session) => [session.id, session]));

  const paths: string[] = [];
  const seen = new Set<string>();
  const visit = (session: Session, isRoot: boolean) => {
    const path = session.worktreePath?.trim();
    // A top-level session owns its directory itself; only a worker's worktree is retired.
    if (path && !isRoot && session.parentSessionId && !shared.has(path) && !seen.has(path)) {
      seen.add(path);
      paths.push(path);
    }
    for (const child of children.get(session.id) ?? []) visit(child, false);
  };
  for (const id of rootIds) {
    const root = byId.get(id);
    if (root) visit(root, !root.parentSessionId);
  }
  return paths;
}

/** Paths archiving a whole group deletes; its own bound worktree survives as a shared binding. */
export function archivedGroupWorktreePaths(
  sessions: Session[],
  groups: Group[],
  groupId: string,
): string[] {
  const groupIds = new Set<string>([groupId]);
  let added = true;
  while (added) {
    added = false;
    for (const group of groups) {
      if (group.parentGroupId && groupIds.has(group.parentGroupId) && !groupIds.has(group.id)) {
        groupIds.add(group.id);
        added = true;
      }
    }
  }
  const roots = sessions
    .filter((s) => !s.parentSessionId && s.groupId && groupIds.has(s.groupId))
    .map((s) => s.id);
  return archivedWorktreePaths(sessions, groups, roots);
}
