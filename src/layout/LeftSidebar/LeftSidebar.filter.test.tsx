import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Group, Project, Session, SessionRuntime } from "../../types";
import type { SidebarTreeView } from "../../store/termStore";
import type { SidebarTreeTab } from "./sidebarTreeLayout";
import { SYSTEM_CHAT, SYSTEM_PROJECTS, SYSTEM_TASKS } from "./systemCollections";

const storeState = vi.hoisted(() => ({
  leftWidth: 280,
  importProject: vi.fn(),
  openProjectPath: vi.fn(),
  addVirtualProject: vi.fn(),
  setCloneModalOpen: vi.fn(),
  renameNode: vi.fn(),
  openSession: vi.fn(),
  treeFilter: "",
  setTreeFilter: vi.fn(),
  selection: [],
  clearSelection: vi.fn(),
  archiveMany: vi.fn(),
  archiveGroup: vi.fn(),
  archiveCollection: vi.fn(),
  clearNodeWorktree: vi.fn(),
  globalSearchOpen: false,
  setGlobalSearchOpen: vi.fn(),
  notifications: {} as Record<string, number>,
  pendingSpawns: [] as unknown[],
  clearAllNotifications: vi.fn(),
  clearAllBadges: vi.fn(),
  ephemeralSessions: {} as Record<string, Session>,
  browserTabs: {},
  docTabs: {},
  renameScratch: vi.fn(),
  sessions: [] as Session[],
  runtimes: {} as Record<string, SessionRuntime>,
  statusFilter: null,
  statusFilterIds: null,
  dynamicStatusFilter: true,
  setStatusFilter: vi.fn(),
  appendSidebarTreeViewStatusMatches: vi.fn(),
  refreshSidebarTreeViewStatusMatches: vi.fn(),
  setSidebarTreeViewStatusFilter: vi.fn(),
  setSidebarTreeViewFilter: vi.fn(),
  markFilter: null,
  setMarkFilter: vi.fn(),
  setSidebarTreeViewMarkFilter: vi.fn(),
  splitSidebarTreeView: vi.fn(),
  deleteSidebarTreeView: vi.fn(),
  setActiveSidebarTreeView: vi.fn(),
  resizeSidebarTreeSplit: vi.fn(),
  shortcutOverrides: {},
  sidebarTreeViews: [] as SidebarTreeView[],
  sidebarTreeTabs: [] as SidebarTreeTab[],
  primarySidebarTreeViewId: "main",
  activeSidebarTreeViewId: "main",
  treeMutationError: null as string | null,
  groups: [] as Group[],
  projects: [] as Project[],
  treeLoaded: true,
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: string) => key,
  getLocale: () => "en",
}));
vi.mock("../../store/termStore", () => {
  const useTermStore = Object.assign(
    (selector: (state: typeof storeState) => unknown) => selector(storeState),
    { getState: () => storeState },
  );
  return { useTermStore };
});
vi.mock("../../hooks/useGitBranch", () => ({ isWorktreeGone: () => false }));
vi.mock("../../components/Icons", () => ({
  default: new Proxy({}, { get: () => () => null }),
}));
vi.mock("../../components/ContextMenu", () => ({ ContextMenu: () => null }));
vi.mock("../sessionMenu", () => ({
  sessionStatusItem: () => null,
  useSessionMenu: () => ({
    newSessionItems: vi.fn(() => []),
    buildSessionItems: vi.fn(() => []),
    buildScratchItems: vi.fn(() => []),
    buildMoveToMany: vi.fn(() => null),
    buildKillProcessMany: vi.fn(() => null),
    buildGitItems: vi.fn(() => []),
    buildMarkItem: vi.fn(() => ({ label: "mark.menu", submenu: [] })),
    openDialog: vi.fn(),
    dialogs: null,
  }),
}));
vi.mock("./ProjectTree", () => ({ ProjectTree: () => null }));
vi.mock("../GlobalSearch/GlobalSearch", () => ({ GlobalSearch: () => null }));

import { LeftSidebar } from "./LeftSidebar";

