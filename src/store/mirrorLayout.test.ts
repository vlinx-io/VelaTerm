//! Snapshot-building and validation tests for mirror mode: what a published arrangement carries, what it
//! deliberately leaves out, and how a malformed or version-skewed one from a peer is handled.

import { describe, expect, it } from "vitest";
import {
  buildMirrorLayout,
  layoutSessionIds,
  sanitizeMirrorLayout,
  MIRROR_LAYOUT_VERSION,
  type MirrorLayoutSource,
} from "./mirrorLayout";
import type { PaneNode } from "../layout/CenterPane/paneTree";
import type { SidebarTreeTab } from "../layout/LeftSidebar/sidebarTreeLayout";
import type { Session } from "../types";
import type { SidebarTreeView } from "./termStore";

const leaf = (paneId: string, sessionId: string): PaneNode => ({
  kind: "leaf",
  paneId,
  sessionId,
});

const split = (a: PaneNode, b: PaneNode): PaneNode => ({
  kind: "split",
  paneId: "s1",
  dir: "horizontal",
  sizes: [60, 40],
  a,
  b,
});

function session(id: string): Session {
  return {
    id,
    projectId: "p1",
    groupId: null,
    name: id,
    kind: "terminal",
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

function view(id: string, over: Partial<SidebarTreeView> = {}): SidebarTreeView {
  return {
    id,
    name: id,
    treeFilter: "",
    statusFilter: null,
    statusFilterIds: null,
    markFilter: null,
    collapsedOverrides: null,
    ...over,
  };
}

const tab = (id: string, viewId: string): SidebarTreeTab => ({
  id,
  root: { kind: "leaf", paneId: `sp-${viewId}`, viewId },
  activeViewId: viewId,
});

function source(over: Partial<MirrorLayoutSource> = {}): MirrorLayoutSource {
  return {
    openTabs: ["A"],
    liveTabs: [],
    pinnedTabs: [],
    activeTabId: "A",
    lastActiveSessionTabId: "A",
    activeSessionId: "A",
    focusedPaneId: "pa",
    paneTrees: { A: leaf("pa", "A") },
    ephemeralSessions: {},
    docTabs: {},
    browserTabs: {},
    taskTabs: {},
    selection: [{ id: "A", kind: "session" }],
    inspectTarget: { id: "A", kind: "session" },
    leftCollapsed: false,
    rightCollapsed: true,
    inspectorTab: "git",
    sidebarTreeViews: [view("main")],
    sidebarTreeTabs: [tab("t1", "main")],
    primarySidebarTreeViewId: "main",
    activeSidebarTreeViewId: "main",
    ...over,
  };
}

describe("buildMirrorLayout", () => {
  it("carries the center arrangement plus both side columns", () => {
    const layout = buildMirrorLayout(source());
    expect(layout.v).toBe(MIRROR_LAYOUT_VERSION);
    expect(layout.center.openTabs).toEqual(["A"]);
    expect(layout.center.activeSessionId).toBe("A");
    expect(layout.center.paneTrees.A).toEqual(leaf("pa", "A"));
    expect(layout.left.inspectTarget).toEqual({ id: "A", kind: "session" });
    expect(layout.right).toEqual({ inspectorTab: "git", collapsed: true });
  });

  it("keeps background tabs, whose trees stay mounted and must survive on the peer too", () => {
    const layout = buildMirrorLayout(
      source({
        liveTabs: ["B"],
        paneTrees: { A: leaf("pa", "A"), B: leaf("pb", "B") },
      }),
    );
    expect(layout.center.liveTabs).toEqual(["B"]);
    expect(Object.keys(layout.center.paneTrees).sort()).toEqual(["A", "B"]);
  });

  const taskTab = { id: "task-1", sessionId: "A", taskId: "t1", title: "probe", taskType: "local_workflow" };
  /** What the store holds while a task tab is in front: no active session and no focused pane. */
  const inTaskTab = { activeTabId: "task-1", activeSessionId: null, focusedPaneId: null, taskTabs: { "task-1": taskTab } };

  it("leaves task tabs out: they are client-local, and an active one publishes the last session tab instead", () => {
    const layout = buildMirrorLayout(source({ ...inTaskTab, openTabs: ["A", "task-1"] }));
    expect(layout.center.openTabs).toEqual(["A"]);
    // Never null with open tabs: the peer would show an empty stage, and its reconcile would publish its
    // first tab back and pull this client out of the task tab. The session and pane come along, derived
    // the way the peer's reconcile would derive them, so that reconcile has nothing to repair.
    expect(layout.center.activeTabId).toBe("A");
    expect(layout.center.activeSessionId).toBe("A");
    expect(layout.center.focusedPaneId).toBe("pa");
    expect(JSON.stringify(layout)).not.toContain("task-1");
    // Opening a task tab must not change the published bytes, or the sync loop would push for nothing.
    expect(JSON.stringify(buildMirrorLayout(source({ openTabs: ["A", "task-1"], taskTabs: { "task-1": taskTab } }))))
      .toBe(JSON.stringify(buildMirrorLayout(source())));
  });

  it("names the last published tab for an active task tab when the session anchor is not open", () => {
    const base = {
      ...inTaskTab,
      openTabs: ["A", "B", "task-1"],
      paneTrees: { A: leaf("pa", "A"), B: split(leaf("pb1", "B1"), leaf("pb2", "B2")) },
    };
    const anchored = buildMirrorLayout(source({ ...base, lastActiveSessionTabId: "B" })).center;
    expect([anchored.activeTabId, anchored.activeSessionId, anchored.focusedPaneId]).toEqual(["B", "B1", "pb1"]);
    expect(buildMirrorLayout(source({ ...base, lastActiveSessionTabId: "gone" })).center.activeTabId).toBe("B");
    expect(buildMirrorLayout(source({ ...base, lastActiveSessionTabId: null })).center.activeTabId).toBe("B");
    // Only an empty published tab list leaves the active tab null.
    const alone = buildMirrorLayout(source({ ...base, openTabs: ["task-1"], lastActiveSessionTabId: null, paneTrees: {} }));
    expect(alone.center.openTabs).toEqual([]);
    expect(alone.center.activeTabId).toBeNull();
    expect(alone.center.activeSessionId).toBeNull();
    expect(alone.center.focusedPaneId).toBeNull();
  });

  it("publishes no session or pane when the tab named for an active task tab is a document tab", () => {
    const doc = { id: "doc-1", path: "/tmp/x.md", title: "x" } as unknown as MirrorLayoutSource["docTabs"][string];
    const layout = buildMirrorLayout(
      source({ ...inTaskTab, openTabs: ["A", "doc-1", "task-1"], lastActiveSessionTabId: null, docTabs: { "doc-1": doc } }),
    ).center;
    expect([layout.activeTabId, layout.activeSessionId, layout.focusedPaneId]).toEqual(["doc-1", null, null]);
  });

  it("publishes a snapshot a peer's reconcile keeps as is, so following it echoes nothing back", () => {
    const published = buildMirrorLayout(source({ ...inTaskTab, openTabs: ["A", "task-1"] }));
    const received = sanitizeMirrorLayout(JSON.parse(JSON.stringify(published)))!;
    // The peer's reconcile only replaces an active tab that is null or not open, and only repairs a session
    // that is null or not in the active tab's tree; this snapshot trips neither.
    expect(received.center.activeTabId).toBe("A");
    expect(received.center.openTabs).toContain("A");
    expect(received.center.activeSessionId).toBe("A");
    expect(received.center.focusedPaneId).toBe("pa");
    // What the peer would publish from the adopted arrangement is byte-identical to what it received.
    expect(JSON.stringify(buildMirrorLayout(source({ ...received.center, taskTabs: {} })))).toBe(JSON.stringify(published));
  });

  it("drops pane trees and session metadata no tab references, keeping the payload to the arrangement", () => {
    const layout = buildMirrorLayout(
      source({
        paneTrees: { A: leaf("pa", "A"), stale: leaf("px", "X") },
        ephemeralSessions: { "eph-1": session("eph-1"), "eph-2": session("eph-2") },
      }),
    );
    expect(Object.keys(layout.center.paneTrees)).toEqual(["A"]);
    expect(layout.center.ephemeralSessions).toEqual({});
  });

  it("carries the ephemeral sessions a split actually shows, so the peer can render that split", () => {
    const layout = buildMirrorLayout(
      source({
        paneTrees: { A: split(leaf("pa", "A"), leaf("pb", "eph-1")) },
        ephemeralSessions: { "eph-1": session("eph-1"), "eph-2": session("eph-2") },
      }),
    );
    expect(Object.keys(layout.center.ephemeralSessions)).toEqual(["eph-1"]);
  });

  it("carries the sidebar projections with their filters, which mirror mode keeps identical on both sides", () => {
    const filtered = view("split-1", {
      treeFilter: "pr",
      statusFilter: ["working"],
      statusFilterIds: { s1: true },
      markFilter: "\u2b50",
      collapsedOverrides: { p1: true },
    });
    const layout = buildMirrorLayout(
      source({
        sidebarTreeViews: [view("main"), filtered],
        sidebarTreeTabs: [
          {
            id: "t1",
            root: {
              kind: "split",
              paneId: "sp-root",
              dir: "vertical",
              sizes: [50, 50],
              a: { kind: "leaf", paneId: "sp-main", viewId: "main" },
              b: { kind: "leaf", paneId: "sp-1", viewId: "split-1" },
            },
            activeViewId: "split-1",
          },
        ],
        activeSidebarTreeViewId: "split-1",
      }),
    );
    expect(layout.left.views[1]).toEqual(filtered);
    expect(layout.left.tabs[0].root.kind).toBe("split");
    expect(layout.left.primaryViewId).toBe("main");
    expect(layout.left.activeViewId).toBe("split-1");
  });

  it("serializes identically for two unchanged builds, which is what stops the sync loop feeding itself", () => {
    const s = source({ liveTabs: ["B"], paneTrees: { A: leaf("pa", "A"), B: leaf("pb", "B") } });
    expect(JSON.stringify(buildMirrorLayout(s))).toBe(JSON.stringify(buildMirrorLayout(s)));
  });
});

describe("sanitizeMirrorLayout", () => {
  const valid = () => JSON.parse(JSON.stringify(buildMirrorLayout(source())));

  it("accepts a snapshot this same build produced", () => {
    expect(sanitizeMirrorLayout(valid())).not.toBeNull();
  });

  it("rejects a foreign version rather than applying half of it", () => {
    expect(sanitizeMirrorLayout({ ...valid(), v: 99 })).toBeNull();
    expect(sanitizeMirrorLayout(null)).toBeNull();
    expect(sanitizeMirrorLayout("nope")).toBeNull();
  });

  it("drops a malformed pane tree instead of rendering a broken split", () => {
    const raw = valid();
    raw.center.paneTrees.bad = { kind: "split", paneId: "s", dir: "sideways", sizes: [1], a: 1, b: 2 };
    const out = sanitizeMirrorLayout(raw);
    expect(Object.keys(out!.center.paneTrees)).toEqual(["A"]);
  });

  it("clears an active tab the snapshot does not actually open", () => {
    const raw = valid();
    raw.center.activeTabId = "ghost";
    expect(sanitizeMirrorLayout(raw)!.center.activeTabId).toBeNull();
  });

  it("keeps only well-formed selections and falls back to the files tab", () => {
    const raw = valid();
    raw.left.selection = [{ id: "A", kind: "session" }, { id: "B", kind: "nonsense" }, 7];
    raw.right.inspectorTab = "weather";
    const out = sanitizeMirrorLayout(raw)!;
    expect(out.left.selection).toEqual([{ id: "A", kind: "session" }]);
    expect(out.right.inspectorTab).toBe("files");
  });
});

describe("sanitizeMirrorLayout: sidebar", () => {
  const valid = () => JSON.parse(JSON.stringify(buildMirrorLayout(source())));

  it("restores a status filter only with the ID snapshot it was captured against", () => {
    const raw = valid();
    raw.left.views = [
      { ...view("main"), statusFilter: ["working"], statusFilterIds: null },
      { ...view("v2"), statusFilter: ["asking"], statusFilterIds: { s1: true } },
    ];
    raw.left.tabs = [tab("t1", "main"), tab("t2", "v2")];
    const out = sanitizeMirrorLayout(raw)!;
    expect(out.left.views[0].statusFilter).toBeNull();
    expect(out.left.views[1].statusFilter).toEqual(["asking"]);
    expect(out.left.views[1].statusFilterIds).toEqual({ s1: true });
  });

  it("drops a tab whose split references a projection that is not there", () => {
    const raw = valid();
    raw.left.tabs = [tab("t1", "ghost"), tab("t2", "main")];
    const out = sanitizeMirrorLayout(raw)!;
    expect(out.left.tabs.map((x) => x.id)).toEqual(["t2"]);
  });

  it("gives a projection no tab claims one of its own, so neither side loses a pane", () => {
    const raw = valid();
    raw.left.views = [view("main"), view("orphan")];
    const out = sanitizeMirrorLayout(raw)!;
    expect(out.left.tabs).toHaveLength(2);
    expect(out.left.tabs[1].root).toMatchObject({ kind: "leaf", viewId: "orphan" });
  });

  it("falls back to the first projection when the primary and active IDs are unknown", () => {
    const raw = valid();
    raw.left.primaryViewId = "gone";
    raw.left.activeViewId = "gone";
    const out = sanitizeMirrorLayout(raw)!;
    expect(out.left.primaryViewId).toBe("main");
    expect(out.left.activeViewId).toBe("main");
  });

  it("returns no projections for an unusable payload, which tells the client to keep its own sidebar", () => {
    const raw = valid();
    raw.left.views = [{ id: 7 }, null];
    expect(sanitizeMirrorLayout(raw)!.left.views).toEqual([]);
    const missing = valid();
    delete missing.left.views;
    expect(sanitizeMirrorLayout(missing)!.left.views).toEqual([]);
  });
});

describe("layoutSessionIds", () => {
  it("returns every session the trees reference, across tabs and splits", () => {
    const layout = buildMirrorLayout(
      source({
        liveTabs: ["B"],
        paneTrees: { A: split(leaf("pa", "A"), leaf("pb", "eph-1")), B: leaf("pc", "B") },
      }),
    );
    expect([...layoutSessionIds(layout)].sort()).toEqual(["A", "B", "eph-1"]);
  });
});
