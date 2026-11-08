//! Knowledge base protocol. Business data and defaults are supplied by the backend.
import type { Session, SessionKind } from "../types";
import { invoke } from "./transport";
export interface MemoryEntry {
  id: string; title: string; summary: string; content: string; tags: string[]; related: string[];
  sources: string[]; version: number; createdAt: number; updatedAt: number;
}
/** Snippet, line and matched literals are present while a search query is active, not when browsing. */
export interface MemorySummary { id: string; sessionId?: string; title: string; summary: string; snippet?: string; line?: number; matched?: string[]; tags: string[]; version: number; updatedAt: number; sourceCount: number; related?: { id: string; title: string }[] }
export interface MemorySource { id: string; sessionId: string; sessionName: string; kind: string; createdAt: number; digest: string; content?: string; agentSessionId?: string }
export interface MemoryDetail {
  location?: { sessionId: string; sessionName: string; projectId: string; projectName: string; kind: string };
  entry: MemoryEntry; revision: MemoryEntry | null; catalog: { id: string; title: string }[]; backlinks: { id: string; title: string }[];
  sources: MemorySource[]; versions: { version: number; author: string; createdAt: number }[];
  /** This entry's tags that other entries also carry. */
  sharedTags?: string[];
}
export interface MemorySessionGroup { id: string; name: string; kind: "session" | "manual" | "legacy"; count: number }
export interface MemoryProjectGroup { id: string; name: string; kind: "project" | "unknown" | "manual" | "legacy"; count: number; sessions: MemorySessionGroup[] }
export interface MemoryList { projects: MemoryProjectGroup[]; selectedSessionId: string | null; entries: MemorySummary[]; total: number; pageSize: number; tags: string[]; fuzzy?: boolean }
/** One archived root session inside a Collections project group. */
/** One archived session. `parentId` names the archived parent it nests under; roots carry their group
 *  chain as parallel `groupIds` / `groupPath`, descendants carry empty arrays. */
export interface MemoryCollectionSession { id: string; name: string; kind: string; count: number; archivedAt: number | null; parentId: string | null; groupIds: string[]; groupPath: string[] }
export interface MemoryCollectionProject { id: string; name: string; kind: "project" | "unknown"; count: number; sessions: MemoryCollectionSession[] }
/** Archived sessions, descendants included, grouped by their original project, plus the complete session objects for viewers. */
export interface MemoryCollections { projects: MemoryCollectionProject[]; sessions: Session[] }
export interface MemoryJob {
  id: string; sourceId: string; sessionName: string; agent: string; agentLabel: string; model: string; effort: string; status: string; stage: string;
  progress: number; total: number; error: string; entries: string[]; createdAt: number; updatedAt: number;
  /** Entries this job produced that still exist, with their current titles. */
  entryLinks: { id: string; title: string }[];
  /** Failures leading up to this run since the session's previous success; only in the per-session history. */
  failedAttempts?: number;
}
/** The processing history with one row per session, showing each session's current state. */
export interface MemoryJobSessions { jobs: MemoryJob[]; total: number; pageSize: number; attention?: number; organized?: number }
export interface MemoryOptions { agents: { id: SessionKind; label: string; available: boolean }[]; defaultAgent: string; maxSourceChars: number; catalog: { id: string; title: string }[] }
export type MemoryEdit = Pick<MemoryEntry, "title" | "summary" | "content" | "tags" | "related" | "version"> & { id: string | null };
export const memoryOptions = () => invoke<MemoryOptions>("memory_options");
export const memoryList = (args: { query: string; tag: string; sort: string; page: number; sessionId?: string; projectId?: string; selectedId?: string }) => invoke<MemoryList>("memory_list", args);
export const memoryCollections = () => invoke<MemoryCollections>("memory_collections");
export const memoryGet = (id: string, version?: number) => invoke<MemoryDetail>("memory_get", { id, version });
export const memorySave = (args: MemoryEdit) => invoke<MemoryEntry>("memory_save", args);
/** Rename one entry from the knowledge tree; the new title is recorded as a manual revision. */
export const memoryRename = (id: string, version: number, title: string) => invoke<MemoryEntry>("memory_rename", { id, version, title });
/** Reassign an entry to another session group; `__manual__` moves it to the manual collection. */
export const memoryMove = (id: string, version: number, sessionId: string) => invoke<MemoryEntry>("memory_move", { id, version, sessionId });
export const memoryGroupRename = (kind: "project" | "session", id: string, name: string) => invoke<void>("memory_group_rename", { kind, id, name });
export const memoryGroupMove = (sessionId: string, targetProjectId: string) => invoke<void>("memory_group_move", { sessionId, targetProjectId });
export const memoryGroupDelete = (kind: "project" | "session", id: string) => invoke<{ removed: number }>("memory_group_delete", { kind, id });
export const memoryDelete = (id: string, version: number) => invoke<void>("memory_delete", { id, version });
export const memoryRestore = (id: string, version: number, targetVersion: number) => invoke<MemoryEntry>("memory_restore", { id, version, targetVersion });
export const memorySource = (id: string) => invoke<MemorySource>("memory_source", { id });
export const memoryStart = (sessionId: string, agent: string, model: string, effort: string) => invoke<{ id: string; reused: boolean }>("memory_start", { sessionId, agent, model, effort });
export const memoryRetry = (id: string) => invoke<{ id: string }>("memory_retry", { id });
export const memoryCancel = (id: string) => invoke<void>("memory_cancel", { id });
export const memoryJobs = (id = "", page = 0) => invoke<{ jobs: MemoryJob[]; total: number; pageSize: number }>("memory_jobs", { id, page });

export const memoryJobSessions = (page = 0) => invoke<MemoryJobSessions>("memory_jobs", { page, bySession: true });
/** Sessions whose latest organizing run failed and is still unresolved. */
export const memoryAttention = () => memoryJobSessions().then((result) => result.attention ?? 0);
