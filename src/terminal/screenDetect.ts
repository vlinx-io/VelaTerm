//! Screen detection: infer agent activity from the bottom screen of xterm's buffer.
//!
//! Corresponds to herdr's Codex/Claude detectors. Because xterm.js already maintains a terminal grid, read
//! `term.buffer.active` directly rather than parsing PTY output in the backend with libghostty-vt.
//!
//! **Key design**: always start at `buffer.active.baseY`, the top of the latest screen, never `viewportY`. User
//! scrolling changes viewportY and would turn detection into a historical snapshot rather than current state.

import type { Terminal } from "@xterm/xterm";
import type { AgentKind, AgentState } from "../types";

// Public types.

/** Screen-detection result corresponding to herdr's ScreenDetection concept. */
export interface ScreenDetection {
  /** Recommended detected status. */
  state: AgentState;
  /** Screen clearly shows confirmation UI chrome, a strong blocker. */
  visibleBlocker: boolean;
  /** Screen clearly shows an idle/ready prompt. */
  visibleIdle: boolean;
  /** Screen clearly shows an active-work indicator. */
  visibleWorking: boolean;
  /** Current screen is a transcript viewer or another non-live view that must not update status. */
  skip: boolean;
}

// Reading screen text.

/**
 * Reads plain text from xterm's latest screen regardless of user scrolling, corresponding to herdr's detection_text.
 *
 * Uses `baseY` rather than `viewportY` to stay on the live bottom screen.
 */
export function readScreenTail(term: Terminal): string {
  const buf = term.buffer.active;
  const rows = Math.max(term.rows, 24); // Match herdr's 24-row DEFAULT_DETECTION_ROWS floor.
  const lines: string[] = [];
  for (let i = 0; i < rows; i++) {
    // baseY keeps detection on the live bottom screen rather than the scrolled viewport.
    const line = buf.getLine(buf.baseY + i);
    lines.push(line?.translateToString(true) ?? ""); // true trims trailing whitespace.
  }
  while (lines.length && lines[lines.length - 1] === "") lines.pop(); // Remove trailing empty lines.
  return lines.join("\n");
}

// Dispatch entry point.

/** Dispatches to the detector for an agent kind. */
export function detectAgentScreen(
  agent: AgentKind,
  text: string,
): ScreenDetection {
  switch (agent) {
    case "codex":
      // Codex state is hook-only. usePtySession excludes it before reading xterm; keep this neutral branch as
      // a defensive no-op for direct callers so terminal content can never become an activity signal.
      return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: true };
    case "claude":
      return detectClaude(text);
    case "opencode":
      return detectOpencode(text);
    case "copilot":
      return detectCopilot(text);
    case "cursor":
      return detectCursor(text);
    case "antigravity":
      return detectAntigravity(text);
    case "cline":
      return detectCline(text);
    case "pi":
      return detectPi(text);
    case "omp":
      return detectOmp(text);
    case "crush":
      return detectCrush(text);
    case "kimi":
      return detectKimi(text);
    case "kiro":
      return detectKiro(text);
    case "grok":
      return detectGrok(text);
    case "zoo":
      return detectZoo(text);
    default:
      return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: false };
  }
}

// OpenCode detection (neutral fallback).
//
// Injected plugin events are authoritative for OpenCode. Return no strong screen signal so applyScreenDetection
// preserves hook state; add TUI rules only if future scenarios cannot be reached by hooks.
function detectOpencode(_text: string): ScreenDetection {
  return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: false };
}

// Copilot detection (neutral fallback).
//
// Installed Copilot command hooks report authoritative activity events. Return a neutral screen result unless a
// future scenario requires TUI-specific fallback rules.
function detectCopilot(_text: string): ScreenDetection {
  return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: false };
}

// Cursor detection (neutral fallback).
//
// Cursor command hooks provide authoritative beforeSubmitPrompt/stop events. Return neutral screen state. Cursor
// has no observable asking hook, so permission waits currently remain working until TUI blocker rules are added.
function detectCursor(_text: string): ScreenDetection {
  return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: false };
}

// Antigravity detection (neutral fallback).
//
// Antigravity's global hooks authoritatively report PreInvocation working and PostInvocation waiting. Return neutral
// screen state. It has no observable approval-wait hook, so permission prompts remain working pending TUI rules.
function detectAntigravity(_text: string): ScreenDetection {
  return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: false };
}

