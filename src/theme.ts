//! Appearance (color scheme + Vlinx design tokens), switched through data-* attributes on documentElement.
//! - ThemeMode has three states: system follows the OS, plus explicit dark and light. system resolves to the
//!   current effective scheme.
//! - Vlinx visual tokens cover accent, density, paneStyle, dividerStyle, and navLayout, written as
//!   data-accent|density|pane|divider|nav.
//! xterm colors stay synchronized with the resolved scheme through the registry.

import type { ITheme } from "@xterm/xterm";
import { invoke, isTauri } from "./ipc/transport";
import { setXtermTheme } from "./terminal/registry";

/** User-selected color-scheme mode; this is the persisted value. */
export type ThemeMode = "system" | "dark" | "light";
/** Effective scheme used for rendering; system resolves to one of these values. */
export type ResolvedTheme = "dark" | "light";
/** Legacy store name, equivalent to ThemeMode. */
export type Theme = ThemeMode;

/** Accent color: 'auto' follows the scheme (dark→green, light→blue); other choices are fixed. */
export type AccentName = "green" | "blue" | "amber" | "violet";
export type AccentChoice = AccentName | "auto";
/** Interface density. */
export type Density = "compact" | "regular" | "comfy";
/** Pane style: flush for seamless iTerm2-style panes, or card for rounded cards. */
export type PaneStyle = "flush" | "card";
/** Divider style: subtle is hairline-thin; visible is prominent. */
export type DividerStyle = "subtle" | "visible";
/** Sidebar layout: tree is standard; compact hides group icons and uses shorter rows. */
export type NavLayout = "tree" | "compact";
/** Active tab in the right-side Inspector. */
export type InspectorTab = "files" | "info" | "git";

/** Vlinx visual settings: design tokens beyond the color scheme. */
export interface VisualSettings {
  accent: AccentChoice;
  density: Density;
  paneStyle: PaneStyle;
  dividerStyle: DividerStyle;
  navLayout: NavLayout;
  /** UI monospace font. null uses CSS --font-mono from vlinx.css; a value names the primary font and receives a fallback chain. */
  uiFontFamily: string | null;
  /** UI font size in pixels. null follows density without an inline --ui-fs; a value overrides the density size inline. */
  uiFontSize: number | null;
}

/**
 * Default monospace stack, kept in sync with --font-mono on :root in vlinx.css.
 * Add "VlxSymbols" before monospace because programming fonts commonly lack newer symbols such as the U+23F5 ⏵
 * media triangle. The embedded subset font (see styles/fonts.css) works offline without a system installation and
 * avoids missing-glyph boxes □. Keep system "Symbola" afterward as an additional optional fallback.
 */
const CJK_FALLBACK =
  '"PingFang SC", "Microsoft YaHei", "Noto Sans CJK SC", "Noto Sans SC"';

export const DEFAULT_MONO_STACK =
  `"JetBrains Mono", ui-monospace, "SF Mono", Menlo, "VlxSymbols", "Symbola", ${CJK_FALLBACK}, monospace`;

/**
 * Build a font-family string with fallbacks from a primary font name:
 * - empty → default stack (JetBrains Mono and others);
 * - contains a comma → return the user-supplied complete fallback chain unchanged;
 * - ordinary name → quote names containing spaces and append generic monospace fallbacks so a missing font never
 *   falls through to serif.
 */
export function fontStack(family: string | null | undefined): string {
  const f = family?.trim();
  if (!f) return DEFAULT_MONO_STACK;
  if (f.includes(",")) return f;
  const quoted = /\s/.test(f) ? `"${f}"` : f;
  return `${quoted}, ui-monospace, "SF Mono", Menlo, "VlxSymbols", "Symbola", ${CJK_FALLBACK}, monospace`;
}

export const XTERM_THEME: Record<ResolvedTheme, ITheme> = {
  // Match the background to Vlinx --bg-term and the foreground to --text so xterm blends with the shell.
  // Define separate 16-color ANSI palettes for light and dark schemes. Agents such as Claude render explicit
  // ANSI reds, greens, yellows, and grays; switching the entire palette avoids the washed-out contrast caused by
  // retaining a dark palette on a light background when only foreground/background change. Hues match project
  // design tokens (red~22, green~158, yellow~90, cyan~215, magenta~320), with brighter/lighter saturation on dark
  // backgrounds and darker values on light backgrounds for contrast.
  // selectionBackground must be explicit: xterm defaults to 30%-opaque white, which is invisible on a light
  // background and appears as if selection is broken. selectionInactiveBackground colors selections in unfocused
  // split panes; cursorAccent is the inverse text color inside a block cursor.
  dark: {
    background: "oklch(0.175 0.006 260)", foreground: "#e9eaeb", cursor: "#e9eaeb",
    cursorAccent: "#212327",
    selectionBackground: "#3a4f6e", selectionInactiveBackground: "#323c4e",
    black: "#41454c", red: "#f0726b", green: "#4fc08d", yellow: "#e3c46a",
    blue: "#6aa0f7", magenta: "#d98fd0", cyan: "#5ec8d8", white: "#c9ccd1",
    brightBlack: "#6b7079", brightRed: "#ff8a82", brightGreen: "#62d6a0",
    brightYellow: "#f2d585", brightBlue: "#86b4ff", brightMagenta: "#eaa6e0",
    brightCyan: "#79dcea", brightWhite: "#f3f4f5",
  },
  light: {
    // Keep pure #ffffff in lockstep with vlinx.css --bg-term (light).
    background: "#ffffff", foreground: "#3c3e42", cursor: "#3c3e42",
    cursorAccent: "#ffffff",
    selectionBackground: "#b8d2f5", selectionInactiveBackground: "#d8e2ef",
    black: "#2a2d33", red: "#c4332b", green: "#1f8a5f", yellow: "#9a7a16",
    blue: "#2f6bd6", magenta: "#a843a0", cyan: "#1f8390", white: "#b7bbc1",
    brightBlack: "#5a5f68", brightRed: "#d84840", brightGreen: "#2aa06f",
    brightYellow: "#b08f25", brightBlue: "#3f7ce6", brightMagenta: "#b955b0",
    brightCyan: "#2f96a3", brightWhite: "#1c1e22",
  },
};

