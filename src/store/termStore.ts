//! Global Zustand state: SQLite-backed tree data, session runtime state, and UI state.
//! The center area uses tabs containing recursively splittable pane trees.

import { create } from "zustand";
import { t } from "../i18n";
import { setBrowserUrl } from "../ipc/browser";
import {
  createWorktree,
  getSessionCwd,
  ptyKill,
  ptyWrite,
  spawnResult,
  type ShellOption,
} from "../ipc/commands";
import { pushSetting } from "../ipc/settingsSync";
import { isTauri } from "../ipc/transport";
import { env } from "../platform";
import { genId } from "../genId";
import type { SpawnRequest, StatusSignal } from "../ipc/events";
import { notify } from "../notify";
import type { ScreenDetection } from "../terminal/screenDetect";
import * as tree from "../ipc/tree";
import { platform } from "../platform";
import {
  collectSessionIds,
  findBySession,
  firstLeaf,
  makeLeaf,
  type PaneNode,
  removeLeaf,
  removeSession,
  setSizes,
  splitAt,
} from "../layout/CenterPane/paneTree";
import {
  collectSidebarViewIds,
  firstSidebarViewId,
  makeSidebarTreeTab,
  migrateLegacySidebarTabs,
  removeSidebarView,
  setSidebarSplitSizes,
  splitSidebarView,
  type SidebarSplitDirection,
  type SidebarTreeTab,
  type SidebarViewPaneNode,
} from "../layout/LeftSidebar/sidebarTreeLayout";
import {
  applyTheme,
  applyVisual,
  loadTheme,
  resolveTheme,
  type AccentChoice,
  type Density,
  type DividerStyle,
  type InspectorTab,
  type NavLayout,
  type PaneStyle,
  type ResolvedTheme,
  type Theme,
} from "../theme";
import { liveTerminalIds } from "../terminal/registry";
import { checkTabInvariants, DEBUG } from "../debug";
import type {
  AgentState,
  Group,
  NodeKind,
  Project,
  Session,
  SessionId,
  SessionRuntime,
} from "../types";
import { effectiveStatus, matchesAgentState } from "../types";
import {
  CLEAN_IMAGES_KEY,
  NOTIFY_KEY,
  RECORD_SESSIONS_KEY,
  SOUND_KEY,
  loadCleanPastedImages,
  loadNotifyEnabled,
  loadRecordSessions,
  loadSettings,
  loadSoundEnabled,
  saveSettings,
  visualOf,
  type AgentDefaultConfig,
  type ImagePasteMode,
  type OrchestrationLimits,
  type OrchestrationProfile,
  type PersistedSettings,
  type TermRenderer,
} from "./settings";
import { docKindOf, makeDocTab, type DocTab } from "./docTab";

// Re-export the public API after moving implementations to settings/docTab, preserving existing import paths.
export { DEFAULT_MAX_LIVE_TABS } from "./settings";
export type {
  AgentDefaultConfig,
  ImagePasteMode,
  OrchestrationLimits,
  OrchestrationProfile,
  TermRenderer,
} from "./settings";
export { docKindOf } from "./docTab";
export type { DocKind, DocTab } from "./docTab";

const LEFT_MIN = 180;
const LEFT_MAX = 480;
const RIGHT_MIN = 220;
const RIGHT_MAX = 520;

/**
 * Configurable limit for background live tabs. Without a cap, visiting every session in single-tab mode
 * leaves every xterm, PTY, and agent resident. On overflow, evict the oldest inactive tab and show a status
 * notice. If all tabs are active, ask the user instead. Removing the pane tree unmounts TerminalView and
 * kills or detaches the process. `liveTabs` order records when tabs entered the background.
 */

/**
 * Selects the oldest inactive background tab for eviction. A tab is active if any session is working,
 * asking, waiting, or has an unread notification. Return `null` when all are active so the UI can ask first.
 */
function pickEvictTab(
  liveTabs: string[],
  paneTrees: Record<string, PaneNode>,
  runtimes: Record<string, SessionRuntime>,
  notifications: Record<string, number>,
): string | null {
  const isActive = (tabId: string) => {
    const pt = paneTrees[tabId];
    if (!pt) return false;
    return collectSessionIds(pt).some((sid) => {
      const st = effectiveStatus(runtimes[sid]);
      return st === "working" || st === "asking" || st === "waiting" || sid in notifications;
    });
  };
  return liveTabs.find((tid) => !isActive(tid)) ?? null;
}


/** Local-storage key for frontend-only tab, split, and activation layout. */
const LAYOUT_KEY = "vlx-layout";

/** Local-storage key for desktop sidebar tree views ("avatars"). */
const SIDEBAR_VIEWS_KEY = "vlx-sidebar-tree-views";

/** One saved projection of the shared project tree. Node data is shared; only view conditions are isolated. */
export interface SidebarTreeView {
  id: string;
  name: string;
  treeFilter: string;
  statusFilter: AgentState[] | null;
  /**
   * Stable membership captured when the status condition changes. It is runtime-only because live session
   * statuses do not survive an application restart.
   */
  statusFilterIds: Record<string, true> | null;
  markFilter: string | null;
  /**
   * Per-view collapse state keyed by node ID. `null` means the view follows the shared tree state stored in the
   * database, which is what the primary view does. Split-off views get their own map so expanding or collapsing a
   * node in one pane never moves the other pane.
   */
  collapsedOverrides: Record<string, boolean> | null;
}

/** Every view condition survives a restart, including the status snapshot and the per-view collapse map. */
type PersistedSidebarTreeView = SidebarTreeView;

interface PersistedSidebarViewsV1 {
  version: 1;
  views: PersistedSidebarTreeView[];
  primaryId: string;
  activeId: string;
  layout: "tabs" | "stack";
}

interface PersistedSidebarViewsV2 {
  version: 2;
  views: PersistedSidebarTreeView[];
  tabs: SidebarTreeTab[];
  primaryId: string;
  activeId: string;
}

const MAIN_TREE_VIEW_ID = "main";

function defaultSidebarViews(): {
  views: SidebarTreeView[];
  tabs: SidebarTreeTab[];
  primaryId: string;
  activeId: string;
} {
  const view: SidebarTreeView = {
    id: MAIN_TREE_VIEW_ID,
    name: t("tree.viewMainName"),
    treeFilter: "",
    statusFilter: null,
    statusFilterIds: null,
    markFilter: null,
    collapsedOverrides: null,
  };
  return {
    views: [view],
    tabs: [makeSidebarTreeTab(view.id)],
    primaryId: MAIN_TREE_VIEW_ID,
    activeId: MAIN_TREE_VIEW_ID,
  };
}

/** Upper bound on persisted per-view ID maps so a corrupted payload cannot grow without limit. */
const SIDEBAR_VIEW_MAP_LIMIT = 20000;

const AGENT_STATES: AgentState[] = ["working", "asking", "waiting"];

function loadStatusFilter(candidate: unknown): AgentState[] | null {
  if (!Array.isArray(candidate)) return null;
  const selected = AGENT_STATES.filter((state) => candidate.includes(state));
  return selected.length > 0 ? selected : null;
}

/** Restore the ID snapshot captured when a status filter was switched on. */
function loadIdSnapshot(candidate: unknown): Record<string, true> | null {
  if (!candidate || typeof candidate !== "object" || Array.isArray(candidate)) return null;
  const out: Record<string, true> = {};
  let count = 0;
  for (const [id, value] of Object.entries(candidate as Record<string, unknown>)) {
    if (value !== true || !id) continue;
    out[id.slice(0, 100)] = true;
    if (++count >= SIDEBAR_VIEW_MAP_LIMIT) break;
  }
  // An empty snapshot is still a snapshot: the filter was on and matched nothing, which the user should get back.
  return out;
}

/** Restore a per-view collapse map, dropping anything that is not an explicit boolean. */
function loadCollapsedOverrides(candidate: unknown): Record<string, boolean> | null {
  if (!candidate || typeof candidate !== "object" || Array.isArray(candidate)) return null;
  const out: Record<string, boolean> = {};
  let count = 0;
  for (const [id, value] of Object.entries(candidate as Record<string, unknown>)) {
    if (typeof value !== "boolean" || !id) continue;
    out[id.slice(0, 100)] = value;
    if (++count >= SIDEBAR_VIEW_MAP_LIMIT) break;
  }
  return count > 0 ? out : null;
}

/** Validate one persisted split tree and reject duplicate or dangling view references. */
function loadSidebarPane(
  candidate: unknown,
  validViewIds: Set<string>,
  usedViewIds: Set<string>,
  usedPaneIds: Set<string>,
): SidebarViewPaneNode | null {
  if (!candidate || typeof candidate !== "object") return null;
  const node = candidate as Partial<SidebarViewPaneNode>;
  const paneId = typeof node.paneId === "string" ? node.paneId.slice(0, 100) : "";
  if (!paneId || usedPaneIds.has(paneId)) return null;
  usedPaneIds.add(paneId);
  if (node.kind === "leaf") {
    const viewId = typeof node.viewId === "string" ? node.viewId : "";
    if (!validViewIds.has(viewId) || usedViewIds.has(viewId)) return null;
    usedViewIds.add(viewId);
    return { kind: "leaf", paneId, viewId };
  }
  if (node.kind !== "split" || (node.dir !== "horizontal" && node.dir !== "vertical")) return null;
  const split = node as Partial<Extract<SidebarViewPaneNode, { kind: "split" }>>;
  const rawFirst = Array.isArray(split.sizes) ? Number(split.sizes[0]) : 50;
  const first = Number.isFinite(rawFirst) ? Math.max(10, Math.min(90, rawFirst)) : 50;
  const a = loadSidebarPane(split.a, validViewIds, usedViewIds, usedPaneIds);
  const b = loadSidebarPane(split.b, validViewIds, usedViewIds, usedPaneIds);
  if (!a || !b) return null;
  return {
    kind: "split",
    paneId,
    dir: node.dir,
    sizes: [first, 100 - first],
    a,
    b,
  };
}

/** Loads, validates, and migrates frontend-only sidebar tabs and projections. */
function loadSidebarViews() {
  const fallback = defaultSidebarViews();
  try {
    const raw = localStorage.getItem(SIDEBAR_VIEWS_KEY);
    if (!raw) return fallback;
    const saved = JSON.parse(raw) as Partial<PersistedSidebarViewsV1 | PersistedSidebarViewsV2>;
    if ((saved.version !== 1 && saved.version !== 2)
      || !Array.isArray(saved.views)
      || saved.views.length === 0) {
      return fallback;
    }
    const seen = new Set<string>();
    const views: SidebarTreeView[] = [];
    for (const candidate of saved.views) {
      if (!candidate || typeof candidate.id !== "string" || seen.has(candidate.id)) continue;
      const id = candidate.id.slice(0, 100);
      if (!id) continue;
      seen.add(id);
      // A status filter only means something together with the ID snapshot taken when it was switched on, because
      // live statuses are gone after a restart. Payloads without that snapshot (older versions) start unfiltered.
      const statusFilterIds = loadIdSnapshot(candidate.statusFilterIds);
      const statusFilter = statusFilterIds ? loadStatusFilter(candidate.statusFilter) : null;
      views.push({
        id,
        name: typeof candidate.name === "string" && candidate.name.trim()
          ? candidate.name.replace(/[\u0000-\u001F\u007F-\u009F]/g, "").trim().slice(0, 80)
          : t("tree.viewUntitled"),
        treeFilter: typeof candidate.treeFilter === "string"
          ? candidate.treeFilter.slice(0, 500)
          : "",
        statusFilter,
        statusFilterIds: statusFilter ? statusFilterIds : null,
        markFilter: typeof candidate.markFilter === "string" && candidate.markFilter.trim()
          ? candidate.markFilter.trim().slice(0, 16)
          : null,
        collapsedOverrides: loadCollapsedOverrides(candidate.collapsedOverrides),
      });
    }
    if (views.length === 0) return fallback;
    const primaryId = views.some((view) => view.id === saved.primaryId)
      ? saved.primaryId as string
      : views[0].id;
    // The main tree keeps its old behavior: it always starts unfiltered and follows the shared collapse state in the
    // database. Only split-off panes come back exactly as the user left them.
    for (const [index, view] of views.entries()) {
      if (view.id !== primaryId) continue;
      views[index] = {
        ...view,
        statusFilter: null,
        statusFilterIds: null,
        markFilter: null,
        collapsedOverrides: null,
      };
    }
    const activeId = views.some((view) => view.id === saved.activeId)
      ? saved.activeId as string
      : primaryId;
    let tabs: SidebarTreeTab[];
    if (saved.version === 1) {
      tabs = migrateLegacySidebarTabs(
        views.map((view) => view.id),
        saved.layout === "stack" ? "stack" : "tabs",
      );
      if (saved.layout === "stack" && tabs[0]) tabs[0].activeViewId = activeId;
    } else {
      const validViewIds = new Set(views.map((view) => view.id));
      let usedViewIds = new Set<string>();
      const seenTabIds = new Set<string>();
      tabs = [];
      const savedV2 = saved as Partial<PersistedSidebarViewsV2>;
      for (const candidate of Array.isArray(savedV2.tabs) ? savedV2.tabs : []) {
        if (!candidate || typeof candidate.id !== "string") continue;
        const id = candidate.id.slice(0, 100);
        if (!id || seenTabIds.has(id)) continue;
        const candidateViewIds = new Set(usedViewIds);
        const root = loadSidebarPane(
          candidate.root,
          validViewIds,
          candidateViewIds,
          new Set<string>(),
        );
        if (!root) continue;
        seenTabIds.add(id);
        usedViewIds = candidateViewIds;
        const memberIds = collectSidebarViewIds(root);
        tabs.push({
          id,
          root,
          activeViewId: memberIds.includes(candidate.activeViewId)
            ? candidate.activeViewId
            : firstSidebarViewId(root),
        });
      }
      for (const view of views) {
        if (!usedViewIds.has(view.id)) tabs.push(makeSidebarTreeTab(view.id));
      }
    }
    tabs = tabs.map((tab) =>
      collectSidebarViewIds(tab.root).includes(activeId)
        ? { ...tab, activeViewId: activeId }
        : tab);
    return {
      views,
      tabs,
      primaryId,
      activeId,
    };
  } catch {
    return fallback;
  }
}

/** Debounce timers for persisting browser-node URLs, keyed by node ID. */
const browserUrlTimers = new Map<string, ReturnType<typeof setTimeout>>();

/** Serialized persistent layout. */
interface PersistedLayout {
  openTabs: string[];
  paneTrees: Record<string, PaneNode>;
  activeTabId: string | null;
  activeSessionId: string | null;
  focusedPaneId: string | null;
  /**
   * Complete tabs kept alive off the tab bar. Their pane trees, splits, and ephemeral sessions remain intact.
   */
  liveTabs: string[];
  /**
   * Metadata for persisted `eph-` sessions in browser/remote mode. Browser closure detaches without killing,
   * so referenced ephemeral terminals must be restored before reattaching. Desktop mode omits them because
   * application exit terminates their processes.
   */
  ephemeralSessions?: Record<string, Session>;
}

/**
 * Whether layout has been restored from local storage. Restore only during the first `loadTree`; later tree
 * refreshes must reconcile current memory state. Reapplying the startup snapshot would discard live background
 * tabs and kill their PTYs.
 */
let layoutRestored = false;

/** Debounces layout writes to local storage. */
let saveLayoutTimer: ReturnType<typeof setTimeout> | undefined;
function saveLayoutTick() {
  clearTimeout(saveLayoutTimer);
  saveLayoutTimer = setTimeout(() => {
    const s = useTermStore.getState();
    const ephemeralIds = new Set(Object.keys(s.ephemeralSessions));
    // Desktop shells remove ephemeral pane leaves because their processes die with the app. Browser/remote
    // mode preserves them and their metadata because closing a page only detaches from shared server sessions.
    const keepEphemeral = platform.env.isBrowser;
    const stripEphemeral = (t0: PaneNode): PaneNode | null => {
      let t: PaneNode | null = t0;
      for (const sid of collectSessionIds(t0)) {
        if (ephemeralIds.has(sid)) {
          t = t ? removeSession(t, sid) : null;
          if (!t) break;
        }
      }
      return t;
    };
    const usedEphemeral: Record<string, Session> = {};
    const collectEphemeral = (tree: PaneNode) => {
      for (const sid of collectSessionIds(tree)) {
        const eph = s.ephemeralSessions[sid];
        if (eph) usedEphemeral[sid] = eph;
      }
    };
    const openTabs: string[] = [];
    const paneTrees: Record<string, PaneNode> = {};
    for (const tabId of s.openTabs) {
      const t0 = s.paneTrees[tabId];
      if (!t0) continue;
      const t = keepEphemeral ? t0 : stripEphemeral(t0);
      if (t) {
        openTabs.push(tabId);
        paneTrees[tabId] = t;
        if (keepEphemeral) collectEphemeral(t);
      }
    }
    // Persist complete background trees, pruning ephemeral leaves on desktop and empty results everywhere.
    const liveTabs: string[] = [];
    for (const tabId of s.liveTabs) {
      const t0 = s.paneTrees[tabId];
      if (!t0) continue;
      const t = keepEphemeral ? t0 : stripEphemeral(t0);
      if (t) {
        liveTabs.push(tabId);
        paneTrees[tabId] = t;
        if (keepEphemeral) collectEphemeral(t);
      }
    }
    // Clear a desktop ephemeral active session; browser mode preserves it across reopen.
    const activeSessionId =
      s.activeSessionId &&
      (keepEphemeral || !ephemeralIds.has(s.activeSessionId))
        ? s.activeSessionId
        : null;
    const layout: PersistedLayout = {
      openTabs,
      paneTrees,
      activeTabId: s.activeTabId,
      activeSessionId,
      focusedPaneId: s.focusedPaneId,
      liveTabs,
      ephemeralSessions: keepEphemeral ? usedEphemeral : undefined,
    };
    try {
      localStorage.setItem(LAYOUT_KEY, JSON.stringify(layout));
    } catch {
      /* Ignore unavailable or full local storage. */
    }
  }, 300); // 300 ms debounce.
}

