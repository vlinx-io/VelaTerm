//! "Collapse all" in the file tree: closes every loaded directory below the root, leaves the root
//! itself open (collapsing it would hide the whole tree and read as a broken panel), and reports
//! honestly whether there is anything to collapse so the control can be disabled when idle.

import { describe, expect, it } from "vitest";
import { collapseAllBelow, hasOpenDescendant, type FileNodeT } from "./fileTree";

/** Directory node with children; `open` defaults to true so tests state closure explicitly. */
const dir = (name: string, children: FileNodeT[], open = true): FileNodeT => ({
  name,
  path: `/${name}`,
  isDir: true,
  isHidden: false,
  open,
  loaded: true,
  children,
});

const file = (name: string): FileNodeT => ({
  name,
  path: `/${name}`,
  isDir: false,
  isHidden: false,
});

const tree = () =>
  dir("root", [
    dir("src", [dir("nested", [dir("deep", [file("x.ts")])]), file("main.ts")]),
    dir("docs", [file("readme.md")]),
    file("package.json"),
  ]);

/** Every directory below the root that is still open. */
const openNames = (node: FileNodeT): string[] => {
  const out: string[] = [];
  const walk = (n: FileNodeT) => {
    for (const c of n.children || []) {
      if (c.open) out.push(c.name);
      walk(c);
    }
  };
  walk(node);
  return out;
};

describe("collapseAllBelow", () => {
  it("closes every directory at every depth, not just the top level", () => {
    const root = tree();
    expect(openNames(root)).toEqual(["src", "nested", "deep", "docs"]);
    expect(openNames(collapseAllBelow(root))).toEqual([]);
  });

  it("leaves the root open, so the panel does not go blank", () => {
    expect(collapseAllBelow(tree()).open).toBe(true);
  });

  it("returns a new tree and leaves the old one as it was", () => {
    const root = tree();
    const next = collapseAllBelow(root);
    expect(next).not.toBe(root);
    expect(openNames(root)).toEqual(["src", "nested", "deep", "docs"]);
  });

  it("keeps the loaded children, so reopening needs no refetch", () => {
    const src = collapseAllBelow(tree()).children?.find((c) => c.name === "src");
    expect(src?.children).toHaveLength(2);
    expect(src?.loaded).toBe(true);
  });

  it("returns an already collapsed tree unchanged", () => {
    const collapsed = collapseAllBelow(tree());
    expect(collapseAllBelow(collapsed)).toBe(collapsed);
  });

  it("handles a root with no children at all", () => {
    const root = dir("empty", []);
    expect(collapseAllBelow(root)).toBe(root);
    expect(root.open).toBe(true);
  });
});

describe("hasOpenDescendant", () => {
  it("is true while any directory below the root is open", () => {
    expect(hasOpenDescendant(tree())).toBe(true);
  });

  it("goes false once everything is collapsed, which is what disables the control", () => {
    expect(hasOpenDescendant(collapseAllBelow(tree()))).toBe(false);
  });

  it("finds a deeply nested open directory under closed parents", () => {
    // Only the deepest node is open: a shallow check would miss it and wrongly disable the button.
    const root = dir("root", [dir("a", [dir("b", [dir("c", [], true)], false)], false)]);
    expect(hasOpenDescendant(root)).toBe(true);
  });

  it("ignores the root's own open flag, which is always true", () => {
    const root = dir("root", [file("only.ts")]);
    expect(hasOpenDescendant(root)).toBe(false);
  });
});
