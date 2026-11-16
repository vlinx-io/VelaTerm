import { useEffect, useRef, useState } from "react";
import { useT } from "../../../i18n";
import { chatReadCancel, chatSearch, type ChatSearchHit } from "../../../ipc/chat";

/** Search holds one result page. Neither opening it nor an empty query requests any historical body. */
export function HistorySearch({ sessionId, onLocate, onClose }: {
  sessionId: string;
  onLocate: (hit: ChatSearchHit, query: string) => Promise<void>;
  onClose: () => void;
}) {
  const t = useT();
  const [query, setQuery] = useState(() => new URLSearchParams(window.location.search).get("chatSearch") ?? "");
  const [hits, setHits] = useState<ChatSearchHit[]>([]);
  const [selected, setSelected] = useState(0);
  const [cursor, setCursor] = useState<string | null>(null);
  const [continuation, setContinuation] = useState<string | undefined>();
  const [coverage, setCoverage] = useState("complete");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const job = useRef<string | undefined>(undefined);
  const generation = useRef(0);
  useEffect(() => { input.current?.focus(); }, []);
  useEffect(() => {
    const update = () => { setQuery(new URLSearchParams(window.location.search).get("chatSearch") ?? ""); setContinuation(undefined); };
    window.addEventListener("popstate", update);
    return () => window.removeEventListener("popstate", update);
  }, []);
  useEffect(() => {
    const current = ++generation.current;
    let disposed = false;
    setHits([]); setSelected(0); setCursor(null); setError(null);
    if (!query) { setLoading(false); setCoverage("complete"); return; }
    setLoading(true);
    const timer = setTimeout(() => void (async () => {
      let next = continuation;
      const collected: ChatSearchHit[] = [];
      try {
        do {
          const page = await chatSearch(sessionId, query, next);
          if (disposed || current !== generation.current) {
            if (page.jobId) await chatReadCancel(sessionId, page.jobId);
            return;
          }
          job.current = page.jobId;
          // The server emits at most fifty hits. Keep this page intact; advancing replaces it.
          collected.push(...page.hits);
          setHits(collected.slice(0, 50)); setCoverage(page.coverage); setCursor(page.nextCursor);
          next = page.nextCursor ?? undefined;
        } while (!disposed && next && !collected.length);
      } catch (cause) { if (!disposed) setError(String(cause)); }
      finally { if (!disposed) setLoading(false); }
    })(), 200);
    return () => {
      disposed = true; clearTimeout(timer);
      // A continuation uses the same job; cancel only when the query, owner, or mounted search changes.
    };
  }, [sessionId, query, continuation, retry]);
  useEffect(() => () => {
    const id = job.current; job.current = undefined;
    if (id) void chatReadCancel(sessionId, id).catch(() => {});
  }, [sessionId, query]);
  const move = async (position: number) => {
    const hit = hits[position]; if (!hit) return;
    setSelected(position); setError(null);
    try { await onLocate(hit, query); } catch (cause) { setError(String(cause)); }
  };
  return <div className="sv-search" role="search" onKeyDown={event => {
    event.stopPropagation();
    if (event.key === "Escape") { event.preventDefault(); onClose(); }
    if (event.key === "Enter" && !event.nativeEvent.isComposing && event.keyCode !== 229 && hits.length) {
      event.preventDefault(); void move((selected + (event.shiftKey ? -1 : 1) + hits.length) % hits.length);
    }
  }}>
    <div className="sv-search-controls">
      <input ref={input} className="vlx-input" aria-label={t("archive.searchTranscript")} placeholder={t("doc.searchPlaceholder")} value={query} onChange={event => {
        const value = event.target.value;
        const url = new URL(window.location.href);
        if (value) url.searchParams.set("chatSearch", value); else url.searchParams.delete("chatSearch");
        window.history.replaceState(null, "", url.href);
        setQuery(value); setContinuation(undefined);
      }} />
      <span role="status">{loading ? t("common.loading") : hits.length ? `${selected + 1}/${hits.length}` : query && coverage === "complete" && !error ? t("doc.searchNoMatch") : ""}</span>
      <button className="vlx-btn" title={t("common.prev")} disabled={!hits.length} onClick={() => void move((selected - 1 + hits.length) % hits.length)}>↑</button>
      <button className="vlx-btn" title={t("common.next")} disabled={!hits.length} onClick={() => void move((selected + 1) % hits.length)}>↓</button>
      {cursor && <button className="vlx-btn" disabled={loading} onClick={() => setContinuation(cursor)}>{t("chat.history.searchMore")}</button>}
      <button className="vlx-btn" title={t("common.close")} onClick={onClose}>✕</button>
    </div>
    {error && <div role="alert">{error} <button className="vlx-btn" onClick={() => setRetry(value => value + 1)}>{t("common.retry")}</button></div>}
    {!loading && coverage === "partial" && !cursor && <div role="status">{t("chat.history.indexing")} <button className="vlx-btn" onClick={() => { setContinuation(undefined); setRetry(value => value + 1); }}>{t("common.retry")}</button></div>}
    {hits[selected] && <div className="sv-search-snippet"><button className="vlx-btn" onClick={() => void move(selected)}>{hits[selected].snippet}</button></div>}
  </div>;
}
