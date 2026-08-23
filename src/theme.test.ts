import { beforeEach, describe, expect, it, vi } from "vitest";

// applyTheme pushes the resolved palette into the live terminal registry; stub it so the module can be
// exercised without any xterm instances.
vi.mock("./terminal/registry", () => ({ setXtermTheme: vi.fn() }));

import { applyTheme } from "./theme";

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
    localStorage.clear();
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
});
