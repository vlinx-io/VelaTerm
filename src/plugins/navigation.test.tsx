// @vitest-environment jsdom
//! Regression coverage for plugin URL clearing on Tauri's empty-pathname origin, tab switching,
//! Memory <-> Plugin overlay transitions, and scoped plugin ErrorBoundary isolation.

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setLang } from "../i18n";
import { memoryNavigate, memoryUrl } from "../layout/Memory/navigation";
import { useMemoryTab } from "../layout/Memory/useMemoryTab";
import { useTermStore } from "../store/termStore";
import {
  clearPluginSearchParams,
  pluginNavigate,
  pluginUrl,
  readPluginRoute,
  usePluginTab,
} from "./navigation";

const mockPluginSummary = {
  id: "colab",
  name: "Google Colab GPU",
  version: "0.1.0",
  description: "",
  source: "dev" as const,
  path: "/tmp/colab",
  enabled: true,
  status: "running" as const,
  lastError: null,
  revision: 1,
  hasBackend: true,
  ui: {
    entry: "ui/entry.js",
    style: "ui/style.css",
    localesDir: "locales",
    rail: { labelKey: "colab.title", defaultRoute: "overview", separatorBefore: true },
    centerPage: { hidesRightPanel: true, tabTitleKey: "colab.title" },
    diagnosticRedactedCommands: [],
  },
  commands: [],
  secretCommands: [],
  ptySchemes: [],
  cli: [],
  skills: [],
  installedAt: 0,
};

let mockCenterPage: (() => React.JSX.Element) | null = () => (
  <div data-testid="colab-workbench">Colab Ready</div>
);

vi.mock("./host", () => ({
  usePlugins: () => [mockPluginSummary],
  useLoadedPlugin: (id: string | null) =>
    id === "colab"
      ? {
          pluginId: "colab",
          revision: 1,
          registration: { CenterPage: mockCenterPage },
          locales: {},
          loading: false,
          loadError: null,
          blobUrl: null,
          styleEl: null,
        }
      : null,
  translatePluginKey: (_id: string, key: string) => key,
}));

vi.mock("../ipc/plugins", () => ({
  pluginReload: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../debug", () => ({ DEBUG: false, dlog: vi.fn(), checkTabInvariants: vi.fn() }));

import { PluginCenterRoute, PluginTab } from "./PluginViews";

function TabHost() {
  useMemoryTab();
  usePluginTab();
  return (
    <>
      <PluginTab otherTabCount={2} onCloseOthers={() => {}} />
      <PluginCenterRoute />
    </>
  );
}

describe("plugin navigation URLs", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    window.history.replaceState(null, "", "/");
  });

  it.each(["tauri://localhost", "tauri://localhost/", "http://localhost:23147/"])(
    "produces a full closing URL for %s so history.replaceState never receives an empty string",
    (base) => {
      vi.stubGlobal("window", {
        location: { href: `${base}?plugin=colab&pluginRoute=overview` },
      });
      expect(pluginUrl(null)).toBe(base);
    },
  );

  it("clears ?plugin and ?pluginRoute with a full URL on tauri://localhost", () => {
    const replaceState = vi.fn();
    const dispatchEvent = vi.fn();
    vi.stubGlobal("window", {
      location: { href: "tauri://localhost?plugin=colab&pluginRoute=overview" },
      history: { replaceState },
      dispatchEvent,
    });

    clearPluginSearchParams("colab");
    expect(replaceState).toHaveBeenCalledWith(null, "", "tauri://localhost");
    expect(dispatchEvent).toHaveBeenCalled();
  });
});

describe("plugin tab switching and isolation", () => {
  beforeEach(() => {
    setLang("en");
    mockCenterPage = () => <div data-testid="colab-workbench">Colab Ready</div>;
    window.history.replaceState(null, "", "/");
    useTermStore.setState({
      openTabs: ["tab-a", "tab-b"],
      activeTabId: "tab-a",
      activeSessionId: "tab-a",
      focusedPaneId: null,
      selection: [{ id: "tab-a", kind: "session" }],
    });
  });

  afterEach(() => {
    cleanup();
    window.history.replaceState(null, "", "/");
  });

  it("switches to another tab, re-activates the pinned plugin tab, and restores the prior tab on close", () => {
    render(<TabHost />);

    act(() => {
      pluginNavigate(pluginUrl("colab", "overview"));
    });
    expect(useTermStore.getState().activeTabId).toBeNull();
    expect(screen.getByTestId("colab-workbench")).toBeTruthy();
    expect(document.querySelector(".plugin-center-surface")).not.toBeNull();
    expect(document.querySelector(".tab.on")).not.toBeNull();

    // Selecting another tab clears plugin query params and keeps the chosen tab active.
    act(() => {
      useTermStore.getState().setActiveTab("tab-b");
    });
    expect(readPluginRoute(window.location.search)).toBeNull();
    expect(useTermStore.getState().activeTabId).toBe("tab-b");
    expect(screen.queryByTestId("colab-workbench")).toBeNull();
    expect(document.querySelector(".tab.on")).toBeNull();
    expect(document.querySelector(".tab")).not.toBeNull();

    // Clicking the inactive plugin tab reopens it; closing it restores tab-b.
    act(() => {
      fireEvent.click(document.querySelector(".tab")!);
    });
    expect(readPluginRoute(window.location.search)).toEqual({
      pluginId: "colab",
      route: "overview",
    });
    expect(useTermStore.getState().activeTabId).toBeNull();

    act(() => {
      fireEvent.click(screen.getByRole("button", { name: "Close" }));
    });
    expect(readPluginRoute(window.location.search)).toBeNull();
    expect(useTermStore.getState().activeTabId).toBe("tab-b");
    expect(document.querySelector(".tab")).toBeNull();
  });

  it("switches cleanly between Memory and Plugin overlays without bouncing back to a session tab", () => {
    render(<TabHost />);

    act(() => {
      memoryNavigate(memoryUrl("notebooks"));
    });
    expect(useTermStore.getState().activeTabId).toBeNull();
    expect(new URLSearchParams(window.location.search).get("memory")).toBe("notebooks");

    // Memory -> Plugin: Memory clears without restoring tab-a.
    act(() => {
      pluginNavigate(pluginUrl("colab", "overview"));
    });
    expect(new URLSearchParams(window.location.search).get("memory")).toBeNull();
    expect(readPluginRoute(window.location.search)?.pluginId).toBe("colab");
    expect(useTermStore.getState().activeTabId).toBeNull();
    expect(screen.getByTestId("colab-workbench")).toBeTruthy();

    // Plugin -> Memory: Plugin clears without restoring tab-a.
    act(() => {
      memoryNavigate(memoryUrl("collections"));
    });
    expect(readPluginRoute(window.location.search)).toBeNull();
    expect(new URLSearchParams(window.location.search).get("memory")).toBe("collections");
    expect(useTermStore.getState().activeTabId).toBeNull();
    expect(screen.queryByTestId("colab-workbench")).toBeNull();
  });

  it("contains a plugin render crash inside the plugin surface instead of covering the whole window", () => {
    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    mockCenterPage = () => {
      throw new Error("boom from plugin");
    };
    try {
      render(<TabHost />);
      act(() => {
        pluginNavigate(pluginUrl("colab", "overview"));
      });
      const surface = document.querySelector(".plugin-center-surface");
      expect(surface).not.toBeNull();
      expect(screen.getByText("boom from plugin")).toBeTruthy();
      expect(surface!.contains(screen.getByText("boom from plugin"))).toBe(true);
      expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    } finally {
      errSpy.mockRestore();
    }
  });
});
