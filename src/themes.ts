//! Named app themes, chosen per mode in Settings → Appearance ("Dark theme" / "Light theme").
//!
//! "classic" is the built-in look: the `[data-theme]` token blocks in vlinx.css and `XTERM_THEME` in theme.ts.
//! A named theme is defined once, as a terminal palette, and paints both the terminal grid and the chrome:
//! `chromeTokens` derives the interface's surfaces, text levels, hairlines and status colors from that palette,
//! so a new theme is one palette entry. A theme may pin any token by hand where the derivation is not what its
//! authors intended; Adventure Time does, for every token, from the palette it is used with elsewhere.
//!
//! The derivation, for a dark theme: the terminal is the lit surface at the palette background, the frame sinks
//! below it, raised surfaces and hairlines are the foreground mixed into the background in small steps, text
//! levels are the foreground faded toward the background, and status colors are the palette's bright ANSI hues.
//! A light theme mirrors it with the regular ANSI hues, which are the ones drawn for a light background.

import type { ITheme } from "@xterm/xterm";

export type ThemeBase = "dark" | "light";
export type AccentHue = "green" | "blue" | "amber" | "violet";

/** The CSS custom properties a theme sets. Everything else (hover washes, shadows, vendor marks) comes from the
 *  `[data-theme]` block for the theme's base. */
export const CHROME_TOKENS = [
  "--bg-0", "--bg-1", "--bg-2", "--bg-term", "--border", "--border-strong",
  "--text", "--text-mid", "--text-dim", "--text-faint", "--term-cmd", "--term-out",
  "--green", "--yellow", "--red", "--cyan", "--mag", "--blue",
  "--c-blue", "--c-yellow", "--c-cyan", "--c-mag", "--c-red", "--c-green",
] as const;
export type ChromeToken = (typeof CHROME_TOKENS)[number];

export interface ThemeDef {
  label: string;
  base: ThemeBase;
  /** Every color as #rrggbb: the derivation mixes them. */
  terminal: Required<Pick<ITheme,
    | "background" | "foreground" | "cursor" | "cursorAccent" | "selectionBackground" | "selectionInactiveBackground"
    | "black" | "red" | "green" | "yellow" | "blue" | "magenta" | "cyan" | "white"
    | "brightBlack" | "brightRed" | "brightGreen" | "brightYellow" | "brightBlue" | "brightMagenta" | "brightCyan"
    | "brightWhite">>;
  /** Tokens pinned by hand instead of derived. */
  chrome?: Partial<Record<ChromeToken, string>>;
  /** Accent presets pinned by hand instead of taken from the status colors. */
  accents?: Partial<Record<AccentHue, string>>;
}

