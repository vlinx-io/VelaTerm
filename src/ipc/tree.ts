//! Transport-adapted tree-management commands and directory selection.

import { invoke } from "./transport";
import type {
  Group,
  NodeKind,
  Project,
  Session,
  SessionKind,
  Tree,
} from "../types";

/** Fetch the complete tree. */
export function listTree(): Promise<Tree> {
  return invoke<Tree>("list_tree");
}

/** Import a known path as a project after native or web directory selection. */
export function importProject(rootPath: string): Promise<Project> {
  return invoke<Project>("import_project", { rootPath });
}

/** Create a collection: a top-level container with no folder behind it, for grouping sessions that belong
 * to no single checkout. Its rootPath comes back empty. */
export function createVirtualProject(name: string): Promise<Project> {
  return invoke<Project>("create_virtual_project", { name });
}

/** Clone a remote repository under parentDir and import it as a project. Derive an empty folderName
 * from the repository URL and use the default branch when branch is empty. */
export function cloneProject(
  url: string,
  parentDir: string,
  folderName?: string,
  branch?: string,
  operationId?: string,
): Promise<Project> {
  return invoke<Project>("clone_project", {
    url,
    parentDir,
    folderName: folderName?.trim() || undefined,
    branch: branch?.trim() || undefined,
    operationId,
  });
}

/** Cancel a clone; the backend terminates Git and removes the operation's dedicated temporary directory. */
export function cancelCloneProject(operationId: string): Promise<boolean> {
  return invoke<boolean>("cancel_clone_project", { operationId });
}

export function createGroup(
  projectId: string,
  parentGroupId: string | null,
  name: string,
  worktree?: { worktreePath?: string | null; worktreeBaseRef?: string | null },
): Promise<Group> {
  return invoke<Group>("create_group", {
    projectId,
    parentGroupId,
    name,
    worktreePath: worktree?.worktreePath ?? null,
    worktreeBaseRef: worktree?.worktreeBaseRef ?? null,
  });
}

export interface CreateSessionInput {
  projectId: string;
  groupId: string | null;
  name: string;
  kind?: SessionKind;
  shell?: string | null;
  cwd?: string | null;
  initCmd?: string | null;
  /** Parent session ID; when set, attach the new session as its child. */
  parentSessionId?: string | null;
  /** Associated Git worktree path already created by create_worktree. */
  worktreePath?: string | null;
  /** Full worktree base ref returned by create_worktree and used as the integration target. */
  worktreeBaseRef?: string | null;
  /** Agent-native session ID used as the resume anchor on the next launch. */
  agentSessionId?: string | null;
  /** User-defined agent launch arguments, persisted and applied to every agent-session launch. */
  agentArgs?: string | null;
  /** Agent permission mode: `"skip"` bypasses confirmations; empty/`"default"` asks incrementally. */
  permissionMode?: string | null;
  /** Preset this session was created from; display data only, since the launch values are copied over. */
  agentPresetId?: string | null;
  /** Executable for this session's agent, overriding the per-kind default. */
  agentPath?: string | null;
}

export function createSession(input: CreateSessionInput): Promise<Session> {
  return invoke<Session>("create_session", {
    projectId: input.projectId,
    groupId: input.groupId,
    name: input.name,
    kind: input.kind ?? "terminal",
    shell: input.shell ?? null,
    cwd: input.cwd ?? null,
    initCmd: input.initCmd ?? null,
    parentSessionId: input.parentSessionId ?? null,
    worktreePath: input.worktreePath ?? null,
    worktreeBaseRef: input.worktreeBaseRef ?? null,
    agentSessionId: input.agentSessionId ?? null,
    agentArgs: input.agentArgs ?? null,
    permissionMode: input.permissionMode ?? null,
    agentPresetId: input.agentPresetId ?? null,
    agentPath: input.agentPath ?? null,
  });
}

/** Fork a sibling conversation from the source session's current history without modifying the source. */
export function forkSession(sessionId: string): Promise<Session> {
  return invoke<Session>("fork_session", { sessionId });
}

