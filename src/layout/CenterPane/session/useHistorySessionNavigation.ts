import { useEffect } from "react";
import { isShareSurface } from "../../../ipc/shareBase";
import { useTermStore } from "../../../store/termStore";

/** A copied history link selects its chat view after the tree arrives, without starting an agent. */
export function useHistorySessionNavigation() {
  useEffect(() => {
    if (isShareSurface) return;
    let applying = false;
    const restore = () => {
      const state = useTermStore.getState();
      const id = new URLSearchParams(window.location.search).get("chatSession");
      if (!state.treeLoaded || !id || !state.sessions.some(session => session.id === id && session.engine === "chat")) return;
      if (state.activeSessionId === id && !state.dormantSessions[id]) return;
      applying = true;
      try { state.openSession(id); } finally { applying = false; }
    };
    const stop = useTermStore.subscribe((state, previous) => {
      if (!applying && state.treeLoaded && previous.treeLoaded && state.activeSessionId !== previous.activeSessionId) {
        const url = new URL(window.location.href);
        const selected = url.searchParams.get("chatSession");
        if (selected && selected !== state.activeSessionId) {
          for (const key of ["chatSession","chatBefore","chatAfter","chatRow","chatField","chatPart","chatFiles","chatSearch","chatChild"]) url.searchParams.delete(key);
          if (state.activeSessionId) url.searchParams.set("session",state.activeSessionId);else url.searchParams.delete("session");
          window.history.pushState(null,"",url.href);
        }
      }
      if (!applying && (state.treeLoaded !== previous.treeLoaded || state.sessions !== previous.sessions || state.dormantSessions !== previous.dormantSessions)) restore();
    });
    window.addEventListener("popstate", restore);restore();
    return () => { stop();window.removeEventListener("popstate", restore); };
  }, []);
}
