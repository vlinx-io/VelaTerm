//! Virtual worktree level in the sidebar tree. Sibling groups and sessions that belong to the same Git worktree are
//! gathered under one display-only worktree node, placed where the first of them sat. Only the outermost level is
//! gathered: anything inside an item that belongs to a worktree is shown without further worktree nodes. The node
//! exists only in the rendered tree: groups and sessions keep their real parent, so moves, menus, and storage are
//! unaffected.

import { useCallback, useState } from "react";
import type { Session } from "../../types";

/** One sibling entry after gathering: a plain item, or a worktree node holding its items in order. */
export type WorktreeItem<T> =
  | { kind: "item"; item: T }
  | { kind: "worktree"; id: string; path: string; name: string; items: T[] };

/** Compare paths without trailing separators so `/a/wt` and `/a/wt/` name the same worktree. */
export function normalizeWorktreePath(path?: string | null): string {
  return (path ?? "").trim().replace(/[\\/]+$/, "");
}

/** Directory basename shown as the worktree node's name. */
export function worktreeName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() || path;
}

/** Short stable hash keeping node IDs within the 100-character collapse-map key limit. */
function hashPath(path: string): string {
  let h = 5381;
  for (let i = 0; i < path.length; i++) h = ((h << 5) + h + path.charCodeAt(i)) | 0;
  return (h >>> 0).toString(36);
}

/** Node ID scoped to its container (project, group, or parent session), so the same worktree shown under two
 *  containers collapses independently. */
export function worktreeNodeId(containerId: string, path: string): string {
  return `wt:${containerId}:${hashPath(normalizeWorktreePath(path))}`;
}

/** Gather siblings by the worktree each belongs to (`worktreeOf`, normalized, empty for none). `containerWorktree`
 *  is the worktree the container itself stands for (a project opened at a worktree directory); its items stay
 *  direct children because a level for the same worktree would add nothing. */
export function gatherByWorktree<T>(
  siblings: T[],
  worktreeOf: (item: T) => string,
  containerId: string,
  containerWorktree?: string | null,
): WorktreeItem<T>[] {
  const own = normalizeWorktreePath(containerWorktree);
  const out: WorktreeItem<T>[] = [];
  const nodes = new Map<string, Extract<WorktreeItem<T>, { kind: "worktree" }>>();
  for (const item of siblings) {
    const path = worktreeOf(item);
    if (!path || path === own) {
      out.push({ kind: "item", item });
      continue;
    }
    let node = nodes.get(path);
    if (!node) {
      node = { kind: "worktree", id: worktreeNodeId(containerId, path), path, name: worktreeName(path), items: [] };
      nodes.set(path, node);
      out.push(node);
    }
    node.items.push(item);
  }
  return out;
}

/** Worktree each session subtree belongs to, keyed by session ID. A session in a worktree belongs to it; a session
 *  outside any worktree whose descendants all sit in one worktree belongs to that worktree, so the whole session
 *  group is gathered under it. Sessions belonging to no single worktree are absent. `worktreeOf` gives a session's
 *  own worktree and defaults to its persisted `worktreePath`. */
export function sessionGroupWorktrees(
  sessions: Session[],
  worktreeOf: (s: Session) => string | null | undefined = (s) => s.worktreePath,
): Map<string, string> {
  const kids = new Map<string, Session[]>();
  for (const s of sessions) {
    if (!s.parentSessionId) continue;
    const list = kids.get(s.parentSessionId);
    if (list) list.push(s);
    else kids.set(s.parentSessionId, [s]);
  }
  // Worktree shared by a session and every descendant, or "" when any of them is outside it.
  const whole = new Map<string, string>();
  const subtree = (s: Session): string => {
    const known = whole.get(s.id);
    if (known !== undefined) return known;
    whole.set(s.id, ""); // Guards against a parent cycle in corrupt data.
    let wt = normalizeWorktreePath(worktreeOf(s));
    if (wt && (kids.get(s.id) ?? []).some((k) => subtree(k) !== wt)) wt = "";
    whole.set(s.id, wt);
    return wt;
  };
  const out = new Map<string, string>();
  for (const s of sessions) {
    const own = normalizeWorktreePath(worktreeOf(s));
    if (own) {
      out.set(s.id, own);
      continue;
    }
    const children = kids.get(s.id);
    if (!children) continue;
    const first = subtree(children[0]);
    if (first && children.every((k) => subtree(k) === first)) out.set(s.id, first);
  }
  return out;
}

const STORAGE_KEY = "vlx-sidebar-worktree-collapsed";

function loadCollapsed(): Record<string, boolean> {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}");
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {};
    const out: Record<string, boolean> = {};
    for (const [id, value] of Object.entries(raw)) if (value === true) out[id] = true;
    return out;
  } catch {
    return {};
  }
}

/** Collapse state of worktree nodes in the primary tree. These nodes have no database row, so the state lives in
 *  localStorage; only collapsed IDs are kept and nodes default to expanded. */
export function useWorktreeCollapse(): [Record<string, boolean>, (id: string, collapsed: boolean) => void] {
  const [collapsed, setState] = useState(loadCollapsed);
  const setCollapsed = useCallback((id: string, value: boolean) => {
    setState((prev) => {
      const next = { ...prev };
      if (value) next[id] = true;
      else delete next[id];
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
      } catch {
        // Storage may be unavailable or full; the state still applies for this window.
      }
      return next;
    });
  }, []);
  return [collapsed, setCollapsed];
}
