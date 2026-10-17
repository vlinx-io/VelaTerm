//! Pure arrow-key navigation over the sidebar's flattened row model.
//!
//! Kept free of React and of the virtualizer on purpose: `rows` in ProjectTree is already a
//! one-dimensional flattening of the project/group/session recursion, so cursor movement is a
//! function of (rows, index, key) and nothing else. ProjectTree maps the result onto the store
//! actions it already has (`toggleNodeCollapsed`, `selectSingle`, `openSession`).
//!
//! Depth convention matches the row model: a project row sits at its collection indent (0 at the top
//! level) and `walkChildren` starts its groups and sessions one level deeper, so "parent" is the
//! nearest earlier row of smaller depth — including a collection above the projects nested in it.

/** The only row facts navigation needs. ProjectTree projects its TreeRow union onto this. */
export interface KeyNavRow {
  /** Project rows carry their collection indent; groups and sessions carry their own nesting depth. */
  depth: number;
  /** Whether the row has children to reveal — a group, or a session/project with kids. */
  expandable: boolean;
  expanded: boolean;
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
  const last = rows.length - 1;

  // With no cursor yet, the first vertical key lands on an end of the list rather than being
  // swallowed: the user pressed a navigation key, so navigation should visibly start.
  if (index < 0 || index > last) {
    if (key === "ArrowDown" || key === "Home") return { kind: "move", index: 0 };
    if (key === "ArrowUp" || key === "End") return { kind: "move", index: last };
    return null;
  }

  const row = rows[index];

  switch (key) {
    case "ArrowDown":
      return index < last ? { kind: "move", index: index + 1 } : null;
    case "ArrowUp":
      return index > 0 ? { kind: "move", index: index - 1 } : null;
    case "Home":
      return index === 0 ? null : { kind: "move", index: 0 };
    case "End":
      return index === last ? null : { kind: "move", index: last };
    case "Enter":
      return { kind: "activate", index };
    case "ArrowRight":
      // Standard tree semantics: open a closed node, then step into it. A leaf does nothing, so
      // Right never silently behaves like Down.
      if (row.expandable && !row.expanded) return { kind: "toggle", index, expand: true };
      if (row.expandable && index < last && rows[index + 1].depth > row.depth) {
        return { kind: "move", index: index + 1 };
      }
      return null;
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

/** Nearest earlier row at a shallower depth, or -1 for a row already at the top level. */
export function parentIndex(rows: KeyNavRow[], index: number): number {
  const depth = rows[index].depth;
  if (depth === 0) return -1;
  for (let i = index - 1; i >= 0; i--) {
    if (rows[i].depth < depth) return i;
  }
  return -1;
}
