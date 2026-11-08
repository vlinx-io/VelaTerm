//! Talking to an agent that runs as a protocol peer rather than in a terminal.
//!
//! Every call here names a session and nothing else; the backend owns the process, so a view can mount,
//! unmount, and mount again — or open on a second device — without disturbing the conversation. What the
//! agent is doing arrives on one event channel per session.

import { invoke, listen, type UnlistenFn } from "./transport";
import { t } from "../i18n";
import { apiUrl } from "./shareBase";
import { mapBackendError } from "./backendError";
import type { Session } from "../types";

/**
 * An image sent with a message: its bytes as base64, without the `data:` prefix, and the type they are
 * encoded in. The same shape travels out with a message and comes back on the row that message became.
 */
export interface ChatImage {
  mimeType: string;
  data: string;
}

/** The compact wire shape used only inside a snapshot; bytes stay behind the session-owned endpoint. */
export interface ChatImageReference {
  mimeType: string;
  attachmentId: string;
  contentRevision?: string;
  sizeBytes?: number;
}

/** A live row carries bytes; a snapshot row carries a reference until its image is mounted. */
export type ChatImageValue = ChatImage | ChatImageReference;

/** A small process-local cache prevents two panes in the same client from fetching the same image twice. */
const snapshotImageCache = new Map<string, ChatImage>();
const SNAPSHOT_IMAGE_CACHE_LIMIT = 16;
const SNAPSHOT_IMAGE_CACHE_BYTES = 8 * 1024 * 1024;
const snapshotImageContexts = new WeakMap<object, { sessionId: string; cacheOwner: string; historyGeneration?: string; childPath?: string[] }>();
let imageReadTail:Promise<void>=Promise.resolve();
function serializeImageRead<T>(read:()=>Promise<T>,signal?:AbortSignal):Promise<T> {
  const previous=imageReadTail;
  let release!:()=>void;
  imageReadTail=new Promise<void>(resolve=>{release=resolve;});
  return previous.then(()=>{signal?.throwIfAborted();return read();}).finally(release);
}
function imageBytes(image:ChatImage):number {
  // Count UTF-16 bytes, the data URL/render copy, serialization, and decoded thumbnail pixels.
  return image.data.length*5+image.mimeType.length*4+256*256*4+256;
}

function rememberSnapshotImage(key: string, image: ChatImage): void {
  const bytes=imageBytes(image);
  if (bytes>SNAPSHOT_IMAGE_CACHE_BYTES) return;
  snapshotImageCache.delete(key);
  let retained=[...snapshotImageCache.values()].reduce((total,item)=>total+imageBytes(item),0);
  while (snapshotImageCache.size && (snapshotImageCache.size>=SNAPSHOT_IMAGE_CACHE_LIMIT || retained+bytes>SNAPSHOT_IMAGE_CACHE_BYTES)) {
    const oldest = snapshotImageCache.keys().next().value as string | undefined;
    if (!oldest) break;
    const previous=snapshotImageCache.get(oldest)!;retained-=imageBytes(previous);snapshotImageCache.delete(oldest);
  }
  snapshotImageCache.set(key, image);
}

/** Remember which session owns every lightweight image reference, without adding client fields to it. */
function registerSnapshotImages(cacheOwner: string, sessionId: string, value: unknown): void {
  if (Array.isArray(value)) {
    value.forEach((item) => registerSnapshotImages(cacheOwner, sessionId, item));
    return;
  }
  if (!value || typeof value !== "object") return;
  const record = value as Record<string, unknown>;
  const attachmentId = record.attachmentId;
  if (typeof attachmentId === "string" && typeof record.mimeType === "string") {
    snapshotImageContexts.set(record, { sessionId, cacheOwner });
    return;
  }
  Object.values(record).forEach((item) => registerSnapshotImages(cacheOwner, sessionId, item));
}

/** Resolve one mounted snapshot image. Unmounted history never crosses the transport. */
export async function resolveChatImage(image: ChatImageValue, original:boolean|"view" = false,signal?:AbortSignal): Promise<ChatImage> {
  signal?.throwIfAborted();
  if ("data" in image) return image;
  const context = snapshotImageContexts.get(image);
  if (!context) throw new Error("This chat attachment is no longer associated with a snapshot");
  // Replayed row ids are stable only within one agent process. Including its start time prevents a new
  // branch that reuses `h-0` from receiving the previous process's image bytes from this client cache.
  const purpose=original==="view" ? "view" : original ? "original" : "preview";
  const key = `${context.cacheOwner}\u0000${image.attachmentId}\u0000${purpose}`;
  const cached = snapshotImageCache.get(key);
  if (cached) return cached;
  return serializeImageRead(async()=> {
    const reused=snapshotImageCache.get(key);if (reused) return reused;
    const resolved=await invoke<ChatImage>("chat_attachment", {
      sessionId: context.sessionId,
      attachmentId: image.attachmentId,
      purpose,
      ...(image.attachmentId.startsWith("history:") ? { historyGeneration: context.historyGeneration, childPath: context.childPath,
        expectedRevision: image.contentRevision } : {}),
    });
    signal?.throwIfAborted();
    if (purpose==="preview") rememberSnapshotImage(key,resolved);
    return resolved;
  },signal);
}

/** One row of the conversation. A tool call and its result are one row, because they are one card. */
export interface SubagentInfo {
  title?: string;
  description?: string;
  model?: string;
  totalTokens?: number;
  toolUses?: number;
  durationMs?: number;
}

export interface MessageOrigin {
  sessionId: string;
  name: string;
  agent: Session["kind"];
  role: "plan" | "exec" | "review" | "system" | "initiator" | "session";
  runId?: string;
  round?: number;
}