const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

/** Finds the tab and pane containing a session. */
function locate(
  paneTrees: Record<string, PaneNode>,
  openTabs: string[],
  sessionId: string,
): { tabId: string; paneId: string } | null {
  for (const tabId of openTabs) {
    const t = paneTrees[tabId];
    if (!t) continue;
    const leaf = findBySession(t, sessionId);
    if (leaf) return { tabId, paneId: leaf.paneId };
  }
  return null;
}

/** Prunes pane leaves absent from `valid`; returns `null` when the whole tree becomes empty. */
function pruneTree(
  tree: PaneNode | null | undefined,
  valid: Set<string>,
): PaneNode | null {
  let t: PaneNode | null = tree ?? null;
  if (!t) return null;
  for (const sid of collectSessionIds(t)) {
    if (!valid.has(sid)) {
      t = t ? removeSession(t, sid) : null;
      if (!t) break;
    }
  }
  return t;
}

/** Reconciles pane trees and active/focused state after data changes. Document and browser tabs are exempt
 * because they have metadata but no pane tree. */
function reconcileTabs(state: {
  sessions: Session[];
  ephemeralSessions: Record<string, Session>;
  openTabs: string[];
  paneTrees: Record<string, PaneNode>;
  activeTabId: string | null;
  activeSessionId: string | null;
  focusedPaneId: string | null;
  liveTabs: string[];
  docTabs: Record<string, DocTab>;
  browserTabs: Record<string, BrowserTab>;
}) {
  const valid = new Set([
    ...state.sessions.map((s) => s.id),
    ...Object.keys(state.ephemeralSessions),
  ]);
  const paneTrees: Record<string, PaneNode> = {};
  const openTabs: string[] = [];
  for (const tabId of state.openTabs) {
    if (state.docTabs[tabId] || state.browserTabs[tabId]) {
      // Preserve metadata-backed document/browser tabs without pane trees. A browser tab bound to a tree
      // node uses the node ID and closes when that node is deleted or archived.
      if (
        state.browserTabs[tabId] &&
        !tabId.startsWith("browser-") &&
        !valid.has(tabId)
      ) {
        continue;
      }
      openTabs.push(tabId);
      continue;
    }
    const t = pruneTree(state.paneTrees[tabId], valid);
    if (t) {
      paneTrees[tabId] = t;
      openTabs.push(tabId);
    }
  }
  // Remove orphan document/browser metadata absent from `openTabs`.
  const inOpen = new Set(openTabs);
  const docTabs: Record<string, DocTab> = {};
  for (const [id, d] of Object.entries(state.docTabs)) {
    if (inOpen.has(id)) docTabs[id] = d;
  }
  const browserTabs: Record<string, BrowserTab> = {};
  for (const [id, b] of Object.entries(state.browserTabs)) {
    if (inOpen.has(id)) browserTabs[id] = b;
  }
  let { activeTabId, activeSessionId, focusedPaneId } = state;
  if (!activeTabId || !openTabs.includes(activeTabId)) {
    activeTabId = openTabs[0] ?? null;
  }
  if (activeTabId && (docTabs[activeTabId] || browserTabs[activeTabId])) {
    // Document/browser tabs have no active session or focused pane.
    activeSessionId = null;
    focusedPaneId = null;
  } else {
    const at = activeTabId ? paneTrees[activeTabId] : null;
    if (!at) {
      activeSessionId = null;
      focusedPaneId = null;
    } else if (!activeSessionId || !findBySession(at, activeSessionId)) {
      const leaf = firstLeaf(at);
      activeSessionId = leaf.sessionId;
      focusedPaneId = leaf.paneId;
    }
  }
  // Reconcile background tabs too, dropping invalid leaves, empty trees, and visible duplicates.
  const inTabs = new Set(openTabs);
  const liveTabs: string[] = [];
  for (const tabId of state.liveTabs ?? []) {
    if (inTabs.has(tabId) || paneTrees[tabId]) continue; // Already visible or collides with an existing tree.
    const t = pruneTree(state.paneTrees[tabId], valid);
    if (t) {
      paneTrees[tabId] = t;
      liveTabs.push(tabId);
    }
  }
  return {
    paneTrees,
    openTabs,
    activeTabId,
    activeSessionId,
    focusedPaneId,
    liveTabs,
    docTabs,
    browserTabs,
  };
}

/** Selected tree node. */
export interface SelNode {
  id: string;
  kind: NodeKind;
}

/**
 * Browser tab parallel to `DocTab`. It has no pane tree or session, so session reuse never replaces it and
 * layout persistence omits it. Content lives in a native child WebView tied to `BrowserView` mount/unmount.
 * Desktop only.
 */
export interface BrowserTab {
  /** `"browser-" + genId()`, used in `openTabs`. */
  id: string;
  /** Current URL from `browser://state`; new tabs start at `about:blank`. */
  url: string;
  /** Tab title; currently the URL host. */
  title: string;
  /** Loading state used to switch the refresh/stop control. */
  loading: boolean;
}

/**
 * Document tab parallel to terminal pane-tree tabs. It has no pane tree or session, so session reuse cannot
 * replace it and persistence omits it. Document content stays in the mounted `DocView`, not Zustand snapshots.
 */

interface TermStore {
  // Persistent structure loaded from SQLite.
  projects: Project[];
  groups: Group[];
  sessions: Session[];
  /** Archived sessions loaded on demand for the archive browser. */
  archivedSessions: Session[];
  /** False until the first `loadTree` resolves, so the sidebar can avoid flashing the empty state at startup. */
  treeLoaded: boolean;

  // In-memory runtime state.
  runtimes: Record<SessionId, SessionRuntime>;
  /** Session restart generation; incrementing forces `TerminalView` reconstruction. */
  epochs: Record<SessionId, number>;
  /** Ephemeral split sessions that are neither persisted nor listed in the sidebar. */
  ephemeralSessions: Record<SessionId, Session>;
  /** Initial prompt pending for a spawned child, consumed by `usePtySession` after startup. */
  pendingPrompts: Record<SessionId, string>;
  /** FIFO spawn-confirmation queue processed one item at a time by `SpawnConfirmModal`. */
  pendingSpawns: SpawnRequest[];
  /** Target ID for the open merge dialog, or `null`. */
  mergeTarget: SessionId | null;
  /** Working directory for the open changes dialog, or `null`. */
  changesCwd: string | null;

  // Center tabs and split panes.
  openTabs: SessionId[]; // Visible tabs, identified by root session ID.
  activeTabId: SessionId | null; // Current tab.
  /**
   * Most recently active session tab, including scratch terminals but excluding document/browser tabs.
   * Single-tab mode reuses this slot when the current tab is not a session, preserving document/browser tabs.
   */
  lastActiveSessionTabId: SessionId | null;
  paneTrees: Record<SessionId, PaneNode>; // Pane tree for each tab.
  activeSessionId: SessionId | null; // Session in the focused pane.
  /**
   * One-shot sidebar reveal suppression for newly created, spawned, or forked sessions. It prevents an
   * automatic scroll from disrupting the user immediately after the new terminal opens. `ProjectTree` consumes it.
   */
  revealSuppressId: SessionId | null;
  /** Project requested by `vela <path>` for sidebar reveal; consumed by `ProjectTree`. */
  revealProjectId: string | null;
  focusedPaneId: string | null; // Currently focused pane.
  /**
   * Complete tabs kept alive off the tab bar; their pane trees remain mounted and restore intact.
   */
  liveTabs: SessionId[];
  /**
   * Pinned tabs opened explicitly in a new tab or as scratch terminals. Single-tab reuse never replaces them;
   * only the one unpinned session slot is reusable.
   */
  pinnedTabs: SessionId[];
  /**
   * Status notice after automatic background-tab eviction. Timestamp retriggers notices for repeated labels.
   */
  liveEvictNotice: { label: string; at: number } | null;
  /**
   * Set when the background limit is exceeded but all tabs are active, prompting rather than killing automatically.
   */
  liveEvictAsk: boolean;
  /** Metadata for `doc-` entries in `openTabs`. */
  docTabs: Record<string, DocTab>;
  /** Metadata for `browser-` entries in `openTabs`. */
  browserTabs: Record<string, BrowserTab>;

  // Other UI state.
  leftCollapsed: boolean;
  rightCollapsed: boolean;
  leftWidth: number;
  rightWidth: number;
  bottomExpanded: boolean;
  theme: Theme;
  /** Saved desktop sidebar projections. The original tree is the initial primary projection. */
  sidebarTreeViews: SidebarTreeView[];
  /** Legacy persisted layout retained only so existing local data can still be loaded without data loss. */
  sidebarTreeTabs: SidebarTreeTab[];
  primarySidebarTreeViewId: string;
  /** Legacy persisted active projection; the desktop sidebar always renders the primary projection. */
  activeSidebarTreeViewId: string;
  /** Primary-view aliases retained for global status-bar actions and the mobile session list. */
  treeFilter: string;
  /**
   * Multi-select sidebar status filter using OR semantics, or `null` for all.
   * `asking` includes sessions with unread notifications.
   */
  statusFilter: AgentState[] | null;
  /**
   * Matching IDs captured when a filter is activated. When enabled, dynamic additions union new matches into this
   * snapshot; stale matches remain visible until the filter changes, preserving the original stable-membership logic.
   */
  statusFilterIds: Record<string, true> | null;
  /**
   * Single-select sidebar marker filter holding the marker emoji, or `null` for all. Unlike the status filter this
   * needs no snapshot: markers change only when the user sets one, so live filtering cannot make a row vanish
   * under the pointer.
   */
  markFilter: string | null;
  searchOpen: boolean;
  /** Whether the notification-permission guidance dialog is open. */
  notifyGuideOpen: boolean;
  setNotifyGuideOpen: (v: boolean) => void;
  /** Whether the archive browser is open. */
  archiveOpen: boolean;
  /** Whether the global session-content search overlay is open. */
  globalSearchOpen: boolean;
  /** Whether the browser-mode project-directory picker is open. */
  dirPickerOpen: boolean;
  /** Whether the create-project dialog is visible. */
  createProjectModalOpen: boolean;
  /** Shared open state for the Settings dialog and native menu command. */
  settingsOpen: boolean;
  /** Shared open state for the Share dialog and native menu command. */
  shareOpen: boolean;
  /** Whether the error-log panel is open. */
  errorLogOpen: boolean;
  /** Whether the cross-platform Git clone dialog is open. */
  cloneModalOpen: boolean;
  /** Browser-mode Save As request. `DocView` creates it and `SaveAsModal` resolves the selected path. */
  saveAsRequest: { defaultName: string; resolve: (path: string | null) => void } | null;

  // Sidebar multi-selection.
  selection: SelNode[];
  selectionAnchor: string | null;

  // Right-panel inspection target, driven by the last focused session, project, or group.
  inspectTarget: SelNode | null;

  // Sessions with recent system notifications, mapped to notification timestamps.
  notifications: Record<SessionId, number>;
  // Whether the application window is focused.
  windowFocused: boolean;
  // Persisted system-notification sound setting, enabled by default.
  soundEnabled: boolean;
  // Persisted system-notification setting; disabling it preserves unread indicators and Dock badges.
  notifyEnabled: boolean;
  // Persisted automatic cleanup for temporary pasted images, enabled by default.
  cleanPastedImages: boolean;
  // Persisted session-recording setting. Backend spawn applies it; plain Terminal sessions are never recorded.
  recordSessions: boolean;

  // Vlinx appearance settings, driven by `data-*` attributes and persisted in `vlx-settings`.
  accent: AccentChoice;
  density: Density;
  paneStyle: PaneStyle;
  dividerStyle: DividerStyle;
  navLayout: NavLayout;
  inspectorTab: InspectorTab;
  /** Single-tab mode: reuse the session slot and keep replaced tabs alive in the background. */
  singleTabMode: boolean;
  /** Terminal renderer: stable DOM, GPU-independent 2D canvas, or sharper WebGL with context limits. */
  termRenderer: TermRenderer;
  /** Optional full redraw when returning to a tab, for GPU artifacts or blank frames. */
  redrawOnReveal: boolean;
  /** Foreground-priority output scheduling; background terminals are coalesced and throttled. */
  outputScheduler: boolean;
  /** Automatically append newly matching sessions to active sidebar status filters. */
  dynamicStatusFilter: boolean;
  /** Configurable limit for background live tabs. */
  maxLiveTabs: number;
  /** Default shell path/name for scratch terminals; empty means system default. Explicit creation flows may override it. */
  defaultShell: string;
  /** Platform shells discovered once at startup for the inline selector. */
  shells: ShellOption[];
  /** Interface monospace font, or `null` for the default stack. */
  uiFontFamily: string | null;
  /** Interface font size in pixels, or `null` to follow density. */
  uiFontSize: number | null;
  /** Terminal monospace font, applied live to all terminals. */
  termFontFamily: string | null;
  /** Terminal font size in pixels, applied live to all terminals. */
  termFontSize: number;
  /** Custom global shortcut overrides by action ID. */
  shortcutOverrides: Record<string, string>;
  /** Default arguments and permission mode by agent type, applied when new sessions omit them. */
  agentDefaults: Record<string, AgentDefaultConfig>;
  /** Whether spawning a child requires confirmation. */
  spawnConfirm: boolean;
  orchestrationProfiles: Record<string, OrchestrationProfile>;
  orchestration: OrchestrationLimits;
  /** Usage auto-refresh interval in seconds; zero disables it. */
  usageRefreshSec: number;
  /** Image-paste mode, configurable only in the local desktop app. */
  imagePasteMode: ImagePasteMode;

  // Data loading and mutations.
  loadTree: () => Promise<void>;
  importProject: () => Promise<void>;
  /** Imports a project selected by the browser directory picker. */
  importProjectPath: (rootPath: string) => Promise<void>;
  /** Handles `vela <path>` by importing or reusing, expanding, selecting, and revealing the project. */
  openProjectPath: (rootPath: string) => Promise<void>;
  setDirPickerOpen: (open: boolean) => void;
  /** Opens or closes the create-project dialog. */
  setCreateProjectModalOpen: (open: boolean) => void;
  /** Opens or closes Settings. */
  setSettingsOpen: (open: boolean) => void;
  /** Opens or closes Share. */
  setShareOpen: (open: boolean) => void;
  /** Opens or closes the error-log panel. */
  setErrorLogOpen: (open: boolean) => void;
  /** Opens or closes the Git clone dialog. */
  setCloneModalOpen: (open: boolean) => void;
  /** Clones into `parentDir` and imports the project. An empty branch uses the remote default. */
  cloneProjectInto: (
    url: string,
    parentDir: string,
    folderName?: string,
    branch?: string,
    operationId?: string,
  ) => Promise<void>;
  /** Browser mode: requests a server-side Save As path, or `null` on cancellation. */
  promptSaveAs: (defaultName: string) => Promise<string | null>;
  addGroup: (
    projectId: string,
    parentGroupId: string | null,
    name: string,
    worktree?: { worktreePath?: string | null; worktreeBaseRef?: string | null },
  ) => Promise<void>;
  addSession: (input: tree.CreateSessionInput) => Promise<Session | null>;
  /** Forks current conversation history into a sibling session and opens it without changing the source. */
  forkSession: (id: SessionId) => Promise<void>;
  /** Handles an agent child-spawn request by queuing confirmation or executing immediately. */
  handleSpawnRequest: (req: SpawnRequest) => Promise<void>;
  /** Confirms and executes the first queued spawn using the possibly edited dialog values. */
  confirmSpawn: (req: SpawnRequest) => Promise<void>;
  /** Cancels the first queued spawn without creating a session. */
  cancelSpawn: () => void;
  /** Opens branch merge for a session or group target. */
  openMerge: (id: SessionId) => void;
  /** Closes branch merge. */
  closeMerge: () => void;
  /** Opens the changes dialog for a working directory. */
  openChanges: (cwd: string) => void;
  /** Closes the changes dialog. */
  closeChanges: () => void;
  /** Executes a spawn: creates the child and optional worktree, stores its prompt, and opens it. */
  executeSpawn: (req: SpawnRequest) => Promise<void>;
  /** Consumes a session's pending initial spawn prompt after PTY startup. */
  takePendingPrompt: (id: SessionId) => string | undefined;
  renameNode: (kind: NodeKind, id: string, name: string) => Promise<void>;
  /** Sets or clears a node's emoji marker; passing null clears it. */
  setNodeMark: (kind: NodeKind, id: string, mark: string | null) => Promise<void>;
  updateSession: (id: string, input: tree.UpdateSessionInput) => Promise<void>;
  /** Converts a node back to a normal session/group after its worktree is deleted, clearing session cwd too. */
  clearNodeWorktree: (kind: NodeKind, id: string) => Promise<void>;
  deleteNode: (kind: NodeKind, id: string) => Promise<void>;
  deleteMany: (nodes: SelNode[]) => Promise<void>;
  /** Archives a session without deleting data, closing any visible/background tab first. */
  archiveSession: (id: SessionId) => Promise<void>;
  /** Archives many sessions, then reloads and reconciles once to avoid concurrent tree-refresh races. */
  archiveMany: (ids: SessionId[]) => Promise<void>;
  /** Archives an entire group and keeps a hidden tombstone that returns when any child is restored. */
  archiveGroup: (id: string) => Promise<void>;
  /** Restores an archived session to the normal tree. */
  restoreSession: (id: SessionId) => Promise<void>;
  /** Loads archived sessions. */
  loadArchived: () => Promise<void>;
  /** Opens/closes the archive browser and loads entries on open. */
  setArchiveOpen: (open: boolean) => void;
  /** Opens/closes global session search; the overlay owns debounced querying. */
  setGlobalSearchOpen: (open: boolean) => void;
  selectSingle: (node: SelNode) => void;
  toggleSelect: (node: SelNode) => void;
  setSelection: (nodes: SelNode[], anchor: string | null) => void;
  clearSelection: () => void;
  /** Sets the right-panel inspection target. */
  setInspectTarget: (node: SelNode | null) => void;
  /** Sets or clears one-shot sidebar reveal suppression. */
  setRevealSuppress: (id: SessionId | null) => void;
  setRevealProject: (id: string | null) => void;
  moveNode: (
    kind: NodeKind,
    id: string,
    targetProjectId: string | null,
    targetGroupId: string | null,
    targetParentSessionId: string | null,
    sortOrder: number,
  ) => Promise<void>;
  /** Moves multiple sessions to one target with increasing sort order, then reloads once and clears selection. */
  moveMany: (
    ids: SessionId[],
    targetProjectId: string,
    targetGroupId: string | null,
    targetParentSessionId: string | null,
  ) => Promise<void>;
  toggleCollapsed: (
    kind: "project" | "group" | "session",
    id: string,
  ) => Promise<void>;

