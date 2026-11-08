//! Fixed sidebar collections. Projects, Tasks and Chat are collection rows the backend creates with well-known
//! IDs (`db/system_collections.rs`); Agents and Terminals exist only here and list what is running right now.
//! None of them can be renamed, deleted or moved, and each carries its own icon color. The activity bar also offers
//! Knowledge and Archived Sessions, which are not collections: a pane showing one lists the knowledge bases or the
//! archived sessions instead of the session tree.

import type { ReactNode } from "react";
import Icons from "../../components/Icons";
import { MemoryIcon } from "../Memory/MemoryIcon";
import type { t as translate } from "../../i18n";
import type { Project, Session, SessionRuntime } from "../../types";

export const SYSTEM_PROJECTS = "vlx-system-projects";
export const SYSTEM_TASKS = "vlx-system-tasks";
export const SYSTEM_CHAT = "vlx-system-chat";
export const LIVE_AGENTS = "vlx-live-agents";
export const LIVE_TERMINALS = "vlx-live-terminals";

export type SystemSection = "projects" | "agents" | "terminals" | "tasks" | "chat";
export type LiveSection = "agents" | "terminals";

const SECTION_BY_ID: Record<string, SystemSection> = {
  [SYSTEM_PROJECTS]: "projects",
  [LIVE_AGENTS]: "agents",
  [LIVE_TERMINALS]: "terminals",
  [SYSTEM_TASKS]: "tasks",
  [SYSTEM_CHAT]: "chat",
};
const ORDER: SystemSection[] = ["projects", "agents", "terminals", "tasks", "chat"];
/** A section the activity bar switches to. Agents is not one: it sits inside Terminals, below the terminal list.
 * Archive and Knowledge are sections without being collections. */
export type RailSection = Exclude<SystemSection, "agents"> | "archive" | "knowledge";
/** Anything the sidebar can name and draw an icon for. */
export type SidebarSection = SystemSection | "archive" | "knowledge";
/** Sections in activity bar order. */
export const RAIL_SECTIONS: RailSection[] = [
  ...ORDER.filter((section): section is Exclude<SystemSection, "agents"> => section !== "agents"),
  "archive",
  "knowledge",
];

/** Sections whose contents start expanded. A pane showing one ignores the collapse state shared with Projects, so
 * only what the user collapses in that pane stays collapsed. */
export const sectionStartsExpanded = (section: RailSection): boolean =>
  section === "terminals" || section === "tasks" || section === "chat";

/** Validate a stored or mirrored section name; anything else means no section was given. */
export const parseRailSection = (candidate: unknown): RailSection | undefined =>
  RAIL_SECTIONS.find((section) => section === candidate);

/** The stored collection behind a section; Terminals, Archive and Knowledge have none. */
export const sectionCollection = (section: RailSection): string | null =>
  section === "projects" ? SYSTEM_PROJECTS : section === "tasks" ? SYSTEM_TASKS : section === "chat" ? SYSTEM_CHAT : null;

export const systemSection = (id: string | null | undefined): SystemSection | null =>
  (id && SECTION_BY_ID[id]) || null;
/** A fixed collection stored in the database. */
export const isSystemCollection = (id: string | null | undefined) =>
  id === SYSTEM_PROJECTS || id === SYSTEM_TASKS || id === SYSTEM_CHAT;
export const systemOrder = (id: string) => {
  const section = systemSection(id);
  return section ? ORDER.indexOf(section) : ORDER.length;
};

const LABEL_KEYS = {
  projects: "systemCollection.projects",
  agents: "systemCollection.agents",
  terminals: "systemCollection.terminals",
  tasks: "systemCollection.tasks",
  chat: "systemCollection.chat",
  archive: "memory.collections",
  knowledge: "memory.title",
} as const;
const HINT_KEYS = {
  projects: "systemCollection.projectsHint",
  agents: "systemCollection.agentsHint",
  terminals: "systemCollection.terminalsHint",
  tasks: "systemCollection.tasksHint",
  chat: "systemCollection.chatHint",
} as const;

export const systemLabel = (t: typeof translate, section: SidebarSection) => t(LABEL_KEYS[section]);
export const systemHint = (t: typeof translate, section: SystemSection) => t(HINT_KEYS[section]);

/** Display name of a project row: fixed collections use the interface language, others their own name. */
export function projectDisplayName(t: typeof translate, project: Pick<Project, "id" | "name">) {
  const section = systemSection(project.id);
  return section ? systemLabel(t, section) : project.name;
}

export function SystemCollectionIcon({ section, size = 15 }: { section: SidebarSection; size?: number }): ReactNode {
  const icon = section === "projects" ? <Icons.project size={size} />
    : section === "agents" ? <Icons.bot size={size} />
    : section === "terminals" ? <Icons.terminal size={size} />
    : section === "tasks" ? <Icons.tasks size={size} />
    : section === "archive" ? <Icons.archive size={size} />
    : section === "knowledge" ? <MemoryIcon size={size} />
    : <Icons.chat size={size} />;
  return <span className={`sys-collection-ic is-${section}`}>{icon}</span>;
}

/** Session type chosen in the New Session tab. Only project sessions belong to a project; task and chat
 * sessions are created in their fixed collections. */
export type SessionType = "task" | "project" | "chat";
export const SESSION_TYPE_COLLECTION: Record<Exclude<SessionType, "project">, string> = {
  task: SYSTEM_TASKS,
  chat: SYSTEM_CHAT,
};
export const sessionTypeOf = (projectId: string | null | undefined): SessionType =>
  projectId === SYSTEM_TASKS ? "task" : projectId === SYSTEM_CHAT ? "chat" : "project";

/** Terminals whose process is running, including drafts that live only in center tabs. */
export function liveTerminalIds(sessions: Session[], drafts: Record<string, Session>, runtimes: Record<string, SessionRuntime>) {
  const running = (s: Session) => s.kind === "terminal" && !s.archivedAt && runtimes[s.id]?.status === "running";
  return [...sessions.filter(running), ...Object.values(drafts).filter(running)].map((s) => s.id);
}

/** Terminals with an agent started inside them. The row goes away when the agent exits. */
export function liveAgentIds(terminalIds: string[], runtimes: Record<string, SessionRuntime>) {
  return terminalIds.filter((id) => !!runtimes[id]?.agent);
}
