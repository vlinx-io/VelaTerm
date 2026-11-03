//! Development diagnostics: toggleable logging plus tab/pane state-invariant checks.
//!
//! Enabled automatically in development builds (import.meta.env.DEV) and disabled by default in production.
//! For temporary production diagnosis, run localStorage.setItem("vlx-debug","1") in the console and refresh;
//! disable with localStorage.removeItem("vlx-debug").

import {
  collectSessionIds,
  findBySession,
  type PaneNode,
} from "./layout/CenterPane/paneTree";

function readFlag(): boolean {
  try {
    if (import.meta.env?.DEV) return true;
    return localStorage.getItem("vlx-debug") === "1";
  } catch {
    return false;
  }
}

/** Master diagnostics switch. When false in production, every dlog/check is a no-op. */
export const DEBUG = readFlag();

/** Toggleable debug log emitted only under DEBUG with a consistent filterable prefix. */
export function dlog(...args: unknown[]): void {
  if (DEBUG) console.log("[vlx] diagnostic", { argumentCount: args.length });
}

/** Minimal state view required by checkTabInvariants, avoiding a reverse dependency on store types. */
interface TabStateView {
  openTabs: string[];
  paneTrees: Record<string, PaneNode>;
  activeTabId: string | null;
  activeSessionId: string | null;
  sessions: { id: string }[];
  ephemeralSessions: Record<string, unknown>;
  /** Document-tab metadata; valid tabs without pane trees are exempted from invariants 1/2 through this map. */
  docTabs: Record<string, unknown>;
  /** Browser, task and New Session tabs also live in openTabs without a pane tree. */
  browserTabs?: Record<string, unknown>;
  taskTabs?: Record<string, unknown>;
  newSessionTabs?: Record<string, unknown>;
}

const hasTabContent = (s: TabStateView, id: string) => !!(s.paneTrees[id] || s.docTabs[id]
  || s.browserTabs?.[id] || s.taskTabs?.[id] || s.newSessionTabs?.[id]);

/**
 * Validate invariants that must hold after closing/switching tabs. Any violation directly produces a blank center
 * pane, so this acts as a sentinel: console.error with a state snapshot on failure, dlog on success.
 */
export function checkTabInvariants(label: string, s: TabStateView): void {
  if (!DEBUG) return;
  const problems: string[] = [];

  // 1) Every openTab must have a pane tree (terminal tab) **or** document metadata in docTabs.
  for (const id of s.openTabs) {
    if (!hasTabContent(s, id)) {
      problems.push(`openTab "${id}" has no paneTree and is not a doc tab`);
    }
  }

  // 2) activeTabId is null or references an openTab with a tree, document, browser, task or New Session tab; otherwise the center pane is blank.
  if (s.activeTabId != null) {
    if (!s.openTabs.includes(s.activeTabId)) {
      problems.push(`activeTabId "${s.activeTabId}" is not in openTabs`);
    } else if (!hasTabContent(s, s.activeTabId)) {
      problems.push(
        `activeTabId "${s.activeTabId}" has no paneTree and is not a doc tab (center pane will blank out)`,
      );
    }
  }

  // 2b) Every docTabs key must appear in openTabs, preventing orphaned metadata leaks.
  for (const id of Object.keys(s.docTabs)) {
    if (!s.openTabs.includes(id)) {
      problems.push(`docTab "${id}" is not in openTabs (orphan doc tab metadata)`);
    }
  }

  // 3) Every session in the active tree must exist in sessions/ephemeralSessions or its pane renders empty.
  const at = s.activeTabId ? s.paneTrees[s.activeTabId] : null;
  if (at) {
    const known = new Set([
      ...s.sessions.map((x) => x.id),
      ...Object.keys(s.ephemeralSessions),
    ]);
    for (const sid of collectSessionIds(at)) {
      if (!known.has(sid)) {
        problems.push(`active-tree session "${sid}" not in sessions/ephemeral (pane renders blank)`);
      }
    }
    if (s.activeSessionId && !findBySession(at, s.activeSessionId)) {
      problems.push(`activeSessionId "${s.activeSessionId}" is not in the active tree`);
    }
  }

  const snap = {
    openTabs: s.openTabs,
    activeTabId: s.activeTabId,
    activeSessionId: s.activeSessionId,
    paneTreeKeys: Object.keys(s.paneTrees),
    docTabKeys: Object.keys(s.docTabs),
  };
  if (problems.length) {
    console.error("[vlx] tab invariant violated", { problemCount: problems.length, tabCount: snap.openTabs.length });
  } else {
    dlog(`after ${label}`, snap);
  }
}
