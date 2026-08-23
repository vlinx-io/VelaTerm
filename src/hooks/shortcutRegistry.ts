//! Customizable global shortcut registry + combo key encode/decode utilities.
//!
//! Design:
//! - "mod" is a cross-platform modifier abstraction: Cmd (metaKey) on macOS, Ctrl on other platforms.
//!   Both `e.metaKey || e.ctrlKey` count as mod, consistent with useKeyboardShortcuts.
//! - Combos are encoded as strings in canonical order: `mod[+shift][+alt]+<letter>` (lowercase).
//!   e.g. "mod+t", "mod+w", "mod+shift+f", "mod+shift+b". mod is always included; main key is
//!   limited to a single letter (A-Z). Numeric keys and +/-/0 are reserved for structural shortcuts
//!   (Cmd/Ctrl+1~9 tab switching, Cmd/Ctrl++/- font size) and are not customizable, to avoid conflicts.
//! - Only "primary function" shortcuts can be registered and customized here.

import { env } from "../platform";

/** Customizable shortcut action ids. */
export type ShortcutAction =
  | "openProject"
  | "newTab"
  | "newBrowserTab"
  | "closePane"
  | "splitRight"
  | "splitDown"
  | "search"
  | "globalSearch"
  | "saveDoc";

/** Action order for the settings UI (newBrowserTab only shown on desktop, gated by isTauri). */
export const SHORTCUT_ACTIONS: ShortcutAction[] = [
  "openProject",
  "newTab",
  "newBrowserTab",
  "splitRight",
  "splitDown",
  "closePane",
  "search",
  "globalSearch",
  "saveDoc",
];

/** Whether the current platform is macOS (for default keymaps and display symbol selection). */
export const IS_MAC =
  typeof navigator !== "undefined" &&
  /mac|iphone|ipad|ipod/i.test(navigator.platform || navigator.userAgent || "");

/**
 * Whether this client is a plain browser (URL remote access). Browsers reserve Cmd/Ctrl letter
 * combos (⌘D bookmark, ⌘T new tab, ⌘W close tab, ⌘F find...) and never deliver them to the page,
 * so plain-browser clients must fall back to the Ctrl+Alt defaults even on macOS. Tauri/Electron
 * shells and remote-connection windows keep the Cmd bindings because their WebViews do not reserve
 * them (desktop Windows/Linux WebView2 gets its browser accelerators disabled at the COM level).
 */
export const IS_PLAIN_BROWSER = env.isBrowser && !env.isRemoteWindow;

/**
 * Default keybindings per platform:
 * - macOS (shell): Cmd+key (split up/down: Cmd+Shift+D, global search: Cmd+Shift+F).
 * - Windows/Linux and plain browsers on any OS: Ctrl+Alt+key. Bare Ctrl+letter is a shell
 *   reserved key (Ctrl+D=EOF, Ctrl+W=delete-word...), Ctrl+Shift is swallowed by IMEs, Alt is
 *   terminal Meta. Only Ctrl+Alt reliably reaches the web without conflicts — browsers consume
 *   Cmd/Ctrl letter combos before the page sees them (see IS_PLAIN_BROWSER).
 *   Split: D=right / E=up-down. Global search: G (global) vs inline search F.
 *   Save remains bare Ctrl+S (intercepted only on doc tabs; reserved as XOFF in terminals).
 *   When the terminal is focused, these Ctrl+Alt combos are intercepted by usePtySession's
 *   customKeyEventHandler to prevent xterm from treating them as Meta (see APP_ALT_KEYS).
 *   Users can override individual bindings via shortcutOverrides in settings.
 *   Recorded combos are platform-independent "mod[+shift][+alt]+key" strings.
 */
export const DEFAULT_BINDINGS: Record<ShortcutAction, string> =
  IS_MAC && !IS_PLAIN_BROWSER
    ? {
        openProject: "mod+o",
        newTab: "mod+t",
        newBrowserTab: "mod+shift+b",
        closePane: "mod+w",
        splitRight: "mod+d",
        splitDown: "mod+shift+d",
        search: "mod+f",
        globalSearch: "mod+shift+f",
        saveDoc: "mod+s",
      }
    : {
        openProject: "mod+alt+o",
        newTab: "mod+alt+t",
        newBrowserTab: "mod+alt+b",
        closePane: "mod+alt+w",
        splitRight: "mod+alt+d",
        splitDown: "mod+alt+e",
        search: "mod+alt+f",
        globalSearch: "mod+alt+g",
        saveDoc: "mod+s",
      };

