import { useEffect, useRef, useState } from "react";
import { Backdrop } from "../../components/Backdrop";
import AgentSelect from "../../components/AgentSelect";
import { LaunchField, LaunchLoadState, ModelEffortFields, useLaunchOptions } from "../../components/LaunchFields";
import { useT } from "../../i18n";
import { auditCancel, auditExport, auditGet, auditList, auditOptions, auditStart, type AuditOptions, type AuditRun, type AuditSummary } from "../../ipc/security";
import { writeTextFile } from "../../ipc/info";
import { env, platform } from "../../platform";
import { useTermStore } from "../../store/termStore";
import { ChatPane } from "../CenterPane/session/ChatPane";
import { Markdown } from "../CenterPane/session/markdown";
import { memoryNavigate, useMemoryLocation } from "../Memory/navigation";
import type { SessionKind } from "../../types";
import { AuditLink, securityUrl } from "./navigation";
import { securityText as s } from "./text";
import { auditError } from "./errors";
import "./security.css";

function Shield() {
  return <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true"><path d="M12 3 4 6v6c0 4 4 7 8 9 4-2 8-5 8-9V6Z"/><path d="m8 12 3 3 5-6"/></svg>;
}
function Status({ value }: { value: string }) {
  return <span className={`security-status ${value}`}><i aria-hidden="true"/>{s(value)}</span>;
}
function Prose({ text }: { text: string }) {
  return <div className="security-prose"><Markdown text={text}/></div>;
}
function Configuration({ run }: { run: AuditSummary }) {
  const t = useT();
  return <span className="security-config-line">{run.model || t("chat.modelDefault")} · {run.effort ? effortLabel(run.effort, t) : t("chat.effort.auto")}</span>;
}
export function SecurityRoute() {
  const location = useMemoryLocation();
  const params = new URLSearchParams(location);
  const projectId = params.get("security");
  return projectId ? <SecuritySurface key={projectId} projectId={projectId} params={params}/> : null;
}
function SecuritySurface({ projectId, params }: { projectId: string; params: URLSearchParams }) {
  const t = useT();
  const ref = useRef<HTMLElement>(null);
  const [history, setHistory] = useState<AuditSummary[] | null>(null);
  const [error, setError] = useState("");
  const [revision, setRevision] = useState(0);
  const name = useTermStore((state) => state.projects.find((p) => p.id === projectId)?.name);
  const runId = params.get("securityRun");
  const isNew = params.get("securityNew") === "1";
  const close = () => memoryNavigate(securityUrl(""));
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    return () => previous?.focus();
  }, []);
  useEffect(() => { ref.current?.focus(); }, [runId, isNew]);
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const load = async () => {
      try {
        const list = await auditList(projectId);
        if (active) { setHistory(list); setError(""); timer = setTimeout(() => void load(), 2500); }
      } catch (e) { if (active) setError(String(e)); }
    };
    void load();
    return () => { active = false; clearTimeout(timer); };
  }, [projectId, revision]);
  return <Backdrop onClose={close}>
    <section ref={ref} tabIndex={-1} className="security-shell" role="dialog" aria-modal="true" aria-labelledby="security-heading" onKeyDown={(e) => {
      if (e.key === "Escape") { e.stopPropagation(); close(); }
      if (e.key === "Tab") {
        const nodes = [...(ref.current?.querySelectorAll<HTMLElement>('a[href],button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),summary') ?? [])].filter((n) => n.getClientRects().length);
        if (nodes.length && ((!e.shiftKey && (document.activeElement === nodes.at(-1) || document.activeElement === ref.current)) || (e.shiftKey && (document.activeElement === nodes[0] || document.activeElement === ref.current)))) {
          e.preventDefault(); (e.shiftKey ? nodes.at(-1) : nodes[0])?.focus();
        }
      }
    }}>
      <header className="security-header">
        <span className="security-mark"><Shield/></span>
        <div className="security-heading"><h2 id="security-heading">{s("title")}</h2><p>{name ?? projectId}</p></div>
        <AuditLink className="btn security-close" projectId="" aria-label={t("common.close")}>×</AuditLink>
      </header>
      <div className={`security-body${isNew || runId ? " has-detail" : ""}`}>
        <aside className="security-history">
          <AuditLink projectId={projectId} values={{ securityNew: "1", securityRun: null, securityTab: null, securityFinding: null }} className="btn btn-primary">＋ {s("new")}</AuditLink>
          <div className="security-history-heading"><h3>{s("history")}</h3><span>{history?.length ?? "—"}</span></div>
          {error && <div className="security-error" role="alert">{auditError(error)}<button className="btn" onClick={() => setRevision((v) => v + 1)}>{t("common.retry")}</button></div>}
          {!history && !error && <p role="status" className="security-muted">{t("common.loading")}</p>}
          {history?.length === 0 && <p className="security-history-empty">{s("empty")}</p>}
          <div className="security-history-list">{history?.map((run) => <AuditLink key={run.id} projectId={projectId} values={{ securityRun: run.id, securityNew: null, securityTab: null, securityFinding: null }} aria-current={runId === run.id && !isNew ? "page" : undefined} className={`security-history-item${runId === run.id && !isNew ? " active" : ""}`}>
            <div><strong>{run.agentLabel}</strong><Status value={run.status}/></div>
            <Configuration run={run}/>
            <small>{s(run.scope === "path" ? "pathScope" : run.scope)}{run.path && ` · ${run.path}`}</small>
            <time dateTime={new Date(run.createdAt).toISOString()}>{new Date(run.createdAt).toLocaleString()}</time>
          </AuditLink>)}</div>
        </aside>
        <main className="security-main">
          <AuditLink className="security-mobile-back" projectId={projectId} values={{ securityRun: null, securityNew: null, securityTab: null, securityFinding: null }}>← {s("history")}</AuditLink>
          {isNew ? <NewAudit projectId={projectId}/> : runId ? <AuditDetail key={runId} id={runId} projectId={projectId} params={params}/> : <div className="security-welcome">
            <span className="security-welcome-mark"><Shield/></span><h2>{s("title")}</h2><p>{s("intro")}</p>
            <WorkflowPreview/>
            <p>{s("config")}</p><AuditLink className="btn btn-primary" projectId={projectId} values={{ securityNew: "1" }}>＋ {s("new")}</AuditLink>
        <p className="security-footnote">{s("upstreamNote")}</p>
          </div>}
        </main>
      </div>
    </section>
  </Backdrop>;
}

