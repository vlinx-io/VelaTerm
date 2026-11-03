import { act, renderHook } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { changesUrl, readChangesRoute, useChangesRoute } from "./changesRoute";

const state = vi.hoisted(() => {
  type S = Record<string, unknown>;
  const listeners = new Set<(s: S, p: S) => void>();
  const store: { s: S } = { s: {} };
  const set = (patch: S) => { const previous = store.s; store.s = { ...store.s, ...patch }; listeners.forEach((l) => l(store.s, previous)); };
  // Enough of the real store: one tab per comparison, opened or focused by openChanges.
  const reset = () => {
    store.s = { treeLoaded: false, activeTabId: null, docTabs: {} };
    store.s.openChanges = (cwd: string, opts: Record<string, unknown> = {}) => {
      const changes = { cwd, side: opts.side ?? "all", commit: opts.commit ?? null,
        base: opts.commit && opts.base ? opts.base : null, path: opts.path ?? null };
      const id = `doc-${cwd}-${changes.side}-${changes.commit}`;
      set({ docTabs: { ...(store.s.docTabs as S), [id]: { id, kind: "changes", changes } }, activeTabId: id });
    };
  };
  return { store, listeners, set, reset };
});
vi.mock("../../store/termStore", () => {
  const useTermStore = () => null;
  useTermStore.getState = () => state.store.s;
  useTermStore.subscribe = (l: (s: Record<string, unknown>, p: Record<string, unknown>) => void) => {
    state.listeners.add(l); return () => state.listeners.delete(l);
  };
  return { useTermStore };
});

beforeEach(() => { state.reset(); window.history.replaceState(null, "", "/?inspector=git"); });

it("round-trips a range comparison through the URL", () => {
  const href = changesUrl("http://x/?inspector=git", { cwd: "/r", side: "all", path: "a b.ts", commit: "bbbb", base: { from: "aaaa", label: "origin/main" } });
  expect(new URL(href).searchParams.get("inspector")).toBe("git");
  expect(readChangesRoute(new URL(href).search)).toEqual({ cwd: "/r", side: "all", path: "a b.ts", commit: "bbbb", base: { from: "aaaa", label: "origin/main" } });
  expect(readChangesRoute("?changes=/r&changesSide=bogus")?.side).toBe("all");
  expect(readChangesRoute("?inspector=git")).toBeNull();
});

it("opens the URL's tab once the workspace is restored and follows the active tab after that", () => {
  window.history.replaceState(null, "", "/?inspector=git&changes=%2Fr&changesSide=staged&changesPath=x.ts");
  const { unmount } = renderHook(() => useChangesRoute());
  // Restoring the workspace would replace a tab opened earlier, and a mirrored layout landing first must
  // not overwrite the request in the URL.
  expect(state.store.s.activeTabId).toBeNull();
  act(() => state.set({ docTabs: { m: { id: "m", kind: "changes", changes: { cwd: "/m", side: "all", commit: null, base: null, path: null } } }, activeTabId: "m" }));
  expect(new URLSearchParams(window.location.search).get("changes")).toBe("/r");
  act(() => state.set({ treeLoaded: true }));
  const tabs = state.store.s.docTabs as Record<string, { changes: unknown }>;
  expect(tabs[state.store.s.activeTabId as string].changes).toMatchObject({ cwd: "/r", side: "staged", path: "x.ts" });
  // Switching to another kind of tab drops the parameters without a new history entry.
  const length = window.history.length;
  act(() => state.set({ activeTabId: "A" }));
  expect(window.location.search).toBe("?inspector=git");
  expect(window.history.length).toBe(length);
  act(() => (state.store.s.openChanges as (c: string, o: object) => void)("/r", { commit: "cccc" }));
  expect(new URLSearchParams(window.location.search).get("changesCommit")).toBe("cccc");
  unmount();
});
