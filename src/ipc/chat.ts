//! Talking to an agent that runs as a protocol peer rather than in a terminal.
//!
//! Every call here names a session and nothing else; the backend owns the process, so a view can mount,
//! unmount, and mount again — or open on a second device — without disturbing the conversation. What the
//! agent is doing arrives on one event channel per session.

import { invoke, listen, type UnlistenFn } from "./transport";
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
}

/** A live row carries bytes; a snapshot row carries a reference until its image is mounted. */
export type ChatImageValue = ChatImage | ChatImageReference;

/** A small process-local cache prevents two panes in the same client from fetching the same image twice. */
const snapshotImageCache = new Map<string, ChatImage>();
const SNAPSHOT_IMAGE_CACHE_LIMIT = 16;
const snapshotImageContexts = new WeakMap<object, { sessionId: string; cacheOwner: string }>();

function rememberSnapshotImage(key: string, image: ChatImage): void {
  if (!snapshotImageCache.has(key) && snapshotImageCache.size >= SNAPSHOT_IMAGE_CACHE_LIMIT) {
    const oldest = snapshotImageCache.keys().next().value as string | undefined;
    if (oldest) snapshotImageCache.delete(oldest);
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
export async function resolveChatImage(image: ChatImageValue): Promise<ChatImage> {
  if ("data" in image) return image;
  const context = snapshotImageContexts.get(image);
  if (!context) throw new Error("This chat attachment is no longer associated with a snapshot");
  // Replayed row ids are stable only within one agent process. Including its start time prevents a new
  // branch that reuses `h-0` from receiving the previous process's image bytes from this client cache.
  const key = `${context.cacheOwner}\u0000${image.attachmentId}`;
  const cached = snapshotImageCache.get(key);
  if (cached) return cached;
  const resolved = await invoke<ChatImage>("chat_attachment", {
    sessionId: context.sessionId,
    attachmentId: image.attachmentId,
  });
  rememberSnapshotImage(key, resolved);
  return resolved;
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

export type ChatRow =
  | { kind: "user"; id: string; text: string; images?: ChatImageValue[]; at?: number }
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
    }
  | { kind: "error"; id: string; message: string }
  /**
   * What a slash command the agent answered itself printed back, such as `/effort` or `/context`.
   *
   * Its own row because it is neither side of the conversation: drawing it as an answer would credit
   * the model with words it never wrote.
   */
  | { kind: "command"; id: string; text: string }
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

/** One of Claude's background tasks: a shell command or a subagent still running after its turn moved on. */
export interface ChatBackgroundTask {
  task_id: string;
  task_type: string;
  description: string;
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
export interface ChatExtras {
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
  /** Images attached to it, waiting along with the text. */
  images?: ChatImageValue[];
}

/** What to do with a message typed while a turn is running. */
export type SendBehavior = "queue" | "interrupt" | "steer";

/** What a rewind should undo. File restoration is delegated to the agent; Codex supports only conversation. */
export type ChatRewindScope = "conversation" | "files" | "both";

/** Backend preview of the file checkpoint associated with a user turn. */
export interface ChatRewindPreview {
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

/** Everything needed to draw a conversation from scratch. */
export interface ChatSnapshot {
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
  /** Flattened `ChatExtras`; read them with `extrasOf`. */
  contextTokens?: number;
  contextWindow?: number;
  totalCostUsd?: number;
  contextUsage?: ChatExtras["contextUsage"];
  rateLimit?: ChatExtras["rateLimit"];
  backgroundTasks?: ChatBackgroundTask[];
  fastMode?: boolean;
  apiRetry?: ChatApiRetry;
}

/** The extras a snapshot carries, as one object. */
export function extrasOf(snapshot: ChatSnapshot): ChatExtras {
  return {
    contextTokens: snapshot.contextTokens,
    contextWindow: snapshot.contextWindow,
    totalCostUsd: snapshot.totalCostUsd,
    contextUsage: snapshot.contextUsage,
    rateLimit: snapshot.rateLimit,
    backgroundTasks: snapshot.backgroundTasks,
    fastMode: snapshot.fastMode,
    apiRetry: snapshot.apiRetry,
  };
}

/** What arrives on a session's chat channel. */
export type ChatEvent =
  | { type: "rows"; positions?: Record<string, number>; rows: ChatRow[]; revision?: number; epoch?: number }
  /** A rewind removed rows. Replace rather than merge so every connected view drops the same tail. */
  | { type: "replaceRows"; positions?: Record<string, number>; rows: ChatRow[]; revision?: number; epoch?: number; hasMore?: boolean }
  /** A new process took over this session: drop everything on screen and follow the rows that come next. */
  | { type: "reset"; epoch?: number }
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
  | { type: "error"; message: string }
  | { type: "extras"; extras: ChatExtras }
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
 * Answers `"sent"` or `"queued"`, so the composer can say which happened.
 */
export function chatSend(
  sessionId: string,
  text: string,
  behavior?: SendBehavior,
  images?: ChatImage[],
  messageId?: string,
): Promise<"sent" | "queued" | "command"> {
  return invoke("chat_send", { sessionId, text, behavior, images, messageId });
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

/** Stop the turn in progress; the conversation stays open. */
export function chatInterrupt(sessionId: string): Promise<void> {
  return invoke("chat_interrupt", { sessionId });
}

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
  options?: { updatedInput?: unknown; message?: string; updatedPermissions?: unknown[] },
): Promise<void> {
  return invoke("chat_permission", {
    sessionId,
    requestId,
    allow,
    updatedInput: options?.updatedInput,
    message: options?.message,
    updatedPermissions: options?.updatedPermissions,
  });
}

/** Change the permission mode of the running conversation. */
export function chatSetMode(sessionId: string, mode: string): Promise<void> {
  return invoke("chat_set_mode", { sessionId, mode });
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

/** Read the whole conversation; snapshot image bytes stay deferred until their individual views mount. */
export async function chatSnapshot(sessionId: string, window?: { before?: string; from?: string; since?: number; epoch?: number }): Promise<ChatSnapshot> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const snapshot = await Promise.race([
    invoke<ChatSnapshot>("chat_snapshot", { sessionId, ...(window ? { window } : {}) }),
    new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error("Conversation synchronization timed out")), 30_000); }),
  ]).finally(() => clearTimeout(timer));
  registerRowPositions(snapshot.rows, snapshot.positions);
  const cacheOwner = `${sessionId}\u0000${snapshot.startedAt ?? "stopped"}`;
  registerSnapshotImages(cacheOwner, sessionId, snapshot.rows);
  registerToolDetails(sessionId, snapshot.startedAt, snapshot.rows);
  registerSnapshotImages(cacheOwner, sessionId, snapshot.queue);
  return snapshot;
}

