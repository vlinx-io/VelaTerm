//! Mirror-mode sync tests: publishing local rearrangements, following a peer's, and the three rules that
//! keep two-way sync from fighting itself — echo dropped, out-of-order frames dropped, apply never
//! bouncing back out as a fresh push. Also covers phones opting out and mirrored removals detaching
//! rather than killing.
//!
//! Stub the Tauri-touching modules so the store loads under jsdom, as in browserTabs.test.ts.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MirrorSnapshot, MirrorStatus } from "../ipc/mirror";

vi.mock("../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
  markMirrorDetach: vi.fn(),
}));
vi.mock("../ipc/tree", () => ({
  listTree: vi
    .fn()
    .mockResolvedValue({ projects: [], groups: [], sessions: [] }),
}));
vi.mock("../notify", () => ({ notify: vi.fn() }));
vi.mock("../ipc/browser", () => ({
  setBrowserUrl: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../ipc/transport", () => ({
  isTauri: true,
  getClientSource: () => "desktop",
}));
vi.mock("../platform", () => {
  const env = {
    kind: "tauri",
    isTauri: true,
    isElectron: false,
    isBrowser: false,
    isRemoteWindow: false,
    hasNativeHost: true,
    isMac: false,
  };
  return {
    env,
    platform: { env, dialog: { pickDirectory: vi.fn(), saveFile: vi.fn() } },
  };
});

// `vi.mock` factories are hoisted above the file body, so everything they close over is hoisted too.
const h = vi.hoisted(() => ({
  isMobileView: vi.fn(() => false),
  mirrorGet: vi.fn(),
  mirrorPush: vi.fn(),
  /** Captured event callbacks, so a test can play the part of the backend broadcast. */
  cbs: { layout: null, mode: null, clients: null } as {
    layout: ((snap: unknown) => void) | null;
    mode: ((enabled: boolean) => void) | null;
    clients: ((count: number, clients: unknown[]) => void) | null;
  },
}));
vi.mock("../mobile/detect", () => ({ isMobileView: h.isMobileView }));
vi.mock("../ipc/mirror", () => ({
  mirrorGet: h.mirrorGet,
  mirrorPush: h.mirrorPush,
}));
vi.mock("../ipc/events", () => ({
  onMirrorLayout: (cb: (snap: unknown) => void) => {
    h.cbs.layout = cb;
    return Promise.resolve(() => {
      h.cbs.layout = null;
    });
  },
  onMirrorMode: (cb: (enabled: boolean) => void) => {
    h.cbs.mode = cb;
    return Promise.resolve(() => {
      h.cbs.mode = null;
    });
  },
  onRemoteClients: (cb: (count: number, clients: unknown[]) => void) => {
    h.cbs.clients = cb;
    return Promise.resolve(() => {
      h.cbs.clients = null;
    });
  },
}));

const mobile = { isMobileView: h.isMobileView };
const mirrorGet = h.mirrorGet as unknown as {
  mockReset: () => void;
  mockResolvedValue: (v: MirrorStatus) => void;
} & ((...a: unknown[]) => Promise<MirrorStatus>);
const mirrorPush = h.mirrorPush as unknown as ReturnType<typeof vi.fn>;
/** Emit a broadcast as the backend would; the sync module registers these on start. */
const emitLayout = (snap: MirrorSnapshot) => h.cbs.layout!(snap);
const emitMode = (enabled: boolean) => h.cbs.mode!(enabled);
const emitClients = (count: number, clients: unknown[] = []) =>
  h.cbs.clients!(count, clients);

import { markMirrorDetach } from "../ipc/commands";
import { listTree } from "../ipc/tree";
import { buildMirrorLayout } from "./mirrorLayout";
import { startMirrorSync } from "./mirrorSync";
import { useTermStore } from "./termStore";

const leaf = (paneId: string, sessionId: string) => ({
  kind: "leaf" as const,
  paneId,
  sessionId,
});