const makeProject = (
  id: string,
  name: string,
  rootPath: string,
  collectionId: string | null = null,
  mark?: string,
): Project => ({
  id,
  name,
  rootPath,
  collectionId,
  collapsed: false,
  sortOrder: 0,
  createdAt: 0,
  mark,
});

const makeSession = (
  id: string,
  projectId: string,
  extra: Partial<Session> = {},
): Session =>
  ({
    id,
    projectId,
    groupId: null,
    name: id,
    kind: "claude",
    sortOrder: 0,
    collapsed: false,
    createdAt: 0,
    ...extra,
  }) as Session;

const makeView = (id: string): SidebarTreeView => ({
  id,
  name: id,
  treeFilter: "",
  statusFilter: null,
  statusFilterIds: null,
  markFilter: null,
  collapsedOverrides: null,
});

function readFilterCounts(container: HTMLElement): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const item of container.querySelectorAll(".filter-menu-item")) {
    const label = item.querySelector(".lbl")?.textContent ?? "";
    const count = Number(item.querySelector(".cnt")?.textContent ?? "0");
    counts[label] = count;
  }
  return counts;
}

describe("LeftSidebar filter menu counts", () => {
  beforeEach(() => {
    storeState.projects = [
      makeProject(SYSTEM_PROJECTS, "Projects", ""),
      makeProject(SYSTEM_TASKS, "Tasks", ""),
      makeProject(SYSTEM_CHAT, "Chat", ""),
      makeProject("p1", "web", "/tmp/web", SYSTEM_PROJECTS, "🚧"),
    ];
    storeState.groups = [
      {
        id: "g-proj",
        projectId: "p1",
        parentGroupId: null,
        name: "G1",
        sortOrder: 0,
        collapsed: false,
        createdAt: 0,
        mark: "🔥",
      },
      {
        id: "g-task",
        projectId: SYSTEM_TASKS,
        parentGroupId: null,
        name: "TGroup",
        sortOrder: 0,
        collapsed: false,
        createdAt: 0,
        mark: "⭐",
      },
    ];
    storeState.sessions = [
      // Projects view: 2 working, 1 asking (unread), 1 waiting; 1 wip mark on session (+ 1 on project, 1 urgent on group)
      makeSession("p-work-1", "p1", { mark: "🚧" }),
      makeSession("p-work-2", "p1"),
      makeSession("p-ask-1", "p1"),
      makeSession("p-wait-1", "p1"),
      // Running terminal inside a project (also shows up in Terminals view)
      makeSession("t-live-1", "p1", { kind: "terminal", mark: "🐛" }),
      // Tasks view: 1 working, 2 waiting, 0 asking; 1 star mark on group + 1 done mark on session
      makeSession("task-work-1", SYSTEM_TASKS),
      makeSession("task-wait-1", SYSTEM_TASKS, { mark: "✅" }),
      makeSession("task-wait-2", SYSTEM_TASKS),
      // Chat view: 1 background, 1 asking; 1 idea mark
      makeSession("chat-bg-1", SYSTEM_CHAT, { mark: "💡" }),
      makeSession("chat-ask-1", SYSTEM_CHAT),
    ];
    storeState.ephemeralSessions = {
      "t-draft-1": makeSession("t-draft-1", "", { kind: "terminal", mark: "📌" }),
    };
    storeState.runtimes = {
      "p-work-1": { status: "running", agent: "claude", agentState: "working" },
      "p-work-2": { status: "running", agent: "claude", agentState: "working" },
      "p-ask-1": { status: "running", agent: "claude", agentState: "waiting" },
      "p-wait-1": { status: "running", agent: "claude", agentState: "waiting" },
      "t-live-1": { status: "running", agent: "codex", agentState: "working" },
      "t-draft-1": { status: "running" },
      "task-work-1": { status: "running", agent: "claude", agentState: "working" },
      "task-wait-1": { status: "running", agent: "claude", agentState: "waiting" },
      "task-wait-2": { status: "running", agent: "claude", agentState: "waiting" },
      "chat-bg-1": { status: "running", agent: "claude", agentState: "background" },
      "chat-ask-1": { status: "running", agent: "claude", agentState: "asking" },
    };
    storeState.notifications = { "p-ask-1": 1 };
    storeState.sidebarTreeViews = [
      makeView("main"),
      makeView("tasks-view"),
      makeView("chat-view"),
      makeView("terms-view"),
    ];
    storeState.sidebarTreeTabs = [
      { id: "tab-projects", section: "projects", root: { kind: "leaf", paneId: "p-main", viewId: "main" }, activeViewId: "main" },
      { id: "tab-tasks", section: "tasks", root: { kind: "leaf", paneId: "p-tasks", viewId: "tasks-view" }, activeViewId: "tasks-view" },
      { id: "tab-chat", section: "chat", root: { kind: "leaf", paneId: "p-chat", viewId: "chat-view" }, activeViewId: "chat-view" },
      { id: "tab-terms", section: "terminals", root: { kind: "leaf", paneId: "p-terms", viewId: "terms-view" }, activeViewId: "terms-view" },
    ];
    storeState.primarySidebarTreeViewId = "main";
    storeState.activeSidebarTreeViewId = "main";
  });

  it("shows only Tasks counts when the Tasks view is active", () => {
    storeState.activeSidebarTreeViewId = "tasks-view";
    const { container } = render(<LeftSidebar />);
    fireEvent.click(screen.getByTitle("tree.filterStatus"));
    const counts = readFilterCounts(container);

    expect(counts["tree.filterWorking"]).toBe(1);
    expect(counts["tree.filterAsking"]).toBe(0);
    expect(counts["tree.filterWaiting"]).toBe(2);
    expect(counts["tree.filterBackground"]).toBe(0);

    expect(counts["mark.important"]).toBe(1);
    expect(counts["mark.done"]).toBe(1);
    expect(counts["mark.wip"]).toBe(0);
    expect(counts["mark.urgent"]).toBe(0);
    expect(counts["mark.idea"]).toBe(0);
  });

  it("shows only Projects counts when the Projects view is active", () => {
    storeState.activeSidebarTreeViewId = "main";
    const { container } = render(<LeftSidebar />);
    fireEvent.click(screen.getByTitle("tree.filterStatus"));
    const counts = readFilterCounts(container);

    expect(counts["tree.filterWorking"]).toBe(3); // p-work-1, p-work-2, t-live-1
    expect(counts["tree.filterAsking"]).toBe(1);
    expect(counts["tree.filterWaiting"]).toBe(1);
    expect(counts["tree.filterBackground"]).toBe(0);

    expect(counts["mark.wip"]).toBe(2); // project p1 + session p-work-1
    expect(counts["mark.urgent"]).toBe(1); // group g-proj
    expect(counts["mark.bug"]).toBe(1); // t-live-1
    expect(counts["mark.done"]).toBe(0);
    expect(counts["mark.important"]).toBe(0);
  });

  it("shows only Chat counts when the Chat view is active", () => {
    storeState.activeSidebarTreeViewId = "chat-view";
    const { container } = render(<LeftSidebar />);
    fireEvent.click(screen.getByTitle("tree.filterStatus"));
    const counts = readFilterCounts(container);

    expect(counts["tree.filterWorking"]).toBe(0);
    expect(counts["tree.filterAsking"]).toBe(1);
    expect(counts["tree.filterWaiting"]).toBe(0);
    expect(counts["tree.filterBackground"]).toBe(1);

    expect(counts["mark.idea"]).toBe(1);
    expect(counts["mark.wip"]).toBe(0);
    expect(counts["mark.done"]).toBe(0);
  });

  it("shows only live terminal counts when the Terminals view is active", () => {
    storeState.activeSidebarTreeViewId = "terms-view";
    const { container } = render(<LeftSidebar />);
    fireEvent.click(screen.getByTitle("tree.filterStatus"));
    const counts = readFilterCounts(container);

    expect(counts["tree.filterWorking"]).toBe(1); // t-live-1 has a working codex agent
    expect(counts["tree.filterAsking"]).toBe(0);
    expect(counts["tree.filterWaiting"]).toBe(0);
    expect(counts["tree.filterBackground"]).toBe(0);

    expect(counts["mark.bug"]).toBe(1); // t-live-1
    expect(counts["mark.pinned"]).toBe(1); // t-draft-1
    expect(counts["mark.wip"]).toBe(0);
  });
});
