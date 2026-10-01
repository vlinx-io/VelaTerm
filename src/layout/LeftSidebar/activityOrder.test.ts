//! The pure ordering behind "Sort by activity": newest first, active before inactive, manual order as the
//! tie-breaker, and container ranks derived from their contents. Nothing here touches the store or React.

import { describe, expect, it } from "vitest";
import type { Group, Project, Session } from "../../types";
import { compareByActivity, isDropZoneAllowed, orderTreeByActivity } from "./activityOrder";

const project = (id: string, sortOrder: number): Project =>
  ({ id, name: id, rootPath: `/tmp/${id}`, sortOrder, collapsed: false, createdAt: 0 }) as Project;
const group = (id: string, projectId: string, sortOrder: number, parentGroupId: string | null = null): Group =>
  ({ id, projectId, parentGroupId, name: id, sortOrder, collapsed: false, createdAt: 0 }) as Group;
const session = (
  id: string,
  projectId: string,
  sortOrder: number,
  extra: Partial<Session> = {},
): Session =>
  ({ id, projectId, name: id, kind: "terminal", sortOrder, collapsed: false, createdAt: 0, ...extra }) as Session;

const ids = (nodes: { id: string }[]) => nodes.map((n) => n.id);

describe("compareByActivity", () => {
  const a = { sortOrder: 1 };
  const b = { sortOrder: 2 };

  it("puts the newer stamp first", () => {
    expect(compareByActivity(a, b, 100, 200)).toBeGreaterThan(0);
    expect(compareByActivity(a, b, 200, 100)).toBeLessThan(0);
  });

  it("puts any activity before none", () => {
    expect(compareByActivity(a, b, undefined, 5)).toBeGreaterThan(0);
    expect(compareByActivity(b, a, 5, undefined)).toBeLessThan(0);
  });

  it("falls back to the manual order on ties and when neither has activity", () => {
    expect(compareByActivity(a, b, 7, 7)).toBeLessThan(0);
    expect(compareByActivity(b, a, 7, 7)).toBeGreaterThan(0);
    expect(compareByActivity(a, b, undefined, undefined)).toBeLessThan(0);
  });
});

describe("orderTreeByActivity", () => {
  const projects = [project("p1", 1), project("p2", 2), project("p3", 3)];
  // Manual sort_order values are creation timestamps in the real tree, so they are distinct across the whole
  // array; the fixture mirrors that, which is what makes the tie-break independent of input order.
  const groups = [group("g1", "p1", 1), group("g2", "p1", 2), group("g2a", "p1", 3, "g2")];
  const sessions = [
    session("s1", "p1", 1),
    session("s2", "p1", 2, { groupId: "g1" }),
    session("s3", "p1", 3),
    session("s4", "p1", 4, { groupId: "g2a" }),
    session("s5", "p2", 5),
    session("s6", "p3", 6),
    session("s6-child", "p3", 7, { parentSessionId: "s6" }),
  ];

  it("orders sessions by activity descending and keeps inactive ones in manual order behind them", () => {
    const out = orderTreeByActivity(projects, groups, sessions, { s3: 300, s1: 100 });
    // s3 (300) then s1 (100), then everything without a stamp in manual order.
    expect(ids(out.sessions)).toEqual(["s3", "s1", "s2", "s4", "s5", "s6", "s6-child"]);
  });

  it("ranks a project by its most recent session and leaves inactive projects in manual order", () => {
    const out = orderTreeByActivity(projects, groups, sessions, { s5: 500, s1: 100 });
    expect(ids(out.projects)).toEqual(["p2", "p1", "p3"]);
  });

  it("ranks a group by the newest stamp of its contents, including nested groups", () => {
    // s4 sits in g2a, which is nested in g2, so both rank 400 and g2 moves ahead of g1 (200 via s2).
    const out = orderTreeByActivity(projects, groups, sessions, { s4: 400, s2: 200 });
    expect(ids(out.groups)).toEqual(["g2", "g2a", "g1"]);
  });

  it("ranks a parent session by its subtree, like a group", () => {
    const out = orderTreeByActivity(projects, groups, sessions, { "s6-child": 900, s1: 100 });
    expect(ids(out.sessions).slice(0, 3)).toEqual(["s6", "s6-child", "s1"]);
    expect(ids(out.projects)[0]).toBe("p3");
  });

  it("is independent of the input order and never mutates the inputs or sortOrder", () => {
    const activity = { s3: 300, s1: 100, s5: 500 };
    const shuffled = [...sessions].reverse();
    const a = orderTreeByActivity(projects, groups, sessions, activity);
    const b = orderTreeByActivity([...projects].reverse(), [...groups].reverse(), shuffled, activity);
    expect(ids(a.sessions)).toEqual(ids(b.sessions));
    expect(ids(a.projects)).toEqual(ids(b.projects));
    expect(ids(a.groups)).toEqual(ids(b.groups));
    expect(ids(sessions)).toEqual(["s1", "s2", "s3", "s4", "s5", "s6", "s6-child"]);
    expect(sessions.map((s) => s.sortOrder)).toEqual([1, 2, 3, 4, 5, 6, 7]);
  });

  it("returns the manual order when nothing has activity", () => {
    const out = orderTreeByActivity(projects, groups, sessions, {});
    expect(ids(out.projects)).toEqual(ids(projects));
    expect(ids(out.groups)).toEqual(ids(groups));
    expect(ids(out.sessions)).toEqual(ids(sessions));
  });
});

describe("isDropZoneAllowed", () => {
  it("always allows center drops and refuses edge drops only while the activity order is on", () => {
    expect(isDropZoneAllowed("center", true)).toBe(true);
    expect(isDropZoneAllowed("center", false)).toBe(true);
    expect(isDropZoneAllowed("top", false)).toBe(true);
    expect(isDropZoneAllowed("bottom", false)).toBe(true);
    expect(isDropZoneAllowed("top", true)).toBe(false);
    expect(isDropZoneAllowed("bottom", true)).toBe(false);
  });
});
