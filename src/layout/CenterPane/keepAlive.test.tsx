//! Headless keep-alive regression test: reproduces the Open A -> Open B -> Return to A sequence and asserts
//! that returning to a previously used session does **not unmount its terminal view**. Unmounting would call
//! ptyKill, terminate the process, and show the starting state again.
//!
//! TerminalView is stubbed to record only mounts and unmounts. React reconciliation determines that lifecycle,
//! independently of TerminalView's internal rendering, so the stub faithfully answers whether it remounts.
//! The Tauri-facing ipc/notify modules are also stubbed so the store can load under Node/jsdom.

import * as React from "react";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

// Record mount and unmount events per session; this is hoisted above the mock factory.
const { lifecycle } = vi.hoisted(() => ({ lifecycle: [] as string[] }));

vi.mock("./TerminalView", () => ({
  TerminalView: ({ session }: { session: { id: string } }) => {
    React.useEffect(() => {
      lifecycle.push(`mount:${session.id}`);
      return () => {
        lifecycle.push(`unmount:${session.id}`);
      };
    }, []);
    return React.createElement("div", { "data-testid": `tv-${session.id}` });
  },
}));
vi.mock("./SearchBar", () => ({ SearchBar: () => null }));
// TabBar is unrelated to keep-alive and uses scrollIntoView, which jsdom lacks; stub it to isolate CenterPane.
vi.mock("./TabBar", () => ({ TabBar: () => null }));
vi.mock("../../ipc/commands", () => ({
  createWorktree: vi.fn(),
  getSessionCwd: vi.fn().mockResolvedValue(null),
  ptyKill: vi.fn().mockResolvedValue(undefined),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../../ipc/tree", () => {
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
      sessions: [s("A"), s("B"), s("C")],
    }),
  };
});
// Provide notify's complete export surface. Platform adapters pulled in by termStore reference every
// permission helper, so stubbing notify() alone would fail at load time with missing exports.
vi.mock("../../notify", () => ({
  notify: vi.fn(),
  getNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestNotifyPermission: vi.fn().mockResolvedValue("granted"),
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("granted"),
}));

import type { Session } from "../../types";
import { t } from "../../i18n";
import { useTermStore } from "../../store/termStore";
import { collectSessionIds } from "./paneTree";
import { CenterPane } from "./CenterPane";

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

/** Resets the store to project p1, named sessions A/B, an empty center pane, and the requested tab mode. */
function seed(singleTabMode: boolean) {
  useTermStore.setState({
    projects: [
      { id: "p1", name: "P", rootPath: "/tmp", color: null, sortOrder: 0, collapsed: false, createdAt: 0 },
    ],
    groups: [],
    sessions: [mkSession("A"), mkSession("B"), mkSession("C")],
    runtimes: { A: { status: "idle" }, B: { status: "idle" }, C: { status: "idle" } },
    epochs: {},
    ephemeralSessions: {},
    pendingPrompts: {},
    openTabs: [],
    activeTabId: null,
    paneTrees: {},
    activeSessionId: null,
    focusedPaneId: null,
    liveTabs: [],
    notifications: {},
    singleTabMode,
  });
}

/** Seeds eviction-limit cases with single-tab mode and n named idle sessions, S1 through Sn. */
const ids18 = Array.from({ length: 18 }, (_, i) => `S${i + 1}`);
function seedMany(n: number) {
  const ids = ids18.slice(0, n);
  useTermStore.setState({
    projects: [
      { id: "p1", name: "P", rootPath: "/tmp", color: null, sortOrder: 0, collapsed: false, createdAt: 0 },
    ],
    groups: [],
    sessions: ids.map(mkSession),
    runtimes: Object.fromEntries(ids.map((id) => [id, { status: "idle" as const }])),
    epochs: {},
    ephemeralSessions: {},
    pendingPrompts: {},
    openTabs: [],
    activeTabId: null,
    paneTrees: {},
    activeSessionId: null,
    focusedPaneId: null,
    liveTabs: [],
    liveEvictNotice: null,
    liveEvictAsk: false,
    notifications: {},
    singleTabMode: true,
    // Pin the limit at 16 independently of DEFAULT_MAX_LIVE_TABS. These cases test eviction at a limit of
    // 16 and should not break merely because the production default changes.
    maxLiveTabs: 16,
  });
}