function WorkflowPreview() {
  const [options, setOptions] = useState<AuditOptions | null>(null);
  const [error, setError] = useState("");
  useEffect(() => { let active = true; void auditOptions().then(value => { if (active) setOptions(value); }).catch(error => { if (active) setError(String(error)); }); return () => { active = false; }; }, []);
  if (error) return <p className="security-error" role="alert">{auditError(error)}</p>;
  if (!options) return null;
  return <><p className="security-muted">{options.workflow.package}@{options.workflow.packageVersion}</p><div className="security-workflow-preview">{options.workflow.phases.map((phase, index) => <div key={phase}><span>{String(index + 1).padStart(2, "0")}</span><strong>{s(phase)}</strong></div>)}</div></>;
}

function UpstreamReport({run}: {run: AuditRun}) {
  const upstream = run.upstream!;
  const progress = upstream.scan.progress.phaseProgress;
  return <div className="security-upstream">
    {upstream.artifactReadError && <div className="security-error" role="alert">{s("error")}<code>{upstream.artifactReadError.split(":")[0]}</code></div>}
    <section className="security-report-section security-provenance">
      <strong>Codex Security · {upstream.pluginVersion}</strong>
      <small>{upstream.package}@{upstream.packageVersion}</small>
      <code>{upstream.scan.scanId}</code>
    </section>
    {!!upstream.scan.warnings?.length && <section className="security-report-section"><h3>{s("gaps")}</h3>{upstream.scan.warnings.map((warning, index) => <p key={index}>{warning}</p>)}</section>}
    {upstream.report ? <section className="security-report-section"><Prose text={upstream.report}/></section> : <section className="security-report-section">
      <h3>{s(run.phase)}</h3>
      {progress.total > 0 && <><progress aria-label={s(run.phase)} value={progress.completed} max={progress.total}/><p>{progress.completed} / {progress.total} · {s(progress.unit ?? "coverage")}</p></>}
      <p>{s("checkpoint")}</p>
      <p>{s("findings")} · {upstream.scan.findingCount}</p>
      {upstream.scan.progress.preflightIssues?.map((issue, index) => <p key={index}>{issue.reason}</p>)}
    </section>}
  </div>;
}
function UpstreamSummary({run, findingId}: {run: AuditRun; findingId: string | null}) {
  const upstream = run.upstream!;
  const findings = upstream.findings?.findings ?? [];
  return <div className="security-upstream">
    <section className="security-report-section security-provenance"><strong>Codex Security · {upstream.pluginVersion}</strong><small>{upstream.package}@{upstream.packageVersion}</small></section>
    <div className="security-metrics"><div><span>{s("findings")}</span><strong>{findings.length}</strong></div><div><span>{s("coverage")}</span><strong className="security-coverage-label">{s(upstream.evidenceError ? "failed" : (upstream.coverageComplete ?? upstream.coverage?.completeness === "complete") ? "completed" : "partial")}</strong></div><div><span>{s("gaps")}</span><strong>{(upstream.coverage?.deferred.length ?? 0) + (upstream.scan.warnings?.length ?? 0) + (upstream.evidenceError ? 1 : 0)}</strong></div></div>
    {!!upstream.scan.warnings?.length && <section className="security-report-section"><h3>{s("gaps")}</h3>{upstream.scan.warnings.map((warning,index)=><p key={index}>{warning}</p>)}</section>}
    {!!upstream.coverage?.deferred.length && <section className="security-report-section"><h3>{s("gaps")}</h3><ul>{upstream.coverage.deferred.map((gap,index)=><li key={index}>{gap.reason}</li>)}</ul></section>}
    <section className="security-report-section"><div className="security-section-title"><h3>{s("findings")}</h3><span>{findings.length}</span></div>
      {!findings.length && <div className="security-empty-result"><Shield/><p>{s("noFindings")}</p><small>{s(run.status)}</small></div>}
      {findings.map(finding => <article key={finding.findingId} className={`security-finding ${finding.severity.level}`}>
        <AuditLink projectId={run.projectId} values={{securityFinding: findingId === finding.findingId ? null : finding.findingId}} aria-expanded={findingId === finding.findingId} aria-controls={`evidence-${finding.findingId}`}>
          <div className="security-finding-labels"><span className={`security-severity ${finding.severity.level}`}>{s(finding.severity.level)}</span><span className="security-disclosure" aria-hidden="true">{findingId === finding.findingId ? "−" : "+"}</span></div><strong>{finding.title}</strong><span className="security-finding-summary">{finding.summary}</span>
        </AuditLink>
        {findingId === finding.findingId && <div id={`evidence-${finding.findingId}`} className="security-finding-content">
          <dl>{finding.attackPath?.summary && <div><dt>{s("attack_path")}</dt><dd><Prose text={finding.attackPath.summary}/></dd></div>}{!!finding.attackPath?.preconditions?.length && <div><dt>{s("preconditions")}</dt><dd><ul>{finding.attackPath.preconditions.map((text,index)=><li key={index}>{text}</li>)}</ul></dd></div>}<div><dt>{s("impact")}</dt><dd><Prose text={finding.severity.rationale}/></dd></div>{(finding.validation?.method || finding.validation?.summary) && <div><dt>{s("validation")}</dt><dd>{finding.validation.method && <Prose text={finding.validation.method}/>} {finding.validation.summary && <Prose text={finding.validation.summary}/>}</dd></div>}<div><dt>{s("recommendation")}</dt><dd><Prose text={finding.remediation}/></dd></div></dl>
          {!!finding.attackPath?.limitations?.length && <><h4>{s("gaps")}</h4><ul>{finding.attackPath.limitations.map((text,index)=><li key={index}>{text}</li>)}</ul></>}
          <h4>{s("evidence")}</h4><div className="security-evidence-list">{finding.codeEvidence?.map((evidence,index)=><div key={index}><small>{evidence.path}:{evidence.startLine}–{evidence.endLine ?? evidence.startLine}</small><pre>{evidence.code.split("\n").map((line,index)=><div key={index}><span aria-hidden="true">{evidence.startLine+index}</span><code>{line}</code></div>)}</pre></div>)}</div>
        </div>}
      </article>)}
    </section>
    <AuditLink className="btn" projectId={run.projectId} values={{securityTab:"report_artifacts"}}>{s("report_artifacts")} →</AuditLink>
    <p className="security-footnote">{s("upstreamNote")}</p>
  </div>;
}

