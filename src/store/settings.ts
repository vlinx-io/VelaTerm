//! Application-settings persistence through localStorage and cross-shell synchronization, plus related
//! types. Extracted from termStore because settings I/O and defaults are self-contained. This lets
//! SettingsModal and the store share types while reducing the size of the main store.

import { pushSetting } from "../ipc/settingsSync";
import { DEFAULT_CONVERSATION_FONT_SIZE, DEFAULT_TERMINAL_FONT_SIZE, DEFAULT_TERMINAL_LINE_HEIGHT, normalizeTextSize, normalizeTextLineHeight } from "../theme";
import type {
  AccentChoice,
  Density,
  DividerStyle,
  InspectorTab,
  NavLayout,
  PaneStyle,
  VisualSettings,
} from "../theme";
import type { SessionEngine, SessionKind } from "../types";

/** User-configurable background keep-alive tab limit, defaulting to 32. */
export const DEFAULT_MAX_LIVE_TABS = 32;

/** Persistence key for notification sound, enabled by default and storing only "0" for off. */
export const SOUND_KEY = "vlx-sound";
export const loadSoundEnabled = (): boolean => localStorage.getItem(SOUND_KEY) !== "0";

/** Persistence key for system notifications, enabled by default and storing only "0" for off.
 * Disabling suppresses OS popups but retains sidebar unread dots, Dock badges, and status-bar counts. */
export const NOTIFY_KEY = "vlx-notify";
export const loadNotifyEnabled = (): boolean => localStorage.getItem(NOTIFY_KEY) !== "0";

/** Persistence key for automatic pasted-image cleanup, enabled by default. Its matching backend
 * app_settings key is shared across shells and gates startup/exit cleanup in `pasted_image_cleanup_enabled`. */
export const CLEAN_IMAGES_KEY = "vlx-clean-images";
export const loadCleanPastedImages = (): boolean => localStorage.getItem(CLEAN_IMAGES_KEY) !== "0";

/** Persistence key for session recording, disabled by default and storing only "1" for on. The
 * matching backend key is shared across shells and gates PTY output recording to `recordings/<id>.log`.
 * Plain terminal sessions are never recorded because replay offers no value. */
export const RECORD_SESSIONS_KEY = "vlx-record-sessions";
export const loadRecordSessions = (): boolean => localStorage.getItem(RECORD_SESSIONS_KEY) === "1";

/** Defaults for one agent type, applied when a new session has no explicit value. A structured object
 * allows additional per-agent defaults without adding more flat maps. */
export interface AgentDefaultConfig {
  /** Default launch arguments such as "--model opus"; empty or missing means no arguments. */
  args?: string;
  /** Backend-persisted permission choice for new sessions; `"skip"` is the legacy bypass spelling. */
  permissionMode?: string;
  /** Absolute executable path; empty uses command-name lookup through PATH. This is global per type and
   * read from app_settings at spawn time by `agent_bin_path`, so changes affect subsequent launches.
   * AgentInstallCard fills it after locating a successful one-click installation, or with a path the
   * user enters in the card's path field. */
  path?: string;
  /** View a new session of this agent opens in. Missing falls back to the conversation view for every
   * chat-capable agent (`defaultEngineFor`). */
  engine?: SessionEngine;
}

/** View a newly created session of this agent opens in: its saved per-agent choice, then the unified
 * conversation default. Callers only use this for kinds the chat engine can drive. */
export function defaultEngineFor(
  kind: SessionKind,
  agentDefaults: Record<string, AgentDefaultConfig>,
): SessionEngine {
  return agentDefaults[kind]?.engine ?? "chat";
}

/** The permission a session launches with: its own choice, or the agent kind's global default when it has
 * none. Mirrors the backend's `permission_catalog::effective`, so displays match what actually runs. */
export function effectivePermissionMode(
  session: { kind: SessionKind; permissionMode?: string | null },
  agentDefaults: Record<string, AgentDefaultConfig>,
): string | null {
  return session.permissionMode?.trim() || agentDefaults[session.kind]?.permissionMode?.trim() || null;
}