// Cline detection (neutral fallback).
//
// Cline's injected CLINE_HOOKS_DIR scripts authoritatively report prompt, completion/error, cancel, and lifecycle
// events. Return neutral screen state. Cline lacks asking/mid-turn hooks, so approval or questions remain working
// until blocker rules are added.
function detectCline(_text: string): ScreenDetection {
  return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: false };
}

// Pi detection (neutral fallback).
//
// Pi's command-line extension reports authoritative input/agent_start working and agent_end waiting events. Return
// neutral screen state. Pi has no permission-confirmation mechanism and therefore no asking state.
function detectPi(_text: string): ScreenDetection {
  return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: false };
}

// OMP detection (neutral fallback).
//
// OMP's command-line extension reports authoritative input/agent_start working, tool_approval_requested asking,
// and agent_end waiting events, so screen state stays neutral.
function detectOmp(_text: string): ScreenDetection {
  return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: false };
}

// Kimi Code lifecycle hooks provide authoritative state; keep screen detection neutral.
function detectKimi(_text: string): ScreenDetection {
  return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: false };
}

// Kiro's shadow-agent hooks cover prompt submission, both tool phases, and turn end, so keep screen detection
// neutral. Kiro has no permission-request hook and therefore no asking state; an interrupted turn is corrected
// by the backend PTY silence heal rather than by reading the screen.
function detectKiro(_text: string): ScreenDetection {
  return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: false };
}

// Grok Build has lifecycle hooks; PTY activity supplies working/waiting only before hooks establish authority.
function detectGrok(_text: string): ScreenDetection {
  return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: false };
}

// Zoo Code CLI detection. With no external lifecycle hooks, recognize stable Ink UI text: approval prompts first,
// then completion/idle input, then Thinking/Using tool. Checking idle before work avoids stale transcript text.
export function detectZoo(text: string): ScreenDetection {
  const lower = text.toLowerCase();
  if (
    lower.includes("approve this action? (y/n)") ||
    lower.includes("allow mcp access? (y/n)") ||
    lower.includes("continue with manual approval? (y/n)") ||
    lower.includes("approve? (y/n)")
  ) {
    return { state: "asking", visibleBlocker: true, visibleIdle: false, visibleWorking: false, skip: false };
  }
  if (
    lower.includes("type your message...") ||
    lower.includes("task completed") ||
    lower.includes("ready to start a new task")
  ) {
    return { state: "waiting", visibleBlocker: false, visibleIdle: true, visibleWorking: false, skip: false };
  }
  if (lower.includes("thinking") || lower.includes("using tool")) {
    return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: true, skip: false };
  }
  return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: false };
}

// Crush detection through real screen parsing, like Codex.
//
// Unlike hook-authoritative agents, Crush exposes only PreToolUse (working) and no authoritative waiting event, so
// idle must be detected from the screen.
//
// Markers come from stable Crush TUI source literals: Permission Required/Allow for Session means asking; ready
// placeholders mean idle; working/processing/thinking placeholders mean working.
//
// Ready and working are mutually exclusive textarea placeholders. Check ready first to avoid stale Thinking text in
// history, allowing stable visibleIdle to heal hook-only working into waiting. Yolo/bang placeholders carry no state
// and therefore preserve the previous result.
export function detectCrush(text: string): ScreenDetection {
  const lower = text.toLowerCase();

  // Asking permission dialog is the strongest blocker and overrides placeholders.
  if (lower.includes("permission required") || lower.includes("allow for session")) {
    return { state: "asking", visibleBlocker: true, visibleIdle: false, visibleWorking: false, skip: false };
  }

  // Ready placeholders definitively indicate idle and are checked before working.
  if (
    lower.includes("ready for instructions") ||
    lower.includes("ready!") ||
    lower.includes("ready...") ||
    lower.includes("ready?")
  ) {
    return { state: "waiting", visibleBlocker: false, visibleIdle: true, visibleWorking: false, skip: false };
  }

  // Working placeholders.
  if (
    lower.includes("working!") ||
    lower.includes("working...") ||
    lower.includes("brrrr") ||
    lower.includes("prrrr") ||
    lower.includes("processing...") ||
    lower.includes("thinking...")
  ) {
    return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: true, skip: false };
  }

  // No strong signal under yolo/bang or modal overlays; preserve prior state.
  return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: false };
}

// Claude detection ported from herdr claude_code.rs.

/**
 * Claude Code TUI screen detection corresponding to herdr's claude_code.rs.
 */
