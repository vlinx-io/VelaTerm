//! Transport-adapted agent-preset commands.
//!
//! A preset is a saved agent launch configuration — name, icon, executable, default arguments and
//! permission mode — on top of a built-in agent kind. Creating a session from one copies its launch values
//! onto the session, so these commands never affect sessions that already exist.

import { invoke } from "./transport";
import type { AgentPreset, Session } from "../types";

/** Fields a caller supplies when creating or updating a preset. */
export interface AgentPresetInput {
  name: string;
  execPath?: string | null;
  agentArgs?: string | null;
  permissionMode?: string | null;
  /** base64 data URL; null keeps the base kind's built-in icon. */
  icon?: string | null;
}

/** Every preset in menu order. */
export function listAgentPresets(): Promise<AgentPreset[]> {
  return invoke<AgentPreset[]>("list_agent_presets", {});
}

/** Create a preset behaving like `baseKind`. */
export function createAgentPreset(
  baseKind: Session["kind"],
  input: AgentPresetInput,
): Promise<AgentPreset> {
  return invoke<AgentPreset>("create_agent_preset", {
    name: input.name,
    baseKind,
    execPath: input.execPath ?? null,
    agentArgs: input.agentArgs ?? null,
    permissionMode: input.permissionMode ?? null,
    icon: input.icon ?? null,
  });
}

/** Update a preset. Sessions created from it keep their own copied launch values. */
export function updateAgentPreset(id: string, input: AgentPresetInput): Promise<void> {
  return invoke<void>("update_agent_preset", {
    id,
    name: input.name,
    execPath: input.execPath ?? null,
    agentArgs: input.agentArgs ?? null,
    permissionMode: input.permissionMode ?? null,
    icon: input.icon ?? null,
  });
}

/** Delete a preset. Sessions created from it keep launching exactly as before. */
export function deleteAgentPreset(id: string): Promise<void> {
  return invoke<void>("delete_agent_preset", { id });
}

/** Persist a new menu order from the full list of preset IDs. */
export function reorderAgentPresets(ids: string[]): Promise<void> {
  return invoke<void>("reorder_agent_presets", { ids });
}
