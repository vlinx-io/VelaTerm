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

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { measureElement, useVirtualizer } from "@tanstack/react-virtual";

import Icons from "../../../components/Icons";
import { StatusIndicator } from "../../../components/StatusIndicator";
import { useT, type I18nKey } from "../../../i18n";
import {
  chatInterrupt,
  chatModels,
  chatPermission,
  chatQueueRemove,
  chatQueueSteer,
  chatQueueUpdate,
  chatRewind,
  chatRewindPreview,
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
  chatDetach,
  chatSnapshot,
  chatCommands,
  chatStart,
  onChatEvent,
  type ChatEvent,
  type ChatCommand,
  type ChatExtras,
  type ChatCollaborationMode,
  type ChatConfigKey,
  type ChatModel,
  type ChatPermission,
  type PendingPermissionMode,
  type ChatRewindPreview,
  type ChatRewindScope,
  type ChatRow,
  type QueuedMessage,
  type SendBehavior,
} from "../../../ipc/chat";
import { attachImages, MAX_IMAGE_BYTES, MAX_IMAGES, type Attachment } from "./attachments";
import { attachChatInputGuard } from "./inputGuard";
import { buildSuggestions, findFileMention, mentionDir, type Suggestion } from "./completion";
import { env } from "../../../platform/env";
import { imageFromNativeClipboard, imagesFromClipboard, imagesFromDrop } from "../../../terminal/imageInput";
import { IS_MAC, IS_PLAIN_BROWSER } from "../../../hooks/shortcutRegistry";
import { useMentionFiles } from "./fileMentions";
import { ControlChip, LevelBar, type ChipOption } from "./controls";
import { FastModeChip, McpChip, NotificationBar, RetryLine, TasksChip, UsageMeter } from "./extras";
import { useEngineSwitch } from "./engineSwitch";
import { PermissionCard, type PermissionAnswer } from "./permissionCards";
import { isMode, modesFor, type Mode } from "./permissions";
import { useTermStore } from "../../../store/termStore";
import { effectiveStatus, type Session } from "../../../types";
import { kindIconEl } from "../../sessionViewers/sessionMeta";
import { assistantLabel } from "../../sessionViewers/TranscriptViewer";
import {
  CompactionRow,
  ChatImageView,
  CommandRow,
  ErrorRow,
  MessageBubble,
  NoticeRow,
  ReasoningRow,
  ToolCard,
  ToolRunCard,
  WorkingRow,
} from "./rows";
import {
  estimateRowHeight,
  groupToolRuns,
  mountedStart,
  type DisplayRow,
} from "./toolRuns";
import "./session-view.css";
import { onTransportReconnect } from "../../../ipc/transport";
import { useOutbox, emptySubmissions, acknowledgeSubmissions, createSubmission, deliverSubmission, retrySubmission, submissionsFor, ChatVersions } from "./outbox";
import { cachedChat, cacheChat, mergeRows, reconcileChat, chatSyncMetrics } from "./chatCache";
import { ChatSearch } from "./ChatSearch";
import { SessionLinkDirectory } from "./links";

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
  onActivate: (paneId: string, id: string) => void;
  onSplit: (paneId: string, id: string, dir: "horizontal" | "vertical") => void;
  onClose: (paneId: string, id: string) => void;
}) {
  const t = useT();
  const paneStyle = useTermStore((s) => s.paneStyle);
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
  const historyBusy = useRef(false);
  const historyGeneration = useRef(0);
  const refreshRef = useRef<() => Promise<void>>(async () => {});
  const prependScroll = useRef<{ height: number; top: number } | null>(null);
  const [submissionReceipts, setSubmissionReceipts] = useState<boolean | null>(null);
  const pendingSubmissions = useOutbox(state => state.sessions[session.id] ?? emptySubmissions);
  // Messages typed while the agent was busy. They belong to the backend, not to this pane: a second view
  // of the same session has to show the same queue, and the queue has to outlive this pane being closed.
  const [queue, setQueue] = useState<QueuedMessage[]>(() => cachedChat(session.id)?.queue ?? []);
  const [steeringQueue, setSteeringQueue] = useState(false);
  const steeringQueueRef = useRef(false);
  /** The queued message being rewritten in place, and the text so far. */
  const [editing, setEditing] = useState<{ id: string; text: string } | null>(null);
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
  const [mode, setMode] = useState<Mode>(() => storedMode(session));
  const [pendingPermissionMode, setPendingPermissionMode] = useState<PendingPermissionMode | null>(null);
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
  /** Bumped when the agent reports its own catalogue, so the model list is read again. */
  const [catalogueVersion, setCatalogueVersion] = useState(0);
  /** Backend-owned start of the logical turn currently in flight. */
  const [actionFeedback, setActionFeedback] = useState("");
  const [sending, setSending] = useState(false);
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
  const [draft, setDraft] = useState("");
  // Caret position in the draft. An `@` mention is read from where the caret is, not from the end of the
  // text, so that a path can be completed in the middle of a sentence that is already written.
  const [caret, setCaret] = useState(0);
  // The draft the completion list was dismissed for. Escape sets it; typing anything else changes the
  // draft and brings the list back, which is what someone who dismissed it by accident expects.
  const [dismissed, setDismissed] = useState<string | null>(null);
  /** Folded tool runs the reader has opened, by the id of the run's first call. */
  const [openRuns, setOpenRuns] = useState<ReadonlySet<string>>(() => new Set());
  /** Whether the view is parked away from the end, which is the only time the "back to the end" button is worth showing. */
  const [away, setAway] = useState(false);
  const [pendingRewind, setPendingRewind] = useState<{
    rowId: string;
    scope: ChatRewindScope;
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
          if (!versions.accept("rows", event.epoch === undefined ? undefined : 0, event.epoch)) break;
          versions.queue = 0;
          setRows([]);
          setQueue([]);
          snapshotRef.current = undefined;
          setHasMore(false);
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
          if (event.mode && isMode(event.mode)) setMode(event.mode);
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
          setError(event.message);
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
            : { fastMode: useTermStore.getState().chatFastModeByKind[session.kind] ?? false },
        );
        setTurnStartedAt(snapshot.running ? snapshot.turnStartedAt : undefined);
        if (snapshot.effort && isEffort(snapshot.effort)) setEffort(snapshot.effort);
        if (snapshot.mode && isMode(snapshot.mode)) setMode(snapshot.mode);
        setPendingPermissionMode(snapshot.pendingPermissionMode ?? null);
        setCollaborationModes(snapshot.collaborationModes ?? []);
        if (isCollaborationMode(snapshot.collaborationMode)) {
          setCollaborationMode(snapshot.collaborationMode);
        }
        if (session.kind === "codex") {
          setServiceTier(snapshot.serviceTier ?? "");
          setPersonality(snapshot.personality ?? "");
        }
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
      snapshotRef.current = { ...snapshotRef.current, rows, queue, hasMore, rowsRevision: liveVersions.current.rows, queueRevision: liveVersions.current.queue };
      cacheChat(session.id, snapshotRef.current);
    }
  }, [session.id, rows, queue, hasMore]);

  const loadHistory = async () => {
    if (historyBusy.current || !hasMore || syncState !== "ready" || !rows[0]) return;
    historyBusy.current = true;
    setHistoryLoading(true);
    const generation = historyGeneration.current;
    try {
      const page = await chatSnapshot(session.id, { before: rows[0].id, epoch: snapshotRef.current?.startedAt });
      if (generation !== historyGeneration.current) return;
      if (page.pageKind !== "history") { await refreshRef.current(); return; }
      const scroll = scrollRef.current;
      if (scroll) prependScroll.current = { height: scroll.scrollHeight, top: scroll.scrollTop };
      setRows(current => mergeRows(page.rows, current));
      setHasMore(page.hasMore ?? false);
    } catch (error) { if (generation === historyGeneration.current) setError(String(error)); }
    finally { historyBusy.current = false; setHistoryLoading(false); }
  };
  useLayoutEffect(() => {
    const previous = prependScroll.current;
    const scroll = scrollRef.current;
    if (previous && scroll) {
      scroll.scrollTop = previous.top + scroll.scrollHeight - previous.height;
      prependScroll.current = null;
    }
  }, [rows]);
  // Searching requests the remaining history explicitly; ordinary navigation stays bounded.
  useEffect(() => { if (searchOpen && hasMore) void loadHistory(); }, [searchOpen, hasMore, rows.length, syncState]);

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
  // so a turn that read a dozen files does not bury the answer that came out of it.
  //
  // Reasoning rows with no text are dropped here. The agent may report that it thought without disclosing
  // what it thought: the block arrives carrying only a signature, and its text stays empty for the whole
  // turn. A heading that opens onto nothing is worse than no heading, so such a row is not drawn at all.
  const display = useMemo(
    () => groupToolRuns(rows.filter((r) => r.kind !== "reasoning" || r.text.trim() !== "")),
    [rows],
  );
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
    overscan: OVERSCAN,
  });

  const locateSearch = useCallback((index: number) => {
    const entry = display[index];
    setSearchTarget(entry?.id ?? null);
    if (!entry) return;
    pinnedRef.current = false;
    setAway(true);
    if (entry.kind === "run") setOpenRuns(previous => previous.has(entry.id) ? previous : new Set([...previous, entry.id]));
    if (index < split) virtualizer.scrollToIndex(index, { align: "center" });
    else scrollRef.current?.querySelectorAll<HTMLElement>(".sv-item[data-search-id]").forEach(element => {
      const root = scrollRef.current;
      if (root && element.dataset.searchId === entry.id) root.scrollTop += element.getBoundingClientRect().top - root.getBoundingClientRect().top - root.clientHeight / 2;
    });
  }, [display, split, virtualizer]);

  // Measuring a row above the viewport moves everything below it. While the reader is parked mid-history
  // that would drag the text they are reading, so the scroll position is corrected to absorb the change;
  // while they are at the end there is nothing to protect, and correcting would fight the follow below.
  useEffect(() => {
    virtualizer.shouldAdjustScrollPositionOnItemSizeChange = (item, _delta, instance) => {
      if (pinnedRef.current) return false;
      return item.start < (instance.scrollOffset ?? 0);
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

  // Follow after every committed layout, including composer and virtual-row size changes.
  // Observe the rows as well: images and rendered Markdown can grow without a pane render.
  useLayoutEffect(() => {
    if (hidden) return;
    const el = scrollRef.current;
    if (!el) return;
    const follow = () => {
      if (pinnedRef.current) el.scrollTop = el.scrollHeight;
      scrollGeometry.current = { height: el.scrollHeight, viewport: el.clientHeight };
    };
    follow();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(follow);
    observer.observe(el);
    for (const child of el.children) observer.observe(child);
    return () => observer.disconnect();
  });

  const busy = status === "working";
  const canRewind =
    engineRunning &&
    !busy &&
    !rewinding &&
    queue.length === 0 &&
    permissions.length === 0;
  const finishRewindRequest = useCallback((token: number) => {
    setRewindRequest((request) => request?.token === token ? null : request);
  }, []);

  const askRewind = (rowId: string, scope: ChatRewindScope) => {
    setError(null);
    if (scope === "conversation") {
      setPendingRewind({ rowId, scope, loading: false });
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
  };

  const doRewind = () => {
    const pending = pendingRewind;
    if (!pending || pending.loading || rewinding || pending.preview?.canRewind === false) return;
    setRewinding(true);
    setError(null);
    void chatRewind(session.id, pending.rowId, pending.scope)
      .then((result) => {
        if (pending.scope !== "files") {
          setRows((current) => {
            const index = current.findIndex((row) => row.id === pending.rowId);
            return index < 0 ? current : current.slice(0, index);
          });
          if (result.prefillText && !draft.trim()) {
            setDraft(result.prefillText);
            setCaret(result.prefillText.length);
            placeCaret.current = result.prefillText.length;
          }
        }
        setPendingRewind(null);
      })
      .catch((err) => setError(String(err)))
      .finally(() => setRewinding(false));
  };

  /** Start the agent; the backend restores this conversation's saved Codex settings. */
  const startAgent = async () => {
    await chatStart(session.id, model, effort || undefined, extras.fastMode === true);
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
    if (sending) return;
    const text = draft.trim();
    // A picture on its own is a message: dropping a screenshot in and pressing send says enough.
    if (!text && attachments.length === 0) return;
    // Paseo treats these as client commands: they change the surrounding session UI and must never be
    // forwarded to Claude or Codex as user prose. Arguments or attachments deliberately opt out.
    const localCommand =
      behavior !== "steer" && attachments.length === 0 && /^\/(clear|new|rewind)$/.exec(text)?.[1];
    // Codex answers these through requests of its own rather than as a message: a summary of the
    // conversation, or a code review run as a turn. Both need the native thread open first.
    const codexCommand =
      behavior !== "steer" && session.kind === "codex" && attachments.length === 0
        ? /^\/(compact|review)(?:\s+([\s\S]*))?$/.exec(text)
        : null;
    if (codexCommand) {
      const [, name, args] = codexCommand;
      setError(null);
      setDraft("");
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
      setDraft("");
      setCaret(0);
      setDismissed(null);
      rewindRequestToken.current += 1;
      setRewindRequest({ rowId: latestUser.id, token: rewindRequestToken.current });
      const displayIndex = display.findIndex(
        (entry) => entry.kind === "row" && entry.row.id === latestUser.id,
      );
      if (displayIndex >= 0 && displayIndex < virtualRows.length) {
        pinnedRef.current = false;
        virtualizer.scrollToIndex(displayIndex, { align: "center" });
      }
      return;
    }
    if (localCommand === "clear" || localCommand === "new") {
      if (clientCommandRunning) return;
      const previousDraft = draft;
      setClientCommandRunning(true);
      setError(null);
      setDraft("");
      setCaret(0);
      setDismissed(null);
      void useTermStore
        .getState()
        .clearChatSession(session.id, model, effort || undefined)
        .catch((err) => {
          setError(String(err));
          setDraft(previousDraft);
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
    setDraft("");
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
    void chatQueueRemove(session.id, id).catch((err) => setError(String(err)));
  };

  const commitEdit = () => {
    if (!editing) return;
    const { id, text } = editing;
    setEditing(null);
    const trimmed = text.trim();
    // Emptying a queued message is how it is thrown away.
    if (!trimmed) {
      removeQueued(id);
      return;
    }
    setQueue((prev) => prev.map((item) => (item.id === id ? { ...item, text: trimmed } : item)));
    void chatQueueUpdate(session.id, id, trimmed).catch((err) => setError(String(err)));
  };

  const applyMode = async (next: Mode) => {
    await chatSetMode(session.id, next);
    setMode(next);
  };
  const pickMode = (next: Mode, keep: boolean) => {
    void applyMode(next).then(() => {
      if (keep && session.kind === "codex") {
        useTermStore.getState().setAgentDefault("codex", {
          permissionMode: next === "full-access" ? "skip" : next,
        });
      }
    }).catch((err) => setError(String(err)));
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
      { name: "rewind", description: t("chat.command.rewindDescription") },
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
    setDraft(suggestion.insert);
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
  /** What a keypress in the composer asks for: interrupt with the primary modifier, steer with Alt. */
  const behaviorOf = (e: React.KeyboardEvent): SendBehavior =>
    e.metaKey || e.ctrlKey ? "interrupt" : e.altKey ? "steer" : "queue";

  const label = assistantLabel(session.kind);
  // The agent's own mark, drawn in the margin beside each of its answers.
  const kindIcon = kindIconEl(session.kind, 12);

  const modelOptions: ChipOption<string>[] = [
    // The default names no model, so it carries the settings mark, in the accent colour, rather than the
    // agent's.
    {
      value: "",
      label: t("chat.modelDefault"),
      glyph: (
        <span style={{ color: "var(--accent)", display: "inline-flex" }}>
          <Icons.sliders size={14} />
        </span>
      ),
    },
    // The catalogue's description opens with the model's own name, because it is also used where the name
    // is not already on screen. Here it is, one line above, so the menu drops the repeat.
    // Every model row carries the agent's own mark in its brand colour, the same mark the chip shows.
    ...catalogue.map((m) => ({
      value: m.id,
      label: m.label,
      hint: withoutNamePrefix(m.description, m.label),
      glyph: kindIconEl(session.kind, 14),
    })),
  ];
  // The chip reads the selected model's own name, but a model set outside this menu — inherited from the
  // session's launch arguments, or reported by the agent — may not be in the list, so fall back to the id.
  const selected = catalogue.find((m) => m.id === model);
  const modelLabel = selected?.label ?? model ?? t("chat.modelDefault");
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
  const modeOptions: ChipOption<Mode>[] = modesFor(session.kind).map((value) => ({
    value,
    label: t(modeLabelKey(value)),
  }));
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

  const submissionFeedback = (id: string) => {
    const item = pendingSubmissions.find(value => value.id === id);
    if (!item) return null;
    return <div className="sv-submission-status" role="status">
      {t(`chat.submission.${item.status}`)}
      {(item.status === "failed" || item.status === "unknown") && <button disabled={submissionReceipts !== true} onClick={() => void retrySubmission(session.id, item, item.status === "failed" && !engineRunning ? startAgent : undefined)}>
        {t(item.status === "unknown" ? "chat.submission.check" : "common.retry")}
      </button>}
      {item.error && <ErrorRow message={item.error} />}
    </div>;
  };

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
        if (e.currentTarget.querySelector("[role=option]")) return;
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
          <span className="session-experimental-badge" title={t("session.showConversation")}>{t("common.experimental")}</span>
          <StatusIndicator status={status} unread={unread} />
          <span className="pane-tools">
            <button title={t("term.searchMenu")} onClick={() => useTermStore.getState().openSearch()}>
              <Icons.search size={14} />
            </button>
            {/* The same control the terminal view carries, in the same place, pointing the other way. */}
            <button title={t("session.showTerminal")} onClick={() => switchTo("tui")}>
              <Icons.terminal size={14} />
            </button>
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
          {searchOpen && focused && !hidden && <ChatSearch key={session.id} entries={display} scrollRef={scrollRef} onLocate={locateSearch} onClose={closeSearch} />}
          <div className="sv-scroll-wrap">
            <div
              className="sv-scroll"
              ref={scrollRef}
              onScroll={(e) => {
                const el = e.currentTarget;
                // Browser anchoring and viewport resizing also emit scroll events. A layout
                // change must not turn off following before the resize observer catches up.
                const geometry = scrollGeometry.current;
                if (pinnedRef.current && (geometry.height !== el.scrollHeight || geometry.viewport !== el.clientHeight)) {
                  el.scrollTop = el.scrollHeight;
                  scrollGeometry.current = { height: el.scrollHeight, viewport: el.clientHeight };
                  return;
                }
                if (el.scrollTop < 80) void loadHistory();
                const pinned = el.scrollHeight - el.scrollTop - el.clientHeight <= PIN_SLACK;
                pinnedRef.current = pinned;
                setAway((prev) => (prev === !pinned ? prev : !pinned));
              }}
            >
              {syncState !== "ready" && <div className="sv-sync-state" role="status">
                {t(syncState === "loading" ? "chat.sync.loading" : "chat.sync.failed")}
                {syncState === "failed" && <button onClick={() => void refreshRef.current()}>{t("common.retry")}</button>}
              </div>}
              {hasMore && <button className="sv-history-more" disabled={historyLoading || syncState !== "ready"} onClick={() => void loadHistory()}>
                {t(historyLoading ? "common.loading" : "chat.sync.history")}
              </button>}
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
                          onRewind={canRewind ? askRewind : undefined}
                          rewindScopes={rewindScopes}
                          rewindRequest={rewindRequest}
                          onRewindRequestHandled={finishRewindRequest}
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
                    onRewind={canRewind ? askRewind : undefined}
                    rewindScopes={rewindScopes}
                    rewindRequest={rewindRequest}
                    onRewindRequestHandled={finishRewindRequest}
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
              {error && <ErrorRow message={error} />}
              {catalogueError && <ErrorRow message={catalogueError} />}
            </div>
            {away && (
              <button className="sv-to-end" onClick={toEnd} title={t("chat.backToEnd")}>
                <Icons.chevD size={14} />
              </button>
            )}
          </div>

          {notice && (
            <NotificationBar text={notice.text} priority={notice.priority} onClose={() => setNotice(null)} />
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
                <div className="sv-rewind-confirm-detail">
                  {pendingRewind.loading
                    ? t("chat.rewind.previewing")
                    : pendingRewind.preview?.canRewind === false
                      ? pendingRewind.preview.error || t("chat.rewind.unavailable")
                      : pendingRewind.scope === "conversation"
                        ? t("chat.rewind.warning")
                        : t(
                            "chat.rewind.fileSummary",
                            pendingRewind.preview?.filesChanged?.length ?? 0,
                            pendingRewind.preview?.insertions ?? 0,
                            pendingRewind.preview?.deletions ?? 0,
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
                    disabled={pendingRewind.loading || rewinding || pendingRewind.preview?.canRewind === false}
                    onClick={doRewind}
                  >
                    {rewinding ? t("chat.rewind.applying") : t("chat.rewind.apply")}
                  </button>
                </div>
              </div>
            </div>
          ) : null}

          {/* The whole composer is the drop target, not just the text box: aiming at a one-line input to
              attach a picture is a needlessly small target. */}
          <div
            className="sv-composer"
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
                {queue.map((item) => (
                  <div key={item.id} className="sv-queue-item">
                    {editing?.id === item.id ? (
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
                      <button
                        className="sv-queue-text"
                        disabled={steeringQueue}
                        title={t("chat.queue.edit")}
                        onClick={() => setEditing({ id: item.id, text: item.text })}
                      >
                        {item.text}
                      </button>
                    )}
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
                  setDraft(e.target.value);
                  setCaret(e.target.selectionStart ?? e.target.value.length);
                }}
                onKeyUp={syncCaret}
                onClick={syncCaret}
                onSelect={syncCaret}
                onKeyDown={(e) => {
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
              {/* Model, effort, collaboration style, and permission mode sit under the input, where they belong to the message
                  about to be sent rather than to the pane. */}
              {(busy || actionFeedback) && (
                <div className="sv-action-feedback" role="status" aria-live="polite">
                  {actionFeedback && <span>{actionFeedback}</span>}
                  {busy && <span>{t("chat.interruptTooltip")} · {t("chat.steerTooltip", steerCombo)}</span>}
                </div>
              )}
              <div className="sv-controls">
                {catalogue.length > 0 && (
                  <ControlChip
                    glyph={kindIconEl(session.kind, 14)}
                    label={modelLabel}
                    title={t("chat.modelTooltip")}
                    value={model ?? ""}
                    options={modelOptions}
                    defaultValue={defaultModel}
                    onPick={pickModel}
                    keepLabel={t("chat.keepChoice")}
                    menuWidth={300}
                    filterPlaceholder={t("chat.filterPlaceholder")}
                  />
                )}
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
                {collaborationModeOptions.length > 0 && (
                  <ControlChip
                    glyph={session.kind === "opencode" ? <Icons.bot size={14} /> : <Icons.compass size={14} />}
                    label={collaborationLabel(session.kind, selectedCollaboration, collaborationMode, t)}
                    title={t(session.kind === "opencode" ? "chat.agentTooltip" : "chat.collaborationModeTooltip")}
                    value={collaborationMode}
                    options={collaborationModeOptions}
                    onPick={pickCollaborationMode}
                    menuWidth={260}
                  />
                )}
                <div className="sv-permission-control">
                  <ControlChip
                    glyph={<Icons.lock size={14} />}
                    label={t(modeLabelKey(mode))}
                    title={t("chat.modeTooltip")}
                    value={mode}
                    options={modeOptions}
                    defaultValue={defaultMode}
                    onPick={pickMode}
                    keepLabel={session.kind === "codex" ? t("chat.keepChoice") : undefined}
                    menuWidth={240}
                  />
                  {pendingPermissionMode && (
                    <span
                      className="sv-mode-pending"
                      role="status"
                      title={t(
                        "chat.modePendingHint",
                        isMode(pendingPermissionMode.current) ? t(modeLabelKey(pendingPermissionMode.current)) : pendingPermissionMode.current,
                        isMode(pendingPermissionMode.next) ? t(modeLabelKey(pendingPermissionMode.next)) : pendingPermissionMode.next,
                      )}
                    >
                      {t("chat.modeNextTurn")}
                    </span>
                  )}
                </div>
                {fastModeOffered && (
                  <FastModeChip enabled={extras.fastMode === true} onToggle={pickFastMode} />
                )}
                {serviceTierOptions.length > 1 && (
                  <ControlChip
                    glyph={<Icons.clock size={14} />}
                    label={serviceTierOptions.find((o) => o.value === serviceTier)?.label ?? t("chat.serviceTier.default")}
                    title={t("chat.serviceTierTooltip")}
                    value={serviceTier || CODEX_STANDARD_TIER}
                    options={serviceTierOptions}
                    onPick={pickServiceTier}
                    menuWidth={240}
                  />
                )}
                {personalityOffered && (
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
                )}
                {(session.kind === "claude" || session.kind === "codex") && engineRunning && <McpChip sessionId={session.id} codex={session.kind === "codex"} />}
                {session.kind === "claude" && engineRunning && (
                  <TasksChip
                    tasks={extras.backgroundTasks ?? []}
                    busy={busy}
                    onStop={(taskId) => void chatStopTask(session.id, taskId).catch((err) => setError(String(err)))}
                    onOpen={(task) => openTaskTab(session.id, task)}
                    onBackgroundAll={() => void chatBackgroundTasks(session.id).catch((err) => setError(String(err)))}
                  />
                )}
                <span className="sv-controls-gap" />
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
                {/* What is typed decides the button, not what the agent is doing: with text in the box the
                    action is always "send" — queued while a turn runs — and only an empty box during a
                    turn turns it into the stop button. */}
                {busy && !clientCommandRunning && !draft.trim() && attachments.length === 0 ? (
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
                    onClick={() => send()}
                    title={
                      busy
                        ? `${t("chat.queueTooltip", interruptCombo)} · ${t("chat.steerTooltip", steerCombo)}`
                        : t("session.send")
                    }
                  >
                    <Icons.arrowRight size={14} />
                  </button>
                )}
              </div>
            </div>
          </div>
        </div>
      </div>
      {engineConfirm}
    </div>
    </SessionLinkDirectory.Provider>
  );
}

/** One entry of the list: a single row, or a folded run of tool calls. */
function Entry({
  entry,
  label,
  icon,
  cwd,
  openRuns,
  onToggleRun,
  onRewind,
  rewindScopes,
  rewindRequest,
  onRewindRequestHandled,
}: {
  entry: DisplayRow;
  label: string;
  icon: ReactNode;
  cwd?: string;
  openRuns: ReadonlySet<string>;
  onToggleRun: (id: string) => void;
  onRewind?: (rowId: string, scope: ChatRewindScope) => void;
  rewindScopes: ChatRewindScope[];
  rewindRequest: { rowId: string; token: number } | null;
  onRewindRequestHandled: (token: number) => void;
}) {
  if (entry.kind === "run") {
    return (
      <ToolRunCard
        calls={entry.calls}
        renderCall={row => <Row row={row} label={label} icon={icon} cwd={cwd} />}
        running={entry.running}
        open={openRuns.has(entry.id)}
        onToggle={() => onToggleRun(entry.id)}
        cwd={cwd}
      />
    );
  }
  return (
    <Row
      row={entry.row}
      label={label}
      icon={icon}
      cwd={cwd}
      onRewind={onRewind}
      rewindScopes={rewindScopes}
      rewindRequest={rewindRequest}
      onRewindRequestHandled={onRewindRequestHandled}
    />
  );
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
  onRewind,
  rewindScopes,
  rewindRequest,
  onRewindRequestHandled,
}: {
  row: ChatRow;
  label: string;
  icon: ReactNode;
  cwd?: string;
  onRewind?: (rowId: string, scope: ChatRewindScope) => void;
  rewindScopes?: ChatRewindScope[];
  rewindRequest?: { rowId: string; token: number } | null;
  onRewindRequestHandled?: (token: number) => void;
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
          who={t("archive.you")}
          isUser
          text={row.text}
          images={row.images}
          at={row.at}
          onRewind={onRewind ? (scope) => onRewind(row.id, scope) : undefined}
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
        />
      );
  }
}
