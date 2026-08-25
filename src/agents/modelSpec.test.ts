//! Guards the per-agent model/effort table and the flag rewriting the spawn path relies on.

import { describe, expect, it } from "vitest";
import { AGENT_MODEL_SPECS, applyFlag, modelSpec, readFlag } from "./modelSpec";

describe("modelSpec table", () => {
  it("covers every agent kind and never leaves a static source without models", () => {
    for (const [kind, spec] of Object.entries(AGENT_MODEL_SPECS)) {
      expect(spec.modelFlag.startsWith("-"), kind).toBe(true);
      if (spec.source === "static") expect(spec.models?.length, kind).toBeGreaterThan(0);
      if (spec.source === "free") expect(spec.placeholder, kind).toBeTruthy();
      if (spec.effort) expect(spec.effort.values.length, kind).toBeGreaterThan(0);
    }
  });

  it("keeps each agent's own effort flag rather than one shared name", () => {
    expect(AGENT_MODEL_SPECS.claude.effort?.flag).toBe("--effort");
    expect(AGENT_MODEL_SPECS.grok.effort?.flag).toBe("--reasoning-effort");
    expect(AGENT_MODEL_SPECS.cline.effort?.flag).toBe("--thinking");
    // Crush and Codex configure reasoning in files, so offering a flag would launch them with an
    // argument their CLI rejects.
    expect(AGENT_MODEL_SPECS.crush.effort).toBeNull();
    expect(AGENT_MODEL_SPECS.codex.effort).toBeNull();
  });

  it("offers Claude's 1M-context aliases, which the old fixed list could not reach", () => {
    expect(AGENT_MODEL_SPECS.claude.models).toContain("fable");
    expect(AGENT_MODEL_SPECS.claude.models).toContain("opus[1m]");
  });

  it("returns null for plain terminals and unknown kinds", () => {
    expect(modelSpec("terminal")).toBeNull();
    expect(modelSpec("nope")).toBeNull();
    expect(modelSpec("kimi")).not.toBeNull();
  });
});

describe("applyFlag", () => {
  it("appends a flag that is absent and replaces one that is present", () => {
    expect(applyFlag("", "--model", "opus")).toBe("--model opus");
    expect(applyFlag("--model sonnet", "--model", "opus")).toBe("--model opus");
  });

  it("leaves other flags untouched so effort never drops an inherited model", () => {
    expect(applyFlag("--model opus --effort high", "--effort", "max")).toBe(
      "--model opus --effort max",
    );
  });

  it("removes the flag when the value is empty, which is how Default clears an override", () => {
    expect(applyFlag("--verbose --model opus", "--model", "")).toBe("--verbose");
  });

  it("does not let one flag name match a longer one that contains it", () => {
    // Replacing --effort must not touch --reasoning-effort, whose name ends with the same word.
    expect(applyFlag("--reasoning-effort high", "--effort", "low")).toBe(
      "--reasoning-effort high --effort low",
    );
  });

  it("keeps bracketed and slashed values intact", () => {
    expect(applyFlag("", "--model", "opus[1m]")).toBe("--model opus[1m]");
    expect(readFlag(applyFlag("", "--model", "anthropic/claude-opus-4.6"), "--model")).toBe(
      "anthropic/claude-opus-4.6",
    );
  });
});

describe("readFlag", () => {
  it("reads a value back and reports empty when the flag or the string is absent", () => {
    expect(readFlag("--model opus --effort high", "--effort")).toBe("high");
    expect(readFlag("--model opus", "--effort")).toBe("");
    expect(readFlag(null, "--model")).toBe("");
  });
});
