//! Turn accounting is owned by the backend. This component only formats the current snapshot.
import { useEffect, useRef, useState } from "react";
import Icons from "../../components/Icons";
import { fmtTokens } from "../../format";
import { useT } from "../../i18n";
import { agentTurnStats, gitChangedFiles, type AgentTurnStats, type TurnChangedFile } from "../../ipc/commands";
import { getGitStatus } from "../../ipc/info";
import { detectFlavor, isAbsolutePath, joinPath } from "../../remote/serverPath";
import { useTermStore } from "../../store/termStore";
import type { Session } from "../../types";
import { fileIcon } from "./fileIcons";
import { Section } from "./parts";
import "./turn-section.css";

const tokens = (value: number | null | undefined) => value == null ? "—" : fmtTokens(value);
const contextTokens = (value: number | null | undefined) => value != null && value >= 1000 && value < 1_000_000
  ? `${(value / 1000).toFixed(1)}k` : tokens(value);

/** Where a recorded path lives: the absolute path to open, plus the name and the muted folder to show. */
function changedFile(path: string, cwd: string | null | undefined) {
  const flavor = detectFlavor(cwd || path);
  const absolute = isAbsolutePath(path, flavor) ? path : cwd ? joinPath(cwd, path, flavor) : null;
  let shown = path;
  if (cwd && absolute) {
    const root = cwd.replace(/[\\/]+$/, "");
    const rest = absolute.slice(root.length);
    if (absolute.startsWith(root) && /^[\\/]/.test(rest)) shown = rest.slice(1);
  }
  const cut = Math.max(shown.lastIndexOf("/"), shown.lastIndexOf("\\"));
  return { path, absolute, shown, name: shown.slice(cut + 1), dir: cut > 0 ? shown.slice(0, cut) : "" };
}

/** One row per file, in the order shown. An agent may name the same file absolutely and relative to
 *  the session folder within one turn, so rows merge on the resolved path and their counts add up. */
function fileRows(files: TurnChangedFile[], cwd: string | null | undefined) {
  const rows = new Map<string, ReturnType<typeof changedFile> & { additions: number | null; deletions: number | null }>();
  for (const f of files) {
    const file = changedFile(f.path, cwd);
    const key = file.absolute ?? file.path;
    const seen = rows.get(key);
    if (!seen) rows.set(key, { ...file, additions: f.additions, deletions: f.deletions });
    else if (seen.additions == null || seen.deletions == null || f.additions == null || f.deletions == null) {
      seen.additions = null;
      seen.deletions = null;
    } else {
      seen.additions += f.additions;
      seen.deletions += f.deletions;
    }
  }
  return [...rows.values()].sort((a, b) => a.shown < b.shown ? -1 : a.shown > b.shown ? 1 : 0);
}

/** Show a changed file's uncommitted changes in a diff tab. A file outside a repository, or with
 *  nothing left to compare because it has since been committed or reverted, opens in the editor. */
async function openChangedFile(absolute: string) {
  const { openDiffTab, openDocTab } = useTermStore.getState();
  try {
    const windows = detectFlavor(absolute) === "win";
    // Git reports forward slashes even on Windows, where letter case does not tell paths apart either.
    const norm = (p: string) => {
      const slashed = p.replace(/\\/g, "/").replace(/\/+$/, "");
      return windows ? slashed.toLowerCase() : slashed;
    };
    const cut = Math.max(absolute.lastIndexOf("/"), absolute.lastIndexOf("\\"));
    const status = await getGitStatus(cut > 0 ? absolute.slice(0, cut) : absolute);
    const root = status.isRepo ? status.root : null;
    if (root && norm(absolute).startsWith(norm(root) + "/")) {
      const rel = norm(absolute).slice(norm(root).length + 1);
      const { files } = await gitChangedFiles(root);
      const file = files.find((f) => norm(f.path) === rel);
      if (file) { openDiffTab(absolute, root, file.path); return; }
    }
  } catch {
    // Without Git the file itself is still worth opening.
  }
  openDocTab(absolute);
}

/** Added and removed lines, or nothing when they are not known. */
function LineCounts({ additions, deletions }: { additions: number | null; deletions: number | null }) {
  if (additions == null || deletions == null) return null;
  return <span className="turn-lines"><span className="plus">+{additions}</span><span className="minus">−{deletions}</span></span>;
}