/** Terminal renderer: DOM is the stable default; WebGL accelerates rendering but can exhaust GPU contexts. */
export type TermRenderer = "dom" | "webgl";

/** Last launch choices for one role of the planning workflow, restored the next time its dialog opens. */
export interface PlanExecuteRolePrefs {
  agent?: SessionKind;
  model?: string;
  effort?: string;
}

/** Last agent, model and reasoning effort chosen for knowledge-base compilation, restored the next time
 * its dialog opens. */
export interface MemoryPrefs {
  agent?: SessionKind;
  model?: string;
  effort?: string;
}

/** One global pre-summary choice for session references. Agent capabilities and CLI argument mapping
 * remain backend-owned; the client persists only the user's selection. */
export interface ReferSummaryConfig {
  /** Off preserves the original `vrefer --ask` full-transcript behavior. */
  enabled: boolean;
  agent: SessionKind;
  /** Empty values use the selected agent's own defaults. */
  model: string;
  effort: string;
}

/** Image paste mode, configurable only on local desktop clients; browser/remote always upload.
 * `upload` stores a temporary file and writes its visible path to the terminal. `agent` sends Ctrl+V
 * to Claude or Codex so the agent reads the system clipboard and displays its attachment placeholder. */
export type ImagePasteMode = "upload" | "agent";

