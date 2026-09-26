//! A session driven by the chat engine: the whole interface, not a second view of a terminal.
//!
//! There is no PTY behind this pane. The backend runs the agent as a protocol peer, so everything the view
//! needs arrives structured: text as it is written, tool calls with their arguments, permission questions
//! that can be answered with a button, and the agent's own list of commands and skills.
//!
//! The pane owns none of that state. It reads a snapshot when it opens and then follows the session's event
//! channel, which means closing the pane, reopening it, or opening the same session on a phone all show the
//! same conversation. Closing the pane does let the agent process go once it is idle; the next message
//! sent starts it again where the conversation left off.

import { useAgentPermissions, savePermissionDefault } from "../../../hooks/useAgentPermissions";
import { useSessionPermissionState } from "../../../hooks/useSessionPermissionState";
import { currentPermissionLabel, PermissionStateDetails } from "../../../components/PermissionStateDetails";
import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { measureElement, observeElementOffset, useVirtualizer } from "@tanstack/react-virtual";

import Icons from "../../../components/Icons";
import { ComposerOptionsButton, useComposerOptions } from "./ComposerOptions";
import { ComposerToolbar } from "./ComposerToolbar";
import type { ComposerChip } from "./composerLayout";
import { ModelCatalogStatus } from "./ModelCatalogStatus";
import { StatusIndicator } from "../../../components/StatusIndicator";
import { useT, type I18nKey } from "../../../i18n";
import {
  chatAutoContinueCancel,
  chatInterrupt,
  chatModels,
  chatPermission,
  chatQueueRemove,
  chatQueueSteer,
  chatQueueUpdate,
  chatRewind,
  chatRewindPreview,
  resolveChatImage,
  chatSetMode,
  chatSetCollaborationMode,
  chatSetModel,
  chatSetEffort,
  chatSetFastMode,
  chatSetPersonality,
  chatSetServiceTier,
  chatCompact,
  chatReview,
  CODEX_PERSONALITIES,
  CODEX_STANDARD_TIER,
  chatStopTask,
  chatBackgroundTasks,
  extrasOf,
  chatAttach,
  chatCancelShell,
  chatDetach,
  chatRunShell,
  chatSnapshot,
  chatCommands,
  chatStart,
  isTaskFinished,
  onChatEvent,
  type ChatAutoContinue,
  type ChatAutoContinueReason,
  type ChatEvent,
  type ChatCommand,
  type ChatExtras,
  type ChatCollaborationMode,
  type ChatConfigKey,
  type ChatModel,
  type ChatImageValue,
  type ChatPermission,
  type PendingPermissionMode,
  type ChatRewindPreview,
  type ChatRewindScope,
  type ChatRow,
  type QueuedMessage,
  type SendBehavior,
} from "../../../ipc/chat";
import { attachImages, restoreAttachment, MAX_IMAGE_BYTES, MAX_IMAGES, type Attachment } from "./attachments";
import { attachChatInputGuard } from "./inputGuard";
import { buildSuggestions, findFileMention, mentionDir, type Suggestion } from "./completion";
import { env } from "../../../platform/env";
import { imageFromNativeClipboard, imagesFromClipboard, imagesFromDrop } from "../../../terminal/imageInput";
import { IS_MAC, IS_PLAIN_BROWSER } from "../../../hooks/shortcutRegistry";
import { useMentionFiles } from "./fileMentions";
import { ControlChip, LevelBar, type ChipOption } from "./controls";
import { AutoContinueBar, FastModeChip, McpChip, NotificationBar, RetryLine, TasksChip, UsageMeter } from "./extras";
import { AgentAccountMenu, AgentAuth } from "./AgentAuth";
import { CodexResetCredits } from "./CodexResetCredits";
import { useEngineSwitch } from "./engineSwitch";
import { ConversationViewHint } from "./ConversationViewHint";
import { AgentMissingNotice } from "./AgentMissingNotice";
import { isShareSurface } from "../../../ipc/shareBase";
import { isAgentNotInstalledError } from "../../../ipc/backendError";
import { usePermissionRestart } from "./permissionRestart";
import { PermissionCard, type PermissionAnswer } from "./permissionCards";
import { isMode, type Mode } from "./permissions";
import { useTermStore } from "../../../store/termStore";
import { effectivePermissionMode } from "../../../store/settings";
import { effectiveStatus, supportsPermissionToggle, type Session } from "../../../types";
import { kindIconEl } from "../../sessionViewers/sessionMeta";
import { assistantLabel } from "../../sessionViewers/TranscriptViewer";
import {
  CompactionRow,
  ChatImageView,
  CommandRow,
  ShellRow,
  ErrorRow,
  MessageBubble,
  NoticeRow,
  ReasoningRow,
  ToolCard,
  ToolRunCard,
  TurnHead,
  WorkingRow,
} from "./rows";
import {
  estimateRowHeight,
  foldAgentTurns,
  groupToolRuns,
  markAgentTurns,
  mountedStart,
  type DisplayRow,
  type TurnFold,
} from "./toolRuns";
import "./session-view.css";
import { onTransportReconnect } from "../../../ipc/transport";
import { useOutbox, emptySubmissions, acknowledgeSubmissions, createSubmission, deliverSubmission, retrySubmission, submissionsFor, ChatVersions } from "./outbox";
import { cachedChat, cacheChat, mergeRows, reconcileChat, chatSyncMetrics } from "./chatCache";
import { ChatSearch } from "./ChatSearch";
import { QueuedMessageText } from "./QueuedMessageText";
import { MessageSender, messageSenderName } from "./MessageSender";
import { parseShellSubmission, shellErrorKey, shellSubmissionFor, acknowledgeShellSubmission } from "./shellMode";
import { SessionLinkDirectory } from "./links";

interface MessageReplacement {
  text: string;
  images: ChatImageValue[];
}

/** Distance from the bottom still counted as "at the bottom", in pixels. */
const PIN_SLACK = 40;

/** Rows kept beyond the viewport on each side of the virtualized part, so scrolling reveals drawn rows. */
const OVERSCAN = 6;

/** Reasoning-effort values understood by the supported chat engines. */
const EFFORTS = ["minimal", "low", "medium", "high", "xhigh", "max", "ultra"] as const;
/**
 * The level above the ladder: extra-high effort with the agent's dynamic workflow orchestration on top.
 * It runs at `xhigh`, so it is offered only by the models that reach that level.
 */
const ULTRACODE = "ultracode" as const;
/**
 * Below the ladder: no extended thinking at all. Claude only; it is applied as a thinking-token cap of
 * zero rather than as a level, which is why the backend handles it apart from the others.
 */
const THINKING_OFF = "off" as const;
/** A level, or the empty string standing for the level the model picks itself. */
type EffortChoice = string;
/** Native catalogues may advertise new effort or variant names without a frontend release. */
/** What to tell the user when automatic continuation after a usage limit ends without sending anything. */
const AUTO_CONTINUE_REASONS: Record<ChatAutoContinueReason, I18nKey> = {
  unknownReset: "chat.autoContinue.unknownReset",
  repeated: "chat.autoContinue.repeated",
  failed: "chat.autoContinue.failed",
};

function isEffort(value: string): boolean {
  return value.trim().length > 0;
}
/** What the menu offers for a model: the model's own ladder, plus ultracode wherever xhigh is reachable. */
function levelsOf(ladder: readonly string[], kind: Session["kind"]): string[] {
  return kind === "claude" && ladder.includes("xhigh") ? [...ladder, ULTRACODE] : [...ladder];
}

function storedMode(session: Pick<Session, "kind" | "permissionMode">): Mode {
  const stored = session.permissionMode;
  if (session.kind === "codex") {
    if (stored === "skip" || stored === "full-access") return "full-access";
    return stored === "read-only" ? "read-only" : "auto";
  }
  if (session.kind === "opencode") {
    return stored === "skip" || stored === "bypassPermissions" ? "bypassPermissions" : "default";
  }
  if (stored === "skip") return "bypassPermissions";
  return stored && isMode(stored) ? stored : "default";
}

function effortKey(kind: Session["kind"], model: string): string {
  return `${kind}:${model}`;
}

function modeLabelKey(mode: Mode): I18nKey {
  if (mode === "read-only") return "chat.mode.readOnly";
  if (mode === "full-access") return "chat.mode.fullAccess";
  return `chat.mode.${mode}` as I18nKey;
}

/** OMP's `default` defers to its own approval configuration, which reads differently from OpenCode's. */
function modeLabelKeyFor(kind: Session["kind"], mode: Mode): I18nKey {
  return kind === "omp" && mode === "default" ? "chat.mode.agentDefault" : modeLabelKey(mode);
}

/** A Codex mode (`default`, `plan`) or an OpenCode agent name; the backend validates it against the catalogue. */
type CollaborationMode = ChatCollaborationMode["mode"];

function isCollaborationMode(value: string | null | undefined): value is CollaborationMode {
  return typeof value === "string" && value.length > 0;
}

function storedCollaborationMode(session: Session): CollaborationMode {
  if (isCollaborationMode(session.collaborationMode)) return session.collaborationMode;
  return session.kind === "codex" ? "default" : "";
}

/** How the chip and its menu name one preset: Codex modes by their translated names, agents by their own. */
function collaborationLabel(
  kind: Session["kind"],
  preset: ChatCollaborationMode | undefined,
  value: string,
  t: ReturnType<typeof useT>,
): string {
  if (kind === "codex" && (value === "default" || value === "plan")) {
    return t(`chat.collaborationMode.${value}` as "chat.collaborationMode.default");
  }
  const name = preset?.name ?? value;
  return name.charAt(0).toUpperCase() + name.slice(1);
}

