//! Runtime frontend plugin loader, reactive registry, i18n dictionary bridge, and event dispatcher.

import { useSyncExternalStore } from "react";
import { getLocale, useT, type Locale } from "../i18n";
import {
  listenPluginEvent,
  listenPluginsChanged,
  pluginInvoke,
  pluginList,
  pluginReadAsset,
  type PluginEventEnvelope,
  type PluginSummary,
} from "../ipc/plugins";
import { registerDiagnosticCommands } from "../ipc/diagnosticSafety";
import { platform } from "../platform";
import { useTermStore } from "../store/termStore";
import {
  clearPluginSearchParams,
  pluginNavigate,
  pluginUrl,
  readPluginRoute,
  usePluginLocation,
} from "./navigation";
import type {
  PluginTranslateFn,
  VlxPluginHost,
  VlxPluginModule,
  VlxPluginRegistration,
} from "./sdk";
import { getPluginUiKit, initPluginSharedGlobals } from "./shared";

export interface LoadedPluginRuntime {
  pluginId: string;
  revision: number;
  registration: VlxPluginRegistration | null;
  locales: Partial<Record<Locale, Record<string, string>>>;
  loading: boolean;
  loadError: string | null;
  blobUrl: string | null;
  styleEl: HTMLStyleElement | null;
}

interface HostSnapshot {
  plugins: PluginSummary[];
  loaded: Record<string, LoadedPluginRuntime>;
  version: number;
}

let plugins: PluginSummary[] = [];
let loaded: Record<string, LoadedPluginRuntime> = {};
let version = 0;
let snapshot: HostSnapshot = { plugins, loaded, version };
const listeners = new Set<() => void>();
const eventSubs = new Map<string, Set<(payload: unknown) => void>>();
const localeFetchInFlight = new Set<string>();
let started = false;

function emitStore(): void {
  version += 1;
  snapshot = { plugins, loaded, version };
  for (const cb of listeners) cb();
}

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

function getSnapshot(): HostSnapshot {
  return snapshot;
}

function eventKey(pluginId: string, eventName: string): string {
  return `${pluginId}\0${eventName}`;
}

function dispatchPluginEvent(env: PluginEventEnvelope): void {
  const subs = eventSubs.get(eventKey(env.pluginId, env.event));
  if (!subs) return;
  for (const cb of subs) {
    try {
      cb(env.payload);
    } catch (err) {
      console.error(`[plugin:${env.pluginId}] event handler failed:`, err);
    }
  }
}

/** Interpolate `{0}`, `{1}`, ... placeholders inside plugin JSON translation strings. */
function formatTemplate(template: string, args: Array<string | number>): string {
  if (args.length === 0) return template;
  return template.replace(/\{(\d+)\}/g, (match, idxStr) => {
    const idx = Number(idxStr);
    return idx >= 0 && idx < args.length ? String(args[idx]) : match;
  });
}

async function ensurePluginLocale(summary: PluginSummary, locale: Locale): Promise<void> {
  const localesDir = summary.ui?.localesDir;
  if (!localesDir) return;
  const rt = loaded[summary.id];
  if (!rt || rt.locales[locale]) return;
  const inflightKey = `${summary.id}:${summary.revision}:${locale}`;
  if (localeFetchInFlight.has(inflightKey)) return;
  localeFetchInFlight.add(inflightKey);
  try {
    const asset = await pluginReadAsset(summary.id, `${localesDir}/${locale}.json`);
    const dict = JSON.parse(asset.content) as Record<string, string>;
    const current = loaded[summary.id];
    if (current && current.revision === summary.revision) {
      loaded = {
        ...loaded,
        [summary.id]: {
          ...current,
          locales: { ...current.locales, [locale]: dict },
        },
      };
      emitStore();
    }
  } catch {
    // Missing optional locale falls back to English.
  } finally {
    localeFetchInFlight.delete(inflightKey);
  }
}

