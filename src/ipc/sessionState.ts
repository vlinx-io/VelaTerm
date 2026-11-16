//! Client access to the backend's authoritative session records.
//!
//! A session's facts — is there a result nobody has read, what is the agent doing — belong to the
//! session, not to the window it happens to be shown in. Each client used to infer them for itself from
//! raw events, so two clients could disagree and both be right by their own reasoning. These calls read
//! the backend's answer and report observations back to it; they never state a conclusion.
//!
//! None of these commands belong in `DIRECT_DESKTOP_CMDS`: staying out of that list is what routes them
//! through `desktop_call`, so the desktop shell and a remote browser travel the same path to the same
//! implementation.

import { invoke } from "./transport";
import type { AgentKind, AgentState } from "../types";
import type { BackgroundRun } from "./runs";

/** One session's authoritative record. Absent from a batch means "nothing known", not "false". */
export interface SessionStateRecord {
  /** A result is waiting for someone to look at it. */
  unread?: boolean;
  /** The marker was set by hand from the session menu; clients show it but pop no system notification. */
  unreadManual?: boolean;
  /** Which agent is running here; null or absent for a plain terminal. */
  agent?: AgentKind | null;
  /** What that agent is doing. Meaningless without an agent. */
  agentState?: AgentState | null;
  /** Activity source chosen at launch: `chat` for a conversation-view session, `hooks` or `legacy` for a Codex
   *  terminal. Other terminal agents leave it unset. */
  stateSource?: "hooks" | "legacy" | "chat" | null;
  /** Whether modern Codex has completed the handshake proving its hooks actually run. */
  hookReady?: boolean;
  /** A source covering the full turn lifecycle has reported; guesses may not override it. */
  authoritative?: boolean;
  /** This session has worked at least once, the threshold screen detection needs. */
  everWorked?: boolean;
  /** A process is running behind this session right now. */
  alive?: boolean;
  /** Commands this session started with `vrun` that are still running, oldest first. */
  runs?: BackgroundRun[];
  /** Milliseconds since the epoch of the record's last change. */
  updatedAt?: number;
}

/** Every record the backend holds, keyed by session ID. */
export type SessionStateBatch = Record<string, SessionStateRecord>;

/**
 * Read every record at once.
 *
 * Call this when a client connects and again after a reconnect: broadcasts that landed while the socket
 * was down are not replayed, so only a full read closes that gap.
 */
export function sessionStates(): Promise<SessionStateBatch> {
  return invoke<SessionStateBatch>("session_states");
}

/**
 * Report that someone has now read this session, clearing its marker on **every** client.
 *
 * That is the point of moving the marker to the backend: reading a reply in the browser has to clear
 * the dot on the desktop, or the desktop keeps insisting the session still matches an unread filter.
 */
export function markSessionRead(sessionId: string): Promise<void> {
  return invoke<void>("session_mark_read", { sessionId });
}

/**
 * Report something worth a look that the backend cannot see for itself.
 *
 * Only screen detection needs this: it reads the rendered terminal grid, which exists solely in the
 * client. Once it reports through `session_report_screen` and the backend raises the marker from its own
 * arbitration, this call goes away.
 */
export function markSessionUnread(sessionId: string): Promise<void> {
  return invoke<void>("session_mark_unread", { sessionId });
}

/**
 * Mark this session Pending by hand, raising its unread marker on every client.
 *
 * Unlike `markSessionUnread`, the marker is flagged as manual, so no client pops a system notification
 * for it: marking a batch from the menu must not fire one notification per session elsewhere.
 */
export function markSessionPending(sessionId: string): Promise<void> {
  return invoke<void>("session_mark_pending", { sessionId });
}

/** What a client saw on a session's rendered terminal screen. Mirrors the backend's `ScreenReport`. */
export interface ScreenReport {
  state: AgentState;
  visibleBlocker: boolean;
  visibleWorking: boolean;
  skip: boolean;
}

/**
 * Report what the rendered terminal screen shows, and let the backend decide what it means.
 *
 * The reading cannot move to the backend: it needs a laid-out grid — the box-drawing rules, the prompt
 * glyph, the spinner frames — which exists only where a terminal is actually rendered. The *arbitration*
 * can, and has, so both clients now reach the same conclusion from the same reading.
 *
 * Only the session's size owner may report. Everyone else renders a scaled copy of the owner's grid, so
 * their reading describes the copy rather than the session; the backend refuses those.
 */
export function reportScreen(
  sessionId: string,
  screen: ScreenReport,
): Promise<void> {
  return invoke<void>("session_report_screen", { sessionId, screen });
}

/**
 * Report that the user interrupted the session with Ctrl+C or a bare Esc.
 *
 * Agents other than Codex emit no completion event after an interrupt, so the turn would otherwise sit
 * on `working` until something else corrected it. This used to be applied locally, which meant the other
 * client never learned the turn had ended.
 */
export function reportInterrupt(sessionId: string): Promise<void> {
  return invoke<void>("session_report_interrupt", { sessionId });
}
