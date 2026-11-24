//! In-memory store and formatting helpers for inline diff review comments, plus resolution of the
//! agent session a review batch can be handed off to.
//!
//! Comments are grouped by comparison scope (`cwd`, `side`, `commit`, `base`) so switching between
//! files in `ChangesView` or between `DiffDocView` and `ChangesView` preserves all drafted feedback
//! until the user sends or clears it.

import { create } from "zustand";
import { genId } from "../../genId";
import type { useT } from "../../i18n";
import { chatStart } from "../../ipc/chat";
import { ptyWrite, type GitChangeSide } from "../../ipc/commands";
import { createSubmission, deliverSubmission } from "../../layout/CenterPane/session/outbox";
import { kindLabel } from "../../layout/sessionMenuShared";
import { useTermStore } from "../../store/termStore";
import type { Project, Session, SessionRuntime } from "../../types";

export type DiffCommentSide = "original" | "modified";

export interface DiffReviewComment {
  id: string;
  path: string;
  side: DiffCommentSide;
  /** 1-based inclusive start line on `side`. */
  startLine: number;
  /** 1-based inclusive end line on `side`. */
  endLine: number;
  /** Source lines quoted from `side` at the time the comment was saved. */
  snippet: string;
  /** Reviewer feedback text. */
  body: string;
}

export interface ReviewScopeInput {
  cwd: string;
  side: GitChangeSide;
  commit: string | null;
  base?: { from: string; label: string } | null;
}

const EMPTY_COMMENTS: DiffReviewComment[] = [];

export function normalizeRepoPath(path: string): string {
  return path.replace(/\\/g, "/").replace(/\/+$/, "");
}

/** Stable key identifying one repository comparison across tabs and file switches. */
export function reviewScopeKey(target: ReviewScopeInput): string {
  const cwd = normalizeRepoPath(target.cwd);
  const commit = target.commit ?? "";
  const base = target.commit && target.base ? target.base.from : "";
  const side = target.commit ? "commit" : target.side;
  return `${cwd}::${side}::${base}::${commit}`;
}

/** Format a 1-based inclusive line span as `L12` or `L10–L15`. */
export function formatLineRange(startLine: number, endLine: number): string {
  return startLine === endLine ? `L${startLine}` : `L${startLine}–L${endLine}`;
}

/** Extract 1-based lines `[startLine..endLine]`, capping long selections to keep prompts concise. */
export function extractLineSnippet(
  source: string,
  startLine: number,
  endLine: number,
  maxLines = 14,
): string {
  if (!source) return "";
  const all = source.split(/\r?\n/);
  const from = Math.max(1, Math.min(startLine, all.length));
  const to = Math.max(from, Math.min(endLine, all.length));
  const slice = all.slice(from - 1, to);
  if (slice.length <= maxLines) return slice.join("\n");
  return [...slice.slice(0, maxLines), "…"].join("\n");
}