/** Persist a temporary session under its existing ID rather than generating a new one. */
export interface PersistSessionInput {
  id: string;
  projectId: string;
  groupId: string | null;
  name: string;
  kind: SessionKind;
  shell?: string | null;
  cwd?: string | null;
  initCmd?: string | null;
  parentSessionId?: string | null;
}

export function persistSession(input: PersistSessionInput): Promise<Session> {
  return invoke<Session>("persist_session", {
    id: input.id,
    projectId: input.projectId,
    groupId: input.groupId,
    name: input.name,
    kind: input.kind,
    shell: input.shell ?? null,
    cwd: input.cwd ?? null,
    initCmd: input.initCmd ?? null,
    parentSessionId: input.parentSessionId ?? null,
  });
}

export function renameNode(
  kind: NodeKind,
  id: string,
  name: string,
): Promise<void> {
  return invoke("rename_node", { kind, id, name });
}

/** Set or clear a node's emoji marker. An empty/null mark clears it. */
export function setNodeMark(
  kind: NodeKind,
  id: string,
  mark: string | null,
): Promise<void> {
  return invoke("set_node_mark", { kind, id, mark: mark ?? "" });
}

export interface UpdateSessionInput {
  name: string;
  shell?: string | null;
  cwd?: string | null;
  initCmd?: string | null;
  /** User-defined agent launch arguments such as `--model opus`. */
  agentArgs?: string | null;
  /** Permission mode: `"skip"` bypasses confirmations; empty/`"default"` asks incrementally. */
  permissionMode?: string | null;
}

export function updateSession(
  id: string,
  input: UpdateSessionInput,
): Promise<void> {
  return invoke("update_session", {
    id,
    name: input.name,
    shell: input.shell ?? null,
    cwd: input.cwd ?? null,
    initCmd: input.initCmd ?? null,
    agentArgs: input.agentArgs ?? null,
    permissionMode: input.permissionMode ?? null,
  });
}

export function deleteNode(kind: NodeKind, id: string): Promise<void> {
  return invoke("delete_node", { kind, id });
}

/** Clear a node's worktree binding when converting it to a regular session/group. Remove
 * worktreePath/worktreeBaseRef and clear session cwd so it falls back to the project root. Call only
 * after deleting the bound worktree. */
export function clearNodeWorktree(kind: NodeKind, id: string): Promise<void> {
  return invoke("clear_node_worktree", { kind, id });
}

/** Bind an existing group to a worktree directory, or re-point it at another one. Sessions already in the
 * group keep their own cwd; only sessions created afterwards inherit the binding. Passing null for both
 * clears it. */
export function setGroupWorktree(
  id: string,
  worktreePath: string | null,
  worktreeBaseRef: string | null,
): Promise<void> {
  return invoke("set_group_worktree", { id, worktreePath, worktreeBaseRef });
}

export function moveNode(
  kind: NodeKind,
  id: string,
  targetProjectId: string | null,
  targetGroupId: string | null,
  targetParentSessionId: string | null,
  sortOrder: number,
): Promise<void> {
  return invoke("move_node", {
    kind,
    id,
    targetProjectId,
    targetGroupId,
    targetParentSessionId,
    sortOrder,
  });
}

export function setCollapsed(
  kind: NodeKind,
  id: string,
  collapsed: boolean,
): Promise<void> {
  return invoke("set_collapsed", { kind, id, collapsed });
}

/** Archive or restore a session. Archiving hides it without data loss and keeps read-only playback. */
export function setSessionArchived(
  id: string,
  archived: boolean,
): Promise<void> {
  return invoke("set_session_archived", { id, archived });
}

/** Archive a whole group: archive its sessions and retain a hidden soft-deleted group tombstone that
 * is restored with any member session. */
export function archiveGroup(id: string): Promise<void> {
  return invoke("archive_group", { id });
}

/** List archived sessions in reverse archive-time order for the archive browser. */
export function listArchivedSessions(): Promise<Session[]> {
  return invoke<Session[]>("list_archived_sessions");
}
