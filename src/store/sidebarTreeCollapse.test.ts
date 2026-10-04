//! Coverage for per-view collapse state: a pane split off the sidebar tree owns its expand/collapse map, keeps it
//! apart from the shared database state used by the primary pane, and gets it back after a restart.

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
import type { Group, Project, ProjectFolder } from "../types";

const project = (id: string, collapsed: boolean): Project => ({
  id,
  name: id,
  rootPath: `/tmp/${id}`,
  collapsed,
  sortOrder: 0,
  createdAt: 0,
});

const group = (id: string, collapsed: boolean): Group => ({
  id,
  projectId: "p1",
  parentGroupId: null,
  name: id,
  collapsed,
  sortOrder: 0,
  createdAt: 0,
});

const folder = (id: string, collapsed: boolean): ProjectFolder => ({
  id,
  name: id,
  sortOrder: 0,
  collapsed,
  createdAt: 0,
});

beforeEach(() => {
  localStorage.removeItem("vlx-sidebar-tree-views");
  useTermStore.setState({
    projects: [project("p1", false)],
    groups: [group("g1", true)],
    projectFolders: [folder("f1", true)],
    sessions: [],
    ephemeralSessions: {},
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
  });
  useTermStore.setState({
    sidebarTreeTabs: [{
      id: "tab-1",
      root: { kind: "leaf", paneId: "pane-1", viewId: "main" },
      activeViewId: "main",
    }],
  });
});

describe("per-view collapse state", () => {
  it("seeds a split-off view with what the source pane currently shows", () => {
    const id = useTermStore.getState().splitSidebarTreeView("vertical", "main");
    const view = useTermStore.getState().sidebarTreeViews.find((v) => v.id === id);

    expect(view?.collapsedOverrides).toEqual({ p1: false, g1: true, f1: true });
  });

  it("snapshots folder collapse so later shared folder changes do not reach the split pane", () => {
    const id = useTermStore.getState().splitSidebarTreeView("vertical", "main");
    useTermStore.setState({ projectFolders: [folder("f1", false)] });

    const view = useTermStore.getState().sidebarTreeViews.find((v) => v.id === id);
    expect(view?.collapsedOverrides?.f1).toBe(true);
  });

  it("keeps a collapse change inside its own view", () => {
    const id = useTermStore.getState().splitSidebarTreeView("vertical", "main");
    useTermStore.getState().setSidebarTreeViewCollapsed(id, "p1", true);

    const state = useTermStore.getState();
    expect(state.sidebarTreeViews.find((v) => v.id === id)?.collapsedOverrides?.p1).toBe(true);
    // The primary view keeps following the shared database state, which nothing here touched.
    expect(state.sidebarTreeViews.find((v) => v.id === "main")?.collapsedOverrides).toBeNull();
    expect(state.projects[0].collapsed).toBe(false);
  });

  it("restores the collapse map after a restart", async () => {
    vi.useFakeTimers();
    try {
      const id = useTermStore.getState().splitSidebarTreeView("vertical", "main");
      useTermStore.getState().setSidebarTreeViewCollapsed(id, "p1", true);
      vi.advanceTimersByTime(250);

      vi.resetModules();
      const { useTermStore: restartedStore } = await import("./termStore");
      const restored = restartedStore.getState().sidebarTreeViews.find((v) => v.id === id);

      expect(restored?.collapsedOverrides).toEqual({ p1: true, g1: true, f1: true });
    } finally {
      vi.useRealTimers();
      localStorage.removeItem("vlx-sidebar-tree-views");
    }
  });
});
