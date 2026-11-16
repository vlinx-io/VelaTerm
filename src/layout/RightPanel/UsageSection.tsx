//! Account quota for the session's provider: one row per limit window, each with a percentage, a slim
//! meter, and the next reset. The backend keeps a single snapshot for the whole machine (see usage.rs),
//! so the container here only reads it, asks for a manual refresh, and renders.
//!
//! Layout split: `UsagePanel` owns store reads and polling side effects; `UsageSection` is presentational
//! and takes the rows as data, which keeps the design testable without a live backend.

import { useCallback, useEffect, useRef, useState } from "react";
import Icons from "../../components/Icons";
import { usageBrandIconEl } from "../../components/brandIcons";
import { dateLocale, useT } from "../../i18n";
import {
  usageRefresh,
  type UsageProvider,
  type UsageSnapshot,
} from "../../ipc/commands";
import { useTermStore } from "../../store/termStore";
import type { SessionKind } from "../../types";
import { kindIconEl } from "../sessionViewers/sessionMeta";
import { Section } from "./parts";
import "./usage-section.css";

/** One limit window reduced to the values the rows render. */
export interface QuotaRow {
  /** Stable key for React: provider data can repeat labels across groups. */
  key: string;
  /** Raw label from the provider, such as `5h · gemini` or `7d`. */
  label: string;
  /** Used percentage, 0-100. */
  pct: number;
  /** ISO 8601 string or Unix seconds, whichever the provider reports. */
  reset: string | number | null | undefined;
}

/** `ok` below 70%, `warn` from 70%, `crit` from 90%. */
type Tone = "ok" | "warn" | "crit";

function toneFor(pct: number): Tone {
  if (pct >= 90) return "crit";
  if (pct >= 70) return "warn";
  return "ok";
}

/** Normalize Claude/Antigravity ISO strings and Codex Unix seconds to Date, returning null for invalid values. */
function toResetDate(at: string | number | null | undefined): Date | null {
  if (at == null || at === "") return null;
  const d = typeof at === "number" ? new Date(at * 1000) : new Date(at);
  return Number.isNaN(d.getTime()) ? null : d;
}

/** Format a reset time as local 24-hour HH:mm, adding M/D when it falls on another day. */
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
export function quotaRows(provider: UsageProvider, snap: UsageSnapshot | null): QuotaRow[] | null {
  if (!snap) return null;
  if (provider === "claude") {
    const u = snap.claude.data;
    if (!u || (!u.fiveHour && !u.sevenDay)) return null;
    const rows: QuotaRow[] = [];
    if (u.fiveHour) rows.push({ key: "5h", label: "5h", pct: u.fiveHour.utilization, reset: u.fiveHour.resetsAt });
    if (u.sevenDay) rows.push({ key: "7d", label: "7d", pct: u.sevenDay.utilization, reset: u.sevenDay.resetsAt });
    if (u.sevenDayOpus) {
      rows.push({ key: "7d-opus", label: "7d · opus", pct: u.sevenDayOpus.utilization, reset: u.sevenDayOpus.resetsAt });
    }
    (u.modelWeekly ?? []).forEach((w, i) => {
      const scope = modelShortName(w.model);
      if (u.sevenDayOpus && scope === "opus") return;
      rows.push({ key: `7d-${scope}-${i}`, label: `7d · ${scope}`, pct: w.utilization, reset: w.resetsAt });
    });
    return rows;
  }
  if (provider === "codex") {
    const u = snap.codex.data;
    if (!u || (!u.primary && !u.secondary)) return null;
    const rows: QuotaRow[] = [];
    if (u.primary) {
      rows.push({
        key: "primary",
        label: codexWindowLabel(u.primary.windowMinutes),
        pct: u.primary.usedPercent,
        reset: u.primary.resetsAt,
      });
    }
    if (u.secondary) {
      rows.push({
        key: "secondary",
        label: codexWindowLabel(u.secondary.windowMinutes),
        pct: u.secondary.usedPercent,
        reset: u.secondary.resetsAt,
      });
    }
    return rows;
  }
  if (provider === "zcode" || provider === "antigravity") {
    const u = snap[provider]?.data;
    if (!u || u.windows.length === 0) return null;
    return u.windows.map((w, i) => ({
      key: `${provider}-${i}-${w.label}`,
      label: w.label,
      pct: w.usedPercent,
      reset: w.resetsAt,
    }));
  }
  const u = snap.grok.data;
  if (!u) return null;
  const rows: QuotaRow[] = [{ key: "pool", label: u.windowLabel || "7d", pct: u.usedPercent, reset: u.periodEnd }];
  if (u.buildPercent != null) rows.push({ key: "build", label: "build", pct: u.buildPercent, reset: null });
  return rows;
}