/** Put the store into a known one-tab arrangement without going through any action. */
function seed(tab: string) {
  useTermStore.setState({
    openTabs: [tab],
    liveTabs: [],
    pinnedTabs: [],
    activeTabId: tab,
    lastActiveSessionTabId: tab,
    activeSessionId: tab,
    focusedPaneId: `p-${tab}`,
    paneTrees: { [tab]: leaf(`p-${tab}`, tab) },
    ephemeralSessions: {},
    docTabs: {},
    browserTabs: {},
    taskTabs: {},
    selection: [],
    inspectTarget: null,
    mirrorFocusSessionId: null,
  });
}

/** Open a task tab for `sessionId` the way the Tasks chip does and return its id; the store sits in it afterwards. */
function openTaskTab(sessionId: string) {
  useTermStore.getState().openTaskTab(sessionId, { task_id: "t1", task_type: "local_workflow", description: "probe", status: "running" });
  return useTermStore.getState().activeTabId!;
}

/** A published snapshot for a single tab, as another client would send it. */
function peerLayout(tab: string, rev: number, source = "ws-2"): MirrorSnapshot {
  seedSnapshotSource(tab);
  return { rev, source, state: JSON.parse(JSON.stringify(peerState)) };
}

let peerState: unknown = null;
function seedSnapshotSource(tab: string) {
  peerState = buildMirrorLayout(snapshotSource(tab));
}

/** The store fields another client would build its single-tab snapshot from. */
function snapshotSource(tab: string): Parameters<typeof buildMirrorLayout>[0] {
  return {
    openTabs: [tab],
    liveTabs: [],
    pinnedTabs: [],
    activeTabId: tab,
    lastActiveSessionTabId: tab,
    activeSessionId: tab,
    focusedPaneId: `p-${tab}`,
    paneTrees: { [tab]: leaf(`p-${tab}`, tab) },
    ephemeralSessions: {},
    docTabs: {},
    browserTabs: {},
    taskTabs: {},
    selection: [],
    inspectTarget: null,
    leftCollapsed: false,
    rightCollapsed: false,
    inspectorTab: "files",
    sidebarTreeViews: [
      {
        id: "main",
        name: "main",
        treeFilter: "",
        statusFilter: null,
        statusFilterIds: null,
        markFilter: null,
        collapsedOverrides: null,
      },
    ],
    sidebarTreeTabs: [
      {
        id: "t1",
        root: { kind: "leaf", paneId: "sp-main", viewId: "main" },
        activeViewId: "main",
      },
    ],
    primarySidebarTreeViewId: "main",
    activeSidebarTreeViewId: "main",
  };
}

/** Let the `mirrorGet` promise chain in `align` settle. */
const settle = () => Promise.resolve().then(() => Promise.resolve());

/** Every sync started by a test, disposed in afterEach so a failed assertion cannot leak a live one. */
const started: Array<() => void> = [];
function start() {
  const stop = startMirrorSync();
  started.push(stop);
  return stop;
}

beforeEach(() => {
  vi.useFakeTimers();
  mirrorGet.mockReset();
  mirrorPush.mockReset();
  mirrorPush.mockImplementation(() =>
    Promise.resolve({ rev: 1, source: "desktop", state: null }),
  );
  vi.mocked(markMirrorDetach).mockClear();
  mobile.isMobileView.mockReturnValue(false);
  h.cbs.layout = null;
  h.cbs.mode = null;
  seed("A");
  useTermStore.setState({ mirrorEnabled: false });
});

afterEach(() => {
  for (const stop of started.splice(0)) stop();
  vi.useRealTimers();
});

/** Mirror on, nothing published yet. */
function alignEnabledEmpty() {
  mirrorGet.mockResolvedValue({
    clients: 0,
    enabled: true,
    rev: 0,
    source: "",
    state: null,
  });
}

