import { describe, expect, it } from "vitest";
import type { PaneNode } from "../layout/CenterPane/paneTree";
import type { Session } from "../types";
import {
  nextVisibleTab,
  projectShowing,
  tabProjectId,
  visibleTabIds,
  type TabScopeState,
} from "./visibleTabs";

const leaf = (sessionId: string): PaneNode => ({ kind: "leaf", paneId: `p-${sessionId}`, sessionId });
const split = (a: PaneNode, b: PaneNode): PaneNode => ({
  kind: "split",
  paneId: "split",
  dir: "horizontal",
  sizes: [50, 50],
  a,
  b,
});
const session = (id: string, projectId: string, extra: Partial<Session> = {}): Session => ({
  id,
  projectId,
  groupId: null,
  name: id,
  kind: "terminal",
  collapsed: false,
  sortOrder: 0,
  createdAt: 0,
  ...extra,
});

function scope(patch: Partial<TabScopeState> = {}): TabScopeState {
  return {
    scopeTabsToProject: true,
    activeProjectId: "A",
    openTabs: ["a1", "b1", "g1", "a2"],
    paneTrees: { a1: leaf("a1"), b1: leaf("b1"), g1: leaf("g1"), a2: leaf("a2") },
    projects: [{ id: "A" }, { id: "B" }],
    sessions: [session("a1", "A"), session("a2", "A"), session("b1", "B")],
    ephemeralSessions: { g1: session("g1", "", { global: true }) },
    taskTabs: {},
    ...patch,
  };
}

describe("visibleTabIds", () => {
  it("shows every tab, unchanged, when the setting is off", () => {
    const state = scope({ scopeTabsToProject: false });
    expect(visibleTabIds(state)).toBe(state.openTabs);
  });

  it("shows every tab when no project is active", () => {
    expect(visibleTabIds(scope({ activeProjectId: null }))).toEqual(["a1", "b1", "g1", "a2"]);
  });

  it("shows the active project's tabs and global terminals in tab order", () => {
    expect(visibleTabIds(scope())).toEqual(["a1", "g1", "a2"]);
    expect(visibleTabIds(scope({ activeProjectId: "B" }))).toEqual(["b1", "g1"]);
  });

  it("shows only global terminals for a project without tabs", () => {
    const state = scope({ activeProjectId: "C", projects: [{ id: "A" }, { id: "B" }, { id: "C" }] });
    expect(visibleTabIds(state)).toEqual(["g1"]);
  });

  it("keeps documents, standalone browser tabs and project-less terminals visible", () => {
    const state = scope({
      openTabs: ["doc-1", "browser-1", "eph-x", "b1"],
      ephemeralSessions: { "eph-x": session("eph-x", "") },
    });
    expect(visibleTabIds(state)).toEqual(["doc-1", "browser-1", "eph-x"]);
  });

  it("treats tabs of a collection as owned by no project, so they stay visible", () => {
    const state = scope({
      projects: [{ id: "A", rootPath: "/a" }, { id: "C", rootPath: "" }],
      openTabs: ["c1", "a1"],
      paneTrees: { c1: leaf("c1"), a1: leaf("a1") },
      sessions: [session("c1", "C"), session("a1", "A")],
      ephemeralSessions: {},
    });
    expect(tabProjectId(state, "c1")).toBeNull();
    expect(visibleTabIds(state)).toEqual(["c1", "a1"]);
  });

  it("keeps tabs of a deleted project visible instead of stranding them", () => {
    const state = scope({
      openTabs: ["eph-z"],
      paneTrees: { "eph-z": leaf("eph-z") },
      ephemeralSessions: { "eph-z": session("eph-z", "gone") },
    });
    expect(visibleTabIds(state)).toEqual(["eph-z"]);
  });
});

describe("tabProjectId", () => {
  it("follows the root session even when a split holds another project's pane", () => {
    expect(tabProjectId(scope({ paneTrees: { a1: split(leaf("a1"), leaf("b1")) } }), "a1")).toBe("A");
  });

  it("falls back to the first pane once the root session is gone", () => {
    expect(tabProjectId(scope({ paneTrees: { "eph-gone": split(leaf("b1"), leaf("a1")) } }), "eph-gone")).toBe("B");
  });

  it("puts a task tab in its conversation's project", () => {
    expect(tabProjectId(scope({ taskTabs: { "task-1": { sessionId: "b1" } } }), "task-1")).toBe("B");
  });

  it("treats a global terminal as project-less", () => {
    expect(tabProjectId(scope(), "g1")).toBeNull();
  });
});

describe("nextVisibleTab", () => {
  it("moves to the next visible tab, skipping other projects", () => {
    expect(nextVisibleTab(scope(), "a1")).toBe("g1");
  });

  it("falls back to the previous visible tab at the end", () => {
    expect(nextVisibleTab(scope(), "a2")).toBe("g1");
  });

  it("falls back to the first visible tab when the closing tab is already hidden", () => {
    expect(nextVisibleTab(scope(), "b1")).toBe("a1");
  });

  it("returns null when the closing tab was the last visible one", () => {
    expect(nextVisibleTab(scope({ openTabs: ["a1", "b1"], ephemeralSessions: {} }), "a1")).toBeNull();
  });

  it("keeps the old neighbour rule when the setting is off", () => {
    expect(nextVisibleTab(scope({ scopeTabsToProject: false }), "a1")).toBe("b1");
  });
});

describe("projectShowing", () => {
  it("keeps the active project for a visible tab", () => {
    expect(projectShowing(scope(), "g1")).toBe("A");
  });

  it("switches to the owner of a hidden tab", () => {
    expect(projectShowing(scope(), "b1")).toBe("B");
  });

  it("keeps the active project when no tab is given", () => {
    expect(projectShowing(scope(), null)).toBe("A");
  });
});
