import { beforeEach, describe, expect, it, vi } from "vitest";

// applyTheme pushes the resolved palette into the live terminal registry; stub it so the module can be
// exercised without any xterm instances.
vi.mock("./terminal/registry", () => ({ setXtermTheme: vi.fn() }));

import { applyConversationTypography, applyTheme, DEFAULT_MONO_STACK, XTERM_THEME, xtermTheme, type VisualSettings } from "./theme";
import { setXtermTheme } from "./terminal/registry";

/** jsdom has no matchMedia; report the OS as the given scheme so 'system' resolves predictably. */
function mockSystemScheme(scheme: "dark" | "light") {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: query.includes("dark") === (scheme === "dark"),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
}

describe("applyTheme", () => {
  beforeEach(() => {
    document.documentElement.removeAttribute("style");
    document.documentElement.removeAttribute("data-theme");
    document.documentElement.removeAttribute("data-dark-style");
    localStorage.clear();
    vi.clearAllMocks();
  });

  // color-scheme decides how the user agent paints native in-page controls. index.html seeds it from
  // prefers-color-scheme, so an explicit theme must override it or native checkboxes, selects, and
  // scrollbars keep following the OS while the rest of the app switches.
  it.each(["dark", "light"] as const)("sets data-theme and color-scheme to %s", (mode) => {
    mockSystemScheme(mode === "dark" ? "light" : "dark"); // opposite OS scheme: the explicit mode must win
    applyTheme(mode);
    expect(document.documentElement.dataset.theme).toBe(mode);
    expect(document.documentElement.style.getPropertyValue("color-scheme")).toBe(mode);
  });

  it("resolves 'system' to the OS scheme", () => {
    mockSystemScheme("dark");
    applyTheme("system");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.style.getPropertyValue("color-scheme")).toBe("dark");
    expect(localStorage.getItem("vlx-theme")).toBe("system"); // the mode persists, not the resolved scheme
  });

  it("applies classic dark to chrome and live terminals by default", () => {
    applyTheme("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.dataset.darkStyle).toBe("classic");
    expect(localStorage.getItem("vlx-theme")).toBe("dark");
    expect(setXtermTheme).toHaveBeenCalledWith(XTERM_THEME.dark);
    expect(xtermTheme("dark")).toBe(XTERM_THEME.dark);
  });

  it("uses the light terminal palette while retaining the compatibility field", () => {
    applyTheme("light");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(document.documentElement.dataset.darkStyle).toBe("classic");
    expect(setXtermTheme).toHaveBeenCalledWith(XTERM_THEME.light);
  });

  it("restores classic dark when the system returns to dark", () => {
    mockSystemScheme("light");
    applyTheme("system");
    expect(setXtermTheme).toHaveBeenLastCalledWith(XTERM_THEME.light);
    mockSystemScheme("dark");
    applyTheme("system");
    expect(setXtermTheme).toHaveBeenLastCalledWith(XTERM_THEME.dark);
    expect(localStorage.getItem("vlx-theme")).toBe("system");
  });
});

describe("conversation typography", () => {
  const apply = (chatFontFamily: string | null) => applyConversationTypography({ chatFontFamily, chatFontSize: 13.5, chatLineHeight: 1.2 } as VisualSettings);
  const style = document.documentElement.style;

  it("leaves Inter and its tracking to the stylesheet by default", () => {
    apply("Menlo");
    apply(null);
    expect(style.getPropertyValue("--chat-font")).toBe("");
    expect(style.getPropertyValue("--chat-tracking")).toBe("");
  });

  it("gives a chosen font its own spacing", () => {
    apply("Menlo");
    expect(style.getPropertyValue("--chat-font")).toContain("Menlo");
    expect(style.getPropertyValue("--chat-font")).not.toBe(DEFAULT_MONO_STACK);
    expect(style.getPropertyValue("--chat-tracking")).toBe("0px");
  });
});
