//! Regression coverage for session lifecycle across clients.
//!
//! Two bugs lived here. Restarting a session made its tab vanish on **every** client, because the death
//! announcement was empty and the only safe reading of an empty announcement is "gone". And a browser
//! connecting to a desktop that had merely *restored* a workspace started every one of those sessions for
//! real, because a client that had not opened a session could not tell "not running" from "running, just
//! not opened here" — so it mounted a terminal, and mounting starts a process.

import { beforeEach, describe, expect, it, vi } from "vitest";

const { ptyKill } = vi.hoisted(() => ({ ptyKill: vi.fn().mockResolvedValue(undefined) }));
vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill,
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
  resolveSpawn: vi.fn().mockResolvedValue(true),
}));
vi.mock("../ipc/sessionState", () => ({
  markSessionRead: vi.fn().mockResolvedValue(undefined),
  markSessionUnread: vi.fn().mockResolvedValue(undefined),
  sessionStates: vi.fn().mockResolvedValue({}),
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
import { registerTerminal, unregisterTerminal } from "../terminal/registry";

const SID = "life-1";
const leaf = (sessionId: string) => ({ kind: "leaf" as const, paneId: `p-${sessionId}`, sessionId });

beforeEach(() => {
  ptyKill.mockClear();
  useTermStore.setState({
    dormantSessions: {},
    runtimes: {},
    notifications: {},
    paneTrees: {},
    openTabs: [],
    liveTabs: [],
  });
});

describe("restarting a session", () => {
  it("says it is a restart, so other clients keep their pane", async () => {
    await useTermStore.getState().restartSession(SID);

    expect(ptyKill).toHaveBeenCalledWith(SID, "restart");
  });
});

describe("what to mount for a laid-out session", () => {
  it("renders a placeholder for a session with no process behind it", () => {
    // Mounting a terminal is what starts a process, so a client must not mount one for a session the
    // backend says is not running — that is the browser-launches-everything bug.
    useTermStore.setState({ paneTrees: { t1: leaf(SID) } });

    useTermStore.getState().applySessionStates({ [SID]: { alive: false } });

    expect(useTermStore.getState().dormantSessions[SID]).toBe(true);
  });

  it("drops the placeholder once a process exists", () => {
    // Someone started the session elsewhere; this client attaches to the running terminal instead.
    useTermStore.setState({ paneTrees: { t1: leaf(SID) }, dormantSessions: { [SID]: true } });

    useTermStore.getState().applySessionStates({ [SID]: { alive: true } });

    expect(SID in useTermStore.getState().dormantSessions).toBe(false);
  });

  it("leaves a terminal this client is already showing on screen", () => {
    // A process exiting must not replace an open terminal with a card: the user still wants to read what
    // it printed before it died.
    const term = {} as never;
    registerTerminal(SID, term);
    useTermStore.setState({ paneTrees: { t1: leaf(SID) } });

    useTermStore.getState().applySessionStates({ [SID]: { alive: false } });

    expect(SID in useTermStore.getState().dormantSessions).toBe(false);
    unregisterTerminal(SID, term);
  });

  it("says nothing about a session that is not in the layout", () => {
    useTermStore.getState().applySessionStates({ [SID]: { alive: false } });

    expect(SID in useTermStore.getState().dormantSessions).toBe(false);
  });

  it("treats an absent record as unknown rather than as 'not running'", () => {
    // A session nobody has ever started has no record at all. Reading that silence as "not running"
    // would put a placeholder in front of every newly created session instead of starting it.
    useTermStore.setState({ paneTrees: { t1: leaf(SID) } });

    useTermStore.getState().applySessionStates({ [SID]: { unread: true } });

    expect(SID in useTermStore.getState().dormantSessions).toBe(false);
  });
});

describe("following a peer's layout", () => {
  it("does not start sessions the peer is only showing placeholders for", () => {
    // This is the reported bug: the desktop restores a workspace without starting anything, the browser
    // follows that layout, and every session comes to life.
    useTermStore.setState({ runtimes: { [SID]: { status: "idle", alive: false } } });

    useTermStore.getState().applyMirrorLayout({
      v: 1,
      center: {
        openTabs: ["t1"],
        liveTabs: ["t1"],
        pinnedTabs: [],
        activeTabId: "t1",
        lastActiveSessionTabId: "t1",
        activeSessionId: SID,
        focusedPaneId: `p-${SID}`,
        paneTrees: { t1: leaf(SID) },
        ephemeralSessions: {},
        docTabs: {},
        browserTabs: {},
      },
      left: {
        selection: [],
        inspectTarget: null,
        collapsed: false,
        views: [],
        tabs: [],
        primaryViewId: "",
        activeViewId: "",
      },
      right: { inspectorTab: "files", collapsed: false },
    });

    expect(useTermStore.getState().dormantSessions[SID]).toBe(true);
  });
});
