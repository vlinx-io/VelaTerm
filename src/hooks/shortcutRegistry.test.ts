//! Regression tests for platform/browser-aware shortcut defaults.
//!
//! Desktop macOS shells bind Cmd combos; Windows/Linux and plain-browser clients (URL remote access)
//! bind Ctrl+Alt combos because browsers consume Cmd/Ctrl letter keys before the page sees them.
//! Remote-connection windows are WebViews without browser reservations, so they keep Cmd on macOS.

import { afterEach, describe, expect, it, vi } from "vitest";

const envState = vi.hoisted(() => ({
  isBrowser: false,
  isRemoteWindow: false,
}));

// The module under test reads only these two env fields; share the mutable object so each
// test can flip the shell without re-registering the mock.
vi.mock("../platform", () => ({ env: envState }));

const MAC_NAVIGATOR = {
  platform: "MacIntel",
  userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
};

const WIN_NAVIGATOR = {
  platform: "Win32",
  userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
};

/** Reload the module so its module-scope constants are recomputed for the current shell. */
async function loadRegistry() {
  vi.resetModules();
  return await import("./shortcutRegistry");
}

/** Build a minimal keyboard-event stand-in for matchCombo/comboFromEvent. */
function keyEvent(
  key: string,
  mods: { meta?: boolean; ctrl?: boolean; shift?: boolean; alt?: boolean } = {},
): KeyboardEvent {
  return {
    key,
    code: `Key${key.toUpperCase()}`,
    metaKey: !!mods.meta,
    ctrlKey: !!mods.ctrl,
    shiftKey: !!mods.shift,
    altKey: !!mods.alt,
  } as KeyboardEvent;
}

