import { useEffect, useSyncExternalStore } from "react";
import { invoke } from "../../ipc/transport";
import { onSettingsChanged } from "../../ipc/events";

export type TabBehavior = "accept" | "complete";
export interface CompletionConfig { mode: "auto" | "tab" | "off"; tabBehavior: TabBehavior; modes: ("auto" | "tab" | "off")[]; debounceMs: number; available: boolean }
const listeners = new Set<() => void>();
let config: CompletionConfig | null = null;
let unlisten: (() => void) | undefined;
let loadVersion = 0;

export async function reloadCompletionConfig(): Promise<void> {
  const version = ++loadVersion;
  const previous = config;
  try {
    const next = await invoke<CompletionConfig>("terminal_completion_config");
    if (version === loadVersion) config = next;
  } catch {
    if (version === loadVersion) config = null;
  }
  if (JSON.stringify(previous) !== JSON.stringify(config)) listeners.forEach(fn => fn());
}

export function watchCompletionConfig(fn: () => void): () => void {
  listeners.add(fn);
  if (listeners.size === 1) {
    void reloadCompletionConfig();
    void onSettingsChanged(keys => {
      if (keys.includes("terminal-completion-mode") || keys.includes("terminal-tab-behavior")) void reloadCompletionConfig();
    }).then(stop => { if (listeners.size) unlisten = stop; else stop(); });
  }
  return () => {
    listeners.delete(fn);
    if (!listeners.size) { unlisten?.(); unlisten = undefined; }
  };
}

export const getCompletionConfig = (): CompletionConfig | null => config;
export function useCompletionConfig(): CompletionConfig | null {
  const value = useSyncExternalStore(watchCompletionConfig, getCompletionConfig);
  useEffect(() => { void reloadCompletionConfig(); }, []);
  return value;
}

export async function setCompletionMode(mode: CompletionConfig["mode"]): Promise<void> {
  await invoke("set_app_settings", { entries: { "terminal-completion-mode": mode } });
  await reloadCompletionConfig();
}

export async function setCompletionTabBehavior(behavior: TabBehavior): Promise<void> {
  await invoke("set_app_settings", { entries: { "terminal-tab-behavior": behavior } });
  await reloadCompletionConfig();
}
