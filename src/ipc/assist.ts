//! App actions a session's agent submitted with `vapp`. Each runs only after the user confirms it in that
//! session's view; consecutive host actions are confirmed together.

import { invoke } from "./transport";

export type AssistStatus = "pending" | "running" | "done" | "failed" | "dismissed";

/** One validated action. Fields beyond the common ones depend on `domain` and `kind`. */
export interface AssistAction {
  domain: string;
  kind: string;
  status: AssistStatus;
  /** Runs on the host through `assistRun`; otherwise the app window performs it and reports with `assistMark`. */
  backend: boolean;
  error?: string;
  /** The `$name` later actions use for the item this one creates. */
  ref?: string;
  /** The created item's ID, once this action has run. */
  createdId?: string;
  [field: string]: unknown;
}

/** One `vapp do` submission. */
export interface AssistPlan {
  id: string;
  createdAt: number;
  /** The agent's summary of the request, in the user's language. */
  summary?: string;
  actions: AssistAction[];
  source?: { sessionId: string; sessionName: string; agent: string | null };
}

export const assistGet = (id: string) => invoke<AssistPlan>("assist_get", { id });
/** Requests from this session's agent that still wait for the user, oldest first. */
export const assistPending = (sessionId: string) => invoke<AssistPlan[]>("assist_pending", { sessionId });
export const assistRun = (id: string, index: number, choice?: string) => invoke<AssistPlan>("assist_run", { id, index, confirmed: true, choice });
/** Runs several host actions in order. Failures are recorded on each action rather than thrown. */
export const assistRunAll = (id: string, indices: number[]) => invoke<AssistPlan>("assist_run", { id, indices, confirmed: true });
export const assistMark = (id: string, index: number, status: "done" | "failed" | "dismissed", error?: string) => invoke<AssistPlan>("assist_mark", { id, index, status, error });
export const assistSkipAll = (id: string, indices: number[]) => invoke<AssistPlan>("assist_mark", { id, indices, status: "dismissed" });
