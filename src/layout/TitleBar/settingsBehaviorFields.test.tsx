//! Regression coverage for image-paste setting visibility and availability: every window sees the setting, but
//! only the desktop shell running on the same machine as the agent can select native image paste.

import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const { env, setMode, setDynamicStatusFilter, latency, setInputLatencyLog, setInputLatencyThresholdMs } = vi.hoisted(() => ({
  env: {
    isTauri: false,
    isElectron: false,
  },
  setMode: vi.fn(),
  setDynamicStatusFilter: vi.fn(),
  latency: { on: false },
  setInputLatencyLog: vi.fn(),
  setInputLatencyThresholdMs: vi.fn(),
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: string) => key,
  getLangChoice: () => "auto",
  LOCALE_NAMES: {},
  LOCALES: [],
  setLang: vi.fn(),
}));
vi.mock("../../platform", () => ({ env }));
vi.mock("../../store/termStore", () => ({
  useTermStore: (selector: (state: object) => unknown) =>
    selector({
      accent: "auto",
      density: "regular",
      paneStyle: "flush",
      dividerStyle: "subtle",
      navLayout: "tree",
      settingsTab: "appearance",
      singleTabMode: true,
      spawnConfirm: true,
      referSummary: { enabled: false, agent: "claude", model: "", effort: "" },
      setReferSummary: vi.fn(),
      termRenderer: "dom",
      redrawOnReveal: false,
      outputScheduler: true,
      inputLatencyLog: latency.on,
      inputLatencyThresholdMs: 50,
      dynamicStatusFilter: true,
      recordSessions: false,
      maxLiveTabs: 32,
      usageRefreshSec: 300,
      soundEnabled: true,
      notifyEnabled: true,
      cleanPastedImages: true,
      uiFontFamily: null,
      uiFontSize: null,
      termFontFamily: null,
      termFontSize: 13,
      imagePasteMode: "upload",
      setImagePasteMode: setMode,
      setAccent: vi.fn(),
      setDensity: vi.fn(),
      setPaneStyle: vi.fn(),
      setDividerStyle: vi.fn(),
      setNavLayout: vi.fn(),
      setSettingsTab: vi.fn(),
      setSingleTabMode: vi.fn(),
      setSpawnConfirm: vi.fn(),
      setTermRenderer: vi.fn(),
      setRedrawOnReveal: vi.fn(),
      setOutputScheduler: vi.fn(),
      setInputLatencyLog,
      setInputLatencyThresholdMs,
      setDynamicStatusFilter,
      setRecordSessions: vi.fn(),
      setMaxLiveTabs: vi.fn(),
      setUsageRefreshSec: vi.fn(),
      toggleSound: vi.fn(),
      setCleanPastedImages: vi.fn(),
      toggleNotify: vi.fn(),
      setUiFontFamily: vi.fn(),
      setUiFontSize: vi.fn(),
      setTermFontFamily: vi.fn(),
      setTermFontSize: vi.fn(),
    }),
}));
vi.mock("../../ipc/commands", () => ({
  cleanPastedImages: vi.fn(),
  spawnSkillsInstalled: vi.fn().mockResolvedValue(false),
  installSpawnSkills: vi.fn(),
  listShells: vi.fn().mockResolvedValue([]),
  giteaGetStatus: vi.fn(),
  giteaProbe: vi.fn(),
  giteaSetConfig: vi.fn(),
}));
vi.mock("../../ipc/settingsSync", () => ({ pushSetting: vi.fn() }));
vi.mock("../../ipc/transport", () => ({ isTauri: false, isRemoteWindow: false, listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("../../notify", () => ({
  getEffectiveNotifyPermission: vi.fn().mockResolvedValue("unsupported"),
  requestEffectiveNotifyPermission: vi.fn().mockResolvedValue("unsupported"),
}));

import { ImagePasteModeField } from "./settingsBehaviorFields";
import { SettingsModal } from "./SettingsModal";
import { loadSettings, SETTINGS_KEY } from "../../store/settings";

afterEach(() => {
  cleanup();
  env.isTauri = false;
  env.isElectron = false;
  setMode.mockReset();
  setDynamicStatusFilter.mockReset();
  setInputLatencyLog.mockReset();
  setInputLatencyThresholdMs.mockReset();
  latency.on = false;
});

describe("image paste settings", () => {
  it("a browser or remote window still shows both modes, with the native one disabled", () => {
    render(<ImagePasteModeField />);
    expect(screen.getByText("settings.imagePasteMode")).toBeTruthy();
    expect(screen.getByText("settings.imagePasteUpload")).toBeTruthy();
    expect(
      (screen.getByRole("button", { name: "settings.imagePasteAgent" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(screen.getByText("settings.imagePasteRemoteHint")).toBeTruthy();
  });

  it("a local Tauri window can select native image paste", () => {
    env.isTauri = true;
    render(<ImagePasteModeField />);
    expect(
      (screen.getByRole("button", { name: "settings.imagePasteAgent" }) as HTMLButtonElement)
        .disabled,
    ).toBe(false);
    expect(screen.getByText("settings.imagePasteHint")).toBeTruthy();
  });

  it("the Terminal category of the full settings dialog always contains the image paste entry", async () => {
    await act(async () => render(<SettingsModal onClose={vi.fn()} />));
    fireEvent.click(screen.getByRole("link", { name: "settings.catTerminal" }));
    expect(screen.getByText("settings.imagePasteMode")).toBeTruthy();
    expect(screen.getByText("settings.imagePasteUpload")).toBeTruthy();
    expect(screen.getByText("settings.imagePasteAgent")).toBeTruthy();
  });

  it("dynamic additions to the status filter are on by default and can be turned off in the behaviour settings", async () => {
    const previous = localStorage.getItem(SETTINGS_KEY);
    localStorage.removeItem(SETTINGS_KEY);
    expect(loadSettings().dynamicStatusFilter).toBe(true);
    if (previous === null) localStorage.removeItem(SETTINGS_KEY);
    else localStorage.setItem(SETTINGS_KEY, previous);

    await act(async () => render(<SettingsModal onClose={vi.fn()} />));
    fireEvent.click(screen.getByRole("link", { name: "settings.catBehavior" }));
    const label = screen.getByText("settings.dynamicStatusFilter");
    const field = label.parentElement;
    expect(field).toBeTruthy();
    fireEvent.click(within(field as HTMLElement).getByRole("button", { name: "common.off" }));
    expect(setDynamicStatusFilter).toHaveBeenCalledWith(false);
  });
});

describe("input latency log settings", () => {
  function withStoredSettings(value: unknown, run: () => void) {
    const previous = localStorage.getItem(SETTINGS_KEY);
    if (value === undefined) localStorage.removeItem(SETTINGS_KEY);
    else localStorage.setItem(SETTINGS_KEY, JSON.stringify(value));
    try { run(); } finally {
      if (previous === null) localStorage.removeItem(SETTINGS_KEY);
      else localStorage.setItem(SETTINGS_KEY, previous);
    }
  }

  it("is off by default with a 50 ms threshold, and unknown thresholds fall back to 50 ms", () => {
    withStoredSettings(undefined, () => {
      expect(loadSettings().inputLatencyLog).toBe(false);
      expect(loadSettings().inputLatencyThresholdMs).toBe(50);
    });
    withStoredSettings({ inputLatencyLog: "yes", inputLatencyThresholdMs: 75 }, () => {
      expect(loadSettings().inputLatencyLog).toBe(false);
      expect(loadSettings().inputLatencyThresholdMs).toBe(50);
    });
    withStoredSettings({ inputLatencyLog: true, inputLatencyThresholdMs: 200 }, () => {
      expect(loadSettings().inputLatencyLog).toBe(true);
      expect(loadSettings().inputLatencyThresholdMs).toBe(200);
    });
  });

  it("the Advanced category hides the threshold until logging is turned on", () => {
    render(<SettingsModal onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole("link", { name: "settings.catAdvanced" }));
    const field = screen.getByText("settings.inputLatencyLog").parentElement as HTMLElement;
    expect(screen.queryByText("settings.inputLatencyThreshold")).toBeNull();
    fireEvent.click(within(field).getByRole("button", { name: "common.on" }));
    expect(setInputLatencyLog).toHaveBeenCalledWith(true);
  });

  it("the threshold offers fixed choices once logging is on", () => {
    latency.on = true;
    render(<SettingsModal onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole("link", { name: "settings.catAdvanced" }));
    const field = screen.getByText("settings.inputLatencyThreshold").parentElement as HTMLElement;
    fireEvent.click(within(field).getByRole("button", { name: "200 ms" }));
    expect(setInputLatencyThresholdMs).toHaveBeenCalledWith(200);
  });
});
