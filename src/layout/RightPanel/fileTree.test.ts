import { describe, expect, it } from "vitest";
import {
  buildSearchTree,
  directoriesToRefresh,
  flattenVisibleNodes,
  isNodeOpen,
  matchesFileQuery,
  openFilterDescendants,
  reconcileChildren,
  shouldRefreshSearch,
  toNode,
  updateFileNode,
  visibleChildren,
  visibleDirectories,
} from "./fileTree";

const dir = (name: string, badge: string | null = null) => ({ name, isDir: true, isHidden: false, gitBadge: badge });
const file = (name: string, badge: string | null = null) => ({ ...dir(name, badge), isDir: false });

describe("Files-tree reconciliation", () => {
  it("updates backend facts while retaining expanded descendants", () => {
    const child = { ...toNode(dir("src"), "/repo"), open: true, loaded: true, children: [toNode(file("文档.txt", "M"), "/repo/src")] };
    const root = { ...toNode(dir("repo"), ""), path: "/repo", open: true, loaded: true, children: [child] };
    const next = reconcileChildren(root, [dir("src", "M"), file("new.txt", "U")]);
    expect(next.children?.[0].open).toBe(true);
    expect(next.children?.[0].children).toBe(child.children);
    expect(next.children?.[0].badge).toBe("M");
    const updated = updateFileNode(next, "/repo/src", (node) => reconcileChildren(node, [file("文档.txt")]));
    expect(updated.children?.[0].children?.[0].badge).toBeNull();
    expect(updated.children?.[0].open).toBe(true);
    expect(visibleDirectories(updated)).toEqual(["/repo", "/repo/src"]);
  });

  it("removes disappeared nodes and resets a node when its type changes", () => {
    const root = { ...toNode(dir("repo"), ""), path: "/repo", children: [{ ...toNode(dir("same"), "/repo"), open: true }, toNode(file("gone"), "/repo")] };
    const next = reconcileChildren(root, [file("same", "M")]);
    expect(next.children).toHaveLength(1);
    expect(next.children?.[0].isDir).toBe(false);
    expect(next.children?.[0].open).toBe(false);
  });
});

describe("directoriesToRefresh", () => {
  const tree = {
    name: "r", path: "/r", isDir: true, isHidden: false, open: true, loaded: true,
    children: [
      { name: "src", path: "/r/src", isDir: true, isHidden: false, open: true, loaded: true, children: [
        { name: "app", path: "/r/src/app", isDir: true, isHidden: false, open: false, loaded: true, children: [] },
      ] },
      { name: "docs", path: "/r/docs", isDir: true, isHidden: false, open: false, loaded: false },
    ],
  };

  it("re-reads only the shown directories an event names", () => {
    expect(directoriesToRefresh(tree, ["", "src", "src/app", "docs", "node_modules/x"], false)).toEqual(["/r", "/r/src"]);
    expect(directoriesToRefresh(tree, ["src/app"], false)).toEqual([]);
  });

  it("re-reads everything shown for an unlisted burst or a git change", () => {
    expect(directoriesToRefresh(tree, null, false)).toEqual(["/r", "/r/src"]);
    expect(directoriesToRefresh(tree, [], true)).toEqual(["/r", "/r/src"]);
  });
});

describe("recursive file filtering helpers", () => {
  it("matches names directly and path segments when the query includes a slash", () => {
    expect(matchesFileQuery("FilesTab.tsx", "src/layout/RightPanel/FilesTab.tsx", "filestab")).toBe(true);
    expect(matchesFileQuery("helper.ts", "src/helper.ts", "src")).toBe(false);
    expect(matchesFileQuery("FilesTab.tsx", "src/layout/RightPanel/FilesTab.tsx", "RightPanel/Files")).toBe(true);
    expect(matchesFileQuery("main.rs", "src/main.rs", "/main.rs")).toBe(true);
    expect(matchesFileQuery("domain.rs", "src/domain.rs", "/main.rs")).toBe(false);
    expect(matchesFileQuery("a.ts", "a.ts", "/")).toBe(false);
  });

  it("builds a sorted tree with ancestors and Git badges from flat search results", () => {
    const root = buildSearchTree("/repo", "repo", "M", [
      { path: "src", isDir: true, isHidden: false, gitBadge: "M" },
      { path: "src/b.ts", isDir: false, isHidden: false, gitBadge: null },
      { path: "src/sub/a.ts", isDir: false, isHidden: false, gitBadge: "U" },
      { path: "README.md", isDir: false, isHidden: false, gitBadge: null },
    ]);
    expect(root.children?.map((c) => c.name)).toEqual(["src", "README.md"]);
    const src = root.children![0];
    expect(src.badge).toBe("M");
    expect(src.children?.map((c) => c.name)).toEqual(["sub", "b.ts"]);
    expect(src.children?.[0].children?.[0].path).toBe("/repo/src/sub/a.ts");
    expect(src.children?.[0].children?.[0].badge).toBe("U");
  });

  it("allows collapsing and expanding matching folders independently of root open state", () => {
    const root = buildSearchTree("/repo", "repo", null, [
      { path: "foo", isDir: true, isHidden: false, gitBadge: null },
      { path: "foo/foo_child.ts", isDir: false, isHidden: false, gitBadge: null },
      { path: "foo_leaf", isDir: true, isHidden: false, gitBadge: null },
    ]);
    const foo = root.children!.find((c) => c.name === "foo")!;
    const fooLeaf = root.children!.find((c) => c.name === "foo_leaf")!;
    const collapsed = new Set<string>();

    expect(isNodeOpen(foo, "/repo", "foo", collapsed, visibleChildren(foo, "/repo", false, "foo").length)).toBe(true);
    expect(isNodeOpen(fooLeaf, "/repo", "foo", collapsed, visibleChildren(fooLeaf, "/repo", false, "foo").length)).toBe(false);
    expect(openFilterDescendants(root, "/repo", false, "foo", collapsed)).toEqual(["/repo/foo"]);
    expect(flattenVisibleNodes(root, "/repo", false, "foo", collapsed).map((n) => n.name)).toEqual([
      "repo", "foo", "foo_child.ts", "foo_leaf",
    ]);

    collapsed.add("/repo/foo");
    expect(isNodeOpen(foo, "/repo", "foo", collapsed, visibleChildren(foo, "/repo", false, "foo").length)).toBe(false);
    expect(openFilterDescendants(root, "/repo", false, "foo", collapsed)).toEqual([]);
    expect(flattenVisibleNodes(root, "/repo", false, "foo", collapsed).map((n) => n.name)).toEqual([
      "repo", "foo", "foo_leaf",
    ]);
  });

  it("filters watch bursts for ignored or hidden directories", () => {
    expect(shouldRefreshSearch(["node_modules/pkg"], false, false)).toBe(false);
    expect(shouldRefreshSearch([".claude/hooks"], false, false)).toBe(false);
    expect(shouldRefreshSearch([".claude/hooks"], false, true)).toBe(true);
    expect(shouldRefreshSearch(["src/utils"], false, false)).toBe(true);
    expect(shouldRefreshSearch([], true, false)).toBe(true);
  });
});
