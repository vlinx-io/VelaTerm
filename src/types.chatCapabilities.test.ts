//! Which conversation-view controls each agent offers, so a control is never shown for a capability its
//! protocol lacks.

import { describe, expect, it } from "vitest";
import { acceptsChatImages, hasTerminalInterface, supportsChatEngine, supportsSteering } from "./types";

describe("chat capabilities", () => {
  it("opens the conversation view for the ACP agents", () => {
    for (const kind of ["copilot", "cursor", "kimi", "grok", "deepseek"] as const) expect(supportsChatEngine(kind)).toBe(true);
    for (const kind of ["cline", "kiro", "crush", "zoo"] as const) expect(supportsChatEngine(kind)).toBe(false);
  });

  it("offers steering only where the protocol can join a running turn", () => {
    for (const kind of ["claude", "codex", "opencode", "pi", "omp"] as const) expect(supportsSteering(kind)).toBe(true);
    for (const kind of ["antigravity", "zcode", "copilot", "cursor", "kimi", "grok", "deepseek"] as const) expect(supportsSteering(kind)).toBe(false);
  });

  it("refuses images for the agents whose protocol declines them", () => {
    for (const kind of ["claude", "zcode", "copilot", "cursor", "kimi"] as const) expect(acceptsChatImages(kind)).toBe(true);
    for (const kind of ["antigravity", "grok", "deepseek"] as const) expect(acceptsChatImages(kind)).toBe(false);
  });

  it("keeps DeepSeek Harness, which has no terminal interface, out of the terminal view", () => {
    for (const kind of ["deepseek", "zcode"] as const) expect(hasTerminalInterface(kind)).toBe(false);
    for (const kind of ["claude", "codex", "copilot", "terminal"] as const) expect(hasTerminalInterface(kind)).toBe(true);
  });
});
