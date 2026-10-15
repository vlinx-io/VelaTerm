// Global terminals, documents, standalone browser tabs, tabs of collections and tabs of deleted projects belong
// to no project and stay visible, so scoping never strands a tab the user still has open.

import { firstLeaf, type PaneNode } from "../layout/CenterPane/paneTree";
import type { Session } from "../types";

export interface TabOwnerState {
  paneTrees: Readonly<Record<string, PaneNode>>;
  projects: readonly { id: string; rootPath?: string }[];
  sessions: readonly Session[];
  ephemeralSessions: Readonly<Record<string, Session>>;
  taskTabs: Readonly<Record<string, { sessionId: string }>>;
}

export interface TabScopeState extends TabOwnerState {
  scopeTabsToProject: boolean;
  activeProjectId: string | null;
  openTabs: readonly string[];
}

export function sessionProjectId(state: Pick<TabOwnerState, "projects">, session: Session): string | null {
  if (session.global || !session.projectId) return null;
  const project = state.projects.find((p) => p.id === session.projectId);
  if (!project || project.rootPath === "") return null;
  return project.id;
}

export function tabProjectId(state: TabOwnerState, tabId: string): string | null {
  const find = (id: string) => state.sessions.find((s) => s.id === id) ?? state.ephemeralSessions[id];
  const tree = state.paneTrees[tabId];
  const session =
    find(state.taskTabs[tabId]?.sessionId ?? tabId) ?? (tree ? find(firstLeaf(tree).sessionId) : undefined);
  return session ? sessionProjectId(state, session) : null;
}

export function visibleTabIds(state: TabScopeState): readonly string[] {
  const { activeProjectId } = state;
  if (!state.scopeTabsToProject || activeProjectId === null) return state.openTabs;
  return state.openTabs.filter((id) => {
    const owner = tabProjectId(state, id);
    return owner === null || owner === activeProjectId;
  });
}

export function nextVisibleTab(state: TabScopeState, closingTabId: string): string | null {
  const visible = visibleTabIds(state);
  const rest = visible.filter((id) => id !== closingTabId);
  const idx = visible.indexOf(closingTabId);
  if (idx < 0) return rest[0] ?? null;
  return rest[idx] ?? rest[idx - 1] ?? null;
}

export function projectShowing(state: TabScopeState, tabId: string | null): string | null {
  if (!tabId || visibleTabIds(state).includes(tabId)) return state.activeProjectId;
  return tabProjectId(state, tabId);
}