export function detectClaude(text: string): ScreenDetection {
  const lower = text.toLowerCase();
  const lines = text.split("\n");

  // Transcript viewer.
  if (isClaudeTranscript(lower)) {
    return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: true };
  }

  // Strong form blocker: selection controls after the final ruler.
  if (checkClaudeFormBlocker(lines)) {
    return { state: "asking", visibleBlocker: true, visibleIdle: false, visibleWorking: false, skip: false };
  }

  // Locate a prompt beginning with `❯` between the final two rulers.
  const promptInfo = findClaudePrompt(lines);

  // Working indicator above the prompt box.
  if (promptInfo && checkClaudeWorking(lines, promptInfo.abovePromptEnd)) {
    return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: true, skip: false };
  }

  // Working detection when no prompt box is present.
  if (!promptInfo && checkClaudeWorkingNoPrompt(lower)) {
    return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: true, skip: false };
  }

  // Permission/prompt blocker.
  if (checkClaudeBlocked(lower)) {
    return { state: "asking", visibleBlocker: true, visibleIdle: false, visibleWorking: false, skip: false };
  }

  // Idle when a prompt box exists without working or blocked signals.
  if (promptInfo) {
    return { state: "waiting", visibleBlocker: false, visibleIdle: true, visibleWorking: false, skip: false };
  }

  // Working fallback.
  return { state: "working", visibleBlocker: false, visibleIdle: false, visibleWorking: false, skip: false };
}

/** Detects the Claude transcript viewer. */
function isClaudeTranscript(lower: string): boolean {
  return (
    lower.includes("showing detailed transcript") &&
    lower.includes("ctrl+o to toggle")
  );
}

/** Detects a Claude form blocker from selection controls after the final ruler. */
function checkClaudeFormBlocker(lines: string[]): boolean {
  // Find the final ruler.
  let lastRulerIdx = -1;
  for (let i = lines.length - 1; i >= 0; i--) {
    if (lines[i].includes("───")) {
      lastRulerIdx = i;
      break;
    }
  }
  if (lastRulerIdx < 0) return false;

  // Content after the ruler.
  const afterRuler = lines
    .slice(lastRulerIdx + 1)
    .join("\n")
    .toLowerCase();

  // Require both enter-to-select and esc-to-cancel hints.
  return (
    afterRuler.includes("enter to select") &&
    afterRuler.includes("esc to cancel")
  );
}

/** Locates Claude's prompt box by finding a `❯` line between the final two rulers. */
function findClaudePrompt(
  lines: string[],
): { abovePromptEnd: number } | null {
  // Search backward for rulers.
  const rulers: number[] = [];
  for (let i = lines.length - 1; i >= 0 && rulers.length < 2; i--) {
    if (lines[i].includes("───")) {
      rulers.push(i);
    }
  }
  if (rulers.length < 2) return null;

  // rulers[0] is the bottom ruler; rulers[1] is the upper ruler.
  const [bottomRuler, topRuler] = rulers;

  // Find `❯` between the rulers.
  for (let i = topRuler + 1; i < bottomRuler; i++) {
    const trimmed = lines[i].trim();
    if (trimmed.startsWith("❯")) {
      return { abovePromptEnd: topRuler };
    }
  }

  return null;
}

/** Detects Claude work from an indicator above the prompt box. */
function checkClaudeWorking(lines: string[], aboveEnd: number): boolean {
  // Spinner characters used by Claude.
  const spinnerChars = /[·✱✲✳⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]/;

  for (let i = 0; i < aboveEnd; i++) {
    const line = lines[i];
    const lower = line.toLowerCase();

    if (
      lower.includes("esc to interrupt") ||
      lower.includes("ctrl+c to interrupt")
    ) {
      return true;
    }

    if (lower.includes("waiting for") && lower.includes("background agent")) {
      return true;
    }

    // Match a spinner plus ellipsis rather than specific action verbs.
    if (spinnerChars.test(line) && line.includes("…")) {
      return true;
    }
  }

  return false;
}

/** Detects Claude work when no prompt box exists. */
function checkClaudeWorkingNoPrompt(lower: string): boolean {
  return (
    lower.includes("esc to interrupt") ||
    lower.includes("ctrl+c to interrupt")
  );
}

/** Detects Claude permission/prompt blocking. */
function checkClaudeBlocked(lower: string): boolean {
  return (
    lower.includes("do you want to proceed?") ||
    lower.includes("would you like to proceed?") ||
    lower.includes("waiting for permission") ||
    lower.includes("tab to amend") ||
    lower.includes("ctrl+e to explain")
  );
}
