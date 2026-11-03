import { invoke } from "./transport";

export interface AgentVersion {
  version: string | null;
  running: boolean;
  latestVersion: string | null;
  updateAvailable: boolean;
  checkStatus: "unchecked" | "checked" | "failed";
  docsUrl: string | null;
  /** The agent's self-update command, present only while an update is available and can run from VelaTerm. */
  updateCommand: string | null;
}

export function agentVersion(sessionId: string, checkUpdates = false): Promise<AgentVersion> {
  return invoke<AgentVersion>("agent_version", { sessionId, checkUpdates });
}

/** Progress of the self-update for the executable a session uses; the backend keeps it until the next run. */
export interface AgentUpdateStatus {
  state: "idle" | "running" | "succeeded" | "failed";
  /** Combined stdout and stderr, raw terminal text with its escape sequences. */
  output: string;
  exitCode: number | null;
  timedOut: boolean;
  /** Version the executable reports after the command finished. */
  version: string | null;
}

export function agentUpdateStatus(sessionId: string): Promise<AgentUpdateStatus> {
  return invoke<AgentUpdateStatus>("agent_update_status", { sessionId });
}

/** Start the self-update, or return the one already running for the same executable. */
export function agentUpdateStart(sessionId: string): Promise<AgentUpdateStatus> {
  return invoke<AgentUpdateStatus>("agent_update_start", { sessionId });
}