/** Persisted Vlinx appearance settings for accent, density, splits, separators, navigation, and Inspector. */
export const SETTINGS_KEY = "vlx-settings";
export interface PersistedSettings {
  accent: AccentChoice;
  density: Density;
  paneStyle: PaneStyle;
  dividerStyle: DividerStyle;
  navLayout: NavLayout;
  inspectorTab: InspectorTab;
  /** Single-tab mode reuses the current tab and keeps the previous tree alive in the background. */
  singleTabMode: boolean;
  /** Terminal renderer. DOM is stable; WebGL can hit context limits with many terminals. */
  termRenderer: TermRenderer;
  /** Advanced full redraw on tab return, off by default. Enable only to mitigate GPU artifacts or
   * blank frames; normal tab switching redraws only after a size change. */
  redrawOnReveal: boolean;
  /** Foreground-priority output scheduling, enabled by default. Foreground output writes immediately,
   * while background output is batched to prevent busy agents from degrading focused typing. */
  outputScheduler: boolean;
  /** Whether active sidebar status filters automatically include sessions that newly match. Existing
   * members remain stable until the filter itself changes. */
  dynamicStatusFilter: boolean;
  /** User-configurable background keep-alive tab limit, defaulting to 32. */
  maxLiveTabs: number;
  /** Default terminal shell; empty uses the system default. */
  defaultShell: string;
  /** Primary UI monospace font; null uses the default JetBrains Mono stack. */
  uiFontFamily: string | null;
  /** UI font size in pixels; null follows density without an inline `--ui-fs`. */
  uiFontSize: number | null;
  /** Primary terminal monospace font; null uses the default stack. */
  termFontFamily: string | null;
  /** Terminal font size in pixels, defaulting to 13. */
  termFontSize: number;
  /** Line-height multiplier, independent of conversation typography. */
  termLineHeight: number;
  /** Conversation typography is saved independently of terminal preferences. */
  chatFontFamily: string | null;
  chatFontSize: number;
  chatLineHeight: number;
  /** Global shortcut overrides from action ID to chord; missing entries use shortcutRegistry defaults. */
  shortcutOverrides: Record<string, string>;
  /** Defaults by agent type, serving as global templates for new local-agent sessions. Explicit
   * per-session agentArgs and permissionMode values take precedence. */
  agentDefaults: Record<string, AgentDefaultConfig>;
  /** Whether to confirm derived sessions before launch. Enabled lets users review/edit vspawn prompts;
   * disabled immediately creates and starts the session. */
  spawnConfirm: boolean;
  /** Default state of the quit dialog's "save workspace" checkbox, remembered from the last exit.
   * Checked by default: losing the layout costs more than an unwanted snapshot. */
  saveWorkspaceOnQuit: boolean;
  /** Whether the backend keeps the account-usage snapshot fresh on its own. Off leaves the Info panel
   * showing the last reading until someone presses its refresh button. */
  usageAutoRefresh: boolean;
  /** How often the backend refreshes that snapshot, in seconds. One poller serves every session and
   * client, so this is the real query rate against Claude, Codex, and Grok. */
  usageRefreshSec: number;
  /** Whether a Claude or Codex conversation stopped by a five-hour or weekly usage limit continues on its own
   * once the limit resets. The backend scheduler reads this key from the shared settings block. */
  autoContinueAtUsageLimit: boolean;
  /** Image paste mode: upload writes a file path, while agent lets the agent read the clipboard and show
   * `[Image #x]`. Configurable only on local desktop clients; browser and remote clients always upload. */
  imagePasteMode: ImagePasteMode;
  /** Model a conversation starts on, remembered from the last one picked. Empty means the agent's own default. */
  chatModel: string;
  /** Model remembered independently for each chat-capable agent. */
  chatModelByKind: Record<string, string>;
  /** Thinking effort per model, remembered from the last one picked for that model.
   *
   * Kept per model rather than as one value because the models do not offer the same ladder: a level that
   * is valid on one is rejected by another, and a single remembered value would be silently dropped every
   * time it did not apply. */
  chatEffortByModel: Record<string, string>;
  /** Whether a new Claude conversation opens with fast mode on, per agent protocol. */
  chatFastModeByKind: Record<string, boolean>;
  /** Last agent, model and reasoning effort chosen for each planning/execution role, so a repeated
   * workflow opens on the setup that was used last time rather than on the parent session's. */
  planExecutePrefs: { plan: PlanExecuteRolePrefs; exec: PlanExecuteRolePrefs };
  /** Last agent, model and reasoning effort chosen when organizing a session into the knowledge base,
   * so the next dialog opens on the setup that was used last time. */
  memoryPrefs: MemoryPrefs;
  /** Optional pre-summary used by `vrefer --ask`; there is one choice for every caller and target. */
  referSummary: ReferSummaryConfig;
  /** Whether the Info panel's Resources section shows the whole-machine group. Off hides those rows and
   * stops sampling the machine, leaving only this session's own CPU and memory. */
  showSystemResources: boolean;
  /** Info panel sections the user collapsed, as a sparse map of section id to `true`. A missing id means
   * the section is open, so the map stays empty until someone collapses something. */
  infoCollapsed: Record<string, boolean>;
  /** Composer chips shown inline under the message input, in display order. Chips missing from the list
   * are off: they stay reachable in the settings and move into the More row whenever it appears. */
  composerInlineChips: ComposerChipId[];
  /** Sidebar order: on keeps the most recently active projects and sessions at the top, off renders the
   * manual order. Off by default; the manual order is never changed by the mode. */
  sortByActivity: boolean;
}

/** Every composer chip the toolbar can show, in the order the toolbar used before the list became
 * configurable. Availability per agent and engine state is decided by the pane, not here. */
export const COMPOSER_CHIP_IDS = [
  "model",
  "effort",
  "collaboration",
  "permission",
  "fastMode",
  "serviceTier",
  "personality",
  "mcp",
  "tasks",
  "account",
  "codexCredits",
] as const;
export type ComposerChipId = (typeof COMPOSER_CHIP_IDS)[number];
/** The chips that sat beside the message before the list became configurable. */
export const DEFAULT_COMPOSER_INLINE_CHIPS: ComposerChipId[] = ["model", "effort", "collaboration", "permission"];

