//! Arrow-key navigation over the flattened sidebar rows: vertical movement clamps at both ends,
//! Right opens then steps in, Left closes then climbs.

import { describe, expect, it } from "vitest";
import {
  parentIndex,
  treeKeyAction,
  TREE_NAV_KEYS,
  type KeyNavRow,
} from "./treeKeyNav";

/** project > group (open) > session (closed, has kids) > second top-level session. */
const tree: KeyNavRow[] = [
  { depth: 0, expandable: true, expanded: true }, // 0 project
  { depth: 1, expandable: true, expanded: true }, // 1 group
  { depth: 2, expandable: true, expanded: false }, // 2 session with collapsed kids
  { depth: 1, expandable: false, expanded: false }, // 3 leaf session under the project
];

describe("treeKeyAction", () => {
  it("moves down and up one row and stops at the ends", () => {
    expect(treeKeyAction(tree, 0, "ArrowDown")).toEqual({ kind: "move", index: 1 });
    expect(treeKeyAction(tree, 2, "ArrowUp")).toEqual({ kind: "move", index: 1 });
    expect(treeKeyAction(tree, 3, "ArrowDown")).toBeNull();
    expect(treeKeyAction(tree, 0, "ArrowUp")).toBeNull();
  });

  it("jumps to the first and last row", () => {
    expect(treeKeyAction(tree, 2, "Home")).toEqual({ kind: "move", index: 0 });
    expect(treeKeyAction(tree, 1, "End")).toEqual({ kind: "move", index: 3 });
    expect(treeKeyAction(tree, 0, "Home")).toBeNull();
    expect(treeKeyAction(tree, 3, "End")).toBeNull();
  });

  it("starts navigation from either end when no row is current yet", () => {
    expect(treeKeyAction(tree, -1, "ArrowDown")).toEqual({ kind: "move", index: 0 });
    expect(treeKeyAction(tree, -1, "ArrowUp")).toEqual({ kind: "move", index: 3 });
    expect(treeKeyAction(tree, -1, "Enter")).toBeNull();
  });

  it("opens a closed row with Right, then steps into it", () => {
    expect(treeKeyAction(tree, 2, "ArrowRight")).toEqual({ kind: "toggle", index: 2, expand: true });
    expect(treeKeyAction(tree, 0, "ArrowRight")).toEqual({ kind: "move", index: 1 });
  });

  it("does not let Right act as Down on a leaf", () => {
    // The leaf must NOT be the last row, or an implementation that simply steps down would pass
    // this by hitting the end-of-list clamp instead of by respecting the leaf.
    const leafThenSibling: KeyNavRow[] = [
      { depth: 0, expandable: true, expanded: true },
      { depth: 1, expandable: false, expanded: false }, // leaf, with a row after it
      { depth: 1, expandable: false, expanded: false },
    ];
    expect(treeKeyAction(leafThenSibling, 1, "ArrowRight")).toBeNull();
    // And a container whose children are already showing still steps in rather than stalling.
    expect(treeKeyAction(leafThenSibling, 0, "ArrowRight")).toEqual({ kind: "move", index: 1 });
  });

  it("closes an open row with Left, then climbs to the parent", () => {
    expect(treeKeyAction(tree, 1, "ArrowLeft")).toEqual({
      kind: "toggle",
      index: 1,
      expand: false,
    });
    // Row 2 is closed, so Left climbs to its group; row 3's parent is the project.
    expect(treeKeyAction(tree, 2, "ArrowLeft")).toEqual({ kind: "move", index: 1 });
    expect(treeKeyAction(tree, 3, "ArrowLeft")).toEqual({ kind: "move", index: 0 });
  });

  it("has nowhere to climb from a top-level row that is already closed", () => {
    const closedProject: KeyNavRow[] = [{ depth: 0, expandable: true, expanded: false }];
    expect(treeKeyAction(closedProject, 0, "ArrowLeft")).toBeNull();
  });

  it("activates the current row on Enter and ignores unrelated keys", () => {
    expect(treeKeyAction(tree, 2, "Enter")).toEqual({ kind: "activate", index: 2 });
    expect(treeKeyAction(tree, 2, "a")).toBeNull();
    expect(treeKeyAction(tree, 2, "Tab")).toBeNull();
  });

  it("does nothing at all on an empty tree", () => {
    for (const key of TREE_NAV_KEYS) expect(treeKeyAction([], -1, key)).toBeNull();
  });
});

describe("parentIndex", () => {
  it("skips siblings and finds the nearest shallower row", () => {
    const deep: KeyNavRow[] = [
      { depth: 0, expandable: true, expanded: true },
      { depth: 1, expandable: false, expanded: false },
      { depth: 1, expandable: false, expanded: false },
      { depth: 1, expandable: false, expanded: false },
    ];
    expect(parentIndex(deep, 3)).toBe(0);
    expect(parentIndex(deep, 0)).toBe(-1);
  });

  it("climbs from a project nested in a collection to the collection", () => {
    // collection (indent 0) > its own session (1) > nested project (indent 1) > that project's session (2).
    const nested: KeyNavRow[] = [
      { depth: 0, expandable: true, expanded: true },
      { depth: 1, expandable: false, expanded: false },
      { depth: 1, expandable: true, expanded: true },
      { depth: 2, expandable: false, expanded: false },
    ];
    expect(parentIndex(nested, 3)).toBe(2);
    expect(parentIndex(nested, 2)).toBe(0);
    expect(treeKeyAction(nested, 2, "ArrowLeft")).toEqual({ kind: "toggle", index: 2, expand: false });
    expect(treeKeyAction(nested.map((r, i) => (i === 2 ? { ...r, expanded: false } : r)), 2, "ArrowLeft"))
      .toEqual({ kind: "move", index: 0 });
  });
});