function fenceFor(text: string): string {
  let len = 3;
  const matches = text.match(/`{3,}/g);
  if (matches) {
    for (const m of matches) {
      if (m.length >= len) len = m.length + 1;
    }
  }
  return "`".repeat(len);
}

function languageTagForPath(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? path;
  const dot = name.lastIndexOf(".");
  if (dot <= 0 || dot === name.length - 1) return "";
  const ext = name.slice(dot + 1).toLowerCase();
  return /^[a-z0-9+-]{1,12}$/.test(ext) ? ext : "";
}

type TranslateFn = ReturnType<typeof useT>;

/** Render a batch of inline diff comments as structured Markdown suitable for any coding agent. */
export function formatReviewPrompt(
  comments: readonly DiffReviewComment[],
  t: TranslateFn,
): string {
  if (comments.length === 0) return "";
  const blocks = comments.map((c, index) => {
    const sideLabel =
      c.side === "original" ? t("changes.review.sideOriginal") : t("changes.review.sideModified");
    const range = formatLineRange(c.startLine, c.endLine);
    const heading = `### ${index + 1}. \`${c.path}\` (${sideLabel}, ${range})`;
    const parts = [heading];
    if (c.snippet.trim().length > 0) {
      const fence = fenceFor(c.snippet);
      const lang = languageTagForPath(c.path);
      parts.push(`${fence}${lang}\n${c.snippet}\n${fence}`);
    }
    parts.push(`${t("changes.review.promptComment")} ${c.body.trim()}`);
    return parts.join("\n");
  });
  return `${t("changes.review.promptHeader")}\n\n${blocks.join("\n\n")}`;
}

interface DiffReviewState {
  scopes: Record<string, DiffReviewComment[]>;
  lastActiveSessionId: string | null;
  addComment: (scope: string, input: Omit<DiffReviewComment, "id">) => DiffReviewComment;
  updateComment: (scope: string, id: string, body: string) => void;
  removeComment: (scope: string, id: string) => void;
  clearScope: (scope: string) => void;
  rememberSession: (sessionId: string) => void;
}

export const useDiffReviewStore = create<DiffReviewState>((set) => ({
  scopes: {},
  lastActiveSessionId: null,
  addComment: (scope, input) => {
    const item: DiffReviewComment = { ...input, id: genId(), body: input.body.trim() };
    set((state) => ({
      scopes: {
        ...state.scopes,
        [scope]: [...(state.scopes[scope] ?? EMPTY_COMMENTS), item],
      },
    }));
    return item;
  },
  updateComment: (scope, id, body) =>
    set((state) => {
      const list = state.scopes[scope];
      if (!list) return state;
      return {
        scopes: {
          ...state.scopes,
          [scope]: list.map((c) => (c.id === id ? { ...c, body: body.trim() } : c)),
        },
      };
    }),
  removeComment: (scope, id) =>
    set((state) => {
      const list = state.scopes[scope];
      if (!list) return state;
      const next = list.filter((c) => c.id !== id);
      const scopes = { ...state.scopes };
      if (next.length === 0) delete scopes[scope];
      else scopes[scope] = next;
      return { scopes };
    }),
  clearScope: (scope) =>
    set((state) => {
      if (!(scope in state.scopes)) return state;
      const scopes = { ...state.scopes };
      delete scopes[scope];
      return { scopes };
    }),
  rememberSession: (sessionId) => set({ lastActiveSessionId: sessionId }),
}));

export function useScopeComments(scope: string | undefined): DiffReviewComment[] {
  return useDiffReviewStore((s) => (scope ? s.scopes[scope] ?? EMPTY_COMMENTS : EMPTY_COMMENTS));
}

// Track the most recent focused session so opening a diff tab (which clears activeSessionId) still
// defaults review delivery to the session the user was just working in. Guard `.subscribe` so unit
// tests that stub `useTermStore` as a plain selector function continue to work.
const maybeStore = useTermStore as unknown as {
  getState?: () => { activeSessionId?: string | null };
  subscribe?: (listener: (state: { activeSessionId?: string | null }) => void) => () => void;
};
if (typeof maybeStore.getState === "function") {
  const initial = maybeStore.getState()?.activeSessionId;
  if (initial) useDiffReviewStore.getState().rememberSession(initial);
}
if (typeof maybeStore.subscribe === "function") {
  maybeStore.subscribe((state) => {
    if (state.activeSessionId) {
      useDiffReviewStore.getState().rememberSession(state.activeSessionId);
    }
  });
}

export interface ReviewSessionCandidate {
  session: Session;
  /** Display name including agent kind and session title. */
  label: string;
  /** Short agent kind name for action buttons, e.g. `Claude` or `Codex`. */
  agentLabel: string;
  /** True when delivery queues/sends directly; false when pasted into a running terminal. */
  sends: boolean;
}

/** Find agent sessions in `cwd`'s repository/worktree that can accept a review handoff right now. */
export function resolveReviewCandidates(
  cwd: string,
  sessions: readonly Session[],
  projects: readonly Project[],
  runtimes: Readonly<Record<string, SessionRuntime | undefined>>,
  preferredId: string | null,
): ReviewSessionCandidate[] {
  const target = normalizeRepoPath(cwd);
  if (!target) return [];
  const projectRoots = new Map<string, string>();
  for (const p of projects) {
    if (p.rootPath) projectRoots.set(p.id, normalizeRepoPath(p.rootPath));
  }

  const scored: { score: number; preferred: number; running: number; candidate: ReviewSessionCandidate }[] = [];
  for (const s of sessions) {
    if (s.archivedAt || s.kind === "terminal" || s.kind === "browser") continue;
    const isRunning = runtimes[s.id]?.status === "running";
    const isChat = s.engine === "chat";
    if (!isChat && !isRunning) continue;

    const projRoot = projectRoots.get(s.projectId) ?? "";
    const sessionDir = normalizeRepoPath(s.worktreePath || s.cwd || projRoot);
    let score = 0;
    if (sessionDir && sessionDir === target) {
      score = 3;
    } else if (
      sessionDir &&
      (sessionDir.startsWith(target + "/") || target.startsWith(sessionDir + "/"))
    ) {
      score = 2;
    } else if (projRoot && (projRoot === target || target.startsWith(projRoot + "/"))) {
      score = 1;
    }
    if (score === 0) continue;

    const agentLabel = kindLabel(s.kind);
    const title = s.name?.trim() ? `${agentLabel} · ${s.name.trim()}` : agentLabel;
    scored.push({
      score,
      preferred: s.id === preferredId ? 1 : 0,
      running: isRunning ? 1 : 0,
      candidate: {
        session: s,
        label: title,
        agentLabel,
        sends: isChat,
      },
    });
  }

  scored.sort((a, b) =>
    b.score - a.score ||
    b.preferred - a.preferred ||
    b.running - a.running ||
    Number(b.candidate.sends) - Number(a.candidate.sends),
  );
  return scored.map((entry) => entry.candidate);
}

/** Deliver formatted review text to `candidate` and reveal its session tab. */
export async function deliverReviewToCandidate(
  candidate: ReviewSessionCandidate,
  text: string,
): Promise<void> {
  const id = candidate.session.id;
  const storeState = (useTermStore as unknown as { getState?: () => { openSession?: (sid: string) => void } }).getState?.();
  storeState?.openSession?.(id);
  useDiffReviewStore.getState().rememberSession(id);
  if (candidate.sends) {
    const item = createSubmission(id, text, [], "queue");
    await deliverSubmission(id, item, () => chatStart(id));
  } else {
    await ptyWrite(id, `\x1b[200~${text}\x1b[201~`);
  }
}
