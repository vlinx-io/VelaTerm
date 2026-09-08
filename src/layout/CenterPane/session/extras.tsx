//! What Claude reports about its own run, drawn beside the composer controls.
//!
//! Each piece here is a small self-contained control: the context meter, the fast-mode switch, the MCP
//! server list, the background-task list, and the two transient lines for a retry in progress and a
//! notification. They read `ChatExtras` and call the chat commands; the pane only passes state through.

import { useEffect, useRef, useState, type ReactNode } from "react";

import Icons from "../../../components/Icons";
import { SELECT_PANEL } from "../../../components/Select";
import { useT } from "../../../i18n";
import {
  chatMcpReconnect,
  chatMcpStatus,
  chatMcpToggle,
  type ChatApiRetry,
  type ChatBackgroundTask,
  type ChatExtras,
  type ChatMcpServer,
} from "../../../ipc/chat";

/** Thousands with one decimal, the way the terminal's own `/context` writes them: 31.6k, 1.0M. */
export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

/** The context figures to show, preferring the agent's own categorised answer over the last frame. */
function contextOf(extras: ChatExtras): { used: number; max: number; percentage: number } | null {
  const usage = extras.contextUsage;
  if (usage && typeof usage.totalTokens === "number" && typeof usage.maxTokens === "number" && usage.maxTokens > 0) {
    return {
      used: usage.totalTokens,
      max: usage.maxTokens,
      percentage: typeof usage.percentage === "number" ? usage.percentage : (usage.totalTokens / usage.maxTokens) * 100,
    };
  }
  if (typeof extras.contextTokens === "number" && typeof extras.contextWindow === "number" && extras.contextWindow > 0) {
    return {
      used: extras.contextTokens,
      max: extras.contextWindow,
      percentage: (extras.contextTokens / extras.contextWindow) * 100,
    };
  }
  return null;
}

/**
 * A ring that fills as the context window does, with the exact figures and the running cost in its
 * tooltip. Nothing is drawn until the agent has reported a first figure.
 */
export function UsageMeter({ extras }: { extras: ChatExtras }) {
  const t = useT();
  const context = contextOf(extras);
  if (!context) return null;
  const pct = Math.max(0, Math.min(100, context.percentage));
  const warning = extras.rateLimit?.status === "allowed_warning";
  const rejected = extras.rateLimit?.status === "rejected";
  const color = rejected || pct >= 90 ? "var(--red)" : warning || pct >= 70 ? "var(--yellow, #d9a400)" : "var(--accent)";
  const radius = 6;
  const circumference = 2 * Math.PI * radius;
  const lines = [
    t("chat.usage.context", formatTokens(context.used), formatTokens(context.max), Math.round(pct)),
  ];
  if (typeof extras.totalCostUsd === "number") lines.push(t("chat.usage.cost", extras.totalCostUsd.toFixed(2)));
  const categories = extras.contextUsage?.categories ?? [];
  for (const category of categories) {
    if (category.tokens > 0) lines.push(`${category.name}: ${formatTokens(category.tokens)}`);
  }
  if (extras.rateLimit && extras.rateLimit.status !== "allowed") {
    const utilization = typeof extras.rateLimit.utilization === "number" ? Math.round(extras.rateLimit.utilization * 100) : undefined;
    const resets = extras.rateLimit.resetsAt ? new Date(extras.rateLimit.resetsAt * 1000).toLocaleString() : "";
    lines.push(
      rejected
        ? t("chat.usage.rateLimited", resets)
        : t("chat.usage.rateWarning", utilization ?? 0, resets),
    );
  }
  return (
    <span className="sv-usage" title={lines.join("\n")} aria-label={lines[0]}>
      <svg width={16} height={16} viewBox="0 0 16 16" aria-hidden="true">
        <circle cx={8} cy={8} r={radius} fill="none" stroke="currentColor" strokeWidth={2.5} opacity={0.2} />
        <circle
          cx={8}
          cy={8}
          r={radius}
          fill="none"
          stroke={color}
          strokeWidth={2.5}
          strokeLinecap="round"
          strokeDasharray={circumference}
          strokeDashoffset={circumference * (1 - pct / 100)}
          transform="rotate(-90 8 8)"
        />
      </svg>
      <span className="sv-usage-pct">{Math.round(pct)}%</span>
    </span>
  );
}