  // Tabs and splits.
  /** Opens a session by reusing the current slot and backgrounding the replaced tab, unless `newTab` is true. */
  openSession: (id: SessionId, opts?: { newTab?: boolean }) => void;
  setActiveTab: (tabId: SessionId) => void;
  /** Reorders a tab before or after a target; invalid or unchanged moves are no-ops. */
  reorderTab: (tabId: string, targetId: string, side: "before" | "after") => void;
  closeTab: (tabId: SessionId) => void;
  /**
   * Closes a background tab without restoring it, removing its tree so unmount kills or detaches the process.
   */
  closeLiveTab: (tabId: string) => void;
  /**
   * Explicitly backgrounds a visible session tab without stopping it. Its pane tree remains mounted; active
   * focus transfers to a neighboring tab and standard overflow eviction applies.
   */
  moveTabToBackground: (tabId: SessionId) => void;
  /** Clears the background-eviction status notice. */
  clearLiveEvictNotice: () => void;
  /** Closes the all-active overflow prompt, whether or not the user evicts a tab. */
  dismissLiveEvictAsk: () => void;
  /**
   * Opens a document tab. Reopening an existing path focuses it and forces a disk reload; otherwise creates one.
   */
  openDocTab: (path: string) => void;
  /** Creates an untitled plain-text draft whose first Save As establishes path and syntax. */
  newDocTab: () => void;
  /** Applies Save As path/title/kind and converts a draft to normal read/write mode. */
  setDocTabPath: (id: string, path: string) => void;
  /** Refreshes a document by incrementing its reload nonce. */
  refreshDocTab: (id: string) => void;
  /** Toggles a document tab's edit mode. */
  setDocTabMode: (id: string, mode: DocTab["mode"]) => void;
  /** Marks or clears unsaved document changes. */
  setDocTabDirty: (id: string, dirty: boolean) => void;
  /** Requests document closure; clean tabs close immediately and dirty tabs prompt. */
  requestCloseDocTab: (id: string) => void;
  /** Cancels a pending document close. */
  cancelCloseDocTab: (id: string) => void;
  /**
   * Opens a new browser tab without deduplication, starting blank when no URL is supplied.
   */
  openBrowserTab: (url?: string) => void;
  /** Merges URL, title, and loading patches from `browser://state`. */
  applyBrowserState: (
    id: string,
    patch: Partial<Pick<BrowserTab, "url" | "title" | "loading">>,
  ) => void;
  focusPane: (paneId: string, sessionId: SessionId) => void;
  splitNew: (direction: "horizontal" | "vertical") => Promise<void>;
  closePane: () => void;
  closeSession: (sessionId: SessionId) => void;
  collapseToFocused: () => void;
  /** Persists an ephemeral session under its existing ID, preserving the running PTY and context. An optional
   * target override is used for drag-and-drop placement. */
  persistSession: (
    id: SessionId,
    override?: { projectId: string; groupId: string | null; parentSessionId: string | null },
  ) => Promise<void>;
  /** Persists a `browser-` draft as a Browser tree node with a new node ID, reloading the current URL while
   * preserving shared persistent login state. */
  persistBrowserDraft: (
    tabId: string,
    override?: { projectId: string; groupId: string | null; parentSessionId: string | null },
  ) => Promise<void>;
  /** Renames an ephemeral session, browser, or document draft in memory only. */
  renameScratch: (id: SessionId, name: string) => void;
  pruneEphemeral: () => void;
  setRuntime: (id: SessionId, partial: Partial<SessionRuntime>) => void;
  /** Processes backend status signals into agent state and optional system notifications. */
  applyStatusSignal: (id: SessionId, signal: StatusSignal) => void;
  /**
   * Arbitrates frontend screen detection using the effective-state priority chain.
   */
  applyScreenDetection: (id: SessionId, screen: ScreenDetection) => void;
  restartSession: (id: SessionId) => Promise<void>;

  // Notification navigation.
  /** Records window focus changes. */
  setWindowFocused: (focused: boolean) => void;
  /** Clears notification markers for missing sessions when the window returns to the foreground. */
  focusReturned: () => void;
  /** Clears a session's notification marker after it has been viewed. */
  clearNotification: (id: SessionId) => void;
  /** Clears all notification markers, session dots, and the Dock badge. */
  clearAllNotifications: () => void;

  // Layout.
  toggleLeft: () => void;
  toggleRight: () => void;
  resizeLeft: (deltaX: number) => void;
  resizeRight: (deltaX: number) => void;
  toggleBottom: () => void;
  toggleTheme: () => void;
  /** Sets the explicit light/dark mode. */
  setTheme: (mode: Theme) => void;
  /** Toggles persisted notification sounds. */
  toggleSound: () => void;
  /** Toggles persisted system notifications. */
  toggleNotify: () => void;
  /** Sets cross-shell automatic cleanup for pasted images. */
  setCleanPastedImages: (v: boolean) => void;
  /** Sets cross-shell session recording; backend spawn applies it and plain terminals remain excluded. */
  setRecordSessions: (v: boolean) => void;
  /** Copies one sidebar projection into a recursively splittable pane below or beside it. */
  splitSidebarTreeView: (
    direction: SidebarSplitDirection,
    sourceViewId?: string,
  ) => string;
  /** Removes one non-primary projection and promotes its sibling in the recursive split tree. */
  deleteSidebarTreeView: (id: string) => void;
  /** Marks the projection that receives subsequent pane-local commands. */
  setActiveSidebarTreeView: (id: string) => void;
  /** Resizes one node in the recursive sidebar split tree. */
  resizeSidebarTreeSplit: (
    tabId: string,
    splitPaneId: string,
    sizes: [number, number],
  ) => void;
  setSidebarTreeViewFilter: (id: string, q: string) => void;
  setSidebarTreeViewStatusFilter: (id: string, st: AgentState) => void;
  /** Adds newly matching sessions to an active status filter without removing stale members. */
  appendSidebarTreeViewStatusMatches: (id: string) => void;
  /** Replaces one view's retained status snapshot with the sessions matching right now. */
  refreshSidebarTreeViewStatusMatches: (id: string) => void;
  /**
   * Re-evaluates a single session against one view's active status filter, adding it to the retained snapshot
   * when it still matches and dropping it when it no longer does.
   */
  refreshSidebarTreeViewStatusMatch: (id: string, sessionId: string) => void;
  setSidebarTreeViewMarkFilter: (id: string, mark: string | null) => void;
  /**
   * Records one node's collapse state inside a split-off projection. The primary view keeps using
   * `toggleCollapsed`, which writes the shared state to the database.
   */
  setSidebarTreeViewCollapsed: (id: string, nodeId: string, collapsed: boolean) => void;
  setTreeFilter: (q: string) => void;
  /**
   * Replaces the primary sidebar's status filter with one state. Selecting that sole state again
   * disables status filtering.
   */
  setStatusFilter: (st: AgentState) => void;
  /** Sets the single-select sidebar marker filter; selecting the same marker again clears it. */
  setMarkFilter: (mark: string | null) => void;
  openSearch: () => void;
  closeSearch: () => void;

  // Persisted Vlinx appearance settings applied through `data-*` attributes.
  setAccent: (v: AccentChoice) => void;
  setDensity: (v: Density) => void;
  setPaneStyle: (v: PaneStyle) => void;
  setDividerStyle: (v: DividerStyle) => void;
  setNavLayout: (v: NavLayout) => void;
  setInspectorTab: (v: InspectorTab) => void;
  /** Toggles persisted single-tab mode. */
  setSingleTabMode: (v: boolean) => void;
  /** Toggles confirmation before spawning child sessions. */
  setSpawnConfirm: (v: boolean) => void;
  /** Sets persisted image-paste mode for subsequent local desktop pastes. */
  setImagePasteMode: (v: ImagePasteMode) => void;
  /** Sets persisted usage-refresh interval in seconds; zero disables it. */
  setUsageRefreshSec: (v: number) => void;
  /** Sets the persisted terminal renderer for new terminals. */
  setTermRenderer: (v: TermRenderer) => void;
  setRedrawOnReveal: (v: boolean) => void;
  /** Toggles persisted foreground-priority output scheduling, effective on the next chunk. */
  setOutputScheduler: (v: boolean) => void;
  /** Enables or disables automatic additions to active sidebar status filters. */
  setDynamicStatusFilter: (v: boolean) => void;
  setMaxLiveTabs: (v: number) => void;
  /** Sets the persisted default terminal shell; empty means system default. */
  setDefaultShell: (v: string) => void;
  /** Interface font family; empty or `null` uses the default stack. */
  setUiFontFamily: (v: string | null) => void;
  /** Interface font size in pixels; `null` follows density. */
  setUiFontSize: (v: number | null) => void;
  /** Terminal font family; empty or `null` uses the default stack. */
  setTermFontFamily: (v: string | null) => void;
  /** Terminal font size in pixels, clamped to 10–24. */
  setTermFontSize: (v: number) => void;
  /** Persists a global shortcut override for one action. */
  setShortcut: (action: string, combo: string) => void;
  /** Restores all global shortcuts by clearing overrides. */
  resetShortcuts: () => void;
  /** Merges and persists an agent-type default patch, removing empty/default values. */
  setAgentDefault: (kind: string, patch: Partial<AgentDefaultConfig>) => void;
  setOrchestrationProfile: (name: string, patch: Partial<OrchestrationProfile> | null) => void;
  setOrchestrationLimits: (patch: Partial<OrchestrationLimits>) => void;
  /** Applies current theme and visual settings to `documentElement` on mount. */
  applyAppearance: () => void;
  /** Reloads preferences from local storage after startup reconciliation rewrites the cache from backend
   * authority. Applies values without writing them back, limiting visible adjustment to one pass. */
  hydrateSettingsFromCache: () => void;

  // Center area: split resizing and scratch tabs.
  /** Updates split-node percentages, which sum to 100. */
  resizePane: (tabId: SessionId, splitPaneId: string, sizes: [number, number]) => void;
  /** Opens an unpersisted scratch terminal tab with optional shell, cwd, and title. */
  newScratchTab: (opts?: {
    shell?: string | null;
    cwd?: string | null;
    name?: string;
    /** Project-tree target used to avoid inheriting an unrelated active session directory. */
    target?: { projectId: string; groupId?: string | null; sessionId?: string | null };
  }) => void;
  /** Changes an ephemeral terminal's in-memory shell; callers restart it to apply. */
  setEphemeralShell: (id: SessionId, shell: string | null) => void;
  /** Switches a terminal shell, persisting normal sessions or updating drafts in memory, then restarts if running. */
  switchSessionShell: (id: SessionId, shellPath: string) => Promise<void>;
}

/**
 * Agent states that trigger system notifications. Asking and waiting notify; working stays quiet.
 */
const NOTIFY_STATES: AgentState[] = ["asking", "waiting"];

/**
 * Last working timestamp per session, used for the 1200 ms working-to-idle hold. Keep it outside reactive
 * runtime state so every signal does not defeat value deduplication and cause unnecessary rerenders.
 */
const workingPulseAt = new Map<string, number>();

/** Localized notification text for each agent state. */
function agentNotifyText(state: AgentState): string {
  if (state === "working") return t("notify.working");
  if (state === "asking") return t("notify.asking");
  return t("notify.waiting");
}

/** Returns whether a session is visible in any pane of the active tab. */
function isVisibleSession(store: TermStore, sessionId: string): boolean {
  const tree = store.activeTabId ? store.paneTrees[store.activeTabId] : null;
  return !!tree && collectSessionIds(tree).includes(sessionId);
}

/**
 * Sends a system notification unless the session is currently visible in the focused window. Prefixes the
 * session name and returns whether notification state should be marked.
 */
function notifyRaw(
  store: TermStore,
  id: string,
  title: string | null | undefined,
  body: string,
): boolean {
  // Suppress only when the window is focused and the session is visible. Use reliable host-maintained
  // `windowFocused`; `document.hasFocus()` can remain true for an unfocused macOS WKWebView.
  const visible = isVisibleSession(store, id);
  if (store.windowFocused && visible) return false;
  const session =
    store.sessions.find((s) => s.id === id) ?? store.ephemeralSessions[id];
  const name = session?.name ?? t("common.session");
  const agent = store.runtimes[id]?.agent ?? "agent";
  const prefix = `${agent} · ${name}`;
  // Append an OSC 777 title after the session prefix; OSC 9 and agent state use the prefix alone.
  const heading = title ? `${prefix} · ${title}` : prefix;
  // When system notifications are disabled, still return true so unread UI and Dock badges remain active.
  if (store.notifyEnabled) void notify(id, heading, body, store.soundEnabled);
  return true;
}

/**
 * Builds an agent notification from the session name and returns whether it should be marked as unread.
 */
function notifyAgentState(
  store: TermStore,
  id: string,
  agentState: AgentState,
): boolean {
  if (!NOTIFY_STATES.includes(agentState)) return false;
  return notifyRaw(store, id, undefined, agentNotifyText(agentState));
}

/** Persists appearance and applies it to `documentElement`, resolving automatic accents against brightness. */
function persistAndApplyVisual(getState: () => TermStore) {
  const s = getState();
  const ps: PersistedSettings = {
    accent: s.accent,
    density: s.density,
    paneStyle: s.paneStyle,
    dividerStyle: s.dividerStyle,
    navLayout: s.navLayout,
    inspectorTab: s.inspectorTab,
    singleTabMode: s.singleTabMode,
    termRenderer: s.termRenderer,
    redrawOnReveal: s.redrawOnReveal,
    outputScheduler: s.outputScheduler,
    dynamicStatusFilter: s.dynamicStatusFilter,
    maxLiveTabs: s.maxLiveTabs,
    defaultShell: s.defaultShell,
    uiFontFamily: s.uiFontFamily,
    uiFontSize: s.uiFontSize,
    termFontFamily: s.termFontFamily,
    termFontSize: s.termFontSize,
    shortcutOverrides: s.shortcutOverrides,
    agentDefaults: s.agentDefaults,
    spawnConfirm: s.spawnConfirm,
    orchestrationProfiles: s.orchestrationProfiles,
    orchestration: s.orchestration,
    usageRefreshSec: s.usageRefreshSec,
    imagePasteMode: s.imagePasteMode,
  };
  saveSettings(ps);
  applyVisual(visualOf(ps));
}

/** Sidebar view writes are small but search updates arrive per keystroke, so coalesce them. */
let saveSidebarViewsTimer: ReturnType<typeof setTimeout> | undefined;
function saveSidebarViewsTick(getState: () => TermStore) {
  clearTimeout(saveSidebarViewsTimer);
  saveSidebarViewsTimer = setTimeout(() => {
    const state = getState();
    const payload: PersistedSidebarViewsV2 = {
      version: 2,
      // Status/marker filtering and the collapse map are written only for split-off panes. The main tree stores
      // just its search text, as it always did, so a restart gives it back unfiltered.
      views: state.sidebarTreeViews.map(({
        id,
        name,
        treeFilter,
        statusFilter,
        statusFilterIds,
        markFilter,
        collapsedOverrides,
      }) => (id === state.primarySidebarTreeViewId
        ? {
            id,
            name,
            treeFilter,
            statusFilter: null,
            statusFilterIds: null,
            markFilter: null,
            collapsedOverrides: null,
          }
        : { id, name, treeFilter, statusFilter, statusFilterIds, markFilter, collapsedOverrides })),
      tabs: state.sidebarTreeTabs,
      primaryId: state.primarySidebarTreeViewId,
      activeId: state.activeSidebarTreeViewId,
    };
    try {
      localStorage.setItem(SIDEBAR_VIEWS_KEY, JSON.stringify(payload));
    } catch {
      /* Ignore unavailable or full local storage. */
    }
  }, 200);
}

