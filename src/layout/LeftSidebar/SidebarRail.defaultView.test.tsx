//! The activity bar's right-click menu chooses the view a launch opens on, and clears that choice again.

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../ipc/transport")>(),
  invoke: vi.fn().mockResolvedValue({ linked: false }),
  listen: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("../../ipc/commands", async (original) => ({
  ...await original<typeof import("../../ipc/commands")>(),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../../ipc/settingsSync", () => ({ pushSetting: vi.fn() }));

import { setLang } from "../../i18n";
import { loadSettings, SETTINGS_KEY } from "../../store/settings";
import { useTermStore } from "../../store/termStore";
import { SidebarRail } from "./SidebarRail";
import { SYSTEM_CHAT, SYSTEM_PROJECTS, SYSTEM_TASKS } from "./systemCollections";

const savedDefault = () => {
  const raw = localStorage.getItem(SETTINGS_KEY);
  return raw ? (JSON.parse(raw) as { defaultSidebarSection?: unknown }).defaultSidebarSection : undefined;
};

beforeEach(() => {
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  localStorage.removeItem(SETTINGS_KEY);
  setLang("en");
  useTermStore.setState({
    ...loadSettings(),
    // The activity bar draws one button per section only when the backend provides the fixed collections.
    projects: [
      { id: SYSTEM_PROJECTS, name: "Projects", rootPath: "", collapsed: false, collectionId: null },
      { id: SYSTEM_TASKS, name: "Tasks", rootPath: "", collapsed: false, collectionId: null },
      { id: SYSTEM_CHAT, name: "Chat", rootPath: "", collapsed: false, collectionId: null },
    ] as never,
  });
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  localStorage.removeItem(SETTINGS_KEY);
});

describe("activity bar default view", () => {
  it("sets and clears the section a launch opens on", async () => {
    await act(async () => render(<SidebarRail />));
    expect(useTermStore.getState().defaultSidebarSection).toBeNull();

    fireEvent.contextMenu(screen.getByTitle("Tasks"));
    await act(async () => fireEvent.click(screen.getByText("Set as default view")));
    expect(useTermStore.getState().defaultSidebarSection).toBe("tasks");
    expect(savedDefault()).toBe("tasks");
    // Choosing a default view does not move the sidebar; it only decides where the next launch starts.
    expect(screen.queryByText("Set as default view")).toBeNull();

    fireEvent.contextMenu(screen.getByTitle("Tasks"));
    await act(async () => fireEvent.click(screen.getByText("Clear default view")));
    expect(useTermStore.getState().defaultSidebarSection).toBeNull();
    expect(savedDefault()).toBeNull();
  });

  it("offers no default for sections that open their own page", async () => {
    await act(async () => render(<SidebarRail />));
    fireEvent.contextMenu(screen.getByTitle("Knowledge Base"));
    expect(screen.queryByText("Set as default view")).toBeNull();
    expect(useTermStore.getState().defaultSidebarSection).toBeNull();
  });
});