const SETTINGS_DEFAULTS: PersistedSettings = {
  accent: "auto",
  density: "regular",
  paneStyle: "flush",
  dividerStyle: "subtle",
  navLayout: "tree",
  inspectorTab: "info",
  singleTabMode: true,
  termRenderer: "dom",
  redrawOnReveal: false,
  outputScheduler: true,
  dynamicStatusFilter: true,
  maxLiveTabs: DEFAULT_MAX_LIVE_TABS,
  defaultShell: "",
  uiFontFamily: null,
  uiFontSize: null,
  termFontFamily: null,
  termFontSize: DEFAULT_TERMINAL_FONT_SIZE,
  termLineHeight: DEFAULT_TERMINAL_LINE_HEIGHT,
  chatFontFamily: null,
  chatFontSize: DEFAULT_CONVERSATION_FONT_SIZE,
  chatLineHeight: DEFAULT_TERMINAL_LINE_HEIGHT,
  shortcutOverrides: {},
  agentDefaults: {},
  spawnConfirm: true,
  saveWorkspaceOnQuit: true,
  usageAutoRefresh: true,
  usageRefreshSec: 300,
  autoContinueAtUsageLimit: false,
  imagePasteMode: "upload",
  chatModel: "",
  chatModelByKind: {},
  chatEffortByModel: {},
  chatFastModeByKind: {},
  planExecutePrefs: { plan: {}, exec: {} },
  memoryPrefs: {},
  referSummary: { enabled: false, agent: "claude", model: "", effort: "" },
  showSystemResources: true,
  infoCollapsed: {},
  composerInlineChips: DEFAULT_COMPOSER_INLINE_CHIPS,
  sortByActivity: false,
};

/** Keeps only known chip ids, once each and in the saved order. A missing or malformed value falls back
 * to the default set; an explicit empty list is a valid choice (no inline chips). */
export function sanitizeComposerInlineChips(value: unknown): ComposerChipId[] {
  if (!Array.isArray(value)) return [...DEFAULT_COMPOSER_INLINE_CHIPS];
  const known = new Set<string>(COMPOSER_CHIP_IDS);
  const result: ComposerChipId[] = [];
  for (const id of value) {
    if (typeof id === "string" && known.has(id) && !result.includes(id as ComposerChipId)) result.push(id as ComposerChipId);
  }
  return result;
}

/** Drops anything a corrupted or older payload may hold, keeping only the three launch choices. */
function sanitizeLaunchChoice(input: unknown): MemoryPrefs {
  if (!input || typeof input !== "object" || Array.isArray(input)) return {};
  const source = input as Record<string, unknown>;
  const result: MemoryPrefs = {};
  if (typeof source.agent === "string" && source.agent) result.agent = source.agent as SessionKind;
  if (typeof source.model === "string") result.model = source.model;
  if (typeof source.effort === "string") result.effort = source.effort;
  return result;
}

function sanitizePlanExecutePrefs(value: unknown): { plan: PlanExecuteRolePrefs; exec: PlanExecuteRolePrefs } {
  const map = value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
  return { plan: sanitizeLaunchChoice(map.plan), exec: sanitizeLaunchChoice(map.exec) };
}

function sanitizeReferSummary(value: unknown): ReferSummaryConfig {
  const source = value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
  return {
    enabled: source.enabled === true,
    agent: typeof source.agent === "string" && source.agent
      ? source.agent as SessionKind
      : SETTINGS_DEFAULTS.referSummary.agent,
    model: typeof source.model === "string" ? source.model : "",
    effort: typeof source.effort === "string" ? source.effort : "",
  };
}