/**
 * Capture the collapse state every node currently shows in one view. A pane split off from it then starts out
 * looking identical while owning its state from that point on.
 */
function snapshotCollapsed(
  state: Pick<TermStore, "projects" | "groups" | "sessions" | "ephemeralSessions">,
  source: SidebarTreeView,
): Record<string, boolean> {
  const overrides = source.collapsedOverrides;
  const out: Record<string, boolean> = {};
  const put = (id: string, collapsed: boolean | undefined) => {
    out[id] = overrides && id in overrides ? overrides[id] : !!collapsed;
  };
  for (const project of state.projects) put(project.id, project.collapsed);
  for (const group of state.groups) put(group.id, group.collapsed);
  for (const session of state.sessions) put(session.id, session.collapsed);
  for (const [id, ephemeral] of Object.entries(state.ephemeralSessions)) {
    put(id, ephemeral.collapsed);
  }
  return out;
}

function statusSnapshot(
  state: Pick<TermStore, "sessions" | "runtimes" | "notifications">,
  filters: AgentState[] | null,
): Record<string, true> | null {
  if (!filters?.length) return null;
  const ids: Record<string, true> = {};
  for (const session of state.sessions) {
    const effective = effectiveStatus(state.runtimes[session.id]);
    const unread = session.id in state.notifications;
    if (filters.some((filter) => matchesAgentState(filter, effective, unread))) {
      ids[session.id] = true;
    }
  }
  return ids;
}

// Last brightness sent to agents, preventing duplicate notifications; the first frame establishes a baseline.
let lastNotifiedScheme: ResolvedTheme | null = null;

/**
 * Notifies running agent sessions after brightness changes so their themes update live.
 *
 * Agents such as Claude use OSC 11 for automatic brightness and re-query only after a DEC color-scheme
 * notification. Write that notification into the PTY so owner xterm answers with the newly applied background.
 *
 * Send only to Claude; plain shell line editors would insert these bytes as text. Call after `applyTheme`.
 */
function notifyAgentsColorScheme(getState: () => TermStore) {
  const s = getState();
  const resolved = resolveTheme(s.theme);
  if (resolved === lastNotifiedScheme) return;
  lastNotifiedScheme = resolved;
  const seq = resolved === "dark" ? "\x1b[?997;1n" : "\x1b[?997;2n";
  // Preindex sessions by ID to avoid repeated linear searches through live terminals.
  const sessById = new Map(s.sessions.map((x) => [x.id, x]));
  for (const id of liveTerminalIds()) {
    const sess = sessById.get(id) ?? s.ephemeralSessions[id];
    if (sess && sess.kind === "claude") {
      ptyWrite(id, seq).catch(() => {});
    }
  }
}

/**
 * Derives a display label for a terminal shell. Match a nonempty path to discovered shell labels, falling
 * back to the executable basename. Empty uses the backend-marked default, then the first option.
 */
export function shellDisplayName(shells: ShellOption[], effShell: string | null): string {
  if (effShell) {
    const hit = shells.find((s) => s.path === effShell);
    if (hit) return hit.label;
    const base = effShell.replace(/\\/g, "/").split("/").pop() ?? effShell;
    return base.replace(/\.exe$/i, "") || effShell;
  }
  const def = shells.find((s) => s.isDefault) ?? shells[0];
  return def?.label ?? t("kind.terminal");
}

/**
 * Returns the next terminal name by incrementing the highest existing suffix across persisted and scratch sessions.
 */
function nextTerminalName(sessions: Session[], ephemeral: Record<string, Session>): string {
  const label = t("kind.terminal");
  const re = new RegExp(`^${label} (\\d+)$`);
  const maxN = [...sessions, ...Object.values(ephemeral)]
    .filter((s) => s.kind === "terminal")
    .reduce((m, s) => {
      const mt = re.exec(s.name);
      return mt ? Math.max(m, Number(mt[1])) : m;
    }, 0);
  return `${label} ${maxN + 1}`;
}

const initialSidebarViews = loadSidebarViews();
const initialPrimarySidebarView =
  initialSidebarViews.views.find((view) => view.id === initialSidebarViews.primaryId) ??
  initialSidebarViews.views[0];

