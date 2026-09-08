//! Tests for the parts of the tool cards that are pure logic: the summary line and the line diff.

import { describe, expect, it } from "vitest";

import { diffFiles, diffLines, relPath, toolSummary } from "./toolCards";

describe("toolSummary", () => {
  it("shows the command for a shell call and the path for a file call", () => {
    expect(toolSummary("Bash", { command: "cargo test" })).toBe("cargo test");
    expect(toolSummary("Read", { file_path: "/repo/src/main.rs" }, "/repo")).toBe("src/main.rs");
  });

  it("leaves a path alone when it is outside the working directory", () => {
    expect(relPath("/etc/hosts", "/repo")).toBe("/etc/hosts");
  });

  it("falls back to a recognizable field for an unfamiliar tool", () => {
    expect(toolSummary("SomePluginTool", { command: "deploy" })).toBe("deploy");
    expect(toolSummary("SomePluginTool", { unrelated: 1 })).toBeUndefined();
  });

  it("returns nothing when the tool has no worthwhile one-liner", () => {
    expect(toolSummary("TodoWrite", { todos: [] })).toBeUndefined();
    expect(toolSummary(undefined, {})).toBeUndefined();
  });
});

describe("diffLines", () => {
  it("marks the changed line and keeps the surrounding lines as context", () => {
    const lines = diffLines("a\nb\nc", "a\nB\nc");
    expect(lines.map((l) => l.sign + l.text)).toEqual([" a", "-b", "+B", " c"]);
  });

  it("finds an insertion in the middle rather than rewriting the tail", () => {
    const lines = diffLines("a\nc", "a\nb\nc");
    expect(lines.map((l) => l.sign + l.text)).toEqual([" a", "+b", " c"]);
  });

  it("treats an empty side as a pure addition or removal", () => {
    expect(diffLines("", "x").map((l) => l.sign)).toEqual(["+"]);
    expect(diffLines("x", "").map((l) => l.sign)).toEqual(["-"]);
  });
});

describe("diffFiles", () => {
  it("names each file a unified diff touches once, without the b/ prefix", () => {
    const diff = [
      "diff --git a/src/a.rs b/src/a.rs",
      "--- a/src/a.rs",
      "+++ b/src/a.rs",
      "+one",
      "--- /dev/null",
      "+++ b/src/new.rs",
      "+two",
      "--- a/src/a.rs",
      "+++ b/src/a.rs",
    ].join("\n");
    expect(diffFiles(diff)).toEqual(["src/a.rs", "src/new.rs"]);
    expect(toolSummary("Diff", { diff })).toBe("src/a.rs, src/new.rs");
  });
});