export const DARK_THEMES = {
  "adventure-time": {
    label: "Adventure Time",
    base: "dark",
    terminal: {
      background: "#1f1d45", foreground: "#f8dcc0", cursor: "#efbf38", cursorAccent: "#08080a",
      selectionBackground: "#706b4e", selectionInactiveBackground: "#4c484a",
      black: "#050404", red: "#bd0013", green: "#4ab118", yellow: "#e7741e",
      blue: "#0f4ac6", magenta: "#665993", cyan: "#70a598", white: "#f8dcc0",
      brightBlack: "#4e7cbf", brightRed: "#fc5f5a", brightGreen: "#9eff6e", brightYellow: "#efc11a",
      brightBlue: "#1997c6", brightMagenta: "#9b5953", brightCyan: "#c8faf4", brightWhite: "#f6f5fb",
    },
    // Surfaces hold the background's hue at other lightnesses, and status colors are the ANSI hues lifted to
    // clear small text on indigo, so the chrome matches the palette as used across other terminals and editors.
    chrome: {
      "--bg-0": "#191738", "--bg-1": "#191738", "--bg-2": "#28265a", "--bg-term": "#1f1d45",
      "--border": "#322f6f", "--border-strong": "#3f3b8c",
      "--text": "#f8dcc0", "--text-mid": "#a099c2", "--text-dim": "#6a6293", "--text-faint": "#4b4478",
      "--term-cmd": "#f8dcc0", "--term-out": "#bba79e",
      "--green": "#9eff6e", "--yellow": "#efc11a", "--red": "#fc5f5a", "--cyan": "#86c7b5", "--mag": "#a290df",
      "--blue": "#35aedc",
      "--c-blue": "#2a729a", "--c-yellow": "#997d26", "--c-cyan": "#5a8183", "--c-mag": "#6b609c", "--c-red": "#a1424c",
      "--c-green": "#69a258",
    },
    accents: { green: "#9eff6e", blue: "#35aedc", amber: "#efbf38", violet: "#a290df" },
  },
  "campbell-powershell": {
    label: "Campbell PowerShell",
    base: "dark",
    terminal: {
      background: "#012456", foreground: "#cccccc", cursor: "#ffffff", cursorAccent: "#012456",
      selectionBackground: "#3a5a8c", selectionInactiveBackground: "#2a4470",
      black: "#0c0c0c", red: "#c50f1f", green: "#13a10e", yellow: "#c19c00",
      blue: "#0037da", magenta: "#881798", cyan: "#3a96dd", white: "#cccccc",
      brightBlack: "#767676", brightRed: "#e74856", brightGreen: "#16c60c", brightYellow: "#f9f1a5",
      brightBlue: "#3b78ff", brightMagenta: "#b4009e", brightCyan: "#61d6d6", brightWhite: "#f2f2f2",
    },
  },
  dracula: {
    label: "Dracula",
    base: "dark",
    terminal: {
      background: "#282a36", foreground: "#f8f8f2", cursor: "#f8f8f2", cursorAccent: "#282a36",
      selectionBackground: "#44475a", selectionInactiveBackground: "#383a4a",
      black: "#21222c", red: "#ff5555", green: "#50fa7b", yellow: "#f1fa8c",
      blue: "#bd93f9", magenta: "#ff79c6", cyan: "#8be9fd", white: "#f8f8f2",
      brightBlack: "#6272a4", brightRed: "#ff6e6e", brightGreen: "#69ff94", brightYellow: "#ffffa5",
      brightBlue: "#d6acff", brightMagenta: "#ff92df", brightCyan: "#a4ffff", brightWhite: "#ffffff",
    },
  },
  "gruvbox-dark": {
    label: "Gruvbox Dark",
    base: "dark",
    terminal: {
      background: "#282828", foreground: "#ebdbb2", cursor: "#ebdbb2", cursorAccent: "#282828",
      selectionBackground: "#504945", selectionInactiveBackground: "#3c3836",
      black: "#282828", red: "#cc241d", green: "#98971a", yellow: "#d79921",
      blue: "#458588", magenta: "#b16286", cyan: "#689d6a", white: "#a89984",
      brightBlack: "#928374", brightRed: "#fb4934", brightGreen: "#b8bb26", brightYellow: "#fabd2f",
      brightBlue: "#83a598", brightMagenta: "#d3869b", brightCyan: "#8ec07c", brightWhite: "#ebdbb2",
    },
  },
  nord: {
    label: "Nord",
    base: "dark",
    terminal: {
      background: "#2e3440", foreground: "#d8dee9", cursor: "#d8dee9", cursorAccent: "#2e3440",
      selectionBackground: "#434c5e", selectionInactiveBackground: "#3b4252",
      black: "#3b4252", red: "#bf616a", green: "#a3be8c", yellow: "#ebcb8b",
      blue: "#81a1c1", magenta: "#b48ead", cyan: "#88c0d0", white: "#e5e9f0",
      brightBlack: "#4c566a", brightRed: "#bf616a", brightGreen: "#a3be8c", brightYellow: "#ebcb8b",
      brightBlue: "#81a1c1", brightMagenta: "#b48ead", brightCyan: "#8fbcbb", brightWhite: "#eceff4",
    },
  },
  "tokyo-night": {
    label: "Tokyo Night",
    base: "dark",
    terminal: {
      background: "#1a1b26", foreground: "#c0caf5", cursor: "#c0caf5", cursorAccent: "#1a1b26",
      selectionBackground: "#33467c", selectionInactiveBackground: "#283457",
      black: "#15161e", red: "#f7768e", green: "#9ece6a", yellow: "#e0af68",
      blue: "#7aa2f7", magenta: "#bb9af7", cyan: "#7dcfff", white: "#a9b1d6",
      brightBlack: "#414868", brightRed: "#f7768e", brightGreen: "#9ece6a", brightYellow: "#e0af68",
      brightBlue: "#7aa2f7", brightMagenta: "#bb9af7", brightCyan: "#7dcfff", brightWhite: "#c0caf5",
    },
  },
} satisfies Record<string, ThemeDef>;

