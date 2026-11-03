//! Backend command wrappers using Tauri IPC on desktop and WebSocket in browsers through the transport adapter.

import type { WatchInfo } from "./info";
import {
  invoke,
  isElectronShell,
  isTauri,
  ptyTeardown,
  readRecordingStream,
  spawnPty,
  type PtySpawnArgs,
  type PtySpawnResult,
} from "./transport";
import type { Session } from "../types";

/** PTY launch parameters, including a child session's first-launch prompt. */
export type SpawnArgs = PtySpawnArgs;

/** PTY launch result: PID and typed-session launch command, null for terminals or attachment. */
export type SpawnResult = PtySpawnResult;

/**
 * Starts or attaches to a PTY and streams output to xterm. Existing sessions attach and replay without resending
 * launch commands, returning null for `launch`.
 */
export function ptySpawn(
  args: SpawnArgs,
  onOutput: (bytes: Uint8Array) => void,
): Promise<SpawnResult> {
  // Send current data-theme so the backend sets COLORFGBG for TUIs that do not query OSC 11.
  const dark = document.documentElement.dataset.theme === "dark";
  return spawnPty({ ...args, dark }, onOutput);
}

/** Writes keyboard data. */
export function ptyWrite(sessionId: string, data: string): Promise<void> {
  return invoke("pty_write", { sessionId, data });
}

/**
 * Resizes the PTY only on explicit takeover, when unowned, or when this client owns sizing. Other requests are
 * ignored by the backend. `takeover` represents Fit This Window and defaults false.
 */
export function ptyResize(
  sessionId: string,
  cols: number,
  rows: number,
  opts?: { takeover?: boolean },
): Promise<void> {
  return invoke("pty_resize", {
    sessionId,
    cols,
    rows,
    takeover: opts?.takeover ?? false,
  });
}

/**
 * Requests a full-screen redraw by nudging SIGWINCH so a running full-screen TUI repaints. xterm term.refresh only
 * redraws its current buffer and cannot repair content rendered at wrong dimensions; the TUI must resend the screen.
 */
export function ptyRedraw(sessionId: string): Promise<void> {
  return invoke("pty_redraw", { sessionId });
}

/**
 * Why a session is being terminated.
 *
 * The backend puts this on `pty://killed/{id}`, which is the difference between the two cases that look
 * identical from outside: `restart` means the same session comes back with a new process, so every client
 * keeps its pane and waits; `close` means it is going away, so other clients close their view.
 */
export type KillReason = "close" | "restart";

/** Explicitly terminates a PTY session on either transport. */
export function ptyKill(
  sessionId: string,
  reason: KillReason = "close",
): Promise<void> {
  return invoke("pty_kill", { sessionId, reason });
}

// Sessions whose view is disappearing because this client is following a peer's mirrored layout, rather than
// because anyone here decided to close them. Marked just before the layout is applied and consumed by the
// unmount that follows within the same render pass; the window only bounds a stale mark after a failed apply.
const mirrorDetaches = new Map<string, number>();
const MIRROR_DETACH_WINDOW_MS = 5000;

/** Mark sessions as leaving this window's layout by mirror, not by a local close. */
export function markMirrorDetach(sessionIds: Iterable<string>): void {
  const now = Date.now();
  for (const [id, at] of mirrorDetaches) {
    if (now - at >= MIRROR_DETACH_WINDOW_MS) mirrorDetaches.delete(id);
  }
  for (const id of sessionIds) mirrorDetaches.set(id, now);
}

/**
 * Releases this client's session use on `usePtySession` unmount. Desktop kills; browser detaches without killing the shared process.
 *
 * The one exception is a mirrored removal. Desktop unmount normally means kill, which is right when someone
 * closes a tab here, but following a peer's layout must never end anyone's work — a remote client closing a tab
 * detaches for itself, and mirroring that decision back must not upgrade it into killing the process. The
 * session keeps running and stays reachable from the sidebar.
 */
export function ptyTeardownSession(sessionId: string): Promise<void> {
  const at = mirrorDetaches.get(sessionId);
  if (at !== undefined) {
    mirrorDetaches.delete(sessionId);
    if ((isTauri || isElectronShell) && Date.now() - at < MIRROR_DETACH_WINDOW_MS) return Promise.resolve();
  }
  return ptyTeardown(sessionId);
}

/** Queries a running session cwd for split-pane inheritance. */
export function getSessionCwd(sessionId: string): Promise<string | null> {
  return invoke<string | null>("get_session_cwd", { sessionId });
}

/** One selectable shell for terminal-session switching. */
export interface ShellOption {
  /** Stable frontend ID such as cmd, powershell, pwsh, gitbash, or wsl:<distribution>. */
  id: string;
  /** Display name such as PowerShell, Git Bash, or WSL: Ubuntu. */
  label: string;
  /** Executable path, command, or WSL launch specification persisted in the session shell field. */
  path: string;
  /** Whether this is the effective system default for sessions whose shell field is empty. */
  isDefault: boolean;
}

