import { useEffect, useState } from "react";
import { genId } from "../genId";
import type { PlanExecuteContext } from "../ipc/launch";

export interface PlanExecuteMenuRoute { requestId: string; context: PlanExecuteContext }

export type BindPlanExecuteRoleMode = "auto" | "exec";

export interface BindPlanExecuteRoute {
  requestId: string;
  role: BindPlanExecuteRoleMode;
  plannerId: string | null;
  executorIds: string[];
  sessionIds: string[];
}

export interface BindPlanExecuteUrlOptions {
  role?: BindPlanExecuteRoleMode;
  plannerId?: string | null;
  executorIds?: readonly string[];
  sessionIds?: readonly string[];
  requestId?: string;
}

const MENU_KEYS = ["planExecute", "planProject", "planGroup", "planParent"] as const;
const BIND_KEYS = ["bindPlanExecute", "bindPlan", "bindExec", "bindRole"] as const;

export function planExecuteUrl(context: PlanExecuteContext | null): string {
  const url = new URL(window.location.href);
  for (const key of MENU_KEYS) url.searchParams.delete(key);
  if (context) {
    for (const key of BIND_KEYS) url.searchParams.delete(key);
    url.searchParams.set("planExecute", genId());
    url.searchParams.set("planProject", context.projectId);
    if (context.groupId) url.searchParams.set("planGroup", context.groupId);
    if (context.parentSessionId) url.searchParams.set("planParent", context.parentSessionId);
  }
  return url.href;
}

export function bindPlanExecuteUrl(opts: BindPlanExecuteUrlOptions | null): string {
  const url = new URL(window.location.href);
  for (const key of BIND_KEYS) url.searchParams.delete(key);
  if (opts) {
    for (const key of MENU_KEYS) url.searchParams.delete(key);
    url.searchParams.set("bindPlanExecute", opts.requestId?.trim() || genId());
    if (opts.role === "exec") url.searchParams.set("bindRole", "exec");
    const planner = opts.plannerId?.trim() || "";
    const execs = (opts.executorIds ?? []).map((id) => id.trim()).filter(Boolean);
    const selected = (opts.sessionIds ?? []).map((id) => id.trim()).filter(Boolean);
    if (planner) {
      url.searchParams.set("bindPlan", planner);
      if (execs.length > 0) url.searchParams.set("bindExec", execs.join(","));
    } else if (selected.length > 0) {
      url.searchParams.set("bindExec", selected.join(","));
    } else if (execs.length > 0) {
      url.searchParams.set("bindExec", execs.join(","));
    }
  }
  return url.href;
}

export function navigatePlanExecute(url: string, replace = false) {
  if (replace) window.history.replaceState(null, "", url);
  else window.history.pushState(null, "", url);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

export const navigateBindPlanExecute = navigatePlanExecute;

function readRoute(): PlanExecuteMenuRoute | null {
  const params = new URL(window.location.href).searchParams;
  const requestId = params.get("planExecute");
  if (!requestId) return null;
  return {
    requestId,
    context: {
      projectId: params.get("planProject") ?? "",
      groupId: params.get("planGroup"),
      parentSessionId: params.get("planParent"),
    },
  };
}

function readBindRoute(): BindPlanExecuteRoute | null {
  const params = new URL(window.location.href).searchParams;
  const requestId = params.get("bindPlanExecute")?.trim();
  if (!requestId) return null;
  const role: BindPlanExecuteRoleMode = params.get("bindRole") === "exec" ? "exec" : "auto";
  const plannerId = params.get("bindPlan")?.trim() || null;
  const rawExec = params.get("bindExec") ?? "";
  const executorIds = rawExec
    .split(",")
    .map((id) => id.trim())
    .filter(Boolean);
  const sessionIds: string[] = [];
  if (plannerId) sessionIds.push(plannerId);
  for (const id of executorIds) {
    if (!sessionIds.includes(id)) sessionIds.push(id);
  }
  return { requestId, role, plannerId, executorIds, sessionIds };
}

export function usePlanExecuteMenuRoute() {
  const [route, setRoute] = useState(readRoute);
  useEffect(() => {
    const update = () => setRoute(previous => {
      const next = readRoute();
      return JSON.stringify(previous) === JSON.stringify(next) ? previous : next;
    });
    window.addEventListener("popstate", update);
    return () => window.removeEventListener("popstate", update);
  }, []);
  return route;
}

export function useBindPlanExecuteRoute() {
  const [route, setRoute] = useState(readBindRoute);
  useEffect(() => {
    const update = () => setRoute(previous => {
      const next = readBindRoute();
      return JSON.stringify(previous) === JSON.stringify(next) ? previous : next;
    });
    window.addEventListener("popstate", update);
    return () => window.removeEventListener("popstate", update);
  }, []);
  return route;
}
