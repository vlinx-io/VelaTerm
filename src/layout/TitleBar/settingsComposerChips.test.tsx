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
import { pushSetting } from "../../ipc/settingsSync";
import { COMPOSER_CHIP_IDS, loadSettings, SETTINGS_KEY } from "../../store/settings";
import { useTermStore } from "../../store/termStore";
import { SettingsModal } from "./SettingsModal";

beforeEach(() => {
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  localStorage.removeItem(SETTINGS_KEY);
  useTermStore.setState(loadSettings());
  setLang("en");
  vi.clearAllMocks();
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

const row = (name: string) => within(screen.getByText(name).parentElement!);

it("lists every composer chip with a switch and arrows, persisting each change", async () => {
  await act(async () => render(<SettingsModal onClose={() => {}} />));
  fireEvent.click(screen.getByRole("button", { name: "Conversation view" }));
  expect(screen.getByText("Composer toolbar")).toBeTruthy();
  for (const name of ["Model", "Thinking effort", "Collaboration mode", "Permission mode", "Fast mode", "Speed", "Tone", "MCP servers", "Background tasks", "Account", "Codex reset credits"]) {
    expect(row(name).getByRole("button", { name: "Off" })).toBeTruthy();
  }
  expect(screen.getAllByRole("button", { name: /^Move .* up$/ })).toHaveLength(COMPOSER_CHIP_IDS.length);

  fireEvent.click(row("Permission mode").getByRole("button", { name: "Off" }));
  expect(useTermStore.getState().composerInlineChips).toEqual(["model", "effort", "collaboration"]);
  expect(pushSetting).toHaveBeenLastCalledWith(SETTINGS_KEY, expect.stringContaining('"composerInlineChips":["model","effort","collaboration"]'));
  expect(loadSettings().composerInlineChips).toEqual(["model", "effort", "collaboration"]);

  fireEvent.click(screen.getByRole("button", { name: "Move Thinking effort up" }));
  expect(useTermStore.getState().composerInlineChips).toEqual(["effort", "model", "collaboration"]);
  expect((screen.getByRole("button", { name: "Move Thinking effort up" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "Move Collaboration mode down" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "Move Permission mode up" }) as HTMLButtonElement).disabled).toBe(true);

  fireEvent.click(row("Account").getByRole("button", { name: "On" }));
  expect(useTermStore.getState().composerInlineChips).toEqual(["effort", "model", "collaboration", "account"]);
  fireEvent.click(screen.getByRole("button", { name: "Move Account up" }));
  expect(useTermStore.getState().composerInlineChips).toEqual(["effort", "model", "account", "collaboration"]);
  // Enabled rows come first in toolbar order, then the rest in their fixed order.
  const labels = [...document.querySelectorAll("button[aria-label^='Move '][aria-label$=' up']")].map((el) => el.getAttribute("aria-label"));
  expect(labels.slice(0, 4)).toEqual(["Move Thinking effort up", "Move Model up", "Move Account up", "Move Collaboration mode up"]);
});