/**
 * Lists shells available for terminal sessions on this platform. Empty means no selector and a plain text fallback.
 */
export function listShells(): Promise<ShellOption[]> {
  return invoke<ShellOption[]>("list_shells");
}

/** Platform-specific agent installation guidance shown when an agent is missing. */
export interface AgentInstallRecipe {
  /** Agent display name. */
  label: string;
  /** Actual executable command name. */
  bin: string;
  /** Recommended platform command executable directly in the session shell. */
  command: string;
  /** Whether Node/npm is required, allowing the frontend to prompt for Node first. */
  needsNode: boolean;
  /** Official installation documentation. */
  docsUrl: string;
  /** Post-install authentication step supplied by the backend. */
  authHint: string;
}

/**
 * Gets installation guidance for a local agent on this platform; unknown or unsupported kinds return null.
 */
export function agentInstallRecipe(agent: string): Promise<AgentInstallRecipe | null> {
  return invoke<AgentInstallRecipe | null>("agent_install_recipe", { agent });
}

/**
 * Detects an installed agent at known platform locations and returns an absolute executable path only after stat
 * confirms it. Retry Start uses it to fill an empty executable-path setting; returns null rather than guessing.
 */
export function agentLocateBin(agent: string): Promise<string | null> {
  return invoke<string | null>("agent_locate_bin", { agent });
}

/** One-click install command to type into the session shell, with the token its completion marker carries. */
export interface AgentInstallRun {
  /** Recommended command followed by the step that leaves the completion marker. */
  command: string;
  /** Token for agentInstallFinished; null when the backend could not add the marker step. */
  token: string | null;
}

/**
 * Builds the one-click install command for an agent. The shell leaves a completion marker once the command
 * returns, so completion is never declared while the installer is still running. Null for unsupported kinds.
 */
export function agentInstallBegin(agent: string): Promise<AgentInstallRun | null> {
  return invoke<AgentInstallRun | null>("agent_install_begin", { agent });
}

/** Whether the install started with this token has returned. A found marker is consumed. */
export function agentInstallFinished(token: string): Promise<boolean> {
  return invoke<boolean>("agent_install_finished", { token });
}

/**
 * Runs the one-click install of an agent without a terminal interface on the host and resolves once it has
 * exited successfully. Takes minutes for a download; rejects with a fixed code when it fails or does not apply.
 */
export function agentInstallRun(agent: string): Promise<void> {
  return invoke<void>("agent_install_run", { agent });
}

/**
 * Asks an agent CLI to print the models it can run, normalized to bare identifiers.
 *
 * Agents whose CLI has no listing command return an empty array rather than an error. The call spawns
 * the CLI and waits for it, so treat it as slow and cache the result per agent kind.
 */
export function agentListModels(agent: string): Promise<string[]> {
  return invoke<string[]>("agent_list_models", { agent });
}

/** Bundled/full Git Bash installation state, meaningful only on Windows. */
export interface GitbashStatus {
  /** Whether any usable bundled-minimal or downloaded-full Git Bash exists. */
  available: boolean;
  /** Whether the full distribution including git/ssh is installed. */
  fullInstalled: boolean;
}

/** Queries Git Bash status; both values are false outside Windows. */
export function gitbashStatus(): Promise<GitbashStatus> {
  return invoke<GitbashStatus>("gitbash_status");
}

/**
 * Starts background download/install of full Git Bash on Windows and returns immediately; events report progress/results.
 */
export function downloadFullGitbash(): Promise<void> {
  return invoke<void>("download_full_gitbash");
}

/** Streams a terminal recording in chunks for archive playback and resolves at EOF. */
export function readRecording(
  sessionId: string,
  onBytes: (bytes: Uint8Array) => void,
): Promise<void> {
  return readRecordingStream(sessionId, onBytes);
}

/** One agent conversation message for archive reading, with adjacent roles merged by the backend. */
export interface TranscriptMessage {
  origin?: import("./chat").MessageOrigin;
  role: "user" | "assistant";
  text: string;
  /** Possibly empty ISO timestamp. */
  timestamp?: string | null;
  /** Ordered tools used in this assistant turn. */
  tools: string[];
}

/**
 * Reads a parsed Claude/Codex conversation. Rejects for missing/unsupported records so callers fall back to recording playback.
 */
export function readAgentTranscript(
  sessionId: string,
): Promise<TranscriptMessage[]> {
  return invoke<TranscriptMessage[]>("read_agent_transcript", { sessionId });
}

/**
 * One row of the session view: a message bubble, or a tool call folded together with its result.
 *
 * Richer than TranscriptMessage, which the archive reader uses: reasoning and tool calls are kept, and a
 * tool row carries its raw input so each card can show the fields that matter for that tool.
 */