export const useTermStore = create<TermStore>((set, get) => ({
  projects: [],
  groups: [],
  sessions: [],
  archivedSessions: [],
  treeLoaded: false,
  runtimes: {},
  epochs: {},
  ephemeralSessions: {},
  pendingPrompts: {},
  pendingSpawns: [],
  mergeTarget: null,
  changesCwd: null,

  openTabs: [],
  activeTabId: null,
  lastActiveSessionTabId: null,
  paneTrees: {},
  activeSessionId: null,
  revealSuppressId: null,
  revealProjectId: null,
  focusedPaneId: null,
  liveTabs: [],
  pinnedTabs: [],
  liveEvictNotice: null,
  liveEvictAsk: false,
  docTabs: {},
  browserTabs: {},

  leftCollapsed: false,
  rightCollapsed: false,
  dirPickerOpen: false,
  createProjectModalOpen: false,
  settingsOpen: false,
  shareOpen: false,
  errorLogOpen: false,
  cloneModalOpen: false,
  saveAsRequest: null,
  leftWidth: 240,
  rightWidth: 280,
  bottomExpanded: false,
  theme: loadTheme(),
  sidebarTreeViews: initialSidebarViews.views,
  sidebarTreeTabs: initialSidebarViews.tabs,
  primarySidebarTreeViewId: initialSidebarViews.primaryId,
  activeSidebarTreeViewId: initialSidebarViews.activeId,
  treeFilter: initialPrimarySidebarView.treeFilter,
  statusFilter: initialPrimarySidebarView.statusFilter,
  statusFilterIds: initialPrimarySidebarView.statusFilterIds,
  markFilter: initialPrimarySidebarView.markFilter,
  searchOpen: false,
  notifyGuideOpen: false,
  archiveOpen: false,
  globalSearchOpen: false,
  selection: [],
  selectionAnchor: null,
  inspectTarget: null,
  notifications: {},
  windowFocused: true,
  soundEnabled: loadSoundEnabled(),
  notifyEnabled: loadNotifyEnabled(),
  cleanPastedImages: loadCleanPastedImages(),
  recordSessions: loadRecordSessions(),
  shells: [],

  ...loadSettings(),

  loadTree: async () => {
    const t = await tree.listTree();
    set((state) => {
      const runtimes = { ...state.runtimes };
      for (const s of t.sessions) {
        if (!runtimes[s.id]) runtimes[s.id] = { status: "idle" };
      }
      let layoutPatch = {};
      if (!layoutRestored) {
        // Restore layout from local storage on the first load only.
        layoutRestored = true;
        // Browser/remote mode restores the complete layout because server sessions survive page closure.
        // Desktop processes die with the app and therefore start with a clean layout.
        try {
          const raw = platform.env.isBrowser ? localStorage.getItem(LAYOUT_KEY) : null;
          if (raw) {
            const saved: PersistedLayout = JSON.parse(raw);
            // Restore ephemeral metadata before reconciliation so valid split leaves survive, with idle runtimes for rendering.
            const restoredEph = {
              ...state.ephemeralSessions,
              ...(saved.ephemeralSessions ?? {}),
            };
            for (const id of Object.keys(saved.ephemeralSessions ?? {})) {
              if (!runtimes[id]) runtimes[id] = { status: "idle" };
            }
            // Prune invalid leaves and repair active state through reconciliation.
            const reconciled = reconcileTabs({
              sessions: t.sessions,
              ephemeralSessions: restoredEph,
              openTabs: saved.openTabs,
              paneTrees: saved.paneTrees,
              activeTabId: saved.activeTabId,
              activeSessionId: saved.activeSessionId,
              focusedPaneId: saved.focusedPaneId,
              liveTabs: saved.liveTabs ?? [],
              // Document and browser tabs are not persisted across restart.
              docTabs: {},
              browserTabs: {},
            });
            // Restore visible tabs, background tabs, and split trees intact.
            layoutPatch = { ...reconciled, ephemeralSessions: restoredEph };
          }
        } catch {
          /* Invalid or obsolete layout data falls back to a clean start. */
        }
      } else {
        // Later tree refreshes reconcile current memory state only, preserving visible and background tabs.
        layoutPatch = reconcileTabs({
          sessions: t.sessions,
          ephemeralSessions: state.ephemeralSessions,
          openTabs: state.openTabs,
          paneTrees: state.paneTrees,
          activeTabId: state.activeTabId,
          activeSessionId: state.activeSessionId,
          focusedPaneId: state.focusedPaneId,
          liveTabs: state.liveTabs,
          docTabs: state.docTabs,
          browserTabs: state.browserTabs,
        });
      }
      const sidebarTreeViews = state.sidebarTreeViews;
      const primaryView =
        sidebarTreeViews.find((view) => view.id === state.primarySidebarTreeViewId) ??
        sidebarTreeViews[0];
      return {
        projects: t.projects,
        groups: t.groups,
        sessions: t.sessions,
        treeLoaded: true,
        runtimes,
        sidebarTreeViews,
        treeFilter: primaryView.treeFilter,
        statusFilter: primaryView.statusFilter,
        statusFilterIds: primaryView.statusFilterIds,
        markFilter: primaryView.markFilter,
        ...layoutPatch,
      };
    });
  },

  importProject: async () => {
    // Desktop shells use a native directory dialog; browser and remote windows use the server-side picker.
    // Do not key this on `hasNativeHost`, because remote windows can have a host without a usable native dialog.
    if (!isTauri && !platform.env.isElectron) {
      set({ dirPickerOpen: true });
      return;
    }
    const picked = await platform.dialog.pickDirectory();
    if (!picked) return; // User canceled.
    await get().openProjectPath(picked);
  },

  importProjectPath: async (rootPath) => {
    await get().openProjectPath(rootPath);
    set({ dirPickerOpen: false });
  },

  openProjectPath: async (rootPath) => {
    const project = await tree.importProject(rootPath);
    await tree.setCollapsed("project", project.id, false).catch(() => {});
    await get().loadTree();
    set((state) => ({
      projects: state.projects.map((p) =>
        p.id === project.id ? { ...p, collapsed: false } : p,
      ),
      selection: [{ id: project.id, kind: "project" }],
      selectionAnchor: project.id,
      inspectTarget: { id: project.id, kind: "project" },
      revealProjectId: project.id,
      leftCollapsed: false,
      archiveOpen: false,
      // Reveal requests clear the primary projection's conditions before the one-shot target is consumed.
      sidebarTreeViews: state.sidebarTreeViews.map((view) =>
        view.id === state.primarySidebarTreeViewId
          ? {
              ...view,
              treeFilter: "",
              statusFilter: null,
              statusFilterIds: null,
              markFilter: null,
            }
          : view),
      treeFilter: "",
      statusFilter: null,
      statusFilterIds: null,
      markFilter: null,
    }));
    saveSidebarViewsTick(get);
  },

  setDirPickerOpen: (open) => set({ dirPickerOpen: open }),
  setCreateProjectModalOpen: (open) => set({ createProjectModalOpen: open }),
  setSettingsOpen: (open) => set({ settingsOpen: open }),
  setShareOpen: (open) => set({ shareOpen: open }),
  setErrorLogOpen: (open) => set({ errorLogOpen: open }),

  setCloneModalOpen: (open) => set({ cloneModalOpen: open }),

  cloneProjectInto: async (url, parentDir, folderName, branch, operationId) => {
    // Let the dialog display Git errors; close and reload only after success.
    await tree.cloneProject(url, parentDir, folderName, branch, operationId);
    set({ cloneModalOpen: false });
    await get().loadTree();
  },

  promptSaveAs: (defaultName) =>
    new Promise<string | null>((resolve) => {
      set({ saveAsRequest: { defaultName, resolve } });
    }),

  addGroup: async (projectId, parentGroupId, name, worktree) => {
    await tree.createGroup(projectId, parentGroupId, name, worktree);
    // Expand the parent so the new group is not hidden under a collapsed node.
    if (parentGroupId) {
      await tree.setCollapsed("group", parentGroupId, false).catch(() => {});
    } else {
      await tree.setCollapsed("project", projectId, false).catch(() => {});
    }
    await get().loadTree();
  },

  addSession: async (input) => {
    const session = await tree.createSession(input);
    // Insert optimistically so `openSession` can launch the PTY immediately, without waiting for parent
    // expansion and a full-tree reload/render.
    set((state) => {
      if (state.sessions.some((s) => s.id === session.id)) return {};
      const runtimes = { ...state.runtimes };
      if (!runtimes[session.id]) runtimes[session.id] = { status: "idle" };
      // Optimistically expand the nearest parent so the session appears immediately; persist in the background.
      let { projects, groups, sessions } = state;
      if (input.parentSessionId) {
        sessions = sessions.map((s) =>
          s.id === input.parentSessionId && s.collapsed ? { ...s, collapsed: false } : s,
        );
      } else if (input.groupId) {
        groups = groups.map((g) =>
          g.id === input.groupId && g.collapsed ? { ...g, collapsed: false } : g,
        );
      } else {
        projects = projects.map((p) =>
          p.id === input.projectId && p.collapsed ? { ...p, collapsed: false } : p,
        );
      }
      // Suppress one automatic sidebar reveal when this newly created session becomes active.
      return { projects, groups, sessions: [...sessions, session], runtimes, revealSuppressId: session.id };
    });
    // Persist expansion and reload authoritative tree data in the background after the terminal can open.
    const expand = input.parentSessionId
      ? tree.setCollapsed("session", input.parentSessionId, false)
      : input.groupId
        ? tree.setCollapsed("group", input.groupId, false)
        : tree.setCollapsed("project", input.projectId, false);
    void Promise.resolve(expand)
      .catch(() => {})
      .then(() => get().loadTree())
      .catch(() => {});
    return session;
  },

  forkSession: async (id) => {
    const created = await tree.forkSession(id);
    await get().loadTree();
    // Suppress the initial sidebar reveal for the newly forked session.
    get().setRevealSuppress(created.id);
    // Follow single-tab policy: reuse the main slot and background the source, or open a new tab in multi-tab mode.
    get().openSession(created.id, { newTab: !get().singleTabMode });
  },

  handleSpawnRequest: async (req) => {
    // The backend threshold always wins over orchestration auto-approval.
    if (req.forceConfirm !== true && (req.autoApprove === true || !get().spawnConfirm)) {
      await get().executeSpawn(req);
      return;
    }
    set((s) => ({ pendingSpawns: [...s.pendingSpawns, req] }));
    // Return a progress result while the confirmation card is open.
    if (req.requestId) {
      void spawnResult(req.requestId, { awaitingConfirmation: true }).catch(() => {});
    }
    // A spawn-confirmation card always notifies when notifications are enabled, even in a focused window,
    // because the nonmodal card is easy to miss. Dock badges derive reactively from queue length elsewhere.
    const s = get();
    if (s.notifyEnabled) {
      const parent = s.sessions.find((x) => x.id === req.parentSessionId);
      const from = parent?.name ?? t("common.session");
      const preview = req.prompt.trim().replace(/\s+/g, " ").slice(0, 80);
      void notify(
        req.parentSessionId,
        t("spawn.notifyTitle"),
        `${from}: ${preview}`,
        s.soundEnabled,
      );
    }
  },

  confirmSpawn: async (req) => {
    // Remove the confirmed, possibly edited request before executing it.
    set((s) => ({ pendingSpawns: s.pendingSpawns.slice(1) }));
    await get().executeSpawn(req);
  },

  cancelSpawn: () => {
    // Cancel by removing the first request without creating a session, telling any parked
    // vagent spawn caller so it does not sit out its full timeout.
    const head = get().pendingSpawns[0];
    if (head?.requestId) {
      void spawnResult(head.requestId, { error: "cancelled by user" }).catch(() => {});
    }
    set((s) => ({ pendingSpawns: s.pendingSpawns.slice(1) }));
  },

  openMerge: (id) => set({ mergeTarget: id }),
  closeMerge: () => set({ mergeTarget: null }),
  openChanges: (cwd) => set({ changesCwd: cwd }),
  closeChanges: () => set({ changesCwd: null }),

  executeSpawn: async (req) => {
    // Answer a parked vagent spawn caller; fire-and-forget for plain vspawn requests.
    const report = (result: {
      sessionId?: string;
      error?: string;
      worktreeError?: string;
    }) => {
      if (req.requestId) void spawnResult(req.requestId, result).catch(() => {});
    };
    const state = get();
    const parent = state.sessions.find((s) => s.id === req.parentSessionId);
    if (!parent) {
      // Ignore a deleted or unknown parent session.
      report({ error: "parent session no longer exists" });
      return;
    }
    const project = state.projects.find((p) => p.id === parent.projectId);
    // Child kind preference: request, parent agent kind, then Claude.
    const fallbackKind =
      parent.kind === "codex"
        ? "codex"
        : parent.kind === "opencode"
          ? "opencode"
          : parent.kind === "copilot"
            ? "copilot"
            : parent.kind === "cursor"
              ? "cursor"
              : parent.kind === "antigravity"
                ? "antigravity"
                : parent.kind === "cline"
                  ? "cline"
                  : parent.kind === "pi"
                    ? "pi"
                    : parent.kind === "crush"
                      ? "crush"
                      : parent.kind === "kiro"
                        ? "kiro"
                      : parent.kind === "grok"
                        ? "grok"
                      : parent.kind === "zoo"
                        ? "zoo"
                      : "claude";
    const kind = ((req.kind ?? null) || fallbackKind) as Session["kind"];
    const name =
      req.name?.trim() || req.prompt.trim().slice(0, 24) || t("store.subtask");

    // By default, create an isolated worktree in the parent's repository.
    const repoRoot = parent.cwd || project?.rootPath || null;
    let cwd: string | null = parent.cwd ?? project?.rootPath ?? null;
    let worktreePath: string | null = null;
    let worktreeBaseRef: string | null = null;
    let worktreeError: string | undefined;
    if (req.worktree !== false && repoRoot) {
      try {
        const wt = await createWorktree(repoRoot, name);
        cwd = wt.path;
        worktreePath = wt.path;
        worktreeBaseRef = wt.baseRef || null;
      } catch (e) {
        // Worktree failure falls back to the parent directory without blocking the spawn.
        worktreeError = e instanceof Error ? e.message : String(e);
        console.warn("vspawn: worktree creation failed, using parent directory", e);
      }
    }

    // Explicit request values win; otherwise apply per-agent defaults, matching manual creation.
    // Permission mode additionally inherits from the parent so a lead running with skipped
    // confirmations gets workers that do the same instead of stalling on every approval.
    const defaults = state.agentDefaults[kind];
    const agentArgs = req.agentArgs?.trim() || defaults?.args || null;
    const permissionMode =
      req.permissionMode?.trim() ||
      parent.permissionMode ||
      defaults?.permissionMode ||
      null;

    let created: Session | null = null;
    try {
      created = await get().addSession({
        projectId: parent.projectId,
        groupId: parent.groupId ?? null,
        name,
        kind,
        cwd,
        parentSessionId: parent.id,
        worktreePath,
        worktreeBaseRef,
        agentArgs,
        permissionMode,
        model: req.model?.trim() || null,
        effort: req.effort?.trim() || null,
      });
    } catch (e) {
      report({ error: e instanceof Error ? e.message : String(e) });
      throw e;
    }
    if (!created) {
      report({ error: "session creation failed" });
      return;
    }
    report({ sessionId: created.id, worktreeError });

    // Store the prompt for `usePtySession` to inject as a positional launch argument, avoiding a timed later write.
    set((s) => ({
      pendingPrompts: { ...s.pendingPrompts, [created.id]: req.prompt },
    }));
    // Follow single-tab policy by backgrounding the parent without stopping it, or use a new tab in multi-tab mode.
    get().openSession(created.id, { newTab: !get().singleTabMode });
  },

  takePendingPrompt: (id) => {
    const p = get().pendingPrompts[id];
    if (p === undefined) return undefined;
    set((s) => {
      const rest = { ...s.pendingPrompts };
      delete rest[id];
      return { pendingPrompts: rest };
    });
    return p;
  },

  renameNode: async (kind, id, name) => {
    await tree.renameNode(kind, id, name);
    await get().loadTree();
  },

  setNodeMark: async (kind, id, mark) => {
    await tree.setNodeMark(kind, id, mark);
    await get().loadTree();
  },

  updateSession: async (id, input) => {
    await tree.updateSession(id, input);
    await get().loadTree();
  },

  clearNodeWorktree: async (kind, id) => {
    await tree.clearNodeWorktree(kind, id);
    await get().loadTree();
  },

  deleteNode: async (kind, id) => {
    await tree.deleteNode(kind, id);
    await get().loadTree();
    set((state) => reconcileTabs(state));
    saveLayoutTick();
  },

  deleteMany: async (nodes) => {
    // Delete sessions before groups and projects so parent removal cannot invalidate child IDs.
    const order = { session: 0, group: 1, project: 2 } as const;
    const sorted = [...nodes].sort((a, b) => order[a.kind] - order[b.kind]);
    for (const n of sorted) {
      await tree.deleteNode(n.kind, n.id).catch(() => {});
    }
    await get().loadTree();
    set((state) => ({ ...reconcileTabs(state), selection: [], selectionAnchor: null }));
    saveLayoutTick();
  },

  archiveSession: async (id) => {
    // Archive, reload, and reconcile so tabs unmount and stop processes without deleting session data.
    await tree.setSessionArchived(id, true);
    await get().loadTree();
    set((state) => reconcileTabs(state));
    saveLayoutTick();
  },

  archiveMany: async (ids) => {
    // Archive sequentially, then reload/reconcile/save once. Concurrent refreshes can destabilize virtualized
    // rows and trigger React's maximum-update-depth failure.
    for (const id of ids) {
      await tree.setSessionArchived(id, true).catch(() => {});
    }
    await get().loadTree();
    set((state) => ({ ...reconcileTabs(state), selection: [], selectionAnchor: null }));
    saveLayoutTick();
  },

  archiveGroup: async (id) => {
    // Group archive hides a tombstone and all children, then reloads, reconciles tabs, and saves layout once.
    await tree.archiveGroup(id);
    await get().loadTree();
    set((state) => reconcileTabs(state));
    saveLayoutTick();
  },

  restoreSession: async (id) => {
    // Clear the archive marker and refresh both the normal tree and archive list.
    await tree.setSessionArchived(id, false);
    await Promise.all([get().loadTree(), get().loadArchived()]);
  },

  loadArchived: async () => {
    try {
      const list = await tree.listArchivedSessions();
      set({ archivedSessions: list });
    } catch {
      /* Preserve the previous list if loading fails. */
    }
  },

  setArchiveOpen: (open) => {
    set({ archiveOpen: open });
    if (open) void get().loadArchived();
  },

  setGlobalSearchOpen: (open) => {
    set({ globalSearchOpen: open });
  },

  moveNode: async (
    kind,
    id,
    targetProjectId,
    targetGroupId,
    targetParentSessionId,
    sortOrder,
  ) => {
    await tree.moveNode(
      kind,
      id,
      targetProjectId,
      targetGroupId,
      targetParentSessionId,
      sortOrder,
    );
    await get().loadTree();
  },

  moveMany: async (ids, targetProjectId, targetGroupId, targetParentSessionId) => {
    // Move sequentially with increasing sort order, then reload once. Avoid concurrent refresh races and rerender loops.
    let order = Date.now();
    for (const id of ids) {
      await tree
        .moveNode("session", id, targetProjectId, targetGroupId, targetParentSessionId, order++)
        .catch(() => {});
    }
    await get().loadTree();
    set({ selection: [], selectionAnchor: null });
    saveLayoutTick();
  },

  selectSingle: (node) => set({ selection: [node], selectionAnchor: node.id }),

  toggleSelect: (node) =>
    set((state) => {
      const exists = state.selection.some((s) => s.id === node.id);
      return {
        selection: exists
          ? state.selection.filter((s) => s.id !== node.id)
          : [...state.selection, node],
        selectionAnchor: node.id,
      };
    }),

  setSelection: (nodes, anchor) =>
    set({ selection: nodes, selectionAnchor: anchor }),

  clearSelection: () => set({ selection: [], selectionAnchor: null }),

  setInspectTarget: (node) => set({ inspectTarget: node }),
  setRevealSuppress: (id) => set({ revealSuppressId: id }),
  setRevealProject: (id) => set({ revealProjectId: id }),

  toggleCollapsed: async (kind, id) => {
    if (kind === "project") {
      const cur = get().projects.find((p) => p.id === id);
      if (!cur) return;
      const next = !cur.collapsed;
      set((state) => ({
        projects: state.projects.map((p) =>
          p.id === id ? { ...p, collapsed: next } : p,
        ),
      }));
      tree.setCollapsed("project", id, next).catch(() => {});
    } else if (kind === "group") {
      const cur = get().groups.find((g) => g.id === id);
      if (!cur) return;
      const next = !cur.collapsed;
      set((state) => ({
        groups: state.groups.map((g) =>
          g.id === id ? { ...g, collapsed: next } : g,
        ),
      }));
      tree.setCollapsed("group", id, next).catch(() => {});
    } else {
      // Session collapse is visible only with children, but persistence is allowed for every session.
      const cur = get().sessions.find((s) => s.id === id);
      if (cur) {
        const next = !cur.collapsed;
        set((state) => ({
          sessions: state.sessions.map((s) =>
            s.id === id ? { ...s, collapsed: next } : s,
          ),
        }));
        tree.setCollapsed("session", id, next).catch(() => {});
        return;
      }
      // Ephemeral sessions keep collapse state in memory because they are not in the database.
      const eph = get().ephemeralSessions[id];
      if (!eph) return;
      set((state) => ({
        ephemeralSessions: {
          ...state.ephemeralSessions,
          [id]: { ...eph, collapsed: !eph.collapsed },
        },
      }));
    }
  },

  openSession: (id, opts) => {
    set((state) => {
      // Opening/focusing no longer clears notifications immediately; `useNotifications` waits two seconds.
      const notifications = state.notifications;

      // Browser nodes open/focus a center browser tab keyed by node ID, outside pane trees and desktop only.
      const sess = state.sessions.find((s) => s.id === id);
      if (sess?.kind === "browser") {
        // Only desktop shells can host native child browser views.
        if (!isTauri && !env.isElectron) return { notifications };
        if (state.browserTabs[id]) {
          return { notifications, activeTabId: id, activeSessionId: null, focusedPaneId: null };
        }
        const tab: BrowserTab = {
          id,
          url: sess.browserUrl || "about:blank",
          title: "",
          loading: false,
        };
        return {
          notifications,
          browserTabs: { ...state.browserTabs, [id]: tab },
          openTabs: [...state.openTabs, id],
          activeTabId: id,
          activeSessionId: null,
          focusedPaneId: null,
        };
      }

      // Focus an already visible tab instead of duplicating the session.
      const inOpen = locate(state.paneTrees, state.openTabs, id);
      if (inOpen) {
        return {
          notifications,
          liveTabs: state.liveTabs.filter((t) => t !== id),
          activeTabId: inOpen.tabId,
          lastActiveSessionTabId: inOpen.tabId,
          activeSessionId: id,
          focusedPaneId: inOpen.paneId,
        };
      }

      // Locate the target in a background tab whose whole split tree remains intact.
      const inLive = locate(state.paneTrees, state.liveTabs, id);

      // Single-tab reuse prefers the active reusable session tab, then the most recent one. Reusable means a
      // pane-tree session tab that is not pinned; document, browser, explicit-new-tab, and scratch tabs survive.
      const isReusableTab = (t: SessionId | null): t is SessionId =>
        t != null && state.paneTrees[t] != null && !state.pinnedTabs.includes(t);
      // Terminal sessions do not displace the agent main slot. They open pinned like scratch terminals; only
      // agent sessions reuse the single agent slot.
      const openedSess = sess ?? state.ephemeralSessions[id];
      const isTerminalKind = openedSess?.kind === "terminal";
      const reuseTabId =
        state.singleTabMode && !opts?.newTab && !isTerminalKind
          ? isReusableTab(state.activeTabId)
            ? state.activeTabId
            : (state.openTabs.find((t) => isReusableTab(t)) ?? null)
          : null;
      const canReuse = reuseTabId != null;

      if (canReuse) {
        // Reuse the current slot browser-style, moving the old complete pane tree into `liveTabs` intact.
        const oldTabId = reuseTabId!;
        const oldTree = state.paneTrees[oldTabId];
        const idx = state.openTabs.indexOf(oldTabId);
        const openTabs = [...state.openTabs];
        const paneTrees = { ...state.paneTrees };
        let liveTabs = state.liveTabs.filter((t) => t !== id);

        // Restore the complete target tree from background, or create a single pane.
        let newTabId: SessionId;
        let focusedPaneId: string;
        if (inLive) {
          newTabId = inLive.tabId;
          focusedPaneId = inLive.paneId;
          liveTabs = liveTabs.filter((t) => t !== newTabId);
        } else {
          const leaf = makeLeaf(id);
          newTabId = id;
          paneTrees[id] = leaf;
          focusedPaneId = leaf.paneId;
        }

        // Preindex sessions by ID for named-session checks and eviction labels.
        const sessById = new Map(state.sessions.map((s) => [s.id, s]));
        // Background only tabs containing a named session; discard purely ephemeral tabs for later pruning.
        const oldHasNamed = oldTree
          ? collectSessionIds(oldTree).some((sid) => sessById.has(sid))
          : false;
        if (oldHasNamed) {
          if (!liveTabs.includes(oldTabId)) liveTabs.push(oldTabId);
        } else {
          delete paneTrees[oldTabId];
        }
        // On overflow, evict the oldest inactive background tab and show a notice. If all are active, ask
        // through `LiveTabsOverLimitDialog` and preserve overflow when declined.
        let liveEvictNotice = state.liveEvictNotice;
        let liveEvictAsk = false;
        while (liveTabs.length > state.maxLiveTabs) {
          const evicted = pickEvictTab(liveTabs, paneTrees, state.runtimes, notifications);
          if (!evicted) {
            liveEvictAsk = true;
            break;
          }
          liveTabs = liveTabs.filter((tid) => tid !== evicted);
          const evictedTree = paneTrees[evicted];
          const label =
            (evictedTree
              ? collectSessionIds(evictedTree)
                  .map((sid) => sessById.get(sid)?.name)
                  .filter((n): n is string => !!n)
                  .join(" ⫽ ")
              : "") || evicted;
          delete paneTrees[evicted];
          liveEvictNotice = { label, at: Date.now() };
        }
        openTabs[idx] = newTabId;

        return {
          notifications,
          openTabs,
          paneTrees,
          liveTabs,
          liveEvictNotice,
          liveEvictAsk,
          activeTabId: newTabId,
          lastActiveSessionTabId: newTabId,
          activeSessionId: id,
          focusedPaneId,
        };
      }

      // Opening in a new tab restores a background tree or creates one pane. Explicit new tabs and Terminal
      // tabs are pinned; a tab created only because no reusable slot exists becomes the next main slot.
      const pin = !!opts?.newTab || (state.singleTabMode && isTerminalKind);
      if (inLive) {
        return {
          notifications,
          liveTabs: state.liveTabs.filter((t) => t !== inLive.tabId),
          openTabs: [...state.openTabs, inLive.tabId],
          pinnedTabs: pin ? [...state.pinnedTabs, inLive.tabId] : state.pinnedTabs,
          activeTabId: inLive.tabId,
          lastActiveSessionTabId: inLive.tabId,
          activeSessionId: id,
          focusedPaneId: inLive.paneId,
        };
      }
      const leaf = makeLeaf(id);
      return {
        notifications,
        openTabs: [...state.openTabs, id],
        pinnedTabs: pin ? [...state.pinnedTabs, id] : state.pinnedTabs,
        paneTrees: { ...state.paneTrees, [id]: leaf },
        activeTabId: id,
        lastActiveSessionTabId: id,
        activeSessionId: id,
        focusedPaneId: leaf.paneId,
      };
    });
    get().pruneEphemeral();
    saveLayoutTick();
  },

  setActiveTab: (tabId) => {
    set((state) => {
      if (state.docTabs[tabId] || state.browserTabs[tabId]) {
        // Document/browser tabs have no session or focused pane, naturally disabling session-only controls.
        return { activeTabId: tabId, activeSessionId: null, focusedPaneId: null };
      }
      const t = state.paneTrees[tabId];
      const leaf = t ? firstLeaf(t) : null;
      return {
        activeTabId: tabId,
        // Update the reusable-session anchor only for session tabs, preserving it while viewing documents/browsers.
        lastActiveSessionTabId: t ? tabId : state.lastActiveSessionTabId,
        activeSessionId: leaf?.sessionId ?? tabId,
        focusedPaneId: leaf?.paneId ?? null,
      };
    });
    saveLayoutTick();
  },

  reorderTab: (tabId, targetId, side) => {
    let changed = false;
    set((state) => {
      if (tabId === targetId) return {};
      if (!state.openTabs.includes(tabId) || !state.openTabs.includes(targetId)) return {};
      // Remove the dragged tab, then insert it before or after the target's current position.
      const openTabs = state.openTabs.filter((t) => t !== tabId);
      const at = openTabs.indexOf(targetId) + (side === "after" ? 1 : 0);
      openTabs.splice(at, 0, tabId);
      if (openTabs.every((t, i) => t === state.openTabs[i])) return {}; // Avoid rerender when order is unchanged.
      changed = true;
      return { openTabs };
    });
    if (changed) saveLayoutTick();
  },

  closeTab: (tabId) => {
    // Explicit tab closure terminates all contained sessions here. Unmount also serves automatic detach-only
    // flows in browser mode, so it cannot express user intent reliably. Repeated desktop kill is idempotent.
    const closingTree = get().paneTrees[tabId];
    if (closingTree) {
      for (const sid of collectSessionIds(closingTree)) {
        void ptyKill(sid).catch(() => {});
      }
    }
    set((state) => {
      const idx = state.openTabs.indexOf(tabId);
      const openTabs = state.openTabs.filter((t) => t !== tabId);
      const paneTrees = { ...state.paneTrees };
      delete paneTrees[tabId];
      // Remove document/browser metadata when those tabs close.
      const docTabs = { ...state.docTabs };
      delete docTabs[tabId];
      const browserTabs = { ...state.browserTabs };
      delete browserTabs[tabId];

      let { activeTabId, activeSessionId, focusedPaneId } = state;
      if (activeTabId === tabId) {
        const nextTab = openTabs[idx] ?? openTabs[idx - 1] ?? null;
        activeTabId = nextTab;
        if (nextTab && paneTrees[nextTab]) {
          const leaf = firstLeaf(paneTrees[nextTab]);
          activeSessionId = leaf.sessionId;
          focusedPaneId = leaf.paneId;
        } else {
          // A document/browser next tab, or no tab, means no active session.
          activeSessionId = null;
          focusedPaneId = null;
        }
      }
      // Retarget a closed reuse anchor to the current or any remaining visible session tab.
      let lastActiveSessionTabId = state.lastActiveSessionTabId;
      if (lastActiveSessionTabId === tabId) {
        lastActiveSessionTabId =
          activeTabId && paneTrees[activeTabId]
            ? activeTabId
            : (openTabs.find((t) => paneTrees[t]) ?? null);
      }
      const pinnedTabs = state.pinnedTabs.filter((t) => t !== tabId);
      return { openTabs, paneTrees, docTabs, browserTabs, pinnedTabs, activeTabId, lastActiveSessionTabId, activeSessionId, focusedPaneId };
    });
    get().pruneEphemeral();
    saveLayoutTick();
  },

  // ── Document tabs opened by the built-in `view` editor ──
  openDocTab: (path) => {
    set((state) => {
      // Focus and reload an already-open canonical path because another `view` request asks for current content.
      const existing = Object.values(state.docTabs).find((d) => d.path === path);
      if (existing) {
        return {
          docTabs: {
            ...state.docTabs,
            [existing.id]: { ...existing, reloadNonce: existing.reloadNonce + 1 },
          },
          activeTabId: existing.id,
          activeSessionId: null,
          focusedPaneId: null,
        };
      }
      const tab = makeDocTab(path);
      return {
        docTabs: { ...state.docTabs, [tab.id]: tab },
        openTabs: [...state.openTabs, tab.id],
        activeTabId: tab.id,
        activeSessionId: null,
        focusedPaneId: null,
      };
    });
    saveLayoutTick();
  },

  newDocTab: () => {
    set((state) => {
      // Drafts are not path-deduplicated; assign a unique Untitled title.
      const drafts = new Set(
        Object.values(state.docTabs).filter((d) => d.isNew).map((d) => d.title),
      );
      let title = "Untitled";
      for (let n = 2; drafts.has(title); n++) title = `Untitled-${n}`;
      const id = `doc-${genId()}`;
      const tab: DocTab = {
        id,
        path: "",
        title,
        kind: "code", // Start as plain text and reclassify after saving by file extension.
        mode: "source",
        dirty: false,
        pendingClose: false,
        reloadNonce: 0,
        isNew: true,
      };
      return {
        docTabs: { ...state.docTabs, [id]: tab },
        openTabs: [...state.openTabs, id],
        activeTabId: id,
        activeSessionId: null,
        focusedPaneId: null,
      };
    });
    saveLayoutTick();
  },

  setDocTabPath: (id, path) =>
    set((state) => {
      const tab = state.docTabs[id];
      if (!tab) return {};
      const title = path.split("/").pop() || path;
      return {
        docTabs: {
          ...state.docTabs,
          [id]: { ...tab, path, title, kind: docKindOf(path), isNew: false },
        },
      };
    }),

  refreshDocTab: (id) =>
    set((state) =>
      state.docTabs[id]
        ? {
            docTabs: {
              ...state.docTabs,
              [id]: { ...state.docTabs[id], reloadNonce: state.docTabs[id].reloadNonce + 1 },
            },
          }
        : {},
    ),

  setDocTabMode: (id, mode) =>
    set((state) =>
      state.docTabs[id]
        ? { docTabs: { ...state.docTabs, [id]: { ...state.docTabs[id], mode } } }
        : {},
    ),

  setDocTabDirty: (id, dirty) =>
    set((state) =>
      state.docTabs[id]
        ? { docTabs: { ...state.docTabs, [id]: { ...state.docTabs[id], dirty } } }
        : {},
    ),

  requestCloseDocTab: (id) => {
    const tab = get().docTabs[id];
    if (!tab) return;
    if (!tab.dirty) {
      get().closeTab(id);
      return;
    }
    set((state) => ({
      docTabs: { ...state.docTabs, [id]: { ...state.docTabs[id], pendingClose: true } },
    }));
  },

  cancelCloseDocTab: (id) =>
    set((state) =>
      state.docTabs[id]
        ? {
            docTabs: {
              ...state.docTabs,
              [id]: { ...state.docTabs[id], pendingClose: false },
            },
          }
        : {},
    ),

  // ── Browser tabs, desktop only ──
  openBrowserTab: (url) => {
    const tab: BrowserTab = {
      id: `browser-${genId()}`,
      url: url ?? "about:blank",
      title: "",
      loading: false,
    };
    set((state) => ({
      browserTabs: { ...state.browserTabs, [tab.id]: tab },
      openTabs: [...state.openTabs, tab.id],
      activeTabId: tab.id,
      activeSessionId: null,
      focusedPaneId: null,
    }));
    saveLayoutTick();
  },

  applyBrowserState: (id, patch) => {
    set((state) =>
      state.browserTabs[id]
        ? { browserTabs: { ...state.browserTabs, [id]: { ...state.browserTabs[id], ...patch } } }
        : {},
    );
    // Persist the last URL for tree-bound browser tabs with a debounce; independent `browser-` tabs are drafts.
    if (!patch.url || patch.url === "about:blank" || id.startsWith("browser-")) return;
    const sess = get().sessions.find((s) => s.id === id);
    if (sess?.kind !== "browser") return;
    clearTimeout(browserUrlTimers.get(id));
    browserUrlTimers.set(
      id,
      setTimeout(() => {
        browserUrlTimers.delete(id);
        const url = get().browserTabs[id]?.url;
        if (!url || url === "about:blank") return;
        if (get().sessions.find((s) => s.id === id)?.browserUrl === url) return;
        void setBrowserUrl(id, url).catch(() => {});
        // Update local sessions immediately to avoid duplicate writes during the debounce window.
        set((state) => ({
          sessions: state.sessions.map((s) => (s.id === id ? { ...s, browserUrl: url } : s)),
        }));
      }, 1000),
    );
  },

  closeLiveTab: (tabId) => {
    // Manual background-tab closure expresses termination intent and must kill server processes in browser mode.
    // Automatic overflow eviction remains detach-only because it is not an explicit user close.
    if (get().liveTabs.includes(tabId)) {
      const closingTree = get().paneTrees[tabId];
      if (closingTree) {
        for (const sid of collectSessionIds(closingTree)) {
          void ptyKill(sid).catch(() => {});
        }
      }
    }
    set((state) => {
      if (!state.liveTabs.includes(tabId)) return {};
      const paneTrees = { ...state.paneTrees };
      delete paneTrees[tabId];
      return { paneTrees, liveTabs: state.liveTabs.filter((t) => t !== tabId) };
    });
    get().pruneEphemeral();
    saveLayoutTick();
  },

  moveTabToBackground: (tabId) => {
    set((state) => {
      // Only visible pane-tree tabs containing a named session can move to the background.
      if (!state.openTabs.includes(tabId)) return {};
      const tree = state.paneTrees[tabId];
      if (!tree) return {};
      const sessById = new Map(state.sessions.map((s) => [s.id, s]));
      const hasNamed = collectSessionIds(tree).some((sid) => sessById.has(sid));
      if (!hasNamed) return {};

      const idx = state.openTabs.indexOf(tabId);
      const openTabs = state.openTabs.filter((t) => t !== tabId);
      // Remove background tabs from the pinned set because they no longer participate in reuse selection.
      const pinnedTabs = state.pinnedTabs.filter((t) => t !== tabId);
      // Append uniquely to `liveTabs`, preserving the complete pane tree.
      let liveTabs = state.liveTabs.filter((t) => t !== tabId);
      liveTabs.push(tabId);
      const paneTrees = { ...state.paneTrees };

      // If active, transfer focus to a neighboring visible tab.
      let { activeTabId, activeSessionId, focusedPaneId, lastActiveSessionTabId } = state;
      if (activeTabId === tabId) {
        const nextTab = openTabs[idx] ?? openTabs[idx - 1] ?? null;
        activeTabId = nextTab;
        if (nextTab && paneTrees[nextTab]) {
          const leaf = firstLeaf(paneTrees[nextTab]);
          activeSessionId = leaf.sessionId;
          focusedPaneId = leaf.paneId;
        } else {
          // A document/browser neighbor, or no neighbor, means no active session.
          activeSessionId = null;
          focusedPaneId = null;
        }
      }
      // Retarget the reuse anchor to the current or another visible session tab.
      if (lastActiveSessionTabId === tabId) {
        lastActiveSessionTabId =
          activeTabId && paneTrees[activeTabId]
            ? activeTabId
            : (openTabs.find((t) => paneTrees[t]) ?? null);
      }

      // Apply standard overflow eviction; the newly backgrounded tab sits at the tail and is evicted last.
      let liveEvictNotice = state.liveEvictNotice;
      let liveEvictAsk = false;
      while (liveTabs.length > state.maxLiveTabs) {
        const evicted = pickEvictTab(liveTabs, paneTrees, state.runtimes, state.notifications);
        if (!evicted) {
          liveEvictAsk = true;
          break;
        }
        liveTabs = liveTabs.filter((tid) => tid !== evicted);
        const evictedTree = paneTrees[evicted];
        const label =
          (evictedTree
            ? collectSessionIds(evictedTree)
                .map((sid) => sessById.get(sid)?.name)
                .filter((n): n is string => !!n)
                .join(" ⫽ ")
            : "") || evicted;
        delete paneTrees[evicted];
        liveEvictNotice = { label, at: Date.now() };
      }

      return {
        openTabs,
        pinnedTabs,
        liveTabs,
        paneTrees,
        activeTabId,
        activeSessionId,
        focusedPaneId,
        lastActiveSessionTabId,
        liveEvictNotice,
        liveEvictAsk,
      };
    });
    get().pruneEphemeral();
    saveLayoutTick();
  },

  clearLiveEvictNotice: () => set({ liveEvictNotice: null }),

  dismissLiveEvictAsk: () => set({ liveEvictAsk: false }),

  focusPane: (paneId, sessionId) => {
    set({ activeSessionId: sessionId, focusedPaneId: paneId });
    saveLayoutTick();
  },

  splitNew: async (direction) => {
    const { activeTabId, focusedPaneId, activeSessionId } = get();
    if (!activeTabId || !focusedPaneId || !activeSessionId) return;
    const focused =
      get().sessions.find((s) => s.id === activeSessionId) ??
      get().ephemeralSessions[activeSessionId];
    if (!focused) return;

    // Inherit the focused pane's runtime working directory.
    let cwd: string | null = null;
    try {
      cwd = await getSessionCwd(activeSessionId);
    } catch {
      /* Fall back when the session is not running or the query fails. */
    }

    // Split panes create in-memory plain terminals only, avoiding an unexpected second agent launch.
    const id = `eph-${genId()}`;
    const ephemeral: Session = {
      id,
      projectId: focused.projectId,
      groupId: focused.groupId ?? null,
      name: t("store.splitPane"),
      kind: "terminal",
      shell: focused.shell ?? null,
      cwd: cwd ?? focused.cwd ?? null,
      envJson: null,
      initCmd: null,
      hotkey: null,
      // Attach a split terminal beneath its focused source session so the sidebar reflects its origin.
      parentSessionId: focused.id,
      // Ephemeral sessions start collapsed so nested splits do not automatically expand the Scratch tree.
      collapsed: true,
      worktreePath: null,
      sortOrder: 0,
      // Use the current Unix time for frontend-only sessions instead of displaying the epoch.
      createdAt: Math.floor(Date.now() / 1000),
    };

    set((state) => {
      const curTree = state.paneTrees[activeTabId];
      if (!curTree) return {};
      const newTree = splitAt(curTree, focusedPaneId, direction, id);
      const newLeaf = findBySession(newTree, id);
      return {
        paneTrees: { ...state.paneTrees, [activeTabId]: newTree },
        ephemeralSessions: { ...state.ephemeralSessions, [id]: ephemeral },
        runtimes: { ...state.runtimes, [id]: { status: "idle" } },
        activeSessionId: id,
        focusedPaneId: newLeaf?.paneId ?? state.focusedPaneId,
      };
    });
    saveLayoutTick();
  },

  closePane: () => {
    const { activeTabId, focusedPaneId, paneTrees } = get();
    if (!activeTabId || !focusedPaneId) return;
    const t = paneTrees[activeTabId];
    if (!t) return;
    const removed = removeLeaf(t, focusedPaneId);
    if (removed === null) {
      // Closing the last pane closes the entire tab.
      get().closeTab(activeTabId);
      return;
    }
    set((state) => {
      const leaf = firstLeaf(removed);
      return {
        paneTrees: { ...state.paneTrees, [activeTabId]: removed },
        activeSessionId: leaf.sessionId,
        focusedPaneId: leaf.paneId,
      };
    });
    get().pruneEphemeral();
    saveLayoutTick();
  },

  closeSession: (sessionId) => {
    // Remove the exited process's pane, closing its tab when last, while leaving the app in an empty state.
    const { paneTrees, openTabs, liveTabs } = get();
    const loc = locate(paneTrees, openTabs, sessionId);
    if (!loc) {
      // If the pane belongs to a background tab, prune it there and remove the tab if its tree becomes empty.
      const bg = locate(paneTrees, liveTabs, sessionId);
      if (bg) {
        const t = paneTrees[bg.tabId];
        const removed = t ? removeLeaf(t, bg.paneId) : null;
        set((s) => {
          const pt = { ...s.paneTrees };
          let lt = s.liveTabs;
          if (removed === null) {
            delete pt[bg.tabId];
            lt = s.liveTabs.filter((x) => x !== bg.tabId);
          } else {
            pt[bg.tabId] = removed;
          }
          return { paneTrees: pt, liveTabs: lt };
        });
        get().pruneEphemeral();
        saveLayoutTick();
      }
      return;
    }
    const { tabId, paneId } = loc;
    const t = paneTrees[tabId];
    if (!t) return;
    const removed = removeLeaf(t, paneId);
    if (removed === null) {
      get().closeTab(tabId);
    } else {
      set((state) => {
        const reassign =
          state.activeTabId === tabId && state.focusedPaneId === paneId;
        const leaf = reassign ? firstLeaf(removed) : null;
        return {
          paneTrees: { ...state.paneTrees, [tabId]: removed },
          ...(leaf
            ? { activeSessionId: leaf.sessionId, focusedPaneId: leaf.paneId }
            : {}),
        };
      });
      get().pruneEphemeral();
    }
    saveLayoutTick();
  },

  collapseToFocused: () => {
    const { activeTabId, activeSessionId } = get();
    if (!activeTabId || !activeSessionId) return;
    const leaf = makeLeaf(activeSessionId);
    set((state) => ({
      paneTrees: { ...state.paneTrees, [activeTabId]: leaf },
      focusedPaneId: leaf.paneId,
    }));
    get().pruneEphemeral();
    saveLayoutTick();
  },

  persistSession: async (id, override) => {
    const eph = get().ephemeralSessions[id];
    if (!eph) return;
    // Placement uses an explicit drag target or preserves the ephemeral session's existing parent/project.
    const projectId = override?.projectId ?? eph.projectId;
    const groupId = override ? override.groupId : (eph.groupId ?? null);
    const parentSessionId = override ? override.parentSessionId : (eph.parentSessionId ?? null);
    // Persist the existing ID so the PTY continues running without restart or context loss.
    const created = await tree.persistSession({
      id,
      projectId,
      groupId,
      name: eph.name,
      kind: eph.kind,
      shell: eph.shell ?? null,
      cwd: eph.cwd ?? null,
      initCmd: eph.initCmd ?? null,
      parentSessionId,
    });
    // Atomically move from ephemeral to persistent state and unpin. A gap between collections could let
    // reconciliation prune the leaf and kill its PTY.
    set((state) => {
      const ephemeralSessions = { ...state.ephemeralSessions };
      delete ephemeralSessions[id];
      return {
        ephemeralSessions,
        sessions: [...state.sessions, created],
        pinnedTabs: state.pinnedTabs.filter((t) => t !== id),
      };
    });
    saveLayoutTick();
  },

  persistBrowserDraft: async (tabId, override) => {
    const tab = get().browserTabs[tabId];
    if (!tab) return;
    const st = get();
    // Project priority: drag override, active session project, then first project; abort when none exists.
    const active = st.activeSessionId
      ? st.sessions.find((s) => s.id === st.activeSessionId) ??
        st.ephemeralSessions[st.activeSessionId] ??
        null
      : null;
    const projectId = override?.projectId ?? active?.projectId ?? st.projects[0]?.id ?? null;
    if (!projectId) return;
    const groupId = override?.groupId ?? null;
    const parentSessionId = override?.parentSessionId ?? null;
    const name = tab.title?.trim() || t("kind.browser");
    // Reuse browser-node creation with a new UUID, then persist the draft's current URL for reopening.
    const node = await get().addSession({
      projectId,
      groupId,
      name,
      kind: "browser",
      parentSessionId,
    });
    if (!node) return;
    const url = tab.url;
    if (url && url !== "about:blank") {
      void setBrowserUrl(node.id, url).catch(() => {});
      // Update local `browserUrl` because the backend setter does not broadcast a tree change.
      set((state) => ({
        sessions: state.sessions.map((s) => (s.id === node.id ? { ...s, browserUrl: url } : s)),
      }));
    }
    get().closeTab(tabId); // Close the draft and destroy its child WebView.
    get().openSession(node.id); // Open the persisted node at its URL with shared login state.
  },

  renameScratch: (id, name) => {
    const nm = name.trim();
    if (!nm) return;
    set((state) => {
      if (state.ephemeralSessions[id]) {
        return {
          ephemeralSessions: {
            ...state.ephemeralSessions,
            [id]: { ...state.ephemeralSessions[id], name: nm },
          },
        };
      }
      if (state.browserTabs[id]) {
        return { browserTabs: { ...state.browserTabs, [id]: { ...state.browserTabs[id], title: nm } } };
      }
      if (state.docTabs[id]) {
        return { docTabs: { ...state.docTabs, [id]: { ...state.docTabs[id], title: nm } } };
      }
      return {};
    });
    saveLayoutTick();
  },

  pruneEphemeral: () =>
    set((state) => {
      // Remove ephemeral sessions and runtimes no longer referenced by visible or background pane trees.
      const used = new Set<string>();
      for (const tabId of [...state.openTabs, ...state.liveTabs]) {
        const t = state.paneTrees[tabId];
        if (t) for (const sid of collectSessionIds(t)) used.add(sid);
      }
      const ephemeralSessions: Record<string, Session> = {};
      const runtimes = { ...state.runtimes };
      for (const [id, s] of Object.entries(state.ephemeralSessions)) {
        if (used.has(id)) ephemeralSessions[id] = s;
        else delete runtimes[id];
      }
      return { ephemeralSessions, runtimes };
    }),

  setRuntime: (id, partial) => {
    // Value-level deduplication avoids replacing `runtimes` or notifying subscribers when all fields match.
    const prev = get().runtimes[id];
    if (
      prev &&
      Object.entries(partial).every(([k, v]) =>
        Object.is((prev as unknown as Record<string, unknown>)[k], v),
      )
    ) {
      return;
    }
    set((state) => ({
      runtimes: {
        ...state.runtimes,
        [id]: { ...state.runtimes[id], ...partial },
      },
    }));
  },

  applyStatusSignal: (id, signal) => {
    // The backend hook service now derives titles from the first prompt, writes once, and broadcasts the tree change.
    //
    // `resized` belongs to `usePtySession` fit/mirror state and is ignored by the agent state machine.
    if (signal.kind === "resized") return;
    // `agent_missing` is consumed directly by `usePtySession`, not the agent state machine.
    if (signal.kind === "agent_missing") return;
    const prev = get().runtimes[id] ?? { status: "idle" as const };
    const next: Partial<SessionRuntime> = {};
    // Read the module-level working timestamp before the switch for state-transition debouncing.
    const pulse = workingPulseAt.get(id);

    switch (signal.kind) {
      case "state":
        // Accept structured hook/notify state directly; full-authority locking is handled below.
        next.agentState = signal.state;
        if (prev.agent === "codex" && signal.authoritative) next.agentHookReady = true;
        // Legacy Codex notify reports only waiting and cannot lock authority. Lifecycle hooks mark every phase
        // authoritative, after which screen/busy fallbacks cannot overwrite Stop's waiting state.
        if (prev.agent !== "codex" || signal.authoritative) next.authoritative = true;
        // Track working outside reactive state for screen-transition prerequisites without defeating deduplication.
        if (signal.state === "working") {
          next.everWorked = true;
          workingPulseAt.set(id, Date.now());
        }
        // Hold idle/waiting transitions for 1200 ms after working to prevent visible state flicker.
        if (
          signal.state !== "working" &&
          prev.agentState === "working" &&
          pulse &&
          Date.now() - pulse < 1200
        ) {
          const holdMs = 1200 - (Date.now() - pulse);
          // Delay the agent-state update until the hold expires.
          const heldState = signal.state;
          setTimeout(() => {
            const cur = useTermStore.getState().runtimes[id];
            // Apply only if no newer event moved the session away from working.
            if (cur?.agentState === "working") {
              useTermStore.getState().applyStatusSignal(id, {
                kind: "state",
                state: heldState,
                // Carry silence through the hold as well; a correction such as the Codex silence heal
                // must never turn into a "replied" notification just because it was delayed.
                silent: signal.silent,
                // Preserve full authority through a delayed Codex Stop so stale fallbacks cannot restore working.
                authoritative: signal.authoritative,
              });
            }
          }, holdMs);
          return; // Do not update immediately.
        }
        break;
      case "agent":
        // Set or clear agent kind from typed spawn or Terminal fallback detection. Codex declares its state
        // source at launch: hook-capable sessions become authoritative immediately, before SessionStart arrives,
        // so screen/output guesses can never win a startup race. Clear any stale state replayed by hot reload;
        // the cached lifecycle snapshot follows this agent marker and restores the latest real hook state.
        next.agent = signal.agent;
        if (signal.agent === "codex") {
          next.agentStateSource = signal.stateSource ?? "legacy";
          next.authoritative = signal.stateSource === "hooks";
          next.agentState = null;
          next.agentHookReady = signal.stateSource === "hooks" ? false : undefined;
          next.busy = false;
        } else if (!signal.agent) {
          next.agentState = null;
          next.agentStateSource = undefined;
          next.agentHookReady = undefined;
        }
        break;
      case "hook_ready":
        // SessionStart proves the modern Codex hook chain without inventing an activity state or notification.
        if (prev.agent === "codex" && prev.agentStateSource === "hooks") {
          next.agentHookReady = true;
        }
        break;
      case "title":
        next.title = signal.title;
        break;
      case "tool":
        // Track the current/recent tool from Claude PreToolUse; clear at Stop, not PostToolUse, to avoid flicker.
        next.currentTool = signal.tool;
        break;
      case "busy":
        // Use activity-based working/waiting only for fallback agents before an authoritative event arrives.
        // Codex never consumes this signal: modern versions are hook-only, while legacy output does not justify
        // pretending that an exact working/waiting state is known.
        next.busy = signal.busy;
        if (prev.agent && prev.agent !== "codex" && !prev.authoritative) {
          next.agentState = signal.busy ? "working" : "waiting";
          if (signal.busy) next.everWorked = true;
        }
        break;
      case "bell":
        // Bell is supplemental and no longer participates in work-state detection.
        break;
      case "notify": {
        // OSC 9/777 is a fallback only for sessions without authoritative sources, preventing duplicate hook/OSC notifications.
        if (prev.authoritative) {
          return;
        }
        // Reuse visibility suppression with OSC-provided title/body and record the notification timestamp.
        const popped = notifyRaw(get(), id, signal.title, signal.body);
        if (popped) {
          set((state) => ({
            notifications: { ...state.notifications, [id]: Date.now() },
          }));
        }
        // OSC notifications do not alter agent state.
        return;
      }
    }

    // Value-level deduplication blocks unchanged signal storms without replacing runtimes or rerendering subscribers.
    const dirty = Object.entries(next).some(
      ([k, v]) => !Object.is((prev as unknown as Record<string, unknown>)[k], v),
    );
    if (dirty) {
      set((state) => ({
        runtimes: { ...state.runtimes, [id]: { ...state.runtimes[id], ...next } },
      }));
    }

    // Only authoritative, nonsilent state changes notify. Busy fallbacks and replayed/idle correction stay quiet.
    if (
      signal.kind === "state" &&
      !signal.silent &&
      next.agentState != null &&
      next.agentState !== prev.agentState &&
      NOTIFY_STATES.includes(next.agentState)
    ) {
      // Mark unread in the sidebar only when notification handling succeeds.
      if (notifyAgentState(get(), id, next.agentState)) {
        set((state) => ({
          notifications: { ...state.notifications, [id]: Date.now() },
        }));
      }
    }
  },

  applyScreenDetection: (id, screen) => {
    if (screen.skip) return;

    const rt = get().runtimes[id];
    if (!rt) return;

    // Codex state comes only from official lifecycle hooks/notify. Reading its terminal screen is intentionally
    // unsupported even for legacy versions; absent events remain running/unavailable rather than guessed.
    if (rt.agent === "codex") return;

    // Once authoritative hooks arrive, ignore screen detection; it serves fallback and pre-hook startup only.
    if (rt.authoritative) return;

    const prevState = rt.agentState;

    const setScreenState = (state: AgentState) => {
      const updates: Partial<SessionRuntime> = { agentState: state };
      if (state === "working") {
        updates.everWorked = true;
        updates.lastIdleAt = 0;
      } else if (state === "waiting" || state === "asking") {
        if (!rt.lastIdleAt) updates.lastIdleAt = Date.now();
      }
      // Skip store updates when screen-detection results match current values.
      const dirty = Object.entries(updates).some(
        ([k, v]) => !Object.is((rt as unknown as Record<string, unknown>)[k], v),
      );
      if (dirty) {
        set((s) => ({
          runtimes: { ...s.runtimes, [id]: { ...s.runtimes[id], ...updates } },
        }));
      }
      if (state !== prevState && NOTIFY_STATES.includes(state)) {
        if (notifyAgentState(get(), id, state)) {
          set((s) => ({
            notifications: { ...s.notifications, [id]: Date.now() },
          }));
        }
      }
    };

    // Arbitration for agents without authoritative state, primarily legacy Codex.

    // A strong visible blocker maps to asking.
    if (screen.visibleBlocker) return setScreenState("asking");

    // Visible work maps to working.
    if (screen.visibleWorking) return setScreenState("working");

    // The screen is the sole remaining source.
    if (screen.state === "working" && !screen.visibleWorking && !rt.everWorked) return;
    if (screen.state === "waiting" && !rt.everWorked) return;
    return setScreenState(screen.state);
  },

  setWindowFocused: (focused) => set({ windowFocused: focused }),

  focusReturned: () => {
    // On refocus, clear stale markers for missing sessions but never navigate automatically. Focus events
    // cannot distinguish notification clicks from incidental OS focus changes, so users choose which unread session to open.
    const { notifications, sessions, ephemeralSessions } = get();
    const exists = (id: string) =>
      sessions.some((s) => s.id === id) || !!ephemeralSessions[id];
    const stale = Object.keys(notifications).filter((id) => !exists(id));
    if (stale.length > 0) {
      set((state) => {
        const rest = { ...state.notifications };
        for (const id of stale) delete rest[id];
        return { notifications: rest };
      });
    }
  },

  clearNotification: (id) =>
    set((state) => {
      if (!(id in state.notifications)) return {};
      const rest = { ...state.notifications };
      delete rest[id];
      return { notifications: rest };
    }),

  clearAllNotifications: () =>
    set((state) =>
      Object.keys(state.notifications).length === 0 ? {} : { notifications: {} },
    ),

  restartSession: async (id) => {
    await ptyKill(id).catch(() => {});
    set((state) => ({
      epochs: { ...state.epochs, [id]: (state.epochs[id] ?? 0) + 1 },
    }));
  },

  toggleLeft: () => set((s) => ({ leftCollapsed: !s.leftCollapsed })),
  toggleRight: () => set((s) => ({ rightCollapsed: !s.rightCollapsed })),
  resizeLeft: (deltaX) =>
    set((s) => ({ leftWidth: clamp(s.leftWidth + deltaX, LEFT_MIN, LEFT_MAX) })),
  resizeRight: (deltaX) =>
    set((s) => ({
      rightWidth: clamp(s.rightWidth - deltaX, RIGHT_MIN, RIGHT_MAX),
    })),
  toggleBottom: () => set((s) => ({ bottomExpanded: !s.bottomExpanded })),
  toggleTheme: () => {
    const mode: Theme = get().theme === "dark" ? "light" : "dark";
    applyTheme(mode);
    pushSetting("vlx-theme", mode); // Mirror to backend for cross-shell sharing.
    persistAndApplyVisual(get); // Re-resolve automatic accents and publish `vlx-settings`.
    set({ theme: mode });
    notifyAgentsColorScheme(get);
  },
  setTheme: (mode) => {
    applyTheme(mode);
    pushSetting("vlx-theme", mode); // Mirror to backend for cross-shell sharing.
    persistAndApplyVisual(get);
    set({ theme: mode });
    notifyAgentsColorScheme(get);
  },
  toggleSound: () =>
    set((s) => {
      const soundEnabled = !s.soundEnabled;
      const v = soundEnabled ? "1" : "0";
      localStorage.setItem(SOUND_KEY, v);
      pushSetting(SOUND_KEY, v); // Mirror to backend for cross-shell sharing.
      return { soundEnabled };
    }),
  toggleNotify: () =>
    set((s) => {
      const notifyEnabled = !s.notifyEnabled;
      const v = notifyEnabled ? "1" : "0";
      localStorage.setItem(NOTIFY_KEY, v);
      pushSetting(NOTIFY_KEY, v); // Mirror to backend for cross-shell sharing.
      return { notifyEnabled };
    }),
  setCleanPastedImages: (v) =>
    set(() => {
      const val = v ? "1" : "0";
      localStorage.setItem(CLEAN_IMAGES_KEY, val);
      pushSetting(CLEAN_IMAGES_KEY, val); // Share with backend, which gates cleanup.
      return { cleanPastedImages: v };
    }),
  setRecordSessions: (v) =>
    set(() => {
      const val = v ? "1" : "0";
      localStorage.setItem(RECORD_SESSIONS_KEY, val);
      pushSetting(RECORD_SESSIONS_KEY, val); // Share with backend, which gates recording at spawn.
      return { recordSessions: v };
    }),
  splitSidebarTreeView: (direction, sourceViewId) => {
    const id = `tree-view-${genId()}`;
    set((state) => {
      const source =
        state.sidebarTreeViews.find(
          (view) => view.id === (sourceViewId ?? state.activeSidebarTreeViewId),
        ) ?? state.sidebarTreeViews[0];
      if (!source) return {};
      const tab = state.sidebarTreeTabs.find((candidate) =>
        collectSidebarViewIds(candidate.root).includes(source.id));
      if (!tab) return {};
      const usedNames = new Set(state.sidebarTreeViews.map((view) => view.name));
      let index = state.sidebarTreeViews.length + 1;
      let name = t("tree.viewDefaultName", index);
      while (usedNames.has(name)) {
        index += 1;
        name = t("tree.viewDefaultName", index);
      }
      const next: SidebarTreeView = {
        ...source,
        id,
        name,
        statusFilterIds: source.statusFilterIds ? { ...source.statusFilterIds } : null,
        // Start from what the source pane shows right now, then keep expanding and collapsing to itself.
        collapsedOverrides: snapshotCollapsed(state, source),
      };
      return {
        sidebarTreeViews: [...state.sidebarTreeViews, next],
        sidebarTreeTabs: state.sidebarTreeTabs.map((candidate) =>
          candidate.id === tab.id
            ? {
                ...candidate,
                root: splitSidebarView(candidate.root, source.id, direction, id),
                activeViewId: id,
              }
            : candidate),
        activeSidebarTreeViewId: id,
      };
    });
    saveSidebarViewsTick(get);
    return id;
  },
  deleteSidebarTreeView: (id) => {
    set((state) => {
      if (id === state.primarySidebarTreeViewId || state.sidebarTreeViews.length <= 1) return {};
      const tabIndex = state.sidebarTreeTabs.findIndex((tab) =>
        collectSidebarViewIds(tab.root).includes(id));
      if (tabIndex < 0) return {};
      const sidebarTreeViews = state.sidebarTreeViews.filter((view) => view.id !== id);
      const sourceTab = state.sidebarTreeTabs[tabIndex];
      const remainingRoot = removeSidebarView(sourceTab.root, id);
      if (!remainingRoot) return {};
      const memberIds = collectSidebarViewIds(remainingRoot);
      const fallbackId =
        memberIds.includes(sourceTab.activeViewId) && sourceTab.activeViewId !== id
          ? sourceTab.activeViewId
          : firstSidebarViewId(remainingRoot);
      return {
        sidebarTreeViews,
        sidebarTreeTabs: state.sidebarTreeTabs.map((tab) =>
          tab.id === sourceTab.id
            ? { ...tab, root: remainingRoot, activeViewId: fallbackId }
            : tab),
        activeSidebarTreeViewId:
          state.activeSidebarTreeViewId === id ? fallbackId : state.activeSidebarTreeViewId,
      };
    });
    saveSidebarViewsTick(get);
  },
  setActiveSidebarTreeView: (id) => {
    set((state) => {
      if (!state.sidebarTreeViews.some((view) => view.id === id)) return {};
      return {
        activeSidebarTreeViewId: id,
        sidebarTreeTabs: state.sidebarTreeTabs.map((tab) =>
          collectSidebarViewIds(tab.root).includes(id)
            ? { ...tab, activeViewId: id }
            : tab),
      };
    });
    saveSidebarViewsTick(get);
  },
  resizeSidebarTreeSplit: (tabId, splitPaneId, sizes) => {
    set((state) => ({
      sidebarTreeTabs: state.sidebarTreeTabs.map((tab) =>
        tab.id === tabId
          ? { ...tab, root: setSidebarSplitSizes(tab.root, splitPaneId, sizes) }
          : tab),
    }));
    saveSidebarViewsTick(get);
  },
  setSidebarTreeViewFilter: (id, q) => {
    set((state) => ({
      sidebarTreeViews: state.sidebarTreeViews.map((view) =>
        view.id === id ? { ...view, treeFilter: q } : view),
      ...(id === state.primarySidebarTreeViewId ? { treeFilter: q } : {}),
    }));
    saveSidebarViewsTick(get);
  },
  setSidebarTreeViewStatusFilter: (id, st) => {
    set((state) => {
      const current = state.sidebarTreeViews.find((view) => view.id === id);
      if (!current) return {};
      const selected = current.statusFilter?.includes(st)
        ? current.statusFilter.filter((filter) => filter !== st)
        : [...(current.statusFilter ?? []), st];
      const statusFilter = selected.length > 0 ? selected : null;
      const statusFilterIds = statusSnapshot(state, statusFilter);
      return {
        sidebarTreeViews: state.sidebarTreeViews.map((view) =>
          view.id === id ? { ...view, statusFilter, statusFilterIds } : view),
        ...(id === state.primarySidebarTreeViewId
          ? { statusFilter, statusFilterIds }
          : {}),
      };
    });
    saveSidebarViewsTick(get);
  },
  appendSidebarTreeViewStatusMatches: (id) => {
    set((state) => {
      if (!state.dynamicStatusFilter) return state;
      const current = state.sidebarTreeViews.find((view) => view.id === id);
      if (!current?.statusFilter) return state;
      const matches = statusSnapshot(state, current.statusFilter);
      if (!matches) return state;
      const previous = current.statusFilterIds ?? {};
      let next: Record<string, true> | null = null;
      for (const matchId of Object.keys(matches)) {
        if (matchId in previous) continue;
        if (!next) next = { ...previous };
        next[matchId] = true;
      }
      if (!next) return state;
      const statusFilterIds = next;
      return {
        sidebarTreeViews: state.sidebarTreeViews.map((view) =>
          view.id === id ? { ...view, statusFilterIds } : view),
        ...(id === state.primarySidebarTreeViewId ? { statusFilterIds } : {}),
      };
    });
  },
  refreshSidebarTreeViewStatusMatches: (id) => {
    set((state) => {
      const current = state.sidebarTreeViews.find((view) => view.id === id);
      if (!current?.statusFilter) return {};
      const statusFilterIds = statusSnapshot(state, current.statusFilter);
      return {
        sidebarTreeViews: state.sidebarTreeViews.map((view) =>
          view.id === id ? { ...view, statusFilterIds } : view),
        ...(id === state.primarySidebarTreeViewId ? { statusFilterIds } : {}),
      };
    });
  },
  refreshSidebarTreeViewStatusMatch: (id, sessionId) => {
    set((state) => {
      const current = state.sidebarTreeViews.find((view) => view.id === id);
      if (!current?.statusFilter) return {};
      const session = state.sessions.find((candidate) => candidate.id === sessionId);
      if (!session) return {};
      const effective = effectiveStatus(state.runtimes[sessionId]);
      const unread = sessionId in state.notifications;
      const matches = current.statusFilter.some((filter) =>
        matchesAgentState(filter, effective, unread));
      const previous = current.statusFilterIds ?? {};
      if (matches === (sessionId in previous)) return {}; // Snapshot already agrees with the live status.
      const statusFilterIds = { ...previous };
      if (matches) statusFilterIds[sessionId] = true;
      else delete statusFilterIds[sessionId];
      return {
        sidebarTreeViews: state.sidebarTreeViews.map((view) =>
          view.id === id ? { ...view, statusFilterIds } : view),
        ...(id === state.primarySidebarTreeViewId ? { statusFilterIds } : {}),
      };
    });
    saveSidebarViewsTick(get);
  },
  setSidebarTreeViewMarkFilter: (id, mark) => {
    set((state) => {
      const current = state.sidebarTreeViews.find((view) => view.id === id);
      if (!current) return {};
      const markFilter = !mark || current.markFilter === mark ? null : mark;
      return {
        sidebarTreeViews: state.sidebarTreeViews.map((view) =>
          view.id === id ? { ...view, markFilter } : view),
        ...(id === state.primarySidebarTreeViewId ? { markFilter } : {}),
      };
    });
    saveSidebarViewsTick(get);
  },
  setSidebarTreeViewCollapsed: (id, nodeId, collapsed) => {
    set((state) => {
      const current = state.sidebarTreeViews.find((view) => view.id === id);
      if (!current) return {};
      const collapsedOverrides = { ...(current.collapsedOverrides ?? {}), [nodeId]: collapsed };
      return {
        sidebarTreeViews: state.sidebarTreeViews.map((view) =>
          view.id === id ? { ...view, collapsedOverrides } : view),
      };
    });
    saveSidebarViewsTick(get);
  },
  // Global and mobile controls always route to the designated primary projection.
  setTreeFilter: (q) => get().setSidebarTreeViewFilter(get().primarySidebarTreeViewId, q),
  setStatusFilter: (st) => {
    const primaryId = get().primarySidebarTreeViewId;
    set((state) => {
      const current = state.sidebarTreeViews.find((view) => view.id === primaryId);
      if (!current) return {};
      const alreadySoleSelection =
        current.statusFilter?.length === 1 && current.statusFilter[0] === st;
      const statusFilter: AgentState[] | null = alreadySoleSelection ? null : [st];
      const statusFilterIds = statusSnapshot(state, statusFilter);
      return {
        sidebarTreeViews: state.sidebarTreeViews.map((view) =>
          view.id === primaryId ? { ...view, statusFilter, statusFilterIds } : view),
        statusFilter,
        statusFilterIds,
      };
    });
    saveSidebarViewsTick(get);
  },
  setMarkFilter: (mark) =>
    get().setSidebarTreeViewMarkFilter(get().primarySidebarTreeViewId, mark),
  setNotifyGuideOpen: (v) => set({ notifyGuideOpen: v }),
  openSearch: () => set({ searchOpen: true }),
  closeSearch: () => set({ searchOpen: false }),

  // ── Vlinx appearance: update, persist, and apply `data-*` ──
  setAccent: (v) => {
    set({ accent: v });
    persistAndApplyVisual(get);
  },
  setDensity: (v) => {
    set({ density: v });
    persistAndApplyVisual(get);
  },
  setPaneStyle: (v) => {
    set({ paneStyle: v });
    persistAndApplyVisual(get);
  },
  setDividerStyle: (v) => {
    set({ dividerStyle: v });
    persistAndApplyVisual(get);
  },
  setNavLayout: (v) => {
    set({ navLayout: v });
    persistAndApplyVisual(get);
  },
  setInspectorTab: (v) => {
    set({ inspectorTab: v });
    persistAndApplyVisual(get);
  },
  setSingleTabMode: (v) => {
    set({ singleTabMode: v });
    persistAndApplyVisual(get);
  },
  setSpawnConfirm: (v) => {
    set({ spawnConfirm: v });
    persistAndApplyVisual(get);
  },
  setImagePasteMode: (v) => {
    set({ imagePasteMode: v });
    persistAndApplyVisual(get);
  },
  setUsageRefreshSec: (v) => {
    set({ usageRefreshSec: Math.max(0, Math.round(v)) });
    persistAndApplyVisual(get);
  },
  setTermRenderer: (v) => {
    set({ termRenderer: v });
    persistAndApplyVisual(get);
  },
  setRedrawOnReveal: (v) => {
    set({ redrawOnReveal: v });
    persistAndApplyVisual(get);
  },
  setOutputScheduler: (v) => {
    set({ outputScheduler: v });
    persistAndApplyVisual(get);
  },
  setDynamicStatusFilter: (v) => {
    set({ dynamicStatusFilter: v });
    persistAndApplyVisual(get);
  },
  setMaxLiveTabs: (v) => {
    set({ maxLiveTabs: Math.max(4, Math.min(64, Math.round(v))) });
    persistAndApplyVisual(get);
  },
  setDefaultShell: (v) => {
    set({ defaultShell: v });
    persistAndApplyVisual(get);
  },
  setUiFontFamily: (v) => {
    set({ uiFontFamily: v && v.trim() ? v.trim() : null });
    persistAndApplyVisual(get);
  },
  setUiFontSize: (v) => {
    set({ uiFontSize: v == null ? null : Math.max(10, Math.min(20, Math.round(v))) });
    persistAndApplyVisual(get);
  },
  setTermFontFamily: (v) => {
    set({ termFontFamily: v && v.trim() ? v.trim() : null });
    persistAndApplyVisual(get);
  },
  setTermFontSize: (v) => {
    set({ termFontSize: Math.max(10, Math.min(24, Math.round(v))) });
    persistAndApplyVisual(get);
  },
  setShortcut: (action, combo) => {
    set((s) => ({ shortcutOverrides: { ...s.shortcutOverrides, [action]: combo } }));
    persistAndApplyVisual(get);
  },
  resetShortcuts: () => {
    set({ shortcutOverrides: {} });
    persistAndApplyVisual(get);
  },
  setAgentDefault: (kind, patch) => {
    set((s) => {
      const merged: AgentDefaultConfig = { ...(s.agentDefaults[kind] ?? {}), ...patch };
      // Normalize empty arguments/paths and default permissions as unset to keep storage compact.
      const clean: AgentDefaultConfig = {};
      const args = merged.args?.trim();
      if (args) clean.args = args;
      if (merged.permissionMode && merged.permissionMode !== "default")
        clean.permissionMode = merged.permissionMode;
      const path = merged.path?.trim();
      if (path) clean.path = path;
      const next = { ...s.agentDefaults };
      if (Object.keys(clean).length) next[kind] = clean;
      else delete next[kind];
      return { agentDefaults: next };
    });
    persistAndApplyVisual(get);
  },
  setOrchestrationProfile: (name, patch) => {
    const key = name.trim();
    if (!key) return;
    set((s) => {
      const next = { ...s.orchestrationProfiles };
      if (patch === null) delete next[key];
      else {
        const current = next[key];
        next[key] = {
          ...current,
          ...patch,
          permissionMode: patch.permissionMode ?? current?.permissionMode ?? "default",
        };
      }
      return { orchestrationProfiles: next };
    });
    persistAndApplyVisual(get);
  },
  setOrchestrationLimits: (patch) => {
    set((s) => ({ orchestration: { ...s.orchestration, ...patch } }));
    persistAndApplyVisual(get);
  },
  applyAppearance: () => {
    applyTheme(get().theme);
    persistAndApplyVisual(get);
    notifyAgentsColorScheme(get);
  },
  hydrateSettingsFromCache: () => {
    // Re-read backend-authoritative values already reconciled into local storage and apply without writing back.
    const ps = loadSettings();
    const theme = loadTheme();
    set({
      ...ps,
      theme,
      soundEnabled: loadSoundEnabled(),
      notifyEnabled: loadNotifyEnabled(),
      cleanPastedImages: loadCleanPastedImages(),
      recordSessions: loadRecordSessions(),
    });
    applyTheme(theme);
    applyVisual(visualOf(ps));
    notifyAgentsColorScheme(get);
  },

  // ── Center area: split dragging and scratch tabs ──
  resizePane: (tabId, splitPaneId, sizes) => {
    set((state) => {
      const t = state.paneTrees[tabId];
      if (!t) return {};
      return {
        paneTrees: { ...state.paneTrees, [tabId]: setSizes(t, splitPaneId, sizes) },
      };
    });
    saveLayoutTick();
  },

  newScratchTab: async (opts) => {
    const st = get();
    const { activeSessionId, inspectTarget } = st;
    const targetSession = opts?.target?.sessionId
      ? st.sessions.find((s) => s.id === opts.target?.sessionId) ??
        st.ephemeralSessions[opts.target.sessionId] ??
        null
      : null;
    const targetGroup = opts?.target?.groupId
      ? st.groups.find((g) => g.id === opts.target?.groupId)
      : undefined;
    const targetProject = opts?.target
      ? st.projects.find((p) => p.id === opts.target?.projectId)
      : undefined;
    // When a project/group is inspected, place a new scratch terminal in that project instead of inheriting
    // an unrelated active session. Groups use their project root.
    const overrideGroup =
      inspectTarget?.kind === "group"
        ? st.groups.find((g) => g.id === inspectTarget.id)
        : undefined;
    const overrideProject =
      inspectTarget?.kind === "project"
        ? st.projects.find((p) => p.id === inspectTarget.id)
        : overrideGroup
          ? st.projects.find((p) => p.id === overrideGroup.projectId)
          : undefined;
    // Otherwise follow the active session's project and working directory, matching split behavior.
    const active = activeSessionId
      ? st.sessions.find((s) => s.id === activeSessionId) ??
        st.ephemeralSessions[activeSessionId] ??
        null
      : null;
    const project =
      targetProject ??
      overrideProject ??
      (active && st.projects.find((p) => p.id === active.projectId)) ??
      st.projects[0];
    // Working-directory priority: explicit, inspected project/group root, runtime cwd, session cwd, session
    // project root, then first project root. Inspection deliberately excludes the previous session's runtime cwd.
    let cwd: string | null = opts?.cwd ?? null;
    if (cwd == null && targetSession) {
      try {
        cwd = await getSessionCwd(targetSession.id);
      } catch {
        /* Fall back when not running or the query fails. */
      }
      cwd = cwd ?? targetSession.cwd ?? null;
    }
    cwd = cwd ?? targetGroup?.worktreePath ?? null;
    if (cwd == null && !opts?.target && !overrideProject && active) {
      try {
        cwd = await getSessionCwd(active.id);
      } catch {
        /* Fall back when not running or the query fails. */
      }
      cwd = cwd ?? active.cwd ?? null;
    }
    cwd = cwd ?? project?.rootPath ?? null;
    const id = `eph-${genId()}`;
    // Use the global default shell when scratch creation does not specify one.
    const effShell = (opts?.shell === undefined ? st.defaultShell : opts.shell) || null;
    const ephemeral: Session = {
      id,
      projectId: opts?.target?.projectId ?? overrideProject?.id ?? active?.projectId ?? project?.id ?? "",
      groupId: opts?.target
        ? (opts.target.groupId ?? targetSession?.groupId ?? null)
        : (overrideGroup?.id ?? (overrideProject ? null : active?.groupId) ?? null),
      name: opts?.name ?? nextTerminalName(st.sessions, st.ephemeralSessions),
      kind: "terminal",
      shell: effShell,
      cwd,
      envJson: null,
      initCmd: null,
      hotkey: null,
      parentSessionId: null,
      // Ephemeral sessions start collapsed to keep nested Scratch entries compact.
      collapsed: true,
      worktreePath: null,
      sortOrder: 0,
      // Assign current Unix time to frontend-only sessions instead of displaying the epoch.
      createdAt: Math.floor(Date.now() / 1000),
    };
    const leaf = makeLeaf(id);
    set((state) => ({
      ephemeralSessions: { ...state.ephemeralSessions, [id]: ephemeral },
      runtimes: { ...state.runtimes, [id]: { status: "idle" } },
      openTabs: [...state.openTabs, id],
      // Scratch terminals are pinned and never replaced by single-tab reuse.
      pinnedTabs: [...state.pinnedTabs, id],
      paneTrees: { ...state.paneTrees, [id]: leaf },
      activeTabId: id,
      lastActiveSessionTabId: id,
      activeSessionId: id,
      focusedPaneId: leaf.paneId,
    }));
    saveLayoutTick();
  },

  setEphemeralShell: (id, shell) =>
    set((state) => {
      const s = state.ephemeralSessions[id];
      if (!s) return {};
      return {
        ephemeralSessions: { ...state.ephemeralSessions, [id]: { ...s, shell: shell || null } },
      };
    }),

  switchSessionShell: async (id, shellPath) => {
    const st = get();
    const persisted = st.sessions.find((x) => x.id === id);
    if (persisted) {
      await get().updateSession(id, {
        name: persisted.name,
        shell: shellPath || null,
        cwd: persisted.cwd ?? null,
        initCmd: persisted.initCmd ?? null,
      });
    } else if (st.ephemeralSessions[id]) {
      get().setEphemeralShell(id, shellPath || null);
    } else {
      return;
    }
    // Restart a running session by killing and incrementing its generation so the new shell applies immediately.
    if (get().runtimes[id]?.status === "running") {
      await get().restartSession(id);
    }
  },
}));

