//! TaskTab state-model tests, parallel to docTabs.test.ts and browserTabs.test.ts: opening and focusing,
//! one tab per task, several tasks side by side, protection from session replacement, survival across tree
//! reconciliation, and active-state fallback after closure.
//!
//! Stub Tauri-touching modules so the store loads under Node/jsdom.

import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../ipc/tree", () => {
  const s = (id: string) => ({
    id,
    projectId: "p1",
    groupId: null,
    name: id,
    kind: "claude",
    shell: null,
    cwd: "/tmp",
    envJson: null,
    initCmd: null,
    hotkey: null,
    parentSessionId: null,
    collapsed: false,
    worktreePath: null,
    sortOrder: 0,
    createdAt: 0,
  });
  return {
    listTree: vi.fn().mockResolvedValue({
      projects: [
        { id: "p1", name: "P", rootPath: "/tmp", color: null, sortOrder: 0, collapsed: false, createdAt: 0 },
      ],
      groups: [],
      sessions: [s("A"), s("B")],
    }),
  };
});
// Provide notify's full export surface: platform adapters indirectly imported by termStore reference
// every permission helper, so stubbing notify() alone causes missing exports at load time.
vi.mock("../notify", () => ({
  notify: vi.fn(),
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));

import type { ChatBackgroundTask } from "../ipc/chat";
import type { Session } from "../types";
import { useTermStore } from "./termStore";

function mkSession(id: string): Session {
  return {
    id,
    projectId: "p1",
    groupId: null,
    name: id,
    kind: "claude",
    shell: null,
    cwd: "/tmp",
    envJson: null,
    initCmd: null,
    hotkey: null,
    parentSessionId: null,
    collapsed: false,
    worktreePath: null,
    sortOrder: 0,
    createdAt: 0,
  };
}

function task(id: string, over: Partial<ChatBackgroundTask> = {}): ChatBackgroundTask {
  return { task_id: id, task_type: "local_workflow", description: `Alpha: ${id}-worker`, summary: `probe ${id}`, ...over };
}

/** Reset to project p1, named sessions A/B, an empty center pane, and single-tab mode. */
function seed() {
  useTermStore.setState({
    projects: [
      { id: "p1", name: "P", rootPath: "/tmp", color: null, sortOrder: 0, collapsed: false, createdAt: 0 },
    ],
    groups: [],
    sessions: [mkSession("A"), mkSession("B")],
    runtimes: { A: { status: "idle" }, B: { status: "idle" } },
    epochs: {},
    ephemeralSessions: {},
    pendingPrompts: {},
    openTabs: [],
    activeTabId: null,
    paneTrees: {},
    activeSessionId: null,
    focusedPaneId: null,
    liveTabs: [],
    docTabs: {},
    browserTabs: {},
    taskTabs: {},
    notifications: {},
    singleTabMode: true,
  });
}

/** The task-tab IDs in tab order. */
function taskTabIds(): string[] {
  const s = useTermStore.getState();
  return s.openTabs.filter((id) => s.taskTabs[id]);
}

afterEach(() => {
  localStorage.clear();
});

describe("openTaskTab: creating and focusing", () => {
  it("appends a tab, activates it, clears the active session, and titles it with the static description", () => {
    seed();
    useTermStore.getState().openTaskTab("A", task("t1"));

    const s = useTermStore.getState();
    const [id] = taskTabIds();
    expect(id.startsWith("task-")).toBe(true);
    expect(s.activeTabId).toBe(id);
    expect(s.activeSessionId).toBeNull();
    expect(s.focusedPaneId).toBeNull();
    const tab = s.taskTabs[id];
    expect(tab.sessionId).toBe("A");
    expect(tab.taskId).toBe("t1");
    expect(tab.title).toBe("probe t1"); // The summary, not the live "Alpha: t1-worker" line.
    expect(tab.taskType).toBe("local_workflow");
    expect(tab.seed?.description).toBe("Alpha: t1-worker");
  });

  it("falls back to the live description, then the id, when the task has no summary", () => {
    seed();
    useTermStore.getState().openTaskTab("A", task("t1", { summary: undefined, description: "npm test" }));
    useTermStore.getState().openTaskTab("A", task("t2", { summary: undefined, description: "" }));
    const s = useTermStore.getState();
    const [first, second] = taskTabIds();
    expect(s.taskTabs[first].title).toBe("npm test");
    expect(s.taskTabs[second].title).toBe("t2");
  });

  it("opening the same task again focuses its tab instead of opening a second one", () => {
    seed();
    useTermStore.getState().openTaskTab("A", task("t1"));
    const [first] = taskTabIds();
    useTermStore.getState().openSession("A");
    expect(useTermStore.getState().activeTabId).toBe("A");

    useTermStore.getState().openTaskTab("A", task("t1", { description: "Beta: gamma-worker" }));
    const s = useTermStore.getState();
    expect(taskTabIds()).toEqual([first]);
    expect(s.activeTabId).toBe(first);
    expect(s.activeSessionId).toBeNull();
  });

  it("the same task id in another session is another tab", () => {
    seed();
    useTermStore.getState().openTaskTab("A", task("t1"));
    useTermStore.getState().openTaskTab("B", task("t1"));
    expect(taskTabIds().length).toBe(2);
  });

  it("three tasks open three tabs at once, each closable on its own", () => {
    seed();
    useTermStore.getState().openTaskTab("A", task("t1"));
    useTermStore.getState().openTaskTab("A", task("t2"));
    useTermStore.getState().openTaskTab("A", task("t3"));
    const [t1, t2, t3] = taskTabIds();
    expect(useTermStore.getState().openTabs).toEqual([t1, t2, t3]);
    expect(useTermStore.getState().activeTabId).toBe(t3);

    useTermStore.getState().closeTab(t2);
    const s = useTermStore.getState();
    expect(s.openTabs).toEqual([t1, t3]);
    expect(s.taskTabs[t2]).toBeUndefined();
    expect(s.taskTabs[t1]).toBeTruthy();
    expect(s.taskTabs[t3]).toBeTruthy();
    expect(s.activeTabId).toBe(t3);
  });
});

describe("closeTab: which tab becomes active after a task tab closes", () => {
  it("closing the active task tab picks the neighbour and restores that session's active state", () => {
    seed();
    useTermStore.getState().openSession("A");
    useTermStore.getState().openTaskTab("A", task("t1"));
    const [id] = taskTabIds();
    expect(useTermStore.getState().activeTabId).toBe(id);

    useTermStore.getState().closeTab(id);
    const s = useTermStore.getState();
    expect(s.openTabs).toEqual(["A"]);
    expect(s.taskTabs[id]).toBeUndefined();
    expect(s.activeTabId).toBe("A");
    expect(s.activeSessionId).toBe("A");
  });

  it("closing the last tab leaves no active tab or session", () => {
    seed();
    useTermStore.getState().openTaskTab("A", task("t1"));
    const [id] = taskTabIds();
    useTermStore.getState().closeTab(id);
    const s = useTermStore.getState();
    expect(s.openTabs).toEqual([]);
    expect(s.activeTabId).toBeNull();
    expect(s.activeSessionId).toBeNull();
  });
});

describe("setActiveTab: switching between a task tab and a session tab", () => {
  it("a task tab has no active session; switching back to the session restores it", () => {
    seed();
    useTermStore.getState().openSession("A");
    useTermStore.getState().openTaskTab("A", task("t1"));
    const [id] = taskTabIds();

    useTermStore.getState().setActiveTab("A");
    expect(useTermStore.getState().activeTabId).toBe("A");
    expect(useTermStore.getState().activeSessionId).toBe("A");
    expect(useTermStore.getState().openTabs).toContain(id); // The task tab stays open behind it.

    useTermStore.getState().setActiveTab(id);
    const s = useTermStore.getState();
    expect(s.activeTabId).toBe(id);
    expect(s.activeSessionId).toBeNull();
    expect(s.focusedPaneId).toBeNull();
  });
});

describe("core regression: a session never overwrites a task tab", () => {
  it("openSession in single-tab mode while a task tab is active opens the session in a new tab and leaves the task tab untouched", () => {
    seed();
    useTermStore.getState().openTaskTab("A", task("t1"));
    const [id] = taskTabIds();

    useTermStore.getState().openSession("A");

    const s = useTermStore.getState();
    expect(s.openTabs).toContain(id);
    expect(s.taskTabs[id]).toBeTruthy();
    expect(s.openTabs).toContain("A");
    expect(s.activeTabId).toBe("A");
    expect(s.activeSessionId).toBe("A");
    expect(s.liveTabs).not.toContain(id);
  });
});

describe("a tree refresh (loadTree reconciliation) does not lose task tabs", () => {
  it("a later loadTree keeps task tabs, and deleting a session does not affect them", async () => {
    seed();
    // The first loadTree follows startup restoration; empty localStorage only flips the restored flag.
    await useTermStore.getState().loadTree();

    useTermStore.getState().openTaskTab("A", task("t1"));
    const [id] = taskTabIds();
    useTermStore.getState().openSession("A");

    // Simulate a tree-change refresh while mocked listTree still returns A/B.
    await useTermStore.getState().loadTree();
    let s = useTermStore.getState();
    expect(s.openTabs).toContain(id);
    expect(s.taskTabs[id]).toBeTruthy();

    // Simulate A being deleted elsewhere; reconciliation removes A but preserves the task tab.
    const { listTree } = await import("../ipc/tree");
    vi.mocked(listTree).mockResolvedValueOnce({
      projects: [
        { id: "p1", name: "P", rootPath: "/tmp", color: null, sortOrder: 0, collapsed: false, createdAt: 0 },
      ],
      groups: [],
      sessions: [mkSession("B")],
    });
    await useTermStore.getState().loadTree();
    s = useTermStore.getState();
    expect(s.openTabs).not.toContain("A");
    expect(s.openTabs).toContain(id);
    expect(s.taskTabs[id]).toBeTruthy();
  });
});
