import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import AgentSelect from "../../components/AgentSelect";
import { LaunchLoadState, ModelEffortFields, useLaunchOptions } from "../../components/LaunchFields";
import { memoryCancel, memoryJobs, memoryOptions, memoryRetry, memoryStart, type MemoryJob, type MemoryOptions } from "../../ipc/memory";
import { useTermStore } from "../../store/termStore";
import type { SessionKind } from "../../types";
import { MemoryLink, memoryNavigate, memoryUrl, useMemoryLocation } from "./navigation";
import { LoadState, StateLabel, memoryError, memoryEffortLabel, memoryTime, useMemoryLoad } from "./shared";

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
  const [data, setData] = useState<{ jobs: MemoryJob[]; total: number; pageSize: number } | null>(null);
  const [error, setError] = useState(""); const [revision, setRevision] = useState(0); const [busy, setBusy] = useState("");
  useEffect(() => {
    let alive = true; let timer: ReturnType<typeof setTimeout>;
    setData(null); setError("");
    const refresh = async () => {
      try {
        const result = await memoryJobs(id, id ? 0 : page);
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
  return <section className="memory-document">
    <div className="memory-row"><h2>{t("memory.jobs")}</h2><span className="memory-spacer" /><button className="btn" onClick={() => setRevision((v) => v + 1)}>{t("common.refresh")}</button></div>
    {error && <p className="memory-error" role="alert">{error}</p>}
    {!data ? <LoadState error={error} reload={() => setRevision((v) => v + 1)} /> : <>
      {!data.jobs.length && <p>{t(id ? "memory.notFound" : "memory.emptyJobs")}</p>}
      {data.jobs.map((job) => <article className="memory-job" key={job.id}>
        <div className="memory-row"><MemoryLink route={job.entryLinks.length === 1 ? `entry/${job.entryLinks[0].id}` : `job/${job.id}`}><strong>{job.sessionName}</strong></MemoryLink><span className="memory-spacer" /><span className={`memory-status ${job.status}`}><StateLabel value={job.status} /></span></div>
        <p className="memory-muted">{job.agentLabel || job.agent}{` · ${job.model || t("chat.modelDefault")}`} · {t("chat.effortTooltip")}: {job.effort ? memoryEffortLabel(job.effort) : t("spawn.modelDefault")} · {memoryTime(job.createdAt)}</p>
        {job.status === "running" && <><p role="status"><StateLabel value={job.stage} /> · {job.progress} / {job.total || "…"}</p><progress max={job.total || 1} value={job.progress} /></>}
        {job.status === "queued" && <p className="memory-muted" role="status">{t("memory.waitingHint")}</p>}
        {job.error && <p className="memory-error">{memoryError(job.error)}</p>}
        {job.status === "completed" && !job.entries.length && <p>{t("memory.noKnowledge")}</p>}
        <div className="memory-job-entries">{job.entryLinks.map((entry) => <MemoryLink key={entry.id} className="btn" title={entry.title} route={`entry/${entry.id}`}>{entry.title} ↗</MemoryLink>)}</div>
        <footer className="memory-row"><MemoryLink route={`source/${job.sourceId}`}>{t("memory.source")}</MemoryLink><span className="memory-spacer" />
          {["queued", "running"].includes(job.status) && <button className="btn" disabled={busy === job.id} onClick={() => void command(job, true)}>{t("common.cancel")}</button>}
          {["failed", "cancelled"].includes(job.status) && <button className="btn" disabled={busy === job.id} onClick={() => void command(job, false)}>{t("common.retry")}</button>}
        </footer>
      </article>)}
      {!id && data.total > data.pageSize && <footer className="memory-pagination">
        {page > 0 ? <MemoryLink route="jobs" values={{ memoryJobPage: page - 1 }}>{t("common.prev")}</MemoryLink> : <span />}
        <span>{page + 1} / {Math.ceil(data.total / data.pageSize)}</span>
        {(page + 1) * data.pageSize < data.total && <MemoryLink route="jobs" values={{ memoryJobPage: page + 1 }}>{t("common.next")}</MemoryLink>}
      </footer>}
    </>}
  </section>;
}
