import { useSyncExternalStore, type AnchorHTMLAttributes, type MouseEvent } from "react";
import { sharedSessionUrl } from "../sharing/sessionNavigation";

export type MobileSessionView = "terminal" | "history";

/** Keep the session and its read-only view in the URL without changing the remote engine. */
export function mobileSessionUrl(id: string | null, view: MobileSessionView = "terminal"): string {
  const url = new URL(sharedSessionUrl(id), window.location.href);
  if (id && view === "history") url.searchParams.set("sessionView", "history");
  else url.searchParams.delete("sessionView");
  return url.href;
}

export function navigateMobileSession(id: string | null, view: MobileSessionView = "terminal", replace = false) {
  const url = mobileSessionUrl(id, view);
  if (url === location.href) return;
  if (replace) history.replaceState(null, "", url);
  else history.pushState(null, "", url);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

const subscribe = (notify: () => void) => {
  window.addEventListener("popstate", notify);
  return () => window.removeEventListener("popstate", notify);
};
const snapshot = () => window.location.search;

export function useMobileSessionNavigation() {
  const params = new URLSearchParams(useSyncExternalStore(subscribe, snapshot));
  return {
    sessionId: params.get("session") || null,
    view: (params.get("sessionView") === "history" ? "history" : "terminal") as MobileSessionView,
  };
}

export function MobileSessionLink({ sessionId, view = "terminal", ...props }:
  Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "href" | "onClick"> & { sessionId: string | null; view?: MobileSessionView }) {
  const follow = (event: MouseEvent<HTMLAnchorElement>) => {
    if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || props.target === "_blank") return;
    event.preventDefault();
    navigateMobileSession(sessionId, view);
  };
  return <a {...props} href={mobileSessionUrl(sessionId, view)} onClick={follow} />;
}
