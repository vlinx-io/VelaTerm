// @vitest-environment jsdom
//! Regression coverage for TitleBar hook order when switching between routes that show and hide the right panel.

import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setLang } from "../../i18n";
import { memoryNavigate, memoryUrl } from "../Memory/navigation";
import { useTermStore } from "../../store/termStore";

vi.mock("../../ipc/commands", () => ({
  getBackendVersion: vi.fn().mockResolvedValue(__APP_VERSION__),
}));
vi.mock("../../ipc/webServer", () => ({
  webServerStatus: vi.fn().mockResolvedValue({ running: false, port: null }),
}));
vi.mock("../../ipc/screenshot", () => ({
  screenshotShortcutGet: vi.fn().mockResolvedValue(null),
  screenshotStart: vi.fn(),
}));
vi.mock("../../plugins/host", () => ({
  pluginRouteHidesRightPanel: () => false,
}));
vi.mock("./AppMenuBar", () => ({ AppMenuBar: () => null }));

import { TitleBar } from "./TitleBar";

describe("TitleBar route hook stability", () => {
  beforeEach(() => {
    setLang("en");
    window.history.replaceState(null, "", "/");
    Object.defineProperty(window, "matchMedia", {
      writable: true,
      configurable: true,
      value: vi.fn().mockReturnValue({
        matches: true,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      }),
    });
    useTermStore.setState({ rightCollapsed: false });
  });

  afterEach(() => {
    cleanup();
    window.history.replaceState(null, "", "/");
  });

  it("keeps hook order stable and toggles the right-panel button across knowledge routes", () => {
    render(<TitleBar />);
    const rightToggle = () => screen.getByTitle("Hide info panel") as HTMLButtonElement;
    expect(rightToggle().disabled).toBe(false);

    act(() => {
      memoryNavigate(memoryUrl("notebooks"));
    });
    expect(rightToggle().disabled).toBe(true);

    act(() => {
      memoryNavigate(memoryUrl("entry/e1"));
    });
    expect(rightToggle().disabled).toBe(false);

    act(() => {
      memoryNavigate(memoryUrl("collections"));
    });
    expect(rightToggle().disabled).toBe(true);

    act(() => {
      memoryNavigate(memoryUrl(""));
    });
    expect(rightToggle().disabled).toBe(false);
  });
});
