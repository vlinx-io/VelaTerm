//! Application-settings persistence through localStorage and cross-shell synchronization, plus related
//! types. Extracted from termStore because settings I/O and defaults are self-contained. This lets
//! SettingsModal and the store share types while reducing the size of the main store.

import { pushSetting } from "../ipc/settingsSync";
import type {
  AccentChoice,
  Density,
  DividerStyle,
  InspectorTab,
  NavLayout,
  PaneStyle,
  VisualSettings,
} from "../theme";

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
  /** Default permission mode: "skip" bypasses all confirmation; missing means staged approval. */
  permissionMode?: string;
  /** Absolute executable path; empty uses command-name lookup through PATH. This is global per type and
   * read from app_settings at spawn time by `agent_bin_path`, so changes affect subsequent launches.
   * AgentInstallCard fills an empty value after locating a successful one-click installation. */
  path?: string;
}

/** Terminal renderer: DOM is the stable default; canvas uses a responsive 2D bitmap without GPU
 * contexts; WebGL is sharpest but many terminals can exhaust contexts and return blank or misaligned. */
export type TermRenderer = "dom" | "canvas" | "webgl";

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
  /** Terminal renderer. DOM is stable; canvas avoids DOM overhead without GPU contexts; WebGL is sharp
   * but many terminals can hit context limits and return blank or misaligned. */
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
  /** Usage auto-refresh interval in seconds; zero disables it. Applies to Claude and Codex in Info. */
  usageRefreshSec: number;
  /** Image paste mode: upload writes a file path, while agent lets the agent read the clipboard and show
   * `[Image #x]`. Configurable only on local desktop clients; browser and remote clients always upload. */
  imagePasteMode: ImagePasteMode;
}
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
  termFontSize: 13,
  shortcutOverrides: {},
  agentDefaults: {},
  spawnConfirm: true,
  saveWorkspaceOnQuit: true,
  usageRefreshSec: 300,
  imagePasteMode: "upload",
};
export function loadSettings(): PersistedSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (!raw) return { ...SETTINGS_DEFAULTS };
    // Older versions stored boolean gpuRender outside PersistedSettings; declare it solely for migration.
    const parsed = JSON.parse(raw) as Partial<PersistedSettings> & { gpuRender?: boolean };
    const merged = { ...SETTINGS_DEFAULTS, ...parsed };
    // Migrate boolean gpuRender to termRenderer only when the new key is absent, preserving WebGL for
    // existing users. Future saves write only the new structure and naturally discard the old field.
    if (parsed.termRenderer === undefined && typeof parsed.gpuRender === "boolean") {
      merged.termRenderer = parsed.gpuRender ? "webgl" : "dom";
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
});
