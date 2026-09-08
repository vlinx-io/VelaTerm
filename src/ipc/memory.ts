//! Global Memory protocol. Business data and defaults are supplied by the backend.
import { invoke } from "./transport";
export interface MemoryEntry {
  id: string; title: string; summary: string; content: string; tags: string[]; related: string[];
  sources: string[]; version: number; createdAt: number; updatedAt: number;
}
export interface MemorySummary { id: string; title: string; summary: string; tags: string[]; version: number; updatedAt: number; sourceCount: number }
export interface MemorySource { id: string; sessionId: string; sessionName: string; kind: string; createdAt: number; digest: string; content?: string; agentSessionId?: string }
export interface MemoryDetail {
  entry: MemoryEntry; revision: MemoryEntry | null; catalog: { id: string; title: string }[]; backlinks: { id: string; title: string }[];
  sources: MemorySource[]; versions: { version: number; author: string; createdAt: number }[];
}
export interface MemoryList { entries: MemorySummary[]; total: number; pageSize: number; tags: string[] }
export interface MemoryJob {
  id: string; sourceId: string; sessionName: string; agent: string; model: string; effort: string; status: string; stage: string;
  progress: number; total: number; error: string; entries: string[]; createdAt: number; updatedAt: number;
}
export interface MemoryOptions { agents: { id: string; label: string; available: boolean }[]; defaultAgent: string; maxSourceChars: number; catalog: { id: string; title: string }[] }
export type MemoryEdit = Pick<MemoryEntry, "title" | "summary" | "content" | "tags" | "related" | "version"> & { id: string | null };
export interface MemoryModel { id: string; label: string; effortLevels: string[] }
export const memoryModels = (agent: string) => invoke<MemoryModel[]>("memory_models", { agent });
export const memoryOptions = () => invoke<MemoryOptions>("memory_options");
export const memoryList = (args: { query: string; tag: string; sort: string; page: number }) => invoke<MemoryList>("memory_list", args);
export const memoryGet = (id: string, version?: number) => invoke<MemoryDetail>("memory_get", { id, version });
export const memorySave = (args: MemoryEdit) => invoke<MemoryEntry>("memory_save", args);
export const memoryDelete = (id: string, version: number) => invoke<void>("memory_delete", { id, version });
export const memoryRestore = (id: string, version: number, targetVersion: number) => invoke<MemoryEntry>("memory_restore", { id, version, targetVersion });
export const memorySource = (id: string) => invoke<MemorySource>("memory_source", { id });
export const memoryStart = (sessionId: string, agent: string, model: string, effort: string) => invoke<{ id: string; reused: boolean }>("memory_start", { sessionId, agent, model, effort });
export const memoryRetry = (id: string) => invoke<{ id: string }>("memory_retry", { id });
export const memoryCancel = (id: string) => invoke<void>("memory_cancel", { id });
export const memoryJobs = (id = "", page = 0) => invoke<{ jobs: MemoryJob[]; total: number; pageSize: number }>("memory_jobs", { id, page });
