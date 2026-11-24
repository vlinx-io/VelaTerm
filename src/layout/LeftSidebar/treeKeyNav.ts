//! Pure arrow-key navigation over the sidebar's flattened row model.
//!
//! Kept free of React and of the virtualizer on purpose: `rows` in ProjectTree is already a
//! one-dimensional flattening of the project/group/session/worktree/live recursion, so cursor movement is a
//! function of (rows, index, key) and nothing else. ProjectTree maps the result onto the store
//! actions it already has (`toggleNodeCollapsed`, `setWtNodeCollapsed`, `selectSingle`, `openSession`).
//!
//! Depth convention matches the row model: a project row sits at its collection indent (0 at the top
//! level) and `walkChildren` starts its groups and sessions one level deeper, so "parent" is the
//! nearest earlier focusable row of smaller depth — including a collection above the projects nested in it.
//! Decorative rows (such as section headers) mark `focusable: false` and are skipped.

/** The only row facts navigation needs. ProjectTree projects its TreeRow union onto this. */
export interface KeyNavRow {
  /** Project rows carry their collection indent; groups, worktrees, and sessions carry their nesting depth. */
  depth: number;
  /** Whether the row has children to reveal — a group, worktree, live list, or a session/project with kids. */
  expandable: boolean;
  expanded: boolean;
  /** False for non-interactive rows such as section dividers; omitted or true means focusable. */
  focusable?: boolean;
}

export type TreeKeyResult =
  | { kind: "move"; index: number }
  | { kind: "toggle"; index: number; expand: boolean }
  | { kind: "activate"; index: number };

/** Keys this module claims. Callers use it to decide whether to preventDefault. */
export const TREE_NAV_KEYS = new Set([
  "ArrowDown",
  "ArrowUp",
  "ArrowRight",
  "ArrowLeft",
  "Home",
  "End",
  "Enter",
]);

function isFocusable(row: KeyNavRow | undefined): boolean {
  return !!row && row.focusable !== false;
}

function findFocusable(rows: KeyNavRow[], start: number, step: 1 | -1): number {
  for (let i = start; i >= 0 && i < rows.length; i += step) {
    if (isFocusable(rows[i])) return i;
  }
  return -1;
}

/**
 * Resolve one keystroke against the flattened rows. `index` is the current cursor, or -1 when the
 * tree has focus but no row is current yet. Returns null when the key does nothing here, which
 * leaves it to the browser and to other handlers.
 */
export function treeKeyAction(
  rows: KeyNavRow[],
  index: number,
  key: string,
): TreeKeyResult | null {
  if (rows.length === 0) return null;
  const first = findFocusable(rows, 0, 1);
  if (first < 0) return null;
  const last = findFocusable(rows, rows.length - 1, -1);

  // With no cursor yet, the first vertical key lands on an end of the list rather than being
  // swallowed: the user pressed a navigation key, so navigation should visibly start.
  if (index < 0 || index >= rows.length || !isFocusable(rows[index])) {
    if (key === "ArrowDown" || key === "Home") return { kind: "move", index: first };
    if (key === "ArrowUp" || key === "End") return { kind: "move", index: last };
    return null;
  }

  const row = rows[index];

  switch (key) {
    case "ArrowDown": {
      const next = findFocusable(rows, index + 1, 1);
      return next >= 0 ? { kind: "move", index: next } : null;
    }
    case "ArrowUp": {
      const prev = findFocusable(rows, index - 1, -1);
      return prev >= 0 ? { kind: "move", index: prev } : null;
    }
    case "Home":
      return index === first ? null : { kind: "move", index: first };
    case "End":
      return index === last ? null : { kind: "move", index: last };
    case "Enter":
      return { kind: "activate", index };
    case "ArrowRight": {
      // Standard tree semantics: open a closed node, then step into it. A leaf does nothing, so
      // Right never silently behaves like Down.
      if (row.expandable && !row.expanded) return { kind: "toggle", index, expand: true };
      const next = findFocusable(rows, index + 1, 1);
      if (row.expandable && next >= 0 && rows[next].depth > row.depth) {
        return { kind: "move", index: next };
      }
      return null;
    }
    case "ArrowLeft": {
      // Mirror image: close an open node, otherwise climb to the parent row.
      if (row.expandable && row.expanded) return { kind: "toggle", index, expand: false };
      const parent = parentIndex(rows, index);
      return parent < 0 ? null : { kind: "move", index: parent };
    }
    default:
      return null;
  }
}

/** Nearest earlier focusable row at a shallower depth, or -1 for a row already at the top level. */
export function parentIndex(rows: KeyNavRow[], index: number): number {
  const depth = rows[index].depth;
  if (depth === 0) return -1;
  for (let i = index - 1; i >= 0; i--) {
    if (isFocusable(rows[i]) && rows[i].depth < depth) return i;
  }
  return -1;
}
