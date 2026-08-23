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
  listTree: vi.fn().mockResolvedValue({ projects: [], groups: [], sessions: [] }),
}));
vi.mock("../notify", () => ({ notify: vi.fn() }));
vi.mock("../ipc/browser", () => ({ setBrowserUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock("../ipc/transport", () => ({ isTauri: true, getClientSource: () => "desktop" }));
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
  return { env, platform: { env, dialog: { pickDirectory: vi.fn(), saveFile: vi.fn() } } };
});

// `vi.mock` factories are hoisted above the file body, so everything they close over is hoisted too.
const h = vi.hoisted(() => ({
  isMobileView: vi.fn(() => false),
  mirrorGet: vi.fn(),
  mirrorPush: vi.fn(),
  /** Captured event callbacks, so a test can play the part of the backend broadcast. */
  cbs: { layout: null, mode: null } as {
    layout: ((snap: unknown) => void) | null;
    mode: ((enabled: boolean) => void) | null;
  },
}));
vi.mock("../mobile/detect", () => ({ isMobileView: h.isMobileView }));
vi.mock("../ipc/mirror", () => ({ mirrorGet: h.mirrorGet, mirrorPush: h.mirrorPush }));
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

import { markMirrorDetach } from "../ipc/commands";
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
    selection: [],
    inspectTarget: null,
    mirrorFocusSessionId: null,
  });
}

/** A published snapshot for a single tab, as another client would send it. */
function peerLayout(tab: string, rev: number, source = "ws-2"): MirrorSnapshot {
  seedSnapshotSource(tab);
  return { rev, source, state: JSON.parse(JSON.stringify(peerState)) };
}

let peerState: unknown = null;
function seedSnapshotSource(tab: string) {
  peerState = buildMirrorLayout({
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
    selection: [],
    inspectTarget: null,
    leftCollapsed: false,
    rightCollapsed: false,
    inspectorTab: "files",
  });
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
  mirrorPush.mockImplementation(() => Promise.resolve({ rev: 1, source: "desktop", state: null }));
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
  mirrorGet.mockResolvedValue({ enabled: true, rev: 0, source: "", state: null });
}

describe("startMirrorSync alignment", () => {
  it("publishes this client's arrangement when nothing is published yet", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();

    expect(useTermStore.getState().mirrorEnabled).toBe(true);
    expect(mirrorPush).toHaveBeenCalledTimes(1);
    expect((mirrorPush.mock.calls[0][0] as { center: { openTabs: string[] } }).center.openTabs).toEqual(["A"]);
    stop();
  });

  it("follows the published arrangement instead when one exists", async () => {
    mirrorGet.mockResolvedValue({ enabled: true, ...peerLayout("B", 4) });
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
    mirrorGet.mockResolvedValue({ enabled: false, rev: 0, source: "", state: null });
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
    expect((mirrorPush.mock.calls[0][0] as { center: { focusedPaneId: string } }).center.focusedPaneId).toBe("p-3");
    stop();
  });

  it("ignores state that is not part of the arrangement", async () => {
    alignEnabledEmpty();
    const stop = start();
    await settle();
    mirrorPush.mockClear();

    // Terminal width is local, and runtime status is shared by its own channel; neither is layout.
    useTermStore.setState({ leftWidth: 320, runtimes: { A: { status: "running" } } });
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
    mirrorGet.mockResolvedValue({ enabled: false, rev: 0, source: "", state: null });
    const stop = start();
    await settle();

    mirrorGet.mockResolvedValue({ enabled: true, ...peerLayout("B", 3) });
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
    mirrorGet.mockResolvedValue({ enabled: true, ...peerLayout("C", 1) });
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
