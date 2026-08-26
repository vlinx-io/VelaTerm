//! Regression coverage for Codex hook-only arbitration: source selection happens before the first lifecycle event,
//! and neither screen scraping nor PTY activity may ever fabricate a Codex state.
//!
//! Arbitration itself now lives in the backend, where both clients read the same answer instead of each
//! computing its own — `src-tauri/src/session_state.rs` carries these same rules and its own tests. What
//! remains in the client is the escape hatch: set `vlx-arbitration` to `frontend` and the old chain runs
//! again. These tests exercise that path, so it cannot rot while it is still the way back.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../ipc/tree", () => ({ listTree: vi.fn().mockResolvedValue({ projects: [], groups: [], sessions: [] }) }));
vi.mock("../notify", () => ({
  notify: vi.fn(),
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));

import { useTermStore } from "./termStore";
import { effectiveStatus } from "../types";
import type { Session } from "../types";

let seq = 0;
let sessionId = "";

const visibleWorking = {
  state: "working" as const,
  visibleBlocker: false,
  visibleIdle: false,
  visibleWorking: true,
  skip: false,
};

beforeEach(() => {
  localStorage.setItem("vlx-arbitration", "frontend");
  sessionId = `codex-status-${++seq}`;
  useTermStore.setState({
    runtimes: {
      [sessionId]: {
        status: "running",
        agent: "codex",
        agentState: null,
        authoritative: false,
      },
    },
    notifications: {},
    activeTabId: null,
    paneTrees: {},
    windowFocused: true,
  });
});

afterEach(() => {
  localStorage.removeItem("vlx-arbitration");
});

describe("Codex lifecycle hook state arbitration", () => {
  it("a hooks source locks into authoritative mode before the first event and explicitly reports the status as unavailable", () => {
    useTermStore.getState().applyStatusSignal(sessionId, {
      kind: "agent",
      agent: "codex",
      stateSource: "hooks",
    });
    const runtime = useTermStore.getState().runtimes[sessionId];
    expect(runtime).toMatchObject({
      agent: "codex",
      agentState: null,
      agentStateSource: "hooks",
      agentHookReady: false,
      authoritative: true,
    });
    expect(effectiveStatus(runtime)).toBe("unavailable");

    useTermStore.getState().applyScreenDetection(sessionId, visibleWorking);
    useTermStore.getState().applyStatusSignal(sessionId, { kind: "busy", busy: true });
    expect(useTermStore.getState().runtimes[sessionId].agentState).toBeNull();

    useTermStore.getState().applyStatusSignal(sessionId, { kind: "hook_ready" });
    const ready = useTermStore.getState().runtimes[sessionId];
    expect(ready.agentHookReady).toBe(true);
    expect(effectiveStatus(ready)).toBe("running");
  });

  it("once Stop arrives, neither a screen-read working nor busy may overwrite waiting", () => {
    useTermStore.getState().applyStatusSignal(sessionId, {
      kind: "agent",
      agent: "codex",
      stateSource: "hooks",
    });
    useTermStore.getState().applyStatusSignal(sessionId, {
      kind: "state",
      state: "waiting",
      authoritative: true,
    });
    expect(useTermStore.getState().runtimes[sessionId]).toMatchObject({
      agentState: "waiting",
      authoritative: true,
    });

    useTermStore.getState().applyScreenDetection(sessionId, visibleWorking);
    useTermStore.getState().applyStatusSignal(sessionId, { kind: "busy", busy: true });
    expect(useTermStore.getState().runtimes[sessionId].agentState).toBe("waiting");
  });

  it("in full authoritative mode neither busy=true nor busy=false may rewrite the hook state", () => {
    useTermStore.getState().applyStatusSignal(sessionId, {
      kind: "agent",
      agent: "codex",
      stateSource: "hooks",
    });
    useTermStore.getState().applyStatusSignal(sessionId, {
      kind: "state",
      state: "working",
      authoritative: true,
    });
    useTermStore.getState().applyStatusSignal(sessionId, { kind: "busy", busy: false });
    expect(useTermStore.getState().runtimes[sessionId].agentState).toBe("working");

    useTermStore.setState((state) => ({
      runtimes: {
        ...state.runtimes,
        [sessionId]: { ...state.runtimes[sessionId], agentState: "waiting" },
      },
    }));
    useTermStore.getState().applyStatusSignal(sessionId, { kind: "busy", busy: true });
    expect(useTermStore.getState().runtimes[sessionId].agentState).toBe("waiting");
  });

  it("silence recovery immediately after working stays silent: it corrects the state without raising a replied notification", () => {
    vi.useFakeTimers();
    try {
      useTermStore.getState().applyStatusSignal(sessionId, {
        kind: "agent",
        agent: "codex",
        stateSource: "hooks",
      });
      useTermStore.getState().applyStatusSignal(sessionId, {
        kind: "state",
        state: "working",
        authoritative: true,
      });
      // The backend silence heal arrives inside the 1200 ms anti-flicker hold and is deferred.
      useTermStore.getState().applyStatusSignal(sessionId, {
        kind: "state",
        state: "waiting",
        silent: true,
        authoritative: true,
      });
      expect(useTermStore.getState().runtimes[sessionId].agentState).toBe("working");

      vi.advanceTimersByTime(1200);
      expect(useTermStore.getState().runtimes[sessionId].agentState).toBe("waiting");
      expect(useTermStore.getState().notifications[sessionId]).toBeUndefined();
    } finally {
      vi.useRealTimers();
    }
  });

  it("older Codex versions do not guess from the screen or busy, but still accept the official notify completion event", () => {
    useTermStore.getState().applyStatusSignal(sessionId, {
      kind: "agent",
      agent: "codex",
      stateSource: "legacy",
    });
    useTermStore.getState().applyScreenDetection(sessionId, visibleWorking);
    useTermStore.getState().applyStatusSignal(sessionId, { kind: "busy", busy: true });
    expect(useTermStore.getState().runtimes[sessionId]).toMatchObject({
      agentState: null,
      agentStateSource: "legacy",
      authoritative: false,
    });
    expect(effectiveStatus(useTermStore.getState().runtimes[sessionId])).toBe("running");

    useTermStore.getState().applyStatusSignal(sessionId, {
      kind: "state",
      state: "waiting",
    });
    expect(useTermStore.getState().runtimes[sessionId].authoritative).toBe(false);
    expect(useTermStore.getState().runtimes[sessionId].agentState).toBe("waiting");
  });
});

const filterSession = (id: string): Session => ({
  id,
  projectId: "filter-project",
  groupId: null,
  name: id,
  kind: "codex",
  collapsed: false,
  sortOrder: 0,
  createdAt: 0,
});

describe("membership updates for the sidebar status filter", () => {
  const newlyMatchingId = "filter-new-match";
  const staleId = "filter-stale";

  beforeEach(() => {
    useTermStore.setState({
      sessions: [filterSession(newlyMatchingId), filterSession(staleId)],
      runtimes: {
        [newlyMatchingId]: {
          status: "running",
          agent: "codex",
          agentState: "working",
        },
        [staleId]: {
          status: "running",
          agent: "codex",
          agentState: "waiting",
        },
      },
      notifications: {},
      sidebarTreeViews: [{
        id: "main",
        name: "Main",
        treeFilter: "",
        statusFilter: ["working"],
        statusFilterIds: { [staleId]: true },
        markFilter: null,
        collapsedOverrides: null,
      }],
      primarySidebarTreeViewId: "main",
      statusFilter: ["working"],
      statusFilterIds: { [staleId]: true },
      dynamicStatusFilter: true,
    });
  });

  it("automatically appends sessions that newly match", () => {
    useTermStore.getState().appendSidebarTreeViewStatusMatches("main");

    expect(useTermStore.getState().statusFilterIds).toEqual({
      [staleId]: true,
      [newlyMatchingId]: true,
    });
  });

  it("does not remove retained sessions when they stop matching", () => {
    useTermStore.setState((state) => ({
      runtimes: {
        ...state.runtimes,
        [newlyMatchingId]: {
          ...state.runtimes[newlyMatchingId],
          agentState: "waiting",
        },
      },
      statusFilterIds: { [staleId]: true, [newlyMatchingId]: true },
      sidebarTreeViews: state.sidebarTreeViews.map((view) => ({
        ...view,
        statusFilterIds: { [staleId]: true, [newlyMatchingId]: true },
      })),
    }));

    useTermStore.getState().appendSidebarTreeViewStatusMatches("main");

    expect(useTermStore.getState().statusFilterIds).toEqual({
      [staleId]: true,
      [newlyMatchingId]: true,
    });
  });

  it("drops a single refreshed session that no longer matches the filter", () => {
    useTermStore.getState().refreshSidebarTreeViewStatusMatch("main", staleId);

    expect(useTermStore.getState().statusFilterIds).toEqual({});
  });

  it("adds a single refreshed session that now matches the filter", () => {
    useTermStore.getState().refreshSidebarTreeViewStatusMatch("main", newlyMatchingId);

    expect(useTermStore.getState().statusFilterIds).toEqual({
      [staleId]: true,
      [newlyMatchingId]: true,
    });
  });

  it("leaves other sessions untouched when refreshing one", () => {
    useTermStore.setState((state) => ({
      runtimes: {
        ...state.runtimes,
        [newlyMatchingId]: { ...state.runtimes[newlyMatchingId], agentState: "waiting" },
      },
      statusFilterIds: { [staleId]: true, [newlyMatchingId]: true },
      sidebarTreeViews: state.sidebarTreeViews.map((view) => ({
        ...view,
        statusFilterIds: { [staleId]: true, [newlyMatchingId]: true },
      })),
    }));

    useTermStore.getState().refreshSidebarTreeViewStatusMatch("main", newlyMatchingId);

    // Only the refreshed row leaves; the equally stale one stays until it is refreshed itself.
    expect(useTermStore.getState().statusFilterIds).toEqual({ [staleId]: true });
  });

  it("keeps the original static snapshot behavior when dynamic additions are disabled", () => {
    useTermStore.setState({ dynamicStatusFilter: false });

    useTermStore.getState().appendSidebarTreeViewStatusMatches("main");

    expect(useTermStore.getState().statusFilterIds).toEqual({
      [staleId]: true,
    });
  });

  it("persists the filters of a split-off view but not the main tree's", () => {
    vi.useFakeTimers();
    try {
      localStorage.removeItem("vlx-sidebar-tree-views");
      const splitId = useTermStore.getState().splitSidebarTreeView("vertical", "main");

      useTermStore.getState().setSidebarTreeViewMarkFilter("main", "🔥");
      useTermStore.getState().setSidebarTreeViewStatusFilter("main", "asking");
      useTermStore.getState().setSidebarTreeViewMarkFilter(splitId, "⭐");
      useTermStore.getState().setSidebarTreeViewStatusFilter(splitId, "waiting");
      vi.advanceTimersByTime(250);

      const saved = JSON.parse(localStorage.getItem("vlx-sidebar-tree-views") ?? "{}") as {
        views?: Record<string, unknown>[];
      };
      const savedMain = saved.views?.find((view) => view.id === "main");
      const savedSplit = saved.views?.find((view) => view.id === splitId);
      expect(savedMain).toMatchObject({ statusFilter: null, markFilter: null });
      expect(savedSplit).toMatchObject({ markFilter: "⭐" });
      expect(savedSplit?.statusFilter).toContain("waiting");
    } finally {
      localStorage.removeItem("vlx-sidebar-tree-views");
      vi.useRealTimers();
    }
  });

  it("restores a split-off view's filters on restart and leaves the main tree unfiltered", async () => {
    localStorage.setItem("vlx-sidebar-tree-views", JSON.stringify({
      version: 2,
      views: [
        {
          id: "main",
          name: "Main",
          treeFilter: "",
          statusFilter: ["working"],
          statusFilterIds: { [staleId]: true },
          markFilter: "🔥",
          collapsedOverrides: null,
        },
        {
          id: "view-2",
          name: "View 2",
          treeFilter: "",
          statusFilter: ["working"],
          statusFilterIds: { [staleId]: true },
          markFilter: "⭐",
          collapsedOverrides: null,
        },
      ],
      tabs: [],
      primaryId: "main",
      activeId: "main",
    }));

    try {
      vi.resetModules();
      const { useTermStore: restartedStore } = await import("./termStore");
      const views = restartedStore.getState().sidebarTreeViews;
      const split = views.find((view) => view.id === "view-2");

      expect(split?.statusFilter).toEqual(["working"]);
      expect(split?.statusFilterIds).toEqual({ [staleId]: true });
      expect(split?.markFilter).toBe("⭐");
      // The main tree starts clean regardless of what an earlier session wrote.
      expect(restartedStore.getState().statusFilter).toBeNull();
      expect(restartedStore.getState().markFilter).toBeNull();
    } finally {
      localStorage.removeItem("vlx-sidebar-tree-views");
    }
  });

  it("keeps a status filter whose saved snapshot matched nothing", async () => {
    localStorage.setItem("vlx-sidebar-tree-views", JSON.stringify({
      version: 2,
      views: [
        { id: "main", name: "Main", treeFilter: "" },
        {
          id: "view-2",
          name: "View 2",
          treeFilter: "",
          statusFilter: ["working"],
          statusFilterIds: {},
          markFilter: null,
        },
      ],
      tabs: [],
      primaryId: "main",
      activeId: "main",
    }));

    try {
      vi.resetModules();
      const { useTermStore: restartedStore } = await import("./termStore");
      const split = restartedStore.getState().sidebarTreeViews.find((v) => v.id === "view-2");

      expect(split?.statusFilter).toEqual(["working"]);
      expect(split?.statusFilterIds).toEqual({});
    } finally {
      localStorage.removeItem("vlx-sidebar-tree-views");
    }
  });

  it("drops a status filter saved without its ID snapshot", async () => {
    localStorage.setItem("vlx-sidebar-tree-views", JSON.stringify({
      version: 2,
      views: [
        { id: "main", name: "Main", treeFilter: "" },
        {
          id: "view-2",
          name: "View 2",
          treeFilter: "",
          statusFilter: ["working"],
          markFilter: "🔥",
        },
      ],
      tabs: [],
      primaryId: "main",
      activeId: "main",
    }));

    try {
      vi.resetModules();
      const { useTermStore: restartedStore } = await import("./termStore");
      const split = restartedStore.getState().sidebarTreeViews.find((v) => v.id === "view-2");

      expect(split?.statusFilter).toBeNull();
      expect(split?.statusFilterIds).toBeNull();
      // The marker filter needs no snapshot, so it still comes back.
      expect(split?.markFilter).toBe("🔥");
    } finally {
      localStorage.removeItem("vlx-sidebar-tree-views");
    }
  });
});
