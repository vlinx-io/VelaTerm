import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import Select from "../../../components/Select";
import { useT } from "../../../i18n";
import { canCopyCompleteChatDetail, copyCompleteChatDetail, downloadCompleteChatDetail, chatDetailImage, chatDetailLocate, chatDetailRead, chatManifest, chatHistorySessionId, chatHistoryChildPath, hasBoundedChatDetails, type ChatDetailChunk, type ChatDetailField, type ChatManifest, type ChatRow } from "../../../ipc/chat";
import { platform } from "../../../platform";
import { copyAttributes } from "./selectionCopy";
import { handleMessageCopy } from "./useMessageCopyMenu";
import "./deferred-contents.css";
import { historyPageURL } from "./historyNavigation";
import { ChatImageView } from "./rows";

/** Keep even an individual exceptionally long physical line out of a single DOM text node. */
export function detailTextParts(text: string): string[] {
  const parts: string[] = [];
  let start = 0;
  let bytes = 0;
  for (let end = 0; end < text.length;) {
    const code = text.codePointAt(end)!;
    const width = code < 128 ? 1 : code < 2048 ? 2 : code < 65536 ? 3 : 4;
    if (bytes + width > 4096) { parts.push(text.slice(start, end));start = end;bytes = 0; }
    bytes += width;end += code > 65535 ? 2 : 1;
  }
  if (start < text.length) parts.push(text.slice(start));
  return parts;
}

function readRoute(row: ChatRow) {
  const query = new URLSearchParams(window.location.search);
  if (query.has("chatSession") && query.get("chatSession") !== chatHistorySessionId(row)) return { reference: "", sequence: 0, fileCursor: undefined };
  return query.get("chatRow") === row.id ? { reference: query.get("chatField") ?? "", sequence: Number(query.get("chatPart") ?? 0), fileCursor: query.get("chatFiles") ?? undefined } : { reference: "", sequence: 0, fileCursor: undefined };
}

function opensRow(row: ChatRow) {
  const query=new URLSearchParams(window.location.search);
  return query.get("chatRow")===row.id && (!query.has("chatSession") || query.get("chatSession")===chatHistorySessionId(row));
}

function routeUrl(row: ChatRow, reference: string, sequence: number) {
  const session = chatHistorySessionId(row);
  const url = new URL(session ? historyPageURL(session,"after") : window.location.href);
  url.searchParams.delete("chatBefore");url.searchParams.delete("chatAfter");
  for (const parent of chatHistoryChildPath(row)) url.searchParams.append("chatChild",parent);
  url.searchParams.set("chatRow", row.id);
  url.searchParams.set("chatField", reference);
  url.searchParams.set("chatPart", String(sequence));
  return url.href;
}

