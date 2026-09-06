import { useEffect, useRef, useState } from "react";
import { Backdrop } from "../../components/Backdrop";
import { useT } from "../../i18n";
import { useTermStore } from "../../store/termStore";
import { knowledgeDisable, knowledgeInstall, knowledgeList, knowledgeSearch, knowledgeStart, type KnowledgeIndex } from "../../ipc/knowledge";
import { MemoryLink, memoryNavigate } from "../Memory/navigation";
import { KnowledgeLink, knowledgeUrl, useKnowledgeLocation } from "./navigation";
import { knowledgeError, KnowledgeState, useKnowledgeLoad } from "./shared";
import { KnowledgeSymbol } from "./KnowledgeSymbol";
import "../Memory/memory.css";
import "./knowledge.css";

export function KnowledgeRoute() {
  const location = useKnowledgeLocation(); const params = new URLSearchParams(location);
  const projectId = params.get("knowledge");
  if (!projectId) return null;
  return <KnowledgeSurface key={projectId} projectId={projectId} params={params} />;
}
function KnowledgeSurface({ projectId, params }: { projectId: string; params: URLSearchParams }) {
  const t = useT(); const ref = useRef<HTMLElement>(null);
  const name = useTermStore((s) => s.projects.find((p) => p.id === projectId)?.name);
  const { data, error, reload } = useKnowledgeLoad(() => knowledgeList(projectId), [projectId]);
  const [failure,setFailure] = useState(""); const [busy,setBusy] = useState(false); const [revision,setRevision] = useState(0);
  const indexId = params.get("knowledgeIndex"); const nodeId = params.get("knowledgeNode");
  const index = data?.indexes.find((i) => i.id === indexId) ?? (!indexId ? data?.indexes[0] : undefined);
  const close = () => memoryNavigate(knowledgeUrl(""));
  useEffect(() => { const previous = document.activeElement as HTMLElement | null; ref.current?.focus(); return () => previous?.focus(); }, []);
  useEffect(() => {
    if (index && !indexId) memoryNavigate(knowledgeUrl(projectId,{ knowledgeIndex:index.id }),true);
  }, [index?.id,indexId,projectId]);
  const processing = data?.runtime.status === "installing" || data?.indexes.some((i) => ["indexing","syncing"].includes(i.status));
  useEffect(() => { if (!processing) return; const timer = setTimeout(reload,1500); return () => clearTimeout(timer); }, [processing,data]);
  const perform = async (fn: () => Promise<unknown>) => {
    setBusy(true);setFailure("");
    try { await fn(); reload(); setRevision((v) => v+1); } catch (e) { setFailure(knowledgeError(e)); } finally { setBusy(false); }
  };
  return <Backdrop onClose={close}><section ref={ref} className="memory-shell knowledge-shell" tabIndex={-1} role="dialog" aria-modal="true" aria-labelledby="knowledge-heading" onKeyDown={(e) => {
    if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); close(); }
    if (e.key === "Tab") {
      const items = [...(ref.current?.querySelectorAll<HTMLElement>('a[href],button:not(:disabled),input:not(:disabled),select:not(:disabled)') ?? [])].filter((n) => n.getClientRects().length);
      if (items.length && ((!e.shiftKey && document.activeElement === items.at(-1)) || (e.shiftKey && (document.activeElement === items[0] || document.activeElement === ref.current)))) { e.preventDefault(); (e.shiftKey ? items.at(-1) : items[0])?.focus(); }
    }
  }}>
    <header className="memory-header"><div className="memory-brand"><div><h2 id="knowledge-heading">{t("knowledge.title")} · {name ?? projectId}<span className="memory-badge">{t("common.experimental")}</span></h2><p>{t("knowledge.intro")}</p></div></div><MemoryLink route="library" className="btn">{t("memory.title")}</MemoryLink><KnowledgeLink projectId="" className="btn" aria-label={t("common.close")}>×</KnowledgeLink></header>
    {!data ? <div className="memory-empty" role={error ? "alert" : "status"}>{error || t("common.loading")}{error && <button className="btn" onClick={reload}>{t("common.retry")}</button>}</div> : <>
      {!data.runtime.available && <div className="knowledge-setup"><strong>CodeGraph {data.runtime.version}</strong><p>{t("knowledge.setup")}</p><p>{t("knowledge.downloadNotice")}</p><button className="btn btn-primary" disabled={busy || !data.runtime.supported || data.runtime.status === "installing"} onClick={() => void perform(knowledgeInstall)}>{t(data.runtime.status === "installing" ? "knowledge.installing" : "knowledge.install")}</button>{data.runtime.error && <p role="alert">{knowledgeError(data.runtime.error)}</p>}</div>}
      <nav className="knowledge-roots" aria-label={t("knowledge.directory")}>{data.indexes.map((item) => <KnowledgeLink key={item.id} projectId={projectId} values={{ knowledgeIndex:item.id, knowledgeNode:null, knowledgePage:null, knowledgeLink:null }} className={index?.id === item.id ? "active" : ""}>{item.root}</KnowledgeLink>)}</nav>
      {failure && <p className="knowledge-warning" role="alert">{failure}</p>}
      {index ? <>
        <div className="knowledge-toolbar"><KnowledgeState value={index.status} /><span>{t("knowledge.symbols")}: {index.stats.nodes ?? "—"} · {t("knowledge.files")}: {index.stats.files ?? "—"} · {t("knowledge.edges")}: {index.stats.edges ?? "—"}</span><span className="memory-spacer" />
          <button className="btn btn-primary" disabled={busy || !data.runtime.available || ["indexing","syncing"].includes(index.status)} onClick={() => void perform(() => knowledgeStart(index.id))}>{t(index.enabled ? "knowledge.sync" : "knowledge.enable")}</button>
          {index.enabled && <button className="btn" disabled={busy} onClick={() => void perform(() => knowledgeDisable(index.id))}>{t("knowledge.disable")}</button>}
        </div>
        {index.error && <p className="knowledge-warning" role="alert">{knowledgeError(index.error)}</p>}
        {!index.enabled ? <p className="memory-empty">{t("knowledge.disabledHelp")}</p> : ["indexing","syncing"].includes(index.status) ? <p className="memory-empty" role="status">{t("knowledge.busy")}</p> : index.status === "ready" ? <div className={`knowledge-body${nodeId ? " has-node" : ""}`}>
          <SymbolSearch index={index} projectId={projectId} params={params} revision={revision} />
          <main className="knowledge-main">{nodeId ? <KnowledgeSymbol key={`${index.id}/${nodeId}/${revision}`} index={index} nodeId={nodeId} linkEntry={params.get("knowledgeLink")} /> : <p className="memory-empty">{t("knowledge.selectSymbol")}</p>}</main>
        </div> : null}
      </> : <p className="memory-empty">{t("knowledge.directoryMissing")}</p>}
      <footer className="knowledge-footer">{t("knowledge.agentHint")}</footer>
    </>}
  </section></Backdrop>;
}
function SymbolSearch({ index, projectId, params, revision }: { index: KnowledgeIndex; projectId: string; params: URLSearchParams; revision: number }) {
  const t = useT(); const query = params.get("knowledgeQuery") ?? "";
  const page = Math.max(0,Number(params.get("knowledgePage")) || 0); const [draft,setDraft] = useState(query);
  useEffect(() => setDraft(query), [query]);
  const { data,error,reload } = useKnowledgeLoad(() => knowledgeSearch(index.id,query,page), [index.id,query,page,revision]);
  return <aside className="knowledge-sidebar"><form onSubmit={(e) => { e.preventDefault(); memoryNavigate(knowledgeUrl(projectId,{ knowledgeQuery:draft,knowledgePage:null,knowledgeNode:null,knowledgeLink:null })); }}>
    <input className="input" aria-label={t("knowledge.search")} placeholder={t("knowledge.search")} maxLength={500} value={draft} onChange={(e) => setDraft(e.target.value)} /><button type="submit" className="btn">{t("knowledge.searchButton")}</button>
  </form><div className="knowledge-results">{!data ? <p className="memory-empty" role={error ? "alert" : "status"}>{error || t("common.loading")}{error && <button className="btn" onClick={reload}>{t("common.retry")}</button>}</p> : data.nodes.length ? data.nodes.map((node) => <KnowledgeLink className={`knowledge-node${params.get("knowledgeNode") === node.id ? " active" : ""}`} key={node.id} projectId={projectId} values={{ knowledgeNode:node.id,knowledgeLink:null }}><strong>{node.name}</strong><small>{node.kind} · {node.filePath}:{node.startLine}</small></KnowledgeLink>) : <p className="memory-empty">{t("knowledge.noResults")}</p>}</div>
    {data && <div className="memory-pagination">{page>0 ? <KnowledgeLink projectId={projectId} values={{ knowledgePage:page-1 }}>{t("common.prev")}</KnowledgeLink> : <span />}<span>{page+1} / {Math.max(1,Math.ceil(data.total/data.pageSize))}</span>{(page+1)*data.pageSize<data.total ? <KnowledgeLink projectId={projectId} values={{ knowledgePage:page+1 }}>{t("common.next")}</KnowledgeLink> : <span />}</div>}
  </aside>;
}
