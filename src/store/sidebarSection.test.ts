//! Coverage for sidebar section groups: every section owns its pane group, the activity bar swaps whole groups, and
//! a group's splits and pane conditions survive while another section is shown.

import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../ipc/tree", () => ({
  listTree: vi.fn().mockResolvedValue({ projects: [], groups: [], sessions: [] }),
  setCollapsed: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../notify", () => ({
  notify: vi.fn(),
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));

import { useTermStore } from "./termStore";
import {
  assignSidebarTabSections,
  collectSidebarViewIds,
  sidebarTabOfView,
  sidebarTabSection,
} from "../layout/LeftSidebar/sidebarTreeLayout";

/** Section of the group currently shown in the sidebar. */
const shown = () => {
  const s = useTermStore.getState();
  return sidebarTabSection(sidebarTabOfView(s.sidebarTreeTabs, s.activeSidebarTreeViewId));
};
const groupViews = (section: string) => {
  const tab = useTermStore.getState().sidebarTreeTabs.find((candidate) => sidebarTabSection(candidate) === section);
  return tab ? collectSidebarViewIds(tab.root) : [];
};

beforeEach(() => {
  localStorage.removeItem("vlx-sidebar-tree-views");
  useTermStore.setState({
    leftCollapsed: false,
    sidebarTreeViews: [{
      id: "main",
      name: "Main",
      treeFilter: "",
      statusFilter: null,
      statusFilterIds: null,
      markFilter: null,
      collapsedOverrides: null,
    }],
    primarySidebarTreeViewId: "main",
    activeSidebarTreeViewId: "main",
    sidebarTreeTabs: [{
      id: "tab-1",
      root: { kind: "leaf", paneId: "pane-1", viewId: "main" },
      activeViewId: "main",
      section: "projects",
    }],
  });
});

describe("sidebar section groups", () => {
  it("swaps the whole pane group and keeps each group's splits", () => {
    const split = useTermStore.getState().splitSidebarTreeView("vertical", "main");
    useTermStore.getState().setSidebarTreeViewFilter(split, "api");
    expect(groupViews("projects")).toEqual(["main", split]);

    // The split pane is active, yet picking a section replaces both panes with that section's group.
    useTermStore.getState().pickSidebarSection("tasks");
    expect(shown()).toBe("tasks");
    const tasksViews = groupViews("tasks");
    expect(tasksViews).toHaveLength(1);
    expect(tasksViews).not.toContain("main");
    expect(tasksViews).not.toContain(split);

    // A section's group can be split on its own.
    const tasksSplit = useTermStore.getState().splitSidebarTreeView("vertical", tasksViews[0]);
    expect(groupViews("tasks")).toEqual([tasksViews[0], tasksSplit]);

    // Coming back restores the Projects group as it was left, filter and active pane included.
    useTermStore.getState().pickSidebarSection("projects");
    expect(shown()).toBe("projects");
    expect(groupViews("projects")).toEqual(["main", split]);
    expect(useTermStore.getState().activeSidebarTreeViewId).toBe(split);
    expect(useTermStore.getState().sidebarTreeViews.find((v) => v.id === split)?.treeFilter).toBe("api");

    useTermStore.getState().pickSidebarSection("tasks");
    expect(groupViews("tasks")).toEqual([tasksViews[0], tasksSplit]);
    expect(useTermStore.getState().activeSidebarTreeViewId).toBe(tasksSplit);
  });

  it("splits a section that starts expanded without copying the shared collapse state", () => {
    useTermStore.setState({ groups: [{ id: "g1", projectId: "p1", parentGroupId: null, name: "g", sortOrder: 0, collapsed: true, createdAt: 0 }] });
    useTermStore.getState().pickSidebarSection("tasks");
    const [tasksView] = groupViews("tasks");
    useTermStore.getState().setSidebarTreeViewCollapsed(tasksView, "s1", true);

    const tasksSplit = useTermStore.getState().splitSidebarTreeView("vertical", tasksView);
    const views = useTermStore.getState().sidebarTreeViews;
    expect(views.find((v) => v.id === tasksSplit)?.collapsedOverrides).toEqual({ s1: true });

    // Projects still starts from what its source pane shows.
    const projectsSplit = useTermStore.getState().splitSidebarTreeView("vertical", "main");
    expect(useTermStore.getState().sidebarTreeViews.find((v) => v.id === projectsSplit)?.collapsedOverrides?.g1).toBe(true);
  });

  it.each(["terminals", "tasks", "chat"] as const)("restores the %s pane's own collapse state after a restart", async (section) => {
    vi.useFakeTimers();
    try {
      useTermStore.getState().pickSidebarSection(section);
      const [view] = groupViews(section);
      expect(useTermStore.getState().sidebarTreeViews.find((v) => v.id === view)?.collapsedOverrides).toBeNull();

      useTermStore.getState().setSidebarTreeViewCollapsed(view, "node", true);
      vi.advanceTimersByTime(250);

      vi.resetModules();
      const { useTermStore: restartedStore } = await import("./termStore");
      const restored = restartedStore.getState();
      expect(restored.sidebarTreeViews.find((v) => v.id === view)?.collapsedOverrides).toEqual({ node: true });
      expect(sidebarTabSection(sidebarTabOfView(restored.sidebarTreeTabs, view))).toBe(section);
      // The pane's collapse never reaches the shared state the primary view follows.
      expect(restored.sidebarTreeViews.find((v) => v.id === "main")?.collapsedOverrides).toBeNull();
    } finally {
      vi.useRealTimers();
      localStorage.removeItem("vlx-sidebar-tree-views");
    }
  });

  it("toggles the sidebar when the shown section is picked again", () => {
    useTermStore.getState().pickSidebarSection("projects");
    expect(useTermStore.getState().leftCollapsed).toBe(true);
    useTermStore.getState().pickSidebarSection("projects");
    expect(useTermStore.getState().leftCollapsed).toBe(false);
  });

  it("keeps an existing split in the primary view's group when migrating per-pane sections", () => {
    const tabs = assignSidebarTabSections(
      [{
        id: "t1",
        root: {
          kind: "split", paneId: "s", dir: "vertical", sizes: [60, 40],
          a: { kind: "leaf", paneId: "a", viewId: "main" },
          b: { kind: "leaf", paneId: "b", viewId: "v2" },
        },
        activeViewId: "v2",
      }],
      "main",
      (viewId) => (viewId === "v2" ? "terminals" : undefined),
    );
    expect(tabs).toHaveLength(1);
    expect(tabs[0].section).toBe("projects");
    expect(collectSidebarViewIds(tabs[0].root)).toEqual(["main", "v2"]);
  });
});
