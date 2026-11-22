//! Typed IPC wrappers for VelaTerm's out-of-process dynamic plugin host.

import { invoke, listen, type UnlistenFn } from "./transport";

export type PluginSource = "installed" | "dev";
export type PluginRuntimeStatus = "disabled" | "starting" | "running" | "stopped" | "error";

export interface PluginUiRailManifest {
  labelKey: string;
  defaultRoute: string;
  iconSvg?: string | null;
  separatorBefore: boolean;
}

export interface PluginUiCenterPageManifest {
  hidesRightPanel: boolean;
  tabTitleKey: string;
}

export interface PluginUiManifest {
  entry: string;
  style?: string | null;
  localesDir?: string | null;
  rail?: PluginUiRailManifest | null;
  centerPage?: PluginUiCenterPageManifest | null;
  diagnosticRedactedCommands: string[];
}

export interface PluginSummary {
  id: string;
  name: string;
  version: string;
  description: string;
  source: PluginSource;
  path: string;
  enabled: boolean;
  status: PluginRuntimeStatus;
  lastError: string | null;
  revision: number;
  hasBackend: boolean;
  ui: PluginUiManifest | null;
  commands: string[];
  secretCommands: string[];
  ptySchemes: string[];
  cli: string[];
  skills: string[];
  installedAt: number;
}

export interface PluginAsset {
  content: string;
  mimeType: string;
  revision: number;
}

export interface PluginEventEnvelope {
  pluginId: string;
  event: string;
  payload: unknown;
}

export const pluginList = (): Promise<PluginSummary[]> => invoke("plugin_list");

export const pluginCapabilities = (): Promise<{ editable: boolean }> =>
  invoke("plugin_capabilities");

export const pluginReadAsset = (pluginId: string, path: string): Promise<PluginAsset> =>
  invoke("plugin_read_asset", { pluginId, path });

export const pluginInvoke = <T = unknown>(
  pluginId: string,
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> => invoke("plugin_invoke", { pluginId, command, args });

export const pluginInstallArchive = (path: string): Promise<PluginSummary> =>
  invoke("plugin_install_archive", { path });

export const pluginLinkDev = (path: string): Promise<PluginSummary> =>
  invoke("plugin_link_dev", { path });

export const pluginSetEnabled = (pluginId: string, enabled: boolean): Promise<PluginSummary> =>
  invoke("plugin_set_enabled", { pluginId, enabled });

export const pluginReload = (pluginId: string): Promise<PluginSummary> =>
  invoke("plugin_reload", { pluginId });

export const pluginUninstall = (pluginId: string, removeData = false): Promise<{ ok: boolean }> =>
  invoke("plugin_uninstall", { pluginId, removeData });

export const listenPluginsChanged = (cb: () => void): Promise<UnlistenFn> =>
  listen("plugins://changed", () => cb());

export const listenPluginEvent = (cb: (ev: PluginEventEnvelope) => void): Promise<UnlistenFn> =>
  listen<PluginEventEnvelope>("plugin://event", cb);