/** Synchronously translate a plugin key using the active interface language, falling back to English. */
export function translatePluginKey(
  pluginId: string,
  key: string,
  ...args: Array<string | number>
): string {
  const locale = getLocale();
  const rt = loaded[pluginId];
  const summary = plugins.find((p) => p.id === pluginId);
  if (summary && rt && locale !== "en" && !rt.locales[locale]) {
    void ensurePluginLocale(summary, locale);
  }
  const raw = rt?.locales[locale]?.[key] ?? rt?.locales.en?.[key] ?? key;
  return formatTemplate(raw, args);
}

function makePluginHost(pluginId: string): VlxPluginHost {
  return {
    pluginId,
    invoke: <T = unknown>(command: string, args: Record<string, unknown> = {}) =>
      pluginInvoke<T>(pluginId, command, args),
    listen: <T = unknown>(eventName: string, handler: (payload: T) => void) => {
      const key = eventKey(pluginId, eventName);
      let set = eventSubs.get(key);
      if (!set) {
        set = new Set();
        eventSubs.set(key, set);
      }
      const wrapped = handler as (payload: unknown) => void;
      set.add(wrapped);
      return () => {
        const s = eventSubs.get(key);
        if (!s) return;
        s.delete(wrapped);
        if (s.size === 0) eventSubs.delete(key);
      };
    },
    openTerminal: ({ shell, name, title }) => {
      void useTermStore.getState().newScratchTab({ shell, name: name ?? title });
    },
    openExternal: (url) => platform.opener.openExternal(url),
    navigate: (subRoute) => {
      pluginNavigate(subRoute ? pluginUrl(pluginId, subRoute) : pluginUrl(null));
    },
    useRoute: () => {
      const loc = usePluginLocation();
      const active = readPluginRoute(loc);
      return active && active.pluginId === pluginId ? active.route : null;
    },
    useT: (): PluginTranslateFn => {
      // Subscribe to both host locale changes and plugin dictionary updates.
      useT();
      useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
      return (key, ...args) => translatePluginKey(pluginId, key, ...args);
    },
    getLocale,
    ui: getPluginUiKit(),
  };
}

function unloadPlugin(pluginId: string): void {
  const prev = loaded[pluginId];
  if (!prev) return;
  try {
    prev.registration?.dispose?.();
  } catch (err) {
    console.error(`[plugin:${pluginId}] dispose failed:`, err);
  }
  if (prev.blobUrl) URL.revokeObjectURL(prev.blobUrl);
  if (prev.styleEl) prev.styleEl.remove();
  const next = { ...loaded };
  delete next[pluginId];
  loaded = next;
  clearPluginSearchParams(pluginId);
}