export interface ChatEvent {
  /** Position in the list. Stable for a given transcript length; used as the React key. */
  index: number;
  /** `wake`: a finished background task that started a turn of its own; `text` holds its summary. */
  kind: "user" | "assistant" | "thinking" | "tool" | "command" | "wake";
  /** Message body. Absent on a tool row. */
  text?: string;
  /** Possibly empty ISO timestamp. */
  timestamp?: string | null;
  /** Tool name, on a tool row. */
  tool?: string;
  /** Raw tool input, shaped by the tool itself. */
  input?: unknown;
  /** Tool output text. Absent while the call is still pending. */
  output?: string;
  /** The tool reported a failure. */
  isError: boolean;
  /** The call has no result yet: still running, or the recording ends mid-call. */
  pending: boolean;
  /** Structured shell history, rendered read-only by recorded conversation views. */
  shell?: Extract<import("./chat").ChatRow, { kind: "shell" }>;
}

/**
 * Reads a parsed conversation for the session view. Rejects when the session has no readable recording
 * yet or the agent stores its history in a format we cannot parse, so callers can offer the terminal view.
 */
export function readAgentChat(sessionId: string): Promise<ChatEvent[]> {
  return invoke<ChatEvent[]>("read_agent_chat", { sessionId });
}

/**
 * Exports complete Claude/Codex session context as Markdown. Desktop `destPath` writes to disk and returns null;
 * browser receives content for local download. `exportedAt` is a preformatted header timestamp. Requires a captured ID.
 */
export function exportSessionContext(
  sessionId: string,
  destPath?: string,
  exportedAt?: string,
): Promise<string | null> {
  return invoke<string | null>("export_session_context", {
    sessionId,
    destPath,
    exportedAt,
  });
}

// Global session-content search.

/** One matching snippet from search.rs::SearchMatch. */
export interface SearchMatch {
  rowId?: string;
  role?: string | null;
  timestamp?: string | null;
  /** Transcript message index used as a scroll anchor; null for recordings. */
  messageIndex: number | null;
  /** Byte offset of the message's first record in its transcript file, which locates it exactly. */
  recordOffset?: number;
  /**
   * One-based match number within a transcript message or across the recording (the required findNext count).
   */
  ordinal: number;
  /** Readable contextual snippet preserving matched text for frontend highlighting. */
  snippet: string;
  /**
   * Literal strings the index matched in this fragment, in document order without duplicates: the whole
   * word or phrase as written in the text for word hits, or the query terms for substring hits. Highlight
   * these rather than the raw query, and use the first one for recording findNext.
   */
  matched: string[];
}

/** One matching session and its snippets from search.rs::SessionSearchHit. */
export interface SessionSearchHit {
  sessionId: string;
  name: string;
  kind: Session["kind"];
  /** Whether archived, determining read-only viewing and live-session availability. */
  archived: boolean;
  /** Source: conversation transcript or terminal recording. */
  source: "transcript" | "recording";
  /** True total hit count even when snippets are capped. */
  matchCount: number;
  /** Matching snippets, capped at 50 per session. */
  matches: SearchMatch[];
}

/** Search scope: live only (default), all, or archived only. */
export type SearchScope = "live" | "all" | "archived";

/**
 * Searches session history within SQL-filtered scope. Claude/Codex use transcripts; other sessions use ANSI-stripped
 * recordings. Empty terms return an empty array.
 */
export function searchSessionContent(
  query: string,
  scope: SearchScope = "live",
): Promise<SessionSearchHit[]> {
  return invoke<SessionSearchHit[]>("search_session_content", { query, scope });
}

export interface ContentSearchPage {
  jobId: string;
  status: "pending" | "complete";
  scannedSessions: number;
  scannedBytes: number;
  /** Sessions not brought up to date in time; their newest content may be missing from the results. */
  pendingSessions?: number;
  hits?: SessionSearchHit[];
  totalSessions?: number;
  totalMatches?: number;
  nextCursor?: number | null;
  previousCursor?: number | null;
}
export function startContentSearch(query: string, scope: SearchScope): Promise<ContentSearchPage> {
  return invoke("search_content_start", { query, scope });
}
export function contentSearchPage(jobId: string, cursor?: number, before?: number): Promise<ContentSearchPage> {
  return invoke("search_content_page", { jobId, cursor, before, limit: 10 });
}
export function cancelContentSearch(jobId: string): Promise<void> {
  return invoke("search_content_cancel", { jobId });
}

/** Info-panel model/context/current-tool snapshot. */
export interface AgentContextInfo {
  /** Actual model ID from the latest assistant row. */
  model: string | null;
  /** Current context tokens: input plus cache creation and cache hits. */
  contextTokens: number | null;
  /** Context limit: explicit [1m], inferred model limit, or 200k fallback. */
  contextLimit: number;
  /** Latest unfinished Codex tool call; Claude uses hook state. */
  currentTool: string | null;
}

