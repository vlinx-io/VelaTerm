import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import en from "../../i18n/locales/en";
import type { PluginSummary } from "../../ipc/plugins";
import { PluginsPanel } from "./PluginsPanel";

const state = vi.hoisted(() => ({
  plugins: [] as PluginSummary[],
  capabilities: vi.fn(),
  installArchive: vi.fn(),
  linkDev: vi.fn(),
  reload: vi.fn(),
  setEnabled: vi.fn(),
  uninstall: vi.fn(),
  refresh: vi.fn(),
}));

vi.mock("../../ipc/plugins", () => ({
  pluginCapabilities: state.capabilities,
  pluginInstallArchive: state.installArchive,
  pluginLinkDev: state.linkDev,
  pluginReload: state.reload,
  pluginSetEnabled: state.setEnabled,
  pluginUninstall: state.uninstall,
}));

vi.mock("../../plugins/host", () => ({
  usePlugins: () => state.plugins,
  refreshPlugins: state.refresh,
}));

vi.mock("../../plugins/PluginViews", () => ({
  PluginIcon: () => <svg data-testid="plugin-icon" />,
}));

vi.mock("../../platform", () => ({
  env: { isBrowser: false, isElectron: false, hasNativeHost: false },
  platform: { dialog: { pickFile: vi.fn(), pickDirectory: vi.fn() } },
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: keyof typeof en, ...args: unknown[]) => {
    const value = en[key];
    return typeof value === "function" ? Reflect.apply(value, null, args) : value;
  },
}));

const samplePlugin: PluginSummary = {
  id: "demo.plugin",
  name: "Demo Plugin",
  version: "1.0.0",
  description: "Example plugin",
  source: "installed",
  path: "/tmp/demo",
  enabled: true,
  status: "running",
  lastError: null,
  revision: 1,
  hasBackend: true,
  ui: null,
  commands: [],
  secretCommands: [],
  cli: ["vdemo"],
  ptySchemes: [],
  skills: [],
  installedAt: 0,
};

beforeEach(() => {
  vi.clearAllMocks();
  state.plugins = [];
  state.capabilities.mockResolvedValue({ editable: true });
  state.refresh.mockResolvedValue(undefined);
});

afterEach(cleanup);

it("applies shared vlx-btn classes to top action bar and manual path controls", async () => {
  render(<PluginsPanel />);

  const installBtn = await screen.findByRole("button", { name: "Install from Archive…" });
  const linkBtn = screen.getByRole("button", { name: "Link Local Plugin…" });
  const refreshBtn = screen.getByRole("button", { name: "Refresh" });

  expect(installBtn.classList.contains("vlx-btn")).toBe(true);
  expect(installBtn.classList.contains("vlx-btn-primary")).toBe(true);
  expect(linkBtn.classList.contains("vlx-btn")).toBe(true);
  expect(refreshBtn.classList.contains("vlx-btn")).toBe(true);

  fireEvent.click(installBtn);

  const input = screen.getByPlaceholderText("/path/to/plugin.vlxplugin");
  const confirmBtn = screen.getByRole("button", { name: en["common.confirm"] });
  const cancelBtn = screen.getByRole("button", { name: en["common.cancel"] });

  expect(input.classList.contains("vlx-input")).toBe(true);
  expect(confirmBtn.classList.contains("vlx-btn")).toBe(true);
  expect(confirmBtn.classList.contains("vlx-btn-primary")).toBe(true);
  expect(cancelBtn.classList.contains("vlx-btn")).toBe(true);
});

it("applies shared vlx-btn classes to plugin card and uninstall confirmation buttons", async () => {
  state.plugins = [samplePlugin];
  render(<PluginsPanel />);

  const disableBtn = await screen.findByRole("button", { name: "Disable" });
  const reloadBtn = screen.getByRole("button", { name: "Reload" });
  const uninstallBtn = screen.getByRole("button", { name: "Uninstall" });

  expect(disableBtn.classList.contains("vlx-btn")).toBe(true);
  expect(reloadBtn.classList.contains("vlx-btn")).toBe(true);
  expect(uninstallBtn.classList.contains("vlx-btn")).toBe(true);
  expect(uninstallBtn.classList.contains("plugins-panel-btn-danger")).toBe(true);

  fireEvent.click(uninstallBtn);

  const uninstallButtons = screen.getAllByRole("button", { name: "Uninstall" });
  const confirmUninstallBtn = uninstallButtons[uninstallButtons.length - 1];
  const cancelBtn = screen.getByRole("button", { name: "Cancel" });

  expect(confirmUninstallBtn.classList.contains("vlx-btn")).toBe(true);
  expect(confirmUninstallBtn.classList.contains("plugins-panel-btn-danger-solid")).toBe(true);
  expect(cancelBtn.classList.contains("vlx-btn")).toBe(true);
});