export function ChatPane({
  session,
  cwd,
  area,
  hidden,
  focused,
  multi,
  paneId,
  readOnly = false,
  mobile = false,
  onActivate,
  onSplit,
  onClose,
}: {
  session: Session;
  cwd?: string;
  area: React.CSSProperties;
  hidden: boolean;
  focused: boolean;
  multi: boolean;
  paneId?: string;
  readOnly?: boolean;
  mobile?: boolean;
  onActivate: (paneId: string, id: string) => void;
  onSplit: (paneId: string, id: string, dir: "horizontal" | "vertical") => void;
  onClose: (paneId: string, id: string) => void;
}) {
  const t = useT();
  const composerOptions = useComposerOptions(mobile);
  const paneStyle = useTermStore((s) => s.paneStyle);
  const composerInlineChips = useTermStore((s) => s.composerInlineChips);
  const status = useTermStore((s) => effectiveStatus(s.runtimes[session.id]));
  const searchOpen = useTermStore((s) => s.searchOpen);
  const closeSearch = useTermStore((s) => s.closeSearch);
  const openTaskTab = useTermStore((s) => s.openTaskTab);
  const [searchTarget, setSearchTarget] = useState<string | null>(null);
  const unread = useTermStore((s) => session.id in s.notifications);
  const defaultMode = useTermStore((s) => storedMode({
    kind: session.kind,
    permissionMode: s.agentDefaults[session.kind]?.permissionMode,
  }));
  // Moving to the terminal restarts the agent under the other engine, so a working one is asked first.
  const { switchTo, confirm: engineConfirm } = useEngineSwitch(session);

  const [rows, setRows] = useState<ChatRow[]>(() => cachedChat(session.id)?.rows ?? []);
  const rowsRef = useRef(rows);
  rowsRef.current = rows;
  const snapshotRef = useRef(cachedChat(session.id));
  const liveVersions = useRef(new ChatVersions());
  const [syncState, setSyncState] = useState<"loading" | "ready" | "failed">("loading");
  const [hasMore, setHasMore] = useState(cachedChat(session.id)?.hasMore ?? false);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const historyBusy = useRef(false);
  const historyGeneration = useRef(0);
  const refreshRef = useRef<() => Promise<void>>(async () => {});
  const prependScroll = useRef<{ height: number; top: number; anchor?: { id: string; offset: number } } | null>(null);
  const [submissionReceipts, setSubmissionReceipts] = useState<boolean | null>(null);
  const pendingSubmissions = useOutbox(state => state.sessions[session.id] ?? emptySubmissions);
  // Messages typed while the agent was busy. They belong to the backend, not to this pane: a second view
  // of the same session has to show the same queue, and the queue has to outlive this pane being closed.
  const [queue, setQueueState] = useState<QueuedMessage[]>(() => cachedChat(session.id)?.queue ?? []);
  const [steeringQueue, setSteeringQueue] = useState(false);
  const steeringQueueRef = useRef(false);
  /** The queued message being rewritten in place, and the text so far. */
  const [editing, setEditing] = useState<{ id: string; text: string } | null>(null);
  const queueRef = useRef(queue);
  const setQueue = useCallback((next: React.SetStateAction<QueuedMessage[]>) => {
    const items = typeof next === "function" ? next(queueRef.current) : next;
    // Reconnect/events invalidate an editor before React commits the replacement DOM. A queued blur
    // or Enter handler must consult these latest facts rather than its previous render's closure.
    queueRef.current = items;
    setQueueState(items);
    setEditing(current => current && items.some(item => item.id === current.id && item.shellCommand === undefined) ? current : null);
  }, []);
  // Images pasted or dropped into the composer, waiting to go out with whatever is being written. They
  // belong to this pane rather than to the session: nothing has been handed over until send is pressed.
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  /** Why the last paste or drop was not taken in full — shown above the thumbnails, not in the transcript. */
  const [attachNote, setAttachNote] = useState<string | null>(null);
  const [permissions, setPermissions] = useState<ChatPermission[]>([]);
  const [commands, setCommands] = useState<ChatCommand[]>([]);
  const [catalogueLoading, setCatalogueLoading] = useState(false);
  const [catalogueError, setCatalogueError] = useState<string | null>(null);
  const [configKeys, setConfigKeys] = useState<ChatConfigKey[]>([]);
  const [model, setModel] = useState<string | undefined>(() => cachedChat(session.id)?.model);
  const defaultModel = useTermStore((s) =>
    s.chatModelByKind[session.kind] ?? (session.kind === "claude" ? s.chatModel : ""),
  );
  const rememberedDefaultEffort = useTermStore((s) =>
    s.chatEffortByModel[effortKey(session.kind, model ?? "")] ??
    (session.kind === "claude" ? s.chatEffortByModel[model ?? ""] : "") ?? "",
  );
  // The models this machine can switch to. Read from the backend rather than hardcoded here: the list
  // depends on the installed CLI version and on what the user configured, neither of which the view knows.
  const [catalogue, setCatalogue] = useState<ChatModel[]>([]);
  // "skip" is what a terminal-driven session stored for "stop asking me"; the agent's own word for it is
  // bypassPermissions, and that is what this control shows.
  const [mode, setMode] = useState<string>(() => storedMode({
    kind: session.kind,
    permissionMode: effectivePermissionMode(session, useTermStore.getState().agentDefaults),
  }));
  const [pendingPermissionMode, setPendingPermissionMode] = useState<PendingPermissionMode | null>(null);
  const permissionState = useSessionPermissionState(hidden ? undefined : session.id,
    `${session.permissionMode}:${mode}:${pendingPermissionMode?.current}:${pendingPermissionMode?.next}`);
  const [collaborationMode, setCollaborationMode] = useState<CollaborationMode>(() =>
    storedCollaborationMode(session),
  );
  // Codex owns this experimental catalogue. An empty list hides the control for older app-server builds
  // while ordinary Default-mode chat continues to work.
  const [collaborationModes, setCollaborationModes] = useState<ChatCollaborationMode[]>([]);
  const [effort, setEffort] = useState<EffortChoice>("");
  const [engineRunning, setEngineRunning] = useState(false);
  /** What the Claude process reports about itself: context, cost, rate limits, tasks, switches. */
  const [extras, setExtras] = useState<ChatExtras>({});
  // Codex only: how fast it answers and how it speaks. Empty means "as configured", which is also what
  // the chips read before a catalogue says the selected model offers anything else.
  const [serviceTier, setServiceTier] = useState("");
  const [personality, setPersonality] = useState("");
  // Callers waiting for the agent to open its native session, which the `session` event announces.
  // `/compact` and `/review` need the thread before they can be asked, unlike a message, which waits
  // in the backend queue on its own.
  const sessionWaiters = useRef<Array<() => void>>([]);
  /** A notification from the agent's loop, shown until dismissed or timed out. */
  const [notice, setNotice] = useState<{ text: string; priority: string } | null>(null);
  /** A usage limit stopped the conversation and it continues on its own at the given time. */
  const [autoContinue, setAutoContinue] = useState<ChatAutoContinue | null>(null);
  /** Bumped when the agent reports its own catalogue, so the model list is read again. */
  const [catalogueVersion, setCatalogueVersion] = useState(0);
  /** Backend-owned start of the logical turn currently in flight. */
  const [actionFeedback, setActionFeedback] = useState("");
  const [sending, setSending] = useState(false);
  const shellSubmission = useRef<{ sessionId: string; command: string; id: string; pending: boolean } | null>(null);
  const stopping = useRef(false);
  const stop = () => {
    if (stopping.current) return;
    stopping.current = true;
    setError(null);
    setActionFeedback(t("chat.stopping"));
    void chatInterrupt(session.id).catch((err) => {
      stopping.current = false;
      setActionFeedback("");
      setError(String(err));
    });
  };
  const [turnStartedAt, setTurnStartedAt] = useState<number | undefined>();
  const [error, setError] = useState<string | null>(null);
  /** Which failed send's missing-agent notice the user closed, so it does not reappear until the next one. */
  const [agentNoticeDismissed, setAgentNoticeDismissed] = useState<string | null>(null);
  const keepRestartPermission = useRef(false);
  const permissionCatalog = useAgentPermissions(session.kind);
  const permissionRestart = usePermissionRestart(session.id, () => {
    setMode("bypassPermissions");
    setError(null);
    if (keepRestartPermission.current) {
      keepRestartPermission.current = false;
      void savePermissionDefault(session.kind, "bypassPermissions").catch(err => setError(String(err)));
    }
  }, setError);
  const [draft, setDraft] = useState("");
  // Only the browsing position and unsent draft are local. Recall reads the backend's conversation and queue.
  const inputHistory = useRef<{
    id: string | null;
    draft: string;
    caret: number;
    loading: boolean;
  } | null>(null);
  const updateDraft = (text: string) => {
    inputHistory.current = null;
    setDraft(text);
  };
  useEffect(() => {
    inputHistory.current = null;
    return () => { inputHistory.current = null; };
  }, [session.id]);
  // Caret position in the draft. An `@` mention is read from where the caret is, not from the end of the
  // text, so that a path can be completed in the middle of a sentence that is already written.
  const [caret, setCaret] = useState(0);
  // The draft the completion list was dismissed for. Escape sets it; typing anything else changes the
  // draft and brings the list back, which is what someone who dismissed it by accident expects.
  const [dismissed, setDismissed] = useState<string | null>(null);
  /** Folded tool runs the reader has opened, by the id of the run's first call. */
  const [openRuns, setOpenRuns] = useState<ReadonlySet<string>>(() => new Set());
  /** Whether agent turns hide their interim work unless the reader chose otherwise for a turn. */
  const [foldAll, setFoldAll] = useState(false);
  /** Per-turn choices that differ from `foldAll`, by turn id; the pane-wide control clears them. */
  const [foldOverrides, setFoldOverrides] = useState<ReadonlyMap<string, boolean>>(() => new Map());
  /** Whether the view is parked away from the end, which is the only time the "back to the end" button is worth showing. */
  const [away, setAway] = useState(false);
  const [pendingRewind, setPendingRewind] = useState<{
    rowId: string;
    scope: ChatRewindScope;
    replacement?: MessageReplacement;
    preview?: ChatRewindPreview;
    loading: boolean;
  } | null>(null);
  const [rewinding, setRewinding] = useState(false);
  const [rewindScopes, setRewindScopes] = useState<ChatRewindScope[]>([]);
  /** Prevents a second submit while `/clear` is replacing this mounted session. */
  const [clientCommandRunning, setClientCommandRunning] = useState(false);
  /** A changing token opens the latest user's existing rewind menu, including after virtualization mounts it. */
  const [rewindRequest, setRewindRequest] = useState<{ rowId: string; token: number } | null>(null);
  const rewindRequestToken = useRef(0);

  const scrollRef = useRef<HTMLDivElement | null>(null);
  const scrollGeometry = useRef({ height: 0, viewport: 0 });
  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  // Where the caret belongs once a chosen suggestion has been rendered into the textarea.
  const placeCaret = useRef<number | null>(null);
  // Whether the reader is following the end of the conversation. Scrolling up parks the view, so an
  // arriving message does not pull the text out from under someone reading back.
  const pinnedRef = useRef(true);
  // Where the reader last was while the list could be seen. A hidden pane has no layout box, and the
  // WebView can bring the scroller back at offset 0 without a scroll event, so the position is
  // kept here and put back when the pane is shown again.
  const readerTop = useRef(0);
  /** Tells the virtualizer the scroller's real offset; set while it observes the scroller. */
  const syncVirtualOffset = useRef<(() => void) | null>(null);

  /** Add or replace rows by id. The engine addresses rows this way so a message can be revised as it lands. */
  const applyRows = useCallback((incoming: ChatRow[]) => {
    setRows(prev => mergeRows(prev, incoming, snapshotRef.current?.hasMore ?? false));
  }, []);

  // Follow the conversation, then start the agent. Subscribing first means nothing said during startup is
  // missed; the snapshot afterwards covers anything that happened before this pane existed.
  useEffect(() => {
    let disposed = false;
    setTurnStartedAt(undefined);
    const versions = new ChatVersions();
    liveVersions.current = versions;
    versions.epoch = cachedChat(session.id)?.startedAt ?? 0;
    versions.rows = cachedChat(session.id)?.rowsRevision ?? 0;
    versions.queue = cachedChat(session.id)?.queueRevision ?? 0;
    let buffered: ChatEvent[] | null = [];
    const handleEvent = (event: ChatEvent) => {
      if (disposed) return;
      switch (event.type) {
        case "rows":
          if (!versions.accept("rows", event.revision, event.epoch)) break;
          acknowledgeSubmissions(session.id, event.rows.map(row => row.id));
          applyRows(event.rows);

          break;
        case "replaceRows":
          historyGeneration.current += 1;
          setHistoryError(null);
          prependScroll.current = null;
          if (!versions.accept("rows", event.revision, event.epoch)) break;
          acknowledgeSubmissions(session.id, event.rows.map(row => row.id));
          setRows(event.rows);
          setHasMore(event.hasMore ?? false);
          setPendingRewind((pending) =>
            pending && event.rows.some((row) => row.id === pending.rowId) ? pending : null,
          );
          break;
        // A restarted conversation builds its timeline again from the recording. Keeping the rows of the
        // process that just ended would show the replayed history a second time, under the old rows.
        case "reset":
          historyGeneration.current += 1;
          setHistoryError(null);
          prependScroll.current = null;
          if (!versions.accept("rows", event.revision ?? (event.epoch === undefined ? undefined : 0), event.epoch)) break;
          versions.queue = 0;
          setRows(event.rows ?? []);
          acknowledgeSubmissions(session.id, (event.rows ?? []).map(row => row.id));
          setQueue([]);
          snapshotRef.current = undefined;
          setHasMore(event.hasMore ?? false);
          setEditing(null);
          setPermissions([]);
          setPendingPermissionMode(null);
          setCollaborationModes([]);
          setError(null);
          setTurnStartedAt(undefined);
          setExtras((prev) => ({ fastMode: prev.fastMode }));
          break;
        case "queued":
          if (!versions.accept("queue", event.revision, event.epoch)) break;
          acknowledgeSubmissions(session.id, event.items.map(item => item.id));
          setQueue(event.items);

          break;
        case "permission":
          setPermissions((prev) =>
            prev.some((p) => p.id === event.request.id) ? prev : [...prev, event.request],
          );
          break;
        case "permissionResolved":
          setPermissions((prev) => prev.filter((p) => p.id !== event.id));
          break;
        case "commands":
          setCommands(event.commands);
          break;
        case "configKeys":
          setConfigKeys(event.keys);
          break;
        case "session":
          sessionWaiters.current.splice(0).forEach((resolve) => resolve());
          if (event.model) setModel(event.model);
          if (event.effort && isEffort(event.effort)) setEffort(event.effort);
          if (isCollaborationMode(event.collaborationMode)) {
            setCollaborationMode(event.collaborationMode);
          }
          break;
        case "collaborationModes":
          setCollaborationModes(event.modes);
          if (isCollaborationMode(event.mode)) setCollaborationMode(event.mode);
          break;
        case "settingsChanged":
          if ("effort" in event) setEffort(event.effort ?? "");
          if ("model" in event) setModel(event.model ?? undefined);
          if (event.mode) setMode(event.mode);
          if ("pendingPermissionMode" in event) setPendingPermissionMode(event.pendingPermissionMode ?? null);
          break;
        case "codexSettingsChanged":
          if ("serviceTier" in event) setServiceTier(event.serviceTier ?? "");
          if ("personality" in event) setPersonality(event.personality ?? "");
          break;
        case "collaborationModeChanged":
          if (isCollaborationMode(event.mode)) setCollaborationMode(event.mode);
          break;
        // A chat session has no PTY, so nothing else records that a process exists. The Info panel reads
        // the same runtime a terminal writes, which is why these two facts go there rather than staying
        // in this pane: without them it shows "—" for started, uptime, and this session's CPU.
        case "process":
          setEngineRunning(true);
          setRewindScopes(event.rewindScopes ?? []);
          useTermStore.getState().setRuntime(session.id, {
            // A session that has never run has no runtime record at all, and `status` is the one field
            // every reader of one assumes is there.
            status: "running",
            pid: event.pid,
            startedAt: event.startedAt,
          });
          break;
        case "turnStarted":
          setActionFeedback("");
          setTurnStartedAt(event.startedAt);
          break;
        case "steerAccepted":
          setActionFeedback(t("chat.steerAccepted"));
          break;
        case "turnInterrupted":
          stopping.current = false;
          setActionFeedback(t("chat.stopped"));
          break;
        case "turnCompleted":
          stopping.current = false;
          setTurnStartedAt(undefined);
          break;
        case "exited":
          stopping.current = false;
          setActionFeedback("");
          setEngineRunning(false);
          setTurnStartedAt(undefined);
          // A process the application let go — its view closed here or on another device — is not
          // the agent failing; the next message simply starts it again. An agent that failed its
          // handshake has already said why. That message is the one worth keeping; the exit that
          // follows it only adds a code nobody can act on.
          if (!event.released) {
            setError((prev) => prev ?? (event.stderr.trim() || t("chat.exited", event.code)));
          }
          break;
        case "error":
          if (stopping.current) {
            stopping.current = false;
            setActionFeedback("");
          }
          // A late permission rejection arrives on this channel rather than as a command failure; it is
          // still a restart request, so it must open the confirmation instead of a bare error.
          if (!permissionRestart.handleError(event.message)) setError(event.message);
          break;
        case "extras":
          setExtras(event.extras);
          break;
        case "models":
          setCatalogueVersion((v) => v + 1);
          break;
        case "notification": {
          setNotice({ text: event.text, priority: event.priority });
          const ms = event.timeoutMs ?? 8000;
          if (ms > 0) window.setTimeout(() => setNotice((n) => (n?.text === event.text ? null : n)), ms);
          break;
        }
        case "autoContinue":
          setAutoContinue(event.waiting);
          if (event.reason) setNotice({ text: t(AUTO_CONTINUE_REASONS[event.reason]), priority: "high" });
          break;
        default:
          break;
      }
    };
    let un: ReturnType<typeof onChatEvent> | null = null;
    const subscribe = () => un ??= onChatEvent(session.id, event => {
      if (buffered) buffered.push(event);
      handleEvent(event);
    }).catch(error => { un = null; throw error; });
    let loading = false;
    let receiptsAvailable = false;
    const refresh = async () => {
      if (loading || disposed) return;
      loading = true;
      historyGeneration.current += 1;
      setHistoryError(null);
      setSyncState("loading");
      buffered = [];
      const requestStarted = performance.now();
      try {
        await subscribe();
        if (disposed) return;
        const known = snapshotRef.current;
        const response = await chatSnapshot(session.id, known ? { since: known.rowsRevision, epoch: known.startedAt, from: known.rows[0]?.id } : {});
        if (disposed) return;
        const pendingEvents = buffered ?? [];
        const snapshot = reconcileChat(response, rowsRef.current, pendingEvents);
        buffered = null;
        if (snapshot.startedAt === undefined || snapshot.startedAt >= versions.epoch) {
          setRows(snapshot.rows);
          setQueue(snapshot.queue);
          versions.epoch = snapshot.startedAt ?? versions.epoch;
          versions.rows = snapshot.rowsRevision ?? versions.rows;
          versions.queue = snapshot.queueRevision ?? versions.queue;
          acknowledgeSubmissions(session.id, [...snapshot.rows, ...snapshot.queue].map(row => row.id));
          setHasMore(snapshot.hasMore ?? false);
          snapshotRef.current = snapshot;
          cacheChat(session.id, snapshotRef.current);
        }
        setSyncState("ready");
        const metric = { requestMs: performance.now() - requestStarted, commitMs: 0, rows: snapshot.rows.length, mode: snapshot.pageKind ?? "full" };
        chatSyncMetrics.push(metric);
        if (chatSyncMetrics.length > 30) chatSyncMetrics.shift();
        requestAnimationFrame(() => { metric.commitMs = performance.now() - requestStarted - metric.requestMs; });
        receiptsAvailable = snapshot.submissionReceipts === true;
        setSubmissionReceipts(receiptsAvailable);
        setRewindScopes(snapshot.rewindScopes ?? []);
        setPermissions(snapshot.permissions);
        if (snapshot.running || snapshot.commands.length > 0) setCommands(snapshot.commands);
        setConfigKeys(snapshot.configKeys);
        setModel(snapshot.model);
        setEngineRunning(snapshot.running);
        setExtras(
          snapshot.running
            ? extrasOf(snapshot)
            : { auth: snapshot.auth, fastMode: useTermStore.getState().chatFastModeByKind[session.kind] ?? false },
        );
        setTurnStartedAt(snapshot.running ? snapshot.turnStartedAt : undefined);
        if (snapshot.effort && isEffort(snapshot.effort)) setEffort(snapshot.effort);
        if (snapshot.mode) setMode(snapshot.mode);
        setPendingPermissionMode(snapshot.pendingPermissionMode ?? null);
        setCollaborationModes(snapshot.collaborationModes ?? []);
        if (isCollaborationMode(snapshot.collaborationMode)) {
          setCollaborationMode(snapshot.collaborationMode);
        }
        if (session.kind === "codex") {
          setServiceTier(snapshot.serviceTier ?? "");
          setPersonality(snapshot.personality ?? "");
        }
        setAutoContinue(snapshot.autoContinue ?? null);
        // Only while a process is actually behind the conversation: a snapshot taken after one exited
        // still names it, and reporting that as running would light the session up in every sidebar.
        if (snapshot.running && snapshot.pid !== undefined) {
          useTermStore.getState().setRuntime(session.id, {
            status: "running",
            pid: snapshot.pid,
            startedAt: snapshot.startedAt,
          });
        }
        if (!snapshot.running) {
          // A conversation opens on the pair last chosen by hand: the model, and the effort that was
          // picked for that model. The snapshot of a process already running wins over both, because
          // whatever it was started with is what is actually answering.
          //
          // The agent itself is not started here. Opening a conversation is for reading it; the process
          // starts with the first message sent, and so does anything that can go wrong with starting it.
          const st = useTermStore.getState();
          const carried = st.takePendingChatStart(session.id);
          const remembered = carried
            ? carried.model ?? ""
            : snapshot.selection ? snapshot.selection.model ?? ""
            : snapshot.model ?? st.chatModelByKind[session.kind] ?? (session.kind === "claude" ? st.chatModel : "");
          const rememberedEffort = carried
            ? carried.effort ?? ""
            : snapshot.selection ? snapshot.selection.effort ?? ""
            : snapshot.effort ?? st.chatEffortByModel[effortKey(session.kind, remembered)] ??
              (session.kind === "claude" ? st.chatEffortByModel[remembered] : "") ??
              "";
          setModel(remembered || undefined);
          setEffort(isEffort(rememberedEffort) ? rememberedEffort : "");
        }
        pendingEvents.filter(event => !["rows", "replaceRows", "queued", "reset", "notification", "models"].includes(event.type)).forEach(handleEvent);
      } catch (err) {
        if (!disposed) { setError(String(err)); setSyncState("failed"); }
      } finally {
        const events = buffered ?? [];
        buffered = null;
        loading = false;
        // Events were rendered as they arrived, including when the snapshot failed.
        void events;
      }
    };
    refreshRef.current = refresh;
    void refresh();
    const reconnect = onTransportReconnect(() => {
      void refresh().then(() => {
        if (!disposed && receiptsAvailable) for (const item of submissionsFor(session.id)) {
          if (item.status === "unknown") void retrySubmission(session.id, item);
        }
      });
    });
    return () => {
      disposed = true;
      historyGeneration.current += 1;
      reconnect();
      void un?.then((f) => f()).catch(() => {});
    };
  }, [session.id, session.kind, applyRows, t]);

  useEffect(() => {
    if (snapshotRef.current) {
      snapshotRef.current = { ...snapshotRef.current, auth: extras.auth, rows, queue, hasMore, rowsRevision: liveVersions.current.rows, queueRevision: liveVersions.current.queue };
      cacheChat(session.id, snapshotRef.current);
    }
  }, [session.id, rows, queue, hasMore, extras.auth]);

  const loadHistory = async (before = rows[0]?.id) => {
    if (historyBusy.current || !hasMore || syncState !== "ready" || !before) return;
    historyBusy.current = true;
    setHistoryLoading(true);
    setHistoryError(null);
    const generation = historyGeneration.current;
    try {
      const page = await chatSnapshot(session.id, { before, epoch: snapshotRef.current?.startedAt });
      if (generation !== historyGeneration.current) return;
      if (page.pageKind !== "history") { await refreshRef.current(); return; }
      const scroll = scrollRef.current;
      if (scroll && !pinnedRef.current) {
        const viewport = scroll.getBoundingClientRect();
        const anchor = Array.from(scroll.querySelectorAll<HTMLElement>(".sv-item[data-search-id]"))
          .find(element => element.getBoundingClientRect().bottom > viewport.top && element.getBoundingClientRect().top < viewport.bottom);
        prependScroll.current = {
          height: scroll.scrollHeight, top: scroll.scrollTop,
          anchor: anchor ? { id: anchor.dataset.searchId!, offset: anchor.getBoundingClientRect().top - viewport.top } : undefined,
        };
      }
      setRows(current => mergeRows(page.rows, current));
      setHasMore(page.hasMore ?? false);
      return page;
    } catch (error) { if (generation === historyGeneration.current) setHistoryError(String(error)); }
    finally { historyBusy.current = false; setHistoryLoading(false); }
  };

  const recallInput = (direction: "ArrowUp" | "ArrowDown") => {
    if (direction === "ArrowDown" && !inputHistory.current) return false;
    const browsing = inputHistory.current ?? {
      id: null, draft, caret: inputRef.current?.selectionStart ?? draft.length, loading: false,
    };
    inputHistory.current = browsing;
    // Down cancels an outstanding older-page request; its result must not replace the restored draft.
    if (browsing.loading && direction === "ArrowUp") return true;
    const entriesOf = (history: ChatRow[]) => {
      const entries = new Map<string, { id: string; text: string }>();
      for (const row of history) {
        if (row.kind === "user" && !row.origin && row.text.trim()) entries.set(row.id, row);
      }
      for (const item of [...queue, ...pendingSubmissions]) {
        if ("origin" in item && item.origin) continue;
        if (item.text.trim() && !entries.has(item.id)) entries.set(item.id, item);
      }
      return [...entries.values()];
    };
    const apply = (entry?: { id: string; text: string }) => {
      const text = entry?.text ?? browsing.draft;
      const at = entry ? (direction === "ArrowUp" ? 0 : text.length) : browsing.caret;
      inputHistory.current = entry ? { ...browsing, id: entry.id, loading: false } : null;
      setDraft(text);
      setCaret(at);
      setDismissed(text);
      placeCaret.current = at;
      // Adjacent submissions can have identical text, so caret placement cannot depend on a rerender.
      if (inputRef.current?.value === text) {
        inputRef.current.setSelectionRange(at, at);
        placeCaret.current = null;
      }
    };
    let history = rows;
    const entries = entriesOf(history);
    const index = browsing.id === null ? entries.length : entries.findIndex(entry => entry.id === browsing.id);
    if (direction === "ArrowDown") {
      apply(index >= 0 ? entries[index + 1] : undefined);
    } else if (index > 0) {
      apply(entries[index - 1]);
    } else if (hasMore) {
      browsing.loading = true;
      const generation = historyGeneration.current;
      void (async () => {
        try {
          let before = history[0]?.id;
          while (before && inputHistory.current === browsing) {
            const page = await loadHistory(before);
            if (!page || inputHistory.current !== browsing || generation !== historyGeneration.current) return;
            history = mergeRows(page.rows, history);
            const older = entriesOf(history);
            const at = browsing.id === null ? older.length : older.findIndex(entry => entry.id === browsing.id);
            if (at > 0) { apply(older[at - 1]); return; }
            const next = page.rows[0]?.id;
            if (!page.hasMore || !next || next === before) return;
            before = next;
          }
        } finally {
          browsing.loading = false;
        }
      })();
    }
    return true;
  };
  // Searching requests the remaining history explicitly; ordinary navigation stays bounded.
  useEffect(() => { if (searchOpen && focused && !hidden && hasMore && !historyError) void loadHistory(); }, [searchOpen, focused, hidden, hasMore, rows.length, syncState, historyError]);

  // Tell the backend when a view holds this conversation and when it lets go. A pane is unmounted only
  // when its session leaves every tab's layout — a background tab keeps it mounted — so letting go
  // means the view was closed. The agent process then ends once it is idle; the conversation stays, and
  // the next message sent starts the agent again where it left off. A pane mounted while a turn still
  // runs cancels the release that a closed one asked for.
  useEffect(() => {
    void chatAttach(session.id).catch(() => {});
    return () => {
      void chatDetach(session.id).catch(() => {});
    };
  }, [session.id]);

  // Read the installed agent's model catalogue once per pane. Discovery may spawn a short-lived CLI, so
  // it stays out of the snapshot path; failure is silent because an empty list simply hides the chip.
  useEffect(() => {
    let disposed = false;
    void chatModels(session.id)
      .then((list) => {
        if (!disposed) setCatalogue(list);
      })
      .catch(() => {});
    return () => {
      disposed = true;
    };
  }, [session.id, catalogueVersion]);

  // What the list draws, rather than what the engine sent: a burst of tool calls becomes one folded row,
  // so a turn that read a dozen files does not bury the answer that came out of it. The first entry of
  // each agent turn is then marked with the author line the view draws above everything the agent did.
  //
  // Reasoning rows with no text are dropped here. The agent may report that it thought without disclosing
  // what it thought: the block arrives carrying only a signature, and its text stays empty for the whole
  // turn. A heading that opens onto nothing is worse than no heading, so such a row is not drawn at all.
  const turnEntries = useMemo(
    () => markAgentTurns(groupToolRuns(rows.filter((r) => r.kind !== "reasoning" || r.text.trim() !== ""))),
    [rows],
  );
  // Turns the reader collapsed keep only their answer. Search still reads `turnEntries`, so a match inside
  // hidden work is found and its turn opened.
  const folded = useMemo(
    () => foldAgentTurns(turnEntries, (id) => foldOverrides.get(id) ?? foldAll),
    [turnEntries, foldOverrides, foldAll],
  );
  const display = folded.rows;
  const allTurnsCollapsed = folded.turns.length > 0 && folded.turns.every((turn) => turn.collapsed);
  // Everything before this index is virtualized; the tail after it stays really mounted, because those
  // are the rows still growing as text arrives, and a row that changes height while a virtualizer is
  // measuring it is how a view ends up jumping under the reader.
  const split = useMemo(() => mountedStart(display), [display]);
  const virtualRows = useMemo(() => display.slice(0, split), [display, split]);
  const mountedRows = useMemo(() => display.slice(split), [display, split]);

  const virtualizer = useVirtualizer({
    count: virtualRows.length,
    enabled: virtualRows.length > 0,
    // Search previews resize the viewport while locating a result changes the mounted rows.
    // Defer observer-driven measurements so rendering cannot resize observed elements in the
    // same notification cycle (reported as a ResizeObserver loop by the desktop WebView).
    useAnimationFrameWithResizeObserver: true,
    getScrollElement: () => scrollRef.current,
    getItemKey: (index) => virtualRows[index]?.id ?? index,
    estimateSize: (index) => estimateRowHeight(virtualRows[index]),
    // A hidden pane has no layout, so everything in it measures zero. Taking those measurements would
    // collapse the list and lose the scroll position of every pane that is not the one on screen, so a
    // zero is read as "not measurable now" and the last real height stands.
    measureElement: (el, entry, instance) => {
      const size = measureElement(el, entry, instance);
      if (size > 0) return size;
      const index = instance.indexFromElement(el);
      const key = instance.options.getItemKey(index);
      return instance.itemSizeCache.get(key) ?? estimateRowHeight(virtualRows[index]);
    },
    // The virtualizer learns the offset only from scroll events. A scroller without layout reads as 0, and
    // one coming back from display:none may change offset without any event; the rows would then be drawn
    // around a position the reader is not at, leaving the view blank until the next scroll. Hidden reads
    // are dropped, and showing the pane reports the real offset through `syncVirtualOffset`.
    observeElementOffset: (instance, notify) => {
      const element = instance.scrollElement;
      if (!element) return;
      const stop = observeElementOffset(instance, (offset, isScrolling) => {
        if (element.clientHeight > 0) notify(offset, isScrolling);
      });
      const sync = () => notify(element.scrollTop, false);
      syncVirtualOffset.current = sync;
      return () => {
        stop?.();
        if (syncVirtualOffset.current === sync) syncVirtualOffset.current = null;
      };
    },
    overscan: OVERSCAN,
  });

  useLayoutEffect(() => {
    const previous = prependScroll.current;
    const scroll = scrollRef.current;
    if (!previous || !scroll) return;
    prependScroll.current = null;
    // Keep the visible message at its original pixel offset. Total-height differences also include
    // new tail output and estimated heights, neither of which belongs to the reader's anchor.
    const anchor = previous.anchor;
    const element = anchor && Array.from(scroll.querySelectorAll<HTMLElement>(".sv-item[data-search-id]"))
      .find(item => item.dataset.searchId === anchor.id);
    if (element && anchor) {
      scroll.scrollTop += element.getBoundingClientRect().top - scroll.getBoundingClientRect().top - anchor.offset;
    } else {
      const index = anchor ? virtualRows.findIndex(row => row.id === anchor.id) : -1;
      const offset = index >= 0 ? virtualizer.getOffsetForIndex(index, "start")?.[0] : undefined;
      const container = scroll.querySelector<HTMLElement>(".sv-virtual");
      if (offset !== undefined && container && anchor) {
        const start = container.getBoundingClientRect().top - scroll.getBoundingClientRect().top + scroll.scrollTop;
        scroll.scrollTop = start + offset - anchor.offset;
      } else {
        scroll.scrollTop = previous.top + scroll.scrollHeight - previous.height;
      }
    }
  }, [rows, virtualRows, virtualizer]);

  /** Scroll a drawn entry to the middle of the view; false when the entry is not in the list. */
  const scrollToEntry = useCallback((id: string) => {
    const index = display.findIndex(entry => entry.id === id);
    if (index < 0) return false;
    if (index < split) virtualizer.scrollToIndex(index, { align: "center" });
    else scrollRef.current?.querySelectorAll<HTMLElement>(".sv-item[data-search-id]").forEach(element => {
      const root = scrollRef.current;
      if (root && element.dataset.searchId === id) root.scrollTop += element.getBoundingClientRect().top - root.getBoundingClientRect().top - root.clientHeight / 2;
    });
    return true;
  }, [display, split, virtualizer]);
  /** A search match inside a collapsed turn, waiting for the opened turn to be drawn. */
  const pendingLocate = useRef<string | null>(null);

  const locateSearch = useCallback((index: number) => {
    const entry = turnEntries[index];
    setSearchTarget(entry?.id ?? null);
    if (!entry) return;
    pinnedRef.current = false;
    setAway(true);
    if (entry.kind === "run") setOpenRuns(previous => previous.has(entry.id) ? previous : new Set([...previous, entry.id]));
    const turn = folded.hiddenIn.get(entry.id);
    if (turn) {
      pendingLocate.current = entry.id;
      setFoldOverrides(previous => new Map(previous).set(turn, false));
      return;
    }
    pendingLocate.current = null;
    scrollToEntry(entry.id);
  }, [turnEntries, folded, scrollToEntry]);
  useEffect(() => {
    const id = pendingLocate.current;
    if (id && scrollToEntry(id)) pendingLocate.current = null;
  }, [scrollToEntry]);

  // Measuring a row above the viewport moves everything below it. While the reader is parked mid-history
  // that would drag the text they are reading, so the scroll position is corrected to absorb the change;
  // while they are at the end there is nothing to protect, and correcting would fight the follow below.
  useEffect(() => {
    virtualizer.shouldAdjustScrollPositionOnItemSizeChange = (item) => {
      if (pinnedRef.current) return false;
      const scroll = scrollRef.current;
      const container = scroll?.querySelector<HTMLElement>(".sv-virtual");
      if (!scroll || !container) return false;
      // A partially visible row may grow below the viewport's top without moving its own top.
      // Only fully preceding rows should move the viewport; include the history control's height.
      const viewportStart = scroll.getBoundingClientRect().top - container.getBoundingClientRect().top;
      return item.end <= viewportStart;
    };
    return () => {
      virtualizer.shouldAdjustScrollPositionOnItemSizeChange = undefined;
    };
  }, [virtualizer]);

  /** Open or close one folded run of tool calls. */
  const toggleRun = useCallback((id: string) => {
    setOpenRuns((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  }, []);

  /** Hide or show one turn's interim work. */
  const toggleTurn = useCallback((fold: TurnFold) => {
    setFoldOverrides(previous => new Map(previous).set(fold.id, !fold.collapsed));
  }, []);

  /** Hide or show the interim work of every turn, keeping the reader's place when parked mid-history. */
  const toggleAllTurns = () => {
    const next = !allTurnsCollapsed;
    const scroll = scrollRef.current;
    if (scroll && !pinnedRef.current) {
      const kept = new Set(foldAgentTurns(turnEntries, () => next).rows.map(entry => entry.id));
      const viewport = scroll.getBoundingClientRect();
      const anchor = Array.from(scroll.querySelectorAll<HTMLElement>(".sv-item[data-search-id]"))
        .find(element => kept.has(element.dataset.searchId!) && element.getBoundingClientRect().bottom > viewport.top);
      prependScroll.current = {
        height: scroll.scrollHeight, top: scroll.scrollTop,
        anchor: anchor ? { id: anchor.dataset.searchId!, offset: anchor.getBoundingClientRect().top - viewport.top } : undefined,
      };
    }
    setFoldAll(next);
    setFoldOverrides(new Map());
  };

  /** Put the view back at the end of the conversation and follow it again. */
  const toEnd = useCallback(() => {
    pinnedRef.current = true;
    setAway(false);
    const el = scrollRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
      scrollGeometry.current = { height: el.scrollHeight, viewport: el.clientHeight };
    }
  }, []);

  // Keep observers alive while the pane is visible. Draft/caret updates do not change the
  // transcript geometry; only viewport or row resizing and added/removed rows need a follow.
  useLayoutEffect(() => {
    if (hidden) return;
    const el = scrollRef.current;
    if (!el) return;
    let frame: number | undefined;
    const follow = () => {
      frame = undefined;
      const height = el.scrollHeight;
      const viewport = el.clientHeight;
      if (pinnedRef.current) el.scrollTop = height;
      scrollGeometry.current = { height, viewport };
    };
    const schedule = () => {
      if (frame === undefined) frame = requestAnimationFrame(follow);
    };
    // Coming back from display:none: put a parked reader back where they were, then give the virtualizer
    // the offset the scroller really has, which no scroll event may have reported.
    if (!pinnedRef.current && el.scrollTop !== readerTop.current) el.scrollTop = readerTop.current;
    follow();
    syncVirtualOffset.current?.();
    const observer = typeof ResizeObserver === "undefined" ? undefined : new ResizeObserver(schedule);
    observer?.observe(el);
    const observed = new Set<Element>();
    const syncChildren = () => {
      for (const child of observed) {
        if (child.parentElement !== el) {
          observer?.unobserve(child);
          observed.delete(child);
        }
      }
      for (const child of el.children) {
        if (!observed.has(child)) {
          observer?.observe(child);
          observed.add(child);
        }
      }
      schedule();
    };
    syncChildren();
    const mutations = new MutationObserver(syncChildren);
    mutations.observe(el, { childList: true });
    return () => {
      observer?.disconnect();
      mutations.disconnect();
      if (frame !== undefined) cancelAnimationFrame(frame);
    };
  }, [hidden, session.id]);

  // Claude's session stays working between turns while its background tasks run. That hold is not a turn, so
  // the composer's stop control and queueing do not follow it.
  const heldForBackground =
    turnStartedAt === undefined && (extras.backgroundTasks ?? []).some((task) => !isTaskFinished(task));
  const busy = status === "working" && !heldForBackground;
  const canRewind =
    !readOnly &&
    engineRunning &&
    !busy &&
    !rewinding &&
    !sending &&
    !clientCommandRunning &&
    queue.length === 0 &&
    permissions.length === 0;
  const rewindDisabledReason = !engineRunning
    ? t("chat.rewind.inactive")
    : !canRewind ? t("chat.command.rewindUnavailable") : undefined;
  const finishRewindRequest = useCallback((token: number) => {
    setRewindRequest((request) => request?.token === token ? null : request);
  }, []);

  const askRewind = useCallback((rowId: string, scope: ChatRewindScope, replacement?: MessageReplacement) => {
    if (!canRewind || !rewindScopes.includes(scope)) return;
    setError(null);
    if (scope === "conversation") {
      setPendingRewind({ rowId, scope, replacement, loading: false });
      return;
    }
    setPendingRewind({ rowId, scope, loading: true });
    void chatRewindPreview(session.id, rowId)
      .then((preview) => setPendingRewind((current) => current?.rowId === rowId && current.scope === scope
        ? { rowId, scope, preview, loading: false } : current))
      .catch((err) => {
        setPendingRewind(null);
        setError(String(err));
      });
  }, [canRewind, rewindScopes, session.id]);

  const rewindTarget = pendingRewind ? rows.find(row => row.id === pendingRewind.rowId) : undefined;

  const doRewind = async () => {
    const pending = pendingRewind;
    if (!pending || pending.loading || !canRewind || pending.preview?.canRewind === false) return;
    const target = rows.find(row => row.id === pending.rowId);
    if (!target || target.kind !== "user") return;
    if (pending.replacement !== undefined && submissionReceipts !== true) {
      setError(t("chat.submission.updateRequired"));
      return;
    }
    setRewinding(true);
    setError(null);
    try {
      // Resolve attachments before removing their snapshot from the conversation.
      const images = pending.scope !== "files"
        ? await Promise.all((pending.replacement?.images ?? target.images ?? []).map(resolveChatImage)) : [];
      const restored = pending.replacement === undefined ? images.map(restoreAttachment) : [];
      const result = await chatRewind(session.id, pending.rowId, pending.scope);
      if (pending.scope !== "files") {
        setRows(current => {
          const index = current.findIndex(row => row.id === pending.rowId);
          return index < 0 ? current : current.slice(0, index);
        });
        if (pending.replacement === undefined && result.prefillText && !draft.trim()) {
          updateDraft(result.prefillText);
          setCaret(result.prefillText.length);
          placeCaret.current = result.prefillText.length;
        }
        if (restored.length > 0) {
          // Keep unsent attachments as well; a rewind must not silently discard either set of images.
          setAttachments(current => [...restored, ...current]);
        }
      }
      setPendingRewind(null);
      if (pending.replacement !== undefined) {
        const item = createSubmission(session.id, pending.replacement.text, images.map(({ mimeType, data }) => ({ mimeType, data })), "queue");
        toEnd();
        await deliverSubmission(session.id, item);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setRewinding(false);
    }
  };

  /** Start the agent; the backend restores this conversation's saved Codex settings. */
  const startAgent = async () => {
    await chatStart(session.id, model, effort || undefined, extras.fastMode === true);
  };

  /** Stop a running `!` command; its row reports the cancelled state through the event channel. */
  const cancelShell = useCallback((rowId: string) => {
    chatCancelShell(session.id, rowId).catch((err) => setError(String(err)));
  }, [session.id]);

  /** Hand a one-click install to the terminal view: it is the view with a shell to run the recipe in,
   *  and its own card completes the installation and detects the new executable path. */
  const installAgent = () => {
    useTermStore.getState().setRuntime(session.id, { agentAutoInstall: true });
    switchTo("tui");
  };

  /** Make sure the agent is running and has opened its native session. */
  const ensureStarted = async () => {
    if (engineRunning) return;
    const opened = new Promise<void>((resolve, reject) => {
      const timer = window.setTimeout(() => reject(new Error(t("chat.command.startTimeout"))), 30_000);
      sessionWaiters.current.push(() => {
        window.clearTimeout(timer);
        resolve();
      });
    });
    await startAgent();
    await opened;
  };

  /**
   * Send what is typed, or decide what happens to it when the agent is busy.
   *
   * `queue` is the default because it is what someone typing a follow-up almost always means: the thought
   * lands now, the agent takes it when it comes up for air. `interrupt` is the deliberate other choice —
   * stop what you are doing and read this instead.
   */
  const send = (behavior: SendBehavior = "queue") => {
    if (sending || shellSubmission.current?.pending) return;
    const text = draft.trim();
    // A picture on its own is a message: dropping a screenshot in and pressing send says enough.
    if (rewinding || pendingRewind) return;
    if (!text && attachments.length === 0) return;
    if (attachments.length > MAX_IMAGES) {
      setAttachNote(t("chat.attach.tooMany", MAX_IMAGES));
      return;
    }
    // `!` runs the rest in the session's shell, as the terminal UI does: never a prompt, so it takes no
    // send behavior and no images. The row it produces comes back through the event channel; the agent
    // is told the result when the command ends, so it is started first like for any message.
    const shell = parseShellSubmission(text);
    if (shell) {
      if (attachments.length > 0) {
        setAttachNote(t("chat.shell.noImages"));
        return;
      }
      if (!shell.command) {
        setAttachNote(t("chat.shell.emptyCommand"));
        return;
      }
      setError(null);
      setDismissed(null);
      setAttachNote(null);
      toEnd();
      const submission = shellSubmissionFor(session.id, shell.command);
      if (submission.pending) return;
      submission.pending = true;
      shellSubmission.current = submission;
      setSending(true);
      const submittedDraft = draft;
      void (async () => {
        try {
          await chatRunShell(session.id, shell.command, submission.id);
          // The typed line leaves the composer only once the command was accepted; a refusal or a
          // failed start keeps it there for a retry, like the outbox keeps a failed message.
          setDraft((current) => current === submittedDraft ? "" : current);
          acknowledgeShellSubmission(submission);
          shellSubmission.current = null;
        } catch (err) {
          const key = shellErrorKey(err);
          setError(key ? t(key) : String(err));
        } finally {
          submission.pending = false;
          setSending(false);
        }
      })();
      return;
    }
    // Paseo treats these as client commands: they change the surrounding session UI and must never be
    // forwarded to Claude or Codex as user prose. Arguments or attachments deliberately opt out.
    const localCommand =
      behavior !== "steer" && attachments.length === 0 && /^\/(clear|new|rewind)$/.exec(text)?.[1];
    // These agents answer compact (and, for Codex, review) through requests of their own rather than as
    // user prose. They need the process started, and for Codex its native thread open, first.
    const chatCommand =
      behavior !== "steer" && attachments.length === 0
        ? /^\/(compact|review)(?:\s+([\s\S]*))?$/.exec(text)
        : null;
    const commandSupported =
      chatCommand?.[1] === "compact"
        ? session.kind === "codex" || session.kind === "pi" || session.kind === "omp"
        : session.kind === "codex";
    if (chatCommand && commandSupported) {
      const [, name, args] = chatCommand;
      setError(null);
      updateDraft("");
      setCaret(0);
      setDismissed(null);
      toEnd();
      void (async () => {
        try {
          await ensureStarted();
          if (name === "compact") await chatCompact(session.id);
          else await chatReview(session.id, args ?? "");
        } catch (err) {
          setError(String(err));
        }
      })();
      return;
    }
    if (localCommand === "rewind") {
      const latestUser = [...rows].reverse().find((row) => row.kind === "user");
      if (!canRewind || !latestUser) {
        setError(t("chat.command.rewindUnavailable"));
        return;
      }
      setError(null);
      updateDraft("");
      setCaret(0);
      setDismissed(null);
      rewindRequestToken.current += 1;
      // Keep the requested message in view when opening its menu triggers another layout.
      // Mounted messages need the same pause in tail following as virtualized messages.
      pinnedRef.current = false;
      setRewindRequest({ rowId: latestUser.id, token: rewindRequestToken.current });
      const displayIndex = display.findIndex(
        (entry) => entry.kind === "row" && entry.row.id === latestUser.id,
      );
      if (displayIndex >= 0 && displayIndex < virtualRows.length) {
        virtualizer.scrollToIndex(displayIndex, { align: "center" });
      }
      return;
    }
    if (localCommand === "clear" || localCommand === "new") {
      if (clientCommandRunning) return;
      const previousDraft = draft;
      setClientCommandRunning(true);
      setError(null);
      updateDraft("");
      setCaret(0);
      setDismissed(null);
      void useTermStore
        .getState()
        .clearChatSession(session.id, model, effort || undefined)
        .catch((err) => {
          setError(String(err));
          updateDraft(previousDraft);
          setCaret(previousDraft.length);
          placeCaret.current = previousDraft.length;
        })
        .finally(() => setClientCommandRunning(false));
      return;
    }
    if (submissionReceipts !== true) {
      if (submissionReceipts === false) setError(t("chat.submission.updateRequired"));
      return;
    }
    const images = attachments.map(({ mimeType, data }) => ({ mimeType, data }));
    const item = createSubmission(session.id, text, images, behavior);
    setSending(true);
    setError(null);
    updateDraft("");
    setCaret(0);
    setDismissed(null);
    setAttachments([]);
    setAttachNote(null);
    toEnd();
    void deliverSubmission(session.id, item, engineRunning ? undefined : startAgent)
      .finally(() => setSending(false));
  };

  /**
   * Take pasted or dropped images into the composer, saying what could not be taken.
   *
   * Files that do not fit are left out rather than failing the whole paste, and the note explains which —
   * a screenshot silently disappearing is worse than one that says why it did.
   */
  const addFiles = async (files: File[]) => {
    const { attachments: next, rejected } = await attachImages(attachments, files);
    setAttachments(next);
    const first = rejected[0];
    setAttachNote(
      !first
        ? null
        : first.reason === "tooMany"
          ? t("chat.attach.tooMany", MAX_IMAGES)
          : first.reason === "tooLarge"
            ? t("chat.attach.tooLarge", first.name, MAX_IMAGE_BYTES / (1024 * 1024))
            : t("chat.attach.unreadable", first.name),
    );
  };

  const dropAttachment = (id: string) => {
    setAttachments((prev) => prev.filter((a) => a.id !== id));
    setAttachNote(null);
  };

  /**
   * Images from a paste, including the ones WKWebView leaves out of the event.
   *
   * A Tauri window often reports an image paste with an empty clipboard payload; the terminal hits the
   * same wall and answers it the same way, by reading the native clipboard instead. Anything that is not
   * an image is left alone so it pastes as text.
   */
  const onPaste = (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
    const files = imagesFromClipboard(e.clipboardData);
    if (files.length > 0) {
      e.preventDefault();
      void addFiles(files);
      return;
    }
    if (!env.isTauri || e.clipboardData?.getData("text/plain")) return;
    e.preventDefault();
    void imageFromNativeClipboard()
      .then((file) => addFiles([file]))
      // Nothing to say: an empty clipboard is not an error, it is a keystroke that did nothing.
      .catch(() => {});
  };

  const steerQueued = async (id: string) => {
    if (steeringQueueRef.current) return;
    steeringQueueRef.current = true;
    setSteeringQueue(true);
    setError(null);
    try {
      await chatQueueSteer(session.id, id);
      setQueue((items) => items.filter((item) => item.id !== id));
    } catch (err) {
      setError(String(err));
    } finally {
      steeringQueueRef.current = false;
      setSteeringQueue(false);
    }
  };

  const removeQueued = (id: string) => {
    if (editing?.id === id) setEditing(null);
    // Drop it here as well as on the backend: the event confirming it costs a round trip, and the row has
    // to stop looking clickable the moment it is dismissed.
    setQueue((prev) => prev.filter((item) => item.id !== id));
    void chatQueueRemove(session.id, id).catch((err) => { setError(String(err)); void refreshRef.current(); });
  };

  const commitEdit = () => {
    if (!editing) return;
    const { id, text } = editing;
    setEditing(null);
    const current = queueRef.current.find(item => item.id === id);
    if (!current || current.shellCommand !== undefined) return;
    const trimmed = text.trim();
    // Emptying a queued message is how it is thrown away.
    if (!trimmed) {
      removeQueued(id);
      return;
    }
    setQueue((prev) => prev.map((item) => (item.id === id ? { ...item, text: trimmed } : item)));
    void chatQueueUpdate(session.id, id, trimmed).catch((err) => { setError(String(err)); void refreshRef.current(); });
  };

  const applyMode = async (next: Mode) => {
    await chatSetMode(session.id, next);
    setMode(next);
  };
  const pickMode = (next: Mode, keep: boolean) => {
    keepRestartPermission.current = false;
    void applyMode(next).then(async () => {
      if (keep) await savePermissionDefault(session.kind, next);
    }).catch((err) => {
      if (session.kind === "claude" && next === "bypassPermissions" && permissionRestart.handleError(err)) {
        keepRestartPermission.current = keep;
        setError(null);
      } else setError(String(err));
    });
  };

  const pickCollaborationMode = (next: CollaborationMode) => {
    setCollaborationMode(next);
    return chatSetCollaborationMode(session.id, next).catch((err) => setError(String(err)));
  };

  const answer = (request: ChatPermission, reply: PermissionAnswer) => {
    void (async () => {
      try {
        // The mode goes first: approving a plan is what ends plan mode, and the agent has to be out of it
        // before the tool it was asking about runs. It is also how a mode the answer adopts reaches the
        // chip and the stored session setting, which the protocol's own `updatedPermissions` never sees.
        if (reply.mode) await applyMode(reply.mode);
        await chatPermission(session.id, request.id, reply.allow, {
          updatedInput: reply.updatedInput,
          message: reply.message,
          updatedPermissions: reply.updatedPermissions,
        });
        setPermissions((prev) => prev.filter((p) => p.id !== request.id));
      } catch (err) {
        setError(String(err));
      }
    })();
  };

  const pickModel = (value: string, keep: boolean) => {
    const next = value || undefined;
    void (async () => {
      // A rejected choice must neither replace the confirmed model nor become a saved default.
      await chatSetModel(session.id, next);
      setModel(next);
      // Each model has its own remembered effort, and its own ladder: carry over what was picked for this
      // one, and drop a level the new model does not offer rather than leaving it showing something the
      // agent would refuse.
      const st = useTermStore.getState();
      const entry =
        st.chatEffortByModel[effortKey(session.kind, value)] ??
        (session.kind === "claude" ? st.chatEffortByModel[value] : "") ??
        "";
      const ladder = catalogue.find((m) => m.id === value)?.effortLevels ?? (EFFORTS as readonly string[]);
      const carried: EffortChoice = isEffort(entry) && levelsOf(ladder, session.kind).includes(entry) ? entry : "";
      if (carried !== effort) {
        await chatSetEffort(session.id, carried || undefined);
        setEffort(carried);
      }
      // What a new conversation starts with is the pair, never the model alone: a model remembered without
      // its level would open on someone else's level.
      if (keep) rememberPair(value, carried);
    })().catch((err) => setError(String(err)));
  };

  const pickEffort = (next: EffortChoice, keep: boolean) => {
    // An empty choice is the agent's own `auto`: it drops the override rather than naming a level, which
    // is also why nothing is remembered for it and no `--effort` reaches the next launch.
    void chatSetEffort(session.id, next || undefined).then(() => {
      setEffort(next);
      if (keep) rememberPair(model ?? "", next);
    }).catch((err) => setError(String(err)));
  };

  /** Switch fast mode: on the running agent now, and as the default for the next conversation. */
  const pickFastMode = (enabled: boolean) => {
    void (async () => {
      if (engineRunning) await chatSetFastMode(session.id, enabled);
      setExtras((prev) => ({ ...prev, fastMode: enabled }));
      useTermStore.getState().setChatFastMode(session.kind, enabled);
    })().catch((err) => setError(String(err)));
  };
  // The switch is offered only where the catalogue says it does something: for the selected model, or
  // for any model while the agent's own default is selected.
  const fastModeOffered =
    session.kind === "claude" &&
    (model ? catalogue.some((m) => m.id === model && m.supportsFastMode) : catalogue.some((m) => m.supportsFastMode));

  /** Save this conversation's choice before reflecting it in the controls. */
  const pickServiceTier = (value: string) => {
    void chatSetServiceTier(session.id, value || undefined)
      .then(() => setServiceTier(value))
      .catch((err) => setError(String(err)));
  };
  const pickPersonality = (value: string) => {
    void chatSetPersonality(session.id, value || undefined)
      .then(() => setPersonality(value))
      .catch((err) => setError(String(err)));
  };
  // Offered only where the catalogue says the model has a faster tier, or honours a personality.
  const codexModel = session.kind === "codex" ? catalogue.find((m) => m.id === model) : undefined;
  const serviceTierOptions: ChipOption<string>[] =
    codexModel?.serviceTiers?.length
      ? [
          { value: CODEX_STANDARD_TIER, label: t("chat.serviceTier.default") },
          ...codexModel.serviceTiers.map((tier) => ({
            value: tier.id,
            label: tier.label,
            hint: tier.description || undefined,
          })),
        ]
      : [];
  const personalityOffered = codexModel?.supportsPersonality === true;
  const personalityOptions: ChipOption<string>[] = [
    { value: "", label: t("chat.personality.default") },
    ...CODEX_PERSONALITIES.map((value) => ({
      value,
      label: t(`chat.personality.${value}` as "chat.personality.none"),
    })),
  ];

  /** Save the pair a new conversation should open on: a model, and the level chosen for that model. */
  const rememberPair = (modelId: string, level: string) => {
    const st = useTermStore.getState();
    st.setChatModel(session.kind, modelId);
    st.setChatEffort(effortKey(session.kind, modelId), level);
  };

  // Only the directory the caret is writing into gets read, and only while a mention is being written.
  const mentionAt = useMemo(() => findFileMention(draft, caret), [draft, caret]);
  const files = useMentionFiles(cwd, mentionAt ? mentionDir(mentionAt.query) : null);

  const wantsCatalogue = /^[/\$][^\s]*$/.test(draft) || /(?:^|\s)\$[^\s$]*$/.test(draft.slice(0, caret));
  useEffect(() => {
    if (!wantsCatalogue || hidden || !["codex", "claude"].includes(session.kind)) return;
    let disposed = false;
    setCatalogueLoading(true);
    void chatCommands(session.id).then((catalogue) => {
      if (!Array.isArray(catalogue)) throw new Error("The server returned an invalid command catalogue");
      if (!disposed) {
        setCommands(catalogue);
        setCatalogueError(null);
      }
    }).catch((err) => {
      if (!disposed) {
        setCommands([]);
        setCatalogueError(String(err));
      }
    }).finally(() => { if (!disposed) setCatalogueLoading(false); });
    return () => { disposed = true; };
  }, [wantsCatalogue, hidden, session.id, session.kind, engineRunning]);

  const completionCommands = useMemo<ChatCommand[]>(() => {
    const local: ChatCommand[] = [
      { name: "clear", description: t("chat.command.clearDescription") },
    ];
    // What Codex's own interface offers as commands and this pane asks for through requests of its own.
    if (session.kind === "codex") {
      local.push(
        { name: "compact", description: t("chat.command.compactDescription") },
        {
          name: "review",
          description: t("chat.command.reviewDescription"),
          argumentHint: t("chat.command.reviewHint"),
        },
      );
    }
    // Pi and OMP expose compaction as a protocol command of their own.
    if (session.kind === "pi" || session.kind === "omp") {
      local.push({ name: "compact", description: t("chat.command.compactDescription") });
    }
    // What OpenCode's own interface answers without a turn, handled by the backend when sent.
    if (session.kind === "opencode") {
      local.push(
        { name: "compact", description: t("chat.command.compactDescription") },
        { name: "undo", description: t("chat.command.undoDescription") },
        { name: "redo", description: t("chat.command.redoDescription") },
        { name: "share", description: t("chat.command.shareDescription") },
        { name: "unshare", description: t("chat.command.unshareDescription") },
      );
    }
    const localNames = new Set(local.map((command) => command.name).concat("new"));
    return [...local, ...commands.filter((command) => command.invocation || !localNames.has(command.name))];
  }, [commands, session.kind, t]);

  const completion = useMemo(
    () =>
      draft === dismissed
        ? null
        : buildSuggestions({ draft, caret, commands: completionCommands, configKeys, files }),
    [draft, caret, completionCommands, configKeys, files, dismissed],
  );

  const complete = (suggestion: Suggestion) => {
    const at = suggestion.caret ?? suggestion.insert.length;
    updateDraft(suggestion.insert);
    setCaret(at);
    placeCaret.current = at;
    inputRef.current?.focus();
  };

  // The chosen text reaches the textarea on the next render, so the caret can only be moved after it.
  useEffect(() => {
    const at = placeCaret.current;
    if (at === null) return;
    placeCaret.current = null;
    inputRef.current?.setSelectionRange(at, at);
  }, [draft]);

  /** Follow the caret through arrow keys, clicks, and selections, which do not change the draft. */
  const syncCaret = (e: React.SyntheticEvent<HTMLTextAreaElement>) => {
    const el = e.currentTarget;
    setCaret(el.selectionStart ?? el.value.length);
  };

  // Written out rather than taken from the shortcut registry: this combo belongs to the textarea, not to
  // a rebindable global action, so there is no binding to read.
  const interruptCombo = IS_MAC && !IS_PLAIN_BROWSER ? "\u2318\u21A9" : "Ctrl+Enter";
  const steerCombo = IS_MAC ? "\u2325\u21A9" : "Alt+Enter";
  /** What a keypress in the composer asks for: interrupt with the primary modifier, steer with Alt.
   *  With no turn running there is nothing to steer or interrupt, so the modifiers fall back to a
   *  normal send instead of an error. */
  const behaviorOf = (e: React.KeyboardEvent): SendBehavior =>
    !busy ? "queue" : e.metaKey || e.ctrlKey ? "interrupt" : e.altKey ? "steer" : "queue";

  const label = assistantLabel(session.kind);
  // The agent's own mark, drawn in the margin beside each of its answers.
  const kindIcon = useMemo(() => kindIconEl(session.kind, 12), [session.kind]);

  const modelOptions: ChipOption<string>[] = [
    // The default names no model, so it carries the settings mark, in the accent colour, rather than the
    // agent's.
    {
      value: "",
      label: t("chat.followModelDefault", label),
      hint: t("chat.followModelDefaultHint"),
      glyph: (
        <span style={{ color: "var(--accent)", display: "inline-flex" }}>
          <Icons.sliders size={14} />
        </span>
      ),
    },
    // The catalogue's description opens with the model's own name, because it is also used where the name
    // is not already on screen. Here it is, one line above, so the menu drops the repeat.
    // Every model row carries the agent's own mark in its brand colour, the same mark the chip shows.
    ...catalogue.toSorted((a, b) => Number(a.largeContext) - Number(b.largeContext)).map((m) => ({
      value: m.id,
      label: m.label,
      hint: withoutNamePrefix(m.description, m.label),
      glyph: kindIconEl(session.kind, 14),
    })),
  ];
  // The chip reads the selected model's own name, but a model set outside this menu — inherited from the
  // session's launch arguments, or reported by the agent — may not be in the list, so fall back to the id.
  const selected = catalogue.find((m) => m.id === model);
  const modelLabel = selected?.label ?? model ?? t("chat.followModelDefault", label);
  // Offering a level the selected model rejects would let someone set it and see it silently ignored, so
  // the list narrows to the ladder reported for that model.
  const allowed = levelsOf(
    selected ? selected.effortLevels : (EFFORTS as readonly string[]),
    session.kind,
  );
  const defaultEffort = isEffort(rememberedDefaultEffort) && allowed.includes(rememberedDefaultEffort)
    ? rememberedDefaultEffort : "";
  // The mark counts from the full ladder, not from the filtered list: a model that stops at high should
  // still show high at its stable place, not as the top of its own short scale. Auto gets an empty bar
  // because it names no level, and ultracode a full one because it runs at the top of the ladder.
  const effortLabel = (value: string) =>
    (EFFORTS as readonly string[]).includes(value) || value === THINKING_OFF || value === ULTRACODE
      ? t(`chat.effort.${value}` as "chat.effort.high")
      : value;

  const effortOptions: ChipOption<EffortChoice>[] = [
    // Below auto for Claude: no thinking at all, marked by an empty bar with nothing lit.
    ...(session.kind === "claude"
      ? [
          {
            value: THINKING_OFF,
            label: t("chat.effort.off"),
            hint: t("chat.effort.offHint"),
            glyph: (
              <span style={{ color: "var(--text-faint)", display: "inline-flex" }}>
                <Icons.minus size={14} />
              </span>
            ),
          },
        ]
      : []),
    {
      value: "",
      label: t("chat.effort.auto"),
      glyph: <LevelBar level={0} of={EFFORTS.length} />,
    },
    ...EFFORTS.filter((value) => allowed.includes(value)).map((value) => ({
      value,
      label: effortLabel(value),
      glyph: <LevelBar level={EFFORTS.indexOf(value) + 1} of={EFFORTS.length} />,
    })),
    ...(allowed.includes(ULTRACODE)
      ? [
          {
            value: ULTRACODE,
            label: t("chat.effort.ultracode"),
            glyph: <LevelBar level={EFFORTS.length} of={EFFORTS.length} />,
          },
        ]
      : []),
  ];
  // The permission chip shows the mode that was chosen; whether that choice is already the running
  // mode is carried by the chip's mark and by the menu's row notes. Only Codex defers the change to
  // the next turn, so the two facts differ during that window and nothing else has to say so in words.
  const permissionValue = permissionState?.value;
  const permissionPending = permissionValue?.activation === "nextTurn" || permissionValue?.activation === "restart";
  const pendingMode = permissionPending ? permissionValue?.pending : null;
  const appliedMode = permissionValue?.running ? permissionValue?.current : null;
  const modeLabel = isMode(mode) ? t(modeLabelKeyFor(session.kind, mode)) : mode;
  const modeOptions: ChipOption<string>[] = (permissionCatalog?.catalog?.modes ?? []).map((value) => ({
    value,
    label: t(modeLabelKeyFor(session.kind, value)),
    // The effective row is noted only while it differs from the target being waited on; on its own,
    // "Applied" says nothing the chip has not already said.
    tag: pendingMode && value === pendingMode
      ? t(permissionValue?.activation === "restart" ? "permission.restart" : "chat.modeNextTurn")
      : pendingMode && appliedMode === value && appliedMode !== pendingMode
        ? t("permission.applied")
        : undefined,
  }));
  // Pi runs every tool without asking, so it exposes no permission control and nothing to report about
  // one. A caption for a setting this agent does not have reads as a fact about it that is not true.
  const hasPermissionControl = supportsPermissionToggle(session.kind);
  const collaborationModeOptions: ChipOption<CollaborationMode>[] = collaborationModes.map(
    (preset) => ({
      value: preset.mode,
      label: collaborationLabel(session.kind, preset, preset.mode, t),
      hint:
        session.kind === "codex" && (preset.mode === "default" || preset.mode === "plan")
          ? t(`chat.collaborationMode.${preset.mode}Hint` as "chat.collaborationMode.defaultHint")
          : preset.description,
    }),
  );
  const selectedCollaboration = collaborationModes.find((preset) => preset.mode === collaborationMode);

  // A send that failed because the agent executable is missing becomes an actionable notice rather than
  // a raw spawn error. Only the newest failure gets the notice so repeated sends do not stack cards, and
  // closing it keeps it closed until the next failure.
  const agentMissingItem = [...pendingSubmissions]
    .reverse()
    .find((item) => item.status === "failed" && isAgentNotInstalledError(item.error));
  const agentMissingKey =
    agentMissingItem?.id ?? (isAgentNotInstalledError(error) ? "session-error" : null);
  const showAgentNotice = agentMissingKey !== null && agentNoticeDismissed !== agentMissingKey;

  const submissionFeedback = (id: string) => {
    const item = pendingSubmissions.find(value => value.id === id);
    if (!item) return null;
    return <div className="sv-submission-status" role="status">
      {t(`chat.submission.${item.status}`)}
      {(item.status === "failed" || item.status === "unknown") && <button disabled={submissionReceipts !== true} onClick={() => void retrySubmission(session.id, item, item.status === "failed" && !engineRunning ? startAgent : undefined)}>
        {t(item.status === "unknown" ? "chat.submission.check" : "common.retry")}
      </button>}
      {item.error && !isAgentNotInstalledError(item.error) && <ErrorRow message={item.error} />}
    </div>;
  };

  // Every chip that exists for this session, in the toolbar's traditional order. The toolbar applies the
  // user's inline preference on top and never renders a chip the pane did not offer.
  const composerChips: ComposerChip[] = [];
  if (catalogue.length > 0) composerChips.push({ id: "model", node: (
    <ControlChip
      glyph={kindIconEl(session.kind, 14)}
      label={modelLabel}
      title={t("chat.modelTooltip")}
      value={model ?? ""}
      options={modelOptions}
      defaultValue={defaultModel || undefined}
      defaultLabel={t("chat.savedModelDefault")}
      advancedFooter={session.kind === "claude" ? <ModelCatalogStatus onChanged={() => setCatalogueVersion(v => v + 1)} /> : undefined}
      onPick={pickModel}
      keepLabel={t("chat.keepChoice")}
      onKeepCurrent={() => rememberPair(model ?? "", effort)}
      menuWidth={300}
      filterPlaceholder={t("chat.filterPlaceholder")}
    />
  ) });
  composerChips.push({ id: "effort", node: (
    <ControlChip
      glyph={<Icons.cpu size={14} />}
      label={effort ? effortLabel(effort) : t("chat.effortDefault")}
      title={t("chat.effortTooltip")}
      value={effort}
      options={effortOptions}
      defaultValue={defaultEffort}
      onPick={pickEffort}
      // A level belongs to a model, so the box says which one it would become the default for.
      keepLabel={selected ? t("chat.keepChoiceFor", selected.label) : t("chat.keepChoice")}
      menuWidth={230}
    />
  ) });
  if (collaborationModeOptions.length > 0) composerChips.push({ id: "collaboration", node: (
    <ControlChip
      glyph={session.kind === "opencode" ? <Icons.bot size={14} /> : <Icons.compass size={14} />}
      label={collaborationLabel(session.kind, selectedCollaboration, collaborationMode, t)}
      title={t(session.kind === "opencode" ? "chat.agentTooltip" : "chat.collaborationModeTooltip")}
      value={collaborationMode}
      options={collaborationModeOptions}
      onPick={pickCollaborationMode}
      menuWidth={260}
    />
  ) });
  if (hasPermissionControl) composerChips.push({ id: "permission", node: (
    <div className="sv-permission-control">
      <ControlChip
        glyph={permissionPending
          ? <Icons.clock size={14} style={{ color: "var(--accent)" }} />
          : <Icons.lock size={14} />}
        label={modeLabel}
        title={permissionValue?.activation === "nextTurn"
          ? `${t("chat.modeTooltip")} · ${t("chat.modePendingHint", currentPermissionLabel(permissionValue), modeLabel)}`
          : permissionValue?.activation === "restart"
            ? `${t("chat.modeTooltip")} · ${t("permission.restart")}`
            : permissionValue?.activation === "applied"
              ? `${t("chat.modeTooltip")} · ${t("permission.applied")}`
              : t("chat.modeTooltip")}
        value={mode}
        options={modeOptions}
        disabled={!permissionCatalog?.catalog}
        defaultValue={defaultMode}
        onPick={(next, keep) => { if (isMode(next)) pickMode(next, keep); }}
        keepLabel={t("chat.keepChoice")}
        menuWidth={240}
      />
    </div>
  ) });
  if (fastModeOffered) composerChips.push({ id: "fastMode", node: (
    <FastModeChip enabled={extras.fastMode === true} onToggle={pickFastMode} />
  ) });
  if (serviceTierOptions.length > 1) composerChips.push({ id: "serviceTier", node: (
    <ControlChip
      glyph={<Icons.clock size={14} />}
      label={serviceTierOptions.find((o) => o.value === serviceTier)?.label ?? t("chat.serviceTier.default")}
      title={t("chat.serviceTierTooltip")}
      value={serviceTier || CODEX_STANDARD_TIER}
      options={serviceTierOptions}
      onPick={pickServiceTier}
      menuWidth={240}
    />
  ) });
  if (personalityOffered) composerChips.push({ id: "personality", node: (
    <ControlChip
      glyph={<Icons.bot size={14} />}
      label={
        personality
          ? t(`chat.personality.${personality}` as "chat.personality.none")
          : t("chat.personality.default")
      }
      title={t("chat.personalityTooltip")}
      value={personality}
      options={personalityOptions}
      onPick={pickPersonality}
      menuWidth={220}
    />
  ) });
  // A chip the user turned on is offered whenever it exists for this agent kind. Between turns the agent
  // process is away, so MCP and Tasks are disabled rather than dropped: a chip that comes and goes with
  // the process cannot be found by someone who just switched it on.
  if (session.kind === "claude" || session.kind === "codex") composerChips.push({ id: "mcp", node: (
    <McpChip sessionId={session.id} codex={session.kind === "codex"} disabled={!engineRunning} />
  ) });
  if (session.kind === "claude") composerChips.push({ id: "tasks", node: (
    <TasksChip
      sessionId={session.id}
      tasks={extras.backgroundTasks ?? []}
      busy={busy}
      disabled={!engineRunning}
      onStop={(taskId) => void chatStopTask(session.id, taskId).catch((err) => setError(String(err)))}
      onOpen={(task) => openTaskTab(session.id, task)}
      onBackgroundAll={() => void chatBackgroundTasks(session.id).catch((err) => setError(String(err)))}
    />
  ) });
  // The account menu disables itself while a sign-in is unresolved; the inline panel handles that flow.
  if (session.kind === "codex" || session.kind === "claude") composerChips.push({ id: "account", node: (
    <AgentAccountMenu provider={session.kind === "claude" ? "Claude" : "Codex"} sessionId={session.id} state={extras.auth} busy={turnStartedAt !== undefined} />
  ) });
  if (session.kind === "codex") composerChips.push({ id: "codexCredits", node: <CodexResetCredits /> });

  // What is typed decides the button, not what the agent is doing: with text in the box the action is
  // always "send" — queued while a turn runs — and only an empty box during a turn turns it into the
  // stop button. Mobile keeps it beside the input, outside the collapsible options.
  const sendButton = busy && !clientCommandRunning && !draft.trim() && attachments.length === 0 ? (
    <button
      className="sv-send sv-stop"
      onClick={stop}
      title={t("chat.interruptTooltip")}
    >
      <span className="sv-stop-square" />
    </button>
  ) : (
    <button
      className="sv-send"
      disabled={sending || clientCommandRunning || (!draft.trim() && attachments.length === 0)}
      onPointerDown={mobile ? (event) => event.preventDefault() : undefined}
      onClick={() => send()}
      title={
        busy
          ? `${t("chat.queueTooltip", interruptCombo)} · ${t("chat.steerTooltip", steerCombo)}`
          : t("session.send")
      }
    >
      <Icons.arrowRight size={mobile ? 20 : 14} />
    </button>
  );

  return (
    <SessionLinkDirectory.Provider value={cwd}>
    <div
      className="term-mount"
      ref={attachChatInputGuard}
      onMouseDown={() => {
        if (paneId) onActivate(paneId, session.id);
      }}
      // Escape stops the running turn, the way it does in the agent's own terminal interface. The two
      // places that give Escape a local meaning — the completion list and a queued message being
      // rewritten — stop the event before it reaches here. An open control menu is the one thing this
      // cannot see from a React handler, because the menu closes itself on a document listener that runs
      // after this one, so look for its rows: while a menu is open, Escape belongs to the menu.
      onKeyDown={(e) => {
        if (e.key !== "Escape" || !busy) return;
        if ([...e.currentTarget.querySelectorAll("[role=option], .sv-popover")].some(menu => !menu.closest("[hidden]"))) return;
        e.preventDefault();
        stop();
      }}
      style={{
        position: "absolute",
        ...area,
        display: hidden ? "none" : "block",
        padding: paneStyle === "card" ? "calc(var(--pane-gap) / 2)" : 0,
      }}
    >
      <div
        className={"pane" + (focused ? " focus" : "")}
        style={{ width: "100%", height: "100%", contain: "layout" }}
      >
        <div className="pane-head">
          <span style={{ color: "var(--text-secondary)", display: "grid", flex: "none" }}>
            {kindIconEl(session.kind, 13)}
          </span>
          <span className="pt">{session.name}</span>
          <StatusIndicator status={status} unread={unread} />
          <span className="pane-tools">
            <button title={t("term.searchMenu")} onClick={() => useTermStore.getState().openSearch()}>
              <Icons.search size={14} />
            </button>
            <button
              title={t(allTurnsCollapsed ? "chat.turnFold.showAll" : "chat.turnFold.hideAll")}
              aria-label={t(allTurnsCollapsed ? "chat.turnFold.showAll" : "chat.turnFold.hideAll")}
              aria-pressed={allTurnsCollapsed}
              disabled={folded.turns.length === 0}
              onClick={toggleAllTurns}
            >
              {allTurnsCollapsed ? <Icons.expand size={14} /> : <Icons.collapse size={14} />}
            </button>
            {/* The same control the terminal view carries, in the same place, pointing the other way. */}
            <ConversationViewHint enabled={!mobile && !isShareSurface && focused && !hidden} onSwitch={() => switchTo("tui")} />
            <button title={t("term.splitRight")} onClick={() => paneId && onSplit(paneId, session.id, "horizontal")}>
              <Icons.splitV size={14} />
            </button>
            <button title={t("term.splitDown")} onClick={() => paneId && onSplit(paneId, session.id, "vertical")}>
              <Icons.splitH size={14} />
            </button>
            <button
              title={t("term.closePane")}
              disabled={!multi}
              onClick={() => multi && paneId && onClose(paneId, session.id)}
            >
              <Icons.close size={14} />
            </button>
          </span>
        </div>

        <div className="sv" style={{ position: "relative", flex: 1, minHeight: 0 }}>
          {searchOpen && focused && !hidden && <ChatSearch key={session.id} entries={turnEntries} scrollRef={scrollRef} onLocate={locateSearch} onClose={closeSearch}
            loadingHistory={hasMore && !historyError} historyError={historyError} onRetryHistory={() => void loadHistory()} />}
          <div className="sv-scroll-wrap">
            <div
              className="sv-scroll"
              ref={scrollRef}
              onScroll={(e) => {
                const el = e.currentTarget;
                // An event that arrives after the pane was hidden reads a scroller with no layout: offset,
                // height and viewport are all 0, which would look like a reader at the end.
                if (el.clientHeight === 0) return;
                readerTop.current = el.scrollTop;
                // Browser anchoring and viewport resizing also emit scroll events. A layout
                // change must not turn off following before the resize observer catches up.
                const geometry = scrollGeometry.current;
                if (pinnedRef.current && (geometry.height !== el.scrollHeight || geometry.viewport !== el.clientHeight)) {
                  el.scrollTop = el.scrollHeight;
                  scrollGeometry.current = { height: el.scrollHeight, viewport: el.clientHeight };
                  return;
                }
                if (el.scrollTop < 80 && !historyError) void loadHistory();
                const pinned = el.scrollHeight - el.scrollTop - el.clientHeight <= PIN_SLACK;
                pinnedRef.current = pinned;
                setAway((prev) => (prev === !pinned ? prev : !pinned));
              }}
            >
              {syncState !== "ready" && <div className="sv-sync-state" role="status">
                {t(syncState === "loading" ? "chat.sync.loading" : "chat.sync.failed")}
                {syncState === "failed" && <button onClick={() => void refreshRef.current()}>{t("common.retry")}</button>}
              </div>}
              {hasMore && <div className="sv-history" aria-busy={historyLoading}>
                <button className="vlx-btn sv-history-more" disabled={historyLoading || syncState !== "ready"} onClick={() => void loadHistory()}>
                  {t(historyLoading ? "common.loading" : historyError ? "common.retry" : "chat.sync.history")}
                </button>
                {historyError && <div className="sv-history-error" role="alert">{historyError}</div>}
              </div>}
              {/* An empty conversation is what a new session opens on, so it gets the middle of the view:
                  the agent's own mark, drawn large enough to say which agent this is, and one line. */}
              {syncState === "ready" && rows.length === 0 && pendingSubmissions.length === 0 && !busy && (
                <div className="sv-blank">
                  <span className="sv-blank-icon">{kindIconEl(session.kind, 40)}</span>
                  <div className="sv-blank-line">{t("chat.empty")}</div>
                </div>
              )}
              {/* The older part of a long conversation. Only the rows near the viewport are drawn; the
                  container is as tall as all of them would be, so the scrollbar tells the truth. */}
              {virtualRows.length > 0 && (
                <div className="sv-virtual" style={{ height: virtualizer.getTotalSize() }}>
                  {virtualizer.getVirtualItems().map((item) => {
                    const entry = virtualRows[item.index];
                    if (!entry) return null;
                    return (
                      <div
                        key={item.key}
                        className={"sv-item sv-item-virtual" + (searchOpen && focused && searchTarget === entry.id ? " sv-search-match" : "")}
                        data-search-id={entry.id}
                        data-index={item.index}
                        ref={virtualizer.measureElement}
                        style={{ transform: `translateY(${item.start}px)` }}
                      >
                        <Entry
                          entry={entry}
                          label={label}
                          icon={kindIcon}
                          cwd={cwd}
                          openRuns={openRuns}
                          onToggleRun={toggleRun}
                          onToggleFold={toggleTurn}
                          onRewind={askRewind}
                    rewindDisabledReason={rewindDisabledReason}
                          rewindScopes={rewindScopes}
                          rewindRequest={rewindRequest}
                          onRewindRequestHandled={finishRewindRequest}
                          onCancelShell={readOnly ? undefined : cancelShell}
                        />
                        {submissionFeedback(entry.id)}
                      </div>
                    );
                  })}
                </div>
              )}
              {mountedRows.map((entry) => (
                <div key={entry.id} data-search-id={entry.id} className={"sv-item" + (searchOpen && focused && searchTarget === entry.id ? " sv-search-match" : "")}>
                  <Entry
                    entry={entry}
                    label={label}
                    icon={kindIcon}
                    cwd={cwd}
                    openRuns={openRuns}
                    onToggleRun={toggleRun}
                    onToggleFold={toggleTurn}
                    onRewind={askRewind}
                    rewindDisabledReason={rewindDisabledReason}
                    rewindScopes={rewindScopes}
                    rewindRequest={rewindRequest}
                    onRewindRequestHandled={finishRewindRequest}
                    onCancelShell={readOnly ? undefined : cancelShell}
                  />
                  {submissionFeedback(entry.id)}
                </div>
              ))}
              {pendingSubmissions.filter(item => !rows.some(row => row.id === item.id) && !queue.some(queued => queued.id === item.id)).map(item => (
                <div className="sv-item sv-submission" key={item.id} data-submission-id={item.id}>
                  <Entry entry={{ kind: "row", id: item.id, row: { kind: "user", id: item.id, text: item.text, images: item.images } }}
                    label={label} icon={kindIcon} cwd={cwd} openRuns={openRuns} onToggleRun={toggleRun}
                    rewindScopes={[]} rewindRequest={null} onRewindRequestHandled={finishRewindRequest} />
                  {submissionFeedback(item.id)}
                </div>
              ))}
              {turnStartedAt !== undefined && <WorkingRow startedAt={turnStartedAt} />}
              {extras.apiRetry && <RetryLine retry={extras.apiRetry} />}
              {showAgentNotice && (
                <AgentMissingNotice
                  key={agentMissingKey}
                  session={session}
                  onInstall={installAgent}
                  onDismiss={() => setAgentNoticeDismissed(agentMissingKey)}
                />
              )}
              {/* Wrapped like every other row: a bare child of the scroller would lose the centered column,
                  because the row's own left indent replaces the auto margins that center it. */}
              {error && !isAgentNotInstalledError(error) && <div className="sv-item"><ErrorRow message={error} /></div>}
              {catalogueError && <div className="sv-item"><ErrorRow message={catalogueError} /></div>}
              {(session.kind === "codex" || session.kind === "claude") && !readOnly && <AgentAuth provider={session.kind === "claude" ? "Claude" : "Codex"} sessionId={session.id} state={extras.auth} busy={turnStartedAt !== undefined} />}
            </div>
            {away && (
              <button className="sv-to-end" onClick={toEnd} title={t("chat.backToEnd")}>
                <Icons.chevD size={14} />
              </button>
            )}
          </div>

          {/* Both notices stand between the conversation and the composer, on the same centred column as
              both, the way the permission lane does. */}
          {((autoContinue && !readOnly) || notice) && (
            <div className="sv-notices">
              {autoContinue && !readOnly && (
                <AutoContinueBar
                  waiting={autoContinue}
                  onCancel={() => void chatAutoContinueCancel(session.id).then(() => setAutoContinue(null), (err) => setError(String(err)))}
                />
              )}
              {notice && (
                <NotificationBar text={notice.text} priority={notice.priority} onClose={() => setNotice(null)} />
              )}
            </div>
          )}
          {permissions.length > 0 && (
            <div className="sv-permissions">
              {permissions.map((request) => (
                <PermissionCard key={request.id} request={request} cwd={cwd} onAnswer={answer} />
              ))}
            </div>
          )}

          {pendingRewind ? (
            <div className="sv-rewind-confirm-wrap">
              <div className="sv-rewind-confirm">
                <div className="sv-rewind-confirm-head">
                  <Icons.restart size={13} />
                  <strong>
                    {t(`chat.rewind.confirm.${pendingRewind.scope}` as "chat.rewind.confirm.conversation")}
                  </strong>
                </div>
                {rewindTarget?.kind === "user" ? <blockquote className="sv-rewind-target">
                  {rewindTarget.at ? <time>{new Date(rewindTarget.at).toLocaleString()}</time> : null}
                  <div>{rewindTarget.text}</div>
                  {rewindTarget.images?.map((image, index) => <ChatImageView key={index} image={image} className="sv-msg-image" alt="" />)}
                </blockquote> : null}
                {pendingRewind.replacement !== undefined ? <>
                  <p>{t("chat.rewind.editWarning")}</p>
                  <strong>{t("chat.rewind.editSend")}</strong>
                  <blockquote className="sv-rewind-target">
                    <div>{pendingRewind.replacement.text}</div>
                    {pendingRewind.replacement.images.length > 0 ? <div className="sv-msg-images">
                      {pendingRewind.replacement.images.map((image, index) => <ChatImageView key={index} image={image} className="sv-msg-image" alt="" />)}
                    </div> : null}
                  </blockquote>
                </> : null}
                <div className="sv-rewind-confirm-detail">
                  {pendingRewind.loading
                    ? t("chat.rewind.previewing")
                    : pendingRewind.preview?.canRewind === false
                      ? pendingRewind.preview.error || t("chat.rewind.unavailable")
                      : pendingRewind.scope === "conversation" || !pendingRewind.preview?.filesChanged
                        ? t("chat.rewind.warning")
                        : t(
                            "chat.rewind.fileSummary",
                            pendingRewind.preview.filesChanged.length,
                            pendingRewind.preview.insertions ?? 0,
                            pendingRewind.preview.deletions ?? 0,
                          )}
                </div>
                {pendingRewind.preview?.filesChanged?.length ? (
                  <pre className="sv-rewind-files">{pendingRewind.preview.filesChanged.join("\n")}</pre>
                ) : null}
                <div className="sv-rewind-actions">
                  <button className="sv-deny" disabled={rewinding} onClick={() => setPendingRewind(null)}>
                    {t("chat.rewind.cancel")}
                  </button>
                  <button
                    className="sv-rewind-apply"
                    disabled={pendingRewind.loading || !canRewind || pendingRewind.preview?.canRewind === false}
                    onClick={doRewind}
                  >
                    {rewinding ? t("chat.rewind.applying") : t(pendingRewind.replacement !== undefined ? "chat.rewind.editConfirm" : "chat.rewind.apply")}
                  </button>
                </div>
              </div>
            </div>
          ) : null}

          {/* The whole composer is the drop target, not just the text box: aiming at a one-line input to
              attach a picture is a needlessly small target. */}
          {!readOnly && <div
            className="sv-composer"
            ref={composerOptions.ref}
            data-options-expanded={mobile ? composerOptions.expanded : undefined}
            onDragOver={(e) => {
              if (e.dataTransfer?.types?.includes("Files")) e.preventDefault();
            }}
            onDrop={(e) => {
              const files = imagesFromDrop(e.dataTransfer);
              if (files.length === 0) return;
              e.preventDefault();
              void addFiles(files);
            }}
          >
            {(completion || (wantsCatalogue && catalogueLoading && draft !== dismissed)) && (
              <div className="sv-complete">
                {wantsCatalogue && catalogueLoading && draft !== dismissed && <div className="sv-complete-item" role="status">{t("common.loading")}</div>}
                {completion?.map((c) => (
                  <button key={c.id} className="sv-complete-item" onClick={() => complete(c)}>
                    <span className="sv-complete-name">{c.label}</span>
                    {c.hint ? <span className="sv-complete-hint">{c.hint}</span> : null}
                    {c.desc ? <span className="sv-complete-desc">{c.desc}</span> : null}
                  </button>
                ))}
              </div>
            )}
            {queue.length > 0 && (
              <div className="sv-queue">
                <div className="sv-queue-header">
                  <span>{t("chat.queue.pending")}</span>
                  <span className="sv-queue-count">({queue.length})</span>
                </div>
                <div className="sv-queue-list" role="region" aria-label={t("chat.queue.pending")} tabIndex={0}>
                  {queue.map((item) => (
                    <div key={item.id} className="sv-queue-item">
                      <div className="sv-queue-content">
                        {item.origin && <MessageSender origin={item.origin} />}
                        <div className="sv-queue-message">
                          {editing?.id === item.id && item.shellCommand === undefined ? (
                            <textarea
                              className="sv-queue-edit"
                              autoFocus
                              rows={1}
                              value={editing.text}
                              onChange={(e) => setEditing({ id: item.id, text: e.target.value })}
                              onBlur={commitEdit}
                              onKeyDown={(e) => {
                                if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                                  e.preventDefault();
                                  commitEdit();
                                }
                                if (e.key === "Escape") {
                                  e.preventDefault();
                                  e.stopPropagation();
                                  setEditing(null);
                                }
                              }}
                            />
                          ) : (
                            // A shell command waiting for the turn to end is shown as what was typed, not as
                            // the tagged context message it carries; the text itself is not for editing.
                            <QueuedMessageText
                              text={queuedShellCommand(item) ?? item.text}
                              origin={item.origin}
                              disabled={steeringQueue || queuedShellCommand(item) !== null}
                              onEdit={() => setEditing({ id: item.id, text: item.text })}
                            />
                          )}
                        </div>
                      </div>
                      {item.images && item.images.length > 0 && (
                        <span className="sv-queue-images">
                          {item.images.map((image, i) => (
                            <ChatImageView
                              key={"attachmentId" in image ? image.attachmentId : i}
                              image={image}
                              className="sv-queue-thumb"
                              alt=""
                            />
                          ))}
                        </span>
                      )}
                      <button
                        className="sv-queue-steer"
                        disabled={!busy || stopping.current || steeringQueue || editing !== null}
                        onClick={() => void steerQueued(item.id)}
                      >
                        {t("chat.steer")}
                      </button>
                      <button
                        className="sv-queue-drop"
                        disabled={steeringQueue}
                        title={t("chat.queue.remove")}
                        aria-label={t("chat.queue.remove")}
                        onClick={() => removeQueued(item.id)}
                      >
                        <Icons.close size={12} />
                      </button>
                    </div>
                  ))}
                </div>
              </div>
            )}
            {attachNote && <div className="sv-attach-note">{attachNote}</div>}
            {attachments.length > 0 && (
              <div className="sv-attach">
                {attachments.map((image) => (
                  <div key={image.id} className="sv-attach-item" title={image.name}>
                    <ChatImageView image={image} className="sv-attach-thumb" alt={image.name} />
                    <button
                      className="sv-attach-drop"
                      disabled={sending}
                      title={t("chat.attach.remove")}
                      onClick={() => dropAttachment(image.id)}
                    >
                      <Icons.close size={10} />
                    </button>
                  </div>
                ))}
              </div>
            )}
            <div className="sv-box">
              <textarea
                ref={inputRef}
                onPaste={onPaste}
                disabled={clientCommandRunning}
                value={draft}
                rows={1}
                placeholder={
                  busy
                    ? t("chat.placeholderBusy")
                    : session.kind === "opencode"
                      ? t("chat.placeholderOpencode")
                      : t("chat.placeholder")
                }
                onChange={(e) => {
                  updateDraft(e.target.value);
                  setCaret(e.target.selectionStart ?? e.target.value.length);
                }}
                onKeyUp={syncCaret}
                onClick={syncCaret}
                onSelect={syncCaret}
                onKeyDown={(e) => {
                  if ((e.key === "ArrowUp" || e.key === "ArrowDown") &&
                      !e.shiftKey && !e.ctrlKey && !e.metaKey && !e.altKey &&
                      !e.nativeEvent.isComposing && e.keyCode !== 229) {
                    const input = e.currentTarget;
                    const style = getComputedStyle(input);
                    const singleLineHeight = Math.max(parseFloat(style.minHeight) || 0,
                      parseFloat(style.lineHeight) + parseFloat(style.paddingTop) + parseFloat(style.paddingBottom));
                    const singleLine = !input.value.includes("\n") && input.scrollHeight <= singleLineHeight + 1;
                    // Let the browser move through explicit and visually wrapped lines before recalling history.
                    const atBoundary = singleLine || (e.key === "ArrowUp"
                      ? input.selectionStart === 0 : input.selectionEnd === input.value.length);
                    if (input.selectionStart === input.selectionEnd && atBoundary && recallInput(e.key)) {
                      e.preventDefault();
                      e.stopPropagation();
                      return;
                    }
                  }
                  if (e.key === "Escape" && (completion || (wantsCatalogue && catalogueLoading))) {
                    // Dismiss the list rather than letting Escape reach the pane, where it means something else.
                    e.preventDefault();
                    e.stopPropagation();
                    setDismissed(draft);
                    return;
                  }
                  if (e.key === "Tab" && completion) {
                    e.preventDefault();
                    complete(completion[0]);
                    return;
                  }
                  if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                    e.preventDefault();
                    // Exact client commands execute on the first Enter. Otherwise completion would add
                    // its cosmetic trailing space and make `/clear` or `/rewind` need two submissions.
                    if (attachments.length === 0 && /^\/(?:clear|new|rewind|compact|review|undo|redo|share|unshare)\s*$/.test(draft)) {
                      send(behaviorOf(e));
                    } else if (wantsCatalogue && catalogueLoading && draft !== dismissed) {
                      return;
                    } else if (completion) complete(completion[0]);
                    // Holding the modifier says "not later, now": the running turn is stopped and this
                    // message goes first. Alt says "and also this": it joins the running turn. Plain
                    // Enter queues, which is the safe default.
                    else send(behaviorOf(e));
                  }
                }}
              />
              {mobile && <div className="sv-quick-actions">
                <ComposerOptionsButton expanded={composerOptions.expanded} onToggle={composerOptions.toggle} />
                {sendButton}
              </div>}
              {/* Model, effort, collaboration style, and permission mode sit under the input, where they belong to the message
                  about to be sent rather than to the pane. */}
              {(busy || actionFeedback) && (
                <div className="sv-action-feedback" role="status" aria-live="polite">
                  {actionFeedback && <span>{actionFeedback}</span>}
                  {busy && <span>{t("chat.interruptTooltip")} · {t("chat.steerTooltip", steerCombo)}</span>}
                </div>
              )}
              <ComposerToolbar mobile={mobile} chips={composerChips} inline={composerInlineChips}
                actions={<>
                  {(session.kind === "claude" || session.kind === "codex") && <UsageMeter extras={extras} />}
                  {busy && (
                    <button
                      className="sv-steer"
                      disabled={sending || clientCommandRunning || (!draft.trim() && attachments.length === 0)}
                      onClick={() => send("steer")}
                      title={t("chat.steerTooltip", steerCombo)}
                    >
                      {t("chat.steer")}
                    </button>
                  )}
                  {!mobile && sendButton}
                </>}
                status={hasPermissionControl ? <>
                  {permissionCatalog?.error && <span role="alert">{permissionCatalog.error}</span>}
                  {/* A next-turn choice is carried by the chip's mark and the menu row notes; the states
                      that need a word of their own, like a restart, stay in this line. */}
                  {permissionValue?.activation !== "nextTurn" && (
                    <PermissionStateDetails state={permissionState?.value} error={permissionState?.error} />
                  )}
                </> : null}
              />
            </div>
          </div>}
        </div>
      </div>
      {engineConfirm}
      {permissionRestart.dialog}
    </div>
    </SessionLinkDirectory.Provider>
  );
}