export const LIGHT_THEMES = {
  "one-half-light": {
    label: "One Half Light",
    base: "light",
    // `white` and `brightWhite` are darkened from the published near-background values so white text stays
    // visible on the light grid, as the classic light palette does.
    terminal: {
      background: "#fafafa", foreground: "#383a42", cursor: "#383a42", cursorAccent: "#fafafa",
      selectionBackground: "#bfceff", selectionInactiveBackground: "#dde4f5",
      black: "#383a42", red: "#e45649", green: "#50a14f", yellow: "#c18401",
      blue: "#0184bc", magenta: "#a626a4", cyan: "#0997b3", white: "#a0a1a7",
      brightBlack: "#4f525e", brightRed: "#e06c75", brightGreen: "#98c379", brightYellow: "#e5c07b",
      brightBlue: "#61afef", brightMagenta: "#c678dd", brightCyan: "#56b6c2", brightWhite: "#2c2e33",
    },
  },
  "solarized-light": {
    label: "Solarized Light",
    base: "light",
    terminal: {
      background: "#fdf6e3", foreground: "#657b83", cursor: "#586e75", cursorAccent: "#fdf6e3",
      selectionBackground: "#eee8d5", selectionInactiveBackground: "#f3eedc",
      black: "#073642", red: "#dc322f", green: "#859900", yellow: "#b58900",
      blue: "#268bd2", magenta: "#d33682", cyan: "#2aa198", white: "#93a1a1",
      brightBlack: "#586e75", brightRed: "#cb4b16", brightGreen: "#586e75", brightYellow: "#657b83",
      brightBlue: "#839496", brightMagenta: "#6c71c4", brightCyan: "#93a1a1", brightWhite: "#002b36",
    },
  },
} satisfies Record<string, ThemeDef>;

export type DarkThemeName = keyof typeof DARK_THEMES;
export type LightThemeName = keyof typeof LIGHT_THEMES;
/** A mode's theme: "classic" or a named theme of that base. */
export type DarkStyle = "classic" | DarkThemeName;
export type LightStyle = "classic" | LightThemeName;

/** The named theme a style refers to, or null for "classic" and for anything unrecognised. */
export function namedTheme(base: ThemeBase, style: unknown): ThemeDef | null {
  const table: Record<string, ThemeDef> = base === "dark" ? DARK_THEMES : LIGHT_THEMES;
  return typeof style === "string" && Object.hasOwn(table, style) ? table[style] : null;
}

/** Restore a saved style; retired, unknown and other-base values fall back to classic. */
export function sanitizeDarkStyle(value: unknown): DarkStyle {
  return namedTheme("dark", value) ? (value as DarkThemeName) : "classic";
}
export function sanitizeLightStyle(value: unknown): LightStyle {
  return namedTheme("light", value) ? (value as LightThemeName) : "classic";
}

