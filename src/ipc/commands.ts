//! Backend command wrappers using Tauri IPC on desktop and WebSocket in browsers through the transport adapter.

import {
  invoke,
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

// Track recently killed sessions locally. The initiator also receives broadcast pty://killed; a late event after a
// restart's epoch remount could close the new view because its disposed flag is false. Ignore our own events for 3s.
const localKills = new Map<string, number>();
const LOCAL_KILL_WINDOW_MS = 3000;

/** Whether this client killed the session within three seconds, used to ignore its own broadcast. */
export function wasKilledLocally(sessionId: string): boolean {
  const t = localKills.get(sessionId);
  return t !== undefined && Date.now() - t < LOCAL_KILL_WINDOW_MS;
}

/** Explicitly terminates a PTY session on either transport. */
export function ptyKill(sessionId: string): Promise<void> {
  // Record before invoking because the event may arrive before resolution; prune expired entries.
  const now = Date.now();
  for (const [id, t] of localKills) {
    if (now - t >= LOCAL_KILL_WINDOW_MS) localKills.delete(id);
  }
  localKills.set(sessionId, now);
  return invoke("pty_kill", { sessionId });
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
    if (isTauri && Date.now() - at < MIRROR_DETACH_WINDOW_MS) return Promise.resolve();
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

export type CodexHookSource =
  | "system"
  | "user"
  | "project"
  | "mdm"
  | "sessionFlags"
  | "plugin"
  | "cloudRequirements"
  | "cloudManagedConfig"
  | "legacyManagedConfigFile"
  | "legacyManagedConfigMdm"
  | "velaterm"
  | "unknown";

export type CodexHookTrustStatus = "managed" | "untrusted" | "trusted" | "modified";

export interface CodexHook {
  key: string;
  eventName: string;
  handlerType: string;
  matcher: string | null;
  command: string | null;
  timeoutSec: number;
  statusMessage: string | null;
  sourcePath: string;
  source: CodexHookSource;
  pluginId: string | null;
  displayOrder: number;
  enabled: boolean;
  currentHash: string;
  trustStatus: CodexHookTrustStatus;
  isManaged: boolean;
}

export interface CodexHookListEntry {
  cwd: string;
  hooks: CodexHook[];
  warnings: string[];
  errors: { message: string; path: string }[];
}

export interface CodexHooksResponse {
  data: CodexHookListEntry[];
}

export interface CodexHooksListArgs {
  cwds: string[];
  codexPath?: string;
}

export interface CodexHookUpdateArgs extends CodexHooksListArgs {
  key: string;
  enabled?: boolean;
  trustedHash?: string;
}

export function codexHooksList(args: CodexHooksListArgs): Promise<CodexHooksResponse> {
  return invoke<CodexHooksResponse>("codex_hooks_list", { ...args });
}

export function codexHookUpdate(args: CodexHookUpdateArgs): Promise<CodexHooksResponse> {
  return invoke<CodexHooksResponse>("codex_hook_update", { ...args });
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
  /** Transcript message index used as a scroll anchor; null for recordings. */
  messageIndex: number | null;
  /**
   * One-based match number within a transcript message or across the recording (the required findNext count).
   */
  ordinal: number;
  /** Readable contextual snippet preserving matched text for frontend highlighting. */
  snippet: string;
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

/** Claude current-turn generated output, tool calls, and changed files after the final user message. */
export interface AgentTurnStats {
  /** Total output tokens generated this turn. */
  tokens: number;
  /** Tool-call count this turn. */
  toolsUsed: number;
  /** Distinct files changed this turn. */
  filesTouched: number;
}

/** Queries current-turn stats, rejecting non-Claude, uncaptured, or deleted transcripts. */
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
}

/**
 * Queries Claude account quota. `force` bypasses backend TTL for manual refresh; normal calls must use cache because
 * the endpoint is tightly rate-limited. Rejects unavailable states so callers hide the section.
 */
export function claudeUsage(force = false): Promise<ClaudeUsage> {
  return invoke<ClaudeUsage>("claude_usage", { force });
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
  primary: CodexRateWindow | null;
  secondary: CodexRateWindow | null;
  /** Optional plan type such as free/plus/pro. */
  planType: string | null;
}

/**
 * Queries Codex account limits. `live=true` asks app-server and falls back to rollout; false reads only local snapshot
 * for high-frequency in-turn refreshes.
 */
export function codexUsage(sessionId: string, live = false): Promise<CodexUsage> {
  return invoke<CodexUsage>("codex_usage", { sessionId, live });
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

/**
 * Queries Grok SuperGrok credit usage (weekly pool when available). `force` bypasses the backend TTL
 * cache for manual refresh. Requires `grok login` OIDC credentials under `~/.grok/auth.json`.
 */
export function grokUsage(force = false): Promise<GrokUsage> {
  return invoke<GrokUsage>("grok_usage", { force });
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

/** Lists worktrees associated with a session and descendants for deletion confirmation. */
export function worktreesInSubtree(sessionId: string): Promise<string[]> {
  return invoke<string[]>("worktrees_in_subtree", { sessionId });
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

/** One file changed relative to HEAD. */
export interface ChangedFile {
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
}

/** Two sides of a file diff: HEAD versus worktree. */
export interface FileDiff {
  path: string;
  /** HEAD content, empty for added/untracked files. */
  original: string;
  /** Worktree content, empty for deleted files. */
  modified: string;
  /** Binary/oversized files leave both sides empty and cannot be line-diffed. */
  binary: boolean;
}

/** Lists files changed from HEAD for View Changes. */
export function gitChangedFiles(cwd: string): Promise<ChangedFile[]> {
  return invoke<ChangedFile[]>("git_changed_files", { cwd });
}

/** Gets HEAD/worktree text for line-by-line View Changes diff. */
export function gitFileDiff(cwd: string, path: string): Promise<FileDiff> {
  return invoke<FileDiff>("git_file_diff", { cwd, path });
}

/** One commit for the group Info panel's recent history. */
export interface CommitInfo {
  hash: string;
  subject: string;
  author: string;
  relative: string;
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
export function gitCommit(cwd: string, message: string, amend = false): Promise<CommitInfo> {
  return invoke<CommitInfo>("git_commit", { cwd, message, amend });
}

/** Total commits reachable from HEAD; zero outside a repository or before the first commit. */
export function gitCommitCount(cwd: string): Promise<number> {
  return invoke<number>("git_commit_count", { cwd });
}

/** One page of history, newest first. */
export function gitLogPage(cwd: string, limit: number, offset: number): Promise<CommitInfo[]> {
  return invoke<CommitInfo[]>("git_log_page", { cwd, limit, offset });
}

/** Files changed by one commit; merge commits return an empty list. */
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

/**
 * Answer a spawn confirmation card, so other clients dismiss theirs.
 *
 * Resolves to true only for the client that answered first. The same card is shown everywhere, so a
 * second answer arriving a moment later must not act on it: the winner is already creating the worktree
 * and the child session, and a loser that proceeded would run the whole task a second time.
 */
export function resolveSpawn(
  parentSessionId: string,
  prompt: string,
  confirmed: boolean,
): Promise<boolean> {
  return invoke<boolean>("resolve_spawn", { parentSessionId, prompt, confirmed });
}

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