/** The turn's changed files, in place of the statistics until the user goes back. */
function ChangedFiles({ files, cwd, onBack }: { files: TurnChangedFile[]; cwd?: string | null; onBack: () => void }) {
  const t = useT();
  const rows = fileRows(files, cwd);
  // Totals only when every file is countable, so a partial sum never passes for the whole turn.
  const counted = rows.length > 0 && rows.every((f) => f.additions != null && f.deletions != null);
  const sum = (key: "additions" | "deletions") => rows.reduce((n, f) => n + (f[key] ?? 0), 0);
  return <div className="turn-files">
    <div className="turn-files-bar">
      <button type="button" className="turn-back" onClick={onBack}>
        <Icons.chevR size={11} /><span>{t("panel.turnStats")}</span>
      </button>
      <span className="turn-files-count">
        <span>{t("panel.fileCount", rows.length)}</span>
        {counted && <LineCounts additions={sum("additions")} deletions={sum("deletions")} />}
      </span>
    </div>
    {rows.length === 0
      ? <p className="turn-files-empty">{t("panel.noFilesChanged")}</p>
      : <ul className="turn-file-list">
        {rows.map((file) => {
          const spec = fileIcon(file.name, false);
          const FileIcon = Icons[spec.icon];
          return <li key={file.absolute ?? file.path}>
            <button type="button" className="turn-file" title={file.absolute ?? file.path}
              disabled={!file.absolute} onClick={() => file.absolute && void openChangedFile(file.absolute)}>
              <span className="turn-file-icon" style={{ color: spec.color }}><FileIcon size={14} /></span>
              <span className="turn-file-name">{file.name}</span>
              <span className="turn-file-dir">{file.dir}</span>
              <LineCounts additions={file.additions} deletions={file.deletions} />
            </button>
          </li>;
        })}
      </ul>}
    <p className="turn-status">{t("panel.shellEditsNotListed")}</p>
  </div>;
}