export type ChatRow = (
  | { kind: "user"; id: string; text: string; images?: ChatImageValue[]; at?: number; origin?: MessageOrigin }
  | {
      kind: "assistant";
      id: string;
      text: string;
      streaming: boolean;
      /** Model reported for this reply, independent of the composer's current selection. */
      model?: string;
      at?: number;
      /** Total wall-clock time for the completed turn. Absent while it is still running. */
      durationMs?: number;
    }
  | { kind: "reasoning"; id: string; text: string; streaming: boolean }
  | {
      kind: "tool";
      id: string;
      name: string;
      input: unknown;
      output?: string;
      isError: boolean;
      status: "running" | "completed" | "failed" | "canceled";
      /** Identity and live usage announced by the provider's subagent protocol. */
      subagent?: SubagentInfo;
      /**
       * What a subagent did inside this call. Only a `Task` call ever has any.
       *
       * The agent reports a subagent's messages and tool calls as ordinary frames tagged with this
       * call's id, so they belong to the card rather than to the conversation — a Task that runs twenty
       * tools would otherwise bury the answer under twenty cards nobody asked for.
       */
      children?: ChatRow[];
      detailAvailable?: boolean;
      childCount?: number;
      childAvailable?: boolean;
    }
  | { kind: "error"; id: string; message: string }
  /**
   * What a slash command the agent answered itself printed back, such as `/effort` or `/context`.
   *
   * Its own row because it is neither side of the conversation: drawing it as an answer would credit
   * the model with words it never wrote.
   */
  | { kind: "command"; id: string; text: string }
  /**
   * A shell command the user ran with `!` from the composer, with what it printed.
   *
   * Neither a prompt nor a tool call: the user ran it and the agent is only told. `running` while it
   * runs, then `completed` with its exit code, or `cancelled`.
   */
  | {
      kind: "shell";
      id: string;
      command: string;
      stdout: string;
      stderr: string;
      /** The head of the stream was cut; only the tail is kept. */
      stdoutTruncated: boolean;
      stderrTruncated: boolean;
      outputIncomplete?: boolean;
      status: "running" | "completed" | "cancelled";
      exitCode?: number;
      at?: number;
    }
  /** A remark about the conversation itself, such as history that was too long to replay in full. */
  | { kind: "notice"; id: string; message: string }
  /**
   * The conversation was summarized to make room in the context window.
   *
   * Worth its own row because it explains why the agent may not remember something said earlier. The
   * wording belongs to the view, so the row carries only facts.
   */
  | {
      kind: "compaction";
      id: string;
      status: "loading" | "completed";
      /** `manual` when someone asked for it, `auto` when the window filled up. */
      trigger?: string;
      /** How large the context was before the summary replaced it. */
      preTokens?: number;
    }
  /**
   * A background task finished while the agent was idle, and the agent answered it in a turn of its own.
   *
   * A turn boundary like a prompt: without it, what the agent reports on its own reads as the answer to
   * whatever the person said last. `summary` is the agent's one-line account of the task, possibly empty.
   */
  | { kind: "wake"; id: string; summary: string; at?: number }) & {
      bodyAvailable?: boolean;
      /**
       * How this row's details are read, when it differs from the rest of the window: a running
       * conversation can show history-page rows in front of rows its process produced itself.
       */
      detailProtocol?: 1 | 2;
      /** The history generation a history-page row belongs to, when the window mixes sources. */
      historyGeneration?: string;
    };

/** A tool the agent wants to run and is waiting for permission on. */
export interface ChatPermission {
  /** Answer with this id. */
  id: string;
  /** `can_use_tool` for a tool asking permission; `elicitation` for an MCP server asking for input. */
  subtype?: string;
  /** Elicitation only: the server asking, what it asks, and the form it wants filled in. */
  mcp_server_name?: string;
  message?: string;
  mode?: "form" | "url";
  url?: string;
  title?: string;
  requested_schema?: unknown;
  tool_name?: string;
  display_name?: string;
  /** Short human-readable subject, such as the file about to be written. */
  description?: string;
  /** Why the agent is asking rather than deciding by itself, in its own words (English). */
  decision_reason?: string;
  input?: unknown;
  /**
   * Standing rules the agent would accept instead of asking again — "accept file edits for the rest of
   * this session", "always allow this command". Read by `permissions.ts`, which keeps the ones a button
   * can honestly describe; taking one sends it straight back as `updatedPermissions`.
   */
  permission_suggestions?: unknown;
}

/** A slash command or skill the agent offers, as reported when it starts. */
export interface ChatCommand {
  name: string;
  description?: string;
  argumentHint?: string;
  /** Native invocation prefix. Provider entries accept either completion trigger. */
  invocation?: "/" | "$";
}

/** Read the current provider catalogue without sending a prompt or opening a conversation. */
export const chatCommands = (sessionId: string) => invoke<ChatCommand[]>("chat_commands", { sessionId });

/** One setting `/config` accepts, as the agent describes it. */
export interface ChatConfigKey {
  key: string;
  /** Allowed values, or empty when the setting takes free text. */
  values: string[];
}

/** One model the composer's model chip can switch to. */
export interface ChatModel {
  /** Sent verbatim when switching; a `[1m]` suffix is part of the identifier, not a modifier. */
  id: string;
  label: string;
  /** What the model is for, or which settings key it was read from. */
  description: string;
  /** Effort levels to offer while this model is selected. */
  effortLevels: string[];
  contextWindow?: number;
  /** False for a model read from the user's settings rather than the curated table. */
  curated: boolean;
  /** True for a 1M-context model, which some accounts must buy usage credits to run. */
  largeContext: boolean;
  /** Reported by the running agent for models that honour fast mode; absent from the curated table. */
  supportsFastMode?: boolean;
  /** Codex: the speeds this model offers besides its standard one, such as `priority` ("Fast"). */
  serviceTiers?: ChatServiceTier[];
  /** Codex: the tier the catalogue runs this model at unless told otherwise. */
  defaultServiceTier?: string;
  /** Codex: whether this model honours the personality setting. */
  supportsPersonality?: boolean;
}

/** One speed a Codex model offers, as its catalogue describes it. */
export interface ChatServiceTier {
  id: string;
  label: string;
  description: string;
}

/** Codex service tier sent when no faster one is chosen. The protocol names it for "standard speed". */
export const CODEX_STANDARD_TIER = "default";

/** How Codex speaks. The empty string leaves the configured personality alone. */
export const CODEX_PERSONALITIES = ["none", "friendly", "pragmatic"] as const;
export type CodexPersonality = (typeof CODEX_PERSONALITIES)[number];

/** One phase of a Claude workflow, as `task_progress.workflow_progress` lists it. */
export interface ChatWorkflowPhase {
  type: "workflow_phase";
  id?: string;
  index?: number;
  title: string;
}

/** One agent of a Claude workflow. Everything but the label is optional: older CLIs send less. */
export interface ChatWorkflowAgent {
  type: "workflow_agent";
  id?: string;
  index?: number;
  label: string;
  phaseIndex?: number;
  phaseTitle?: string;
  agentId?: string;
  model?: string;
  state?: "start" | "done" | string;
  startedAt?: number;
  queuedAt?: number;
  attempt?: number;
  promptPreview?: string;
  promptFramed?: boolean;
  lastProgressAt?: number;
  tokens?: number;
  toolCalls?: number;
  elapsedMs?: number;
  elapsedRunning?: boolean;
  durationMs?: number;
  resultPreview?: string;
}

export type ChatWorkflowEntry = ChatWorkflowPhase | ChatWorkflowAgent;

/**
 * One of Claude's background tasks: a shell command, a subagent or a workflow still running after its turn
 * moved on, merged by the backend from the inventory and every task-protocol frame. Keys are Claude's
 * snake_case. Only `task_id`, `task_type`, `description` and `status` are always present.
 */
export interface ChatBackgroundTask {
  task_id: string;
  task_type: string;
  /** Live: for workflows "Phase: agent", refreshed every few seconds. */
  description: string;
  /** "running", one of Claude's terminal values, or "ended" when the task left the inventory silently. */
  status?: string;
  finished?: boolean;
  can_stop?: boolean;
  elapsed_ms?: number;
  /** The static description, then the final summary once the task ends. */
  summary?: string;
  tool_use_id?: string;
  workflow_name?: string;
  /** Last reported tool for local_agent, last reported agent label for local_workflow. */
  last_tool_name?: string;
  started_at?: number;
  ended_at?: number;
  usage?: { total_tokens?: number; tool_uses?: number; duration_ms?: number };
  workflow_progress?: ChatWorkflowEntry[];
  output_file?: string;
}

