//! Shared sessionMenu helpers: agent-kind sets, localized session-kind/state names, worktree selection types and
//! labels, information-row components, and related utilities. Shared by useSessionMenu (sessionMenu.tsx) and its
//! dialogs (sessionMenuDialogs.tsx). Keeping them separate avoids duplicate definitions and circular imports
//! between the hook and dialogs.

import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import Icons from "../components/Icons";
import { t, useT } from "../i18n";
import { type WorktreeEntry } from "../ipc/commands";
import { platform } from "../platform";
import { type SessionKind } from "../types";

/** Agent kinds that support custom launch arguments. */
export const AGENT_ARGS_KINDS: SessionKind[] = [
  "claude",
  "codex",
  "zcode",
  "opencode",
  "copilot",
  "cursor",
  "antigravity",
  "cline",
  "pi",
  "omp",
  "crush",
  "kimi",
  "kiro",
  "grok",
  "zoo",
];
export function supportsAgentArgs(kind: SessionKind): boolean {
  return AGENT_ARGS_KINDS.includes(kind);
}

/** Resumable agent kinds. Copilot cannot be validated locally, but an invalid ID does not hang, so resuming is safe. */
export const RESUMABLE_KINDS: { kind: SessionKind; label: string }[] = [
  { kind: "claude", label: "Claude" },
  { kind: "codex", label: "Codex" },
  { kind: "opencode", label: "OpenCode" },
  { kind: "copilot", label: "Copilot" },
  { kind: "cursor", label: "Cursor" },
  { kind: "antigravity", label: "Antigravity" },
  { kind: "cline", label: "Cline" },
  { kind: "pi", label: "Pi" },
  { kind: "omp", label: "OMP" },
  { kind: "crush", label: "Crush" },
  { kind: "kimi", label: "Kimi Code" },
  { kind: "kiro", label: "Kiro" },
  { kind: "grok", label: "Grok Build" },
  { kind: "zoo", label: "Zoo Code" },
];

/** Worktree choice for a new session: none, create one, or attach to an existing worktree. */
export type WorktreeChoice =
  | { mode: "none" }
  | { mode: "new"; name: string }
  | { mode: "existing"; path: string };

/** Localized session-kind display name; brand names remain untranslated. */
export function kindLabel(kind: SessionKind): string {
  if (kind === "terminal") return t("kind.terminal");
  if (kind === "browser") return t("kind.browser");
  if (kind === "claude") return "Claude";
  if (kind === "codex") return "Codex";
  if (kind === "zcode") return "Zcode";
  if (kind === "opencode") return "OpenCode";
  if (kind === "copilot") return "Copilot";
  if (kind === "cursor") return "Cursor";
  if (kind === "antigravity") return "Antigravity";
  if (kind === "cline") return "Cline";
  if (kind === "pi") return "Pi";
  if (kind === "omp") return "OMP";
  if (kind === "crush") return "Crush";
  if (kind === "kimi") return "Kimi Code";
  if (kind === "kiro") return "Kiro";
  if (kind === "grok") return "Grok Build";
  return "Zoo Code";
}

/** Localized state display name; return unknown values unchanged. */
export function statusLabel(status: string): string {
  switch (status) {
    case "idle":
      return t("status.idle");
    case "running":
      return t("status.running");
    case "exited":
      return t("status.exited");
    case "error":
      return t("status.error");
    case "working":
      return t("status.working");
    case "asking":
      return t("status.asking");
    case "waiting":
      return t("status.waiting");
    case "background":
      return t("status.background");
    default:
      return status;
  }
}

/** Label for an existing-worktree option: branch name (directory name when detached) plus directory name. */
export function worktreeLabel(w?: WorktreeEntry): string {
  if (!w) return "";
  const leaf = w.path.split("/").filter(Boolean).pop() || w.path;
  return w.branch ? `${w.branch}  ·  ${leaf}` : leaf;
}

/** Copy supplied text with an optional contextual tooltip and confirmed clipboard feedback. */
export function CopyBtn({ value, label, copiedLabel, disabled = false, icon, tooltip = false }: {
  value: string; label?: string; copiedLabel?: string; disabled?: boolean; icon?: ReactNode; tooltip?: boolean;
}) {
  const t = useT(); const tooltipId = useId(); const request = useRef(0);
  const [status, setStatus] = useState<"idle" | "copying" | "copied" | "failed">("idle");
  useEffect(() => {
    setStatus("idle");
    return () => { request.current += 1; };
  }, [value]);
  useEffect(() => {
    if (status !== "copied" && status !== "failed") return;
    const timer = window.setTimeout(() => setStatus("idle"), 2000);
    return () => window.clearTimeout(timer);
  }, [status]);
  const action = label ?? t("common.copy");
  const feedback = status === "copied" ? copiedLabel ?? t("common.copied") : status === "failed" ? t("common.copyFailed") : action;
  const copy = async () => {
    if (disabled || status === "copying") return;
    const id = ++request.current;
    setStatus("copying");
    try {
      await platform.clipboard.writeText(value, { reportFailure: true });
      if (request.current === id) setStatus("copied");
    } catch {
      if (request.current === id) setStatus("failed");
    }
  };
  const button = (
    <button
      type="button"
      className="icon-btn sm"
      title={tooltip ? undefined : feedback}
      aria-label={action}
      aria-describedby={tooltip ? tooltipId : undefined}
      disabled={disabled}
      aria-busy={status === "copying"}
      aria-disabled={disabled || status === "copying"}
      style={{ flex: "0 0 auto", color: status === "copied" ? "var(--green)" : "var(--text-muted)" }}
      onClick={() => void copy()}
    >
      {status === "copied" ? <Icons.check size={13} /> : status === "failed" ? <Icons.x size={13} /> : icon ?? <Icons.copy size={13} />}
    </button>
  );
  if (!tooltip) return button;
  return <span className="copy-btn-wrap" data-copy-feedback={status === "copied" || status === "failed"}>
    {button}
    <span id={tooltipId} className="copy-btn-tooltip" role="tooltip">{feedback}</span>
    <span className="copy-btn-announcement" role={status === "copied" || status === "failed" ? "status" : undefined}>{status === "copied" || status === "failed" ? feedback : ""}</span>
  </span>;
}

/** Information row with a left label and right value; IDs/paths use monospace and can be copied. */
export function InfoRow({
  label,
  value,
  mono,
  copy,
}: {
  label: string;
  value: React.ReactNode;
  mono?: boolean;
  copy?: string | null;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "5px 0",
        borderBottom: "1px solid var(--border)",
      }}
    >
      <span
        style={{ flex: "0 0 84px", fontSize: 12, color: "var(--text-muted)" }}
      >
        {label}
      </span>
      <span
        style={{
          flex: 1,
          minWidth: 0,
          fontSize: 12.5,
          color: "var(--text-primary)",
          fontFamily: mono
            ? 'Menlo, Monaco, "Courier New", monospace'
            : undefined,
          wordBreak: "break-all",
          userSelect: "text",
        }}
      >
        {value}
      </span>
      {copy ? <CopyBtn value={copy} /> : null}
    </div>
  );
}