interface ParsedCombo {
  shift: boolean;
  alt: boolean;
  /** Main key: a single lowercase letter. */
  key: string;
}

/** Parse a combo string into structured parts. Returns empty key on empty/invalid input. */
function parseCombo(combo: string): ParsedCombo {
  if (!combo) return { shift: false, alt: false, key: "" };
  const tokens = combo.split("+");
  return {
    shift: tokens.includes("shift"),
    alt: tokens.includes("alt"),
    key: tokens[tokens.length - 1] ?? "",
  };
}

/** Match a letter key, handling both e.key and e.code to avoid IME "Process" issues. */
function isLetter(e: KeyboardEvent, letter: string): boolean {
  return e.key.toLowerCase() === letter || e.code === `Key${letter.toUpperCase()}`;
}

/**
 * Check whether a keyboard event matches a combo.
 * Requirements: mod (meta or ctrl) pressed, shift/alt state matches exactly, main key matches.
 */
export function matchCombo(e: KeyboardEvent, combo: string): boolean {
  if (!(e.metaKey || e.ctrlKey)) return false;
  const c = parseCombo(combo);
  if (!c.key) return false;
  if (e.shiftKey !== c.shift) return false;
  if (e.altKey !== c.alt) return false;
  return isLetter(e, c.key);
}

/**
 * Record a combo string from a keyboard event.
 * Must have mod held and main key is a single letter A-Z, otherwise returns null.
 * Prefers physical key code (e.code "KeyX") over e.key for layout-independence.
 */
export function comboFromEvent(e: KeyboardEvent): string | null {
  if (!(e.metaKey || e.ctrlKey)) return null;
  let letter: string | null = null;
  const m = /^Key([A-Z])$/.exec(e.code);
  if (m) letter = m[1].toLowerCase();
  else if (/^[a-zA-Z]$/.test(e.key)) letter = e.key.toLowerCase();
  if (!letter) return null;
  const parts = ["mod"];
  if (e.shiftKey) parts.push("shift");
  if (e.altKey) parts.push("alt");
  parts.push(letter);
  return parts.join("+");
}

/** Format a combo string for display: macOS uses symbols (⌘⇧F), others use + (Ctrl+Shift+F). */
export function formatCombo(combo: string): string {
  const c = parseCombo(combo);
  // Plain-browser clients bind to Ctrl+Alt even on macOS, so use the text form there.
  const symbols = IS_MAC && !IS_PLAIN_BROWSER;
  const parts: string[] = [];
  parts.push(symbols ? "\u2318" : "Ctrl");
  if (c.shift) parts.push(symbols ? "\u21E7" : "Shift");
  if (c.alt) parts.push(symbols ? "\u2325" : "Alt");
  parts.push(c.key.toUpperCase());
  return parts.join(symbols ? "" : "+");
}

/** Effective binding for an action: the user override when present, otherwise the platform default. */
export function effectiveCombo(
  action: ShortcutAction,
  overrides?: Partial<Record<ShortcutAction, string>>,
): string {
  return overrides?.[action] || DEFAULT_BINDINGS[action];
}

/**
 * Append an action's current binding to a UI label, e.g. `Save (⌘S)`.
 *
 * Tooltips must never hardcode a combo: the defaults differ per platform and per shell (see
 * IS_PLAIN_BROWSER), and users can rebind any action, so a literal hint goes stale silently.
 */
export function labelWithCombo(
  label: string,
  action: ShortcutAction,
  overrides?: Partial<Record<ShortcutAction, string>>,
): string {
  return `${label} (${formatCombo(effectiveCombo(action, overrides))})`;
}

/**
 * Physical key codes that app shortcuts claim with Ctrl+Alt, derived from the bindings in effect.
 *
 * usePtySession blocks these so xterm does not turn them into Meta escape sequences. Deriving the set
 * instead of hardcoding letters keeps a rebound action from leaking into the terminal.
 */
export function appAltKeyCodes(
  overrides?: Partial<Record<ShortcutAction, string>>,
): Set<string> {
  const codes = new Set<string>();
  for (const action of Object.keys(DEFAULT_BINDINGS) as ShortcutAction[]) {
    const parsed = parseCombo(effectiveCombo(action, overrides));
    if (parsed.alt && /^[a-z]$/.test(parsed.key)) {
      codes.add(`Key${parsed.key.toUpperCase()}`);
    }
  }
  return codes;
}
