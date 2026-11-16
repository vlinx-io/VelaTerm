import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useT } from "../../../i18n";
import { chatChildSnapshot, chatHistorySessionId, chatHistoryChildPath, chatReadCancel, chatToolContextKey, type ChatRow, type ChatSnapshot } from "../../../ipc/chat";
import { STALE_CURSOR, failureText } from "./historyCursor";

/** Only an expanded parent owns a child page; moving between pages replaces the previous window. */
export function ChildConversation({ row, renderRows }: { row: ChatRow; renderRows: (rows: ChatRow[]) => ReactNode }) {
  const t=useT();
  const [page,setPage]=useState<ChatSnapshot>();
  const [error,setError]=useState<string>();
  const [loading,setLoading]=useState(false);
  const current=useRef<ChatSnapshot | undefined>(undefined);
  const source=useRef(row);source.current=row;
  const section=useRef<HTMLDivElement>(null);
  const reader=useRef<string | undefined>(undefined);
  const request=useRef(0);
  const busy=useRef(false);
  /** One automatic reload per cursor failure: a stale cursor is dropped, then the view asks again. */
  const coldRetry=useRef(false);
  const context=chatToolContextKey(row);
  const load=useCallback(async (direction?:"before"|"after") => {
    if (busy.current) return;
    busy.current=true;setLoading(true);
    const sequence=++request.current;
    // After a cursor the server no longer resolves, the window and its reader are forgotten so the
    // next read asks for the newest rows instead of repeating the same failure.
    const known=coldRetry.current ? undefined : current.current;
    const owner=source.current;
    const session=chatHistorySessionId(owner);
    const query=new URLSearchParams(window.location.search);
    const path=[...chatHistoryChildPath(source.current),source.current.id];
    const route=query.getAll("chatChild");
    const anchor=(!coldRetry.current && (!query.has("chatSession") || query.get("chatSession")===chatHistorySessionId(source.current)) && path.every((parent,index)=>route[index]===parent))
      ? route[path.length]??query.get("chatRow")??undefined : undefined;
    let cold=false;
    const recover=()=>{
      if (coldRetry.current) return false;
      coldRetry.current=true;cold=true;
      if (reader.current && session) void chatReadCancel(session,reader.current).catch(()=>{});
      reader.current=undefined;current.current=undefined;setError(undefined);
      return true;
    };
    try {
      const next=await chatChildSnapshot(source.current,{
        ...(direction==="before" ? {before:known?.rows[0]?.id} : direction==="after" ? {after:known?.rows.at(-1)?.id}
          : known?.hasNewer ? {anchor:known.rows.at(-1)?.id} : anchor ? {anchor} : {}),readJobId:reader.current,
      });
      if (sequence!==request.current) {
        if (next.readJobId && session) void chatReadCancel(session,next.readJobId).catch(()=>{});
        return;
      }
      reader.current=next.readJobId;current.current=next;setPage(next);
      const failure=next.historyError;
      if (!failure) {coldRetry.current=false;setError(undefined);}
      else if (!STALE_CURSOR.test(failure) || !recover()) setError(failure);
    } catch (failure) {
      if (sequence===request.current) {
        const text=failureText(failure);
        if (!STALE_CURSOR.test(text) || !recover()) setError(text);
      }
    }
    finally {
      if (sequence===request.current) {
        busy.current=false;setLoading(false);
        if (cold) void load();
      }
    }
  },[]);
  useEffect(()=>{
    request.current++;busy.current=false;current.current=undefined;coldRetry.current=false;setPage(undefined);setError(undefined);
    const session=chatHistorySessionId(source.current);
    void load();
    const timer=setInterval(()=>{
      if (document.visibilityState!=="hidden" && section.current?.getClientRects().length) void load();
    },2000);
    return ()=>{
      clearInterval(timer);request.current++;busy.current=false;
      if (reader.current && session) void chatReadCancel(session,reader.current).catch(()=>{});
      reader.current=undefined;
    };
  },[context,load]);
  return <div className="sv-subagent" ref={section}>
    {page?.hasMore && <button className="vlx-btn" disabled={loading} onClick={()=>void load("before")}>{t("chat.sync.history")}</button>}
    {error && <div role="alert">{error}<button className="vlx-btn" disabled={loading} onClick={()=>void load()}>{t("common.retry")}</button></div>}
    {page?.coverage==="partial" || loading && !page ? <div className="sv-sync-state sv-loading-hint" role="status">{t("chat.history.indexing")}</div> : null}
    {page && renderRows(page.rows)}
    {page?.hasNewer && <button className="vlx-btn" disabled={loading} onClick={()=>void load("after")}>{t("chat.history.newer")}</button>}
  </div>;
}