/** Whether a task reports nothing more. A task without a status is one the backend still lists as live. */
export function isTaskFinished(task: Pick<ChatBackgroundTask, "finished" | "ended_at">): boolean {
  return task.finished ?? task.ended_at != null;
}

/** An MCP server as the running agent reports it. */
export interface ChatMcpServer {
  canToggle?: boolean;
  name: string;
  status: string;
  scope?: string;
  serverInfo?: { name?: string; version?: string };
  tools?: { name: string }[];
  error?: string;
}

/** A failed API call the agent is about to retry. */
export interface ChatApiRetry {
  attempt: number;
  maxRetries: number;
  delayMs: number;
  message: string;
}

/** What Claude's protocol reports about the process beyond the conversation itself. Replaced whole. */
export interface ChatAuthState {
  status: "required" | "starting" | "pending" | "submitting" | "canceling" | "success" | "failed" | "canceled" | "signingOut" | "signedOut" | "logoutFailed";
  verificationUrl?: string;
  userCode?: string;
}

/** `provider` picks ZCode's account provider (`zai` or `bigmodel`); Claude and Codex have one each. */
export const chatAuthStart = (sessionId: string, provider?: "zai" | "bigmodel") => invoke<void>("chat_auth_start", { sessionId, provider });
export const chatAuthLogout = (sessionId: string) => invoke<void>("chat_auth_logout", { sessionId });
export const chatAuthSubmit = (sessionId: string, code: string) => invoke<void>("chat_auth_submit", { sessionId, code });
export const chatAuthCancel = (sessionId: string) => invoke<void>("chat_auth_cancel", { sessionId });

/** What the session's agent needs before it can take a message. `signedIn` is absent or null when unknown. */
export interface AgentReadiness { installed: boolean; signedIn?: boolean | null }
export const chatAgentReadiness = (sessionId: string) => invoke<AgentReadiness>("chat_agent_readiness", { sessionId });

export interface ChatExtras {
  auth?: ChatAuthState;
  contextTokens?: number;
  contextWindow?: number;
  totalCostUsd?: number;
  /** The agent's own categorised breakdown of the context window after the last turn. */
  contextUsage?: {
    totalTokens?: number;
    maxTokens?: number;
    percentage?: number;
    model?: string;
    categories?: { name: string; tokens: number; isDeferred?: boolean }[];
  };
  rateLimit?: {
    status?: "allowed" | "allowed_warning" | "rejected";
    resetsAt?: number;
    rateLimitType?: string;
    utilization?: number;
  };
  backgroundTasks?: ChatBackgroundTask[];
  fastMode?: boolean;
  apiRetry?: ChatApiRetry;
  /** Claude: the installed CLI predates live model listing, so the model menu shows the built-in catalogue. */
  listModelsUnsupported?: boolean;
}

/**
 * One collaboration preset: a Codex mode reported by the installed app-server, or an OpenCode agent
 * reported by its server. Both decide how the agent works on subsequent turns, so they share one chip.
 */
export interface ChatCollaborationMode {
  /** Protocol value sent back on subsequent turns: `default` or `plan` for Codex, an agent name for OpenCode. */
  mode: string;
  /** Native label, retained as a fallback when the UI does not yet know this mode. */
  name: string;
  /** What the preset is for, in the provider's own words; OpenCode agents describe themselves. */
  description?: string;
  /** Native preset effort; an explicit effort selected in the composer overrides it. */
  reasoningEffort?: string;
}

/**
 * A message typed while the agent was busy, waiting for the turn in progress to end.
 *
 * Not a timeline row: nothing has been said yet, so it can still be rewritten or dropped. It becomes a
 * user row the moment it is actually sent.
 */
export interface QueuedMessage {
  id: string;
  text: string;
  textDeferred?: boolean;
  textRevision?: string;
  /** Command preview parsed by the backend; the original context stays in text. */
  shellCommand?: string;
  origin?: MessageOrigin;
  /** Images attached to it, waiting along with the text. */
  images?: ChatImageValue[];
}

/** What to do with a message typed while a turn is running. */
export type SendBehavior = "queue" | "interrupt" | "steer";

/** What a rewind should undo. File restoration is delegated to the agent; Codex supports only conversation. */
export type ChatRewindScope = "conversation" | "files" | "both";

/** Backend preview of the file checkpoint associated with a user turn. */
export interface ChatRewindPreview {
  historyGeneration?: string;
  canRewind?: boolean;
  filesChanged?: string[];
  insertions?: number;
  deletions?: number;
  error?: string | null;
}

export interface ChatRewindResult {
  /** The removed message, ready to edit and send again. */
  prefillText?: string | null;
  files?: ChatRewindPreview | null;
}

/** Backend-confirmed difference between the active permissions and the saved choice. */
export interface PendingPermissionMode {
  /** Policy accepted by Codex for the current or most recent turn. */
  current: string;
  /** Saved choice to apply on the next turn; steering does not apply it. */
  next: string;
}

/** A backend-owned index of top-level user messages, including history outside the loaded window. */
export interface ChatUserMessage {
  id: string;
  text: string;
  hasImages: boolean;
}

/** Everything needed to draw a conversation from scratch. */
export interface ChatSnapshot {
  protocolVersion?: 1 | 2;
  historyGeneration?: string;
  readJobId?: string;
  processEpoch?: number | null;
  snapshotRevision?: string;
  coverage?: "complete" | "partial" | "error";
  indexing?: boolean;
  pendingLocation?: { before?: string; after?: string; anchor?: string; messageIndex?: number; recordOffset?: number };
  locatedRowId?: string;
  indexedBytes?: number;
  sourceBytes?: number;
  knownRows?: number;
  hasNewer?: boolean;
  historyError?: string | null;
  userMessages?: ChatUserMessage[];
  positions?: Record<string, number>;
  pageKind?: "full" | "recent" | "history" | "delta";
  hasMore?: boolean;
  totalRows?: number;
  submissionReceipts?: boolean;
  rowsRevision?: number;
  queueRevision?: number;
  rewindScopes?: ChatRewindScope[];
  running: boolean;
  rows: ChatRow[];
  /** Messages waiting for the running turn to end, in the order they will be sent. */
  queue: QueuedMessage[];
  permissions: ChatPermission[];
  commands: ChatCommand[];
  configKeys: ChatConfigKey[];
  agentSessionId?: string;
  model?: string;
  effort?: string;
  mode?: string;
  pendingPermissionMode?: PendingPermissionMode | null;
  /** Backend-owned conversation choices; null fields explicitly retain automatic defaults. */
  selection?: { model: string | null; effort: string | null };
  /** Codex collaboration style, independent of `mode`, which controls permissions. */
  collaborationMode?: string;
  collaborationModes?: ChatCollaborationMode[];
  /** The agent process behind this conversation, so the Info panel can describe it like any other. */
  pid?: number;
  /** When that process started, in milliseconds since the epoch. */
  startedAt?: number;
  /** Authoritative start of the logical turn currently being processed. */
  turnStartedAt?: number;
  /** Codex: the service tier and personality chosen for this conversation, when set. */
  serviceTier?: string;
  personality?: string;
  /** Claude: whether Claude in Chrome is attached to this conversation, running or not. */
  chrome?: boolean;
  /** A usage limit stopped this conversation, and it continues on its own at the time given. */
  autoContinue?: ChatAutoContinue;
  auth?: ChatAuthState;
  /** Flattened `ChatExtras`; read them with `extrasOf`. */
  contextTokens?: number;
  contextWindow?: number;
  totalCostUsd?: number;
  contextUsage?: ChatExtras["contextUsage"];
  rateLimit?: ChatExtras["rateLimit"];
  backgroundTasks?: ChatBackgroundTask[];
  fastMode?: boolean;
  apiRetry?: ChatApiRetry;
  listModelsUnsupported?: boolean;
}

