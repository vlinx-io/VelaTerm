import { useEffect } from "react";
import { useTermStore } from "../../store/termStore";
import { memoryNavigate, memoryUrl } from "./navigation";

/** Coordinate the URL-owned library tab with the existing session/document tab selection. */
export function useMemoryTab() {
  useEffect(() => {
    let selected = false;
    let updating = false;
    let previous: string | null = null;
    const clearSelection = () => {
      updating = true;
      useTermStore.setState({ activeTabId: null, activeSessionId: null, focusedPaneId: null });
      updating = false;
    };
    const sync = () => {
      const open = !!new URLSearchParams(window.location.search).get("memory");
      if (open && !selected) {
        previous = useTermStore.getState().activeTabId;
        selected = true;
        clearSelection();
      } else if (!open && selected) {
        selected = false;
        const state = useTermStore.getState();
        const target = previous && state.openTabs.includes(previous) ? previous : state.openTabs[0];
        if (target) state.setActiveTab(target);
      }
    };
    const unsubscribe = useTermStore.subscribe((state, before) => {
      if (updating || !selected || !state.activeTabId || state.activeTabId === before.activeTabId) return;
      const target = state.activeTabId;
      clearSelection();
      // Tree restoration and mirrored layouts must not override an explicitly opened URL.
      if (state.projects !== before.projects || state.sidebarTreeViews !== before.sidebarTreeViews) {
        if (!previous || !state.openTabs.includes(previous)) previous = target;
        return;
      }
      previous = target;
      // The editor's existing discard guard also applies when selecting another tab.
      memoryNavigate(memoryUrl(""));
    });
    window.addEventListener("popstate", sync);
    sync();
    return () => {
      unsubscribe();
      window.removeEventListener("popstate", sync);
      if (selected && previous && useTermStore.getState().openTabs.includes(previous)) {
        useTermStore.getState().setActiveTab(previous);
      }
    };
  }, []);
}