describe("shortcut defaults per shell", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    envState.isBrowser = false;
    envState.isRemoteWindow = false;
  });

  it("macOS desktop shell uses Cmd bindings and symbol formatting", async () => {
    vi.stubGlobal("navigator", MAC_NAVIGATOR);
    envState.isBrowser = false;
    envState.isRemoteWindow = false;
    const { DEFAULT_BINDINGS, IS_PLAIN_BROWSER, formatCombo } = await loadRegistry();

    expect(IS_PLAIN_BROWSER).toBe(false);
    expect(DEFAULT_BINDINGS.splitRight).toBe("mod+d");
    expect(DEFAULT_BINDINGS.splitDown).toBe("mod+shift+d");
    expect(DEFAULT_BINDINGS.newTab).toBe("mod+t");
    expect(formatCombo("mod+shift+d")).toBe("\u2318\u21E7D");
  });

  it("macOS plain browser uses Ctrl+Alt bindings because the browser eats Cmd combos", async () => {
    vi.stubGlobal("navigator", MAC_NAVIGATOR);
    envState.isBrowser = true;
    envState.isRemoteWindow = false;
    const { DEFAULT_BINDINGS, IS_PLAIN_BROWSER, formatCombo } = await loadRegistry();

    expect(IS_PLAIN_BROWSER).toBe(true);
    expect(DEFAULT_BINDINGS.splitRight).toBe("mod+alt+d");
    expect(DEFAULT_BINDINGS.splitDown).toBe("mod+alt+e");
    expect(DEFAULT_BINDINGS.newTab).toBe("mod+alt+t");
    expect(formatCombo("mod+alt+d")).toBe("Ctrl+Alt+D");
  });

  it("macOS remote-connection window keeps Cmd bindings", async () => {
    vi.stubGlobal("navigator", MAC_NAVIGATOR);
    envState.isBrowser = true;
    envState.isRemoteWindow = true;
    const { DEFAULT_BINDINGS, IS_PLAIN_BROWSER } = await loadRegistry();

    expect(IS_PLAIN_BROWSER).toBe(false);
    expect(DEFAULT_BINDINGS.splitRight).toBe("mod+d");
    expect(DEFAULT_BINDINGS.splitDown).toBe("mod+shift+d");
  });

  it("blocked terminal keys follow the bindings in effect, including overrides", async () => {
    vi.stubGlobal("navigator", WIN_NAVIGATOR);
    const { appAltKeyCodes, labelWithCombo } = await loadRegistry();

    // Defaults: every Ctrl+Alt letter an action claims is blocked from reaching xterm.
    expect([...appAltKeyCodes({})].sort()).toEqual(
      ["KeyB", "KeyD", "KeyE", "KeyF", "KeyG", "KeyO", "KeyT", "KeyW"],
    );
    // A rebound action moves the blocked key with it; the old letter goes back to the terminal.
    const overridden = appAltKeyCodes({ splitRight: "mod+alt+k" });
    expect(overridden.has("KeyK")).toBe(true);
    expect(overridden.has("KeyD")).toBe(false);
    // Tooltips render the effective binding rather than a hardcoded hint.
    expect(labelWithCombo("Save", "saveDoc", {})).toBe("Save (Ctrl+S)");
    expect(labelWithCombo("Split right", "splitRight", { splitRight: "mod+alt+k" })).toBe(
      "Split right (Ctrl+Alt+K)",
    );
  });

  it("macOS desktop shell claims no Ctrl+Alt keys", async () => {
    vi.stubGlobal("navigator", MAC_NAVIGATOR);
    const { appAltKeyCodes } = await loadRegistry();

    // Cmd bindings leave Ctrl+Alt entirely to the terminal.
    expect(appAltKeyCodes({}).size).toBe(0);
  });

  it("non-macOS shells always use Ctrl+Alt bindings", async () => {
    vi.stubGlobal("navigator", WIN_NAVIGATOR);
    envState.isBrowser = false;
    envState.isRemoteWindow = false;
    const { DEFAULT_BINDINGS, IS_PLAIN_BROWSER, formatCombo } = await loadRegistry();

    expect(IS_PLAIN_BROWSER).toBe(false);
    expect(DEFAULT_BINDINGS.splitRight).toBe("mod+alt+d");
    expect(DEFAULT_BINDINGS.splitDown).toBe("mod+alt+e");
    expect(formatCombo("mod+alt+d")).toBe("Ctrl+Alt+D");
  });

  it("macOS desktop treats only Cmd as mod, leaving Ctrl+letter to the shell", async () => {
    vi.stubGlobal("navigator", MAC_NAVIGATOR);
    const { matchCombo, comboFromEvent } = await loadRegistry();

    // Ctrl+D is EOF and Ctrl+W deletes a word; neither may trigger the Cmd bindings.
    expect(matchCombo(keyEvent("d", { ctrl: true }), "mod+d")).toBe(false);
    expect(matchCombo(keyEvent("w", { ctrl: true }), "mod+w")).toBe(false);
    expect(comboFromEvent(keyEvent("d", { ctrl: true }))).toBeNull();
    // Cmd still works, and Cmd+Ctrl is not the binding either.
    expect(matchCombo(keyEvent("d", { meta: true }), "mod+d")).toBe(true);
    expect(matchCombo(keyEvent("d", { meta: true, ctrl: true }), "mod+d")).toBe(false);
    expect(comboFromEvent(keyEvent("d", { meta: true }))).toBe("mod+d");
  });

  it("non-macOS shells treat only Ctrl as mod", async () => {
    vi.stubGlobal("navigator", WIN_NAVIGATOR);
    const { matchCombo } = await loadRegistry();

    expect(matchCombo(keyEvent("d", { ctrl: true, alt: true }), "mod+alt+d")).toBe(true);
    expect(matchCombo(keyEvent("d", { meta: true, alt: true }), "mod+alt+d")).toBe(false);
    // Bare Ctrl+D matches nothing because the defaults all carry Alt.
    expect(matchCombo(keyEvent("d", { ctrl: true }), "mod+alt+d")).toBe(false);
  });
});