/** A lightning mark: fast mode. Lit in the accent colour while on. */
export function FastModeChip({
  enabled,
  disabled,
  onToggle,
}: {
  enabled: boolean;
  disabled?: boolean;
  onToggle: (enabled: boolean) => void;
}) {
  const t = useT();
  return (
    <button
      className={`sv-chip sv-fast${enabled ? " sv-fast-on" : ""}`}
      title={enabled ? t("chat.fastMode.on") : t("chat.fastMode.off")}
      aria-pressed={enabled}
      disabled={disabled}
      onClick={() => onToggle(!enabled)}
    >
      <span className="sv-chip-glyph">
        <svg width={14} height={14} viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
          <path d="M9.5 1 3 9h4.2L6 15l7-8.5H8.8L9.5 1z" />
        </svg>
      </span>
      <span className="sv-chip-label">{t("chat.fastMode.label")}</span>
    </button>
  );
}

/** A chip whose menu is arbitrary content rather than a list of choices. Closes on outside click and Escape. */
function ChipPopover({
  glyph,
  label,
  title,
  badge,
  width = 300,
  children,
  onOpen,
}: {
  glyph: ReactNode;
  label: string;
  title: string;
  badge?: ReactNode;
  width?: number;
  children: ReactNode;
  onOpen?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const boxRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!open) return;
    onOpen?.();
    const onDown = (e: MouseEvent) => {
      if (!boxRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
    // The opener runs once per opening; it is a refresh, not a subscription.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);
  return (
    <div className="sv-chip-box" ref={boxRef}>
      <button
        className="sv-chip"
        title={title}
        style={open ? { background: "var(--bg-hover)" } : undefined}
        onClick={() => setOpen((v) => !v)}
      >
        <span className="sv-chip-glyph">{glyph}</span>
        <span className="sv-chip-label">{label}</span>
        {badge}
        <Icons.chevD size={12} />
      </button>
      {open && (
        <div
          className="sv-popover"
          style={{ ...SELECT_PANEL, top: "auto", bottom: "calc(100% + 4px)", left: 0, width, maxHeight: 320 }}
        >
          {children}
        </div>
      )}
    </div>
  );
}

/** The MCP servers the agent knows, each with a switch and, when it is down, a reconnect. */
export function McpChip({ sessionId, codex = false }: { sessionId: string; codex?: boolean }) {
  const t = useT();
  const [servers, setServers] = useState<ChatMcpServer[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const apply = (result: Promise<{ mcpServers?: ChatMcpServer[] }>, name?: string) => {
    setBusy(name ?? "*");
    setError(null);
    result
      .then((r) => setServers(r.mcpServers ?? []))
      .catch((err) => setError(String(err)))
      .finally(() => setBusy(null));
  };
  const connected = servers?.filter((s) => s.status === "connected").length;
  return (
    <ChipPopover
      glyph={<Icons.connect size={14} />}
      label="MCP"
      title={t("chat.mcp.tooltip")}
      badge={connected !== undefined ? <span className="sv-chip-badge">{connected}</span> : undefined}
      width={340}
      onOpen={() => apply(chatMcpStatus(sessionId))}
    >
      {error ? (
        <>
          <div className="sv-popover-error" role="alert">
            {/^(?:Error: )?Unknown command: chat_mcp_(?:status|toggle|reconnect)$/.test(error)
              ? t("chat.mcp.backendUnsupported") : error}
          </div>
          <button className="sv-popover-action" disabled={busy !== null} onClick={() => apply(chatMcpStatus(sessionId))}>
            {t("common.retry")}
          </button>
        </>
      ) : servers === null ? (
        busy !== null ? <div className="sv-popover-empty">{t("chat.mcp.loading")}</div> : null
      ) : servers.length === 0 ? (
        <div className="sv-popover-empty">{t("chat.mcp.none")}</div>
      ) : (
        servers.map((server) => {
          const enabled = server.status !== "disabled";
          const down = server.status === "failed" || server.status === "disconnected";
          return (
            <div className="sv-popover-row" key={server.name}>
              <span className={`sv-mcp-dot sv-mcp-${server.status}`} aria-hidden="true" />
              <span className="sv-popover-main">
                <span className="sv-popover-title">{server.name}</span>
                <span className="sv-popover-sub">
                  {t(`chat.mcp.status.${statusKey(server.status)}` as "chat.mcp.status.connected")}
                  {server.tools?.length ? ` · ${t("chat.mcp.tools", server.tools.length)}` : ""}
                </span>
              </span>
              {down && enabled ? (
                <button
                  className="sv-popover-action"
                  disabled={busy !== null}
                  onClick={() => apply(chatMcpReconnect(sessionId, server.name), server.name)}
                >
                  {t("chat.mcp.reconnect")}
                </button>
              ) : null}
              <button
                className="sv-popover-action"
                disabled={busy !== null || server.canToggle === false}
                onClick={() => {
                  if (codex && !window.confirm(t("chat.mcp.codexScope"))) return;
                  apply(chatMcpToggle(sessionId, server.name, !enabled), server.name);
                }}
              >
                {enabled ? t("chat.mcp.disable") : t("chat.mcp.enable")}
              </button>
            </div>
          );
        })
      )}
    </ChipPopover>
  );
}

function statusKey(status: string): "connected" | "disabled" | "failed" | "pending" | "disconnected" | "other" {
  switch (status) {
    case "connected":
    case "disabled":
    case "failed":
    case "pending":
    case "disconnected":
      return status;
    default:
      return "other";
  }
}

/**
 * Claude's background tasks, and the way to send the running turn's foreground work there.
 *
 * Shown while there is something to list or something to background; otherwise the row stays clean.
 */
export function TasksChip({
  tasks,
  busy,
  onStop,
  onBackgroundAll,
}: {
  tasks: ChatBackgroundTask[];
  busy: boolean;
  onStop: (taskId: string) => void;
  onBackgroundAll: () => void;
}) {
  const t = useT();
  if (tasks.length === 0 && !busy) return null;
  return (
    <ChipPopover
      glyph={<Icons.layers size={14} />}
      label={t("chat.tasks.label")}
      title={t("chat.tasks.tooltip")}
      badge={tasks.length > 0 ? <span className="sv-chip-badge">{tasks.length}</span> : undefined}
      width={340}
    >
      {busy ? (
        <button className="sv-popover-wide" onClick={onBackgroundAll}>
          <Icons.download size={12} />
          {t("chat.tasks.backgroundAll")}
        </button>
      ) : null}
      {tasks.length === 0 ? (
        <div className="sv-popover-empty">{t("chat.tasks.none")}</div>
      ) : (
        tasks.map((task) => (
          <div className="sv-popover-row" key={task.task_id}>
            <span className="sv-popover-main">
              <span className="sv-popover-title">{task.description || task.task_id}</span>
              <span className="sv-popover-sub">{task.task_type.replace(/_/g, " ")}</span>
            </span>
            <button className="sv-popover-action" onClick={() => onStop(task.task_id)}>
              {t("chat.tasks.stop")}
            </button>
          </div>
        ))
      )}
    </ChipPopover>
  );
}

/** One line under the working indicator while the agent waits to retry a failed API call. */
export function RetryLine({ retry }: { retry: ChatApiRetry }) {
  const t = useT();
  const seconds = Math.max(1, Math.round(retry.delayMs / 1000));
  return (
    <div className="sv-retry">
      <Icons.restart size={12} />
      {t("chat.retry.line", retry.attempt, retry.maxRetries, seconds, retry.message)}
    </div>
  );
}

/** A notification from the agent's own loop, dismissable, gone on its own after its timeout. */
export function NotificationBar({
  text,
  priority,
  onClose,
}: {
  text: string;
  priority: string;
  onClose: () => void;
}) {
  const t = useT();
  return (
    <div className={`sv-notify sv-notify-${priority}`} role="status">
      <span className="sv-notify-text">{text}</span>
      <button className="sv-notify-close" onClick={onClose} aria-label={t("chat.notify.dismiss")}>
        <Icons.x size={12} />
      </button>
    </div>
  );
}