const STORAGE_KEY = "vlx-theme";
const SYSTEM_QUERY = "(prefers-color-scheme: dark)";

/** Return the operating system's current light/dark scheme. */
export function getSystemTheme(): ResolvedTheme {
  return window.matchMedia(SYSTEM_QUERY).matches ? "dark" : "light";
}

/** Resolve a mode to the effective rendering scheme. */
export function resolveTheme(mode: ThemeMode): ResolvedTheme {
  return mode === "system" ? getSystemTheme() : mode;
}

export function loadTheme(): ThemeMode {
  const v = localStorage.getItem(STORAGE_KEY);
  return v === "dark" || v === "light" ? v : "system";
}

/** Resolve the 'auto' accent from the current scheme: dark→green, light→blue. */
export function effectiveAccent(
  accent: AccentChoice,
  resolved: ResolvedTheme,
): AccentName {
  if (accent === "auto") return resolved === "dark" ? "green" : "blue";
  return accent;
}

/**
 * Match the native window chrome to the scheme. The app keeps the system title bar, and Windows paints it
 * light until DWM is told otherwise, so a dark UI carries a white strip above it. The mode goes over as-is
 * rather than the resolved scheme: `system` hands control back to the OS, which is what that mode means, and
 * pinning a value there would freeze the title bar the next time the OS scheme changed. The backend ignores
 * this on macOS and Linux, where the window theme is an app-wide override rather than title-bar tinting.
 *
 * Only the desktop shell calls this. Remote/SSH windows are native windows too, but they run in browser
 * transport and cannot reach native commands; the backend applies the same value to every window it owns and
 * builds later ones with it, so they follow without asking.
 */
function syncNativeChrome(mode: ThemeMode) {
  if (!isTauri) return;
  void invoke("set_native_theme", { mode }).catch(() => {
    /* Chrome tinting is cosmetic; an older backend without the command must not break theming. */
  });
}

/** Apply a scheme: resolve it, write data-theme, persist the mode, and synchronize xterm colors. */
export function applyTheme(mode: ThemeMode) {
  const resolved = resolveTheme(mode);
  document.documentElement.dataset.theme = resolved;
  localStorage.setItem(STORAGE_KEY, mode);
  setXtermTheme(XTERM_THEME[resolved]);
  syncNativeChrome(mode);
}

/**
 * Apply Vlinx visual tokens beyond the color scheme to documentElement data-* attributes. Call after applyTheme
 * because the 'auto' accent relies on data-theme to resolve the effective scheme.
 */
export function applyVisual(s: VisualSettings) {
  const root = document.documentElement;
  const resolved = (root.dataset.theme as ResolvedTheme) || resolveTheme("system");
  root.dataset.accent = effectiveAccent(s.accent, resolved);
  root.dataset.density = s.density;
  root.dataset.pane = s.paneStyle;
  root.dataset.divider = s.dividerStyle;
  root.dataset.nav = s.navLayout;

  // Write UI font family/size inline on documentElement to override CSS. Inline priority lets font size supersede
  // the --ui-fs tier selected by density, while density still controls line height and spacing. Empty values remove
  // the inline override and fall back to CSS (--font-mono's default stack / density's --ui-fs).
  if (s.uiFontFamily) root.style.setProperty("--font-mono", fontStack(s.uiFontFamily));
  else root.style.removeProperty("--font-mono");
  if (s.uiFontSize != null) root.style.setProperty("--ui-fs", `${s.uiFontSize}px`);
  else root.style.removeProperty("--ui-fs");
}

/** Listen for operating-system scheme changes and return an unsubscribe function. */
export function watchSystemTheme(cb: () => void): () => void {
  const mql = window.matchMedia(SYSTEM_QUERY);
  mql.addEventListener("change", cb);
  return () => mql.removeEventListener("change", cb);
}