/** Display label for each quota source. */
function providerLabel(kind: SessionKind): string {
  if (kind === "codex") return "Codex";
  if (kind === "grok") return "Grok";
  if (kind === "zcode") return "ZCode";
  if (kind === "antigravity") return "Antigravity";
  return "Claude";
}

/**
 * Split a quota row label (`"<window> · <scope>"` or `"<window>"`) into its window key and scope.
 */
function splitQuotaLabel(label: string): { window: string; scope: string } {
  const cut = label.indexOf(" · ");
  if (cut >= 0) {
    return { window: label.slice(0, cut), scope: label.slice(cut + 3) };
  }
  return { window: label, scope: "" };
}

/** Match Antigravity's Gemini quota buckets (`gemini`, `gemini 4 argon`) while excluding `claude/gpt`. */
function isGeminiScope(row: QuotaRow): boolean {
  const { scope } = splitQuotaLabel(row.label);
  return /^gemini\b/i.test(scope.trim());
}

interface QuotaGroup {
  window: string;
  rows: Array<{ row: QuotaRow; scope: string }>;
}

/**
 * Group quota rows by their time window (`5h`, `7d`, ...) in first-seen order when any row carries a
 * model/scope suffix; otherwise return null so plain 2-row providers render flat rows without the rail.
 */
function groupQuotaRows(rows: QuotaRow[]): QuotaGroup[] | null {
  const parsed = rows.map((row) => ({ row, ...splitQuotaLabel(row.label) }));
  if (!parsed.some((p) => p.scope !== "")) return null;
  const map = new Map<string, QuotaGroup>();
  for (const item of parsed) {
    let group = map.get(item.window);
    if (!group) {
      group = { window: item.window, rows: [] };
      map.set(item.window, group);
    }
    group.rows.push({ row: item.row, scope: item.scope || "all" });
  }
  return Array.from(map.values());
}

/**
 * Single compact quota row: optional 22px window rail, flexible scope label, optional inline
 * reset timestamp, fixed-width slim bar, and tabular percentage.
 */
function QuotaLine({
  row,
  label,
  windowLabel,
  showWinRail = false,
}: {
  row: QuotaRow;
  label?: string;
  windowLabel?: string;
  showWinRail?: boolean;
}) {
  const pct = Math.max(0, Math.min(100, Math.round(row.pct)));
  const tone = toneFor(pct);
  const displayLabel = label ?? row.label;
  const target = toResetDate(row.reset);
  const resetStr = target ? fmtAbsTime(target) : null;
  const tip = target
    ? `${row.label} — ${pct}% used (resets ${target.toLocaleString(dateLocale())})`
    : `${row.label} — ${pct}% used`;
  return (
    <div className={`usg-row ${tone}${pct === 0 ? " zero" : ""}`} title={tip}>
      {showWinRail && (
        <span className="usg-win" aria-hidden={!windowLabel || undefined}>
          {windowLabel}
        </span>
      )}
      <span className={showWinRail ? "usg-scope" : "usg-scope is-flat"}>{displayLabel}</span>
      {resetStr && <span className="usg-reset">{resetStr}</span>}
      <span
        className={`usg-meter ${tone}`}
        role="progressbar"
        aria-valuenow={pct}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label={row.label}
      >
        <i style={{ width: `${pct}%` }} />
      </span>
      <span className="usg-pct" aria-label={`${pct}% used`}>
        {pct}%
      </span>
    </div>
  );
}

