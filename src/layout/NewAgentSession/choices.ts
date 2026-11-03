import type { LaunchOption } from "../../ipc/launch";
import type { AgentKind, AgentPreset } from "../../types";

export interface AgentChoice {
  id: string;
  label: string;
  kind: AgentKind;
  preset?: AgentPreset;
  recent: boolean;
}
const RECENT_KEY = "vlx-agent-picker-recent";

/** Recent IDs are a display preference only; every available choice comes from the backend. */
export function recentAgentChoices(): string[] {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(value) ? value.filter((id): id is string => typeof id === "string").slice(0, 20) : [];
  } catch { return []; }
}
export function rememberAgentChoice(id: string) {
  try { localStorage.setItem(RECENT_KEY, JSON.stringify([id, ...recentAgentChoices().filter(item => item !== id)].slice(0, 20))); }
  catch { /* Storage restrictions must not turn a successful creation into a failure. */ }
}
export function agentChoices(options: LaunchOption[], presets: AgentPreset[], recent: string[]): AgentChoice[] {
  const kinds = options.filter((option): option is LaunchOption & { id: AgentKind } => option.id !== "terminal");
  const choices: AgentChoice[] = [
    ...kinds.map(option => ({ id: `kind:${option.id}`, kind: option.id, label: option.label, recent: false })),
    ...presets.filter(preset => kinds.some(option => option.id === preset.baseKind)).map(preset => ({
      id: `preset:${preset.id}`, kind: preset.baseKind as AgentKind, label: preset.name, preset, recent: false,
    })),
  ];
  const rank = (id: string) => { const index = recent.indexOf(id); return index < 0 ? Infinity : index; };
  return choices.map(choice => ({ ...choice, recent: recent.includes(choice.id) })).sort((a, b) => rank(a.id) - rank(b.id));
}

const PROJECT_KEY = "vlx-new-session-project";
/** Project the last session was created in, so a New Session tab opened without a location starts there. */
export function recentProjectId(): string | null {
  try { return localStorage.getItem(PROJECT_KEY); } catch { return null; }
}
export function rememberProject(id: string) {
  try { localStorage.setItem(PROJECT_KEY, id); }
  catch { /* Storage restrictions must not turn a successful creation into a failure. */ }
}