/** A conversation waiting for its usage limit to reset before it continues on its own. */
export interface ChatAutoContinue {
  /** Unix seconds at which the continuation is sent. */
  continueAt: number;
  /** `five_hour`, `seven_day`, or a provider-specific weekly bucket; absent when the provider did not say. */
  limitType?: string;
}

/** Why automatic continuation ended without sending anything. */
export type ChatAutoContinueReason = "unknownReset" | "repeated" | "failed";

/** The extras a snapshot carries, as one object. */
export function extrasOf(snapshot: ChatSnapshot): ChatExtras {
  return {
    auth: snapshot.auth,
    contextTokens: snapshot.contextTokens,
    contextWindow: snapshot.contextWindow,
    totalCostUsd: snapshot.totalCostUsd,
    contextUsage: snapshot.contextUsage,
    rateLimit: snapshot.rateLimit,
    backgroundTasks: snapshot.backgroundTasks,
    fastMode: snapshot.fastMode,
    apiRetry: snapshot.apiRetry,
    listModelsUnsupported: snapshot.listModelsUnsupported,
  };
}

/** What arrives on a session's chat channel. */
export type ChatEvent =
  | { type: "historyReadError"; message: string }
  | { type: "rows"; protocolVersion?: 1 | 2; positions?: Record<string, number>; rows: ChatRow[]; revision?: number; epoch?: number }
  /** A rewind removed rows. Replace rather than merge so every connected view drops the same tail. */
  | { type: "replaceRows"; protocolVersion?: 1 | 2; userMessages?: ChatUserMessage[]; positions?: Record<string, number>; rows: ChatRow[]; revision?: number; epoch?: number; hasMore?: boolean }
  /** A new process took over; its restored history replaces the previous timeline atomically. */
  | { type: "reset"; protocolVersion?: 1 | 2; userMessages?: ChatUserMessage[]; epoch?: number; revision?: number; rows?: ChatRow[]; positions?: Record<string, number>; hasMore?: boolean }
  | { type: "permission"; request: ChatPermission }
  /** The whole queue, whenever it changes. A full list rather than one change, so clients cannot drift. */
  | { type: "queued"; items: QueuedMessage[]; revision?: number; epoch?: number }
  | { type: "permissionResolved"; id: string }
  | { type: "commands"; commands: ChatCommand[] }
  | { type: "configKeys"; keys: ChatConfigKey[] }
  | {
      type: "session";
      agentSessionId: string;
      model?: string | null;
      effort?: string | null;
      collaborationMode?: string | null;
    }
  | { type: "collaborationModes"; modes: ChatCollaborationMode[]; mode: string }
  | { type: "collaborationModeChanged"; mode: string }
  | { type: "codexSettingsChanged"; serviceTier?: string | null; personality?: string | null }
  /** Claude in Chrome was attached or detached for this conversation, by this view or another one. */
  | { type: "chromeChanged"; enabled: boolean }
  | { type: "settingsChanged"; model?: string | null; effort?: string | null; mode?: string; pendingPermissionMode?: PendingPermissionMode | null }
  /**
   * Which process is behind this conversation, announced as soon as it exists.
   *
   * A pane that opened before the agent started has already read its snapshot, and nothing else in the
   * product can see a process that has no PTY.
   */
  | { type: "process"; pid: number; startedAt: number; rewindScopes?: ChatRewindScope[] }
  | { type: "turnStarted"; startedAt: number }
  | { type: "turnCompleted" }
  | { type: "turnInterrupted" }
  | { type: "steerAccepted" }
  /**
   * The process behind the conversation ended. `released` says the application let it go on purpose —
   * the view was closed, the session archived, or the engine switched — so there is nothing to report.
   */
  | { type: "exited"; code: number; stderr: string; released?: boolean }
  | { type: "error"; message: string; /** Text of messages that were waiting and will not be sent. */ unsent?: string[] }
  /** The agent refused this message before keeping it, so it goes back to the composer. */
  | { type: "unsent"; text: string; images: ChatImage[] }
  | { type: "storagePressure"; message: string }
  | { type: "extras"; extras: ChatExtras }
  /** The wait for a usage limit to reset began, moved, or ended; `reason` says why it ended unsent. */
  | { type: "autoContinue"; waiting: ChatAutoContinue | null; reason?: ChatAutoContinueReason | null }
  /** The agent reported its own model catalogue; `chatModels` now answers from it. */
  | { type: "models" }
  | { type: "notification"; text: string; priority: string; timeoutMs?: number | null };

/**
 * Start the agent for this session, or do nothing when it is already running.
 *
 * The conversation continues rather than restarts: the session's recorded agent id is resumed, including
 * one written while the same session was running in a terminal.
 */
export function chatStart(
  sessionId: string,
  model?: string,
  effort?: string,
  fastMode?: boolean,
): Promise<void> {
  return invoke("chat_start", { sessionId, model, effort, fastMode });
}

/** Switch Claude's fast mode for the running conversation. */
export function chatSetFastMode(sessionId: string, enabled: boolean): Promise<void> {
  return invoke("chat_set_fast_mode", { sessionId, enabled });
}

/** Attach or detach Claude in Chrome for this conversation, now and for every later process. */
export function chatSetChrome(sessionId: string, enabled: boolean): Promise<void> {
  return invoke("chat_set_chrome", { sessionId, enabled });
}

/** Choose the Codex service tier for subsequent turns; omit to return to the thread's own. */
export function chatSetServiceTier(sessionId: string, tier?: string): Promise<void> {
  return invoke("chat_set_service_tier", { sessionId, tier });
}

/** Choose how Codex speaks on subsequent turns; omit to return to the configured personality. */
export function chatSetPersonality(sessionId: string, personality?: string): Promise<void> {
  return invoke("chat_set_personality", { sessionId, personality });
}

/** `/compact`: ask Codex to summarize the conversation now rather than when the window fills up. */
export function chatCompact(sessionId: string): Promise<void> {
  return invoke("chat_compact", { sessionId });
}

