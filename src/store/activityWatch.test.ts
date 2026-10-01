//! The activity record behind "Sort by activity": which store changes count as activity, how a burst is
//! coalesced into one persisted write, which ids are ignored, and how the tree snapshot seeds the live copy.
//! Uses the real store with the IPC layer stubbed, as the other store tests do.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
}));
const treeMocks = vi.hoisted(() => ({
  touchSessionActivity: vi.fn().mockResolvedValue(true),
  listTree: vi.fn(),
}));
vi.mock("../ipc/tree", () => ({
  listTree: treeMocks.listTree,
  touchSessionActivity: treeMocks.touchSessionActivity,
}));
// The tree load waits for the first mirror alignment in a browser environment; settle it at once.
vi.mock("./mirrorAlign", () => ({ whenFirstMirrorAlign: () => Promise.resolve(false), settleFirstMirrorAlign: vi.fn() }));
vi.mock("../notify", () => ({
  notify: vi.fn(),
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));

import type { Session } from "../types";
import { isActivityTransition, startActivityWatch } from "./activityWatch";
import { useTermStore } from "./termStore";

const session = (id: string, extra: Partial<Session> = {}): Session =>
  ({
    id, projectId: "p1", groupId: null, name: id, kind: "terminal", collapsed: false, sortOrder: 0, createdAt: 0, ...extra,
  }) as Session;

const rt = (agentState: "working" | "asking" | "waiting" | null) =>
  ({ status: "running" as const, agent: "claude" as const, agentState });

describe("isActivityTransition", () => {
  it("counts a change into working, asking or waiting from another agent state", () => {
    expect(isActivityTransition(rt("working"), rt("waiting"))).toBe(true);
    expect(isActivityTransition(rt("waiting"), rt("working"))).toBe(true);
    expect(isActivityTransition(rt("working"), rt("asking"))).toBe(true);
  });

  it("counts working from no state but not asking or waiting, which the startup replay restores", () => {
    expect(isActivityTransition(undefined, rt("working"))).toBe(true);
    expect(isActivityTransition(rt(null), rt("working"))).toBe(true);
    expect(isActivityTransition(undefined, rt("waiting"))).toBe(false);
    expect(isActivityTransition(rt(null), rt("asking"))).toBe(false);
  });

  it("ignores unchanged states, leaving a state, and runtimes without an agent state", () => {
    expect(isActivityTransition(rt("working"), rt("working"))).toBe(false);
    expect(isActivityTransition(rt("working"), rt(null))).toBe(false);
    expect(isActivityTransition({ status: "idle" }, { status: "running" })).toBe(false);
  });
});

describe("activity record", () => {
  let stop: () => void = () => {};
  // The burst window is a module-level map that outlives one test, so every test starts a minute later.
  let base = 1_700_000_000_000;

  beforeEach(() => {
    base += 60_000;
    vi.useFakeTimers();
    vi.setSystemTime(new Date(base));
    treeMocks.touchSessionActivity.mockClear();
    useTermStore.setState({
      sessions: [session("a"), session("b"), session("child", { parentSessionId: "a" })],
      sessionActivity: {},
      runtimes: {},
      activeSessionId: null,
      activeTabId: null,
      paneTrees: {},
      openTabs: [],
      liveTabs: [],
      notifications: {},
    });
    stop = startActivityWatch();
  });

  afterEach(() => {
    stop();
    vi.useRealTimers();
  });

  it("does not count opening, activating or focusing a session: reading is not activity", () => {
    useTermStore.getState().openSession("a");
    useTermStore.getState().openSessionInSplit("b", "horizontal");
    useTermStore.getState().focusPane("pane-x", "b");
    useTermStore.getState().setActiveTab("a");
    useTermStore.getState().tileSessions(["a", "child"]);
    // The layout restore at startup and the fallback after a close write the active session directly.
    useTermStore.setState({ activeSessionId: "child" });
    expect(useTermStore.getState().sessionActivity).toEqual({});
    expect(treeMocks.touchSessionActivity).not.toHaveBeenCalled();
  });

  it("switching the mode on re-reads the tree once so stamps written by other clients arrive", async () => {
    // jsdom has no matchMedia, and persisting settings re-resolves the theme.
    vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
    treeMocks.listTree.mockResolvedValue({ projects: [], groups: [], sessions: [] });
    useTermStore.setState({ treeLoaded: true });
    useTermStore.getState().setSortByActivity(true);
    expect(treeMocks.listTree).toHaveBeenCalledTimes(1);
    useTermStore.getState().setSortByActivity(false);
    expect(treeMocks.listTree).toHaveBeenCalledTimes(1);
    await Promise.resolve();
  });

  it("records agent-state activity transitions and skips the startup replay of asking/waiting", () => {
    useTermStore.setState({ runtimes: { a: rt(null), b: rt(null) } });
    expect(treeMocks.touchSessionActivity).not.toHaveBeenCalled();
    // The replayed record arrives as waiting from nothing: old state, not new activity.
    useTermStore.setState({ runtimes: { a: rt("waiting"), b: rt(null) } });
    expect(useTermStore.getState().sessionActivity.a).toBeUndefined();
    // A real start and the later hand-back both count.
    useTermStore.setState({ runtimes: { a: rt("waiting"), b: rt("working") } });
    expect(useTermStore.getState().sessionActivity.b).toBe(base);
    vi.setSystemTime(new Date(base + 2_000));
    useTermStore.setState({ runtimes: { a: rt("waiting"), b: rt("waiting") } });
    expect(useTermStore.getState().sessionActivity.b).toBe(base + 2_000);
    expect(treeMocks.touchSessionActivity).toHaveBeenCalledTimes(2);
  });

  it("counts a burst once per second and writes again afterwards", () => {
    const note = useTermStore.getState().noteSessionActivity;
    note("a");
    vi.setSystemTime(new Date(base + 400));
    note("a");
    vi.setSystemTime(new Date(base + 999));
    note("a");
    expect(treeMocks.touchSessionActivity).toHaveBeenCalledTimes(1);
    expect(useTermStore.getState().sessionActivity.a).toBe(base);
    vi.setSystemTime(new Date(base + 1_000));
    note("a");
    expect(treeMocks.touchSessionActivity).toHaveBeenCalledTimes(2);
    expect(useTermStore.getState().sessionActivity.a).toBe(base + 1_000);
    // Another session has its own burst window.
    note("b");
    expect(treeMocks.touchSessionActivity).toHaveBeenCalledTimes(3);
  });

  it("ignores ids that are not persistent sessions, such as drafts and browser tabs", () => {
    useTermStore.getState().noteSessionActivity("draft-1");
    useTermStore.getState().noteSessionActivity("browser-tab-1");
    expect(treeMocks.touchSessionActivity).not.toHaveBeenCalled();
    expect(useTermStore.getState().sessionActivity).toEqual({});
  });

  it("seeds the live copy from the tree snapshot, keeping a newer local stamp and dropping vanished ids", async () => {
    useTermStore.setState({ sessionActivity: { a: 5_000, gone: 1 }, treeLoaded: true });
    treeMocks.listTree.mockResolvedValueOnce({
      projects: [{ id: "p1", name: "P", rootPath: "/tmp", color: null, sortOrder: 0, collapsed: false, createdAt: 0 }],
      groups: [],
      sessions: [session("a", { lastActiveAt: 3_000 }), session("b", { lastActiveAt: 4_000 }), session("c")],
    });
    await useTermStore.getState().loadTree();
    expect(useTermStore.getState().sessionActivity).toEqual({ a: 5_000, b: 4_000 });
  });
});