/** Render either window-railed groups separated by hairline rules or flat single-line rows. */
function QuotaList({ rows }: { rows: QuotaRow[] }) {
  const groups = groupQuotaRows(rows);
  if (!groups) {
    return (
      <div className="usg-list">
        {rows.map((r) => (
          <QuotaLine key={r.key} row={r} />
        ))}
      </div>
    );
  }
  return (
    <div className="usg-groups">
      {groups.map((g) => (
        <div key={g.window} className="usg-group">
          {g.rows.map(({ row, scope }, idx) => (
            <QuotaLine
              key={row.key}
              row={row}
              label={scope}
              windowLabel={idx === 0 ? g.window : ""}
              showWinRail
            />
          ))}
        </div>
      ))}
    </div>
  );
}

/** Placeholder for a quota the backend has not read yet or could not read, with the full error on hover. */
function UsageHint({ busy, err }: { busy: boolean; err: string | null }) {
  if (busy) return <div className="usg-empty">loading…</div>;
  if (!err) return <div className="usg-empty">—</div>;
  return (
    <div className="usg-empty" title={err}>
      <span className="usg-empty-key">unavailable</span>
      <span className="usg-empty-err">{err}</span>
    </div>
  );
}

/** Props for the presentational section, so a preview can render it without the store or backend. */
export interface UsageSectionProps {
  kind: SessionKind;
  rows: QuotaRow[] | null;
  usageAt: number | null;
  error: string | null;
  errorAt: number | null;
  refreshSec: number;
  busy: boolean;
  spinTick: number;
  onRefresh: () => void;
  /** Optional expansion toggle for non-Gemini Antigravity quota buckets (hidden by default). */
  showOtherScopes?: boolean;
  onShowOtherScopesChange?: (show: boolean) => void;
}

/**
 * Quota section with a Usage title, the branded source matching sidebar colors, a refresh countdown, and
 * a manual refresh button. Keep the last-updated time in a tooltip to preserve a compact header.
 *
 * `error` set while rows are showing means the latest poll failed and the numbers are from `usageAt`;
 * a stale marker says so, with the reason on hover, instead of letting old numbers pass as current.
 */
export function UsageSection({
  kind,
  rows,
  usageAt,
  error,
  errorAt,
  refreshSec,
  busy,
  spinTick,
  onRefresh,
  showOtherScopes = false,
  onShowOtherScopesChange,
}: UsageSectionProps) {
  const t = useT();
  const otherRows =
    kind === "antigravity" && rows != null ? rows.filter((row) => !isGeminiScope(row)) : [];
  const geminiRows =
    kind === "antigravity" && rows != null ? rows.filter(isGeminiScope) : [];
  // Collapse non-Gemini buckets (such as `claude/gpt`) by default only when Gemini rows exist, so an
  // account without Gemini quotas never collapses down to an empty list.
  const canToggleOtherScopes =
    kind === "antigravity" &&
    onShowOtherScopesChange != null &&
    geminiRows.length > 0 &&
    otherRows.length > 0;
  const visibleRows = rows && canToggleOtherScopes && !showOtherScopes ? geminiRows : rows;
  const otherLabel =
    Array.from(
      new Set(otherRows.map((row) => splitQuotaLabel(row.label).scope).filter(Boolean)),
    ).join(", ") || "claude/gpt";
  const stale = error != null && usageAt != null;
  // Keep the one-second countdown tick local so only this section rerenders, and only run the clock while
  // a countdown is actually shown: with auto-refresh off or a stale reading it would tick for nothing.
  const now = useNowTick(refreshSec > 0 && !stale);
  // Count down from the last attempt, not the last success: a failing poller still retries on schedule.
  const lastAttempt = Math.max(usageAt ?? 0, errorAt ?? 0) || null;
  const nextLeft =
    lastAttempt != null && refreshSec > 0 ? Math.max(0, lastAttempt + refreshSec * 1000 - now) : null;
  const tip = usageAt != null ? `Updated ${fmtUpdatedAt(usageAt)}` : t("common.refresh");
  const staleTip = stale
    ? `Refresh failed${errorAt != null ? ` at ${fmtUpdatedAt(errorAt)}` : ""}: ${error}\nShowing reading from ${fmtUpdatedAt(usageAt)}`
    : "";
  return (
    <Section
      id="usage"
      title="Usage"
      className="usg-section"
      actions={
        <>
          <span className="usg-provider">
            {usageBrandIconEl(kind, 11) ?? kindIconEl(kind, 11)}
            <span className="usg-provider-name">{providerLabel(kind)}</span>
          </span>
          <span className="usg-head-meta">
            {stale && (
              <span className="usg-stale" title={staleTip}>
                <i aria-hidden="true" />
                stale
              </span>
            )}
            {/* Stale data already explains itself; countdown shows pure numeric time without a duplicate icon. */}
            {!stale && nextLeft != null && (
              <span className="usg-countdown" title={tip}>
                {fmtRefreshLeft(nextLeft)}
              </span>
            )}
            <button
              type="button"
              className="usg-refresh"
              title={tip}
              aria-label={t("common.refresh")}
              onClick={onRefresh}
              disabled={busy}
            >
              <span key={spinTick} className={busy ? "spin" : undefined}>
                <Icons.restart size={11} aria-hidden="true" />
              </span>
            </button>
          </span>
        </>
      }
    >
      {visibleRows ? (
        <>
          <QuotaList rows={visibleRows} />
          {canToggleOtherScopes && (
            <button
              type="button"
              className="usg-more"
              aria-expanded={showOtherScopes}
              onClick={() => onShowOtherScopesChange!(!showOtherScopes)}
            >
              {showOtherScopes ? (
                <Icons.chevD size={11} aria-hidden="true" />
              ) : (
                <Icons.chevR size={11} aria-hidden="true" />
              )}
              <span>
                {showOtherScopes
                  ? t("panel.usageHideScope", otherLabel)
                  : t("panel.usageShowScope", otherLabel)}
              </span>
            </button>
          )}
        </>
      ) : (
        <UsageHint busy={busy} err={error} />
      )}
    </Section>
  );
}