describe("startMirrorSync alignment", () => {
  it("publishes this client's arrangement when nothing is published yet", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();

    expect(useTermStore.getState().mirrorEnabled).toBe(true);
    expect(mirrorPush).toHaveBeenCalledTimes(1);
    expect(
      (mirrorPush.mock.calls[0][0] as { center: { openTabs: string[] } }).center
        .openTabs,
    ).toEqual(["A"]);
    stop();
  });

  it("follows the published arrangement instead when one exists", async () => {
    mirrorGet.mockResolvedValue({
      clients: 0,
      enabled: true,
      ...peerLayout("B", 4),
    });
    const stop = start();
    await settle();

    expect(useTermStore.getState().openTabs).toEqual(["B"]);
    expect(mirrorPush).not.toHaveBeenCalled();
    stop();
  });

  it("stays out of it entirely on a phone", async () => {
    mobile.isMobileView.mockReturnValue(true);
    const stop = start();
    await settle();

    expect(mirrorGet).not.toHaveBeenCalled();
    expect(h.cbs.layout).toBeNull();
    stop();
  });

  it("neither follows nor publishes while mirror mode is off", async () => {
    mirrorGet.mockResolvedValue({
      clients: 0,
      enabled: false,
      rev: 0,
      source: "",
      state: null,
    });
    const stop = start();
    await settle();
    useTermStore.setState({ activeTabId: "Z" });
    vi.advanceTimersByTime(500);

    expect(mirrorPush).not.toHaveBeenCalled();
    stop();
  });
});

describe("publishing local changes", () => {
  it("coalesces a burst of local edits into one push", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();
    mirrorPush.mockClear();

    useTermStore.setState({ activeTabId: "A", focusedPaneId: "p-1" });
    useTermStore.setState({ focusedPaneId: "p-2" });
    useTermStore.setState({ focusedPaneId: "p-3" });
    vi.advanceTimersByTime(500);

    expect(mirrorPush).toHaveBeenCalledTimes(1);
    expect(
      (mirrorPush.mock.calls[0][0] as { center: { focusedPaneId: string } })
        .center.focusedPaneId,
    ).toBe("p-3");
    stop();
  });

  it("ignores state that is not part of the arrangement", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();
    mirrorPush.mockClear();

    // Terminal width is local, and runtime status is shared by its own channel; neither is layout.
    useTermStore.setState({
      leftWidth: 320,
      runtimes: { A: { status: "running" } },
    });
    vi.advanceTimersByTime(500);

    expect(mirrorPush).not.toHaveBeenCalled();
    stop();
  });

  it("retries after a failed push instead of assuming it was published", async () => {
    alignEnabledEmpty();
    mirrorPush.mockRejectedValueOnce(new Error("socket down"));
    const stop = start();
    await settle();
    await settle();
    mirrorPush.mockClear();

    // Any further local edit must be sent, not deduplicated against an attempt that never landed.
    useTermStore.setState({ focusedPaneId: "p-A2" });
    vi.advanceTimersByTime(500);

    expect(mirrorPush).toHaveBeenCalledTimes(1);
    stop();
  });
});