/**
 * `/review`: run Codex's code review as a turn of the conversation.
 *
 * `args` is what followed the command: nothing for the working tree, `branch <name>`, `commit <sha>`, or
 * free-form instructions.
 */
export function chatReview(sessionId: string, args: string): Promise<void> {
  return invoke("chat_review", { sessionId, args });
}

/** The MCP servers the running agent knows, with their connection state and tools. */
export function chatMcpStatus(sessionId: string): Promise<{ mcpServers?: ChatMcpServer[] }> {
  return invoke("chat_mcp_status", { sessionId });
}

/** Enable or disable one MCP server; answers with the refreshed list. */
export function chatMcpToggle(
  sessionId: string,
  server: string,
  enabled: boolean,
): Promise<{ mcpServers?: ChatMcpServer[] }> {
  return invoke("chat_mcp_toggle", { sessionId, server, enabled });
}

/** Reconnect one MCP server; answers with the refreshed list. */
export function chatMcpReconnect(
  sessionId: string,
  server: string,
): Promise<{ mcpServers?: ChatMcpServer[] }> {
  return invoke("chat_mcp_reconnect", { sessionId, server });
}

/** Stop one of Claude's background tasks. */
export function chatStopTask(sessionId: string, taskId: string): Promise<void> {
  return invoke("chat_stop_task", { sessionId, taskId });
}

/** The command a background shell task runs and the tail of its output. */
export interface ChatTaskOutput {
  command?: string;
  /** Absent when the output file cannot be read. */
  output?: string;
  /** Earlier output exists beyond what was returned. */
  truncated: boolean;
}

export function chatTaskOutput(sessionId: string, taskId: string): Promise<ChatTaskOutput> {
  return invoke("chat_task_output", { sessionId, taskId });
}

/** What one of Claude's subagents has done so far, as rows of its own conversation. */
export function chatSubagentRows(sessionId: string, taskId: string): Promise<ChatRow[]> {
  return invoke("chat_subagent_rows", { sessionId, taskId });
}

/** Move every foreground task of the running turn to the background. */
export function chatBackgroundTasks(sessionId: string): Promise<void> {
  return invoke("chat_background_tasks", { sessionId });
}

/** Archive this conversation and return its empty, identically configured replacement. */
export function chatClear(sessionId: string): Promise<Session> {
  return invoke("chat_clear", { sessionId });
}

/**
 * Send a user turn, with any images attached to it.
 *
 * `behavior` only matters while a turn is already running. `queue` — the default — waits for that turn to
 * finish and then sends this message; `interrupt` stops the turn and sends this one first; `steer` writes
 * it into the running turn, which the agent answers together with what it was already doing. When the
 * agent is idle every behavior means the same thing: send it now.
 *
 * `images` go out as part of the same turn and wait with it when it is queued.
 *
 * Answers `"sent"`, `"steered"` when it joined a turn already running, `"blocked"` when that turn is
 * waiting for an answer to a permission question, or `"queued"`, so the composer can say which happened.
 */
export function chatSend(
  sessionId: string,
  text: string,
  behavior?: SendBehavior,
  images?: ChatImage[],
  messageId?: string,
  originalFallback?: boolean,
): Promise<"sent" | "steered" | "blocked" | "queued" | "command"> {
  return invoke("chat_send", { sessionId, text, behavior, images, messageId, originalFallback });
}

/** Stop waiting for the usage limit to reset; the conversation will not continue on its own. */
export function chatAutoContinueCancel(sessionId: string): Promise<void> {
  return invoke("chat_auto_continue_cancel", { sessionId });
}

/**
 * Run a shell command typed with `!` in the composer.
 *
 * The command runs in the agent's directory and environment, shows on a row of its own, and its result is
 * handed to the agent as context when it ends. `messageId` is that row's id and the handle for cancel.
 */
export function chatRunShell(sessionId: string, command: string, messageId: string): Promise<void> {
  return invoke("chat_run_shell", { sessionId, command, messageId });
}

/** Stop a running shell command together with everything it started. */
export function chatCancelShell(sessionId: string, messageId: string): Promise<void> {
  return invoke("chat_cancel_shell", { sessionId, messageId });
}

/** Add an existing queued message to the running turn. */
export function chatQueueSteer(sessionId: string, id: string): Promise<void> {
  return invoke("chat_queue_steer", { sessionId, id });
}

/** Drop a message that is still waiting; its text is never sent. */
export function chatQueueRemove(sessionId: string, id: string): Promise<void> {
  return invoke("chat_queue_remove", { sessionId, id });
}

/** Rewrite the words of a message that is still waiting. Its images stay attached. */
export function chatQueueUpdate(sessionId: string, id: string, text: string): Promise<void> {
  return invoke("chat_queue_update", { sessionId, id, text });
}

/** Read an explicit editable draft through revision-bound text pieces. */
export interface QueuedTextPiece {text:string;contentRevision:string;chunkSequence:number;eof:boolean;sizeBytes:number}
export function chatQueueRead(sessionId:string,item:QueuedMessage,sequence=0,expectedRevision=item.textRevision):Promise<QueuedTextPiece> {
  return invoke("chat_queue_read",{sessionId,id:item.id,expectedRevision,sequence});
}

export async function readCompleteQueuedText(sessionId:string,item:QueuedMessage,signal?:AbortSignal):Promise<string> {
  if (!item.textDeferred) return item.text;
  let revision=item.textRevision;
  const pieces:string[]=[];
  let bytes=0;
  for (let sequence=0;;sequence++) {
    signal?.throwIfAborted();
    const part=await chatQueueRead(sessionId,item,sequence,revision);
    signal?.throwIfAborted();
    if (part.chunkSequence!==sequence || (revision!==undefined && revision!==part.contentRevision)) throw new Error("source_changed: "+t("chat.history.contentChanged"));
    if (part.sizeBytes>COMPLETE_COPY_BYTES) throw new Error("detail_requires_download: "+t("chat.history.clipboardLimit"));
    bytes+=new TextEncoder().encode(part.text).byteLength;
    if (bytes>COMPLETE_COPY_BYTES) throw new Error("detail_requires_download: "+t("chat.history.clipboardLimit"));
    revision=part.contentRevision;pieces.push(part.text);
    if (part.eof) return pieces.join("");
  }
}

/** Stop the turn in progress; the conversation stays open. */
export function chatInterrupt(sessionId: string): Promise<void> {
  return invoke("chat_interrupt", { sessionId });
}

/** Native option indices and explicitly typed text stay separate until the host builds the answer. */
export interface QuestionAnswerSelection { selected: number[]; text: string }

/**
 * Answer a permission question.
 *
 * `updatedInput` replaces the arguments the tool runs with. It is for the tools whose answer *is* the
 * input — AskUserQuestion carries what was picked in its own arguments — and left out, the agent runs
 * exactly what it proposed.
 *
 * `updatedPermissions` is what turns a yes into "and stop asking me about this": the suggestions the
 * question itself offered, handed back as they arrived.
 */
