//! Killing a session's process must retire its unread marker. The tree node outlives the process, so
//! `focusReturned`'s stale sweep -- which only drops IDs whose session is gone -- never reaches it, and
//! the sidebar's blue dot would otherwise sit on a dead session claiming there is something to read.

import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../ipc/sessionState", () => ({
  markSessionRead: vi.fn().mockResolvedValue(undefined),
  markSessionUnread: vi.fn().mockResolvedValue(undefined),
  reportScreen: vi.fn().mockResolvedValue(undefined),
  sessionStates: vi.fn().mockResolvedValue({ sessions: [] }),
}));
vi.mock("../ipc/tree", () => ({
  listTree: vi.fn().mockResolvedValue({ projects: [], groups: [], sessions: [] }),
}));
vi.mock("../notify", () => ({
  notify: vi.fn(),
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));

import { useTermStore } from "./termStore";
import { markSessionRead } from "../ipc/sessionState";
import type { Session } from "../types";

const session = (id: string): Session => ({
  id,
  projectId: "p1",
  groupId: null,
  parentSessionId: null,
  name: id,
  kind: "claude",
  collapsed: false,
  sortOrder: 0,
  createdAt: 0,
});

let seq = 0;
let sessionId = "";

beforeEach(() => {
  sessionId = `close-session-${++seq}`;
  useTermStore.setState({
    sessions: [session(sessionId)],
    ephemeralSessions: {},
    runtimes: { [sessionId]: { status: "running" } },
    notifications: { [sessionId]: Date.now() },
    paneTrees: {},
    openTabs: [],
    liveTabs: [],
    activeTabId: null,
  });
});

describe("closeSession and the unread marker", () => {
  it("clears the unread marker when the process stops", () => {
    expect(sessionId in useTermStore.getState().notifications).toBe(true);
    useTermStore.getState().closeSession(sessionId);
    expect(sessionId in useTermStore.getState().notifications).toBe(false);
    // The backend owns unread state, so it has to hear about it too or its next broadcast restores the dot.
    expect(markSessionRead).toHaveBeenCalledWith(sessionId);
  });

  it("leaves the session node in the tree", () => {
    useTermStore.getState().closeSession(sessionId);
    expect(useTermStore.getState().sessions.some((s) => s.id === sessionId)).toBe(true);
  });

  it("cannot rely on the stale sweep, which spares a marker whose node survives", () => {
    // focusReturned's sweep is the ONLY other thing that clears markers. It keys off the session having
    // disappeared, and the node survives the kill -- so it does nothing here. This is what makes the fix
    // belong in closeSession.
    useTermStore.getState().focusReturned();
    expect(sessionId in useTermStore.getState().notifications).toBe(true);
  });

  it("does not disturb other sessions' unread markers", () => {
    const other = `${sessionId}-other`;
    useTermStore.setState({
      sessions: [session(sessionId), session(other)],
      notifications: { [sessionId]: 1, [other]: 2 },
    });
    useTermStore.getState().closeSession(sessionId);
    const { notifications } = useTermStore.getState();
    expect(sessionId in notifications).toBe(false);
    expect(notifications[other]).toBe(2);
  });
});
