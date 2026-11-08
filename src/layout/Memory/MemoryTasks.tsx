import { useEffect, useState } from "react";
import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import AgentSelect from "../../components/AgentSelect";
import { LaunchLoadState, ModelEffortFields, useLaunchOptions } from "../../components/LaunchFields";
import { memoryCancel, memoryJobSessions, memoryJobs, memoryOptions, memoryRetry, memoryStart, type MemoryJob, type MemoryJobSessions, type MemoryOptions } from "../../ipc/memory";
import { useTermStore } from "../../store/termStore";
import type { SessionKind } from "../../types";
import { MemoryLink, memoryNavigate, memoryUrl, useMemoryLocation } from "./navigation";
import { KnowledgeBackLink } from "../Notebook/KnowledgeBack";
import { kindIconEl } from "../sessionViewers/sessionMeta";
import { LoadState, StateLabel, memoryError, memoryEffortLabel, runTime, useMemoryLoad } from "./shared";

/** The remembered agent when it is still installed, otherwise the backend's first available one. */
function rememberedAgent(data: MemoryOptions): string {
  const saved = useTermStore.getState().memoryPrefs.agent;
  if (saved && data.agents.some((item) => item.id === saved && item.available)) return saved;
  return data.defaultAgent;
}

export function MemoryCompile({ sessionId }: { sessionId: string }) {
  const t = useT(); const session = useTermStore((s) => [...s.sessions, ...s.archivedSessions].find((item) => item.id === sessionId));
  const loadArchived = useTermStore((s) => s.loadArchived);
  // A direct link to the organizer may arrive before the archived list is in the store; load it so an
  // archived session still shows its name instead of its id.
  useEffect(() => { if (!session) void loadArchived(); }, [session, loadArchived]);
  const { data, error, reload } = useMemoryLoad(memoryOptions, []);
  const [agent, setAgent] = useState(""); const [model, setModel] = useState("");
  const [effort, setEffort] = useState("");
  // The launch catalogue supplies the model and effort fields; the organizer only says which agents it runs.
  const launch = useLaunchOptions(true);
  const spec = launch.options.find((option) => option.id === agent);
  const [busy, setBusy] = useState(false); const [failure, setFailure] = useState("");
  const remember = (patch: { agent?: SessionKind | null; model?: string | null; effort?: string | null }) =>
    useTermStore.getState().setMemoryPrefs(patch);
  // Seed the remembered agent, and its model and effort when they were chosen for that same agent. A model
  // the catalogue no longer lists is kept, as every launch form keeps it; the agent's CLI decides.
  useEffect(() => {
    if (!data) return;
    const next = rememberedAgent(data);
    const prefs = useTermStore.getState().memoryPrefs;
    setAgent(next);
    setModel(prefs.agent === next ? prefs.model ?? "" : "");
    setEffort(prefs.agent === next ? prefs.effort ?? "" : "");
  }, [data]);
  if (!data) return <LoadState error={error} reload={reload} />;
  const start = async () => {
    setBusy(true); setFailure("");
    try { const job = await memoryStart(sessionId, agent, model, effort); memoryNavigate(memoryUrl(`job/${job.id}`)); }
    catch (e) { setFailure(memoryError(e)); }
    finally { setBusy(false); }
  };
  return <form className="memory-document memory-editor memory-compile" onSubmit={(e) => { e.preventDefault(); void start(); }}>
    <h2>{t("memory.add")}</h2><p className="memory-lead">{session?.name ?? sessionId}</p><p>{t("memory.compileHelp")}</p>
    <label>{t("memory.selectAgent")}<AgentSelect value={agent as SessionKind | ""} disabled={busy} options={data.agents}
      onChange={(value) => { setAgent(value); setModel(""); setEffort(""); remember({ agent: value || null, model: null, effort: null }); }} /></label>
    {spec ? <div className="launch-model-grid"><ModelEffortFields spec={spec} model={model} effort={effort} disabled={busy}
      onChange={(nextModel, nextEffort) => { setModel(nextModel); setEffort(nextEffort); }}
      onModelCommit={(value) => remember({ model: value || null })}
      onEffortCommit={(value) => remember({ effort: value || null })} /></div>
      : launch.state !== "ready" && <LaunchLoadState state={launch.state} error={launch.error} retry={launch.retry} />}
    {failure && <p className="memory-error" role="alert">{failure}</p>}
    <p className="memory-muted">{t("memory.schedulingHint")} {t("memory.closeHint")}</p>
    <footer className="memory-row"><MemoryLink route="" className="btn">{t("common.cancel")}</MemoryLink><span className="memory-spacer" /><button className="btn btn-primary" disabled={busy || !data.agents.some((item) => item.id === agent && item.available)}>{t(busy ? "common.loading" : "memory.compile")}</button></footer>
  </form>;
}

