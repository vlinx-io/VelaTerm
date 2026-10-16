import { useEffect, useState } from "react";
import { invoke, onTransportReconnect } from "../ipc/transport";
import type { SessionKind } from "../types";
import type { Mode } from "../layout/CenterPane/session/permissions";
import { useTermStore } from "../store/termStore";
import { SETTINGS_KEY, type AgentDefaultConfig } from "../store/settings";

export interface PermissionCatalog { modes: Mode[]; selected: Mode }

/** Reload after reconnect; unavailable catalogues never fall back to built-in permission choices. */
export function useAgentPermissions(kind: SessionKind, stored?: string | null) {
  const [state, setState] = useState<{ key: string; catalog?: PermissionCatalog; error?: string }>();
  const key = JSON.stringify([kind, stored]);
  useEffect(() => {
    let active = true;
    let generation = 0;
    const refresh = () => {
      const request = ++generation;
      setState({ key });
      if (!["claude", "codex", "zcode", "opencode", "omp", "antigravity"].includes(kind)) return;
      void invoke<PermissionCatalog>("agent_permission_catalog", { agent: kind, stored: stored ?? null })
        .then(catalog => { if (active && request === generation) setState({ key, catalog }); })
        .catch(error => { if (active && request === generation) setState({ key, error: String(error) }); });
    };
    refresh();
    const unlisten = onTransportReconnect(refresh);
    return () => { active = false; unlisten(); };
  }, [key, kind, stored]);
  return state?.key === key ? state : undefined;
}

/** Reflect a confirmed backend patch in the local cache without writing the whole settings object back. */
export async function savePermissionDefault(kind: SessionKind, mode: string) {
  const agentDefaults = await invoke<Record<string, AgentDefaultConfig>>("agent_set_default_permission", { agent: kind, mode });
  useTermStore.setState({ agentDefaults });
  try {
    const cached = JSON.parse(localStorage.getItem(SETTINGS_KEY) || "{}");
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ ...cached, agentDefaults }));
  } catch { /* The backend remains authoritative when local caching is unavailable. */ }
}
