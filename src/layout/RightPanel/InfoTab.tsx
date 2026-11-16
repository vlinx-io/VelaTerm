//! Right-side Info tab: basic session details and uptime, the provider quota section, current-turn
//! statistics, and a Resources section at the bottom holding this session's process-tree CPU/memory
//! alongside whole-machine CPU, memory, and saturation. Extracted from RightPanel; the shared KV row
//! lives in parts, and the quota rows with their own layout live in UsageSection.

import { useEffect, useState } from "react";
import { fmtBytes } from "../../format";
import { dateLocale } from "../../i18n";
import { useGitBranch } from "../../hooks/useGitBranch";
import { type UsageProvider } from "../../ipc/commands";
import { processStats, systemStats, type ProcStats, type SystemStats } from "../../ipc/info";
import { type Session } from "../../types";
import { useTermStore } from "../../store/termStore";
import { KV, Section } from "./parts";
import { TurnSection } from "./TurnSection";
import { KiroHistoryLink } from "../KiroHistory/navigation";
import { UsagePanel } from "./UsageSection";

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

/**
 * Start time and uptime share one row: the clock reads when the process began, the elapsed time
 * beside it says how long ago that was. Own the one-second tick here so only this KV rerenders.
 */
function StartedKV({ startedAt }: { startedAt: number | undefined }) {
  const now = useNowTick();
  return (
    <KV
      k="started"
      v={
        startedAt ? (
          <>
            {new Date(startedAt).toLocaleTimeString(dateLocale())}
            <span className="kv-sub">{fmtUptime(now - startedAt)}</span>
          </>
        ) : (
          "—"
        )
      }
    />
  );
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

/** Whole-machine rows. The backend supplies every saturation signal available on the host: macOS sends
 * memory pressure and load averages, while Linux sends load averages. The UI does not sniff the platform. */
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
      // Nobody sees the numbers while the window is hidden; sampling resumes the moment it shows again.
      if (document.hidden) return;
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
    document.addEventListener("visibilitychange", sample);
    return () => {
      cancelled = true;
      clearInterval(t);
      document.removeEventListener("visibilitychange", sample);
    };
  }, [pid, showSystem]);

  return (
    <Section
      id="resources"
      title="Resources"
      actions={
        <label className="res-toggle" title="Show whole-machine CPU, memory, and saturation">
          <input
            type="checkbox"
            checked={showSystem}
            onChange={(e) => setShowSystem(e.target.checked)}
          />
          system
        </label>
      }
    >
      <div className="res-sub">this session</div>
      <KV k="cpu" v={stats ? `${stats.cpu.toFixed(1)}%` : "—"} />
      <KV k="memory" v={stats ? fmtBytes(stats.rssBytes) : "—"} />
      {showSystem && (
        <>
          <div className="res-sub">system</div>
          <SystemRows sys={sys} />
        </>
      )}
    </Section>
  );
}


export function InfoTab({ session, cwd }: { session: Session; cwd: string | null }) {
  const runtime = useTermStore((s) => s.runtimes[session.id]);
  const branch = useGitBranch(cwd);
  const isAgent = session.kind !== "terminal";
  const isClaude = session.kind === "claude";
  const isCodex = session.kind === "codex";
  const isGrok = session.kind === "grok";
  const isOpencode = session.kind === "opencode";
  const isPi = session.kind === "pi" || session.kind === "omp";
  // Every agent whose backend records turn usage belongs here; DeepSeek and Antigravity read their
  // own native recordings and have no quota row to reveal the gate.
  const hasContext = isClaude || isCodex || isGrok || isOpencode || isPi
    || session.kind === "kiro" || session.kind === "zcode" || session.kind === "deepseek"
    || session.kind === "antigravity";

  const agentState = runtime?.agentState;
  const currentTool = runtime?.currentTool;

  // Account-level quotas come from the backend's single stored snapshot: one poller per machine keeps it
  // fresh and broadcasts changes, so this panel only reads it. Which entry to show follows the session kind.
  // UsagePanel reads the machine-wide snapshot itself: which provider to show still follows the session
  // kind, but the polling, refresh state, and Codex reconciliation live with the section.
  const provider: UsageProvider | null = isClaude
    ? "claude"
    : isCodex
      ? "codex"
      : isGrok
        ? "grok"
        : session.kind === "zcode"
          ? "zcode"
          : session.kind === "antigravity"
            ? "antigravity"
            : null;

  // `started` is the process start time. Leaf components own their one-second ticks so InfoTab does
  // not rerender as a whole every second.
  const startedAt = runtime?.startedAt;

  return (
    <div className="info-sections">
      {/* Which agent this is belongs with the section title rather than in a row of its own: it
          names the whole group below, the way "Process" is qualified by its pid. */}
      <Section
        id="agent"
        title={isAgent ? "Agent" : "Process"}
        tag={isAgent ? runtime?.agent || session.kind : (runtime?.pid ?? "—")}
      >
        <KV k="session" v={session.name} />
        <KV k="cwd" v={cwd || "—"} />
        <KV k="branch" v={branch || "—"} accent />
        <StartedKV startedAt={startedAt} />
        {session.kind === "kiro" && <KiroHistoryLink sessionId={session.id} />}
      </Section>

      {provider && <UsagePanel sessionId={session.id} kind={session.kind} provider={provider} />}

      {hasContext && <TurnSection key={session.id} session={session} cwd={cwd} agentState={agentState} currentTool={currentTool} />}

      <ResourcesSection pid={runtime?.pid} />
    </div>
  );
}
