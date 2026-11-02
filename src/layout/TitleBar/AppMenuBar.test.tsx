//! Coverage for the Alt gesture that shows the Windows/Linux menu bar: a bare Alt press toggles it,
//! an Alt+key combination leaves it alone so the terminal still receives Meta sequences, and Escape
//! closes the open dropdown before hiding the bar.

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const { storeState, splitNewMock, newScratchTabMock, focusTerminalMock } =
  vi.hoisted(() => ({
    storeState: {
      shortcutOverrides: {} as Record<string, string>,
      activeSessionId: "s1",
      scopeTabsToProject: false,
      splitNew: vi.fn(),
      newScratchTab: vi.fn(),
      setSettingsOpen: vi.fn(),
      setShareOpen: vi.fn(),
    },
    splitNewMock: vi.fn(),
    newScratchTabMock: vi.fn(),
    focusTerminalMock: vi.fn(),
  }));

vi.mock("../../i18n", () => ({
  // Echo keys so assertions stay locale-independent.
  useT: () => (key: string) => key,
}));
// Windows/Linux is the platform under test; the bar renders nothing on macOS.
vi.mock("../../platform", () => ({
  env: { isMac: false },
  platform: { opener: { openExternal: vi.fn() } },
}));
vi.mock("../../terminal/registry", () => ({ focusTerminal: focusTerminalMock }));
vi.mock("../../store/termStore", () => ({
  useTermStore: Object.assign(
    (sel: (s: unknown) => unknown) => sel(storeState),
    {
      getState: () => ({
        ...storeState,
        splitNew: splitNewMock,
        newScratchTab: newScratchTabMock,
      }),
    },
  ),
}));

import { AppMenuBar } from "./AppMenuBar";

/** A bare Alt press and release, the gesture that toggles the bar. */
function tapAlt() {
  fireEvent.keyDown(window, { key: "Alt" });
  fireEvent.keyUp(window, { key: "Alt" });
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("AppMenuBar", () => {
  it("offers a global terminal only while tabs are scoped to a project", () => {
    storeState.scopeTabsToProject = true;
    try {
      render(<AppMenuBar />);
      tapAlt();
      fireEvent.click(screen.getByText("menubar.terminal"));
      fireEvent.click(screen.getByText("tab.newGlobalTerminal"));
      expect(newScratchTabMock).toHaveBeenCalledWith({ global: true });
    } finally {
      storeState.scopeTabsToProject = false;
    }
  });

  it("hides the global terminal entry while scoping is off", () => {
    render(<AppMenuBar />);
    tapAlt();
    fireEvent.click(screen.getByText("menubar.terminal"));
    expect(screen.queryByText("tab.newGlobalTerminal")).toBeNull();
  });

  it("stays hidden until a bare Alt press, and hides again on the next one", () => {
    render(<AppMenuBar />);
    expect(screen.queryByText("menubar.help")).toBeNull();

    tapAlt();
    expect(screen.getByText("menubar.help")).toBeTruthy();

    tapAlt();
    expect(screen.queryByText("menubar.help")).toBeNull();
  });

  it("ignores Alt held with another key so Meta sequences still reach the terminal", () => {
    render(<AppMenuBar />);
    fireEvent.keyDown(window, { key: "Alt" });
    fireEvent.keyDown(window, { key: "b", altKey: true });
    fireEvent.keyUp(window, { key: "b", altKey: true });
    fireEvent.keyUp(window, { key: "Alt" });
    expect(screen.queryByText("menubar.help")).toBeNull();
  });

  it("opens a menu, runs an item, and hides itself afterwards", () => {
    render(<AppMenuBar />);
    tapAlt();
    fireEvent.click(screen.getByText("menubar.terminal"));
    fireEvent.click(screen.getByText("term.splitRight"));
    // The second argument is the split's origin, which the menu bar owns: the split log tells
    // shortcut, menu, and pane-button splits apart, and this path is the only source of "menu".
    expect(splitNewMock).toHaveBeenCalledWith("horizontal", "menu");
    expect(screen.queryByText("menubar.terminal")).toBeNull();
    // Focus goes back to the terminal the keyboard came from.
    expect(focusTerminalMock).toHaveBeenCalledWith("s1");
  });

  it("closes an open dropdown with Escape before hiding the bar", () => {
    render(<AppMenuBar />);
    tapAlt();
    fireEvent.click(screen.getByText("menubar.terminal"));
    expect(screen.getByText("term.splitDown")).toBeTruthy();

    fireEvent.keyDown(screen.getByText("menubar.terminal"), { key: "Escape" });
    expect(screen.queryByText("term.splitDown")).toBeNull();
    expect(screen.getByText("menubar.terminal")).toBeTruthy();

    fireEvent.keyDown(screen.getByText("menubar.terminal"), { key: "Escape" });
    expect(screen.queryByText("menubar.terminal")).toBeNull();
  });
});
