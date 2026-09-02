//! Regression coverage for the authoritative session records: the unread marker and the agent state.
//!
//! The marker used to be raised and cleared inside each client, which made "unread" mean "unread in this
//! window". Two clients then disagreed by design: reading a reply in the browser left the dot on the
//! desktop, the desktop kept judging the session a match for its unread filter, and its dynamic
//! additions pushed the filtered row straight back. These tests pin the split that fixes it — the
//! backend owns the marker, and the client owns only the decision to interrupt its own user.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { markSessionRead, markSessionUnread, reportScreen, sessionStates } = vi.hoisted(() => ({
  markSessionRead: vi.fn().mockResolvedValue(undefined),
  markSessionUnread: vi.fn().mockResolvedValue(undefined),
  reportScreen: vi.fn().mockResolvedValue(undefined),
  sessionStates: vi.fn().mockResolvedValue({}),
}));
vi.mock("../ipc/sessionState", () => ({
  markSessionRead,
  markSessionUnread,
  reportScreen,
  sessionStates,
}));

const { resolveSpawnMock } = vi.hoisted(() => ({
  resolveSpawnMock: vi.fn().mockResolvedValue(true),
}));

const { notify } = vi.hoisted(() => ({ notify: vi.fn() }));
vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
  resolveSpawn: resolveSpawnMock,
}));
vi.mock("../ipc/tree", () => ({
  listTree: vi.fn().mockResolvedValue({ projects: [], groups: [], sessions: [] }),
}));
vi.mock("../notify", () => ({
  notify,
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));

import { useTermStore } from "./termStore";
import { effectiveStatus } from "../types";
import { registerTerminal, unregisterTerminal } from "../terminal/registry";

const SID = "unread-1";

beforeEach(() => {
  markSessionRead.mockClear();
  markSessionUnread.mockClear();
  reportScreen.mockClear();
  sessionStates.mockClear();
  notify.mockClear();
  useTermStore.setState({
    notifications: {},
    runtimes: { [SID]: { status: "running", agent: "claude", authoritative: true } },
    windowFocused: false,
    notifyEnabled: true,
    activeTabId: null,
    paneTrees: {},
  });
});

describe("who owns the unread marker", () => {
  it("marks locally on the rising edge, then lets the backend confirm it", () => {
    // The backend raises the marker for every client from the same signal, arriving a moment later. This
    // client marks at the instant it pops its notification, so one result cannot interrupt the user twice
    // — once from the signal it read directly, once from the record that follows.
    useTermStore.getState().applyStatusSignal(SID, {
      kind: "state",
      state: "waiting",
      authoritative: true,
    });
    expect(SID in useTermStore.getState().notifications).toBe(true);

    useTermStore.getState().applySessionStates({ [SID]: { unread: true } });

    expect(notify).toHaveBeenCalledTimes(1);
  });

  it("still pops a system notification, because that is this window's business", () => {
    useTermStore.getState().applyStatusSignal(SID, {
      kind: "state",
      state: "waiting",
      authoritative: true,
    });

    expect(notify).toHaveBeenCalledTimes(1);
  });

  it("stays quiet when this window is already showing the session", () => {
    // Whether to interrupt depends on which window has focus here — a device fact, unlike the marker.
    useTermStore.setState({
      windowFocused: true,
      activeTabId: "t1",
      paneTrees: { t1: { kind: "leaf", paneId: "p1", sessionId: SID } },
    });

    useTermStore.getState().applyStatusSignal(SID, {
      kind: "state",
      state: "waiting",
      authoritative: true,
    });

    expect(notify).not.toHaveBeenCalled();
  });
});

describe("applying a batch of backend records", () => {
  it("raises the markers the backend reports", () => {
    useTermStore.getState().applySessionStates({ [SID]: { unread: true } });

    expect(SID in useTermStore.getState().notifications).toBe(true);
  });

  it("clears a marker the backend reports as read, wherever it was read", () => {
    useTermStore.setState({ notifications: { [SID]: 1 } });

    useTermStore.getState().applySessionStates({ [SID]: { unread: false } });

    expect(useTermStore.getState().notifications).toEqual({});
  });

  it("keeps the timestamp it already had, so a broadcast cannot restart the read delay", () => {
    // The timestamp drives the two-second delay before a viewed session counts as read. Refreshing it on
    // every broadcast would keep pushing that moment back while the user sits there reading.
    useTermStore.setState({ notifications: { [SID]: 111 } });

    useTermStore.getState().applySessionStates({ [SID]: { unread: true } });

    expect(useTermStore.getState().notifications[SID]).toBe(111);
  });

  it("leaves sessions the batch does not mention alone", () => {
    // A broadcast carries only what changed. Treating silence as "read" would wipe every other marker.
    useTermStore.setState({ notifications: { other: 5 } });

    useTermStore.getState().applySessionStates({ [SID]: { unread: true } });

    expect(useTermStore.getState().notifications.other).toBe(5);
  });
});

describe("agent state from the backend record", () => {
  it("fills in a session this client has never opened", () => {
    // The per-session status event only reaches a client that opened that session, which is why a
    // freshly connected browser used to show a dot on nothing but its own tabs. The record does not
    // depend on having opened anything.
    useTermStore.setState({ runtimes: {} });

    useTermStore.getState().applySessionStates({
      "never-opened": { agent: "claude", agentState: "working", authoritative: true },
    });

    const rt = useTermStore.getState().runtimes["never-opened"];
    expect(rt.agent).toBe("claude");
    expect(rt.agentState).toBe("working");
    expect(effectiveStatus(rt)).toBe("working");
  });

  it("answers for a session this client is displaying too", () => {
    // Everything the client used to conclude on its own — a screen only it could read, an interrupt only
    // it saw — is reported to the backend now, so the record is the answer for every session. A displayed
    // session following its own copy of the chain is exactly the disagreement this set out to end.
    // A registered xterm instance is what "this client is displaying it" means.
    const term = {} as never;
    registerTerminal(SID, term);
    useTermStore.setState({
      runtimes: { [SID]: { status: "running", agent: "crush", agentState: "asking" } },
    });

    useTermStore.getState().applySessionStates({ [SID]: { agent: "crush", agentState: "waiting" } });

    expect(useTermStore.getState().runtimes[SID].agentState).toBe("waiting");
    unregisterTerminal(SID, term);
  });

  it("carries the flags that gate the fallbacks", () => {
    useTermStore.setState({ runtimes: {} });

    useTermStore.getState().applySessionStates({
      "cx-1": {
        agent: "codex",
        stateSource: "hooks",
        hookReady: false,
        authoritative: false,
        everWorked: false,
      },
    });

    const rt = useTermStore.getState().runtimes["cx-1"];
    expect(rt.agentStateSource).toBe("hooks");
    // Codex in hook-only mode reads as unavailable until its handshake proves the hooks actually run.
    expect(effectiveStatus(rt)).toBe("unavailable");
  });
});

describe("reporting a session as read", () => {
  it("tells the backend and clears locally without waiting for the echo", () => {
    useTermStore.setState({ notifications: { [SID]: 1 } });

    useTermStore.getState().clearNotification(SID);

    expect(markSessionRead).toHaveBeenCalledWith(SID);
    expect(useTermStore.getState().notifications).toEqual({});
  });

  it("says nothing about a session that was not marked", () => {
    useTermStore.getState().clearNotification(SID);

    expect(markSessionRead).not.toHaveBeenCalled();
  });

  it("reports every marked session when clearing them all", () => {
    useTermStore.setState({ notifications: { a: 1, b: 2 } });

    useTermStore.getState().clearAllNotifications();

    expect(markSessionRead.mock.calls.map((c) => c[0]).sort()).toEqual(["a", "b"]);
    expect(useTermStore.getState().notifications).toEqual({});
  });

  it("also drops the spawn cards the Dock badge counts", () => {
    // A spawn card nobody answered keeps the badge at one while showing no dot to click, which is the
    // state the manual clear exists for. Declining the request settles it for other clients too.
    useTermStore.setState({
      notifications: { a: 1 },
      pendingSpawns: [
        { parentSessionId: "p1", prompt: "do the thing" },
      ] as never,
    });

    useTermStore.getState().clearAllBadges();

    expect(markSessionRead).toHaveBeenCalledWith("a");
    expect(resolveSpawnMock).toHaveBeenCalledWith("p1", "do the thing", false);
    expect(useTermStore.getState().notifications).toEqual({});
    expect(useTermStore.getState().pendingSpawns).toEqual([]);
  });
});

describe("screen detection", () => {
  it("reports what it saw instead of deciding what it means", () => {
    // The reading needs a laid-out grid, which exists only where a terminal is rendered. The conclusion
    // drawn from it is a fact about the session, and two clients drawing their own is what left them
    // disagreeing — so the reading is reported and the backend arbitrates.
    useTermStore.setState({
      runtimes: { [SID]: { status: "running", agent: "crush", everWorked: true } },
    });

    useTermStore.getState().applyScreenDetection(SID, {
      state: "waiting",
      visibleBlocker: false,
      visibleIdle: true,
      visibleWorking: false,
      skip: false,
    });

    expect(reportScreen).toHaveBeenCalledWith(SID, {
      state: "waiting",
      visibleBlocker: false,
      visibleWorking: false,
      skip: false,
    });
    expect(useTermStore.getState().runtimes[SID].agentState).toBeUndefined();
  });
});

describe("notifying about a conclusion only the backend reached", () => {
  it("pops one when a record turns unread with nothing heard directly", () => {
    // A state derived from a screen reading or from output activity produces no signal this client can
    // read, so the record is the only place that result ever appears.
    useTermStore.getState().applySessionStates({
      [SID]: { unread: true, agent: "crush", agentState: "asking" },
    });

    expect(notify).toHaveBeenCalledTimes(1);
  });

  it("stays quiet for a record that was already marked here", () => {
    useTermStore.setState({ notifications: { [SID]: 1 } });

    useTermStore.getState().applySessionStates({ [SID]: { unread: true } });

    expect(notify).not.toHaveBeenCalled();
  });
});

describe("the arbitration escape hatch", () => {
  // Agent state is the product's core signal and every rule in the chain was added because something went
  // wrong without it, so the old client-side path stays reachable for a release or two.
  afterEach(() => localStorage.removeItem("vlx-arbitration"));

  it("decides locally and leaves the displayed session's own answer standing", () => {
    localStorage.setItem("vlx-arbitration", "frontend");
    const term = {} as never;
    registerTerminal(SID, term);
    useTermStore.setState({
      runtimes: { [SID]: { status: "running", agent: "crush", agentState: "asking" } },
    });

    useTermStore.getState().applyScreenDetection(SID, {
      state: "working",
      visibleBlocker: false,
      visibleIdle: false,
      visibleWorking: true,
      skip: false,
    });
    useTermStore.getState().applySessionStates({ [SID]: { agent: "crush", agentState: "waiting" } });

    expect(reportScreen).not.toHaveBeenCalled();
    expect(useTermStore.getState().runtimes[SID].agentState).toBe("working");
    unregisterTerminal(SID, term);
  });
});

describe("reading every record at once", () => {
  it("merges what the backend returns", async () => {
    // Broadcasts are not replayed after a reconnect, so only a batch read closes that gap — and it is
    // the only way a client learns about sessions it has never opened.
    sessionStates.mockResolvedValueOnce({ [SID]: { unread: true } });

    await useTermStore.getState().syncSessionStates();

    expect(SID in useTermStore.getState().notifications).toBe(true);
  });

  it("leaves local markers untouched when the backend cannot answer", async () => {
    useTermStore.setState({ notifications: { [SID]: 1 } });
    sessionStates.mockRejectedValueOnce(new Error("offline"));

    await useTermStore.getState().syncSessionStates();

    expect(useTermStore.getState().notifications[SID]).toBe(1);
  });
});
