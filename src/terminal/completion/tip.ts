import { useCallback, useSyncExternalStore } from "react";

/** Which key the pane header should name: Tab or → takes the hint, or Tab reverts the one just taken. */
export type CompletionTip = "accept-tab" | "accept-arrow" | "undo";

const tips = new Map<string, CompletionTip>();
const listeners = new Set<() => void>();

export function setCompletionTip(sessionId: string, tip: CompletionTip | null): void {
  if ((tips.get(sessionId) ?? null) === tip) return;
  if (tip) tips.set(sessionId, tip); else tips.delete(sessionId);
  listeners.forEach(fn => fn());
}

export function useCompletionTip(sessionId: string): CompletionTip | null {
  const subscribe = useCallback((fn: () => void) => {
    listeners.add(fn);
    return () => { listeners.delete(fn); };
  }, []);
  return useSyncExternalStore(subscribe, () => tips.get(sessionId) ?? null);
}