export function TurnStatsView({ stats, state = "ready", cwd }: {
  stats: AgentTurnStats | null;
  state?: "loading" | "waiting" | "error" | "ready";
  /** Session folder: shortens the changed-file paths and resolves relative ones. */
  cwd?: string | null;
}) {
  const t = useT();
  const filesView = useTermStore((s) => s.turnFilesView);
  const setFilesView = useTermStore((s) => s.setTurnFilesView);
  const files = stats?.changedFiles;
  const percent = stats?.contextPercent;
  const tone = percent != null && percent >= 90 ? "crit" : percent != null && percent >= 70 ? "warn" : "ok";
  const context = `${contextTokens(stats?.contextTokens)} / ${contextTokens(stats?.contextLimit)}`;
  const rate = stats?.generationTokensPerSecond;
  const model = stats?.model;
  // Each group renders only when it has something to say, so an agent that reports nothing shows the
  // note alone instead of a column of dashes.
  const hasContext = stats?.contextTokens != null || stats?.contextLimit != null || percent != null;
  const hasTurn = stats?.inputTokens != null || stats?.tokens != null || stats?.totalTokens != null
    || stats?.cacheHitPercent != null || rate != null || stats?.filesTouched != null;
  const hasSession = stats?.sessionTokens != null;
  const hasData = model != null || hasContext || hasTurn || hasSession;
  // The remembered choice yields to the statistics while there is no list to show, such as before the
  // first reading or for an agent that does not record its edits.
  if (filesView && files) {
    return <Section id="turn" title="This turn" className="turn-section" ariaLabel="This turn" busy={state === "loading"}>
      <ChangedFiles files={files} cwd={cwd} onBack={() => setFilesView(false)} />
    </Section>;
  }
  return <Section id="turn" title="This turn" className="turn-section" ariaLabel="This turn" busy={state === "loading"}>
    {model != null && (
      <div className="kv" title="Model named by the most recent context reading from this session.">
        <span className="k">model</span><span className="v">{model}</span>
      </div>
    )}
    {hasContext && (
      <div className={`turn-context ${tone}`}
        title="Tokens currently in the model context / context capacity. This is not cumulative token usage.">
        <div className="kv">
          <span className="k">context</span>
          <span className="v">
            {context}
            {percent != null && <span className="turn-pill">{Math.round(percent)}%</span>}
          </span>
        </div>
        {percent != null && (
          <div className="turn-meter" role="progressbar"
            aria-label="Context usage" aria-valuemin={0} aria-valuemax={100}
            aria-valuenow={Math.min(100, Math.max(0, percent))}
            aria-valuetext={context}>
            <i style={{ width: `${Math.min(100, Math.max(0, percent))}%` }} />
          </div>
        )}
      </div>
    )}
    {hasTurn && (
      <div className="turn-rows">
        <div className="turn-divider" />
        <div className="turn-metrics">
          <div className="turn-metric" title="Input plus output across model requests in this turn. Repeated context counts each time; cached input is already included.">
            <span className="m-k">turn tokens</span><span className="m-v">{tokens(stats?.totalTokens)}</span>
          </div>
          <div className="turn-metric" title="Input tokens, including cache reads and writes.">
            <span className="m-k">input</span><span className="m-v">{tokens(stats?.inputTokens)}</span>
          </div>
          <div className="turn-metric" title="Generated output tokens, including reasoning when reported by the agent.">
            <span className="m-k">output</span><span className="m-v">{tokens(stats?.tokens)}</span>
          </div>
          <div className="turn-metric" title={stats?.cachedTokens == null ? "Cached input divided by total input for this turn." : `${stats.cachedTokens.toLocaleString()} cached input tokens. Cache reads / total input for this turn.`}>
            <span className="m-k">cache hit</span>
            <span className="m-v">{stats?.cacheHitPercent == null ? "—" : `${stats.cacheHitPercent.toFixed(1)}%`}</span>
          </div>
          <div className="turn-metric" title="Estimated output tokens per second over measured response time, including reported reasoning. Excludes tool execution and user waits. Unavailable when usage and timing cannot be matched reliably. This is not pure model decoding speed.">
            <span className="m-k">average output</span>
            <span className="m-v">{rate == null ? "—" : `${rate.toFixed(1)} tok/s`}</span>
          </div>
          <div className="turn-metric" title="Distinct paths from successful, explicitly recorded file edits. Shell edits and child-agent changes may not be reported.">
            <span className="m-k">files changed</span>
            {files
              ? <button type="button" className="m-v turn-files-link" title={t("panel.showChangedFiles")}
                aria-label={t("panel.showChangedFiles")} onClick={() => setFilesView(true)}>
                {fileRows(files, cwd).length}<Icons.chevR size={10} />
              </button>
              : <span className="m-v">{stats?.filesTouched == null ? "—" : String(stats.filesTouched)}</span>}
          </div>
        </div>
      </div>
    )}
    {hasSession && (
      <div className="turn-rows" title="Cumulative input plus output for this recorded main session. Cached input is included; independent child-agent sessions are excluded.">
        <div className="turn-divider" />
        <div className="kv">
          <span className="k">session total</span>
          <span className="v turn-strong">{tokens(stats?.sessionTokens)}<small>tokens</small></span>
        </div>
      </div>
    )}
    {!hasData && <p className="turn-status" role="status">{
      state === "loading" ? "Loading statistics…" : state === "waiting" ? "Statistics appear after the first turn." :
      state === "error" ? "Statistics are temporarily unavailable." : "This agent has not reported usage statistics."
    }</p>}
  </Section>;
}

export function TurnSection({ session, cwd, agentState, currentTool }: {
  session: Session;
  /** Effective session folder, falling back to the project root like the rest of the Info panel. */
  cwd?: string | null;
  agentState?: string | null; currentTool?: string | null;
}) {
  const [stats, setStats] = useState<AgentTurnStats | null>(null);
  const [state, setState] = useState<"loading" | "waiting" | "error" | "ready">("loading");
  const previous = useRef({ identity: "", agentState });
  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const identity = `${session.id}:${session.agentSessionId ?? ""}`;
    const reset = previous.current.identity !== identity || (agentState === "working" && previous.current.agentState !== "working");
    previous.current = { identity, agentState };
    if (reset) { setStats(null); setState("loading"); }
    if (!session.agentSessionId) { setState("waiting"); return; }
    const sample = async (settled = false) => {
      try {
        const next = await agentTurnStats(session.id);
        if (!cancelled) { setStats(next); setState("ready"); }
      } catch {
        if (!cancelled) { setStats(null); setState("error"); }
      }
      // Serialize reads. A final delayed read absorbs the provider's turn-end disk flush.
      if (!cancelled && (agentState === "working" || !settled)) {
        timer = setTimeout(() => { void sample(true); }, agentState === "working" ? 1500 : 750);
      }
    };
    void sample();
    return () => { cancelled = true; clearTimeout(timer); };
  }, [session.id, session.agentSessionId, agentState, currentTool]);
  return <TurnStatsView stats={stats} state={state} cwd={cwd ?? session.cwd} />;
}