async function loadPluginUi(summary: PluginSummary): Promise<void> {
  const ui = summary.ui;
  if (!ui) return;

  initPluginSharedGlobals();

  if (ui.diagnosticRedactedCommands.length > 0) {
    registerDiagnosticCommands(ui.diagnosticRedactedCommands);
  }

  const prev = loaded[summary.id];
  if (prev) {
    if (prev.revision === summary.revision && (prev.registration || prev.loading)) {
      return;
    }
    unloadPlugin(summary.id);
  }

  loaded = {
    ...loaded,
    [summary.id]: {
      pluginId: summary.id,
      revision: summary.revision,
      registration: null,
      locales: {},
      loading: true,
      loadError: null,
      blobUrl: null,
      styleEl: null,
    },
  };
  emitStore();

  let blobUrl: string | null = null;
  let styleEl: HTMLStyleElement | null = null;
  try {
    const locales: Partial<Record<Locale, Record<string, string>>> = {};
    if (ui.localesDir) {
      try {
        const enAsset = await pluginReadAsset(summary.id, `${ui.localesDir}/en.json`);
        locales.en = JSON.parse(enAsset.content) as Record<string, string>;
      } catch {
        // Optional English dictionary.
      }
      const activeLocale = getLocale();
      if (activeLocale !== "en") {
        try {
          const locAsset = await pluginReadAsset(summary.id, `${ui.localesDir}/${activeLocale}.json`);
          locales[activeLocale] = JSON.parse(locAsset.content) as Record<string, string>;
        } catch {
          // Fallback to English.
        }
      }
    }

    if (ui.style && typeof document !== "undefined") {
      const cssAsset = await pluginReadAsset(summary.id, ui.style);
      styleEl = document.createElement("style");
      styleEl.dataset.vlxPlugin = summary.id;
      styleEl.dataset.vlxPluginRev = String(summary.revision);
      styleEl.textContent = cssAsset.content;
      document.head.appendChild(styleEl);
    }

    const entryAsset = await pluginReadAsset(summary.id, ui.entry);
    const blob = new Blob([entryAsset.content], { type: "text/javascript" });
    blobUrl = URL.createObjectURL(blob);
    const mod = (await import(/* @vite-ignore */ blobUrl)) as VlxPluginModule;
    if (typeof mod?.activate !== "function") {
      throw new Error("Plugin entry module must export an activate(host) function.");
    }

    // Store locales before activate so initial render reads translated strings immediately.
    loaded = {
      ...loaded,
      [summary.id]: {
        pluginId: summary.id,
        revision: summary.revision,
        registration: null,
        locales,
        loading: true,
        loadError: null,
        blobUrl,
        styleEl,
      },
    };
    const registration = await mod.activate(makePluginHost(summary.id));

    // Abort if a newer revision replaced this load while awaiting activate().
    if (loaded[summary.id]?.revision !== summary.revision) {
      registration.dispose?.();
      if (blobUrl) URL.revokeObjectURL(blobUrl);
      if (styleEl) styleEl.remove();
      return;
    }

    loaded = {
      ...loaded,
      [summary.id]: {
        pluginId: summary.id,
        revision: summary.revision,
        registration,
        locales,
        loading: false,
        loadError: null,
        blobUrl,
        styleEl,
      },
    };
    emitStore();
  } catch (err) {
    if (blobUrl) URL.revokeObjectURL(blobUrl);
    if (styleEl) styleEl.remove();
    const message = err instanceof Error ? err.message : String(err);
    loaded = {
      ...loaded,
      [summary.id]: {
        pluginId: summary.id,
        revision: summary.revision,
        registration: null,
        locales: {},
        loading: false,
        loadError: message,
        blobUrl: null,
        styleEl: null,
      },
    };
    emitStore();
  }
}

/** Visibility and runtime switch for the dynamic plugin host. */
export const PLUGINS_ENABLED = true;

export async function refreshPlugins(): Promise<void> {
  if (!PLUGINS_ENABLED) return;
  try {
    const next = await pluginList();
    plugins = next;
    const enabledIds = new Set(next.filter((p) => p.enabled && p.ui).map((p) => p.id));
    for (const id of Object.keys(loaded)) {
      if (!enabledIds.has(id)) {
        unloadPlugin(id);
      }
    }
    emitStore();
    await Promise.all(
      next.filter((p) => p.enabled && p.ui).map((p) => loadPluginUi(p)),
    );
  } catch {
    // Backend without plugin support or transient reconnect; keep current state.
  }
}

/** Boot the frontend plugin host once during application startup. */
export function initPluginHost(): void {
  if (!PLUGINS_ENABLED || started) return;
  started = true;
  initPluginSharedGlobals();
  void refreshPlugins();
  void listenPluginsChanged(() => {
    void refreshPlugins();
  });
  void listenPluginEvent((ev) => {
    dispatchPluginEvent(ev);
  });
}

export function usePlugins(): PluginSummary[] {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot).plugins;
}

export function useLoadedPlugin(pluginId: string | null): LoadedPluginRuntime | null {
  const snap = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  return pluginId ? snap.loaded[pluginId] ?? null : null;
}

export function useRailPlugins(): PluginSummary[] {
  const list = usePlugins();
  return list.filter((p) => p.enabled && Boolean(p.ui?.rail));
}

export function pluginRouteHidesRightPanel(search: string): boolean {
  const active = readPluginRoute(search);
  if (!active) return false;
  const p = plugins.find((x) => x.id === active.pluginId && x.enabled);
  return Boolean(p?.ui?.centerPage?.hidesRightPanel);
}