function NewAudit({ projectId }: { projectId: string }) {
  const t = useT();
  const [options, setOptions] = useState<AuditOptions | null>(null);
  const [agent, setAgent] = useState("");
  const [scope, setScope] = useState("");
  const [path, setPath] = useState("");
  const [model, setModel] = useState("");
  const [effort, setEffort] = useState("");
  const launch = useLaunchOptions(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    let active = true;
    void auditOptions().then((v) => { if (active) { setOptions(v); setAgent(v.defaultAgent); setScope(v.defaultScope); setError(""); } }).catch((e) => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, [revision]);
  // The launch catalogue supplies the agent's model and effort fields; the audit list only says which
  // agents can run an audit at all.
  const spec = launch.options.find((o) => o.id === agent);
  return <form className="security-new" onSubmit={(e) => {
    e.preventDefault(); if (!options || busy || !spec) return;
    setBusy(true); setError("");
    void auditStart({ projectId, agent, scope, path: scope === "path" ? path.trim() : "", model, effort }).then((run) => memoryNavigate(securityUrl(projectId, { securityRun: run.id, securityNew: null, securityTab: null, securityFinding: null }))).catch((e) => setError(String(e))).finally(() => setBusy(false));
  }}>
    <div className="security-page-heading"><h2>{s("new")}</h2><p>{s("config")}</p></div>
    {error && <div className="security-error" role="alert">{auditError(error)}{!options && <button type="button" className="btn" onClick={() => setRevision((v) => v + 1)}>{t("common.retry")}</button>}</div>}
    {!options && !error && <p role="status">{t("common.loading")}</p>}
    {options && <>
      <p className="security-muted">{options.workflow.package}@{options.workflow.packageVersion} · Codex Security {options.workflow.pluginVersion}</p>
      <fieldset className="security-form-section"><legend><span>01</span>{s("agent")}</legend>
        <div className="launch-model-grid">
          <LaunchField label={t("orch.agentLabel")}>
            <AgentSelect value={agent as SessionKind | ""} disabled={busy}
              options={options.agents.map((a) => ({ id: a.id as SessionKind, label: a.label }))}
              onChange={(next) => { if (next !== agent) { setAgent(next); setModel(""); setEffort(""); } }} />
          </LaunchField>
        </div>
        {launch.state !== "ready" && <LaunchLoadState state={launch.state} error={launch.error} retry={launch.retry} />}
        {spec && <div className="launch-model-grid">
          <ModelEffortFields spec={spec} model={model} effort={effort} disabled={busy} context={{ inheritArgs: true }}
            onChange={(nextModel, nextEffort) => { setModel(nextModel); setEffort(nextEffort); }} />
        </div>}
      </fieldset>
      <fieldset className="security-form-section"><legend><span>02</span>{s("scope")}</legend>
        <div className="security-scope-options">{options.scopes.map((value) => <label key={value} className={scope === value ? "selected" : ""}><input type="radio" name="audit-scope" value={value} checked={scope === value} disabled={busy} onChange={() => setScope(value)}/>{s(value === "path" ? "pathScope" : value)}</label>)}</div>
        {scope === "path" && <label className="security-path-field">{s("path")}<input required maxLength={4096} placeholder="src" value={path} disabled={busy} onChange={(e) => setPath(e.target.value)}/></label>}
      </fieldset>
      <div className="security-form-footer"><p>{s("upstreamNote")}</p><button type="submit" className="btn btn-primary" disabled={busy || !spec}>{busy ? t("common.loading") : s("start")} <span aria-hidden="true">→</span></button></div>
    </>}
  </form>;
}
function effortLabel(value: string, t: ReturnType<typeof useT>) {
  const key = `chat.effort.${value}`;
  const translated = t(key as Parameters<typeof t>[0]);
  return translated === key ? value : translated;
}
function AuditDetail({ id, projectId, params }: { id: string; projectId: string; params: URLSearchParams }) {
  const t = useT();
  const [run, setRun] = useState<AuditRun | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [revision, setRevision] = useState(0);
  const tab = params.get("securityTab") ?? "report";
  const findingId = params.get("securityFinding");
  const session = useTermStore((state) => state.sessions.find((item) => item.id === run?.sessionId));
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const load = async () => {
      try {
        const value = await auditGet(id);
        if (value.projectId !== projectId) throw new Error("security_not_found");
        if (active) { setRun(value); setError(""); if (["running", "waiting"].includes(value.status)) timer = setTimeout(() => void load(), 1500); }
      } catch (e) { if (active) setError(String(e)); }
    };
    void load();
    return () => { active = false; clearTimeout(timer); };
  }, [id, projectId, revision]);
  useEffect(() => { if (run?.sessionId && !session) void useTermStore.getState().loadTree().catch(() => {}); }, [run?.sessionId, session]);
  const perform = async (fn: () => Promise<unknown>) => { setBusy(true); setError(""); try { await fn(); setRevision((v) => v + 1); } catch (e) { setError(String(e)); } finally { setBusy(false); } };
  const download = async (format: "md" | "json") => {
    const data = await auditExport(id);
    const text = format === "md" ? data.markdown : JSON.stringify(data.run, null, 2);
    const name = `security-${id}.${format}`;
    if (!env.isBrowser) { const dest = await platform.dialog.saveFile({ defaultPath: name, filters: [{ name: format === "md" ? "Markdown" : "JSON", extensions: [format] }] }); if (dest) await writeTextFile(dest, text, null); return; }
    const url = URL.createObjectURL(new Blob([text], { type: format === "md" ? "text/markdown;charset=utf-8" : "application/json" }));
    const a = document.createElement("a"); a.href = url; a.download = name; document.body.appendChild(a); a.click(); a.remove(); setTimeout(() => URL.revokeObjectURL(url), 1000);
  };
  const active = run && ["running", "waiting"].includes(run.status);
  const gaps = run ? [...run.gaps, ...run.files.filter((f) => !run.reviewed.includes(f.path)).map((f) => f.path)] : [];
  return <div className="security-detail">
    {error && <div className="security-error" role="alert">{auditError(error)}<button className="btn" onClick={() => setRevision((v) => v + 1)}>{t("common.retry")}</button></div>}
    {!run && !error && <p className="security-loading" role="status">{t("common.loading")}</p>}
    {run && <>
      <div className="security-toolbar"><div className="security-run-heading"><div><h2>{run.agentLabel}</h2><Status value={run.status}/></div><p>{run.model || t("chat.modelDefault")} · {run.effort ? effortLabel(run.effort, t) : t("chat.effort.auto")}</p><small>{s(run.scope === "path" ? "pathScope" : run.scope)}{run.path && ` · ${run.path}`} · {new Date(run.createdAt).toLocaleString()}</small></div>
        <div className="security-actions">{active ? <button className="btn" disabled={busy || !!error} onClick={() => void perform(() => auditCancel(id))}>{t("common.cancel")}</button> : <button className="btn" disabled={busy || !!error} onClick={() => void perform(async () => { const next = await auditStart({ projectId, agent: run.agent, scope: run.scope, path: run.path, model: run.model ?? "", effort: run.effort ?? "" }); memoryNavigate(securityUrl(projectId, { securityRun: next.id, securityTab: null, securityFinding: null })); })}>{s("start")}</button>}
          <details className="security-export"><summary className="btn">↓ {s("export")}</summary><div><button disabled={busy} onClick={(e) => { e.currentTarget.closest("details")?.removeAttribute("open"); void perform(() => download("md")); }}>Markdown</button><button disabled={busy} onClick={(e) => { e.currentTarget.closest("details")?.removeAttribute("open"); void perform(() => download("json")); }}>JSON</button></div></details>
        </div>
      </div>
      <nav className="security-tabs" aria-label={s("report")}>{(run.upstream?.report ? ["report", "report_artifacts", "conversation"] : ["report", "conversation"]).map((value) => <AuditLink key={value} projectId={projectId} values={{ securityTab: value }} aria-current={tab === value ? "page" : undefined} className={tab === value ? "active" : ""}>{s(value)}</AuditLink>)}</nav>
      {run.status === "waiting" && <div className="security-notice" role="status"><p>{s("waitingHint")}</p><AuditLink className="btn" projectId={projectId} values={{ securityTab: "conversation" }}>{s("conversation")} →</AuditLink></div>}
      {tab === "conversation" ? <div className="security-chat">{session ? <ChatPane session={session} cwd={run.root} area={{ position: "absolute", inset: 0 }} hidden={false} focused={true} multi={false} readOnly={!!run.upstream} onActivate={() => {}} onSplit={() => {}} onClose={() => memoryNavigate(securityUrl(projectId, { securityTab: "report" }))}/> : <p className="security-loading">{s("missingSession")}</p>}</div> : <div className="security-report">
        {run.error && <div className="security-error" role="alert"><div>{auditError(run.error)}{run.upstream?.failureDetail && <Prose text={run.upstream.failureDetail}/>}<code>{run.error.split(":")[0]}</code></div><AuditLink projectId={projectId} values={{ securityTab: "conversation" }}>{s("conversation")} →</AuditLink></div>}
        {active && <div className="security-progress-state" role="status"><span className="security-pulse"/>{s(run.phase)}</div>}
        {run.upstream ? (run.upstream.report && tab !== "report_artifacts" ? <UpstreamSummary run={run} findingId={findingId}/> : <UpstreamReport run={run}/>) : <><div className="security-notice">{s("legacy")}</div><div className="security-metrics"><div><span>{s("coverage")}</span><strong>{run.reviewed.length}<small> / {run.files.length}</small></strong><progress aria-label={s("reviewed")} value={run.reviewed.length} max={Math.max(1, run.files.length)}/><small>{s("reviewed")}</small></div><div><span>{s("findings")}</span><strong>{run.findings.filter((f) => f.verdict === "validated").length}</strong><small>{s("validated")}</small></div><div><span>{s("candidate")}</span><strong>{run.findings.filter((f) => f.verdict === "candidate").length}</strong><small>{s("findings")}</small></div></div>
        <section className="security-report-section"><div className="security-section-title"><h3>{s("findings")}</h3><span>{run.findings.length}</span></div>
          {!run.findings.length && <div className="security-empty-result"><Shield/><p>{active ? s(run.phase) : s("noFindings")}</p><small>{s(run.status)}</small></div>}
          {run.findings.map((f) => <article key={f.id} className={`security-finding ${f.severity}`}><AuditLink projectId={projectId} values={{ securityFinding: findingId === f.id ? null : f.id }} aria-expanded={findingId === f.id} aria-controls={`evidence-${f.id}`}><div className="security-finding-labels"><span className={`security-severity ${f.severity}`}>{s(f.severity)}</span><span>{s(f.verdict)}</span><span className="security-disclosure" aria-hidden="true">{findingId === f.id ? "−" : "+"}</span></div><strong>{f.title}</strong><small>{f.path}:{f.line}–{f.endLine}</small></AuditLink>
            {findingId === f.id && <div id={`evidence-${f.id}`} className="security-finding-content"><dl>{(["source", "sink", "preconditions", "impact", "recommendation"] as const).map((key) => <div key={key}><dt>{s(key)}</dt><dd><Prose text={f[key]}/></dd></div>)}</dl><h4>{s("evidence")}</h4><pre>{f.evidence.split("\n").map((line, i) => <div key={i}><span aria-hidden="true">{f.line + i}</span><code>{line}</code></div>)}</pre></div>}
          </article>)}
        </section>
        {run.threatModel && <section className="security-report-section"><details className="security-section-details"><summary>{s("threat")}</summary><Prose text={run.threatModel}/></details></section>}
        {(gaps.length > 0 || run.excluded.length > 0) && <section className="security-report-section"><div className="security-section-title"><h3>{s("gaps")}</h3><span>{gaps.length + run.excluded.length}</span></div>{gaps.length > 0 && <ul>{gaps.map((gap, index) => <li key={index}>{gap.startsWith("security_") ? auditError(gap) : gap}</li>)}</ul>}{!!run.excluded.length && <details><summary>{s("excluded")} · {run.excluded.length}</summary><ul>{run.excluded.map((file) => <li key={file}>{file}</li>)}</ul></details>}</section>}
        {!!run.steps.length && <section className="security-report-section security-steps">{run.steps.map((step, index) => <details key={step.id}><summary><span className="security-step-number">{String(index + 1).padStart(2, "0")}</span><span>{s(step.phase)}</span><small>{(step.durationMs / 1000).toFixed(1)} s</small></summary><Prose text={step.summary}/></details>)}</section>}
        <p className="security-footnote">{s("note")}</p></>}
      </div>}
    </>}
  </div>;
}
