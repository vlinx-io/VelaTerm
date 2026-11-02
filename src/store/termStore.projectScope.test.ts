//! Project-scoped tab bar: switching projects, remembering each project's last tab, never jumping to a
//! hidden project's tab, and resetting when the active project disappears.

import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PaneNode } from "../layout/CenterPane/paneTree";
import type { Project, Session } from "../types";

const env = vi.hoisted(() => ({
  kind: "tauri",
  isTauri: true,
  isElectron: false,
  isBrowser: false,
  isRemoteWindow: false,
  hasNativeHost: true,
  isMac: false,
}));
const tree = vi.hoisted(() => ({
  data: { projects: [] as unknown[], groups: [] as unknown[], sessions: [] as unknown[] },
}));

vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../ipc/tree", () => ({
  listTree: vi.fn(async () => tree.data),
  createSession: vi.fn(),
  setCollapsed: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../ipc/chat", () => ({
  chatClear: vi.fn(),
  setSessionEngine: vi.fn(),
  chatStop: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../notify", () => ({ notify: vi.fn() }));
vi.mock("../ipc/transport", () => ({ isTauri: true, invokeNative: vi.fn().mockResolvedValue(undefined) }));
vi.mock("../ipc/browser", () => ({ setBrowserUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock("../platform", () => ({
  env,
  platform: { env, dialog: { pickDirectory: vi.fn(), saveFile: vi.fn() } },
}));

import * as ipcTree from "../ipc/tree";
import { useTermStore } from "./termStore";
import { visibleTabIds } from "./visibleTabs";

type State = ReturnType<typeof useTermStore.getState>;

const project = (id: string): Project => ({
  id,
  name: id,
  rootPath: `/tmp/${id}`,
  sortOrder: 0,
  collapsed: false,
  createdAt: 0,
});
const session = (id: string, projectId: string, extra: Partial<Session> = {}): Session => ({
  id,
  projectId,
  groupId: null,
  name: id,
  kind: "terminal",
  collapsed: false,
  sortOrder: 0,
  createdAt: 0,
  ...extra,
});
const leaf = (sessionId: string): PaneNode => ({ kind: "leaf", paneId: `p-${sessionId}`, sessionId });
const globalTerm = (id: string) => session(id, "", { global: true });

function seed(patch: Partial<State> = {}) {
  useTermStore.setState({
    projects: [project("A"), project("B")],
    groups: [],
    sessions: [session("a1", "A"), session("a2", "A"), session("b1", "B")],
    ephemeralSessions: {},
    runtimes: {},
    notifications: {},
    dormantSessions: {},
    openTabs: ["a1", "b1", "a2"],
    paneTrees: { a1: leaf("a1"), b1: leaf("b1"), a2: leaf("a2") },
    liveTabs: [],
    pinnedTabs: [],
    docTabs: {},
    browserTabs: {},
    taskTabs: {},
    activeTabId: "a1",
    activeSessionId: "a1",
    focusedPaneId: "p-a1",
    inspectTarget: null,
    scopeTabsToProject: true,
    activeProjectId: "A",
    lastTabByProject: {},
    singleTabMode: false,
    ...patch,
  });
}

beforeEach(() => {
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  env.isRemoteWindow = false;
  tree.data = {
    projects: [project("A"), project("B")],
    groups: [],
    sessions: [session("a1", "A"), session("a2", "A"), session("b1", "B")],
  };
  seed();
});

describe("switching the active project", () => {
  it("activates the first tab of a project without a remembered tab", () => {
    useTermStore.getState().setActiveProject("B");
    const s = useTermStore.getState();
    expect(s.activeProjectId).toBe("B");
    expect(s.activeTabId).toBe("b1");
    expect(s.activeSessionId).toBe("b1");
    expect(s.focusedPaneId).toBe("p-b1");
  });

  it("returns to the tab last active in a project", () => {
    seed({ activeTabId: "a2", activeSessionId: "a2", focusedPaneId: "p-a2" });
    useTermStore.getState().setActiveProject("B");
    useTermStore.getState().setActiveProject("A");
    expect(useTermStore.getState().activeTabId).toBe("a2");
  });

  it("shows the empty state for a project without tabs and keeps hidden tabs alive", () => {
    seed({ projects: [project("A"), project("B"), project("C")] });
    useTermStore.getState().setActiveProject("C");
    const s = useTermStore.getState();
    expect(s.activeTabId).toBeNull();
    expect(s.activeSessionId).toBeNull();
    expect(s.openTabs).toEqual(["a1", "b1", "a2"]);
  });

  it("keeps global terminals visible in every project", () => {
    seed({
      projects: [project("A"), project("B"), project("C")],
      ephemeralSessions: { g1: globalTerm("g1") },
      openTabs: ["a1", "g1"],
      paneTrees: { a1: leaf("a1"), g1: leaf("g1") },
    });
    useTermStore.getState().setActiveProject("C");
    expect(useTermStore.getState().activeTabId).toBe("g1");
  });

  it("only records the project when tabs are not scoped", () => {
    seed({ scopeTabsToProject: false });
    useTermStore.getState().setActiveProject("B");
    const s = useTermStore.getState();
    expect(s.activeProjectId).toBe("B");
    expect(s.activeTabId).toBe("a1");
  });

  it("ignores collections and unknown project ids", () => {
    seed({ projects: [project("A"), project("B"), { ...project("C"), rootPath: "" }] });
    useTermStore.getState().setActiveProject("C");
    useTermStore.getState().setActiveProject("nope");
    const s = useTermStore.getState();
    expect(s.activeProjectId).toBe("A");
    expect(s.activeTabId).toBe("a1");
  });

  it("never picks an active project in a remote window", () => {
    seed({ activeProjectId: null });
    env.isRemoteWindow = true;
    useTermStore.getState().setActiveProject("B");
    expect(useTermStore.getState().activeProjectId).toBeNull();
  });
});

describe("keeping the active tab visible", () => {
  it("never falls back to a hidden tab through the last-active anchor", () => {
    seed({
      openTabs: ["a1", "b1"],
      paneTrees: { a1: leaf("a1"), b1: leaf("b1") },
      lastActiveSessionTabId: "a1",
    });
    useTermStore.getState().closeTab("a1");
    const s = useTermStore.getState();
    expect(s.activeTabId).toBeNull();
    expect(s.lastActiveSessionTabId).not.toBe("b1");
  });

  it("moving the last visible tab to the background shows the empty state", () => {
    seed({ openTabs: ["a1", "b1"], paneTrees: { a1: leaf("a1"), b1: leaf("b1") } });
    useTermStore.getState().moveTabToBackground("a1");
    expect(useTermStore.getState().activeTabId).toBeNull();
  });

  it("closing the last visible tab shows the empty state instead of another project's tab", () => {
    seed({ openTabs: ["a1", "b1"], paneTrees: { a1: leaf("a1"), b1: leaf("b1") } });
    useTermStore.getState().closeTab("a1");
    expect(useTermStore.getState().activeTabId).toBeNull();
  });

  it("closing a tab moves to the next visible tab", () => {
    seed({ openTabs: ["b1", "a1", "a2"] });
    useTermStore.getState().closeTab("a1");
    expect(useTermStore.getState().activeTabId).toBe("a2");
  });

  it("activating a hidden tab reveals its project", () => {
    useTermStore.getState().setActiveTab("b1");
    expect(useTermStore.getState().activeProjectId).toBe("B");
  });

  it("turning the setting on reveals the active tab's project", () => {
    seed({ scopeTabsToProject: false, activeProjectId: "B" });
    useTermStore.getState().setScopeTabsToProject(true);
    expect(useTermStore.getState().activeProjectId).toBe("A");
  });

  it("falls back to a visible tab when a refresh drops the active one", async () => {
    await useTermStore.getState().loadTree(); // consumes the one-time startup restore
    seed();
    tree.data = { ...tree.data, sessions: [session("a2", "A"), session("b1", "B")] };
    await useTermStore.getState().loadTree();
    expect(useTermStore.getState().activeTabId).toBe("a2");
  });

  it("clears the active project once that project is deleted", async () => {
    await useTermStore.getState().loadTree();
    seed();
    tree.data = { projects: [project("B")], groups: [], sessions: [session("b1", "B")] };
    await useTermStore.getState().loadTree();
    const s = useTermStore.getState();
    expect(s.activeProjectId).toBeNull();
    expect(visibleTabIds(s)).toEqual(["b1"]);
  });

  it("follows the active tab when a refresh moves its session to another project", async () => {
    await useTermStore.getState().loadTree();
    seed();
    tree.data = {
      ...tree.data,
      sessions: [session("a1", "B"), session("a2", "A"), session("b1", "B")],
    };
    await useTermStore.getState().loadTree();
    const s = useTermStore.getState();
    expect(s.activeTabId).toBe("a1");
    expect(s.activeProjectId).toBe("B");
    expect(visibleTabIds(s)).toContain("a1");
  });

  it("keeps the active project null in a remote window when activating a tab", () => {
    seed({ activeProjectId: null });
    env.isRemoteWindow = true;
    useTermStore.getState().setActiveTab("b1");
    expect(useTermStore.getState().activeProjectId).toBeNull();
  });
});

describe("opening sessions and terminals", () => {
  it("switches to the project of a session opened from outside the sidebar", () => {
    useTermStore.getState().openSession("b1");
    const s = useTermStore.getState();
    expect(s.activeProjectId).toBe("B");
    expect(s.activeTabId).toBe("b1");
  });

  it("switches to the project of a session that has no tab yet", () => {
    seed({ sessions: [session("a1", "A"), session("a2", "A"), session("b1", "B"), session("b2", "B")] });
    useTermStore.getState().openSession("b2");
    const s = useTermStore.getState();
    expect(s.activeProjectId).toBe("B");
    expect(s.activeTabId).toBe("b2");
  });

  it("single-tab reuse never replaces another project's hidden tab", () => {
    seed({
      singleTabMode: true,
      sessions: [session("a1", "A", { kind: "claude" }), session("b2", "B", { kind: "claude" })],
      openTabs: ["a1"],
      paneTrees: { a1: leaf("a1") },
    });
    useTermStore.getState().openSession("b2");
    const s = useTermStore.getState();
    expect(s.openTabs).toEqual(["a1", "b2"]);
    expect(s.liveTabs).toEqual([]);
    expect(s.activeTabId).toBe("b2");
  });

  it("new terminal from a global tab joins the active project", async () => {
    seed({
      ephemeralSessions: { g1: globalTerm("g1") },
      openTabs: ["a1", "g1"],
      paneTrees: { a1: leaf("a1"), g1: leaf("g1") },
      activeTabId: "g1",
      activeSessionId: "g1",
      focusedPaneId: "p-g1",
    });
    await useTermStore.getState().newScratchTab();
    const s = useTermStore.getState();
    const created = s.ephemeralSessions[s.activeTabId!];
    expect(created.projectId).toBe("A");
    expect(created.cwd).toBe("/tmp/A");
    expect(created.global).toBeUndefined();
  });

  it("creates a global terminal that stays visible in every project", async () => {
    await useTermStore.getState().newScratchTab({ global: true });
    const id = useTermStore.getState().activeTabId!;
    const created = useTermStore.getState().ephemeralSessions[id];
    expect(created.global).toBe(true);
    expect(created.projectId).toBe("");
    expect(created.cwd).toBeNull();
    expect(useTermStore.getState().activeProjectId).toBe("A");
    useTermStore.getState().setActiveProject("B");
    expect(visibleTabIds(useTermStore.getState())).toContain(id);
  });

  it("splitting a global terminal creates another global terminal", async () => {
    seed({
      ephemeralSessions: { g1: globalTerm("g1") },
      openTabs: ["g1"],
      paneTrees: { g1: leaf("g1") },
      activeTabId: "g1",
      activeSessionId: "g1",
      focusedPaneId: "p-g1",
    });
    await useTermStore.getState().splitNew("horizontal");
    const s = useTermStore.getState();
    const split = s.ephemeralSessions[s.activeSessionId!];
    expect(split.id).not.toBe("g1");
    expect(split.global).toBe(true);
    expect(split.projectId).toBe("");
  });

  it("a new terminal for another project switches to that project", async () => {
    await useTermStore.getState().newScratchTab({ target: { projectId: "B" } });
    const s = useTermStore.getState();
    expect(s.activeProjectId).toBe("B");
    expect(visibleTabIds(s)).toContain(s.activeTabId);
  });

  it("creating a session for a project makes it the active project", async () => {
    vi.mocked(ipcTree.createSession).mockResolvedValue(session("b9", "B"));
    await useTermStore.getState().addSession({ projectId: "B", groupId: null, name: "b9" });
    expect(useTermStore.getState().activeProjectId).toBe("B");
  });

  it("creating a session keeps the active project null in a remote window", async () => {
    seed({ activeProjectId: null });
    env.isRemoteWindow = true;
    vi.mocked(ipcTree.createSession).mockResolvedValue(session("b9", "B"));
    await useTermStore.getState().addSession({ projectId: "B", groupId: null, name: "b9" });
    expect(useTermStore.getState().activeProjectId).toBeNull();
  });
  it("with scoping off, a new terminal without context lands in the first project", async () => {
    seed({ scopeTabsToProject: false, activeProjectId: "B", activeSessionId: null, activeTabId: null, inspectTarget: null });
    await useTermStore.getState().newScratchTab();
    const s = useTermStore.getState();
    expect(s.ephemeralSessions[s.activeTabId!].projectId).toBe("A");
  });

  it("a session from another project dropped into a pane switches to that project", () => {
    seed({ sessions: [session("a1", "A"), session("a2", "A"), session("b1", "B"), session("b2", "B")] });
    useTermStore.getState().openSessionInPane("b2", "p-a1");
    const s = useTermStore.getState();
    expect(s.activeTabId).toBe("b2");
    expect(s.activeProjectId).toBe("B");
    expect(visibleTabIds(s)).toContain(s.activeTabId);
  });

  it("tiling sessions of another project switches to that project", () => {
    seed({ sessions: [session("a1", "A"), session("a2", "A"), session("b1", "B"), session("b2", "B")] });
    useTermStore.getState().tileSessions(["b1", "b2"]);
    const s = useTermStore.getState();
    expect(s.activeProjectId).toBe("B");
    expect(visibleTabIds(s)).toContain(s.activeTabId);
  });

  it("opening a task tab of another project switches to that project", () => {
    useTermStore.getState().openTaskTab("b1", {
      task_id: "t1",
      task_type: "workflow",
      description: "d",
      summary: "s",
    } as Parameters<State["openTaskTab"]>[1]);
    const s = useTermStore.getState();
    expect(s.activeTabId).toMatch(/^task-/);
    expect(s.activeProjectId).toBe("B");
    expect(visibleTabIds(s)).toContain(s.activeTabId);
  });

  it("saving a browser draft while scoped keeps the active project", async () => {
    seed({
      activeProjectId: "B",
      activeTabId: "browser-1",
      activeSessionId: null,
      focusedPaneId: null,
      openTabs: ["a1", "b1", "browser-1"],
      browserTabs: { "browser-1": { id: "browser-1", url: "about:blank", title: "Web", loading: false, chromeHidden: false } },
    });
    vi.mocked(ipcTree.createSession).mockResolvedValue(session("b9", "B", { kind: "browser" }));
    await useTermStore.getState().persistBrowserDraft("browser-1");
    expect(vi.mocked(ipcTree.createSession).mock.calls.at(-1)?.[0].projectId).toBe("B");
    expect(useTermStore.getState().activeProjectId).toBe("B");
  });

  it("with scoping off, a browser draft without context lands in the first project", async () => {
    seed({
      scopeTabsToProject: false,
      activeProjectId: "B",
      activeTabId: "browser-1",
      activeSessionId: null,
      focusedPaneId: null,
      openTabs: ["a1", "b1", "browser-1"],
      browserTabs: { "browser-1": { id: "browser-1", url: "about:blank", title: "Web", loading: false, chromeHidden: false } },
    });
    vi.mocked(ipcTree.createSession).mockResolvedValue(session("a9", "A", { kind: "browser" }));
    await useTermStore.getState().persistBrowserDraft("browser-1");
    expect(vi.mocked(ipcTree.createSession).mock.calls.at(-1)?.[0].projectId).toBe("A");
  });

  it("with scoping off, single-tab reuse still replaces a tab from any project", () => {
    seed({
      scopeTabsToProject: false,
      singleTabMode: true,
      sessions: [session("a1", "A", { kind: "claude" }), session("b2", "B", { kind: "claude" })],
      openTabs: ["a1"],
      paneTrees: { a1: leaf("a1") },
      activeTabId: null,
      activeSessionId: null,
    });
    useTermStore.getState().openSession("b2");
    expect(useTermStore.getState().openTabs).toEqual(["b2"]);
  });
});