export function chatPermission(
  sessionId: string,
  requestId: string,
  allow: boolean,
  options?: { updatedInput?: unknown; message?: string; updatedPermissions?: unknown[]; questionAnswers?: QuestionAnswerSelection[]; sendOriginal?: boolean },
): Promise<void> {
  return invoke("chat_permission", {
    sessionId,
    requestId,
    allow,
    updatedInput: options?.updatedInput,
    message: options?.message,
    updatedPermissions: options?.updatedPermissions,
    questionAnswers: options?.questionAnswers,
    sendOriginal: options?.sendOriginal,
  });
}

/** Change the permission mode of the running conversation. */
export function chatSetMode(sessionId: string, mode: string): Promise<void> {
  return invoke("chat_set_mode", { sessionId, mode });
}

/** Restart only the Claude process whose permission rejection the user confirmed. */
export function chatRestartPermissionMode(sessionId: string, pid: number): Promise<void> {
  return invoke("chat_restart_permission_mode", { sessionId, pid });
}

/** Change the collaboration style used by subsequent Codex turns. */
export function chatSetCollaborationMode(sessionId: string, mode: string): Promise<void> {
  return invoke("chat_set_collaboration_mode", { sessionId, mode });
}

/** Change the model of the running conversation; omit to restore the default. */
export function chatSetModel(sessionId: string, model?: string): Promise<void> {
  return invoke("chat_set_model", { sessionId, model });
}

/** Change reasoning effort; an omitted value restores the agent's automatic choice. */
export function chatSetEffort(sessionId: string, effort?: string): Promise<void> {
  return invoke("chat_set_effort", { sessionId, effort });
}

/** Models offered by this session's installed agent and account. */
export function chatModels(sessionId: string): Promise<ChatModel[]> {
  return invoke("chat_models", { sessionId });
}

/** Negotiate a bounded conversation window; image and history body bytes remain deferred. */
export async function chatSnapshot(sessionId: string, window?: { before?: string; from?: string; since?: number; epoch?: number; protocolVersion?: 2; historyGeneration?: string; after?: string; anchor?: string; readJobId?: string; historyOnly?: boolean; messageIndex?: number; recordOffset?: number; controlOnly?: boolean; childPath?: string[] }): Promise<ChatSnapshot> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const snapshot = await Promise.race([
    invoke<ChatSnapshot>("chat_snapshot", { sessionId, ...(!window?.controlOnly ? { preferBounded: true } : {}),
      ...(window?.historyOnly !== undefined ? { historyOnly: window.historyOnly } : {}),
      ...(window?.childPath?.length ? { childPath: window.childPath } : {}),
      ...(window?.messageIndex !== undefined ? { messageIndex: window.messageIndex } : {}),
      ...(window?.recordOffset !== undefined ? { recordOffset: window.recordOffset } : {}), ...(window ? { window } : {}),
      ...(window?.protocolVersion ? { protocolVersion: window.protocolVersion, historyGeneration: window.historyGeneration, after: window.after, anchor: window.anchor, readJobId: window.readJobId } : {}) }),
    new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error("Conversation synchronization timed out")), 30_000); }),
  ]).finally(() => clearTimeout(timer));
  if (snapshot.historyError) snapshot.historyError = mapBackendError(snapshot.historyError);
  registerRowPositions(snapshot.rows, snapshot.positions);
  const cacheOwner = `${sessionId}\u0000${snapshot.startedAt ?? "stopped"}`;
  registerSnapshotImages(cacheOwner, sessionId, snapshot.rows);
  registerToolDetails(sessionId, snapshot.startedAt, snapshot.rows, snapshot.protocolVersion, snapshot.historyGeneration, window?.childPath);
  registerSnapshotImages(cacheOwner, sessionId, snapshot.queue);
  return snapshot;
}

/** Preview which files Claude would restore, without changing files or conversation state. */
export function chatRewindPreview(sessionId: string, rowId: string, historyGeneration?: string): Promise<ChatRewindPreview> {
  return invoke("chat_rewind_preview", { sessionId, rowId, ...(historyGeneration ? { protocolVersion: 2, historyGeneration } : {}) });
}

/** Native targets are paged without loading the transcript or depending on the visible row position. */
export function chatRewindCandidates(sessionId: string, cursor?: string): Promise<{
  protocolVersion: 2; historyGeneration: string;
  rows: { rowId: string; text: string; at?: number; hasImages: boolean }[]; nextCursor?: string | null;
}> {
  return invoke("chat_rewind_candidates", { sessionId, ...(cursor ? { cursor } : {}) });
}

/** Apply the confirmed rewind. The backend remains the authority for the resulting timeline. */
export function chatRewind(
  sessionId: string,
  rowId: string,
  scope: ChatRewindScope,
  historyGeneration?: string,
): Promise<ChatRewindResult> {
  return invoke("chat_rewind", { sessionId, rowId, scope, ...(historyGeneration ? { protocolVersion: 2, historyGeneration } : {}) });
}

/**
 * Say that a view is showing this conversation. Cancels a release a closed view asked for; it starts
 * nothing — the agent is started by the first message sent.
 */
export function chatAttach(sessionId: string): Promise<void> {
  return invoke("chat_attach", { sessionId });
}

/**
 * Say that a view stopped showing this conversation. Its process goes as soon as it is idle, while the
 * conversation itself stays: a pane opened later shows it and restarts the agent on the next message.
 */
export function chatDetach(sessionId: string): Promise<void> {
  return invoke("chat_detach", { sessionId });
}

/** End the conversation and let its process go. */
export function chatStop(sessionId: string): Promise<void> {
  return invoke("chat_stop", { sessionId });
}

/** Choose how a session is driven: its own terminal interface, or the chat engine. */
export function setSessionEngine(
  sessionId: string,
  engine: "tui" | "chat",
): Promise<void> {
  return invoke("set_session_engine", { sessionId, engine });
}

/** On a share connection: whether this visitor may move a terminal-view session to the conversation view,
 * and whether it may open the live terminal of a terminal-view agent. Both are the host's own account only. */
export interface ShareAccess {
  canSwitchEngine: boolean;
  canUseTerminal: boolean;
}
export function shareAccess(): Promise<ShareAccess> {
  return invoke("share_access");
}

/** Subscribe to everything happening in one conversation. */
export function onChatEvent(
  sessionId: string,
  cb: (event: ChatEvent) => void,
): Promise<UnlistenFn> {
  return listen<ChatEvent>(`chat://event/${sessionId}`, event => {
    if (event.type === "historyReadError") event = { ...event, message: mapBackendError(event.message) };
    if ((event.type === "rows" || event.type === "replaceRows" || event.type === "reset") && event.rows) {
      registerRowPositions(event.rows, event.positions);
      registerSnapshotImages(`${sessionId}\u0000${event.epoch ?? "live"}`, sessionId, event.rows);
      registerToolDetails(sessionId, event.epoch, event.rows, event.protocolVersion);
    }
    cb(event);
  });
}