/**
 * Queries model/context usage for Info. Rejects unsupported, uncaptured, or deleted transcripts so callers show a dash.
 */
export function agentContextInfo(sessionId: string): Promise<AgentContextInfo> {
  return invoke<AgentContextInfo>("agent_context_info", { sessionId });
}

/** One file the turn edited. Line counts sum every recorded edit to it; both are null when any edit was not countable. */
export interface TurnChangedFile {
  /** As the agent wrote it: absolute, or relative to the session folder. */
  path: string;
  additions: number | null;
  deletions: number | null;
}

/** Backend-owned statistics for the latest root turn and its recorded session. Missing data is null. */
export interface AgentTurnStats {
  /** Model the latest context reading came from, as the provider reports it. */
  model: string | null;
  /** Generated output, including reported reasoning. */
  tokens: number | null;
  inputTokens: number | null;
  totalTokens: number | null;
  cachedTokens: number | null;
  cacheHitPercent: number | null;
  sessionTokens: number | null;
  toolsUsed: number | null;
  filesTouched: number | null;
  /** The files behind `filesTouched`, sorted by path. */
  changedFiles: TurnChangedFile[] | null;
  contextTokens: number | null;
  contextLimit: number | null;
  contextPercent: number | null;
  /** Estimated output throughput over measured response time, including reported reasoning. */
  generationTokensPerSecond: number | null;
}

/** Queries provider usage and activity. Unsupported metrics are null; unreadable recordings reject. */
export function agentTurnStats(sessionId: string): Promise<AgentTurnStats> {
  return invoke<AgentTurnStats>("agent_turn_stats", { sessionId });
}

// Info-panel usage limits.

/** One Claude account quota window (five-hour or weekly). */
export interface UsageWindow {
  /** Used percentage, 0-100. */
  utilization: number;
  /** Possibly empty ISO 8601 reset time. */
  resetsAt: string | null;
}

/** Account-level Claude subscription quota shared by all Claude sessions. */
export interface ClaudeUsage {
  /** Five-hour rolling window. */
  fiveHour: UsageWindow | null;
  /** Seven-day total rolling window. */
  sevenDay: UsageWindow | null;
  /** Optional seven-day Opus-specific quota. */
  sevenDayOpus: UsageWindow | null;
  /** Per-model weekly quotas (for example Fable), from the API's `limits` array. */
  modelWeekly: ClaudeModelWindow[];
}

/** A weekly quota scoped to one model. */
export interface ClaudeModelWindow {
  /** Model display name as the API reports it, such as "Fable 5". */
  model: string;
  /** Used percentage, 0-100. */
  utilization: number;
  /** Possibly empty ISO 8601 reset time. */
  resetsAt: string | null;
}

/** One Codex primary short or secondary long rate-limit window. */
export interface CodexRateWindow {
  /** Used percentage, 0-100. */
  usedPercent: number;
  /** Window duration in minutes, used for frontend labels. */
  windowMinutes: number;
  /** Optional reset time in Unix seconds. */
  resetsAt: number | null;
}

/** Codex account rate-limit snapshot from live app-server or local rollout fallback. */
export interface CodexUsage {
  /** Undefined/null means the service did not supply an authoritative count. */
  resetCredits?: number | null;
  primary: CodexRateWindow | null;
  secondary: CodexRateWindow | null;
  /** Optional plan type such as free/plus/pro. */
  planType: string | null;
}

/** Grok SuperGrok usage pool (weekly by default; same source as Grok Build `/usage`). */
export interface GrokUsage {
  /** Used percentage of the current pool, 0–100. */
  usedPercent: number;
  /** Window label for the usage row, such as `7d` or `mo`. */
  windowLabel: string;
  /** ISO 8601 period end (pool reset). */
  periodEnd: string | null;
  /** ISO 8601 period start. */
  periodStart: string | null;
  /** Optional Grok Build share of the same weekly pool, 0–100. */
  buildPercent: number | null;
  /** Optional pay-as-you-go cap when configured (0 means disabled). */
  onDemandCap: number | null;
}

/** One ZCode plan quota window. */
export interface ZcodeWindow {
  /** Row label such as `5h` or `7d`; tool-call quotas are prefixed `mcp`. */
  label: string;
  /** Used percentage, 0-100. */
  usedPercent: number;
  /** Unix seconds of the next reset. */
  resetsAt: number | null;
}

/** GLM Coding Plan quota of the platform ZCode is signed in to. */
export interface ZcodeUsage {
  /** `zai` or `bigmodel`. */
  platform: string;
  /** Plan level as the server names it, such as `lite` or `pro`. */
  level: string | null;
  windows: ZcodeWindow[];
}

/** Which account a usage row comes from; matches the session kinds that publish a quota. */
export type UsageProvider = "claude" | "codex" | "grok" | "zcode";