const open = (id: string) =>
  act(() => {
    useTermStore.getState().openSession(id);
  });

const openNewTab = (id: string) =>
  act(() => {
    useTermStore.getState().openSession(id, { newTab: true });
  });

const countMounts = (id: string) => lifecycle.filter((e) => e === `mount:${id}`).length;
const countUnmounts = (id: string) => lifecycle.filter((e) => e === `unmount:${id}`).length;

afterEach(() => {
  cleanup();
  lifecycle.length = 0;
  localStorage.clear();
  useTermStore.setState({ scopeTabsToProject: false, activeProjectId: null });
});

describe("keep-alive invariants when switching sessions in the centre pane", () => {
  it("single-tab mode: A to B and back to A never unmounts A, which mounts exactly once", () => {
    seed(true);
    beforeEachRender();

    open("A");
    open("B");
    open("A"); // Return to the previously used A.

    expect(countUnmounts("A")).toBe(0); // No remount means the process was not terminated.
    expect(countMounts("A")).toBe(1);
    expect(countUnmounts("B")).toBe(0);
    expect(countMounts("B")).toBe(1);
  });

  it("single-tab mode: with three sessions, A to B to C and back to A keeps A alive throughout", () => {
    seed(true);
    beforeEachRender();

    open("A");
    open("B");
    open("C");
    open("A");

    expect(countUnmounts("A")).toBe(0);
    expect(countMounts("A")).toBe(1);
  });

  it("multi-tab mode: A to B and back to A does not unmount A either", () => {
    seed(false);
    beforeEachRender();

    open("A");
    open("B");
    open("A");

    expect(countUnmounts("A")).toBe(0);
    expect(countMounts("A")).toBe(1);
  });

  // Regression: in single-tab mode, "Open in New Tab" creates a pinned tab. Even while it is active,
  // subsequently selected sessions replace only the unpinned primary slot and leave the pinned tab intact.
  it("single-tab mode: clicking a session while a pinned tab is active replaces only the main slot, leaving the pinned tab alone", () => {
    seed(true);
    beforeEachRender();

    open("A"); // A is the unpinned primary slot.
    openNewTab("B"); // B is pinned by Open in New Tab and becomes active.
    expect(useTermStore.getState().pinnedTabs).toContain("B");

    open("C"); // Selecting C while B is active reuses A's primary slot and leaves B untouched.

    const st = useTermStore.getState();
    expect(st.openTabs).toContain("B"); // Pinned B remains visible.
    expect(countUnmounts("B")).toBe(0); // B stays mounted and its process survives.
    expect(st.openTabs).toContain("C"); // C occupies the visible primary slot.
    expect(st.openTabs).not.toContain("A"); // C replaces A in the tab bar.
    expect(st.liveTabs).toContain("A"); // A moves to background keep-alive.
    expect(countUnmounts("A")).toBe(0); // A also remains mounted.
  });

  // Regression: in single-tab mode, creating or opening a terminal must not push a running agent session
  // into the background. The terminal gets a separate pinned tab while the agent's primary slot stays visible.
  it("single-tab mode: opening a terminal session does not displace a running agent; the terminal opens its own pinned tab", () => {
    seed(true);
    // Add terminal session T; seed creates A/B/C as Claude agents by default.
    useTermStore.setState((s) => ({
      sessions: [...s.sessions, { ...mkSession("T"), kind: "terminal" }],
      runtimes: { ...s.runtimes, T: { status: "idle" } },
    }));
    beforeEachRender();

    open("A"); // A is the running agent's unpinned primary slot.
    open("T"); // Open terminal session T.

    const st = useTermStore.getState();
    expect(st.openTabs).toContain("A"); // Agent A remains visible.
    expect(st.liveTabs).not.toContain("A"); // A does not move to background keep-alive.
    expect(countUnmounts("A")).toBe(0); // A stays mounted and its process survives.
    expect(st.openTabs).toContain("T"); // Terminal T is visible.
    expect(st.pinnedTabs).toContain("T"); // T is pinned and does not compete for the primary slot.
    expect(st.activeTabId).toBe("T"); // The newly opened terminal receives focus.
  });

  // Control: the terminal exception must not affect single-tab reuse between agents. Opening another agent
  // still reuses the primary slot and moves the previous one to background keep-alive.
  it("single-tab mode: opening an agent session still reuses the main slot, so only agents displace each other", () => {
    seed(true);
    beforeEachRender();

    open("A"); // Agent A occupies the primary slot.
    open("B"); // Agent B reuses it, moving A to background keep-alive.

    const st = useTermStore.getState();
    expect(st.openTabs).toContain("B");
    expect(st.openTabs).not.toContain("A");
    expect(st.liveTabs).toContain("A"); // A remains alive in the background.
    expect(countUnmounts("A")).toBe(0);
  });

  // Regression: leaving and returning to a split tab must restore its entire pane tree, including temporary
  // panes. The old implementation flattened background tabs into a session list and lost the split layout.
  it("single-tab mode: after splitting, switching away and back restores the split unchanged", async () => {
    seed(true);
    beforeEachRender();

    open("A");
    // Split A horizontally into A | temporary terminal.
    await act(async () => {
      await useTermStore.getState().splitNew("horizontal");
    });
    const splitTree = useTermStore.getState().paneTrees["A"];
    expect(splitTree.kind).toBe("split"); // Confirm that the split was created.
    const ephId = collectSessionIds(splitTree).find((id) => id.startsWith("eph-"));
    expect(ephId).toBeTruthy();

    open("B"); // Move A's entire split tree to background keep-alive.
    expect(useTermStore.getState().liveTabs).toContain("A");

    open("A"); // Return to A.
    const restored = useTermStore.getState().paneTrees["A"];
    expect(restored.kind).toBe("split"); // The split is restored unchanged.
    expect(collectSessionIds(restored)).toContain(ephId!); // The temporary pane is still present.
    expect(countUnmounts(ephId!)).toBe(0); // It remained mounted and its process survived.
  });

  it("control case: closing a tab really does unmount it, confirming the probe can detect an unmount", () => {
    seed(true);
    beforeEachRender();

    open("A");
    expect(countMounts("A")).toBe(1);
    act(() => {
      useTermStore.getState().closeTab("A");
    });
    expect(countUnmounts("A")).toBe(1); // Closing the tab ends the session and unmounts it.
  });

  // Regression: exceeding MAX_LIVE_TABS=16 automatically ends the oldest inactive background tab without a
  // dialog and records a liveEvictNotice for the status bar.
  it("single-tab mode: exceeding the background keep-alive limit ends the oldest inactive tab and records a status-bar notice", () => {
    seedMany(18);
    beforeEachRender();

    // Open 18 sessions in sequence, moving each previous one into the background. Opening S18 creates 17
    // background tabs, exceeding the limit of 16; because all are idle, the oldest (S1) is ended.
    for (const id of ids18) open(id);

    const { liveTabs, liveEvictNotice } = useTermStore.getState();
    expect(liveTabs.length).toBe(16);
    expect(liveTabs).not.toContain("S1");
    expect(countUnmounts("S1")).toBe(1); // Eviction unmounts it and ends the desktop process.
    expect(countUnmounts("S2")).toBe(0); // Tabs within the limit remain alive.
    expect(countUnmounts("S18")).toBe(0); // The visible tab is unaffected.
    expect(liveEvictNotice?.label).toBe("S1"); // A status-bar notice was recorded.
  });

  // Regression: limit eviction skips active background tabs. Sessions that are working or awaiting user
  // input (asking or carrying an unread notification) survive, and the oldest idle tab is selected instead.
  it("single-tab mode: eviction skips tabs that are working or awaiting a reply and ends the oldest idle tab instead", () => {
    seedMany(18);
    useTermStore.setState({
      runtimes: {
        ...useTermStore.getState().runtimes,
        S1: { status: "running", agent: "claude", agentState: "working" },
        S2: { status: "running", agent: "claude", agentState: "asking" },
      },
    });
    beforeEachRender();

    // Open S1 through S17, producing exactly 16 background tabs without exceeding the limit.
    for (const id of ids18.slice(0, 17)) open(id);
    // Mark S3 unread after it enters the background, because opening a session clears its notification.
    act(() => {
      useTermStore.setState({ notifications: { S3: Date.now() } });
    });
    // Open S18 to exceed the limit and trigger eviction.
    open("S18");

    const { liveTabs } = useTermStore.getState();
    expect(liveTabs).toContain("S1"); // Skip: working.
    expect(liveTabs).toContain("S2"); // Skip: awaiting confirmation.
    expect(liveTabs).toContain("S3"); // Skip: unread notification.
    expect(liveTabs).not.toContain("S4"); // Evict the oldest idle tab.
    expect(countUnmounts("S4")).toBe(1);
    expect(countUnmounts("S1")).toBe(0);
    expect(countUnmounts("S2")).toBe(0);
    expect(countUnmounts("S3")).toBe(0);
  });

  // Regression: when the limit is exceeded and **all** background tabs are active, do not end one
  // automatically. Ask whether to end the oldest and keep every tab alive until the user confirms.
  it("single-tab mode: over the limit with every tab active, a dialog asks first and confirming ends the oldest", () => {
    seedMany(18);
    useTermStore.setState({
      runtimes: Object.fromEntries(
        ids18.map((id) => [
          id,
          { status: "running" as const, agent: "claude" as const, agentState: "working" as const },
        ]),
      ),
    });
    beforeEachRender();

    for (const id of ids18) open(id);

    // All are active: preserve the over-limit set and show the confirmation dialog.
    expect(useTermStore.getState().liveTabs.length).toBe(17);
    for (const id of ids18) expect(countUnmounts(id)).toBe(0);
    // Resolve text through i18n; jsdom defaults to English, and hard-coded text would vary with detection.
    const killBtn = screen.getByText(t("overlimit.kill"));

    // Confirming ends and unmounts the oldest tab, S1, then dismisses the dialog.
    act(() => {
      fireEvent.click(killBtn);
    });
    const { liveTabs } = useTermStore.getState();
    expect(liveTabs.length).toBe(16);
    expect(liveTabs).not.toContain("S1");
    expect(countUnmounts("S1")).toBe(1);
    expect(screen.queryByText(t("overlimit.kill"))).toBeNull();
  });

  it("single-tab mode: declining that dialog keeps the over-limit state and ends nothing", () => {
    seedMany(18);
    useTermStore.setState({
      runtimes: Object.fromEntries(
        ids18.map((id) => [
          id,
          { status: "running" as const, agent: "claude" as const, agentState: "working" as const },
        ]),
      ),
    });
    beforeEachRender();

    for (const id of ids18) open(id);

    act(() => {
      fireEvent.click(screen.getByText(t("overlimit.keep")));
    });
    expect(useTermStore.getState().liveTabs.length).toBe(17); // Preserve the over-limit set.
    for (const id of ids18) expect(countUnmounts(id)).toBe(0); // No session is ended.
    expect(screen.queryByText(t("overlimit.kill"))).toBeNull(); // The dialog is dismissed.
  });

  // Regression: after a tab moves into liveTabs, a tree-change loadTree call must only reconcile state and
  // preserve the background list. It must never reapply the localStorage layout and call collapseToSingleTab,
  // which would clear liveTabs and terminate background sessions.
  it("regression: a backgrounded keep-alive session is not unmounted by loadTree when the tree changes", async () => {
    seed(true);
    beforeEachRender();

    // The first loadTree represents startup restoration. Empty localStorage makes it a no-op except for
    // setting layoutRestored.
    await act(async () => {
      await useTermStore.getState().loadTree();
    });

    open("A");
    open("B"); // Reuse the current tab and move A into liveTabs.
    expect(useTermStore.getState().liveTabs).toContain("A");

    // Persist the layout. If loadTree reapplies and collapses it, A will incorrectly disappear from liveTabs.
    localStorage.setItem(
      "vlx-layout",
      JSON.stringify({
        openTabs: useTermStore.getState().openTabs,
        paneTrees: useTermStore.getState().paneTrees,
        activeTabId: useTermStore.getState().activeTabId,
        activeSessionId: useTermStore.getState().activeSessionId,
        focusedPaneId: useTermStore.getState().focusedPaneId,
        liveTabs: useTermStore.getState().liveTabs,
      }),
    );
    lifecycle.length = 0; // Observe only lifecycle events after loadTree.

    // Simulate a refresh triggered by creating, deleting, renaming, or moving a session.
    await act(async () => {
      await useTermStore.getState().loadTree();
    });

    expect(countUnmounts("A")).toBe(0); // The background session stays mounted; ptyKill is not called.
    expect(useTermStore.getState().liveTabs).toContain("A"); // A remains in background keep-alive.
  });

  // Feature: Move to Background manually transfers a visible session tab from openTabs to liveTabs while
  // retaining its pane tree and process. If it was active, focus passes to an adjacent tab.
  it("sending to background: manually backgrounding the active tab keeps the process mounted and hands activation to the adjacent tab", () => {
    seed(false); // Multi-tab mode: A and B can both be visible.
    beforeEachRender();

    openNewTab("A");
    openNewTab("B"); // openTabs = [A, B], with B active.
    expect(useTermStore.getState().activeTabId).toBe("B");

    act(() => {
      useTermStore.getState().moveTabToBackground("B");
    });

    const st = useTermStore.getState();
    expect(st.openTabs).toEqual(["A"]); // B leaves the tab bar.
    expect(st.liveTabs).toContain("B"); // B enters background keep-alive.
    expect(countUnmounts("B")).toBe(0); // It stays mounted and its process survives.
    expect(st.activeTabId).toBe("A"); // Focus passes to the adjacent tab.
  });

  // Feature: moving the only tab to the background empties the tab bar and leaves no active tab, but keeps
  // the session alive. Returning restores the complete tree without a remount or process interruption.
  it("sending to background: the only tab stays alive in the background and is not remounted when switched back", () => {
    seed(true);
    beforeEachRender();

    open("A");
    expect(countMounts("A")).toBe(1);

    act(() => {
      useTermStore.getState().moveTabToBackground("A");
    });
    let st = useTermStore.getState();
    expect(st.openTabs).toEqual([]); // The tab bar is empty.
    expect(st.activeTabId).toBeNull();
    expect(st.liveTabs).toContain("A"); // A remains alive in the background.
    expect(countUnmounts("A")).toBe(0); // A remains mounted.

    open("A"); // Return from the background.
    st = useTermStore.getState();
    expect(st.openTabs).toContain("A");
    expect(st.liveTabs).not.toContain("A");
    expect(countMounts("A")).toBe(1); // A was mounted only once throughout.
    expect(countUnmounts("A")).toBe(0);
  });
});

describe("empty state with tabs scoped to a project", () => {
  it("shows the empty state when the active project has no visible tab, even though other tabs are open", () => {
    seed(false);
    const newScratchTab = vi.fn();
    useTermStore.setState({
      projects: [
        { id: "p1", name: "P", rootPath: "/tmp", color: null, sortOrder: 0, collapsed: false, createdAt: 0 },
        { id: "p2", name: "Q", rootPath: "/tmp2", color: null, sortOrder: 1, collapsed: false, createdAt: 0 },
      ],
      scopeTabsToProject: true,
      activeProjectId: "p2",
      openTabs: ["A"],
      activeTabId: "A",
      newScratchTab,
    });
    beforeEachRender();
    expect(screen.getByText(t("center.noSession"))).toBeTruthy();
    fireEvent.click(screen.getByText(t("center.createTerminal")));
    expect(newScratchTab).toHaveBeenCalledTimes(1);
  });
});

// Render CenterPane once per case after seeding so the initial store state is ready.
function beforeEachRender() {
  render(<CenterPane />);
}
