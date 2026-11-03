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
import { type SetStateAction, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { measureElement, observeElementOffset, useVirtualizer } from "@tanstack/react-virtual";

import Icons from "../../../components/Icons";
import { UserMessageRail } from "./UserMessageRail";
import { userMessageItems, type UserMessageItem } from "./userMessageRailItems";
import { useUserMessageJump } from "./useUserMessageJump";
import { ComposerOptionsButton, useComposerOptions } from "./ComposerOptions";
import { ComposerToolbar } from "./ComposerToolbar";
import { setComposerHeight, startComposerResize, useComposerHeight } from "./composerHeight";
import type { ComposerChip } from "./composerLayout";
import { navigateTranslation, settingsTranslationUrl, useTranslationRoute } from "../../../translation/navigation";
import { TranslationProvider, useTranslationSession } from "../../../translation/state";
import { TranslationComposerControl } from "../../../translation/ComposerControl";
import { StatusDot } from "../../../components/StatusDot";
import { translationError } from "../../../translation/SettingsSection";
import { cancelSubmissionTranslation, sendOriginalSubmission } from "./outbox";
import { ModelCatalogStatus } from "./ModelCatalogStatus";
import { useChatModels } from "./useChatModels";
import { useT, type I18nKey } from "../../../i18n";
import {
  chatAutoContinueCancel,
  chatInterrupt,
  chatPermission,
  chatQueueRemove,
  chatQueueSteer,
  chatQueueUpdate,
  chatQueueRead,
  readCompleteQueuedText,
  chatRewind,
  chatRewindPreview,
  chatHistoryGeneration,
  resolveChatImage,
  chatSetMode,
  chatSetCollaborationMode,
  chatSetModel,
  chatSetEffort,
  chatSetFastMode,
  chatSetChrome,
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
  chatReadCancel,
  readCompleteChatBody,
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
  type ChatPermission,
  type PendingPermissionMode,
  type ChatRewindPreview,
  type ChatRewindScope,
  type ChatRow,
  type ChatSnapshot,
  type QueuedMessage,
  type SendBehavior,
} from "../../../ipc/chat";
import { attachImages, restoreAttachment, MAX_IMAGE_BYTES, MAX_IMAGES, type Attachment } from "./attachments";
import { attachChatInputGuard } from "./inputGuard";
import { attachInputLatencyLog } from "./inputLatency";
import { buildSuggestions, findFileMention, mentionDir, type Suggestion } from "./completion";
import { env } from "../../../platform/env";
import { imageFromNativeClipboard, imagesFromClipboard, imagesFromDrop } from "../../../terminal/imageInput";
import { IS_MAC, IS_PLAIN_BROWSER, labelWithCombo } from "../../../hooks/shortcutRegistry";
import { useMentionFiles } from "./fileMentions";
import { ControlChip, type ChipOption } from "./controls";
import { EFFORTS, THINKING_OFF, effortChipLabel, effortChipOptions, levelsOf, modelChipOptions } from "./modelChips";
import { AutoContinueBar, FastModeChip, McpChip, NotificationBar, RetryLine, TasksChip, UsageMeter } from "./extras";
import { AgentAccountMenu, AgentAuth } from "./AgentAuth";
import { CodexResetCredits } from "./CodexResetCredits";
import { useEngineSwitch } from "./engineSwitch";
import { ConversationViewHint } from "./ConversationViewHint";
import { AgentMissingNotice } from "./AgentMissingNotice";
import { AgentVersionBadge } from "./AgentVersionBadge";
import { isShareSurface } from "../../../ipc/shareBase";
import { isAgentNotInstalledError, mapBackendError } from "../../../ipc/backendError";
import { usePermissionRestart } from "./permissionRestart";
import { PermissionCard, type PermissionAnswer } from "./permissionCards";
import { isMode, type Mode } from "./permissions";
import { useTermStore } from "../../../store/termStore";
import { effectivePermissionMode } from "../../../store/settings";
import { acceptsChatImages, effectiveStatus, hasTerminalInterface, supportsPermissionToggle, supportsSteering, type Session } from "../../../types";
import { kindIconEl } from "../../sessionViewers/sessionMeta";
import { assistantLabel } from "../../sessionViewers/TranscriptViewer";
import { ChatImageView, ErrorRow, WorkingRow } from "./rows";
import {
  estimateRowHeight,
  foldAgentTurns,
  groupToolRuns,
  markAgentTurns,
  mountedStart,
  type TurnFold,
} from "./toolRuns";
import "./session-view.css";
import { onTransportReconnect, onTransportDisconnect } from "../../../ipc/transport";
import { TranslationTime } from "../../../translation/Time";
import { useOutbox, emptySubmissions, acknowledgeSubmissions, createSubmission, deliverSubmission, retrySubmission, submissionsFor, ChatVersions, unconfirmedSubmissionRows } from "./outbox";
import { cachedChat, cacheChat, mergeRows, reconcileChat, chatSyncMetrics, limitHistoryWindow } from "./chatCache";
import { ChatSearch } from "./ChatSearch";
import { HistorySearch } from "./HistorySearch";
import { historyLocation, historyPageURL } from "./historyNavigation";
import type { ChatSearchHit } from "../../../ipc/chat";
import { QueuedMessageText } from "./QueuedMessageText";
import { MessageSender } from "./MessageSender";
import { parseShellSubmission, shellErrorKey, shellSubmissionFor, acknowledgeShellSubmission } from "./shellMode";
import { SessionLinkDirectory } from "./links";
import { Entry, type MessageReplacement } from "./ConversationRows";
import { Splitter } from "../../../components/Splitter";
import { ArtifactDock } from "../doc/artifact/ArtifactDock";
import { ArtifactHostContext, type ArtifactHost } from "../doc/artifact/ArtifactCard";
import {
  ARTIFACT_OPEN_BESIDE_EVENT,
  COMPOSER_INSERT_EVENT,
  type ArtifactOpenBesideDetail,
  type ComposerInsertDetail,
} from "../doc/artifact/artifactWriters";

/** Width of the artifact panel beside a conversation, remembered across panes and restarts. */
const DOCK_WIDTH_KEY = "vlx-artifact-dock-width";
const DOCK_MIN_W = 280;
const DOCK_MAX_W = 1400;

function readDockWidth(): number {
  const n = Number(localStorage.getItem(DOCK_WIDTH_KEY));
  return Number.isFinite(n) && n >= DOCK_MIN_W && n <= DOCK_MAX_W ? n : 520;
}

/** Distance from the bottom still counted as "at the bottom", in pixels. */
const PIN_SLACK = 40;
// Delays between automatic retries of a failed synchronization; the last one repeats.
const SYNC_RETRY_DELAYS_MS = [2_000, 5_000, 10_000, 20_000, 30_000];

/** Rows kept beyond the viewport on each side of the virtualized part, so scrolling reveals drawn rows. */
const OVERSCAN = 6;

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

/** OMP and Antigravity defer to their own approval settings in `default` mode. */
function modeLabelKeyFor(kind: Session["kind"], mode: Mode): I18nKey {
  return (kind === "omp" || kind === "antigravity") && mode === "default" ? "chat.mode.agentDefault" : modeLabelKey(mode);
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
  historyOnly = false,
  initialMessageIndex,
  initialRecordOffset,
  initialRowId,
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
  historyOnly?: boolean;
  initialMessageIndex?: number;
  /** Byte offset of a search hit's first record, which locates it more exactly than its message number. */
  initialRecordOffset?: number;
  initialRowId?: string;
  mobile?: boolean;
  onActivate: (paneId: string, id: string) => void;
  onSplit: (paneId: string, id: string, dir: "horizontal" | "vertical") => void;
  onClose: (paneId: string, id: string) => void;
}) {
  const t = useT();
  const cacheId=historyOnly ? `${session.id}\u0000history` : session.id;
  const composerOptions = useComposerOptions(mobile);
  const paneStyle = useTermStore((s) => s.paneStyle);
  const shortcutOverrides = useTermStore((s) => s.shortcutOverrides);
  const composerInlineChips = useTermStore((s) => s.composerInlineChips);
  const inputLatencyLog = useTermStore((s) => s.inputLatencyLog);
  const composerHeight = useComposerHeight();
  const status = useTermStore((s) => effectiveStatus(s.runtimes[session.id]));
  const searchOpen = useTermStore((s) => s.searchOpen);
  const closeSearch = useTermStore((s) => s.closeSearch);
  const openTaskTab = useTermStore((s) => s.openTaskTab);
  const [searchTarget, setSearchTarget] = useState<string | null>(null);
  const [historySearchQuery, setHistorySearchQuery] = useState("");
  const defaultMode = useTermStore((s) => storedMode({
    kind: session.kind,
    permissionMode: s.agentDefaults[session.kind]?.permissionMode,
  }));
  // Moving to the terminal restarts the agent under the other engine, so a working one is asked first.
  const { switchTo, confirm: engineConfirm } = useEngineSwitch(session);

  const [rows, setRows] = useState<ChatRow[]>(() => cachedChat(cacheId)?.rows ?? []);
  const [userMessages, setUserMessages] = useState(() => cachedChat(cacheId)?.userMessages ?? []);
  const rowsRef = useRef(rows);
  rowsRef.current = rows;
  const snapshotRef = useRef(cachedChat(cacheId));
  const indexReader = useRef<string | undefined>(undefined);
  const liveVersions = useRef(new ChatVersions());
  const [syncState, setSyncState] = useState<"loading" | "ready" | "failed">("loading");
  // A synchronization failed and has not yet been followed by a successful one. Sending waits for
  // synchronization, so the composer says why and offers a retry instead of leaving the button disabled.
  const [syncInterrupted, setSyncInterrupted] = useState(false);
  const [hasMore, setHasMore] = useState(cachedChat(cacheId)?.hasMore ?? false);
  const [hasNewer, setHasNewer] = useState(cachedChat(cacheId)?.hasNewer ?? false);
  const [historyCoverage, setHistoryCoverage] = useState(cachedChat(cacheId)?.coverage);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const historyBusy = useRef(false);
  // A snapshot request is in flight; page loads wait so its result does not replace the page they add.
  const refreshing = useRef(false);
  const historyRequest = useRef<Promise<ChatSnapshot | undefined> | null>(null);
  const historyGeneration = useRef(0);
  const refreshRef = useRef<() => Promise<void>>(async () => {});
  const prependScroll = useRef<{ height: number; top: number; anchor?: { id: string; offset: number } } | null>(null);
  const readingRestore = useRef<(() => void) | undefined>(undefined);
  const [submissionReceipts, setSubmissionReceipts] = useState<boolean | null>(null);
  const pendingSubmissions = useOutbox(state => state.sessions[session.id] ?? emptySubmissions);
  // Messages typed while the agent was busy. They belong to the backend, not to this pane: a second view
  // of the same session has to show the same queue, and the queue has to outlive this pane being closed.
  const [queue, setQueueState] = useState<QueuedMessage[]>(() => cachedChat(cacheId)?.queue ?? []);
  // Retire optimistic rows only after React has committed their host-backed replacements.
  useEffect(() => {
    acknowledgeSubmissions(session.id, [...rows, ...queue].map(row => row.id));
  }, [session.id, rows, queue, pendingSubmissions]);
  const [steeringQueue, setSteeringQueue] = useState(false);
  const steeringQueueRef = useRef(false);
  /** The queued message being rewritten in place, and the text so far. */
  const [editing, setEditing] = useState<{ id: string; text: string } | null>(null);
  const editingRead=useRef<AbortController|null>(null);
  const [readingQueued,setReadingQueued]=useState<string|null>(null);
  useEffect(()=>{
    setReadingQueued(null);
    return ()=>{editingRead.current?.abort();editingRead.current=null;};
  },[session.id]);
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
  const [model, setModel] = useState<string | undefined>(() => cachedChat(cacheId)?.model);
  const defaultModel = useTermStore((s) =>
    s.chatModelByKind[session.kind] ?? (session.kind === "claude" ? s.chatModel : ""),
  );
  const rememberedDefaultEffort = useTermStore((s) =>
    s.chatEffortByModel[effortKey(session.kind, model ?? "")] ??
    (session.kind === "claude" ? s.chatEffortByModel[model ?? ""] : "") ?? "",
  );
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
  /** Claude: whether Claude in Chrome is attached to this conversation. The backend owns the value. */
  const [chrome, setChrome] = useState(false);
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
  const { models: catalogue, loading: modelsLoading, failed: modelsFailed, retry: retryModels } = useChatModels(session.id, catalogueVersion, !historyOnly);
  /** Backend-owned start of the logical turn currently in flight. */
  const [actionFeedback, setActionFeedback] = useState("");
  const [sending, setSending] = useState(false);
  const translation = useTranslationSession(session.id);
  const translationRoute = useTranslationRoute();
  useEffect(() => {
    if (translation.settings && !translation.settings.config.enabled && translationRoute.get("translation") === "languages" && translationRoute.get("translationSession") === session.id) navigateTranslation(settingsTranslationUrl());
  }, [translation.settings?.config.enabled, session.id, translationRoute.get("translation"), translationRoute.get("translationSession")]);
  const shellSubmission = useRef<{ sessionId: string; command: string; id: string; pending: boolean } | null>(null);
  const stopping = useRef(false);
  const stop = () => {
    if (readOnly || stopping.current) return;
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
  const [error, setErrorState] = useState<string | null>(null);
  // A request cut short by a dropped connection is not an error of this conversation: the connection
  // banner and the sync notice already report the outage, and the snapshot taken after reconnecting
  // shows what actually happened. Showing it here would leave a stale row after the connection recovers.
  const setError = useCallback((next: SetStateAction<string | null>) => setErrorState((prev) => {
    const value = typeof next === "function" ? next(prev) : next;
    return value?.startsWith("TransportError: ") ? prev : value;
  }), []);
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
  const latestDraft = useRef(draft);
  latestDraft.current = draft;
  // Only the browsing position and unsent draft are local. Recall reads the backend's conversation and queue.
  const inputHistory = useRef<{
    id: string | null;
    draft: string;
    caret: number;
    loading: boolean;
    bodyRead?:AbortController;
  } | null>(null);
  const updateDraft = (text: string) => {
    inputHistory.current?.bodyRead?.abort();
    inputHistory.current = null;
    setDraft(text);
  };
  useEffect(() => {
    inputHistory.current?.bodyRead?.abort();
    inputHistory.current = null;
    return () => { inputHistory.current?.bodyRead?.abort();inputHistory.current = null; };
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
    historyGeneration?: string;
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

  // The artifact panel: an HTML page this conversation wrote, open beside it. Phones have no room for it,
  // so cards there open the page as a tab instead.
  const [dockPath, setDockPath] = useState<string | null>(null);
  const [dockWidth, setDockWidth] = useState(readDockWidth);
  const artifactHost = useMemo<ArtifactHost>(
    () => ({ sessionId: session.id, openBeside: mobile || readOnly ? null : setDockPath, openPath: dockPath }),
    [session.id, mobile, readOnly, dockPath],
  );
  // "Send to session" from an artifact's console puts its report in this composer without sending it.
  useEffect(() => {
    const onInsert = (e: Event) => {
      const detail = (e as CustomEvent<ComposerInsertDetail>).detail;
      if (!detail || detail.sessionId !== session.id) return;
      setDraft((prev) => (prev.trim() ? `${prev.replace(/\s+$/, "")}\n\n${detail.text}` : detail.text));
      requestAnimationFrame(() => inputRef.current?.focus());
    };
    window.addEventListener(COMPOSER_INSERT_EVENT, onInsert);
    return () => window.removeEventListener(COMPOSER_INSERT_EVENT, onInsert);
  }, [session.id]);
  // `vopen page.html` run by this conversation's agent opens the page beside it, when this pane is on screen.
  const canDock = !mobile && !readOnly && !hidden;
  useEffect(() => {
    if (!canDock) return;
    const onOpen = (e: Event) => {
      const detail = (e as CustomEvent<ArtifactOpenBesideDetail>).detail;
      if (e.defaultPrevented || !detail || detail.sessionId !== session.id) return;
      e.preventDefault();
      setDockPath(detail.path);
    };
    window.addEventListener(ARTIFACT_OPEN_BESIDE_EVENT, onOpen);
    return () => window.removeEventListener(ARTIFACT_OPEN_BESIDE_EVENT, onOpen);
  }, [canDock, session.id]);

  useEffect(() => {
    if (!inputLatencyLog) return;
    return attachInputLatencyLog(() => inputRef.current, session.id);
  }, [inputLatencyLog, session.id]);
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

  /** The first row still in view and its offset, so a reshaped window can put the reader back. Reads refs only. */
  const readingAnchor = () => {
    const scroll = scrollRef.current;
    if (!scroll) return null;
    const viewport = scroll.getBoundingClientRect();
    const anchor = Array.from(scroll.querySelectorAll<HTMLElement>(".sv-item[data-search-id]")).find(element => element.getBoundingClientRect().bottom > viewport.top);
    return { height: scroll.scrollHeight, top: scroll.scrollTop,
      anchor: anchor ? { id: anchor.dataset.searchId!, offset: anchor.getBoundingClientRect().top - viewport.top } : undefined };
  };

  /** Add or replace rows by id. The engine addresses rows this way so a message can be revised as it lands. */
  const applyRows = useCallback((incoming: ChatRow[]) => {
    setRows(prev => {
      const known = snapshotRef.current;
      if (known?.protocolVersion !== 2) return mergeRows(prev, incoming, known?.hasMore ?? false);
      // A window parked mid-history only revises what it shows. A window that holds the end keeps every
      // live row, wherever the reader is, and gives up its oldest rows instead.
      const visible = known.hasNewer ? incoming.filter(row => prev.some(current => current.id === row.id)) : incoming;
      const window = limitHistoryWindow(mergeRows(prev, visible, known.hasMore ?? false), known.hasNewer ? "older" : "newer");
      // Dropping rows above a reader who scrolled up would slide the text out from under them.
      if (window.droppedBefore && !pinnedRef.current) prependScroll.current ??= readingAnchor();
      if (window.droppedBefore) setHasMore(true);
      if (window.droppedAfter) setHasNewer(true);
      snapshotRef.current = { ...known, hasMore: known.hasMore || window.droppedBefore, hasNewer: known.hasNewer || window.droppedAfter };
      return window.rows;
    });
  }, []);

  // Follow the conversation, then start the agent. Subscribing first means nothing said during startup is
  // missed; the snapshot afterwards covers anything that happened before this pane existed.
  useEffect(() => {
    let disposed = false;
    let progressTimer: ReturnType<typeof setTimeout> | undefined;
    let retryTimer: ReturnType<typeof setTimeout> | undefined;
    let syncFailures = 0;
    let syncError: string | null = null;
    let initialLocation = true;
    indexReader.current = undefined;
    setTurnStartedAt(undefined);
    const versions = new ChatVersions();
    liveVersions.current = versions;
    versions.epoch = cachedChat(cacheId)?.startedAt ?? 0;
    versions.rows = cachedChat(cacheId)?.rowsRevision ?? 0;
    versions.queue = cachedChat(cacheId)?.queueRevision ?? 0;
    let buffered: ChatEvent[] | null = [];
    const handleEvent = (event: ChatEvent) => {
      if (disposed) return;
      switch (event.type) {
        case "rows":
          if (!versions.accept("rows", event.revision, event.epoch)) break;
          applyRows(event.rows);

          break;
        case "replaceRows":
          historyGeneration.current += 1;
          setHistoryError(null);
          prependScroll.current = null;
          if (!versions.accept("rows", event.revision, event.epoch)) break;
          setUserMessages(event.userMessages ?? []);
          {
            const window = snapshotRef.current?.protocolVersion === 2 ? limitHistoryWindow(event.rows, "newer") : undefined;
            setRows(window?.rows ?? event.rows);
            setHasMore((event.hasMore ?? false) || (window?.droppedBefore ?? false));
            setHasNewer(window?.droppedAfter ?? false);
          }
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
          setUserMessages(event.userMessages ?? []);
          versions.queue = 0;
          {
            const window = snapshotRef.current?.protocolVersion === 2 ? limitHistoryWindow(event.rows ?? [], "newer") : undefined;
            setRows(window?.rows ?? event.rows ?? []);
            setHasMore((event.hasMore ?? false) || (window?.droppedBefore ?? false));
            setHasNewer(window?.droppedAfter ?? false);
          }
          setQueue([]);
          snapshotRef.current = snapshotRef.current?.protocolVersion === 2 ? { ...snapshotRef.current, rows: [], queue: [], startedAt: event.epoch, rowsRevision: event.revision } : undefined;
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
          setCatalogueVersion(v => v + 1);
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
        case "chromeChanged":
          setChrome(event.enabled);
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
          if (!permissionRestart.handleError(event.message)) setError(mapBackendError(event.message));
          // Messages the agent never received go back to an empty composer, so signing in and sending
          // again does not mean typing them again.
          if (event.unsent?.length && !latestDraft.current) updateDraft(event.unsent.join("\n\n"));
          break;
        case "storagePressure":
          setHistoryError(t("chat.history.storagePressure"));
          break;
        case "historyReadError":
          setHistoryError(event.message);
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
    const subscribe = () => historyOnly ? Promise.resolve() : un ??= onChatEvent(session.id, event => {
      if (buffered) buffered.push(event);
      handleEvent(event);
    }).catch(error => { un = null; throw error; });
    let loading = false;
    // The conversation is on screen. Later refreshes (index progress, reconnects that succeeded) update it
    // in place instead of covering it with the synchronizing notice again.
    let synced = false;
    let receiptsAvailable = false;
    const refresh = async () => {
      if (loading || disposed) return;
      if (historyBusy.current) {progressTimer = setTimeout(() => void refresh(), 1000);return;}
      clearTimeout(retryTimer);
      let failed = false;
      loading = true;
      refreshing.current = true;
      historyGeneration.current += 1;
      setHistoryError(null);
      if (!synced) setSyncState("loading");
      buffered = [];
      const requestStarted = performance.now();
      try {
        await subscribe();
        if (disposed) return;
        const known = snapshotRef.current;
        const location = known?.pendingLocation ?? (initialLocation ? { ...historyLocation(session.id), ...(initialRowId ? {anchor:initialRowId} : {
          ...(initialMessageIndex!==undefined ? {messageIndex: initialMessageIndex} : {}), ...(initialRecordOffset!==undefined ? {recordOffset: initialRecordOffset} : {}) }) } : {});
        const hasLocation = Boolean(location.before || location.after || location.anchor || location.messageIndex!==undefined || location.recordOffset!==undefined);
        let response: ChatSnapshot;
        try {
          response = await chatSnapshot(session.id, { historyOnly,
            ...(known ? { since: known.rowsRevision, epoch: known.startedAt, from: known.rows[0]?.id } : {}),
            ...(known?.protocolVersion === 2 || hasLocation ? { protocolVersion: 2 as const, readJobId: indexReader.current,
              anchor: pinnedRef.current ? undefined : known?.rows.at(-1)?.id } : {}),
            ...location,
          });
        } catch (error) {
          if (known?.protocolVersion !== 2 || !/^(source_changed|cursor_expired):/.test(String(error).replace(/^Error: /, ""))) throw error;
          if (indexReader.current) void chatReadCancel(session.id, indexReader.current).catch(() => {});
          indexReader.current = undefined;
          response = await chatSnapshot(session.id, { historyOnly, protocolVersion: 2 });
        }
        if (disposed) {
          if (response.readJobId) void chatReadCancel(session.id, response.readJobId).catch(() => {});
          return;
        }
        indexReader.current = response.readJobId;
        initialLocation = false;
        const pendingEvents = buffered ?? [];
        const snapshot = reconcileChat(response, rowsRef.current, pendingEvents);
        if (hasLocation) {
          pinnedRef.current = false;setAway(true);
          if (location.anchor || snapshot.locatedRowId) {const anchor=snapshot.locatedRowId ?? location.anchor!;pendingHistoryHit.current = { rowId: anchor, snippet: "", offset: 0 };setSearchTarget(anchor);}
        }
        buffered = null;
        if (snapshot.startedAt === undefined || snapshot.startedAt >= versions.epoch) {
          setRows(snapshot.rows);
          setUserMessages(snapshot.userMessages ?? []);
          setQueue(snapshot.queue);
          versions.epoch = snapshot.startedAt ?? versions.epoch;
          versions.rows = snapshot.rowsRevision ?? versions.rows;
          versions.queue = snapshot.queueRevision ?? versions.queue;
          setHasMore(snapshot.hasMore ?? false);
          setHasNewer(snapshot.hasNewer ?? false);
          setHistoryCoverage(snapshot.coverage);
          snapshotRef.current = snapshot;
          cacheChat(cacheId, snapshotRef.current);
        }
        setSyncState("ready");
        synced = true;
        syncFailures = 0;
        setSyncInterrupted(false);
        if (syncError) { const shown = syncError; setError(prev => (prev === shown ? null : prev)); syncError = null; }
        setHistoryError(snapshot.historyError ?? null);
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
        if (session.kind === "claude") setChrome(snapshot.chrome ?? false);
        setAutoContinue(snapshot.autoContinue ?? null);
        // Only while a process is actually behind the conversation: a snapshot taken after one exited
        // still names it, and reporting that as running would light the session up in every sidebar.
        if (!historyOnly && snapshot.running && snapshot.pid !== undefined) {
          useTermStore.getState().setRuntime(session.id, {
            status: "running",
            pid: snapshot.pid,
            startedAt: snapshot.startedAt,
          });
        }
        if (!historyOnly && !snapshot.running) {
          useTermStore.getState().setRuntime(session.id, { status: "idle", pid: undefined, startedAt: undefined });
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
        if (!disposed) {
          syncError = String(err);
          setError(syncError);
          setSyncState("failed");
          setSyncInterrupted(true);
          synced = false;
          failed = true;
        }
      } finally {
        const events = buffered ?? [];
        buffered = null;
        loading = false;
        refreshing.current = false;
        clearTimeout(progressTimer);
        // A failed synchronization retries by itself with a growing delay; otherwise sending stays blocked
        // until the user finds the retry button or the transport happens to reconnect.
        if (failed && !disposed) {
          const delay = SYNC_RETRY_DELAYS_MS[Math.min(syncFailures, SYNC_RETRY_DELAYS_MS.length - 1)];
          syncFailures += 1;
          retryTimer = setTimeout(() => void refresh(), delay);
        }
        if (!disposed && snapshotRef.current?.protocolVersion === 2 && (historyOnly || snapshotRef.current.coverage === "partial" || snapshotRef.current.indexing)) {
          progressTimer = setTimeout(() => void refresh(), snapshotRef.current.coverage === "complete" && !snapshotRef.current.indexing ? 5000 : 1000);
        }
        // Events were rendered as they arrived, including when the snapshot failed.
        void events;
      }
    };
    refreshRef.current = refresh;
    void refresh();
    const disconnect = onTransportDisconnect(() => {
      synced = false;
      setSyncState("failed");
      setSyncInterrupted(true);
      setTurnStartedAt(undefined);
      setPermissions([]);
    });
    const reconnect = onTransportReconnect(() => {
      void refresh().then(() => {
        if (!disposed && receiptsAvailable) for (const item of submissionsFor(session.id)) {
          if (item.status === "unknown") void retrySubmission(session.id, item, undefined, () => translation.refresh(true));
        }
      });
    });
    return () => {
      disposed = true;
      clearTimeout(progressTimer);
      clearTimeout(retryTimer);
      if (indexReader.current) void chatReadCancel(session.id, indexReader.current).catch(() => {});
      indexReader.current = undefined;
      historyGeneration.current += 1;
      reconnect();
      disconnect();
      void un?.then((f) => f()).catch(() => {});
    };
  }, [session.id, session.kind, applyRows, t, cacheId, historyOnly, initialMessageIndex,initialRecordOffset,initialRowId]);

  useEffect(() => {
    if (snapshotRef.current) {
      snapshotRef.current = { ...snapshotRef.current, userMessages, auth: extras.auth, rows, queue, hasMore, hasNewer, rowsRevision: liveVersions.current.rows, queueRevision: liveVersions.current.queue };
      cacheChat(cacheId, snapshotRef.current);
    }
  }, [session.id, rows, userMessages, queue, hasMore, hasNewer, extras.auth, cacheId]);

  const loadHistory = async (before = rows[0]?.id) => {
    if (historyRequest.current) return historyRequest.current;
    if (historyBusy.current || refreshing.current || !hasMore || syncState !== "ready" || !before) return;
    const rememberPosition = () => {
      const scroll = scrollRef.current;
      if (!scroll || pinnedRef.current) return;
      const viewport = scroll.getBoundingClientRect();
      // Match the rail's probe beyond the jump inset instead of anchoring a sliver of the preceding row.
      const top = viewport.top + 16;
      const anchor = Array.from(scroll.querySelectorAll<HTMLElement>(".sv-item[data-search-id]"))
        .find(element => element.getBoundingClientRect().bottom > top && element.getBoundingClientRect().top < viewport.bottom);
      prependScroll.current = {
        height: scroll.scrollHeight, top: scroll.scrollTop,
        anchor: anchor ? { id: anchor.dataset.searchId!, offset: anchor.getBoundingClientRect().top - viewport.top } : undefined,
      };
    };
    // Showing or clearing the history error must preserve the same reading anchor as a prepend.
    if (historyError) rememberPosition();
    historyBusy.current = true;
    setHistoryLoading(true);
    setHistoryError(null);
    const generation = historyGeneration.current;
    const request = (async () => {
      try {
        const page = await chatSnapshot(session.id, { historyOnly, before, epoch: snapshotRef.current?.startedAt ?? (liveVersions.current.epoch || undefined),
          ...(snapshotRef.current?.protocolVersion === 2 ? { protocolVersion: 2 as const, historyGeneration: snapshotRef.current.historyGeneration, readJobId: indexReader.current } : {}) });
        if (generation !== historyGeneration.current) return;
        if (page.pageKind !== "history") { await refreshRef.current(); return; }
        rememberPosition();
        setRows(current => {
          const merged = mergeRows(page.rows, current);
          if (page.protocolVersion !== 2) return merged;
          const window = limitHistoryWindow(merged, "older");
          const hasNewer = (snapshotRef.current?.hasNewer ?? false) || window.droppedAfter;
          setHasNewer(hasNewer);
          if (snapshotRef.current) snapshotRef.current = { ...snapshotRef.current, hasNewer };
          return window.rows;
        });
        setHasMore(page.hasMore ?? false);
        return page;
      } catch (error) {
        if (generation === historyGeneration.current) { rememberPosition(); setHistoryError(String(error)); }
      }
      finally { historyBusy.current = false; historyRequest.current = null; setHistoryLoading(false); }
    })();
    historyRequest.current = request;
    return request;
  };

  const loadNewerHistory = async () => {
    const known = snapshotRef.current;
    if (known?.protocolVersion !== 2 || !hasNewer || historyBusy.current || refreshing.current || syncState !== "ready") return;
    const after = rowsRef.current.at(-1)?.id;
    if (!after) return;
    historyBusy.current = true;setHistoryLoading(true);setHistoryError(null);
    const generation = historyGeneration.current;
    try {
      const page = await chatSnapshot(session.id, { historyOnly, protocolVersion: 2, after, historyGeneration: known.historyGeneration, readJobId: indexReader.current });
      if (generation !== historyGeneration.current) return;
      prependScroll.current = readingAnchor() ?? prependScroll.current;
      setRows(current => {
        const window = limitHistoryWindow(mergeRows(current, page.rows), "newer");
        setHasMore((known.hasMore ?? false) || window.droppedBefore);return window.rows;
      });
      setHasNewer(page.hasNewer ?? false);
      if (snapshotRef.current) snapshotRef.current = { ...snapshotRef.current, hasNewer: page.hasNewer };
      return page;
    } catch (error) {if (generation === historyGeneration.current) setHistoryError(String(error));}
    finally {historyBusy.current = false;setHistoryLoading(false);}
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
      const original=entry && history.find(row=>row.id===entry.id);
      browsing.bodyRead?.abort();
      if (original?.bodyAvailable) {
        browsing.loading=true;const controller=new AbortController();browsing.bodyRead=controller;
        void readCompleteChatBody(original,controller.signal).then(text=> {
          if (inputHistory.current!==browsing) return;
          const at=direction==="ArrowUp"?0:text.length;
          inputHistory.current={...browsing,id:original.id,loading:false};setDraft(text);setCaret(at);setDismissed(text);placeCaret.current=at;
        }).catch(cause=>{if (!controller.signal.aborted && inputHistory.current===browsing) setError(String(cause));}).finally(()=>{browsing.loading=false;});
        return;
      }
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
            history = page.protocolVersion === 2 ? limitHistoryWindow(mergeRows(page.rows, history), "older").rows : mergeRows(page.rows, history);
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
  // Legacy readers request older pages only after an actual query. V2 searches stay on the backend.
  useEffect(() => { if (searchOpen && historySearchQuery && focused && !hidden && hasMore && !historyError && snapshotRef.current?.protocolVersion !== 2) void loadHistory(); }, [searchOpen, historySearchQuery, focused, hidden, hasMore, rows.length, syncState, historyError]);
  useEffect(() => { if (!searchOpen) setHistorySearchQuery(""); }, [searchOpen]);

  // Tell the backend when a view holds this conversation and when it lets go. A pane is unmounted only
  // when its session leaves every tab's layout — a background tab keeps it mounted — so letting go
  // means the view was closed. The agent process then ends once it is idle; the conversation stays, and
  // the next message sent starts the agent again where it left off. A pane mounted while a turn still
  // runs cancels the release that a closed one asked for.
  useEffect(() => {
    if (historyOnly) return;
    void chatAttach(session.id).catch(() => {});
    return () => {
      void chatDetach(session.id).catch(() => {});
    };
  }, [session.id, historyOnly]);

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
  const display = useMemo(() => [...folded.rows, ...unconfirmedSubmissionRows(rows, queue, pendingSubmissions)
    .map(row => ({ kind: "row" as const, id: row.id, row }))], [folded.rows, rows, queue, pendingSubmissions]);
  const messageOrderRef = useRef<string[]>([]);
  const messageOrder = useMemo(() => {
    const next = display.map(entry => entry.id);
    const previous = messageOrderRef.current;
    if (next.length === previous.length && next.every((id, i) => id === previous[i])) return previous;
    messageOrderRef.current = next;
    return next;
  }, [display]);
  const railItemsRef = useRef<UserMessageItem[]>([]);
  const railItems = useMemo(() => {
    const next = userMessageItems(rows, userMessages, messageOrder, railItemsRef.current);
    railItemsRef.current = next;
    return next;
  }, [rows, userMessages, messageOrder]);
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
    const anchor = previous.anchor;
    let frame: number | undefined;
    let stopped = false;
    let frames = 0;
    let stable = 0;
    const stop = () => {
      stopped = true;
      if (frame !== undefined) cancelAnimationFrame(frame);
      if (readingRestore.current === stop) readingRestore.current = undefined;
      scroll.removeEventListener("wheel", stop);
      scroll.removeEventListener("touchstart", stop);
      scroll.removeEventListener("pointerdown", stop);
      scroll.removeEventListener("keydown", onKey);
    };
    const onKey = (event: KeyboardEvent) => {
      if (["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End"].includes(event.key)) stop();
    };
    // Preserve the anchor until deferred virtual measurements settle, including after a canceled jump.
    const restore = () => {
      if (stopped || pinnedRef.current || !scroll.clientHeight) { stop(); return; }
      const element = anchor && Array.from(scroll.querySelectorAll<HTMLElement>(".sv-item[data-search-id]"))
        .find(item => item.dataset.searchId === anchor.id);
      const top = scroll.scrollTop;
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
      readerTop.current = scroll.scrollTop;
      syncVirtualOffset.current?.();
      stable = Math.abs(scroll.scrollTop - top) < 1 ? stable + 1 : 0;
      if (++frames >= 60 || (frames >= 3 && stable >= 2)) { stop(); return; }
      frame = requestAnimationFrame(restore);
    };
    scroll.addEventListener("wheel", stop, { passive: true });
    scroll.addEventListener("touchstart", stop, { passive: true });
    scroll.addEventListener("pointerdown", stop);
    scroll.addEventListener("keydown", onKey);
    readingRestore.current = stop;
    restore();
    return stop;
  }, [rows, historyError, virtualRows, virtualizer]);

  /** Scroll a drawn entry to the middle of the view; false when the entry is not in the list. */
  const scrollToEntry = useCallback((id: string) => {
    readingRestore.current?.();
    const index = display.findIndex(entry => entry.id === id);
    if (index < 0) return false;
    if (index < split) virtualizer.scrollToIndex(index, { align: "center" });
    else scrollRef.current?.querySelectorAll<HTMLElement>(".sv-item[data-search-id]").forEach(element => {
      const root = scrollRef.current;
      if (root && element.dataset.searchId === id) root.scrollTop += element.getBoundingClientRect().top - root.getBoundingClientRect().top - root.clientHeight / 2;
    });
    return true;
  }, [display, split, virtualizer]);
  const locateUserMessage = useCallback((id: string, signal: AbortSignal) => new Promise<boolean>(resolve => {
    const index = display.findIndex(entry => entry.id === id);
    if (index < 0 || signal.aborted) { resolve(false); return; }
    readingRestore.current?.();
    pinnedRef.current = false;
    setAway(true);
    if (index < split) virtualizer.scrollToIndex(index, { align: "start" });
    let frames = 0;
    let stable = 0;
    const settle = () => {
      const scroll = scrollRef.current;
      if (signal.aborted || !scroll?.clientHeight) { resolve(false); return; }
      const element = Array.from(scroll.querySelectorAll<HTMLElement>(".sv-item[data-message-id]"))
        .find(element => element.dataset.messageId === id);
      if (element) {
        const delta = element.getBoundingClientRect().top - scroll.getBoundingClientRect().top - 12;
        const previous = scroll.scrollTop;
        scroll.scrollTop += delta;
        // The last message can reach the scroll limit before it reaches the top.
        stable = Math.abs(delta) < 1 || scroll.scrollTop === previous ? stable + 1 : 0;
        readerTop.current = scroll.scrollTop;
        syncVirtualOffset.current?.();
        if (stable >= 2) { resolve(true); return; }
      }
      if (++frames >= 60) { resolve(Boolean(element)); return; }
      requestAnimationFrame(settle);
    };
    requestAnimationFrame(settle);
  }), [display, split, virtualizer]);
  const railJump = useUserMessageJump({
    items: railItems, sessionKey: `${session.id}:${historyGeneration.current}`, enabled: syncState === "ready" && !hidden && !(searchOpen && focused),
    hasMore, failed: t("chat.rail.failed"), unavailable: t("chat.rail.unavailable"), locate: locateUserMessage,
    loadEarlier: async () => {
      const before = rows[0]?.id;
      const page = await loadHistory();
      return Boolean(page?.rows.length && page.rows[0].id !== before);
    },
  });
  /** A search match inside a collapsed turn, waiting for the opened turn to be drawn. */
  const pendingLocate = useRef<string | null>(null);

  const locateSearch = useCallback((index: number) => {
    railJump.cancel();
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
  }, [turnEntries, folded, scrollToEntry, railJump.cancel]);
  const pendingHistoryHit = useRef<ChatSearchHit | null>(null);
  const locateHistorySearch = useCallback(async (hit: ChatSearchHit) => {
    railJump.cancel();
    pinnedRef.current = false; setAway(true);
    if (!rowsRef.current.some(row => row.id === hit.rowId)) {
      const page = await chatSnapshot(session.id, { historyOnly, protocolVersion: 2, anchor: hit.rowId, historyGeneration: snapshotRef.current?.historyGeneration, readJobId: indexReader.current });
      setRows(page.rows); setHasMore(page.hasMore ?? false);setHasNewer(page.hasNewer ?? false);
      if (snapshotRef.current) snapshotRef.current = { ...snapshotRef.current, ...page, pendingLocation: page.pendingLocation };
    }
    pendingHistoryHit.current = hit;
    const url = new URL(window.location.href);
    url.searchParams.set("chatSession", session.id);
    url.searchParams.set("session", session.id);
    url.searchParams.delete("chatBefore");url.searchParams.delete("chatAfter");
    url.searchParams.set("chatRow", hit.rowId);
    if (hit.detailRef) url.searchParams.set("chatField", hit.detailRef); else url.searchParams.delete("chatField");
    url.searchParams.set("chatPart", String(hit.chunkSequence ?? 0));
    window.history.pushState(null, "", url.href); window.dispatchEvent(new PopStateEvent("popstate"));
    setSearchTarget(hit.rowId);
  }, [session.id, railJump.cancel]);
  useEffect(() => {
    const hit = pendingHistoryHit.current; if (!hit) return;
    const index = turnEntries.findIndex(entry => entry.kind === "row" ? entry.row.id === hit.rowId : entry.kind === "run" && entry.calls.some(row => row.id === hit.rowId));
    if (index < 0) return;
    pendingHistoryHit.current = null; locateSearch(index);
  }, [turnEntries, locateSearch, searchTarget]);
  useEffect(() => {
    if (syncState !== "ready" || snapshotRef.current?.protocolVersion !== 2) return;
    let active = true;
    let revision = 0;
    const restore = async () => {
      const location = historyLocation(session.id);
      const current = ++revision;
      if (location.anchor && rowsRef.current.some(row => row.id === location.anchor)) {
        pendingHistoryHit.current = { rowId: location.anchor, snippet: "", offset: 0 };
        setSearchTarget(location.anchor);return;
      }
      try {
        const page = await chatSnapshot(session.id, { historyOnly, protocolVersion: 2, ...location, readJobId: indexReader.current });
        if (!active || current !== revision) return;
        indexReader.current = page.readJobId;
        pinnedRef.current = !location.before && !location.after && !location.anchor;
        setAway(!pinnedRef.current);setRows(page.rows);setHasMore(page.hasMore ?? false);setHasNewer(page.hasNewer ?? false);
        setHistoryCoverage(page.coverage);setHistoryError(page.historyError ?? null);
        snapshotRef.current = { ...snapshotRef.current!, ...page, pendingLocation: page.pendingLocation };
        if (location.anchor) {pendingHistoryHit.current = { rowId: location.anchor, snippet: "", offset: 0 };setSearchTarget(location.anchor);}
      } catch (error) {if (active && current === revision) setHistoryError(String(error));}
    };
    window.addEventListener("popstate", restore);
    return () => {active = false;revision++;window.removeEventListener("popstate", restore);};
  }, [session.id, syncState]);
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
    railJump.cancel();
    pinnedRef.current = true;
    setAway(false);
    if (snapshotRef.current?.protocolVersion === 2 && snapshotRef.current.hasNewer) {
      window.history.pushState(null, "", historyPageURL(session.id, "after"));
      void refreshRef.current();
    }
    const el = scrollRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
      scrollGeometry.current = { height: el.scrollHeight, viewport: el.clientHeight };
    }
  }, [railJump.cancel, session.id]);

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
    const target=rows.find(row=>row.id===rowId);
    const historyGeneration=target ? chatHistoryGeneration(target) : undefined;
    if (scope === "conversation") {
      setPendingRewind({ rowId, scope, replacement, historyGeneration, loading: false });
      return;
    }
    setPendingRewind({ rowId, scope, historyGeneration, loading: true });
    void chatRewindPreview(session.id, rowId, historyGeneration)
      .then((preview) => setPendingRewind((current) => current?.rowId === rowId && current.scope === scope
        ? { rowId, scope, preview, historyGeneration: preview.historyGeneration ?? historyGeneration, loading: false } : current))
      .catch((err) => {
        setPendingRewind(null);
        setError(String(err));
      });
  }, [canRewind, rewindScopes, session.id, rows]);

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
        ? await Promise.all((pending.replacement?.images ?? target.images ?? []).map(image=>resolveChatImage(image,true))) : [];
      const restored = pending.replacement === undefined ? images.map(restoreAttachment) : [];
      const result = await chatRewind(session.id, pending.rowId, pending.scope, pending.historyGeneration);
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
        const text = pending.replacement.text;
        const item = createSubmission(session.id, text, images.map(({ mimeType, data }) => ({ mimeType, data })), "queue", shouldTranslate(text));
        toEnd();
        const receipt = await deliverSubmission(session.id, item, undefined, () => translation.refresh(true));
        finishSubmission(item, receipt);
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

  // Codex opens its thread before receiving input. Claude reports its native session only after the
  // first message, so translated submissions must use the ordinary process startup for other agents.
  const startTranslatedAgent = session.kind === "codex" ? ensureStarted : startAgent;

  /**
   * Send what is typed, or decide what happens to it when the agent is busy.
   *
   * `queue` is the default because it is what someone typing a follow-up almost always means: the thought
   * lands now, the agent takes it when it comes up for air. `interrupt` is the deliberate other choice —
   * stop what you are doing and read this instead.
   */
  const finishSubmission = (item: import("./outbox").Submission, receipt: string | undefined) => {
    if (!receipt && !latestDraft.current && submissionsFor(session.id).find(value => value.id === item.id)?.error?.includes("translation:required")) updateDraft(item.text);
    void translation.refresh();
  };

  const shouldTranslate = (text: string) => !!translation.settings?.config.enabled && !!translation.value?.enabled
    && !!translation.value.preferences.outgoing && !!text.trim() && !text.trimStart().startsWith("/");

  const send = (behavior: SendBehavior = "queue") => {
    if (sending || shellSubmission.current?.pending || syncState !== "ready") return;
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
        ? session.kind === "codex" || session.kind === "pi" || session.kind === "omp" || session.kind === "zcode"
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
    const translate = shouldTranslate(text);
    const existing = submissionsFor(session.id).find(item => item.text === text && item.behavior === behavior && JSON.stringify(item.images) === JSON.stringify(images) && !["sent", "queued"].includes(item.status));
    const item = existing ?? createSubmission(session.id, text, images, behavior, translate);
    setSending(true);
    setError(null);
    updateDraft("");
    setCaret(0);
    setDismissed(null);
    setAttachments([]);
    setAttachNote(null);
    toEnd();
    void deliverSubmission(session.id, item, engineRunning ? undefined : translate ? startTranslatedAgent : startAgent, () => translation.refresh(true))
      .then(receipt => finishSubmission(item, receipt))
      .finally(() => setSending(false));
  };

  /**
   * Take pasted or dropped images into the composer, saying what could not be taken.
   *
   * Files that do not fit are left out rather than failing the whole paste, and the note explains which —
   * a screenshot silently disappearing is worse than one that says why it did.
   */
  const addFiles = async (files: File[]) => {
    if (!acceptsChatImages(session.kind)) {
      setAttachNote(session.kind === "antigravity" ? t("chat.antigravity.textOnly") : t("chat.textOnly", assistantLabel(session.kind)));
      return;
    }
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
      void refreshRef.current();
    } finally {
      steeringQueueRef.current = false;
      setSteeringQueue(false);
    }
  };

  const removeQueued = (id: string) => {
    if (readingQueued===id) {editingRead.current?.abort();setReadingQueued(null);}
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
    void chatQueueUpdate(session.id, id, trimmed).catch((err) => { setError(String(err)); void refreshRef.current(); });
  };

  const editQueued=async (item:QueuedMessage)=>{
    if (!item.textDeferred) {setEditing({id:item.id,text:item.text});return;}
    editingRead.current?.abort();const controller=new AbortController();editingRead.current=controller;setReadingQueued(item.id);
    try {
      const text=await readCompleteQueuedText(session.id,item,controller.signal);
      if (controller.signal.aborted) return;
      const current=queueRef.current.find(current=>current.id===item.id);
      if (!current || current.textRevision!==item.textRevision || current.shellCommand!==undefined) return;
      setEditing({id:item.id,text});
    } catch (cause) {if (!controller.signal.aborted) setError(String(cause));}
    finally {if (editingRead.current===controller) {editingRead.current=null;setReadingQueued(null);}}
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
    return (async () => {
      try {
        // The mode goes first: approving a plan is what ends plan mode, and the agent has to be out of it
        // before the tool it was asking about runs. It is also how a mode the answer adopts reaches the
        // chip and the stored session setting, which the protocol's own `updatedPermissions` never sees.
        if (reply.mode) await applyMode(reply.mode);
        await chatPermission(session.id, request.id, reply.allow, {
          updatedInput: reply.updatedInput,
          message: reply.message,
          updatedPermissions: reply.updatedPermissions,
          questionAnswers: reply.questionAnswers,
          sendOriginal: reply.sendOriginal,
        });
        setPermissions((prev) => prev.filter((p) => p.id !== request.id));
      } catch (err) {
        if (reply.questionAnswers) throw err;
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

  /** Attach or detach Claude in Chrome for this conversation; ticking the box also makes it the default. */
  const chromeDefault = useTermStore((s) => s.chatChromeDefault);
  const pickChrome = (value: "on" | "off", keep: boolean) => {
    const enabled = value === "on";
    void (async () => {
      if (enabled !== chrome) {
        await chatSetChrome(session.id, enabled);
        setChrome(enabled);
      }
      if (keep) useTermStore.getState().setChatChromeDefault(enabled);
    })().catch((err) => setError(String(err)));
  };
  const chromeOptions: ChipOption<"on" | "off">[] = [
    { value: "on", label: t("chat.chrome.on") },
    { value: "off", label: t("chat.chrome.off") },
  ];

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
    // Pi, OMP and ZCode expose compaction as a protocol command of their own.
    if (session.kind === "pi" || session.kind === "omp" || session.kind === "zcode") {
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
  // The suggestion Tab and Enter take. Arrow keys move it; a new list starts again from its first entry.
  const [activeSuggestion, setActiveSuggestion] = useState(0);
  const completionList = useRef<HTMLDivElement>(null);
  const completionPointer = useRef<{ x: number; y: number } | null>(null);
  useEffect(() => { setActiveSuggestion(0); }, [completion]);
  useEffect(() => {
    completionList.current?.querySelector('[aria-selected="true"]')?.scrollIntoView?.({ block: "nearest" });
  }, [activeSuggestion, completion]);
  const chosenSuggestion = completion?.[Math.min(activeSuggestion, completion.length - 1)];

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
    !busy ? "queue" : e.metaKey || e.ctrlKey ? "interrupt" : e.altKey && supportsSteering(session.kind) ? "steer" : "queue";

  const label = assistantLabel(session.kind);
  // The agent's own mark, drawn in the margin beside each of its answers.
  const kindIcon = useMemo(() => kindIconEl(session.kind, 12), [session.kind]);

  // The default names no model, so it carries the settings mark; every model row carries the agent's own
  // mark in its brand colour, the same mark the chip shows.
  const modelOptions = modelChipOptions(t, session.kind, catalogue, t("chat.followModelDefault", label));
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
  const effortLabel = (value: string) => effortChipLabel(t, value);

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
    ...effortChipOptions(t, allowed),
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
  /** An agent without a terminal interface installs from the notice itself; the send that found it missing
   *  then goes again, which starts the agent. */
  const afterInPlaceInstall = () => {
    setError(null);
    const item = agentMissingItem;
    if (!item || readOnly) return;
    void retrySubmission(session.id, item, !engineRunning ? item.translation ? startTranslatedAgent : startAgent : undefined, () => translation.refresh(true))
      .then(receipt => finishSubmission(item, receipt));
  };

  const submissionFeedback = (id: string) => {
    const item = pendingSubmissions.find(value => value.id === id);
    if (!item) return null;
    return <div className="sv-submission-status" role="status">
      <span className={item.status === "translating" ? "translation-progress" : undefined}>{item.status === "translating" && <StatusDot status="working" size={6} />}{item.status === "translating" ? t("translation.translating") : item.status === "translationFailed" ? t("translation.failed") : item.status === "canceled" ? t("translation.canceled") : t(`chat.submission.${item.status}`)}</span>
      {item.translation && ["translating", "translationFailed", "canceled"].includes(item.status) && <TranslationTime timing={translation.value?.pending.find(value => value.id === id) ?? item} serverTime={translation.value?.serverTime} running={item.status === "translating"} compact />}
      {!readOnly && translation.settings?.config.enabled && item.status === "translating" && <button className="vlx-btn" onClick={() => void cancelSubmissionTranslation(session.id, item).catch(err => setError(translationError(err, t)))}>{t("translation.cancel")}</button>}
      {!readOnly && translation.settings?.config.enabled && (item.status === "translationFailed" || item.status === "canceled") && <>
        <button className="vlx-btn" disabled={!translation.value?.enabled} onClick={() => void retrySubmission(session.id, item, !engineRunning ? item.translation ? startTranslatedAgent : startAgent : undefined, () => translation.refresh(true)).then(receipt => finishSubmission(item, receipt))}>{t("translation.retry")}</button>
        <button className="vlx-btn" onClick={() => void sendOriginalSubmission(session.id, item, !engineRunning ? startAgent : undefined).then(receipt => finishSubmission(item, receipt)).catch(err => setError(translationError(err, t)))}>{t("translation.sendOriginal")}</button>
      </>}
      {!readOnly && (item.status === "failed" || item.status === "unknown") && <button className="vlx-btn" disabled={submissionReceipts !== true} onClick={() => void retrySubmission(session.id, item, item.status === "failed" && !engineRunning ? item.translation ? startTranslatedAgent : startAgent : undefined, () => translation.refresh(true)).then(receipt => finishSubmission(item, receipt))}>
        {t(item.status === "unknown" ? "chat.submission.check" : "common.retry")}
      </button>}
      {item.error && item.status !== "canceled" && !isAgentNotInstalledError(item.error) && <ErrorRow message={item.translation ? translationError(item.error, t) : item.error} />}
    </div>;
  };

  // Every chip that exists for this session, in the toolbar's traditional order. The toolbar applies the
  // user's inline preference on top and never renders a chip the pane did not offer.
  const composerChips: ComposerChip[] = [];
  if (translation.settings?.config.enabled && translation.value?.enabled) composerChips.push({ id: "translation", node: <TranslationComposerControl /> });
  composerChips.push({ id: "model", node: (
    <ControlChip
      glyph={kindIconEl(session.kind, 14)}
      label={modelLabel}
      title={session.kind === "antigravity" ? t("chat.antigravity.settingsHint") : t("chat.modelTooltip")}
      disabled={session.kind === "antigravity" && busy}
      value={model ?? ""}
      options={modelsFailed || catalogue.length === 0 ? [] : modelOptions}
      defaultValue={defaultModel || undefined}
      defaultLabel={t("chat.savedModelDefault")}
      footer={<>
        {(modelsLoading || modelsFailed || catalogue.length === 0) && <div className="sv-model-catalog-status">
          <div role="status">{t(modelsLoading ? "common.loading" : modelsFailed ? "chat.modelsLoadFailed" : "chat.modelsEmpty")}</div>
          {!modelsLoading && <button type="button" className="sv-model-catalog-refresh" onClick={retryModels}>{t("common.retry")}</button>}
        </div>}
        {session.kind === "claude" && extras.listModelsUnsupported && <div className="sv-model-cli-outdated" role="note">{t("chat.modelsCliOutdated")}</div>}
      </>}
      advancedFooter={session.kind === "claude" ? <ModelCatalogStatus onChanged={() => setCatalogueVersion(v => v + 1)} /> : undefined}
      onPick={pickModel}
      keepLabel={modelsFailed || catalogue.length === 0 ? undefined : t("chat.keepChoice")}
      onKeepCurrent={() => rememberPair(model ?? "", effort)}
      menuWidth={300}
      filterPlaceholder={t("chat.filterPlaceholder")}
    />
  ) });
  composerChips.push({ id: "effort", node: (
    <ControlChip
      glyph={<Icons.cpu size={14} />}
      label={effort ? effortLabel(effort) : t("chat.effortDefault")}
      title={session.kind === "antigravity" ? t("chat.antigravity.settingsHint") : t("chat.effortTooltip")}
      disabled={session.kind === "antigravity" && busy}
      value={effort}
      options={effortOptions}
      defaultValue={defaultEffort}
      onPick={pickEffort}
      // A level belongs to a model, so the box says which one it would become the default for.
      keepLabel={selected ? t("chat.keepChoiceFor", selected.label) : t("chat.keepChoice")}
      onKeepCurrent={(value) => rememberPair(model ?? "", value)}
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
        disabled={!permissionCatalog?.catalog || (session.kind === "antigravity" && busy)}
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
  if (session.kind === "claude") composerChips.push({ id: "chrome", node: (
    <ControlChip
      glyph={<Icons.globe size={14} style={chrome ? { color: "var(--accent)" } : undefined} />}
      label={t("chat.chrome.label")}
      title={chrome ? t("chat.chrome.tooltipOn") : t("chat.chrome.tooltipOff")}
      value={chrome ? "on" : "off"}
      options={chromeOptions}
      defaultValue={chromeDefault ? "on" : "off"}
      defaultLabel={t("chat.savedModelDefault")}
      onPick={pickChrome}
      keepLabel={t("chat.keepChoice")}
      onKeepCurrent={(value) => useTermStore.getState().setChatChromeDefault(value === "on")}
      menuWidth={240}
    />
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
  const authProvider = session.kind === "claude" ? "Claude" : session.kind === "codex" ? "Codex" : session.kind === "zcode" ? "ZCode" : null;
  if (authProvider) composerChips.push({ id: "account", node: (
    <AgentAccountMenu provider={authProvider} sessionId={session.id} state={extras.auth} busy={turnStartedAt !== undefined} />
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
      disabled={sending || clientCommandRunning || syncState !== "ready" || (!draft.trim() && attachments.length === 0)}
      onPointerDown={mobile ? (event) => event.preventDefault() : undefined}
      onClick={() => send()}
      title={
        busy
          ? `${t("chat.queueTooltip", interruptCombo)}${supportsSteering(session.kind) ? ` · ${t("chat.steerTooltip", steerCombo)}` : ""}`
          : t("session.send")
      }
    >
      <Icons.arrowRight size={mobile ? 20 : 14} />
    </button>
  );

  return (
    <TranslationProvider state={translation}><SessionLinkDirectory.Provider value={cwd}>
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
          {!mobile && <AgentVersionBadge session={session} hidden={hidden} />}
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
            {/* The same control the terminal view carries, in the same place, pointing the other way. An agent
                without a terminal interface has no other view to point to. */}
            {hasTerminalInterface(session.kind) && <ConversationViewHint enabled={!readOnly && !mobile && !isShareSurface && focused && !hidden} onSwitch={() => switchTo("tui")} />}
            <button title={labelWithCombo(t("term.splitRight"), "splitRight", shortcutOverrides)} onClick={() => paneId && onSplit(paneId, session.id, "horizontal")}>
              <Icons.splitV size={14} />
            </button>
            <button title={labelWithCombo(t("term.splitDown"), "splitDown", shortcutOverrides)} onClick={() => paneId && onSplit(paneId, session.id, "vertical")}>
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

        <ArtifactHostContext.Provider value={artifactHost}>
        <div style={{ display: "flex", flex: 1, minHeight: 0, minWidth: 0 }}>
        <div className="sv" style={{ position: "relative", flex: 1, minHeight: 0, minWidth: 0 }}>
          {searchOpen && focused && !hidden && (snapshotRef.current?.protocolVersion === 2
            ? <HistorySearch key={session.id} sessionId={session.id} onLocate={locateHistorySearch} onClose={closeSearch} />
            : <ChatSearch key={session.id} entries={turnEntries} scrollRef={scrollRef} onLocate={locateSearch} onClose={closeSearch}
              onQueryChange={setHistorySearchQuery} loadingHistory={Boolean(historySearchQuery && hasMore && !historyError)} historyError={historyError} onRetryHistory={() => void loadHistory()} />)}
          <div className="sv-scroll-wrap">
            <div
              className="sv-scroll"
              ref={scrollRef}
              onWheel={railJump.cancel}
              onTouchStart={railJump.cancel}
              onPointerDown={railJump.cancel}
              onKeyDown={event => { if (["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End"].includes(event.key)) railJump.cancel(); }}
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
                const below = el.scrollHeight - el.scrollTop - el.clientHeight;
                if (below < 80 && hasNewer && !historyError) void loadNewerHistory();
                const pinned = below <= PIN_SLACK;
                pinnedRef.current = pinned;
                setAway((prev) => (prev === !pinned ? prev : !pinned));
              }}
            >
              {syncState !== "ready" && <div className="sv-sync-state" role="status">
                {t(syncState === "loading" ? "chat.sync.loading" : "chat.sync.failed")}
                {syncState === "failed" && <button onClick={() => void refreshRef.current()}>{t("common.retry")}</button>}
              </div>}
              {hasMore && <div className="sv-history" aria-busy={historyLoading}>
                {snapshotRef.current?.protocolVersion === 2 ? <a className="vlx-btn sv-history-more" href={historyPageURL(session.id, "before", rows[0]?.id)} aria-disabled={historyLoading || syncState !== "ready"} onClick={event => {
                  if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
                  event.preventDefault();if (historyLoading || syncState !== "ready") return;
                  const url = event.currentTarget.href;
                  void loadHistory().then(page => {if (page?.protocolVersion === 2) window.history.pushState(null, "", url);});
                }}>
                  {t(historyLoading ? "common.loading" : historyError ? "common.retry" : "chat.sync.history")}
                </a> : <button className="vlx-btn sv-history-more" disabled={historyLoading || syncState !== "ready"} onClick={() => void loadHistory()}>
                  {t(historyLoading ? "common.loading" : historyError ? "common.retry" : "chat.sync.history")}
                </button>}
                {historyError && <div className="sv-history-error" role="alert">{historyError}</div>}
              </div>}
              {historyCoverage === "partial" && <div className="sv-sync-state sv-loading-hint" role="status">{t("chat.history.indexing")}</div>}
              {historyError && !hasMore && <div className="sv-sync-state" role="alert">{historyError}</div>}
              {/* An empty conversation is what a new session opens on, so it gets the middle of the view:
                  the agent's own mark, drawn large enough to say which agent this is, and one line. */}
              {syncState === "ready" && historyCoverage !== "partial" && historyCoverage !== "error" && rows.length === 0 && pendingSubmissions.length === 0 && !busy && (
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
                        data-message-id={entry.id}
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
                          onRewind={!readOnly && rows.some(row => row.id === entry.id) ? askRewind : undefined}
                          rewindDisabledReason={rewindDisabledReason}
                          rewindScopes={rewindScopes}
                          rewindRequest={rewindRequest}
                          onRewindRequestHandled={finishRewindRequest}
                          onCancelShell={readOnly ? undefined : cancelShell}
                          feedback={submissionFeedback(entry.id)}
                        />
                      </div>
                    );
                  })}
                </div>
              )}
              {mountedRows.map((entry) => (
                <div key={entry.id} data-search-id={entry.id} data-message-id={entry.id} className={"sv-item" + (searchOpen && focused && searchTarget === entry.id ? " sv-search-match" : "")}>
                  <Entry
                    entry={entry}
                    label={label}
                    icon={kindIcon}
                    cwd={cwd}
                    openRuns={openRuns}
                    onToggleRun={toggleRun}
                    onToggleFold={toggleTurn}
                    onRewind={!readOnly && rows.some(row => row.id === entry.id) ? askRewind : undefined}
                    rewindDisabledReason={rewindDisabledReason}
                    rewindScopes={rewindScopes}
                    rewindRequest={rewindRequest}
                    onRewindRequestHandled={finishRewindRequest}
                    onCancelShell={readOnly ? undefined : cancelShell}
                    feedback={submissionFeedback(entry.id)}
                  />
                </div>
              ))}
              {hasNewer && <div className="sv-history" aria-busy={historyLoading}>
                <a className="vlx-btn sv-history-more" href={historyPageURL(session.id, "after", rows.at(-1)?.id)} aria-disabled={historyLoading || syncState !== "ready"} onClick={event => {
                  if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
                  event.preventDefault();if (historyLoading || syncState !== "ready") return;
                  const url = event.currentTarget.href;
                  void loadNewerHistory().then(page => {if (page) window.history.pushState(null, "", url);});
                }}>
                  {t(historyLoading ? "common.loading" : "chat.history.newer")}
                </a>
              </div>}
              {turnStartedAt !== undefined && <WorkingRow startedAt={turnStartedAt} />}
              {extras.apiRetry && <RetryLine retry={extras.apiRetry} />}
              {/* Wrapped like every other row so it sits in the centered column. */}
              {showAgentNotice && !readOnly && <div className="sv-item">
                <AgentMissingNotice
                  key={agentMissingKey}
                  session={session}
                  onInstall={hasTerminalInterface(session.kind) ? installAgent : undefined}
                  onInstalled={afterInPlaceInstall}
                  onDismiss={() => setAgentNoticeDismissed(agentMissingKey)}
                />
              </div>}
              {/* Wrapped like every other row: a bare child of the scroller would lose the centered column,
                  because the row's own left indent replaces the auto margins that center it. */}
              {error && !isAgentNotInstalledError(error) && <div className="sv-item"><ErrorRow message={error} /></div>}
              {catalogueError && <div className="sv-item"><ErrorRow message={catalogueError} /></div>}
              {authProvider && !readOnly && <AgentAuth provider={authProvider} sessionId={session.id} state={extras.auth} busy={turnStartedAt !== undefined} />}
            </div>
            <UserMessageRail key={session.id} items={railItems} order={messageOrder} scrollRef={scrollRef} enabled={syncState === "ready" && !hidden && !(searchOpen && focused)}
              pendingId={railJump.pendingId} error={railJump.error} onCancel={railJump.cancel} onSelect={id => {
                pinnedRef.current = false;
                setAway(true);
                closeSearch();
                railJump.select(id);
              }} />
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
          {!readOnly && permissions.length > 0 && (
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
              <div className="sv-complete" ref={completionList} role="listbox">
                {wantsCatalogue && catalogueLoading && draft !== dismissed && <div className="sv-complete-item" role="status">{t("common.loading")}</div>}
                {completion?.map((c) => (
                  <button key={c.id} role="option" aria-selected={c === chosenSuggestion}
                    className={`sv-complete-item${c === chosenSuggestion ? " on" : ""}`}
                    onMouseMove={(e) => {
                      // Scrolling the highlight into view moves rows under a resting pointer; only a real move counts.
                      const last = completionPointer.current;
                      completionPointer.current = { x: e.clientX, y: e.clientY };
                      if (last && last.x === e.clientX && last.y === e.clientY) return;
                      setActiveSuggestion(completion.indexOf(c));
                    }}
                    onClick={() => complete(c)}>
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
                              disabled={steeringQueue || readingQueued!==null || queuedShellCommand(item) !== null}
                              onEdit={() => void editQueued(item)}
                              loadPart={item.textDeferred ? async (sequence,signal)=>{
                                signal.throwIfAborted();const piece=await chatQueueRead(session.id,item,sequence);signal.throwIfAborted();return piece;
                              }:undefined}
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
                      {supportsSteering(session.kind) && <button
                        className="sv-queue-steer"
                        disabled={!busy || stopping.current || steeringQueue || editing !== null}
                        onClick={() => void steerQueued(item.id)}
                      >
                        {t("chat.steer")}
                      </button>}
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
            {session.kind === "antigravity" && mode !== "bypassPermissions" && (
              <div className="sv-attach-note" role="note">{t("chat.antigravity.permissionsHint")}</div>
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
              {/* The top edge resizes the text box; a double-click returns it to the default height. */}
              {!mobile && <div
                className="sv-box-resize"
                title={t("splitter.dragToResize")}
                onMouseDown={(e) => startComposerResize(e, inputRef.current)}
                onDoubleClick={() => setComposerHeight(null)}
              />}
              <textarea
                ref={inputRef}
                onPaste={onPaste}
                disabled={clientCommandRunning}
                value={draft}
                rows={1}
                style={!mobile && composerHeight !== null ? { height: composerHeight, maxHeight: "70vh" } : undefined}
                placeholder={
                  busy
                    ? t("chat.placeholderBusy")
                    : session.kind === "opencode"
                      ? t("chat.placeholderOpencode")
                      : session.kind === "antigravity"
                        ? t("chat.antigravity.placeholder")
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
                    // An open list takes the arrows; input history waits until it is closed.
                    if (completion && chosenSuggestion) {
                      e.preventDefault();
                      e.stopPropagation();
                      const at = completion.indexOf(chosenSuggestion);
                      const step = e.key === "ArrowUp" ? -1 : 1;
                      setActiveSuggestion((at + step + completion.length) % completion.length);
                      return;
                    }
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
                  if (e.key === "Tab" && chosenSuggestion) {
                    e.preventDefault();
                    complete(chosenSuggestion);
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
                    } else if (chosenSuggestion) complete(chosenSuggestion);
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
              {syncInterrupted && syncState !== "ready" && (
                <div className="sv-sync-notice" role="status" aria-live="polite">
                  <span>{t(syncState === "loading" ? "chat.sync.loading" : "chat.sync.failed")}</span>
                  {syncState === "failed" && <button className="vlx-btn" onClick={() => void refreshRef.current()}>{t("common.retry")}</button>}
                </div>
              )}
              {(busy || actionFeedback) && (
                <div className="sv-action-feedback" role="status" aria-live="polite">
                  {actionFeedback && <span>{actionFeedback}</span>}
                  {busy && <span>{t("chat.interruptTooltip")}{supportsSteering(session.kind) && <> · {t("chat.steerTooltip", steerCombo)}</>}</span>}
                </div>
              )}
              <ComposerToolbar mobile={mobile} chips={composerChips} inline={["translation", ...composerInlineChips]}
                actions={<>
                  {(session.kind === "claude" || session.kind === "codex" || session.kind === "deepseek" || session.kind === "zcode") && <UsageMeter extras={extras} />}
                  {busy && supportsSteering(session.kind) && (
                    <button
                      className="sv-steer"
                      disabled={sending || clientCommandRunning || syncState !== "ready" || (!draft.trim() && attachments.length === 0)}
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
        {dockPath && artifactHost.openBeside && (
          <>
            <Splitter
              onDrag={(dx) =>
                setDockWidth((w) => {
                  const next = Math.min(DOCK_MAX_W, Math.max(DOCK_MIN_W, w - dx));
                  try { localStorage.setItem(DOCK_WIDTH_KEY, String(Math.round(next))); } catch { /* Keep it for this session. */ }
                  return next;
                })
              }
            />
            <ArtifactDock path={dockPath} onClose={() => setDockPath(null)} style={{ width: dockWidth, maxWidth: "70%" }} />
          </>
        )}
        </div>
        </ArtifactHostContext.Provider>
      </div>
      {engineConfirm}
      {permissionRestart.dialog}
    </div>
    </SessionLinkDirectory.Provider></TranslationProvider>
  );
}

/** How a queued shell-mode context message reads in the queue: the command as it was typed. */
function queuedShellCommand(item: QueuedMessage): string | null {
  return item.shellCommand === undefined ? null : `! ${item.shellCommand}`;
}