/** One provider's stored reading, kept with the reason the last attempt failed. */
export interface UsageEntry<T> {
  /** Last successful reading, kept while later attempts fail so the panel does not blink empty. */
  data: T | null;
  /** Last failure message. With `data` present it marks the numbers stale; without, it replaces them. */
  error: string | null;
  /** Unix milliseconds of the last successful reading. */
  fetchedAt: number | null;
  /** Unix milliseconds of the failure in `error`, null once a later attempt succeeds. */
  errorAt: number | null;
}

/**
 * The one account-usage copy the backend keeps for this machine.
 *
 * A single background poller fills it, so opening ten sessions costs exactly as many provider queries as
 * opening one: none. Sessions read this snapshot and render the entry matching their kind.
 */
export interface UsageSnapshot {
  claude: UsageEntry<ClaudeUsage>;
  codex: UsageEntry<CodexUsage>;
  grok: UsageEntry<GrokUsage>;
  /** Absent in snapshots from a backend that predates the ZCode quota. */
  zcode?: UsageEntry<ZcodeUsage>;
  /** Whether automatic polling is on; the panel hides its countdown when it is off. */
  auto: boolean;
  /** Effective poll interval in seconds, used for the countdown to the next refresh. */
  intervalSec: number;
}

/** Reads the stored snapshot. Pure memory on the backend: no network and no subprocess. */
export function usageSnapshot(): Promise<UsageSnapshot> {
  return invoke<UsageSnapshot>("usage_snapshot", {});
}

/**
 * Refreshes now and returns the updated snapshot, which also reaches every other client as
 * `usage://changed`. `provider` narrows the work to one source; omitting it refreshes every provider
 * whose account exists on this machine. `force` additionally bypasses the backend's short TTL cache.
 */
export function usageRefresh(provider?: UsageProvider, force = false): Promise<UsageSnapshot> {
  return invoke<UsageSnapshot>("usage_refresh", { provider, force });
}

/** Worktree path, new branch, and full base-branch ref. */
export interface WorktreeInfo {
  path: string;
  branch: string;
  /** Full base ref from repository root when created; landing targets it. */
  baseRef: string;
}

/** Creates an isolated Git worktree for a spawned/manual child; errors let callers fall back without one. */
export function createWorktree(
  repoRoot: string,
  name: string,
): Promise<WorktreeInfo> {
  return invoke<WorktreeInfo>("create_worktree", { repoRoot, name });
}

/**
 * Render a model and effort choice into one agent's own command-line spelling.
 *
 * Every agent CLI spells this differently — some carry effort inside the model string — so the mapping
 * lives in one Rust table. Asking for it keeps a second, drifting copy out of the frontend.
 */
export function composeAgentArgs(
  kind: string,
  model: string | null,
  effort: string | null,
  extraArgs: string | null,
): Promise<string | null> {
  return invoke<string | null>("compose_agent_args", { kind, model, effort, extraArgs });
}

/** Existing Git worktree offered by Select Existing Worktree. */
export interface WorktreeEntry {
  /** Worktree top-level directory. */
  path: string;
  /** Current short branch name, null for detached HEAD. */
  branch: string | null;
  /** Current HEAD commit SHA. */
  head: string | null;
  /** Whether this is the main worktree, excluded from the selection list. */
  isMain: boolean;
  /** Whether this is a bare-repository entry. */
  isBare: boolean;
}

/** Lists all worktrees including main; non-repositories error so callers can use an empty list. */
export function listWorktrees(repoRoot: string): Promise<WorktreeEntry[]> {
  return invoke<WorktreeEntry[]>("list_worktrees", { repoRoot });
}

/** Removes a worktree directory; force allows removal with local changes. */
export function removeWorktree(path: string, force: boolean): Promise<void> {
  return invoke("remove_worktree", { path, force });
}

// Unified branch merging. The dialog lets users choose and reverse source/target directions.

/** Local branch short name and its checkout worktree, null when not checked out. */
export interface BranchEntry {
  name: string;
  checkoutDir: string | null;
}

/** Branch list for the merge dialog: repository state, cwd branch, and all local branches. */
export interface BranchList {
  isRepo: boolean;
  /** cwd short branch, null when detached or outside a repository. */
  current: string | null;
  branches: BranchEntry[];
}

/** Merge preflight: source-to-target direction, diff summary, both worktree dirty states, and availability. */
export interface MergeBranchesPreview {
  source: string;
  target: string;
  /** Summary of committed source changes plus uncommitted source worktree changes that would enter target. */
  diffStat: string;
  /** Source branch checkout directory, null when not checked out. */
  sourceDir: string | null;
  /** Whether source has uncommitted changes requiring a pre-merge commit message. */
  sourceDirty: boolean;
  /** Target checkout directory where merge executes; null means unavailable. */
  targetDir: string | null;
  /** Whether target has uncommitted changes; warn without blocking. */
  targetDirty: boolean;
  /** Source is already merged and clean, so no merge is needed. */
  upToDate: boolean;
  /** Whether local merge is available because target is checked out. */
  available: boolean;
  /** `"ok"` / `"same_branch"` / `"branch_not_found"` / `"target_not_checked_out"`。 */
  reason: string;
}