/** Blend `pct`% of `fg` into `bg`, per sRGB channel. Both are #rrggbb. */
export function mix(fg: string, bg: string, pct: number): string {
  const channel = (hex: string, i: number) => parseInt(hex.slice(1 + i * 2, 3 + i * 2), 16);
  return `#${[0, 1, 2]
    .map((i) => Math.round((channel(fg, i) * pct + channel(bg, i) * (100 - pct)) / 100).toString(16).padStart(2, "0"))
    .join("")}`;
}

/** WCAG contrast ratio between two #rrggbb colors. */
function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

/** WCAG relative luminance of #rrggbb. */
function luminance(hex: string): number {
  const [r, g, b] = [0, 1, 2].map((i) => {
    const c = parseInt(hex.slice(1 + i * 2, 3 + i * 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** The chrome tokens a theme sets: derived from its palette, then any it pins by hand. */
export function chromeTokens(theme: ThemeDef): Record<ChromeToken, string> {
  const t = theme.terminal;
  const dark = theme.base === "dark";
  const bg = t.background;
  const fg = t.foreground;
  const frame = dark ? mix("#000000", bg, 18) : mix("#000000", bg, 4);
  const status = dark
    ? { green: t.brightGreen, yellow: t.brightYellow, red: t.brightRed, cyan: t.brightCyan, mag: t.brightMagenta, blue: t.brightBlue }
    : { green: t.green, yellow: t.yellow, red: t.red, cyan: t.cyan, mag: t.magenta, blue: t.blue };
  // File-type icons are the status hues with reduced chroma, pulled toward the sidebar they sit on.
  const icon = (c: string) => mix(c, frame, dark ? 60 : 70);
  const derived: Record<ChromeToken, string> = {
    "--bg-0": frame,
    "--bg-1": frame,
    "--bg-2": dark ? mix(fg, bg, 8) : mix("#000000", bg, 7),
    "--bg-term": bg,
    "--border": mix(fg, bg, 14),
    "--border-strong": mix(fg, bg, 24),
    "--text": fg,
    "--text-mid": mix(fg, bg, 80),
    "--text-dim": mix(fg, bg, 62),
    "--text-faint": mix(fg, bg, 45),
    "--term-cmd": fg,
    "--term-out": mix(fg, bg, 72),
    "--green": status.green, "--yellow": status.yellow, "--red": status.red,
    "--cyan": status.cyan, "--mag": status.mag, "--blue": status.blue,
    "--c-blue": icon(status.blue), "--c-yellow": icon(status.yellow), "--c-cyan": icon(status.cyan),
    "--c-mag": icon(status.mag), "--c-red": icon(status.red), "--c-green": icon(status.green),
  };
  return { ...derived, ...theme.chrome };
}

/** The accent tokens for one preset: the theme's own hue for it, a soft wash, a line, and legible text on it. */
export function accentTokens(theme: ThemeDef, hue: AccentHue): Record<string, string> {
  const chrome = chromeTokens(theme);
  const fallback: Record<AccentHue, string> = {
    green: chrome["--green"], blue: chrome["--blue"], amber: chrome["--yellow"], violet: chrome["--mag"],
  };
  const accent = theme.accents?.[hue] ?? fallback[hue];
  const soft = theme.base === "dark" ? "29" : "24";
  // Text on an accent is whichever end of the palette contrasts with it more.
  const [darkest, lightest] = theme.base === "dark"
    ? [theme.terminal.background, theme.terminal.foreground]
    : [theme.terminal.foreground, theme.terminal.background];
  return {
    "--accent": accent,
    "--accent-soft": `${accent}${soft}`,
    "--accent-line": `${accent}80`,
    // Selected-row tint; read as var(--accent-dim, …) where a stock-green rgba fallback would otherwise show.
    "--accent-dim": `${accent}1f`,
    "--accent-green-swatch": hue === "green" ? accent : chrome["--green"],
    "--text-on-accent": contrast(accent, darkest) >= contrast(accent, lightest) ? darkest : lightest,
  };
}
