import { useState } from "react";
import { useT } from "../../i18n";
import { knowledgeLink, knowledgeNode, knowledgeReview, knowledgeUnlink, type CodeEdge, type KnowledgeIndex } from "../../ipc/knowledge";
import { memoryGet, memoryOptions } from "../../ipc/memory";
import { MemoryLink, memoryNavigate } from "../Memory/navigation";
import { MemoryMarkdown } from "../Memory/shared";
import { KnowledgeLink, knowledgeUrl } from "./navigation";
import { knowledgeError, KnowledgeState, useKnowledgeLoad } from "./shared";

export function KnowledgeSymbol({ index, nodeId, linkEntry }: { index: KnowledgeIndex; nodeId: string; linkEntry: string | null }) {
  const t = useT(); const { data,error,reload } = useKnowledgeLoad(() => knowledgeNode(index.id,nodeId), [index.id,nodeId]);
  const [failure,setFailure] = useState(""); const [busy,setBusy] = useState(false);
  const perform = async (fn: () => Promise<unknown>) => {
    setBusy(true); setFailure("");
    try { await fn(); memoryNavigate(knowledgeUrl(index.projectId,{ knowledgeLink:null }));reload(); } catch (e) { setFailure(knowledgeError(e)); } finally { setBusy(false); }
  };
  if (!data) return <p className="memory-empty" role={error ? "alert" : "status"}>{error || t("common.loading")}{error && <button className="btn" onClick={reload}>{t("common.retry")}</button>}</p>;
  const existing = data.memories.find((m) => m.entryId === linkEntry);
  return <article className="knowledge-symbol">
    <div className="knowledge-symbol-heading"><KnowledgeLink className="knowledge-mobile-back" projectId={index.projectId} values={{ knowledgeNode:null,knowledgeLink:null }}>← {t("knowledge.symbols")}</KnowledgeLink><h2>{data.node.qualifiedName}</h2><p>{data.node.filePath}:{data.node.startLine} · {data.node.kind} · {data.node.language}</p><button className="btn" onClick={reload}>{t("knowledge.sync")}</button></div>
    <p className="knowledge-note">{t("knowledge.analysisNote")}</p>
    {(data.changedDuringRead || data.truncated || data.sourceTruncated) && <p className="knowledge-warning" role="status">{t(data.changedDuringRead ? "knowledge.changed" : "knowledge.truncated")}</p>}
    <div className="knowledge-graph"><EdgeColumn edges={data.incoming} index={index} incoming />
      <section className="knowledge-source"><h3>{t("knowledge.source")}</h3><pre aria-label={t("knowledge.source")}>{data.source.split("\n").map((line,i) => <div key={i}><span className="knowledge-line" aria-hidden="true">{data.node.startLine+i}</span><code>{line || " "}</code></div>)}</pre></section>
      <EdgeColumn edges={data.outgoing} index={index} /></div>
    <section className="knowledge-memories"><div className="memory-row"><h3>{t("memory.related")}</h3><span className="memory-spacer" /><KnowledgeLink className="btn" projectId={index.projectId} values={{ knowledgeLink:"choose" }}>{t("knowledge.linkMemory")}</KnowledgeLink></div>
      {failure && <p className="knowledge-warning" role="alert">{failure}</p>}
      {!data.memories.length && <p className="knowledge-note">{t("knowledge.noLinks")}</p>}
      {data.memories.map((link) => <div className="knowledge-memory" key={link.id}><MemoryLink route={`entry/${link.entryId}`}>{link.title}</MemoryLink><KnowledgeState value={link.status} /><KnowledgeLink className="btn" projectId={index.projectId} values={{ knowledgeLink:link.entryId }}>{t("knowledge.inspect")}</KnowledgeLink><button className="btn" disabled={busy} onClick={() => void perform(() => knowledgeUnlink(link.entryId,link.id))}>{t("knowledge.unlink")}</button></div>)}
      {linkEntry && <LinkEditor index={index} entryId={linkEntry} busy={busy || data.changedDuringRead} existing={!!existing} onSave={(version) => void perform(() => existing ? knowledgeReview(linkEntry,existing.id,version,data.digest) : knowledgeLink(linkEntry,version,index.id,nodeId,data.digest))} />}
    </section>
  </article>;
}
function EdgeColumn({ edges,index,incoming=false }: { edges: CodeEdge[]; index: KnowledgeIndex; incoming?: boolean }) {
  const t = useT();
  return <section className={`knowledge-edges ${incoming ? "incoming" : "outgoing"}`}><h3>{t(incoming ? "knowledge.incoming" : "knowledge.outgoing")} <span aria-hidden="true">→</span></h3>
    {!edges.length && <p className="knowledge-note">{t("knowledge.noEdges")}</p>}
    {edges.map((edge,i) => <KnowledgeLink key={`${edge.node.id}/${edge.kind}/${i}`} className="knowledge-edge" projectId={index.projectId} values={{ knowledgeNode:edge.node.id,knowledgeLink:null }}><strong>{edge.node.name}</strong><small>{edge.kind}{edge.line ? ` · L${edge.line}` : ""}</small><small>{edge.node.filePath}:{edge.node.startLine}</small>{edge.provenance && <small>{edge.provenance}</small>}</KnowledgeLink>)}
  </section>;
}
function LinkEditor({ index,entryId,busy,existing,onSave }: { index: KnowledgeIndex; entryId: string; busy: boolean; existing: boolean; onSave: (version: number) => void }) {
  const t = useT(); const { data,error } = useKnowledgeLoad(() => memoryOptions(), []);
  return <section className="knowledge-link-editor" aria-label={t("knowledge.linkMemory")}><div className="memory-row"><h3>{t(existing ? "knowledge.inspect" : "knowledge.linkMemory")}</h3><span className="memory-spacer" /><KnowledgeLink projectId={index.projectId} values={{ knowledgeLink:null }}>{t("common.close")}</KnowledgeLink></div>
    <label>{t("memory.title")}<select className="input" value={entryId === "choose" ? "" : entryId} onChange={(e) => memoryNavigate(knowledgeUrl(index.projectId,{ knowledgeLink:e.target.value || "choose" }))}><option value="">{t("knowledge.chooseMemory")}</option>{data?.catalog.map((entry) => <option key={entry.id} value={entry.id}>{entry.title}</option>)}</select></label>
    {error && <p role="alert">{error}</p>}
    {entryId !== "choose" && <MemoryReview key={entryId} id={entryId} busy={busy} existing={existing} onSave={onSave} />}
    {data?.catalog.length === 0 && <MemoryLink route="new">{t("memory.new")}</MemoryLink>}
  </section>;
}
function MemoryReview({ id,busy,existing,onSave }: { id: string; busy: boolean; existing: boolean; onSave: (version: number) => void }) {
  const t = useT(); const { data,error,reload } = useKnowledgeLoad(() => memoryGet(id), [id]);
  if (!data) return <p role={error ? "alert" : "status"}>{error || t("common.loading")}{error && <button className="btn" onClick={reload}>{t("common.retry")}</button>}</p>;
  return <><p>{data.entry.summary}</p><MemoryMarkdown content={data.entry.content} /><p className="knowledge-note">{t("knowledge.reviewHelp")}</p><button className="btn btn-primary" disabled={busy} onClick={() => onSave(data.entry.version)}>{t(existing ? "knowledge.confirmReview" : "knowledge.linkMemory")}</button></>;
}