/** Merge result: success or conflicts, preserving conflict files for manual resolution. */
export interface MergeOutcome {
  merged: boolean;
  conflict: boolean;
  conflicts: string[];
  message: string;
}

/** Lists local branches and checkout locations for cwd; isRepo=false outside a repository. */
export function gitBranchList(cwd: string): Promise<BranchList> {
  return invoke<BranchList>("git_branch_list", { cwd });
}

/** Preflights a source-to-target merge. */
export function gitMergePreview(
  cwd: string,
  source: string,
  target: string,
): Promise<MergeBranchesPreview> {
  return invoke<MergeBranchesPreview>("git_merge_preview", { cwd, source, target });
}

/** Executes a merge, first committing dirty source changes with commitMessage, then merging in the target checkout. */
export function gitMergeApply(
  cwd: string,
  source: string,
  target: string,
  commitMessage: string | null,
): Promise<MergeOutcome> {
  return invoke<MergeOutcome>("git_merge_apply", { cwd, source, target, commitMessage });
}

// Change viewing and diffs.

/** The Git area selected for a comparison. */
export type GitChangeSide = "all" | "staged" | "unstaged" | "untracked" | "conflicted";

/** One changed file with independent index and worktree facts. */
export interface ChangedFile {
  /** Original repository-relative path for a rename. */
  originalPath?: string | null;
  /** Repository-root-relative path. */
  path: string;
  /** `"modified"` / `"added"` / `"deleted"` / `"untracked"` / `"renamed"`。 */
  status: string;
  additions: number;
  deletions: number;
  binary: boolean;
  /** Index-side state: `""` when nothing is staged, otherwise modified/added/deleted/renamed. */
  index: string;
  /** Worktree-side state: `""` when the worktree matches the index, otherwise the change or `"untracked"`. */
  worktree: string;
  /** Line counts of the staged part alone. */
  stagedAdditions: number;
  stagedDeletions: number;
  stagedBinary: boolean;
  /** Line counts of the not-yet-staged part alone. */
  unstagedAdditions: number;
  unstagedDeletions: number;
  unstagedBinary: boolean;
  /** For an unmerged path, which side did what. */
  conflict?: GitConflictKind | null;
}

export type GitConflictKind =
  | "both-modified" | "both-added" | "both-deleted"
  | "added-by-us" | "added-by-them" | "deleted-by-us" | "deleted-by-them";

/** Two sides of a file diff: HEAD versus worktree. */
export interface FileDiff {
  /** Text exceeds the viewer limit; this is not a binary file. */
  tooLarge?: boolean;
  path: string;
  /** HEAD content, empty for added/untracked files. */
  original: string;
  /** Worktree content, empty for deleted files. */
  modified: string;
  /** Binary/oversized files leave both sides empty and cannot be line-diffed. */
  binary: boolean;
}

/** Changed files of a repository, capped by the backend. */
export interface ChangedFileList {
  files: ChangedFile[];
  /** More changes exist than `files` holds; every line count is then zero. */
  truncated: boolean;
}

/** Lists files changed from HEAD for View Changes and the Git panel. */
export function gitChangedFiles(cwd: string): Promise<ChangedFileList> {
  return invoke<ChangedFileList>("git_changed_files", { cwd });
}

/** Folder that floods a cut-off change list and could be added to `.gitignore`, if one does. */
export function gitIgnoreSuggestion(cwd: string): Promise<string | null> {
  return invoke<string | null>("git_ignore_suggestion", { cwd });
}

/** Adds the suggested folder to the repository's root `.gitignore`; false when it is already listed. */
export function gitIgnoreFolder(cwd: string, folder: string): Promise<boolean> {
  return invoke<boolean>("git_ignore_folder", { cwd, folder });
}

/** A repository found below a folder that is not itself a repository. */
export interface NestedRepo {
  /** Absolute path of the repository's top-level directory. */
  path: string;
  /** Path relative to the scanned folder, for display. */
  name: string;
}

/** Lists repositories up to three levels below `path`, for the Git panel's repository picker. */
export function gitDiscoverRepos(path: string): Promise<NestedRepo[]> {
  return invoke<NestedRepo[]>("git_discover_repos", { path });
}

/** Gets HEAD/worktree text for line-by-line View Changes diff. */
export function gitFileDiff(cwd: string, path: string, side: GitChangeSide = "all"): Promise<FileDiff> {
  return invoke<FileDiff>("git_file_diff", { cwd, path, side: side === "untracked" ? "unstaged" : side === "conflicted" ? "all" : side });
}

