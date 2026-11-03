//! NewSessionTab state-model tests: opening, focusing by ID, retargeting, closing back to the source pane,
//! surviving tree reconciliation, and staying out of the published mirror layout.
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

import { buildMirrorLayout } from "./mirrorLayout";
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
    newSessionTabs: {},
    notifications: {},
    singleTabMode: true,
  });
}

afterEach(() => {
  localStorage.clear();
});

describe("New Session tabs", () => {
  it("opens an active tab with no session or pane and records where to return", () => {
    seed();
    useTermStore.getState().openSession("A");
    const pane = useTermStore.getState().focusedPaneId;
    const id = useTermStore.getState().openNewSessionTab({ projectId: "p1", parentSessionId: "A" });
    const s = useTermStore.getState();
    expect(id.startsWith("new-session-")).toBe(true);
    expect(s.openTabs).toEqual(["A", id]);
    expect(s.activeTabId).toBe(id);
    expect(s.activeSessionId).toBeNull();
    expect(s.focusedPaneId).toBeNull();
    expect(s.newSessionTabs[id]).toMatchObject({ projectId: "p1", groupId: null, parentSessionId: "A",
      returnTabId: "A", returnPaneId: pane });
  });

  it("focuses an open tab by ID and opens a missing one under the requested ID", () => {
    seed();
    const first = useTermStore.getState().openNewSessionTab({ projectId: "p1" });
    useTermStore.getState().openSession("A");
    expect(useTermStore.getState().openNewSessionTab({ projectId: "p1" }, first)).toBe(first);
    expect(useTermStore.getState().activeTabId).toBe(first);
    expect(useTermStore.getState().openTabs.filter((t) => t.startsWith("new-session-"))).toEqual([first]);
    expect(useTermStore.getState().openNewSessionTab({ projectId: "p1" }, "new-session-restored")).toBe("new-session-restored");
    expect(useTermStore.getState().newSessionTabs["new-session-restored"]).toBeDefined();
  });

  it("retargets the location without moving focus", () => {
    seed();
    const id = useTermStore.getState().openNewSessionTab({ projectId: "p1", parentSessionId: "A" });
    useTermStore.getState().retargetNewSessionTab(id, { projectId: "p2" });
    expect(useTermStore.getState().newSessionTabs[id]).toMatchObject({ projectId: "p2", groupId: null, parentSessionId: null });
    expect(useTermStore.getState().activeTabId).toBe(id);
  });

  it("closing returns to the pane it was opened from and drops its metadata", () => {
    seed();
    useTermStore.getState().openSession("A");
    const pane = useTermStore.getState().focusedPaneId;
    const id = useTermStore.getState().openNewSessionTab({ projectId: "p1" });
    useTermStore.getState().closeTab(id);
    const s = useTermStore.getState();
    expect(s.newSessionTabs[id]).toBeUndefined();
    expect(s.openTabs).toEqual(["A"]);
    expect(s.activeTabId).toBe("A");
    expect(s.activeSessionId).toBe("A");
    expect(s.focusedPaneId).toBe(pane);
  });

  it("survives a tree refresh and is never replaced when a session opens in single-tab mode", async () => {
    seed();
    // The first load is startup restoration; the refresh under test is the one after it.
    await useTermStore.getState().loadTree();
    const id = useTermStore.getState().openNewSessionTab({ projectId: "p1" });
    await useTermStore.getState().loadTree();
    expect(useTermStore.getState().newSessionTabs[id]).toBeDefined();
    useTermStore.getState().openSession("A");
    useTermStore.getState().openSession("B");
    expect(useTermStore.getState().openTabs).toContain(id);
  });

  it("is left out of the published mirror layout, which names the return tab as active", () => {
    seed();
    useTermStore.getState().openSession("A");
    const id = useTermStore.getState().openNewSessionTab({ projectId: "p1" });
    const layout = buildMirrorLayout(useTermStore.getState());
    expect(layout.center.openTabs).toEqual(["A"]);
    expect(layout.center.activeTabId).toBe("A");
    expect(layout.center.activeSessionId).toBe("A");
    expect(useTermStore.getState().activeTabId).toBe(id);
  });
});