/** One entry of the list: a single row, or a folded run of tool calls. */
const Entry = memo(function Entry({
  entry,
  label,
  icon,
  cwd,
  openRuns,
  onToggleRun,
  onToggleFold,
  onRewind,
  rewindDisabledReason,
  rewindScopes,
  rewindRequest,
  onRewindRequestHandled,
  onCancelShell,
}: {
  entry: DisplayRow;
  label: string;
  icon: ReactNode;
  cwd?: string;
  openRuns: ReadonlySet<string>;
  onToggleRun: (id: string) => void;
  onToggleFold?: (fold: TurnFold) => void;
  onRewind?: (rowId: string, scope: ChatRewindScope, replacement?: MessageReplacement) => void;
  rewindDisabledReason?: string;
  rewindScopes: ChatRewindScope[];
  rewindRequest: { rowId: string; token: number } | null;
  onRewindRequestHandled: (token: number) => void;
  onCancelShell?: (rowId: string) => void;
}) {
  // The author line the pass in `toolRuns` placed on the first entry of an agent turn. Everything the
  // agent did in that turn sits under it, so reasoning and tool calls read as the agent's.
  const head = entry.head ? (
    <TurnHead
      who={entry.head.who ?? label}
      icon={icon}
      at={entry.head.at}
      durationMs={entry.head.durationMs}
      fold={entry.head.fold}
      onToggleFold={onToggleFold}
    />
  ) : null;
  if (entry.kind === "fold") return head;
  if (entry.kind === "run") {
    return (
      <>
        {head}
        <ToolRunCard
          calls={entry.calls}
          renderCall={row => <Row row={row} label={label} icon={icon} cwd={cwd} />}
          running={entry.running}
          open={openRuns.has(entry.id)}
          onToggle={() => onToggleRun(entry.id)}
          cwd={cwd}
        />
      </>
    );
  }
  return (
    <>
      {head}
      <Row
        row={entry.row}
        label={label}
        icon={icon}
        cwd={cwd}
        headless
        onRewind={onRewind}
        rewindDisabledReason={rewindDisabledReason}
        rewindScopes={rewindScopes}
        rewindRequest={rewindRequest}
        onRewindRequestHandled={onRewindRequestHandled}
        onCancelShell={onCancelShell}
      />
    </>
  );
});

