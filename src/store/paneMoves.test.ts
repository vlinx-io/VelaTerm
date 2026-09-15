//! Placement transforms for existing sessions: every move must leave the moved session in exactly one tree, keep
//! tab keys pointing at a session inside their own tree, and never drop a named session unless asked to.

import { describe, expect, it } from "vitest";
import {
  buildGrid,
  collectSessionIds,
  findBySession,
  makeLeaf,
  type PaneNode,
  splitAt,
} from "../layout/CenterPane/paneTree";
import { detachSessions, placeInGrid, placeInPane, placeInSplit, type TabLayout } from "./paneMoves";

function layout(partial: Partial<TabLayout> & { paneTrees: Record<string, PaneNode> }): TabLayout {
  return {
    openTabs: Object.keys(partial.paneTrees),
    liveTabs: [],
    pinnedTabs: [],
    activeTabId: Object.keys(partial.paneTrees)[0] ?? null,
    lastActiveSessionTabId: null,
    ...partial,
  };
}

function paneOf(tree: PaneNode, sessionId: string): string {
  const leaf = findBySession(tree, sessionId);
  if (!leaf) throw new Error(`no pane for ${sessionId}`);
  return leaf.paneId;
}

/** Every session appears in at most one tree, and every tree holds the session it is keyed by. */
function expectConsistent(l: TabLayout) {
  const seen = new Map<string, string>();
  for (const tabId of [...l.openTabs, ...l.liveTabs]) {
    const tree = l.paneTrees[tabId];
    expect(tree, `tab ${tabId} has a tree`).toBeTruthy();
    expect(findBySession(tree, tabId), `tab ${tabId} holds its keying session`).toBeTruthy();
    for (const sid of collectSessionIds(tree)) {
      expect(seen.has(sid), `${sid} shown in ${seen.get(sid)} and ${tabId}`).toBe(false);
      seen.set(sid, tabId);
    }
  }
}

describe("detachSessions", () => {
  it("removes a tab left empty and clears references to it", () => {
    const l = layout({
      paneTrees: { A: makeLeaf("A"), B: makeLeaf("B") },
      pinnedTabs: ["B"],
      activeTabId: "B",
      lastActiveSessionTabId: "B",
    });
    const next = detachSessions(l, new Set(["B"]));
    expect(next.openTabs).toEqual(["A"]);
    expect(next.pinnedTabs).toEqual([]);
    expect(next.paneTrees.B).toBeUndefined();
    expect(next.activeTabId).toBeNull();
    expect(next.lastActiveSessionTabId).toBeNull();
  });

  it("re-keys a tab whose keying session leaves while other panes stay", () => {
    // [A | C] keyed by A, in the background, pinned and recorded as active.
    const base = makeLeaf("A");
    const ac = splitAt(base, base.paneId, "horizontal", "C");
    const l = layout({
      paneTrees: { A: ac },
      openTabs: [],
      liveTabs: ["A"],
      pinnedTabs: ["A"],
      activeTabId: "A",
      lastActiveSessionTabId: "A",
    });
    const next = detachSessions(l, new Set(["A"]));
    expect(next.liveTabs).toEqual(["C"]);
    expect(next.pinnedTabs).toEqual(["C"]);
    expect(next.activeTabId).toBe("C");
    expect(next.lastActiveSessionTabId).toBe("C");
    expect(collectSessionIds(next.paneTrees.C)).toEqual(["C"]);
    expect(next.paneTrees.A).toBeUndefined();
  });
});

describe("placeInSplit", () => {
  it("moves a session out of another tab into a split beside the target pane", () => {
    const l = layout({ paneTrees: { A: makeLeaf("A"), B: makeLeaf("B") }, activeTabId: "A" });
    const target = paneOf(l.paneTrees.A, "A");
    const next = placeInSplit(l, "B", target, "horizontal", false)!;
    expect(next).not.toBeNull();
    expect(next.openTabs).toEqual(["A"]);
    expect(collectSessionIds(next.paneTrees.A)).toEqual(["A", "B"]);
    expect(next.activeTabId).toBe("A");
    expect(next.activeSessionId).toBe("B");
    expect(next.focusedPaneId).toBe(paneOf(next.paneTrees.A, "B"));
    expectConsistent(next);
  });

  it("returns only layout fields even when handed the whole store state", () => {
    const state = { ...layout({ paneTrees: { A: makeLeaf("A"), B: makeLeaf("B") } }), epochs: { B: 1 } };
    const next = placeInSplit(state, "B", paneOf(state.paneTrees.A, "A"), "horizontal", false)!;
    expect(Object.keys(next).sort()).toEqual(
      [
        "activeSessionId",
        "activeTabId",
        "focusedPaneId",
        "lastActiveSessionTabId",
        "liveTabs",
        "openTabs",
        "paneTrees",
        "pinnedTabs",
      ].sort(),
    );
  });

  it("places the session before the target for left and top drops", () => {
    const l = layout({ paneTrees: { A: makeLeaf("A"), B: makeLeaf("B") } });
    const next = placeInSplit(l, "B", paneOf(l.paneTrees.A, "A"), "vertical", true)!;
    const tree = next.paneTrees.A;
    expect(tree.kind === "split" && tree.dir).toBe("vertical");
    expect(collectSessionIds(tree)).toEqual(["B", "A"]);
  });

  it("rearranges panes of the same tab and re-keys the tab when its keying session moves", () => {
    // [A | C] keyed by A; move A below C.
    const base = makeLeaf("A");
    const l = layout({ paneTrees: { A: splitAt(base, base.paneId, "horizontal", "C") } });
    const cPane = paneOf(l.paneTrees.A, "C");
    const next = placeInSplit(l, "A", cPane, "vertical", false)!;
    expect(next.openTabs).toEqual(["C"]);
    expect(next.activeTabId).toBe("C");
    const tree = next.paneTrees.C;
    expect(tree.kind === "split" && tree.dir).toBe("vertical");
    expect(collectSessionIds(tree)).toEqual(["C", "A"]);
    // The target pane keeps its identity.
    expect(paneOf(tree, "C")).toBe(cPane);
    expectConsistent(next);
  });

  it("is a no-op for a pane that already shows the session or is not visible", () => {
    const l = layout({ paneTrees: { A: makeLeaf("A"), B: makeLeaf("B") }, openTabs: ["A"], liveTabs: ["B"] });
    expect(placeInSplit(l, "A", paneOf(l.paneTrees.A, "A"), "horizontal", false)).toBeNull();
    expect(placeInSplit(l, "A", paneOf(l.paneTrees.B, "B"), "horizontal", false)).toBeNull();
  });
});

