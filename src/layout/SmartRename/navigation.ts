//! Recoverable rename confirmation; navigating never starts a title generation request.

import { useEffect, useState } from "react";

export interface SmartRenameRoute {
  sessionId: string;
  sessionIds: string[];
  agent: string | null;
  model: string | null;
  effort: string | null;
}

function normalizeSessionIds(input: string | readonly string[] | null): string[] {
  if (!input) return [];
  const raw = typeof input === "string" ? input.split(",") : input;
  const seen = new Set<string>();
  const ids: string[] = [];
  for (const item of raw) {
    const id = item.trim();
    if (!id || seen.has(id)) continue;
    seen.add(id);
    ids.push(id);
  }
  return ids;
}

export function readSmartRenameRoute(): SmartRenameRoute | null {
  const query = new URLSearchParams(window.location.search);
  const sessionIds = normalizeSessionIds(query.get("smartRename"));
  if (sessionIds.length === 0) return null;
  return {
    sessionId: sessionIds[0],
    sessionIds,
    agent: query.get("smartRenameAgent"),
    model: query.get("smartRenameModel"),
    effort: query.get("smartRenameEffort"),
  };
}

export function smartRenameUrl(sessionIds: string | readonly string[] | null): string {
  const url = new URL(window.location.href);
  url.searchParams.delete("smartRename");
  url.searchParams.delete("smartRenameAgent");
  url.searchParams.delete("smartRenameModel");
  url.searchParams.delete("smartRenameEffort");
  const ids = normalizeSessionIds(sessionIds);
  if (ids.length > 0) url.searchParams.set("smartRename", ids.join(","));
  return url.href;
}

export function navigateSmartRename(sessionIds: string | readonly string[] | null, replace = false) {
  const url = smartRenameUrl(sessionIds);
  if (url === window.location.href) return;
  window.history[replace ? "replaceState" : "pushState"](null, "", url);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

export function writeSmartRenameSelection(values: Record<string, string>) {
  const url = new URL(window.location.href);
  for (const [field, param] of [["agent", "smartRenameAgent"], ["model", "smartRenameModel"], ["effort", "smartRenameEffort"]]) {
    url.searchParams.set(param, values[field] ?? "");
  }
  window.history.replaceState(null, "", url.href);
}

export function useSmartRenameRoute() {
  const [route, setRoute] = useState(readSmartRenameRoute);
  useEffect(() => {
    const update = () => setRoute(readSmartRenameRoute());
    window.addEventListener("popstate", update);
    return () => window.removeEventListener("popstate", update);
  }, []);
  return route;
}