/** One-second clock tick for leaf components, limiting rerenders to the small regions that display it. */
function useNowTick(enabled: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!enabled) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [enabled]);
  return now;
}

/** Store-backed wrapper: reads the machine-wide snapshot, refreshes one provider, and reconciles Codex. */
export function UsagePanel({
  sessionId,
  kind,
  provider,
}: {
  sessionId: string;
  kind: SessionKind;
  provider: UsageProvider;
}) {
  const usage = useTermStore((s) => s.usage);
  const entry = usage ? usage[provider] : null;
  const rows = quotaRows(provider, usage);
  const agentState = useTermStore((s) => s.runtimes[sessionId]?.agentState);
  const showOtherScopes = useTermStore((s) => s.usageAntigravityShowOther);
  const setShowOtherScopes = useTermStore((s) => s.setUsageAntigravityShowOther);
  const [busy, setBusy] = useState(false);
  // Increment on refresh to remount the spinner and reliably restart its animation.
  const [spinTick, setSpinTick] = useState(0);
  const previousAgentStateRef = useRef(agentState);
  const spinHoldRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Manual refresh: ask the backend to refetch this one provider now and bypass its short TTL cache. The
  // updated snapshot reaches every client through `usage://changed`, this one included.
  const refreshUsage = useCallback(() => {
    setSpinTick((n) => n + 1);
    setBusy(true);
    // Keep the spinner visible for at least two rotations: a cached answer can return in milliseconds,
    // which would otherwise look like nothing happened.
    const start = Date.now();
    void usageRefresh(provider, true)
      .then((snap) => useTermStore.getState().setUsage(snap))
      .catch(() => {})
      .finally(() => {
        if (spinHoldRef.current) clearTimeout(spinHoldRef.current);
        spinHoldRef.current = setTimeout(
          () => setBusy(false),
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
    if (provider !== "codex" || previous !== "working" || agentState === "working") return;
    const t = setTimeout(() => {
      void usageRefresh("codex")
        .then((snap) => useTermStore.getState().setUsage(snap))
        .catch(() => {});
    }, 750);
    return () => clearTimeout(t);
  }, [agentState, provider]);

  return (
    <UsageSection
      kind={kind}
      rows={rows}
      usageAt={entry?.fetchedAt ?? null}
      error={entry?.error ?? null}
      errorAt={entry?.errorAt ?? null}
      refreshSec={usage?.auto ? usage.intervalSec : 0}
      busy={busy}
      spinTick={spinTick}
      onRefresh={refreshUsage}
      showOtherScopes={provider === "antigravity" ? showOtherScopes : undefined}
      onShowOtherScopesChange={provider === "antigravity" ? setShowOtherScopes : undefined}
    />
  );
}