describe("following a peer", () => {
  it("adopts a peer's arrangement without pushing it straight back", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();
    mirrorPush.mockClear();

    emitLayout(peerLayout("B", 2));
    vi.advanceTimersByTime(500);

    expect(useTermStore.getState().openTabs).toEqual(["B"]);
    expect(useTermStore.getState().activeSessionId).toBe("B");
    expect(mirrorPush).not.toHaveBeenCalled();
    stop();
  });

  it("keeps a publisher's task tab in front: its snapshot names a session tab, so reconciling echoes nothing", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();
    mirrorPush.mockClear();

    // The peer sits in a task tab, which it never publishes. Its snapshot must still name an active tab,
    // or this client shows an empty stage and its next reconcile publishes its first tab back, pulling the
    // peer out of the task tab.
    // The publisher's real store state in front of a task tab: no active session and no focused pane
    // (`openTaskTab`/`setActiveTab` null both). Those must not travel as null either, or the reconcile
    // below would repair them to the first leaf and push that repair back.
    const taskTab = { id: "task-1", sessionId: "B", taskId: "t1", title: "probe", taskType: "local_workflow" };
    const published = buildMirrorLayout({
      ...snapshotSource("B"),
      openTabs: ["B", "task-1"],
      activeTabId: "task-1",
      activeSessionId: null,
      focusedPaneId: null,
      taskTabs: { "task-1": taskTab },
    });
    emitLayout({ rev: 2, source: "ws-2", state: JSON.parse(JSON.stringify(published)) });
    expect(useTermStore.getState().openTabs).toEqual(["B"]);
    expect(useTermStore.getState().activeTabId).toBe("B");
    expect(useTermStore.getState().activeSessionId).toBe("B");
    expect(useTermStore.getState().focusedPaneId).toBe("p-B");

    // A tree refresh reconciles the adopted arrangement with the real `reconcileTabs`.
    vi.mocked(listTree).mockResolvedValueOnce({
      projects: [{ id: "p1", name: "p1", rootPath: "/tmp", sortOrder: 0, collapsed: false, createdAt: 0 }],
      groups: [],
      sessions: [{ id: "B", projectId: "p1", groupId: null, name: "B", kind: "terminal", sortOrder: 0, collapsed: false, createdAt: 0 }],
    });
    await useTermStore.getState().loadTree();
    vi.advanceTimersByTime(500);

    expect(useTermStore.getState().activeTabId).toBe("B");
    expect(useTermStore.getState().activeSessionId).toBe("B");
    expect(useTermStore.getState().focusedPaneId).toBe("p-B");
    expect(mirrorPush).not.toHaveBeenCalled();
    stop();
  });

  it("stays in its own task tab as a publisher: neither the reconcile echo nor a peer's push names it", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();
    mirrorPush.mockClear();

    const taskTabId = openTaskTab("A");
    vi.advanceTimersByTime(500);
    await settle();
    expect(mirrorPush).toHaveBeenCalledTimes(1);
    const published = mirrorPush.mock.calls[0][0] as { center: { activeTabId: string | null } };
    expect(published.center.activeTabId).toBe("A");

    // The peer adopted our snapshot and echoes it back: the anchor A is what it now names as active.
    emitLayout({ rev: 2, source: "ws-2", state: JSON.parse(JSON.stringify(published)) });
    expect(useTermStore.getState().activeTabId).toBe(taskTabId);
    expect(useTermStore.getState().activeSessionId).toBeNull();
    expect(useTermStore.getState().focusedPaneId).toBeNull();
    expect(useTermStore.getState().openTabs).toEqual(["A", taskTabId]);

    // An ordinary peer push (the peer clicked a sidebar row, resized a split): the arrangement follows,
    // the task tab stays in front.
    emitLayout(peerLayout("B", 3));
    expect(useTermStore.getState().openTabs).toEqual(["B", taskTabId]);
    expect(useTermStore.getState().activeTabId).toBe(taskTabId);
    expect(useTermStore.getState().activeSessionId).toBeNull();
    expect(useTermStore.getState().focusedPaneId).toBeNull();
    expect(useTermStore.getState().lastActiveSessionTabId).toBe("B");
    expect(useTermStore.getState().mirrorFocusSessionId).toBeNull();
    stop();
  });

  it("stays in its own task tab as a follower while the publisher keeps pushing", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();
    emitLayout(peerLayout("B", 2));
    expect(useTermStore.getState().activeTabId).toBe("B");
    mirrorPush.mockClear();

    const taskTabId = openTaskTab("B");
    vi.advanceTimersByTime(500);
    await settle();

    emitLayout(peerLayout("B", 3));
    expect(useTermStore.getState().activeTabId).toBe(taskTabId);
    expect(useTermStore.getState().activeSessionId).toBeNull();
    expect(useTermStore.getState().focusedPaneId).toBeNull();
    expect(useTermStore.getState().openTabs).toEqual(["B", taskTabId]);

    // Leaving the task tab hands the active trio back to the mirror.
    useTermStore.getState().setActiveTab("B");
    vi.advanceTimersByTime(500);
    await settle();
    emitLayout(peerLayout("C", 4));
    expect(useTermStore.getState().activeTabId).toBe("C");
    expect(useTermStore.getState().activeSessionId).toBe("C");
    expect(useTermStore.getState().focusedPaneId).toBe("p-C");
    stop();
  });

  it("drops its own broadcast", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();

    emitLayout(peerLayout("B", 5, "desktop"));

    expect(useTermStore.getState().openTabs).toEqual(["A"]);
    stop();
  });

  it("drops a frame older than one already applied", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();

    emitLayout(peerLayout("B", 7));
    emitLayout(peerLayout("C", 6));

    expect(useTermStore.getState().openTabs).toEqual(["B"]);
    stop();
  });

  it("marks the session a peer activated, so its terminal view can skip one automatic focus", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();
    expect(useTermStore.getState().mirrorFocusSessionId).toBe(null);

    emitLayout(peerLayout("B", 2));

    expect(useTermStore.getState().mirrorFocusSessionId).toBe("B");
    stop();
  });

  it("leaves a consumed focus marker alone while a peer keeps publishing the same active session", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();

    emitLayout(peerLayout("B", 2));
    // The terminal view consumes the marker when it skips its focus.
    useTermStore.setState({ mirrorFocusSessionId: null });

    // A peer dragging a divider republishes the same active session every 150 ms. Re-marking here would
    // suppress this window's own tab switches for as long as the drag lasts.
    emitLayout(peerLayout("B", 3));

    expect(useTermStore.getState().mirrorFocusSessionId).toBe(null);
    stop();
  });

  it("marks sessions leaving the layout as detaching, so following a peer never kills a process", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();

    emitLayout(peerLayout("B", 2));

    expect(markMirrorDetach).toHaveBeenCalledWith(["A"]);
    stop();
  });

  it("realigns when the host switches mirror mode back on", async () => {
    mirrorGet.mockResolvedValue({
      clients: 0,
      enabled: false,
      rev: 0,
      source: "",
      state: null,
    });
    const stop = start();
    await settle();

    mirrorGet.mockResolvedValue({
      clients: 0,
      enabled: true,
      ...peerLayout("B", 3),
    });
    emitMode(true);
    await settle();

    expect(useTermStore.getState().mirrorEnabled).toBe(true);
    expect(useTermStore.getState().openTabs).toEqual(["B"]);
    stop();
  });

  it("accepts a lower revision after realigning, so a restarted service still converges", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();
    emitLayout(peerLayout("B", 9));
    expect(useTermStore.getState().openTabs).toEqual(["B"]);

    // The service restarted (or mirror mode was cycled), so its counter is back near zero. A client
    // holding on to the old high-water mark would dismiss everything that follows as stale.
    emitMode(false);
    mirrorGet.mockResolvedValue({
      clients: 0,
      enabled: true,
      ...peerLayout("C", 1),
    });
    emitMode(true);
    await settle();

    expect(useTermStore.getState().openTabs).toEqual(["C"]);
    stop();
  });

  it("stops following once disposed", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();
    const cb = h.cbs.layout!;
    stop();

    cb(peerLayout("B", 9));

    expect(useTermStore.getState().openTabs).toEqual(["A"]);
  });
});

