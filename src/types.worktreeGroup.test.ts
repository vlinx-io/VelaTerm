//! Which group's worktree applies inside nested groups, so work started in a subgroup lands in the bound worktree.

import { describe, expect, it } from "vitest";
import { type Group, worktreeGroup } from "./types";

const group = (id: string, parentGroupId: string | null, worktreePath: string | null = null): Group => ({
  id, projectId: "p", parentGroupId, name: id, sortOrder: 0, collapsed: false, createdAt: 0, worktreePath,
});

describe("worktreeGroup", () => {
  const groups = [
    group("outer", null, "/repo/.vlx-worktrees/outer"),
    group("nested", "outer"),
    group("deep", "nested"),
    group("own", "nested", "/repo/.vlx-worktrees/own"),
    group("plain", null),
  ];

  it("returns the group itself when it is bound", () => {
    expect(worktreeGroup(groups, "outer")?.id).toBe("outer");
  });

  it("walks up to the nearest bound ancestor", () => {
    expect(worktreeGroup(groups, "nested")?.id).toBe("outer");
    expect(worktreeGroup(groups, "deep")?.id).toBe("outer");
  });

  it("lets a nested binding override its ancestor", () => {
    expect(worktreeGroup(groups, "own")?.id).toBe("own");
  });

  it("returns null without a bound group above", () => {
    expect(worktreeGroup(groups, "plain")).toBeNull();
    expect(worktreeGroup(groups, null)).toBeNull();
    expect(worktreeGroup(groups, "missing")).toBeNull();
  });

  it("stops on a parent cycle", () => {
    expect(worktreeGroup([group("a", "b"), group("b", "a")], "a")).toBeNull();
  });
});
