import { invoke } from "./transport";
import { getLocale } from "../i18n";
export interface AuditRequest { projectId: string; agent: string; scope: string; path: string; language?: string; model?: string | null; effort?: string | null }
export interface AuditFinding {
  id: string; title: string; severity: string; path: string; line: number; endLine: number;
  evidence: string; source: string; sink: string; preconditions: string; impact: string;
  recommendation: string; verdict: string;
}
export interface CanonicalFinding {
  findingId: string; title: string; summary: string; severity: {level: string; rationale: string};
  rootCause?: {summary: string} | string; remediation: string;
  validation?: {method?: string; summary?: string; counterEvidence?: string[]} | null;
  attackPath?: {summary?: string; preconditions?: string[]; limitations?: string[]} | null;
  codeEvidence?: {id: string; path: string; startLine: number; endLine?: number; code: string}[];
}
export interface AuditRun extends AuditRequest {
  id: string; sessionId: string; agentLabel: string; root: string; status: string; phase: string; createdAt: number; updatedAt: number;
  files: { path: string; sha256: string }[]; excluded: string[]; reviewed: string[];
  findings: AuditFinding[]; threatModel: string; gaps: string[]; error: string;
  steps: { id: string; phase: string; summary: string; durationMs: number; findingCount: number }[];
  upstream?: {
    package: string; packageVersion: string; pluginVersion: string; adapter: string;
    scan: { scanId: string; findingCount: number; warnings: string[];
      progress: { phase: string; phaseProgress: {completed: number; total: number; unit: string | null};
        preflightIssues: {reason: string; severity: string; status: string}[] } };
    report?: string;
    findings?: {findings: CanonicalFinding[]};
    failureDetail?: string;
    artifactReadError?: string;
  evidenceError?: string | null;
    coverage?: { completeness?: string; deferred: {id: string; reason: string}[] };
    /** Derived by the backend; absent on records imported before upstream 0.8 dropped `completeness`. */
    coverageComplete?: boolean;
  } | null;
}
export type AuditSummary = Pick<AuditRun,"id"|"agent"|"agentLabel"|"status"|"phase"|"createdAt"|"scope"|"path"|"model"|"effort"> & {findingCount: number};
export interface AuditOptions { agents: {id: string; label: string}[]; scopes: string[]; defaultAgent: string; defaultScope: string; workflow: {phases: string[]; package: string; packageVersion: string; pluginVersion: string; adapter: string} }
export const auditOptions = () => invoke<AuditOptions>("security_options");
export const auditList = (projectId: string) => invoke<AuditSummary[]>("security_list",{projectId});
export const auditGet = (id: string) => invoke<AuditRun>("security_get",{id});
export const auditStart = (request: AuditRequest) => invoke<AuditRun>("security_start",{language:getLocale(),...request});
export const auditCancel = (id: string) => invoke<AuditRun>("security_cancel",{id});
export const auditExport = (id: string) => invoke<{markdown: string; run: AuditRun}>("security_export",{id});