export function DeferredBody({ row }: { row: ChatRow }) {
  const t=useT();
  const [open,setOpen]=useState(() => opensRow(row));
  useEffect(() => {
    const update=() => setOpen(opensRow(row));
    window.addEventListener("popstate",update);
    return () => window.removeEventListener("popstate",update);
  },[row.id]);
  if (!row.bodyAvailable || !hasBoundedChatDetails(row)) return null;
  // A message or thought keeps the conversation's text face when read in full; tool details stay monospace.
  return <div className="sv-deferred sv-deferred-prose">
    <a className="vlx-btn" href={routeUrl(row,"",0)} onClick={event => {
      if (event.button!==0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      event.preventDefault(); window.history.pushState(null,"",event.currentTarget.href); window.dispatchEvent(new PopStateEvent("popstate"));
    }}>{t("chat.history.readBody")}</a>
    {open && <DeferredContents row={row} />}
  </div>;
}

/** A manifest is small. Only an explicitly selected field/file is fetched, one replaceable part at a time. */
export function DeferredContents({ row }: { row: ChatRow }) {
  const t = useT();
  const [manifest, setManifest] = useState<ChatManifest | null>(null);
  const [route, setRoute] = useState(() => readRoute(row));
  const [chunk, setChunk] = useState<ChatDetailChunk | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const [loading, setLoading] = useState(false);
  const [fullField,setFullField]=useState<ChatDetailField | null>(null);
  const image=useMemo(()=>fullField?.kind==="image" ? chatDetailImage(row,fullField) : undefined,[row,fullField]);
  const [transferring,setTransferring]=useState(false);
  const copyTransfer=useRef<AbortController | null>(null);
  useEffect(()=>()=>{copyTransfer.current?.abort();},[row,route.reference]);
  useEffect(() => {
    const update = () => setRoute(readRoute(row));
    window.addEventListener("popstate", update);
    return () => window.removeEventListener("popstate", update);
  }, [row]);
  useEffect(() => {
    let active = true;
    setManifest(null); setError(null);
    void chatManifest(row, route.fileCursor).then(value => { if (active) setManifest(value); }, cause => { if (active) setError(String(cause)); });
    return () => { active = false; };
  }, [row, route.fileCursor, retry]);
  const fields = useMemo(() => {
    const result = new Map<string, ChatDetailField>();
    for (const file of manifest?.files ?? []) if (file.detailRef && file.contentRevision && file.kind) result.set(file.detailRef, {
      detailRef: file.detailRef, contentRevision: file.contentRevision, kind: file.kind, path: file.path, label: "input", sizeBytes: file.sizeBytes ?? undefined,
    });
    for (const field of manifest?.fields ?? []) {
      if (manifest?.files.length && field.label==="input" && field.path.startsWith("/payload/item/changes")) continue;
      if (!result.has(field.detailRef)) result.set(field.detailRef, field);
    }
    return [...result.values()];
  }, [manifest]);
  useEffect(() => {
    if (!route.reference || !Number.isSafeInteger(route.sequence) || route.sequence < 0) { setChunk(null); setFullField(null); return; }
    let active = true;
    setLoading(true); setChunk(null); setFullField(null); setError(null);
    void (async () => {
      try {
        const locator=await chatDetailLocate(row, route.reference, route.sequence);
        if (!active) return;
        setFullField(locator.field);
        if (locator.field.kind==="image") return;
        const value=await chatDetailRead(row, locator.field, locator.cursor);
        if (active) setChunk(value);
      } catch (cause) { if (active) setError(String(cause)); }
      finally { if (active) setLoading(false); }
    })();
    return () => { active = false; };
  }, [row, route.reference, route.sequence, retry]);
  const navigate = (reference: string, sequence: number) => {
    window.history.pushState(null, "", routeUrl(row, reference, sequence));
    window.dispatchEvent(new PopStateEvent("popstate"));
  };
  const follow = (event: MouseEvent<HTMLAnchorElement>, sequence: number) => {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    event.preventDefault(); navigate(route.reference, sequence);
  };
  const manifestUrl = (cursor: string | null) => {
    const url=new URL(routeUrl(row,route.reference,route.sequence)); url.searchParams.set("chatRow",row.id);
    if (cursor) url.searchParams.set("chatFiles",cursor); else url.searchParams.delete("chatFiles");
    return url.href;
  };
  const followManifest = (event: MouseEvent<HTMLAnchorElement>) => {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    event.preventDefault(); window.history.pushState(null,"",event.currentTarget.href); window.dispatchEvent(new PopStateEvent("popstate"));
  };
  return <div className="sv-deferred">
    {!manifest && !error && <div role="status">{t("common.loading")}</div>}
    {error && <div role="alert">{error} <button className="vlx-btn" onClick={() => setRetry(value => value + 1)}>{t("common.retry")}</button></div>}
    {manifest && <>
      <div className="sv-deferred-toolbar">
        <Select value={route.reference} ariaLabel={t("chat.history.selectDetail")} placeholder={t("chat.history.selectDetail")} options={fields.map(field => ({
          value: field.detailRef, label: field.path.replace(/^\/(?:payload\/(?:item\/)?|message\/)/, ""),
        }))} onChange={reference => navigate(reference, 0)} />
        {route.fileCursor && <a className="vlx-btn" href={manifestUrl(manifest.previousCursor ?? null)} onClick={followManifest}>{t("common.prev")}</a>}
        {manifest.nextCursor !== null && <a className="vlx-btn" href={manifestUrl(manifest.nextCursor)} onClick={followManifest}>{t("common.next")}</a>}
      </div>
      {!fields.length && <div>{t("session.toolNoDetail")}</div>}
    </>}
    {loading && <div role="status">{t("common.loading")}</div>}
    {image && fullField && <div className="sv-deferred-content">
      <ChatImageView image={image} className="sv-user-image" alt={t("crepe.image")} />
      <button className="vlx-btn" disabled={transferring} onClick={()=>{
        setTransferring(true);void downloadCompleteChatDetail(row,fullField).catch(cause=>setError(String(cause))).finally(()=>setTransferring(false));
      }}>{t("chat.history.downloadComplete")}</button>
    </div>}
    {chunk && <div className="sv-deferred-content">
      <div className="sv-deferred-toolbar">
        <span>{t("chat.history.part")} {chunk.chunkSequence + 1}</span>
        {chunk.chunkSequence > 0 && <a className="vlx-btn" href={routeUrl(row, route.reference, chunk.chunkSequence - 1)} onClick={event => follow(event, chunk.chunkSequence - 1)}>{t("common.prev")}</a>}
        {chunk.nextCursor !== null && <a className="vlx-btn" href={routeUrl(row, route.reference, chunk.chunkSequence + 1)} onClick={event => follow(event, chunk.chunkSequence + 1)}>{t("common.next")}</a>}
        <button className="vlx-btn" onClick={() => void platform.clipboard.writeText(chunk.text, { reportFailure: true }).catch(() => setError(t("common.copyFailed")))}>{t("chat.history.copyPart")}</button>
        {fullField && canCopyCompleteChatDetail(fullField) && <button className="vlx-btn" disabled={transferring} onClick={() => {
          setTransferring(true);const controller=new AbortController();copyTransfer.current=controller;
          void copyCompleteChatDetail(row,fullField,controller.signal).catch(cause=>{if (!controller.signal.aborted) setError(String(cause));}).finally(()=>setTransferring(false));
        }}>{t("chat.history.copyComplete")}</button>}
        {fullField && <button className="vlx-btn" disabled={transferring} onClick={() => {
          setTransferring(true);void downloadCompleteChatDetail(row,fullField).catch(cause=>setError(String(cause))).finally(()=>setTransferring(false));
        }}>{t("chat.history.downloadComplete")}</button>}
      </div>
      <div className="sv-deferred-text" {...copyAttributes.pre} onCopy={handleMessageCopy}>
        {detailTextParts(chunk.text).map((part, index) => <span key={index}>{part}</span>)}
      </div>
    </div>}
  </div>;
}
