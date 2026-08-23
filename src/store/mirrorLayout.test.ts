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
import type { Session } from "../types";

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
    selection: [{ id: "A", kind: "session" }],
    inspectTarget: { id: "A", kind: "session" },
    leftCollapsed: false,
    rightCollapsed: true,
    inspectorTab: "git",
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
