//! On the public share surface the activity record stays off: a visitor's focus is not the host's activity,
//! and the restricted share dispatch rejects the write, which would otherwise log one failed request per focus.

import { describe, expect, it, vi } from "vitest";

vi.mock("../ipc/shareBase", () => ({
  shareBasePath: "/r/0123",
  isShareSurface: true,
  apiUrl: (path: string) => (path.startsWith("/") ? `/r/0123${path}` : path),
}));
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
vi.mock("./mirrorAlign", () => ({ whenFirstMirrorAlign: () => Promise.resolve(false), settleFirstMirrorAlign: vi.fn() }));
vi.mock("../notify", () => ({
  notify: vi.fn(),
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));

import type { Session } from "../types";
import { startActivityWatch } from "./activityWatch";
import { useTermStore } from "./termStore";

const session = (id: string): Session =>
  ({ id, projectId: "p1", groupId: null, name: id, kind: "terminal", collapsed: false, sortOrder: 0, createdAt: 0 }) as Session;

describe("activity record on the share surface", () => {
  it("records nothing for input or agent state and never calls the backend", () => {
    useTermStore.setState({
      sessions: [session("a")],
      sessionActivity: {},
      runtimes: {},
      activeSessionId: null,
      activeTabId: null,
      paneTrees: {},
      openTabs: [],
      liveTabs: [],
      notifications: {},
    });
    const stop = startActivityWatch();
    // The input paths call the record directly.
    useTermStore.getState().noteSessionActivity("a");
    // The agent-state observer runs here too.
    useTermStore.setState({ runtimes: { a: { status: "running", agent: "claude", agentState: "working" } } });
    expect(useTermStore.getState().sessionActivity).toEqual({});
    expect(treeMocks.touchSessionActivity).not.toHaveBeenCalled();
    stop();
  });
});
