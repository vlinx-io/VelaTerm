//! Which conversation-view controls each agent offers, so a control is never shown for a capability its
//! protocol lacks.

import { describe, expect, it } from "vitest";
import { acceptsChatImages, hasTerminalInterface, supportsChatEngine, supportsSteering } from "./types";

describe("chat capabilities", () => {
  it("opens the conversation view for protocol-driven agents", () => {
    for (const kind of ["copilot", "cursor", "kimi", "grok", "deepseek"] as const) expect(supportsChatEngine(kind)).toBe(true);
    for (const kind of ["cline", "kiro", "crush", "zoo"] as const) expect(supportsChatEngine(kind)).toBe(false);
  });

  it("offers steering only where the protocol can join a running turn", () => {
    for (const kind of ["claude", "codex", "opencode", "pi", "omp", "zcode", "antigravity"] as const) expect(supportsSteering(kind)).toBe(true);
    for (const kind of ["copilot", "cursor", "kimi", "grok"] as const) expect(supportsSteering(kind)).toBe(false);
    for (const kind of ["opencode", "deepseek", "claude"] as const) expect(supportsSteering(kind)).toBe(true);
  });

  it("offers images to every agent whose protocol can carry them", () => {
    for (const kind of ["claude", "zcode", "opencode", "deepseek", "antigravity", "copilot", "cursor", "kimi"] as const) expect(acceptsChatImages(kind)).toBe(true);
    expect(acceptsChatImages("grok")).toBe(false);
  });

  it("keeps DeepSeek, which has no terminal interface, out of the terminal view", () => {
    for (const kind of ["deepseek", "zcode"] as const) expect(hasTerminalInterface(kind)).toBe(false);
    for (const kind of ["claude", "codex", "copilot", "terminal"] as const) expect(hasTerminalInterface(kind)).toBe(true);
  });
});