/** One commit for the group Info panel's recent history. */
export interface CommitInfo {
  /** Full identity; older servers may only provide the abbreviated hash. */
  oid?: string;
  hash: string;
  subject: string;
  message?: string;
  author: string;
  relative: string;
  /** Parent identities; empty for a root commit, two or more for a merge. Absent from older servers. */
  parents?: string[];
  /** Ref names pointing at the commit, such as `HEAD -> main`, `origin/main`, or `tag: v1`. */
  refs?: string[];
}

/** Gets recent commits for cwd's repository; returns empty outside a repository or with no commits. */
export function gitRecentCommits(cwd: string): Promise<CommitInfo[]> {
  return invoke<CommitInfo[]>("git_recent_commits", { cwd });
}

// Git panel: staging, committing, discarding, and history browsing.

/** Stages the given repository-relative paths, untracked files and deletions included. */
export function gitStage(cwd: string, paths: string[]): Promise<void> {
  return invoke<void>("git_stage", { cwd, paths });
}

/** Unstages the given paths, leaving the worktree untouched. */
export function gitUnstage(cwd: string, paths: string[]): Promise<void> {
  return invoke<void>("git_unstage", { cwd, paths });
}

/**
 * Discards worktree changes: tracked files go back to the index, untracked files are deleted.
 * Staged content survives, so a discard can never throw away what the user already staged.
 */
export function gitDiscard(cwd: string, paths: string[]): Promise<void> {
  return invoke<void>("git_discard", { cwd, paths });
}

/** Commits what is staged and returns the new commit. */
export function gitCommit(cwd: string, message: string, amend = false, expectHead?: string | null): Promise<CommitInfo> {
  return invoke<CommitInfo>("git_commit", { cwd, message, amend, expectHead: expectHead ?? null });
}

/** How Pull integrates upstream commits: fast-forward only, rebase local commits, or merge. */
export type GitPullMode = "ff-only" | "rebase" | "merge";

/** Fetches from the default remote and prunes deleted remote branches. */
export function gitFetch(cwd: string): Promise<void> {
  return invoke<void>("git_fetch", { cwd });
}

/** Integrates the upstream branch into the current one. */
export function gitPull(cwd: string, mode: GitPullMode = "ff-only"): Promise<void> {
  return invoke<void>("git_pull", { cwd, mode });
}

/** Pushes the current branch, publishing it with upstream tracking when it has none. `force` uses --force-with-lease. */
export function gitPush(cwd: string, force = false): Promise<void> {
  return invoke<void>("git_push", { cwd, force });
}

/** What the current branch committed since it split from a base branch. */
export interface BranchCompare {
  /** Ref compared against, such as `origin/main`. */
  base: string;
  /** Where the histories split; the left side of each file diff. */
  mergeBase: string;
  /** HEAD at comparison time; the right side of each file diff. */
  head: string;
  /** Commits on this branch since the split. */
  ahead: number;
  /** Commits on the base since the split. */
  behind: number;
  files: ChangedFile[];
  /** Branches that can be chosen as the base. */
  candidates: string[];
}

/** Compares HEAD with `base`, or the default base; null when there is nothing to compare with. */
export function gitBranchCompare(cwd: string, base?: string | null): Promise<BranchCompare | null> {
  return invoke<BranchCompare | null>("git_branch_compare", { cwd, base: base ?? null });
}

/** Files changed between two commits. */
export function gitRangeFiles(cwd: string, from: string, to: string): Promise<ChangedFile[]> {
  return invoke<ChangedFile[]>("git_range_files", { cwd, from, to });
}

/** Both sides of one file between two commits. */
export function gitRangeFileDiff(cwd: string, from: string, to: string, path: string): Promise<FileDiff> {
  return invoke<FileDiff>("git_range_file_diff", { cwd, from, to, path });
}

/** Whether the Git panel can open a pull request here, and why not when it cannot. */
export interface GitPrProvider {
  provider: "gitea" | null;
  available: boolean;
  reason: "ok" | "not_gitea" | "no_token" | "unreachable";
}

export function gitPrProvider(cwd: string): Promise<GitPrProvider> {
  return invoke<GitPrProvider>("git_pr_provider", { cwd });
}

/** Result of opening a pull request: its URL (empty when the server gave none) and a message. */
export interface PrOutcome {
  url: string;
  message: string;
}

/** Pushes the current branch and opens a pull request into `base`. */
export function gitOpenPullRequest(cwd: string, base: string, title: string, body: string): Promise<PrOutcome> {
  return invoke<PrOutcome>("git_open_pull_request", { cwd, base, title, body });
}

/** Change-notification lease for the repository containing `cwd`, keyed by its top level. */
export function gitWatch(cwd: string): Promise<WatchInfo> {
  return invoke<WatchInfo>("git_watch", { cwd });
}

/** Aborts the merge, rebase, cherry-pick, or revert that stopped on conflicts. */
export function gitAbortOperation(cwd: string): Promise<void> {
  return invoke<void>("git_abort_operation", { cwd });
}