describe("placeInPane", () => {
  const named = (sid: string) => !sid.startsWith("eph-");

  it("swaps two panes of the same tab without backgrounding anything", () => {
    const base = makeLeaf("A");
    const l = layout({ paneTrees: { A: splitAt(base, base.paneId, "horizontal", "C") } });
    const next = placeInPane(l, "C", paneOf(l.paneTrees.A, "A"), named)!;
    expect(collectSessionIds(next.paneTrees.A)).toEqual(["C", "A"]);
    expect(next.backgrounded).toBeNull();
    expect(next.activeSessionId).toBe("C");
  });

  it("moves a named displaced session into its own background tab and re-keys the target tab", () => {
    const l = layout({ paneTrees: { A: makeLeaf("A"), B: makeLeaf("B") }, activeTabId: "A" });
    const target = paneOf(l.paneTrees.A, "A");
    const next = placeInPane(l, "B", target, named)!;
    expect(next.backgrounded).toBe("A");
    expect(next.openTabs).toEqual(["B"]);
    expect(next.liveTabs).toEqual(["A"]);
    expect(next.activeTabId).toBe("B");
    expect(next.focusedPaneId).toBe(target);
    expect(collectSessionIds(next.paneTrees.B)).toEqual(["B"]);
    expect(collectSessionIds(next.paneTrees.A)).toEqual(["A"]);
    expectConsistent(next);
  });

  it("drops a scratch terminal it displaces", () => {
    const base = makeLeaf("A");
    const tree = splitAt(base, base.paneId, "horizontal", "eph-1");
    const l = layout({ paneTrees: { A: tree, B: makeLeaf("B") } });
    const next = placeInPane(l, "B", paneOf(tree, "eph-1"), named)!;
    expect(next.backgrounded).toBeNull();
    expect(collectSessionIds(next.paneTrees.A)).toEqual(["A", "B"]);
    expect(next.liveTabs).toEqual([]);
    expectConsistent(next);
  });

  it("refuses rather than dropping a named session whose id is already taken as a tab key", () => {
    // A stale background tab is keyed by A without showing it, so the displaced A has no key to live under.
    const l = layout({
      paneTrees: { T: makeLeaf("A"), A: makeLeaf("Q"), B: makeLeaf("B") },
      openTabs: ["T", "B"],
      liveTabs: ["A"],
      activeTabId: "T",
    });
    expect(placeInPane(l, "B", paneOf(l.paneTrees.T, "A"), named)).toBeNull();
  });
});

describe("placeInGrid", () => {
  it("tiles four sessions 2×2 in reading order in a pinned tab after the active one", () => {
    const l = layout({
      paneTrees: { A: makeLeaf("A"), B: makeLeaf("B"), C: makeLeaf("C"), D: makeLeaf("D"), E: makeLeaf("E") },
      openTabs: ["E", "A", "B"],
      liveTabs: ["C", "D"],
      activeTabId: "E",
    });
    const next = placeInGrid(l, ["A", "B", "C", "D"])!;
    expect(next.openTabs).toEqual(["E", "A"]);
    expect(next.liveTabs).toEqual([]);
    expect(next.pinnedTabs).toEqual(["A"]);
    expect(next.activeTabId).toBe("A");
    expect(next.activeSessionId).toBe("A");
    const grid = next.paneTrees.A;
    // Columns [A over C] | [B over D]: reading order is A B / C D.
    expect(grid.kind === "split" && grid.dir).toBe("horizontal");
    expect(collectSessionIds(grid)).toEqual(["A", "C", "B", "D"]);
    expectConsistent(next);
  });

  it("ignores duplicates and needs at least two distinct sessions", () => {
    const l = layout({ paneTrees: { A: makeLeaf("A") } });
    expect(placeInGrid(l, ["A", "A"])).toBeNull();
  });
});

describe("buildGrid", () => {
  it("lays out two, three and four sessions with even shares", () => {
    const two = buildGrid(["a", "b"])!;
    expect(two.kind === "split" && [two.dir, two.sizes]).toEqual(["horizontal", [50, 50]]);
    const three = buildGrid(["a", "b", "c"])!;
    expect(collectSessionIds(three)).toEqual(["a", "b", "c"]);
    expect(three.kind === "split" && three.b.kind === "split" && three.b.dir).toBe("vertical");
    expect(collectSessionIds(buildGrid(["a", "b", "c", "d", "e"])!)).toEqual(["a", "c", "b", "d"]);
    expect(buildGrid([])).toBeNull();
  });
});
