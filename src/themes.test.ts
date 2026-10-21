//! App themes: saved choices are validated per mode, a named theme paints both the chrome (inline tokens) and the
//! terminal for its own mode only, classic removes every token it wrote, and the accent presets take theme hues.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { applyTheme, applyVisual, xtermTheme, XTERM_THEME, type VisualSettings } from "./theme";
import {
  CHROME_TOKENS,
  DARK_THEMES,
  LIGHT_THEMES,
  accentTokens,
  chromeTokens,
  mix,
  sanitizeDarkStyle,
  sanitizeLightStyle,
} from "./themes";

const visual = (overrides: Partial<VisualSettings>): VisualSettings => ({
  darkStyle: "classic",
  lightStyle: "classic",
  accent: "auto",
  density: "regular",
  paneStyle: "flush",
  dividerStyle: "subtle",
  navLayout: "tree",
  uiFontFamily: null,
  uiFontSize: null,
  chatFontFamily: null,
  chatFontSize: 13.5,
  chatLineHeight: 1.5,
  ...overrides,
});

const prop = (name: string) => document.documentElement.style.getPropertyValue(name);

describe("app themes", () => {
  beforeEach(() => applyTheme("dark"));
  afterEach(() => {
    applyTheme("dark");
    applyVisual(visual({}));
  });

  it("keeps a mode's own themes and falls back to classic for anything else", () => {
    expect(sanitizeDarkStyle("adventure-time")).toBe("adventure-time");
    expect(sanitizeLightStyle("solarized-light")).toBe("solarized-light");
    for (const value of ["soft", "crisp", "solarized-light", "toString", undefined, 3]) {
      expect(sanitizeDarkStyle(value)).toBe("classic");
    }
    expect(sanitizeLightStyle("adventure-time")).toBe("classic");
  });

  it("derives every chrome token for every theme, and keeps hand-pinned values", () => {
    for (const theme of [...Object.values(DARK_THEMES), ...Object.values(LIGHT_THEMES)]) {
      const tokens = chromeTokens(theme);
      for (const token of CHROME_TOKENS) expect(tokens[token]).toMatch(/^#[0-9a-f]{6}$/i);
      expect(tokens["--bg-term"]).toBe(theme.terminal.background);
      expect(tokens["--text"]).toBe(theme.terminal.foreground);
    }
    expect(chromeTokens(DARK_THEMES["adventure-time"])["--bg-0"]).toBe("#191738");
    expect(mix("#ffffff", "#000000", 50)).toBe("#808080");
  });

  it("puts the more legible end of the palette on every accent", () => {
    const ratio = (a: string, b: string) => {
      const lum = (hex: string) =>
        [0, 1, 2]
          .map((i) => parseInt(hex.slice(1 + i * 2, 3 + i * 2), 16) / 255)
          .map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4))
          .reduce((sum, c, i) => sum + c * [0.2126, 0.7152, 0.0722][i], 0);
      const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
      return (hi + 0.05) / (lo + 0.05);
    };
    for (const theme of [...Object.values(DARK_THEMES), ...Object.values(LIGHT_THEMES)]) {
      for (const hue of ["green", "blue", "amber", "violet"] as const) {
        const tokens = accentTokens(theme, hue);
        const other =
          tokens["--text-on-accent"] === theme.terminal.background ? theme.terminal.foreground : theme.terminal.background;
        expect(ratio(tokens["--accent"], tokens["--text-on-accent"])).toBeGreaterThanOrEqual(
          ratio(tokens["--accent"], other),
        );
      }
    }
    // Adventure Time's violet is mid-tone: indigo on it reads at about 5.8:1, cream only about 2.1:1.
    const at = DARK_THEMES["adventure-time"];
    expect(accentTokens(at, "violet")["--text-on-accent"]).toBe(at.terminal.background);
  });

  it("paints the chrome and the terminal in its own mode and removes everything for classic", () => {
    applyTheme("dark", "adventure-time");
    applyVisual(visual({ darkStyle: "adventure-time" }));
    expect(document.documentElement.dataset.darkStyle).toBe("adventure-time");
    expect(prop("--bg-term")).toBe("#1f1d45");
    expect(prop("--accent")).toBe("#9eff6e");
    expect(prop("--accent-dim")).toBe("#9eff6e1f");
    expect(xtermTheme("dark")).toBe(DARK_THEMES["adventure-time"].terminal);
    // Light mode keeps its own theme, classic here.
    expect(xtermTheme("light")).toBe(XTERM_THEME.light);

    applyTheme("dark", "classic");
    applyVisual(visual({}));
    for (const token of CHROME_TOKENS) expect(prop(token)).toBe("");
    expect(prop("--accent")).toBe("");
    expect(prop("--accent-dim")).toBe("");
    expect(xtermTheme("dark")).toBe(XTERM_THEME.dark);
  });

  it("uses the light theme in light mode", () => {
    applyTheme("light", "adventure-time", "solarized-light");
    applyVisual(visual({ darkStyle: "adventure-time", lightStyle: "solarized-light" }));
    expect(document.documentElement.dataset.lightStyle).toBe("solarized-light");
    expect(prop("--bg-term")).toBe("#fdf6e3");
    expect(xtermTheme("light")).toBe(LIGHT_THEMES["solarized-light"].terminal);
  });

  it("follows the chosen accent preset", () => {
    applyTheme("dark", "adventure-time");
    applyVisual(visual({ darkStyle: "adventure-time", accent: "amber" }));
    expect(prop("--accent")).toBe("#efbf38");
  });
});