/** Continues the stopped operation once its conflicts are resolved and staged. */
export function gitContinueOperation(cwd: string): Promise<void> {
  return invoke<void>("git_continue_operation", { cwd });
}

/** Total commits reachable from HEAD; zero outside a repository or before the first commit. */
export function gitCommitCount(cwd: string): Promise<number> {
  return invoke<number>("git_commit_count", { cwd });
}

/** One page of history, newest first. */
export function gitLogPage(cwd: string, limit: number, offset: number, head?: string | null): Promise<CommitInfo[]> {
  return invoke<CommitInfo[]>("git_log_page", { cwd, limit, offset, head });
}

/** Files changed against a commit's first parent, or the empty tree for its first commit. */
export function gitCommitFiles(cwd: string, hash: string): Promise<ChangedFile[]> {
  return invoke<ChangedFile[]>("git_commit_files", { cwd, hash });
}

/** Both sides of one file's diff inside a commit: the parent's content versus the commit's. */
export function gitCommitFileDiff(cwd: string, hash: string, path: string): Promise<FileDiff> {
  return invoke<FileDiff>("git_commit_file_diff", { cwd, hash, path });
}

// Gitea integration, phase two.

/** Gitea integration status for settings, without plaintext token. */
export interface GiteaStatus {
  configured: boolean;
  baseUrl: string | null;
  hasToken: boolean;
}

/** Test result from /version platform and /user token checks. */
export interface GiteaProbe {
  ok: boolean;
  version: string | null;
  user: string | null;
  message: string;
}

/** Queries Gitea status without returning the token. */
export function giteaGetStatus(): Promise<GiteaStatus> {
  return invoke<GiteaStatus>("gitea_get_status");
}

/** Saves Gitea base URL/token and returns updated status. */
export function giteaSetConfig(baseUrl: string, token: string): Promise<GiteaStatus> {
  return invoke<GiteaStatus>("gitea_set_config", { baseUrl, token });
}

/** Tests platform and token without persisting. */
export function giteaProbe(baseUrl: string, token: string): Promise<GiteaProbe> {
  return invoke<GiteaProbe>("gitea_probe", { baseUrl, token });
}

// The land_gitea_pr backend remains, but phase-one unified merge removed old frontend binding. Reconnect it as an
// Open PR option in the next dialog revision.

// ─────────────────────────── Spawn Skill ───────────────────────────

export function spawnSkillsInstalled(): Promise<boolean> {
  return invoke<boolean>("spawn_skills_installed");
}

export function installSpawnSkills(): Promise<void> {
  return invoke("install_spawn_skills");
}

export function uninstallSpawnSkills(): Promise<void> {
  return invoke("uninstall_spawn_skills");
}

export interface SpawnReceipt {
  resolvedPlanExecute?: import("./launch").PlanExecuteConfig | null;
  requestId: string;
  request: import("./events").SpawnRequest;
  decision: "pending" | "confirmed" | "cancelled";
  state: "pending" | "ready" | "waiting" | "failed" | "dispatching" | "uncertain" | "complete" | "cancelled";
  sessionId: string;
  messageId: string;
  session: import("../types").Session | null;
  error: string | null;
}
/** Decisions and execution results are persisted together on the backend. */
export const resolveSpawn = (requestId: string, confirmed: boolean, request?: import("./events").SpawnRequest) =>
  invoke<SpawnReceipt>("resolve_spawn", { requestId, confirmed, request });
export const spawnRequests = () => invoke<SpawnReceipt[]>("spawn_requests");
export const spawnRequest = (requestId: string) => invoke<SpawnReceipt>("spawn_request", { requestId });
export const retrySpawn = (requestId: string) => invoke<SpawnReceipt>("spawn_retry", { requestId });
export const prepareSpawn = (requestId: string, kind?: string) =>
  invoke<{ kind: string; cwd: string | null; model: string | null; effort: string | null }>("spawn_prepare", { requestId, kind });

// Pasted-image cleanup.

/** Clear Now removes vlx-uploads temporary images and returns deleted count plus freed bytes. */
export function cleanPastedImages(): Promise<{ removed: number; freedBytes: number }> {
  return invoke<{ removed: number; freedBytes: number }>("clean_pasted_images");
}

// Version consistency.

/**
 * Gets backend CARGO_PKG_VERSION and compares it at frontend startup with package.json `__APP_VERSION__`. A mismatch
 * indicates build drift and errors. Tauri and WS dispatch share the same source.
 */
export function getBackendVersion(): Promise<string> {
  return invoke<string>("app_version");
}

export type CodexResetOutcome = "reset" | "alreadyRedeemed" | "nothingToReset" | "noCredit";

export function codexResetCreditConsume(idempotencyKey: string): Promise<{ outcome: CodexResetOutcome; usage: UsageSnapshot }> {
  return invoke("codex_reset_credit_consume", { idempotencyKey });
}