/** How a queued shell-mode context message reads in the queue: the command as it was typed. */
function queuedShellCommand(item: QueuedMessage): string | null {
  return item.shellCommand === undefined ? null : `! ${item.shellCommand}`;
}

/** Drop a leading "<name> · " from a model's description, where the name is already the row's label. */
function withoutNamePrefix(description: string, label: string): string {
  const prefix = `${label} \u00B7 `;
  return description.startsWith(prefix) ? description.slice(prefix.length) : description;
}

/** One row of the live conversation. */
function Row({
  row,
  label,
  icon,
  cwd,
  headless = false,
  onRewind,
  rewindDisabledReason,
  rewindScopes,
  rewindRequest,
  onRewindRequestHandled,
  onCancelShell,
}: {
  row: ChatRow;
  label: string;
  icon: ReactNode;
  cwd?: string;
  /** True when the turn's author line already stands above this row; subagent rows keep their own. */
  headless?: boolean;
  onRewind?: (rowId: string, scope: ChatRewindScope, replacement?: MessageReplacement) => void;
  rewindDisabledReason?: string;
  rewindScopes?: ChatRewindScope[];
  rewindRequest?: { rowId: string; token: number } | null;
  onRewindRequestHandled?: (token: number) => void;
  /** Stop a running shell command; the row knows only its own id. */
  onCancelShell?: (rowId: string) => void;
}) {
  const t = useT();
  switch (row.kind) {
    case "tool": {
      // A subagent's own work, drawn with the same components as the conversation: it is the same kind
      // of thing, only reported under the call that started it.
      const steps = row.children?.length ? (
        <div className="sv-subagent">
          {row.children.map((child) => (
            <Row key={child.id} row={child} label={label} icon={icon} cwd={cwd} />
          ))}
        </div>
      ) : undefined;
      return (
        <ToolCard
          source={row}
          renderChildren={children => <div className="sv-subagent">{children.map(child => <Row key={child.id} row={child} label={label} icon={icon} cwd={cwd} />)}</div>}
          name={row.name}
          input={row.input}
          output={row.output}
          isError={row.isError}
          running={row.status === "running"}
          cwd={cwd}
          steps={steps}
          stepCount={row.childCount ?? row.children?.length}
          subagent={row.subagent}
        />
      );
    }
    case "reasoning":
      return <ReasoningRow text={row.text} />;
    case "error":
      return <ErrorRow message={row.message} />;
    case "command":
      return <CommandRow text={row.text} />;
    case "shell":
      return <ShellRow row={row} onCancel={onCancelShell} />;
    case "notice":
      return <NoticeRow message={row.message} />;
    case "compaction":
      return (
        <CompactionRow
          done={row.status === "completed"}
          trigger={row.trigger}
          preTokens={row.preTokens}
        />
      );
    case "user":
      return (
        <MessageBubble
          who={row.origin ? messageSenderName(row.origin, t) : t("archive.you")}
          icon={row.origin ? kindIconEl(row.origin.agent, 14) : undefined}
          isUser
          text={row.text}
          images={row.images}
          at={row.at}
          onRewind={onRewind ? (scope) => onRewind(row.id, scope) : undefined}
          onEditSend={onRewind ? (text, images) => onRewind(row.id, "conversation", { text, images }) : undefined}
          rewindDisabledReason={rewindDisabledReason || (rewindScopes?.length ? undefined : t("chat.rewind.unsupported"))}
          editDisabledReason={rewindDisabledReason || (rewindScopes?.includes("conversation") ? undefined : t("chat.rewind.unsupported"))}
          rewindScopes={rewindScopes}
          openRewindToken={rewindRequest?.rowId === row.id ? rewindRequest.token : undefined}
          onRewindMenuOpened={onRewindRequestHandled}
        />
      );
    default:
      return (
        <MessageBubble
          who={row.model ?? label}
          isUser={false}
          icon={icon}
          text={row.text}
          at={row.at}
          durationMs={row.durationMs}
          showHead={!headless}
        />
      );
  }
}