export function loadSettings(): PersistedSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (!raw) return { ...SETTINGS_DEFAULTS };
    // Older versions stored boolean gpuRender and an app-wide defaultSessionEngine outside the current
    // shape; declare both solely for migration.
    const parsed = JSON.parse(raw) as Partial<PersistedSettings> & {
      gpuRender?: boolean;
      defaultSessionEngine?: SessionEngine;
    };
    const merged = { ...SETTINGS_DEFAULTS, ...parsed };
    merged.chatFontFamily = typeof merged.chatFontFamily === "string" ? merged.chatFontFamily.trim() || null : null;
    merged.chatFontSize = normalizeTextSize(merged.chatFontSize, DEFAULT_CONVERSATION_FONT_SIZE);
    merged.chatLineHeight = normalizeTextLineHeight(merged.chatLineHeight);
    merged.termLineHeight = normalizeTextLineHeight(merged.termLineHeight);
    if (
      !parsed.chatModelByKind ||
      typeof parsed.chatModelByKind !== "object" ||
      Array.isArray(parsed.chatModelByKind)
    ) {
      merged.chatModelByKind = typeof parsed.chatModel === "string" ? { claude: parsed.chatModel } : {};
    }
    // Collapsed sections are a sparse map keyed by section id; drop anything that is not an explicit
    // `true` so a corrupted payload cannot hide sections the user never closed.
    merged.infoCollapsed =
      parsed.infoCollapsed && typeof parsed.infoCollapsed === "object" && !Array.isArray(parsed.infoCollapsed)
        ? Object.fromEntries(Object.entries(parsed.infoCollapsed).filter(([, closed]) => closed === true))
        : {};
    merged.planExecutePrefs = sanitizePlanExecutePrefs(parsed.planExecutePrefs);
    merged.memoryPrefs = sanitizeLaunchChoice(parsed.memoryPrefs);
    merged.referSummary = sanitizeReferSummary(parsed.referSummary);
    merged.composerInlineChips = sanitizeComposerInlineChips(parsed.composerInlineChips);
    // Only an explicit boolean true switches the activity order on; anything else keeps the manual order.
    merged.sortByActivity = parsed.sortByActivity === true;
    // Migrate boolean gpuRender to termRenderer only when the new key is absent, preserving WebGL for
    // existing users. Future saves write only the new structure and naturally discard the old field.
    if (parsed.termRenderer === undefined && typeof parsed.gpuRender === "boolean") {
      merged.termRenderer = parsed.gpuRender ? "webgl" : "dom";
    }
    // xterm 6 removed the Canvas addon. Migrate legacy or invalid values to the stable DOM renderer;
    // the next settings save persists this choice, including settings received from older clients.
    if (merged.termRenderer !== "dom" && merged.termRenderer !== "webgl") {
      merged.termRenderer = "dom";
    }
    // The view choice used to be a single app-wide setting covering Claude, Codex and OpenCode, while Pi
    // and OMP were outside its scope. Fold a saved choice into those three agents; newer chat-capable
    // agents use the unified conversation default. Clone first so the module-level defaults are never mutated.
    merged.agentDefaults =
      merged.agentDefaults && typeof merged.agentDefaults === "object" && !Array.isArray(merged.agentDefaults)
        ? { ...merged.agentDefaults }
        : {};
    if (parsed.defaultSessionEngine === "chat" || parsed.defaultSessionEngine === "tui") {
      for (const kind of ["claude", "codex", "opencode"] as const) {
        if (merged.agentDefaults[kind]?.engine === undefined) {
          merged.agentDefaults[kind] = { ...merged.agentDefaults[kind], engine: parsed.defaultSessionEngine };
        }
      }
    }
    return merged;
  } catch {
    return { ...SETTINGS_DEFAULTS };
  }
}
export function saveSettings(s: PersistedSettings) {
  try {
    const json = JSON.stringify(s);
    localStorage.setItem(SETTINGS_KEY, json);
    // Mirror to the backend for cross-shell sharing, but only after reconciliation avoids startup races.
    pushSetting(SETTINGS_KEY, json);
  } catch {
    /* Ignore unavailable localStorage. */
  }
}
export const visualOf = (s: PersistedSettings): VisualSettings => ({
  accent: s.accent,
  density: s.density,
  paneStyle: s.paneStyle,
  dividerStyle: s.dividerStyle,
  navLayout: s.navLayout,
  uiFontFamily: s.uiFontFamily,
  uiFontSize: s.uiFontSize,
  chatFontFamily: s.chatFontFamily,
  chatFontSize: s.chatFontSize,
  chatLineHeight: s.chatLineHeight,
});