const toolContexts = new WeakMap<object, { sessionId: string; epoch?: number; protocolVersion?: 1 | 2; historyGeneration?: string; childPath?: string[] }>();
function registerToolDetails(sessionId: string, epoch: number | undefined, rows: ChatRow[], windowProtocol?: 1 | 2, historyGeneration?: string, childPath?: string[]) {
  for (const row of rows) {
    const protocolVersion = row.detailProtocol ?? windowProtocol;
    // A history row in front of a running timeline names its own generation; the process's own
    // rows read from memory are addressed without one.
    const generation = row.historyGeneration ?? (row.detailProtocol === 1 ? undefined : historyGeneration);
    toolContexts.set(row, { sessionId, epoch, protocolVersion, historyGeneration: generation, ...(childPath?.length ? {childPath} : {}) });
    if (row.kind === "user" && protocolVersion===2) for (const image of row.images ?? []) {
      if ("attachmentId" in image) snapshotImageContexts.set(image,{sessionId,cacheOwner:`${sessionId}\u0000${generation ?? "history"}`,historyGeneration:generation,childPath});
    }
    if (row.kind === "tool" && row.children) registerToolDetails(sessionId, epoch, row.children, protocolVersion, generation, childPath);
  }
}

export interface ChatDetailField {
  detailRef: string;
  contentRevision: string;
  label: string;
  path: string;
  sizeBytes?: number;
  append?: boolean;
  prefix?: string;
  suffix?: string;
  literal?: string;
  mimeType?: string;
  kind: "text" | "diff" | "json" | "image" | "binary";
}
export interface ChatFileDetail {
  path: string;
  changeType?: string | null;
  detailRef?: string | null;
  contentRevision?: string | null;
  kind?: ChatDetailField["kind"] | null;
  sizeBytes?: number | null;
}
export interface ChatManifest {
  protocolVersion: 2;
  historyGeneration: string;
  row: ChatRow;
  fields: ChatDetailField[];
  files: ChatFileDetail[];
  nextCursor: string | null;
  previousCursor?: string | null;
}
export interface ChatDetailChunk {
  contentRevision: string;
  chunkSequence: number;
  text: string;
  nextCursor: string | null;
  previousCursor?: string | null;
  eof: boolean;
  decodedStart: number;
  decodedEnd: number;
}
export function hasBoundedChatDetails(row: ChatRow) { return toolContexts.get(row)?.protocolVersion === 2; }
export function chatHistoryGeneration(row: ChatRow) { return toolContexts.get(row)?.historyGeneration; }
export function chatHistorySessionId(row: ChatRow) { return toolContexts.get(row)?.sessionId; }
export function chatHistoryChildPath(row: ChatRow): readonly string[] { return toolContexts.get(row)?.childPath??[]; }
/** Preserve authorized detail ownership when a view adds local presentation facts to a row. */
export function inheritChatRowContext<T extends ChatRow>(original:ChatRow,presented:T):T {
  const context=toolContexts.get(original);if (context) toolContexts.set(presented,context);
  const position=rowPositions.get(original);if (position!==undefined) rowPositions.set(presented,position);
  return presented;
}
/** Follow one explicit parent tool association using the same bounded page and detail contract. */
export function chatChildSnapshot(row: ChatRow, window?: { before?: string; after?: string; anchor?: string; readJobId?: string; historyGeneration?: string }): Promise<ChatSnapshot> {
  const context=toolContexts.get(row);
  if (!context || context.protocolVersion!==2 || row.kind!=="tool" || !row.childAvailable) return Promise.reject(new Error(t("chat.history.unavailable")));
  return chatSnapshot(context.sessionId,{...window,protocolVersion:2,historyOnly:true,childPath:[...(context.childPath??[]),row.id]});
}
export async function chatSubagentSnapshot(sessionId:string,taskId:string,window?:{before?:string;after?:string;anchor?:string;readJobId?:string}):Promise<ChatSnapshot> {
  const page=await invoke<ChatSnapshot & {childPath:string[]}>("chat_subagent_rows",{sessionId,taskId,protocolVersion:2,...window});
  if (page.historyError) page.historyError=mapBackendError(page.historyError);
  registerRowPositions(page.rows,page.positions);
  registerToolDetails(sessionId,undefined,page.rows,2,page.historyGeneration,page.childPath);
  return page;
}

export function chatManifest(row: ChatRow, cursor?: string): Promise<ChatManifest> {
  const context = toolContexts.get(row);
  if (!context || context.protocolVersion !== 2) return Promise.reject(new Error(t("chat.history.unavailable")));
  return invoke("chat_row", { ...context, rowId: row.id, cursor });
}
export function chatDetailRead(row: ChatRow, field: ChatDetailField, cursor?: string): Promise<ChatDetailChunk> {
  const context = toolContexts.get(row);
  if (!context || context.protocolVersion !== 2) return Promise.reject(new Error(t("chat.history.unavailable")));
  return invoke("chat_detail_read", { sessionId: context.sessionId, childPath: context.childPath, detailRef: field.detailRef, expectedRevision: field.contentRevision, cursor });
}

export function chatDetailLocate(row: ChatRow, reference: string, chunkSequence: number): Promise<{ field: ChatDetailField; cursor: string; historyGeneration: string }> {
  const context = toolContexts.get(row);
  if (!context || context.protocolVersion !== 2) return Promise.reject(new Error(t("chat.history.unavailable")));
  return invoke("chat_locate", { sessionId: context.sessionId, childPath: context.childPath, rowId: row.id, detailRef: reference, chunkSequence });
}

const COMPLETE_COPY_BYTES=8*1024*1024;
let copyingCompleteDetail=false;
export function canCopyCompleteChatDetail(field: ChatDetailField) { return field.kind!=="image" && field.kind!=="binary" && field.sizeBytes !== undefined && field.sizeBytes <= COMPLETE_COPY_BYTES; }

export function chatDetailImage(row:ChatRow,field:ChatDetailField):ChatImageReference | undefined {
  const context=toolContexts.get(row);if (!context || context.protocolVersion!==2 || field.kind!=="image") return;
  const image={attachmentId:`history:${field.detailRef}`,mimeType:field.mimeType ?? "application/octet-stream",contentRevision:field.contentRevision,sizeBytes:field.sizeBytes};
  snapshotImageContexts.set(image,{sessionId:context.sessionId,cacheOwner:`${context.sessionId}\u0000${context.historyGeneration ?? "history"}`,historyGeneration:context.historyGeneration,childPath:context.childPath});return image;
}
async function readCompleteDetail(row:ChatRow,field:ChatDetailField,signal?:AbortSignal):Promise<string> {
  if (!canCopyCompleteChatDetail(field)) throw new Error("detail_requires_download: "+t("chat.history.clipboardLimit"));
  const parts:string[]=[];let bytes=0;let cursor:string|undefined;
  do {
    signal?.throwIfAborted();const chunk=await chatDetailRead(row,field,cursor);signal?.throwIfAborted();
    if (chunk.contentRevision!==field.contentRevision) throw new Error("source_changed: "+t("chat.history.contentChanged"));
    bytes+=new TextEncoder().encode(chunk.text).byteLength;
    if (bytes>COMPLETE_COPY_BYTES) throw new Error("detail_requires_download: "+t("chat.history.clipboardLimit"));
    parts.push(chunk.text);cursor=chunk.nextCursor??undefined;
  } while(cursor);
  if (bytes!==field.sizeBytes) throw new Error("source_changed: "+t("chat.history.contentChanged"));
  return parts.join("");
}

