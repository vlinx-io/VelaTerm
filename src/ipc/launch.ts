//! Backend-owned choices and argument handling for child-session launch dialogs.

import { invoke } from "./transport";
import type { AgentKind } from "../types";

export interface LaunchOption {
  supportsPlanExecute: boolean;
  planExecuteRoles?: ("plan" | "exec" | "review")[];
  supportsReferSummary: boolean;
  id: AgentKind | "terminal";
  label: string;
  acceptsTask: boolean;
  effortFlag: string | null;
  effortLevels: string[];
}

export const launchOptions = () => invoke<LaunchOption[]>("launch_options");

export interface PlanExecuteContext {
  projectId: string;
  groupId: string | null;
  parentSessionId: string | null;
}

export type PlanExecuteConfig = NonNullable<import("./events").SpawnRequest["planExecute"]>;
export interface SplitTask { name: string; prompt: string; config: PlanExecuteConfig["exec"] }
export interface SplitProposal {
  runId: string; proposalId: string; state: string; tasks: SplitTask[];
  plannerId: string; plannerName: string; cwd: string | null; task: string;
  run: { id: string; state: string; summary: string; config?: PlanExecuteConfig };
  executionTasks: { name: string; prompt: string; run: { id: string; state: string; executorId: string | null } }[];
}
export const pendingSplitProposals = () => invoke<string[]>("plan_execute_split_pending");
export const readSplitProposal = (runId: string) => invoke<SplitProposal>("plan_execute_split_read", { runId });
export const confirmSplitProposal = (request: {runId: string; proposalId: string; tasks: SplitTask[]}) =>
  invoke<{ tasks: SplitProposal["executionTasks"]; errors: { runId: string; error: string }[] }>("plan_execute_split_confirm", { request });
export const cancelSplitProposal = (runId: string, proposalId: string) => invoke<SplitProposal>("plan_execute_split_cancel", { runId, proposalId });
export interface PlanExecutePreparation {
  config: PlanExecuteConfig;
  cwd: string | null;
  worktree: boolean;
  locationNames: string[];
}

export const preparePlanExecute = (context: PlanExecuteContext) =>
  invoke<PlanExecutePreparation>("plan_execute_prepare", { context });

export const createPlanExecute = (request: {
  requestId: string; context: PlanExecuteContext; prompt: string; cwd: string | null;
  worktree: boolean; config: PlanExecuteConfig; images?: import("./chat").ChatImage[];
}) => invoke<{ planner: import("../types").Session; run: { id: string; state: string; summary: string } }>("plan_execute_create", { request });

export const startPlanExecute = (request: import("./events").SpawnRequest) =>
  invoke<{ planner: import("../types").Session; run: { id: string; state: string; summary: string } }>("plan_execute_start", { request });

export const planExecuteDefaults = (parentSessionId: string, config: NonNullable<import("./events").SpawnRequest["planExecute"]>) =>
  invoke<NonNullable<import("./events").SpawnRequest["planExecute"]>>("plan_execute_defaults", { parentSessionId, config });
export const launchSelection = (kind: string, args?: string | null) =>
  invoke<{ model: string; effort: string }>("launch_selection", {
    kind,
    args: args ?? null,
  });

/** Null inherits an argument; an empty string explicitly removes it. */
export const applyLaunchArgs = (
  kind: string,
  args: string | null,
  model?: string | null,
  effort?: string | null,
) =>
  invoke<string | null>("apply_launch_args", {
    kind,
    args,
    model: model ?? null,
    effort: effort ?? null,
  });


export interface LaunchModelContext { parentSessionId?: string | null; cwd?: string | null; inheritArgs?: boolean }
export interface LaunchModel { id: string; label: string; effortLevels: string[] }
export interface LaunchModelCatalog { models: LaunchModel[]; effortLevels: string[] }
export const launchModels = (kind: string, context: LaunchModelContext) =>
  invoke<LaunchModelCatalog>("launch_models", { kind, context });

export interface AgentSessionSelection { kind?: AgentKind; presetId?: string }
export interface AgentSessionContext {
  projectId: string | null;
  groupId: string | null;
  activeSessionId: string | null;
  placement: "sibling" | "child";
}
export interface NewAgentSessionRequest extends AgentSessionSelection {
  requestId: string;
  context: AgentSessionContext;
}
export interface AgentSessionPreparation {
  context: AgentSessionContext;
  options: LaunchOption[];
  presets: import("../types").AgentPreset[];
  locationNames: string[];
}
export const prepareAgentSession = (context: AgentSessionContext) =>
  invoke<AgentSessionPreparation>("agent_session_prepare", { context });
export const createAgentSession = (request: NewAgentSessionRequest) =>
  invoke<import("../types").Session>("agent_session_create", { request });
