//! Customizable global shortcut registry + combo key encode/decode utilities.
//!
//! Design:
//! - "mod" is a cross-platform modifier abstraction: Cmd (metaKey) on desktop macOS, Ctrl everywhere
//!   else (including plain browsers on macOS, which bind Ctrl+Alt). Only the modifier the current
//!   shell actually binds counts as mod - see hasMod().
//! - Combos are encoded as strings in canonical order: `mod[+shift][+alt]+<letter>` (lowercase).
//!   An explicit `cmd` modifier supports macOS browser splits alongside existing Ctrl bindings.
//!   e.g. "mod+t", "cmd+d", "cmd+shift+d". A primary modifier is required; main key is
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
 * Whether this client is a plain browser (URL remote access). Most actions use Ctrl+Alt to avoid
 * browser commands such as new/close tab. macOS splits use explicit Cmd bindings; the global
 * capture listener cancels the browser default when it handles a split.
 */
export const IS_PLAIN_BROWSER = env.isBrowser && !env.isRemoteWindow;

/** Whether "mod" means Cmd on this client. Desktop macOS binds Cmd; every other shell binds Ctrl. */
const MOD_IS_CMD = IS_MAC && !IS_PLAIN_BROWSER;

/**
 * Whether the event carries this client's mod modifier, and only that one.
 *
 * Accepting "metaKey || ctrlKey" everywhere would make Ctrl+letter fire the macOS Cmd bindings: on a
 * Mac, Ctrl+D (EOF) opened a split, Ctrl+W (delete word) closed a pane, and Ctrl+F/Ctrl+T/Ctrl+O were
 * swallowed the same way. Those are shell keys and must stay shell keys, so require exactly the
 * modifier the platform binds and reject the other one.
 */
export function hasMod(e: KeyboardEvent): boolean {
  return MOD_IS_CMD ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
}

/**
 * Default keybindings per platform:
 * - macOS (shell): Cmd+key (split up/down: Cmd+Shift+D, global search: Cmd+Shift+F).
 * - macOS plain browsers: Cmd+D / Cmd+Shift+D for splits, with Ctrl bindings for other actions.
 * - Windows/Linux and other plain-browser actions: Ctrl+Alt+key. Bare Ctrl+letter is a shell
 *   reserved key (Ctrl+D=EOF, Ctrl+W=delete-word...), Ctrl+Shift is swallowed by IMEs, Alt is
 *   terminal Meta. Ctrl+Alt avoids conflicts with browser tab commands.
 *   Split: D=right / E=up-down. Global search: G (global) vs inline search F.
 *   Save remains bare Ctrl+S (intercepted only on doc tabs; reserved as XOFF in terminals).
 *   When the terminal is focused, these Ctrl+Alt combos are intercepted by usePtySession's
 *   customKeyEventHandler to prevent xterm from treating them as Meta (see APP_ALT_KEYS).
 *   Users can override individual bindings via shortcutOverrides in settings.
 *   Recorded combos use "mod[+shift][+alt]+key", or explicit "cmd" for macOS browser Cmd chords.
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
        splitRight: IS_MAC ? "cmd+d" : "mod+alt+d",
        splitDown: IS_MAC ? "cmd+shift+d" : "mod+alt+e",
        search: "mod+alt+f",
        globalSearch: "mod+alt+g",
        saveDoc: "mod+s",
      };

interface ParsedCombo {
  cmd: boolean;
  shift: boolean;
  alt: boolean;
  /** Main key: a single lowercase letter. */
  key: string;
}

/** Parse a combo string into structured parts. Returns empty key on empty/invalid input. */
function parseCombo(combo: string): ParsedCombo {
  if (!combo) return { cmd: false, shift: false, alt: false, key: "" };
  const tokens = combo.split("+");
  return {
    cmd: tokens[0] === "cmd",
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
 * Requirements: the specified primary modifier, exact shift/alt state, and matching main key.
 */
export function matchCombo(e: KeyboardEvent, combo: string): boolean {
  const c = parseCombo(combo);
  if (c.cmd ? !(IS_MAC && e.metaKey && !e.ctrlKey) : !hasMod(e)) return false;
  if (!c.key) return false;
  if (e.shiftKey !== c.shift) return false;
  if (e.altKey !== c.alt) return false;
  return isLetter(e, c.key);
}

/**
 * Record a combo string from a keyboard event.
 * Accept this client's mod or explicit Cmd on macOS browsers, with a single letter A-Z.
 * Prefers physical key code (e.code "KeyX") over e.key for layout-independence.
 */
export function comboFromEvent(e: KeyboardEvent): string | null {
  const explicitCmd = IS_MAC && IS_PLAIN_BROWSER && e.metaKey && !e.ctrlKey;
  if (!explicitCmd && !hasMod(e)) return null;
  let letter: string | null = null;
  const m = /^Key([A-Z])$/.exec(e.code);
  if (m) letter = m[1].toLowerCase();
  else if (/^[a-zA-Z]$/.test(e.key)) letter = e.key.toLowerCase();
  if (!letter) return null;
  const parts = [explicitCmd ? "cmd" : "mod"];
  if (e.shiftKey) parts.push("shift");
  if (e.altKey) parts.push("alt");
  parts.push(letter);
  return parts.join("+");
}

/** Format a combo string for display: macOS uses symbols (⌘⇧F), others use + (Ctrl+Shift+F). */
export function formatCombo(combo: string): string {
  const c = parseCombo(combo);
  // Browser Cmd splits use symbols; existing browser Ctrl bindings retain the text form.
  const symbols = IS_MAC && (c.cmd || !IS_PLAIN_BROWSER);
  const parts: string[] = [];
  parts.push(symbols ? "\u2318" : c.cmd ? "Cmd" : "Ctrl");
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
    if (!parsed.cmd && parsed.alt && /^[a-z]$/.test(parsed.key)) {
      codes.add(`Key${parsed.key.toUpperCase()}`);
    }
  }
  return codes;
}