// Do not probe shells at module scope: remote windows would call the backend before login and race an
// unauthenticated WebSocket connection. `App.tsx` runs the probe after authenticated mount.

// Keep single-selection highlighting synchronized with the active session through one subscription. This
// prevents stale and active sessions from both appearing selected while leaving Cmd/Shift multi-selection intact.
{
  let prevActive = useTermStore.getState().activeSessionId;
  useTermStore.subscribe((s) => {
    if (s.activeSessionId === prevActive) return;
    prevActive = s.activeSessionId;
    const sel: SelNode[] = s.activeSessionId
      ? [{ id: s.activeSessionId, kind: "session" }]
      : [];
    // Avoid redundant writes and renders when already synchronized.
    const same =
      s.selection.length === sel.length &&
      (sel.length === 0 || s.selection[0]?.id === sel[0]?.id);
    if (!same) {
      useTermStore.setState({ selection: sel, selectionAnchor: s.activeSessionId });
    }
    // Clear project/group inspection whenever the active session changes, even while the right panel is unmounted,
    // so scratch-directory selection cannot use stale inspection state.
    if (s.inspectTarget) {
      useTermStore.setState({ inspectTarget: null });
    }
  });
}

// Development invariant guard for structural tab/session changes, catching blank-after-close inconsistencies
// without running on high-frequency runtime updates.
if (DEBUG) {
  let sig = "";
  useTermStore.subscribe((s) => {
    const next = JSON.stringify([
      s.openTabs,
      s.activeTabId,
      s.activeSessionId,
      Object.keys(s.paneTrees),
      s.sessions.length,
      Object.keys(s.ephemeralSessions),
      Object.keys(s.docTabs),
    ]);
    if (next !== sig) {
      sig = next;
      checkTabInvariants("state-change", s);
    }
  });
}