export function MemoryJobs({ id }: { id?: string }) {
  const t = useT(); const location = useMemoryLocation();
  const page = Math.max(0, Number(new URLSearchParams(location).get("memoryJobPage")) || 0);
  const [data, setData] = useState<MemoryJobSessions | null>(null);
  const [error, setError] = useState(""); const [revision, setRevision] = useState(0); const [busy, setBusy] = useState("");
  const [open, setOpen] = useState<Set<string>>(new Set());
  useEffect(() => {
    let alive = true; let timer: ReturnType<typeof setTimeout>;
    setData(null); setError("");
    const refresh = async () => {
      try {
        const result = id ? await memoryJobs(id, 0) : await memoryJobSessions(page);
        if (alive) { setData(result); setError(""); }
      } catch (e) { if (alive) { setError(memoryError(e)); setData(null); } }
      if (alive) timer = setTimeout(() => void refresh(), 2000);
    };
    void refresh(); return () => { alive = false; clearTimeout(timer); };
  }, [id, page, revision]);
  const command = async (job: MemoryJob, cancel: boolean) => {
    setBusy(job.id); setError("");
    try {
      if (cancel) { await memoryCancel(job.id); setRevision((v) => v + 1); }
      else { const result = await memoryRetry(job.id); memoryNavigate(memoryUrl(`job/${result.id}`)); }
    } catch (e) { setError(memoryError(e)); }
    finally { setBusy(""); }
  };
  const toggle = (key: string) => setOpen((old) => { const next = new Set(old); if (next.has(key)) next.delete(key); else next.add(key); return next; });
  const jobs = data?.jobs ?? [];
  return <div className="kbx-page kbx-narrow">
    <div className="kbx-bar">
      <KnowledgeBackLink className="kbx-ibtn" />
      <nav className="kbx-crumbs" aria-label={t("memory.hierarchy")}><MemoryLink route="library" values={{ memoryProject: null, memorySession: null }}>{t("memory.globalMemory")}</MemoryLink><Icons.chevR size={12} aria-hidden="true" /><b>{t("memory.jobs")}</b></nav>
      <div className="kbx-acts"><button type="button" className="kbx-ibtn" title={t("common.refresh")} aria-label={t("common.refresh")} onClick={() => setRevision((v) => v + 1)}><Icons.restart size={14} aria-hidden="true" /></button></div>
    </div>
    <div className="kbx-head closed"><div className="t">
      <div className="t1"><Icons.history size={16} aria-hidden="true" /><h1>{t("memory.jobs")}</h1></div>
      {data && !id && <div className="kbx-meta"><span>{t("archive.sessionsCount", String(data.total))}</span>{!!data.organized && <span>{t("kb.organizedCount", String(data.organized))}</span>}{!!data.attention && <span className="bad">{t("kb.needsAttention", String(data.attention))}</span>}</div>}
    </div></div>
    {error && <p className="memory-error" role="alert">{error}</p>}
    <div className="kbx-list">
      {!data ? <LoadState error={error} reload={() => setRevision((v) => v + 1)} /> : <>
        {!jobs.length && <p className="kbx-empty">{t(id ? "memory.notFound" : "memory.emptyJobs")}</p>}
        {jobs.length > 0 && <div>{jobs.map((job, index) => {
          const links = job.entryLinks;
          // The newest run opens with its entries listed; older runs keep the log compact until asked.
          const expanded = open.has(job.id);
          const bad = job.status === "failed" || job.status === "cancelled";
          return <article className="kbx-job" key={job.id}>
            <div className="a">
              <MemoryLink className="tt" route={links.length === 1 ? `entry/${links[0].id}` : `job/${job.id}`}>{job.sessionName}</MemoryLink>
              <div className="m">{kindIconEl(job.agent as SessionKind, 14)}<span>{id ? [job.agentLabel || job.agent, job.model || t("chat.modelDefault"), job.effort && memoryEffortLabel(job.effort)].filter(Boolean).join(" · ") : job.model || job.agentLabel || job.agent}</span></div>
              {job.status === "running" && <><p role="status" className="m"><StateLabel value={job.stage} /> · {job.progress} / {job.total || "…"}</p><progress max={job.total || 1} value={job.progress} /></>}
              {job.status === "queued" && <p className="m" role="status">{t("memory.waitingHint")}</p>}
              {(id || job.status === "failed") && job.error && <span className="e">{memoryError(job.error)}</span>}
              {!id && !!job.failedAttempts && (job.status === "completed" ? <span className="m">{t("kb.succeededAfter", String(job.failedAttempts))}</span> : job.failedAttempts > 1 && <span className="m">{t("kb.failedTimes", String(job.failedAttempts))}</span>)}
              {job.status === "completed" && !job.entries.length && <span className="m">{t("memory.noKnowledge")}</span>}
              {links.length > 0 && (index === 0 && !open.has(job.id)
                ? <div className="outs">
                  {links.slice(0, 3).map((entry) => <MemoryLink key={entry.id} className="o" title={entry.title} route={`entry/${entry.id}`}><Icons.diamond size={12} aria-hidden="true" />{entry.title}</MemoryLink>)}
                  {links.length > 3 && <button type="button" className="x" onClick={() => toggle(job.id)}>{t("kb.andMore", String(links.length - 3))}</button>}
                </div>
                : <>
                  <button type="button" className="x" aria-expanded={expanded} onClick={() => toggle(job.id)}>{expanded ? <Icons.chevD size={12} aria-hidden="true" /> : <Icons.chevR size={12} aria-hidden="true" />}{t("kb.entriesCount", String(links.length))}</button>
                  {expanded && <div className="outs">{links.map((entry) => <MemoryLink key={entry.id} className="o" title={entry.title} route={`entry/${entry.id}`}><Icons.diamond size={12} aria-hidden="true" />{entry.title}</MemoryLink>)}</div>}
                </>)}
            </div>
            <div className="b">
              {["queued", "running"].includes(job.status) && <button className="kbx-btn" disabled={busy === job.id} onClick={() => void command(job, true)}>{t("common.cancel")}</button>}
              {bad && <button className="kbx-btn" disabled={busy === job.id} onClick={() => void command(job, false)}><Icons.restart size={14} aria-hidden="true" />{t("common.retry")}</button>}
              <MemoryLink className="kbx-ibtn" route={`source/${job.sourceId}`} title={t("memory.source")} aria-label={t("memory.source")}><Icons.docLines size={14} aria-hidden="true" /></MemoryLink>
            </div>
            <div className="w"><span className={`kbx-st ${job.status === "completed" ? "ok" : bad ? "bad" : "run"}`}><i />{!id && job.status === "completed" ? <span>{t("kb.statusOrganized")}</span> : !id && job.status === "failed" ? <span>{t("kb.statusAttention")}</span> : <StateLabel value={job.status} />}</span><span>{runTime(job.createdAt)}</span></div>
          </article>;
        })}</div>}
        {!id && data.total > data.pageSize && <footer className="memory-pagination">
          {page > 0 ? <MemoryLink route="jobs" values={{ memoryJobPage: page - 1 }}>{t("common.prev")}</MemoryLink> : <span />}
          <span>{page + 1} / {Math.ceil(data.total / data.pageSize)}</span>
          {(page + 1) * data.pageSize < data.total && <MemoryLink route="jobs" values={{ memoryJobPage: page + 1 }}>{t("common.next")}</MemoryLink>}
        </footer>}
      </>}
    </div>
  </div>;
}
