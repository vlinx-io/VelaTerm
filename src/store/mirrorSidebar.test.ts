//! Store-side coverage for mirroring the left tree.
//!
//! The browser run proved the visible half: a pane split on one client appears on the other, and search text
//! and status filters land on both. These cover what that run could not reach after the automation browser
//! died — a projection's own collapse map arriving from a peer, and a peer closing a split pane.

import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
  resolveSpawn: vi.fn(),
}));
vi.mock("../ipc/tree", () => ({
  listTree: vi.fn().mockResolvedValue({ projects: [], groups: [], sessions: [] }),
}));

import { buildMirrorLayout, sanitizeMirrorLayout, type MirrorLayout } from "./mirrorLayout";
import { useTermStore, type SidebarTreeView } from "./termStore";

function view(id: string, over: Partial<SidebarTreeView> = {}): SidebarTreeView {
  return {
    id,
    name: id,
    treeFilter: "",
    statusFilter: null,
    statusFilterIds: null,
    markFilter: null,
    collapsedOverrides: null,
    ...over,
  };
}

/** What a peer would publish for this sidebar, round-tripped through JSON as the socket does. */
function peerLayout(over: {
  views: SidebarTreeView[];
  tabs: MirrorLayout["left"]["tabs"];
  activeViewId?: string;
}): MirrorLayout {
  const base = buildMirrorLayout({
    ...useTermStore.getState(),
    sidebarTreeViews: over.views,
    sidebarTreeTabs: over.tabs,
    primarySidebarTreeViewId: "main",
    activeSidebarTreeViewId: over.activeViewId ?? "main",
  });
  return sanitizeMirrorLayout(JSON.parse(JSON.stringify(base)))!;
}

const leafTab = (id: string, viewId: string): MirrorLayout["left"]["tabs"][number] => ({
  id,
  root: { kind: "leaf", paneId: `sp-${viewId}`, viewId },
  activeViewId: viewId,
});

describe("applying a peer's left tree", () => {
  beforeEach(() => {
    useTermStore.setState({
      sidebarTreeViews: [view("main"), view("v2")],
      sidebarTreeTabs: [leafTab("t1", "main"), leafTab("t2", "v2")],
      primarySidebarTreeViewId: "main",
      activeSidebarTreeViewId: "v2",
    });
  });

  it("takes over a projection's own collapse map, which is not the shared tree state in the database", () => {
    const layout = peerLayout({
      views: [view("main"), view("v2", { collapsedOverrides: { p1: true, g1: false } })],
      tabs: [leafTab("t1", "main"), leafTab("t2", "v2")],
    });
    useTermStore.getState().applyMirrorLayout(layout);
    const applied = useTermStore.getState().sidebarTreeViews;
    expect(applied[1].collapsedOverrides).toEqual({ p1: true, g1: false });
    // The primary projection still follows the database rather than carrying a map of its own.
    expect(applied[0].collapsedOverrides).toBeNull();
  });

  it("closes a split pane the peer closed, instead of leaving the two sidebars a pane apart", () => {
    const layout = peerLayout({ views: [view("main")], tabs: [leafTab("t1", "main")] });
    useTermStore.getState().applyMirrorLayout(layout);
    const s = useTermStore.getState();
    expect(s.sidebarTreeViews.map((v) => v.id)).toEqual(["main"]);
    expect(s.sidebarTreeTabs).toHaveLength(1);
    // The projection that just went away cannot stay the active one.
    expect(s.activeSidebarTreeViewId).toBe("main");
  });

  it("keeps the local sidebar when a peer publishes nothing usable, rather than emptying it", () => {
    const layout = peerLayout({ views: [view("main"), view("v2")], tabs: [leafTab("t1", "main")] });
    useTermStore.getState().applyMirrorLayout({ ...layout, left: { ...layout.left, views: [] } });
    expect(useTermStore.getState().sidebarTreeViews.map((v) => v.id)).toEqual(["main", "v2"]);
  });

  it("republishes the primary projection's conditions through the top-level aliases", () => {
    const layout = peerLayout({
      views: [
        view("main", { treeFilter: "checkout", statusFilter: ["working"], statusFilterIds: { s1: true } }),
      ],
      tabs: [leafTab("t1", "main")],
    });
    useTermStore.getState().applyMirrorLayout(layout);
    const s = useTermStore.getState();
    expect(s.treeFilter).toBe("checkout");
    expect(s.statusFilter).toEqual(["working"]);
    expect(s.statusFilterIds).toEqual({ s1: true });
  });
});