export async function readCompleteChatBody(row:ChatRow,signal?:AbortSignal):Promise<string> {
  if (copyingCompleteDetail) throw new Error(t("chat.history.copyBusy"));
  copyingCompleteDetail=true;
  try {
    const fields:ChatDetailField[]=[];let cursor:string|undefined;let bytes=0;
    do {
      signal?.throwIfAborted();const manifest=await chatManifest(row,cursor);signal?.throwIfAborted();
      for (const field of manifest.fields) if (field.label==="body") {
        if (!canCopyCompleteChatDetail(field)) throw new Error("detail_requires_download: "+t("chat.history.clipboardLimit"));
        bytes+=(field.literal!==undefined?new TextEncoder().encode(field.literal).byteLength:field.sizeBytes!+new TextEncoder().encode((field.prefix??"")+(field.suffix??"")).byteLength)+(fields.length && !field.append?1:0);
        if (bytes>COMPLETE_COPY_BYTES) throw new Error("detail_requires_download: "+t("chat.history.clipboardLimit"));
        fields.push(field);
      }
      cursor=manifest.nextCursor??undefined;
    } while(cursor);
    if (!fields.length) throw new Error(t("chat.history.unavailable"));
    const parts:string[]=[];for (const field of fields) {
      if (parts.length && !field.append) parts.push("\n");
      const original=await readCompleteDetail(row,field,signal);
      parts.push(field.literal??((field.prefix??"")+original+(field.suffix??"")));
    }
    return parts.join("");
  } finally {copyingCompleteDetail=false;}
}
export async function copyCompleteChatDetail(row: ChatRow, field: ChatDetailField,signal?: AbortSignal): Promise<void> {
  if (copyingCompleteDetail) throw new Error(t("chat.history.copyBusy"));
  if (field.sizeBytes === undefined || field.sizeBytes > COMPLETE_COPY_BYTES) throw new Error("detail_requires_download: "+t("chat.history.clipboardLimit"));
  copyingCompleteDetail=true;
  try {
    const text=await readCompleteDetail(row,field,signal);
    signal?.throwIfAborted();
    const { platform } = await import("../platform");
    signal?.throwIfAborted();
    await platform.clipboard.writeText(text, {reportFailure:true});
  } finally {copyingCompleteDetail=false;}
}
export async function downloadCompleteChatDetail(row: ChatRow,field: ChatDetailField): Promise<void> {
  const context=toolContexts.get(row);
  if (!context || context.protocolVersion!==2) throw new Error(t("chat.history.unavailable"));
  const request={sessionId:context.sessionId,childPath:context.childPath,detailRef:field.detailRef,expectedRevision:field.contentRevision};
  const imageExtensions:Record<string,string>={"image/png":"png","image/jpeg":"jpg","image/gif":"gif","image/webp":"webp"};
  const extension=field.kind==="image" ? (imageExtensions[field.mimeType ?? ""] ?? "bin") : field.kind==="json" ? "json" : field.kind==="diff" ? "diff" : "txt";
  const fileName=`history-detail.${extension}`;
  const { platform } = await import("../platform");
  if ((platform.env.isTauri || platform.env.isElectron) && !platform.env.isRemoteWindow) {
    const destination=await platform.dialog.saveFile({defaultPath:fileName});
    if (destination) await invoke("chat_detail_save",{...request,destPath:destination});
    return;
  }
  const url=await invoke<string>("chat_detail_download",request);
  const link=document.createElement("a");link.href=apiUrl(url);link.download=fileName;
  document.body.appendChild(link);link.click();link.remove();
}

export interface ChatSearchHit { rowId: string; snippet: string; detailRef?: string; chunkSequence?: number; offset: number; /** Literal strings the full-text index matched; absent for literal-scan hits. */ matched?: string[]; }
export interface ChatSearchPage { historyGeneration?: string; hits: ChatSearchHit[]; jobId?: string; nextCursor: string | null; coverage: "complete" | "partial" | "error"; scannedBytes: number; scannedRows?: number; }
export function chatSearch(sessionId: string, query: string, cursor?: string): Promise<ChatSearchPage> {
  return invoke("chat_search", { sessionId, query, cursor });
}
export function chatReadCancel(sessionId: string, jobId: string): Promise<void> { return invoke("chat_read_cancel", { sessionId, jobId }); }

interface ChatExportProgress { jobId: string; status: "pending" | "complete"; download?: string | null; sizeBytes?: number; }
/** Generate the complete document on disk. The client holds only job progress and the final ticket. */
export async function chatExportContext(sessionId: string, exportedAt: string, destination?: string, signal?: AbortSignal): Promise<string | null> {
  signal?.throwIfAborted();
  const job=await invoke<ChatExportProgress>("chat_export_start",{sessionId,destPath:destination,exportedAt});
  const cancel=() => void chatReadCancel(sessionId,job.jobId).catch(()=>{});
  signal?.addEventListener("abort",cancel,{once:true});
  try {
    signal?.throwIfAborted();
    let current=job;
    while (current.status!=="complete") {
      await new Promise<void>(resolve=>setTimeout(resolve,500));signal?.throwIfAborted();
      current=await invoke<ChatExportProgress>("chat_export_status",{sessionId,jobId:job.jobId});
    }
    signal?.throwIfAborted();return current.download ? apiUrl(current.download) : null;
  } catch (error) {cancel();throw error;}
  finally {signal?.removeEventListener("abort",cancel);}
}
export async function loadChatTool(row: Extract<ChatRow, { kind: "tool" }>): Promise<Extract<ChatRow, { kind: "tool" }>> {
  const context = toolContexts.get(row);
  if (!context) throw new Error("Tool details are no longer associated with a conversation");
  if (context.protocolVersion === 2) throw new Error("detail_requires_paging: "+t("chat.history.openViewer"));
  const result = await invoke<Extract<ChatRow, { kind: "tool" }>>("chat_row", { ...context, rowId: row.id });
  registerToolDetails(context.sessionId, context.epoch, [result]);
  registerSnapshotImages(`${context.sessionId}\u0000${context.epoch ?? "stopped"}`, context.sessionId, result);
  return result;
}

export function chatToolContextKey(row: ChatRow): string {
  const context = toolContexts.get(row);
  return `${context?.sessionId}:${context?.epoch}:${context?.historyGeneration}:${context?.childPath?.join("/")??""}:${row.id}`;
}

const rowPositions = new WeakMap<ChatRow, number>();
export function chatRowPosition(row: ChatRow) { return rowPositions.get(row); }
export function registerRowPositions(rows: ChatRow[], positions?: Record<string, number>) {
  for (const row of rows) if (positions?.[row.id] !== undefined) rowPositions.set(row, positions[row.id]);
}
