import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { memoryCancel, memoryJobs, memoryOptions, memoryRetry, memoryStart, type MemoryJob } from "../../ipc/memory";
import { useTermStore } from "../../store/termStore";
import { MemoryLink, memoryNavigate, memoryUrl, useMemoryLocation } from "./navigation";
import { LoadState, StateLabel, memoryError, memoryTime, useMemoryLoad } from "./shared";

export function MemoryCompile({ sessionId }: { sessionId: string }) {
  const t = useT(); const session = useTermStore((s) => [...s.sessions, ...s.archivedSessions].find((item) => item.id === sessionId));
  const { data, error, reload } = useMemoryLoad(memoryOptions, []);
  const [agent, setAgent] = useState(""); const [model, setModel] = useState("");
  const [busy, setBusy] = useState(false); const [failure, setFailure] = useState("");
  useEffect(() => { if (data) setAgent(data.defaultAgent); }, [data]);
  if (!data) return <LoadState error={error} reload={reload} />;
  const start = async () => {
    setBusy(true); setFailure("");
    try { const job = await memoryStart(sessionId, agent, model.trim()); memoryNavigate(memoryUrl(`job/${job.id}`)); }
    catch (e) { setFailure(memoryError(e)); }
    finally { setBusy(false); }
  };
  return <form className="memory-document memory-editor" onSubmit={(e) => { e.preventDefault(); void start(); }}>
    <h2>{t("memory.add")}</h2><p className="memory-lead">{session?.name ?? sessionId}</p><p>{t("memory.compileHelp")}</p>
    <label>{t("memory.selectAgent")}<select className="input" value={agent} disabled={busy} onChange={(e) => setAgent(e.target.value)}>
      {data.agents.map((item) => <option key={item.id} value={item.id} disabled={!item.available}>{item.label}{!item.available ? ` · ${t("memory.unavailable")}` : ""}</option>)}
    </select></label>
    <label>{t("memory.model")}<input className="input" value={model} disabled={busy} maxLength={200} onChange={(e) => setModel(e.target.value)} placeholder={t("memory.modelHint")} /></label>
    {failure && <p className="memory-error" role="alert">{failure}</p>}
    <p className="memory-muted">{t("memory.closeHint")}</p>
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
        <div className="memory-row"><MemoryLink route={`job/${job.id}`}><strong>{job.sessionName}</strong></MemoryLink><span className="memory-spacer" /><span className={`memory-status ${job.status}`}><StateLabel value={job.status} /></span></div>
        <p className="memory-muted">{job.agent === "claude" ? "Claude" : "Codex"}{job.model ? ` · ${job.model}` : ""} · {memoryTime(job.createdAt)}</p>
        {job.status === "running" && <><p role="status"><StateLabel value={job.stage} /> · {job.progress} / {job.total || "…"}</p><progress max={job.total || 1} value={job.progress} /></>}
        {job.error && <p className="memory-error">{memoryError(job.error)}</p>}
        {job.status === "completed" && !job.entries.length && <p>{t("memory.noKnowledge")}</p>}
        <div className="memory-job-entries">{job.entries.map((entryId, index) => <MemoryLink key={entryId} className="btn" route={`entry/${entryId}`}>{t("memory.entries")} {index + 1} ↗</MemoryLink>)}</div>
        <footer className="memory-row"><MemoryLink route={`source/${job.sourceId}`}>{t("memory.source")}</MemoryLink><span className="memory-spacer" />
          {job.status === "running" && <button className="btn" disabled={busy === job.id} onClick={() => void command(job, true)}>{t("common.cancel")}</button>}
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