/** Preview which files Claude would restore, without changing files or conversation state. */
export function chatRewindPreview(sessionId: string, rowId: string): Promise<ChatRewindPreview> {
  return invoke("chat_rewind_preview", { sessionId, rowId });
}

/** Apply the confirmed rewind. The backend remains the authority for the resulting timeline. */
export function chatRewind(
  sessionId: string,
  rowId: string,
  scope: ChatRewindScope,
): Promise<ChatRewindResult> {
  return invoke("chat_rewind", { sessionId, rowId, scope });
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

/** Subscribe to everything happening in one conversation. */
export function onChatEvent(
  sessionId: string,
  cb: (event: ChatEvent) => void,
): Promise<UnlistenFn> {
  return listen<ChatEvent>(`chat://event/${sessionId}`, event => {
    if (event.type === "rows" || event.type === "replaceRows") {
      registerRowPositions(event.rows, event.positions);
      registerSnapshotImages(`${sessionId}\u0000${event.epoch ?? "live"}`, sessionId, event.rows);
      registerToolDetails(sessionId, event.epoch, event.rows);
    }
    cb(event);
  });
}

const toolContexts = new WeakMap<object, { sessionId: string; epoch?: number }>();
function registerToolDetails(sessionId: string, epoch: number | undefined, rows: ChatRow[]) {
  for (const row of rows) if (row.kind === "tool") {
    toolContexts.set(row, { sessionId, epoch });
    if (row.children) registerToolDetails(sessionId, epoch, row.children);
  }
}
export async function loadChatTool(row: Extract<ChatRow, { kind: "tool" }>): Promise<Extract<ChatRow, { kind: "tool" }>> {
  const context = toolContexts.get(row);
  if (!context) throw new Error("Tool details are no longer associated with a conversation");
  const result = await invoke<Extract<ChatRow, { kind: "tool" }>>("chat_row", { ...context, rowId: row.id });
  registerToolDetails(context.sessionId, context.epoch, [result]);
  registerSnapshotImages(`${context.sessionId}\u0000${context.epoch ?? "stopped"}`, context.sessionId, result);
  return result;
}

export function chatToolContextKey(row: ChatRow): string {
  const context = toolContexts.get(row);
  return `${context?.sessionId}:${context?.epoch}:${row.id}`;
}

const rowPositions = new WeakMap<ChatRow, number>();
export function chatRowPosition(row: ChatRow) { return rowPositions.get(row); }
export function registerRowPositions(rows: ChatRow[], positions?: Record<string, number>) {
  for (const row of rows) if (positions?.[row.id] !== undefined) rowPositions.set(row, positions[row.id]);
}