describe("attached client count", () => {
  it("takes the count from alignment and then follows the broadcast", async () => {
    // Alignment is the only place a client that just started learns the count: the broadcast fires on
    // the next connect or disconnect, which on a quiet host may never come.
    mirrorGet.mockResolvedValue({
      clients: 2,
      enabled: true,
      rev: 0,
      source: "",
      state: null,
    });
    const stop = start();
    await settle();
    expect(useTermStore.getState().remoteClients).toBe(2);

    emitClients(1);
    expect(useTermStore.getState().remoteClients).toBe(1);

    emitClients(0);
    expect(useTermStore.getState().remoteClients).toBe(0);
    stop();
  });

  it("keeps the attached clients themselves, not just how many", async () => {
    // The badge names who is attached, so the list has to survive both the alignment reply and the
    // broadcast; a count alone would leave the host unable to tell one peer from another.
    const phone = {
      source: "ws-1",
      name: "iOS · Safari",
      deviceId: "d1",
      ip: "10.0.0.5",
      since: 100,
    };
    mirrorGet.mockResolvedValue({
      clients: 1,
      clientList: [phone],
      enabled: true,
      rev: 0,
      source: "",
      state: null,
    });
    const stop = start();
    await settle();
    expect(useTermStore.getState().remoteClientList).toEqual([phone]);

    emitClients(0, []);
    expect(useTermStore.getState().remoteClientList).toEqual([]);
    stop();
  });

  it("still reports the count while mirroring is off", async () => {
    // The host badge is gated on mirroring, but the count itself is not: switching mirroring back on must
    // not have to wait for the next connect before the badge can appear.
    mirrorGet.mockResolvedValue({
      clients: 3,
      enabled: false,
      rev: 0,
      source: "",
      state: null,
    });
    const stop = start();
    await settle();

    expect(useTermStore.getState().remoteClients).toBe(3);
    stop();
  });
});
