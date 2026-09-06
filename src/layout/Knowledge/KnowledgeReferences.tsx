import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { knowledgeLinks, knowledgeUnlink } from "../../ipc/knowledge";
import { KnowledgeLink } from "./navigation";
import { knowledgeError, KnowledgeState, useKnowledgeLoad } from "./shared";

export function KnowledgeReferences({ entryId }: { entryId: string }) {
  const t = useT(); const { data,error,reload } = useKnowledgeLoad(() => knowledgeLinks(entryId), [entryId]);
  const [busy,setBusy] = useState(false); const [failure,setFailure] = useState("");
  const unlink = async (id: string) => {
    setBusy(true); setFailure("");
    try { await knowledgeUnlink(entryId,id); reload(); } catch (e) { setFailure(knowledgeError(e)); } finally { setBusy(false); }
  };
  useEffect(() => {
    const refresh = () => { if (!document.hidden) reload(); };
    window.addEventListener("focus",refresh);
    const timer = window.setInterval(refresh,5000);
    return () => { window.removeEventListener("focus",refresh); window.clearInterval(timer); };
  }, [entryId]);
  return <section className="knowledge-references"><div className="memory-row"><h3>{t("knowledge.codeReferences")}</h3><button className="btn" onClick={reload}>{t("knowledge.refresh")}</button></div>
    {(error || failure) && <p role="alert">{error || failure}</p>}
    {data?.length === 0 && <p className="memory-muted">{t("knowledge.noLinks")}</p>}
    {data?.map((link) => <div key={link.id} className="knowledge-reference"><KnowledgeLink projectId={link.projectId} values={{ knowledgeIndex:link.indexId,knowledgeNode:link.nodeId,knowledgeQuery:link.nodeId ? null : link.symbol,knowledgeLink:link.nodeId ? entryId : null }}><strong>{link.symbol}</strong><small>{link.root} · {link.filePath}</small></KnowledgeLink><KnowledgeState value={link.status} /><button className="btn" disabled={busy} onClick={() => void unlink(link.id)}>{t("knowledge.unlink")}</button></div>)}
  </section>;
}
