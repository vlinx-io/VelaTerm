//! Right-side Info tab: basic session details and uptime, model/context, Claude/Codex/Grok quotas,
//! current-turn statistics, and a Resources section at the bottom holding this session's process-tree
//! CPU/memory alongside whole-machine CPU, memory, and saturation. Extracted from RightPanel;
//! usage helpers are private to this tab, while the shared KV row lives in parts.

import { useCallback, useEffect, useRef, useState } from "react";
import Icons from "../../components/Icons";
import { fmtBytes, fmtTokens } from "../../format";
import { dateLocale } from "../../i18n";
import { useGitBranch } from "../../hooks/useGitBranch";
import {
  agentContextInfo,
  agentTurnStats,
  usageRefresh,
  type AgentContextInfo,
  type AgentTurnStats,
  type UsageProvider,
  type UsageSnapshot,
} from "../../ipc/commands";
import { processStats, systemStats, type ProcStats, type SystemStats } from "../../ipc/info";
import { type Session, type SessionKind } from "../../types";
import { useTermStore } from "../../store/termStore";
import { usageBrandIconEl } from "../../components/brandIcons";
import { kindIconEl } from "../sessionViewers/sessionMeta";
import { KV } from "./parts";

/** Format uptime as 12s, 4m 12s, 1h 3m, or 2d 5h. */
function fmtUptime(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ${m % 60}m`;
  return `${Math.floor(h / 24)}d ${h % 24}h`;
}

/** One-second clock tick for leaf components, limiting rerenders to the small regions that display it. */
function useNowTick(): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);
  return now;
}

/** Self-contained uptime row that rerenders only this KV each second. */
function UptimeKV({ startedAt }: { startedAt: number | undefined }) {
  const now = useNowTick();
  return <KV k="uptime" v={startedAt ? fmtUptime(now - startedAt) : "—"} />;
}

/** Color a 0-100 percentage by severity, matching the quota rows: red at 90%, yellow at 70%. */
function loadTone(pct: number): "ok" | "warn" | "crit" {
  if (pct >= 90) return "crit";
  if (pct >= 70) return "warn";
  return "ok";
}

/** Small inline bar that gives the system rows a shape to read at a glance instead of bare numbers. */
function Meter({ pct, tone }: { pct: number; tone: "ok" | "warn" | "crit" }) {
  const w = Math.max(0, Math.min(100, pct));
  return (
    <span className={`res-meter ${tone}`}>
      <i style={{ width: `${w}%` }} />
    </span>
  );
}

/** KV row whose value is a meter plus text. The key column is fixed width so every bar in the group
 * starts at the same x and the group reads as one small chart rather than four ragged rows. */
function MeterKV({
  k,
  pct,
  tone,
  text,
  title,
}: {
  k: string;
  pct: number;
  tone: "ok" | "warn" | "crit";
  text: string;
  title?: string;
}) {
  return (
    <div className="kv meter" title={title}>
      <span className="k">{k}</span>
      <span className="v">
        <Meter pct={pct} tone={tone} />
        <span className="num">{text}</span>
      </span>
    </div>
  );
}

/** Whole-machine rows. Which saturation signal appears is decided by the backend, not by sniffing the
 * platform here: macOS sends the kernel's memory pressure level, Linux sends load averages. */
function SystemRows({ sys }: { sys: SystemStats | null }) {
  if (!sys) {
    return (
      <>
        <KV k="cpu" v="—" />
        <KV k="memory" v="—" />
      </>
    );
  }
  const memPct = sys.memTotal > 0 ? (sys.memUsed / sys.memTotal) * 100 : 0;
  const level = sys.pressure === "critical" ? "crit" : sys.pressure === "warning" ? "warn" : "ok";
  // On macOS the kernel's verdict outranks the used/total ratio, which reads alarmingly high there
  // because compressed and cached pages count as used.
  const memTone = sys.pressure ? level : loadTone(memPct);
  // A load average equal to the core count means the machine is exactly saturated, so normalize by cores.
  const load1Pct = sys.load && sys.cores > 0 ? (sys.load[0] / sys.cores) * 100 : 0;
  return (
    <>
      <MeterKV
        k="cpu"
        pct={sys.cpu}
        tone={loadTone(sys.cpu)}
        text={`${sys.cpu.toFixed(0)}%`}
        title={`${sys.cores} logical cores`}
      />
      <MeterKV
        k="memory"
        pct={memPct}
        tone={memTone}
        text={`${fmtBytes(sys.memUsed)} / ${fmtBytes(sys.memTotal)}`}
      />
      {sys.pressure && (
        <MeterKV
          k="pressure"
          pct={sys.pressurePct ?? 0}
          tone={level}
          text={sys.pressurePct != null ? `${Math.round(sys.pressurePct)}% · ${sys.pressure}` : sys.pressure}
          title="macOS memory pressure: the kernel's own level, with the share of memory it counts as unavailable"
        />
      )}
      {sys.load && (
        <MeterKV
          k="load"
          pct={load1Pct}
          tone={loadTone(load1Pct)}
          text={sys.load.map((n) => n.toFixed(2)).join("  ")}
          title={`1 / 5 / 15 minute load average over ${sys.cores} cores`}
        />
      )}
      {sys.swapTotal > 0 && sys.swapUsed > 0 && (
        <MeterKV
          k="swap"
          pct={(sys.swapUsed / sys.swapTotal) * 100}
          tone={loadTone((sys.swapUsed / sys.swapTotal) * 100)}
          text={`${fmtBytes(sys.swapUsed)} / ${fmtBytes(sys.swapTotal)}`}
        />
      )}
    </>
  );
}

/** Resources leaf: sample this session's PID tree and the whole machine every three seconds and rerender
 * only this section. Both calls run through desktop_call on the blocking thread pool. Polling occurs only
 * while this component is mounted and stops when the panel is hidden.
 *
 * The two groups answer different questions — "what is this session costing" and "how loaded is the box" —
 * so they are labeled subgroups of one Resources section rather than two sections that would read as
 * unrelated. For a remote session both describe the remote host, which is what the session actually runs on.
 * The machine group is optional: users who only care about their own session hide it from the header
 * checkbox, which also stops the machine-wide sampling.
 */
function ResourcesSection({ pid }: { pid: number | undefined }) {
  const [stats, setStats] = useState<ProcStats | null>(null);
  const [sys, setSys] = useState<SystemStats | null>(null);
  // Persisted per user, so the choice survives restarts and applies to every session's Info panel.
  const showSystem = useTermStore((s) => s.showSystemResources);
  const setShowSystem = useTermStore((s) => s.setShowSystemResources);
  useEffect(() => {
    let cancelled = false;
    const sample = () => {
      if (pid == null) setStats(null);
      else
        processStats(pid)
          .then((s) => !cancelled && setStats(s))
          .catch(() => !cancelled && setStats(null));
      // Hiding the group also stops sampling the machine: the rows are the only consumer of this call.
      if (showSystem)
        systemStats()
          .then((s) => !cancelled && setSys(s))
          .catch(() => !cancelled && setSys(null));
    };
    sample();
    // Three seconds still appears live to users while reducing sampling cost by one-third versus 2s.
    const t = setInterval(sample, 3000);
    return () => {
      cancelled = true;
      clearInterval(t);
    };
  }, [pid, showSystem]);

  return (
    <div className="insp-section">
      <h4 style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span>Resources</span>
        <label className="res-toggle" title="Show whole-machine CPU, memory, and saturation">
          <input
            type="checkbox"
            checked={showSystem}
            onChange={(e) => setShowSystem(e.target.checked)}
          />
          system
        </label>
      </h4>
      <div className="res-sub">this session</div>
      <KV k="cpu" v={stats ? `${stats.cpu.toFixed(1)}%` : "—"} />
      <KV k="memory" v={stats ? fmtBytes(stats.rssBytes) : "—"} />
      {showSystem && (
        <>
          <div className="res-sub">system</div>
          <SystemRows sys={sys} />
        </>
      )}
    </div>
  );
}

/** Normalize Claude ISO strings and Codex Unix seconds to Date, returning null for invalid values. */
function toResetDate(at: string | number | null | undefined): Date | null {
  if (at == null || at === "") return null;
  const d = typeof at === "number" ? new Date(at * 1000) : new Date(at);
  return Number.isNaN(d.getTime()) ? null : d;
}

/** Format quota reset time as local 24-hour HH:mm, adding M/D when it falls on another day. */
function fmtAbsTime(d: Date): string {
  const hm = d.toLocaleTimeString(dateLocale(), {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
  const now = new Date();
  if (d.toDateString() === now.toDateString()) return hm;
  const md = d.toLocaleDateString(dateLocale(), { month: "numeric", day: "numeric" });
  return `${md} ${hm}`;
}

/** Countdown to automatic refresh as mm:ss or h:mm:ss, clamped to 0:00 and driven by `now`. */
function fmtRefreshLeft(ms: number): string {
  let s = Math.max(0, Math.ceil(ms / 1000));
  const h = Math.floor(s / 3600);
  s -= h * 3600;
  const m = Math.floor(s / 60);
  const ss = String(s % 60).padStart(2, "0");
  const mm = String(m).padStart(h > 0 ? 2 : 1, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

/** Format the "updated at" time as HH:mm:ss. */
function fmtUpdatedAt(ts: number): string {
  return new Date(ts).toLocaleTimeString(dateLocale(), {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}

/** Convert a Codex window length in minutes to labels such as 5h, 7d, or 30d. */
function codexWindowLabel(min: number): string {
  if (min <= 360) return "5h";
  if (min < 1440) return `${Math.round(min / 60)}h`;
  if (min <= 10080) return "7d";
  if (min <= 44640) return "30d";
  return `${Math.round(min / 1440)}d`;
}

/** Color usage percentages by severity: red at 90%, orange at 70%, otherwise normal. */
function usageColor(pct: number): string {
  if (pct >= 90) return "var(--red)";
  if (pct >= 70) return "var(--orange, #d8954a)";
  return "var(--text-primary)";
}

/** Quota row with a label and the used percentage plus the window's reset time. */
function UsageRow({
  label,
  pct,
  reset,
}: {
  label: string;
  pct: number;
  reset?: string | number | null;
}) {
  const target = toResetDate(reset);
  return (
    <div className="kv">
      <span className="k">{label}</span>
      <span className="v">
        <span style={{ color: usageColor(pct), fontVariantNumeric: "tabular-nums" }}>
          {Math.round(pct)}% used
        </span>
        {target && (
          <span
            style={{
              color: "var(--text-faint)",
              marginLeft: 8,
              fontSize: 11,
              fontVariantNumeric: "tabular-nums",
            }}
          >
            · ↻ {fmtAbsTime(target)}
          </span>
        )}
      </span>
    </div>
  );
}

/** Placeholder for a quota the backend has not read yet or could not read, with the full error on hover. */
function UsageHint({ busy, err }: { busy: boolean; err: string | null }) {
  return (
    <div className="kv">
      <span className="k" style={{ color: "var(--text-faint)" }}>
        {busy ? "loading…" : err ? "unavailable" : "—"}
      </span>
      {!busy && err && (
        <span
          className="v"
          title={err}
          style={{
            color: "var(--text-faint)",
            fontSize: 10,
            maxWidth: 190,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {err}
        </span>
      )}
    </div>
  );
}

/**
 * "Fable 5" / "Claude Opus 4.8" -> "fable" / "opus": the family name only, matching the existing
 * "7d · opus" row label. The API sends display names with or without the "Claude " prefix, so drop it
 * before taking the first word, or every model would come out labelled "claude".
 */
function modelShortName(display: string): string {
  const name = display.trim().replace(/^claude\s+/i, "");
  return name.split(/\s+/)[0]?.toLowerCase() || display.trim().toLowerCase();
}

/** Rows for one provider's stored reading, or null when it holds no window worth showing yet. */
function usageRows(provider: UsageProvider, snap: UsageSnapshot | null): React.ReactNode {
  if (!snap) return null;
  if (provider === "claude") {
    const u = snap.claude.data;
    if (!u || (!u.fiveHour && !u.sevenDay)) return null;
    return (
      <>
        {u.fiveHour && <UsageRow label="5h" pct={u.fiveHour.utilization} reset={u.fiveHour.resetsAt} />}
        {u.sevenDay && <UsageRow label="7d" pct={u.sevenDay.utilization} reset={u.sevenDay.resetsAt} />}
        {u.sevenDayOpus && (
          <UsageRow label="7d · opus" pct={u.sevenDayOpus.utilization} reset={u.sevenDayOpus.resetsAt} />
        )}
        {(u.modelWeekly ?? [])
          .filter((w) => !(u.sevenDayOpus && modelShortName(w.model) === "opus"))
          .map((w) => (
            <UsageRow
              key={w.model}
              label={`7d · ${modelShortName(w.model)}`}
              pct={w.utilization}
              reset={w.resetsAt}
            />
          ))}
      </>
    );
  }
  if (provider === "codex") {
    const u = snap.codex.data;
    if (!u || (!u.primary && !u.secondary)) return null;
    return (
      <>
        {u.primary && (
          <UsageRow
            label={codexWindowLabel(u.primary.windowMinutes)}
            pct={u.primary.usedPercent}
            reset={u.primary.resetsAt}
          />
        )}
        {u.secondary && (
          <UsageRow
            label={codexWindowLabel(u.secondary.windowMinutes)}
            pct={u.secondary.usedPercent}
            reset={u.secondary.resetsAt}
          />
        )}
      </>
    );
  }
  const u = snap.grok.data;
  if (!u) return null;
  return (
    <>
      <UsageRow label={u.windowLabel || "7d"} pct={u.usedPercent} reset={u.periodEnd} />
      {u.buildPercent != null && <UsageRow label="build" pct={u.buildPercent} />}
    </>
  );
}

/** Display label for each quota source. */
function providerLabel(kind: SessionKind): string {
  if (kind === "codex") return "Codex";
  if (kind === "grok") return "Grok";
  return "Claude";
}

/** Quota section with a Usage title, branded source matching sidebar colors, refresh countdown, and
 * refresh button. Keep the last-updated time in a tooltip to preserve a compact layout.
 *
 * `error` set while rows are showing means the latest poll failed and the numbers are from `usageAt`;
 * a stale marker says so, with the reason on hover, instead of letting old numbers pass as current. */
function UsageSection({
  kind,
  usageAt,
  usageBusy,
  error,
  errorAt,
  refreshSec,
  spinTick,
  onRefresh,
  children,
}: {
  kind: SessionKind;
  usageAt: number | null;
  usageBusy: boolean;
  error: string | null;
  errorAt: number | null;
  refreshSec: number;
  spinTick: number;
  onRefresh: () => void;
  children: React.ReactNode;
}) {
  // Keep the one-second countdown tick local so only this section rerenders.
  const now = useNowTick();
  // Count down from the last attempt, not the last success: a failing poller still retries on schedule.
  const lastAttempt = Math.max(usageAt ?? 0, errorAt ?? 0) || null;
  const nextLeft =
    lastAttempt != null && refreshSec > 0 ? Math.max(0, lastAttempt + refreshSec * 1000 - now) : null;
  const tip = usageAt != null ? `Updated ${fmtUpdatedAt(usageAt)}` : "Refresh";
  const stale = error != null && usageAt != null;
  const staleTip = stale
    ? `Refresh failed${errorAt != null ? ` at ${fmtUpdatedAt(errorAt)}` : ""}: ${error}\nShowing reading from ${fmtUpdatedAt(usageAt)}`
    : "";
  return (
    <div className="insp-section">
      <h4 style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span>Usage</span>
        <span
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 4,
            fontWeight: 600,
            letterSpacing: 0.3,
            textTransform: "none",
            fontSize: 9,
            lineHeight: 1.5,
            padding: "1px 7px 1px 5px",
            borderRadius: 999,
            background: "var(--bg-active)",
            color: "var(--text-dim)",
          }}
        >
          {usageBrandIconEl(kind, 11) ?? kindIconEl(kind, 11)}
          {providerLabel(kind)}
        </span>
        {stale && (
          <span
            title={staleTip}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 3,
              fontWeight: 600,
              letterSpacing: 0.3,
              textTransform: "none",
              fontSize: 9,
              lineHeight: 1.5,
              padding: "1px 6px",
              borderRadius: 999,
              background: "color-mix(in srgb, var(--orange, #d8954a) 18%, transparent)",
              color: "var(--orange, #d8954a)",
              cursor: "help",
            }}
          >
            <Icons.info size={10} />
            stale
          </span>
        )}
        {nextLeft != null && (
          <span
            title={tip}
            style={{
              marginLeft: "auto",
              fontWeight: 400,
              letterSpacing: 0,
              textTransform: "none",
              color: "var(--text-faint)",
              fontSize: 10,
              fontVariantNumeric: "tabular-nums",
            }}
          >
            ↻ {fmtRefreshLeft(nextLeft)}
          </span>
        )}
        <button
          title={tip}
          onClick={onRefresh}
          disabled={usageBusy}
          style={{
            marginLeft: nextLeft != null ? 0 : "auto",
            display: "inline-flex",
            alignItems: "center",
            background: "transparent",
            border: "none",
            padding: 2,
            color: usageBusy ? "var(--accent)" : "var(--text-faint)",
            cursor: usageBusy ? "default" : "pointer",
            opacity: 1,
          }}
        >
          <span key={spinTick} className={usageBusy ? "spin" : undefined} style={{ display: "inline-flex" }}>
            <Icons.restart size={11} />
          </span>
        </button>
      </h4>
      {children}
    </div>
  );
}

export function InfoTab({ session, cwd }: { session: Session; cwd: string | null }) {
  const runtime = useTermStore((s) => s.runtimes[session.id]);
  const branch = useGitBranch(cwd);
  const isAgent = session.kind !== "terminal";
  const isClaude = session.kind === "claude";
  const isCodex = session.kind === "codex";
  const isGrok = session.kind === "grok";
  const hasContext = isClaude || isCodex || isGrok;

  // Model/context usage comes from Claude/Codex transcripts or Grok session signals. Refresh on
  // session changes, work-state changes at turn end, and tool changes after each call.
  const [ctx, setCtx] = useState<AgentContextInfo | null>(null);
  const [turn, setTurn] = useState<AgentTurnStats | null>(null);
  const agentState = runtime?.agentState;
  const currentTool = runtime?.currentTool;
  useEffect(() => {
    // Gate reads until hooks capture agentSessionId (transcript / signals path is then locatable).
    if (!hasContext || !session.agentSessionId) {
      setCtx(null);
      setTurn(null);
      return;
    }
    let cancelled = false;
    agentContextInfo(session.id)
      .then((c) => !cancelled && setCtx(c))
      .catch(() => !cancelled && setCtx(null));
    if (isClaude) {
      agentTurnStats(session.id)
        .then((t) => !cancelled && setTurn(t))
        .catch(() => !cancelled && setTurn(null));
    } else {
      setTurn(null);
    }
    return () => {
      cancelled = true;
    };
  }, [session.id, session.agentSessionId, hasContext, isClaude, agentState, currentTool]);

  // While working, context tokens and turn statistics change between tool and turn-end events. Poll
  // every 1.5 seconds during work for freshness and stop outside the working state.
  useEffect(() => {
    if (!hasContext || !session.agentSessionId || agentState !== "working") return;
    let cancelled = false;
    const t = setInterval(() => {
      agentContextInfo(session.id)
        .then((c) => !cancelled && setCtx(c))
        .catch(() => {});
      if (isClaude) {
        agentTurnStats(session.id)
          .then((s) => !cancelled && setTurn(s))
          .catch(() => {});
      }
    }, 1500);
    return () => {
      cancelled = true;
      clearInterval(t);
    };
  }, [session.id, session.agentSessionId, hasContext, isClaude, agentState]);

  // Account-level quotas come from the backend's single stored snapshot: one poller per machine keeps it
  // fresh and broadcasts changes, so this panel only reads it. Which entry to show follows the session kind.
  const usage = useTermStore((s) => s.usage);
  const provider: UsageProvider | null = isClaude
    ? "claude"
    : isCodex
      ? "codex"
      : isGrok
        ? "grok"
        : null;
  const entry = provider && usage ? usage[provider] : null;
  const rows = provider ? usageRows(provider, usage) : null;
  const [usageBusy, setUsageBusy] = useState(false);
  // Increment on refresh to remount the spinner and reliably restart its animation.
  const [spinTick, setSpinTick] = useState(0);
  const previousAgentStateRef = useRef(agentState);
  const spinHoldRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Manual refresh: ask the backend to refetch this one provider now and bypass its short TTL cache. The
  // updated snapshot reaches every client through `usage://changed`, this one included.
  const refreshUsage = useCallback(() => {
    if (!provider) return;
    setSpinTick((n) => n + 1);
    setUsageBusy(true);
    // Keep the spinner visible for at least two rotations: a cached answer can return in milliseconds,
    // which would otherwise look like nothing happened.
    const start = Date.now();
    void usageRefresh(provider, true)
      .then((snap) => useTermStore.getState().setUsage(snap))
      .catch(() => {})
      .finally(() => {
        if (spinHoldRef.current) clearTimeout(spinHoldRef.current);
        spinHoldRef.current = setTimeout(
          () => setUsageBusy(false),
          Math.max(0, 1100 - (Date.now() - start)),
        );
      });
  }, [provider]);

  // The spinner hold above outlives a fast refresh, so drop it when the panel goes away rather than
  // letting it set state on an unmounted component.
  useEffect(
    () => () => {
      if (spinHoldRef.current) clearTimeout(spinHoldRef.current);
    },
    [],
  );

  // Codex writes its final rate-limit snapshot just after the turn-end event, and a finished turn is
  // exactly when the numbers moved. Reconcile once after that short write window instead of leaving the
  // panel stale until the next poll, which is five minutes away by default.
  useEffect(() => {
    const previous = previousAgentStateRef.current;
    previousAgentStateRef.current = agentState;
    if (!isCodex || previous !== "working" || agentState === "working") return;
    const t = setTimeout(() => {
      void usageRefresh("codex")
        .then((snap) => useTermStore.getState().setUsage(snap))
        .catch(() => {});
    }, 750);
    return () => clearTimeout(t);
  }, [agentState, isCodex]);

  // Grok signals.json updates as the turn progresses; reread context when tools/state change. Account
  // usage is untouched here — that stays with the backend poller.
  useEffect(() => {
    if (!isGrok || !session.agentSessionId) return;
    let cancelled = false;
    agentContextInfo(session.id)
      .then((c) => {
        if (!cancelled) setCtx(c);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [isGrok, session.id, session.agentSessionId, agentState, currentTool]);

  // `started` is the process start time. Leaf components own their one-second ticks so InfoTab does
  // not rerender as a whole every second.
  const startedAt = runtime?.startedAt;

  return (
    <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
      <div className="insp-section">
        <h4>{isAgent ? "Agent" : "Process"}</h4>
        <KV k="session" v={session.name} />
        <KV k="cwd" v={cwd || "—"} />
        <KV k="branch" v={branch || "—"} accent />
        {isAgent ? (
          <KV k="agent" v={runtime?.agent || session.kind} accent />
        ) : (
          <KV k="pid" v={runtime?.pid ?? "—"} />
        )}
        <KV
          k="started"
          v={startedAt ? new Date(startedAt).toLocaleTimeString(dateLocale()) : "—"}
        />
        <UptimeKV startedAt={startedAt} />
      </div>

      {isAgent && (
        <div className="insp-section">
          <h4>Model</h4>
          <KV k="model" v={ctx?.model ?? "—"} accent />
          <KV
            k="context"
            v={
              ctx?.contextTokens != null
                ? `${fmtTokens(ctx.contextTokens)} / ${fmtTokens(ctx.contextLimit)}`
                : "—"
            }
          />
          <KV k="tool" v={currentTool ?? ctx?.currentTool ?? "—"} accent />
        </div>
      )}

      {provider && (
        <UsageSection
          kind={session.kind}
          usageAt={entry?.fetchedAt ?? null}
          usageBusy={usageBusy}
          error={entry?.error ?? null}
          errorAt={entry?.errorAt ?? null}
          refreshSec={usage?.auto ? usage.intervalSec : 0}
          spinTick={spinTick}
          onRefresh={refreshUsage}
        >
          {rows ?? <UsageHint busy={usageBusy} err={entry?.error ?? null} />}
        </UsageSection>
      )}

      {turn && (
        <div className="insp-section">
          <h4>This turn</h4>
          <KV k="tokens" v={fmtTokens(turn.tokens)} />
          <KV k="tools used" v={turn.toolsUsed} />
          <KV k="files touched" v={turn.filesTouched} accent />
        </div>
      )}

      <ResourcesSection pid={runtime?.pid} />
    </div>
  );
}
