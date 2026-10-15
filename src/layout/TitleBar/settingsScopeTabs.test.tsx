import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../ipc/transport")>(),
  invoke: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("../../ipc/commands", async (original) => ({
  ...await original<typeof import("../../ipc/commands")>(),
  spawnSkillsInstalled: vi.fn().mockResolvedValue(false),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../../ipc/settingsSync", () => ({ pushSetting: vi.fn() }));

import { setLang } from "../../i18n";
import { loadSettings, SETTINGS_KEY } from "../../store/settings";
import { useTermStore } from "../../store/termStore";
import { SettingsModal } from "./SettingsModal";

beforeEach(() => {
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  localStorage.removeItem(SETTINGS_KEY);
  useTermStore.setState({ ...loadSettings() });
  setLang("en");
  vi.clearAllMocks();
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

it("scopes tabs to the project only after the Behavior toggle is switched on", async () => {
  await act(async () => render(<SettingsModal onClose={() => {}} />));
  fireEvent.click(screen.getByRole("button", { name: "Behavior" }));
  const field = within(screen.getByText("Show only the active project's tabs").parentElement!);
  expect(useTermStore.getState().scopeTabsToProject).toBe(false);
  expect(screen.getByText(/Hidden tabs keep running/)).toBeTruthy();

  fireEvent.click(field.getByRole("button", { name: "On" }));
  expect(useTermStore.getState().scopeTabsToProject).toBe(true);
  expect(loadSettings().scopeTabsToProject).toBe(true);
});
