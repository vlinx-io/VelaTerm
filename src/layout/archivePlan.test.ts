//! The archive confirmation may only list worktrees the backend actually deletes.

import { describe, expect, it } from "vitest";

import { archivedGroupWorktreePaths, archivedWorktreePaths } from "./archivePlan";
import type { Group, Session } from "../types";

function session(id: string, extra: Partial<Session> = {}): Session {
  return {
    id,
    projectId: "p1",
    name: id,
    kind: "claude",
    collapsed: false,
    sortOrder: 0,
    createdAt: 0,
    ...extra,
  } as Session;
}

function group(id: string, extra: Partial<Group> = {}): Group {
  return {
    id,
    projectId: "p1",
    name: id,
    sortOrder: 0,
    collapsed: false,
    createdAt: 0,
    ...extra,
  } as Group;
}

describe("archivedWorktreePaths", () => {
  const lead = session("lead", { worktreePath: "/repo" });
  const worker = session("worker", {
    parentSessionId: "lead",
    worktreePath: "/repo/.vlx-worktrees/worker",
  });
  const nested = session("nested", {
    parentSessionId: "worker",
    worktreePath: "/repo/.vlx-worktrees/nested",
  });

  it("lists every worker worktree in the subtree and skips the top-level directory", () => {
    expect(archivedWorktreePaths([lead, worker, nested], [], ["lead"])).toEqual([
      "/repo/.vlx-worktrees/worker",
      "/repo/.vlx-worktrees/nested",
    ]);
  });

  it("skips a worktree a group also binds, which the archive leaves in place", () => {
    const shared = [group("g1", { worktreePath: "/repo/.vlx-worktrees/worker" })];
    expect(archivedWorktreePaths([lead, worker], shared, ["lead"])).toEqual([]);
  });

  it("reports one path when two selected roots share the same subtree", () => {
    expect(archivedWorktreePaths([lead, worker], [], ["lead", "worker"])).toEqual([
      "/repo/.vlx-worktrees/worker",
    ]);
  });

  it("returns nothing for a subtree without worker worktrees", () => {
    const plain = session("plain", { parentSessionId: "lead" });
    expect(archivedWorktreePaths([lead, plain], [], ["lead"])).toEqual([]);
  });
});

describe("archivedGroupWorktreePaths", () => {
  it("covers every root of the group and its subgroups", () => {
    const groups = [group("g1"), group("g2", { parentGroupId: "g1" })];
    const sessions = [
      session("lead-a", { groupId: "g1" }),
      session("worker-a", { parentSessionId: "lead-a", worktreePath: "/repo/wt/a" }),
      session("lead-b", { groupId: "g2" }),
      session("worker-b", { parentSessionId: "lead-b", worktreePath: "/repo/wt/b" }),
      session("outside", { groupId: "g9" }),
      session("worker-c", { parentSessionId: "outside", worktreePath: "/repo/wt/c" }),
    ];
    expect(archivedGroupWorktreePaths(sessions, groups, "g1")).toEqual([
      "/repo/wt/a",
      "/repo/wt/b",
    ]);
  });
});
