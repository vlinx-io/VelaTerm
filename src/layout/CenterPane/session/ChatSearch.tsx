import { useEffect, useId, useMemo, useRef, useState, type RefObject } from "react";
import { useT } from "../../../i18n";
import { loadChatTool, type ChatRow } from "../../../ipc/chat";
import type { DisplayRow } from "./toolRuns";

/** Search content supplied by the engine, including folded and virtualized rows. */
function texts(row: ChatRow): string[] {
  if (row.kind === "tool") return [row.name, typeof row.input === "string" ? row.input : JSON.stringify(row.input, null, 2) ?? "", row.output ?? "", ...(row.children ?? []).flatMap(texts)];
  if ("text" in row) return [row.text];
  if ("message" in row) return [row.message];
  return [];
}

const searchDetails = new WeakMap<ChatRow, string[]>();
async function completeTexts(row: ChatRow): Promise<string[]> {
  const cached = searchDetails.get(row);
  if (cached) return cached;
  if (row.kind !== "tool" || !row.detailAvailable) return texts(row);
  const detail = await loadChatTool(row);
  const result = [detail.name, typeof detail.input === "string" ? detail.input : JSON.stringify(detail.input, null, 2) ?? "", detail.output ?? ""];
  for (const child of detail.children ?? []) result.push(...await completeTexts(child));
  searchDetails.set(row, result);
  return result;
}

export function ChatSearch({ entries, onLocate, onClose, scrollRef }: {
  entries: DisplayRow[];
  scrollRef?: RefObject<HTMLDivElement | null>;
  onLocate: (index: number, query: string) => void;
  onClose: () => void;
}) {
  const t = useT();
  const [query, setQuery] = useState("");
  const [position, setPosition] = useState(0);
  const highlightName = "chat-search-" + useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const input = useRef<HTMLInputElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const [detailsVersion, setDetailsVersion] = useState(0);
  const [loadingDetails, setLoadingDetails] = useState(false);
  const [detailsError, setDetailsError] = useState<string | null>(null);
  useEffect(() => {
    const pending = entries.flatMap(entry => entry.kind === "run" ? entry.calls : [entry.row]).filter(row => row.kind === "tool" && row.detailAvailable && !searchDetails.has(row));
    if (!pending.length) { setLoadingDetails(false); return; }
    let disposed = false;
    setLoadingDetails(true);
    setDetailsError(null);
    // Search explicitly reads deferred output; ordinary conversation navigation stays lightweight.
    void (async () => {
      try {
        for (const row of pending) {
          if (disposed) return;
          await completeTexts(row);
        }
        if (!disposed) setDetailsVersion(version => version + 1);
      } catch (error) { if (!disposed) setDetailsError(String(error)); }
      finally { if (!disposed) setLoadingDetails(false); }
    })();
    return () => { disposed = true; };
  }, [entries]);
  const content = useMemo(() => entries.map(entry => (entry.kind === "run" ? entry.calls : [entry.row]).flatMap(row => searchDetails.get(row) ?? texts(row))), [entries, detailsVersion]);
  const matches = useMemo(() => {
    if (!query) return [];
    const pattern = new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "giu");
    return content.flatMap((parts, index) => parts.flatMap(text => Array.from(text.matchAll(pattern), match => ({
      index, text, start: match.index, length: match[0].length,
    }))));
  }, [content, query]);
  const current = matches[Math.min(position, Math.max(0, matches.length - 1))];
  useEffect(() => { input.current?.focus(); }, []);
  useEffect(() => {
    if (current) onLocate(current.index, query);
    else onLocate(-1, "");
  }, [current, onLocate, query]);
  useEffect(() => {
    const root = scrollRef?.current;
    if (!root || !query || typeof Highlight === "undefined" || !CSS.highlights) return;
    let located = false;
    const update = () => {
      const activeRanges: Range[] = [];
      const ranges: Range[] = [];
      const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
      const pattern = new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "giu");
      let node: Node | null;
      while ((node = walker.nextNode())) {
        if (node.parentElement?.closest("button, textarea, input, script, style")) continue;
        for (const match of (node.textContent ?? "").matchAll(pattern)) {
          const range = document.createRange();
          range.setStart(node, match.index);
          range.setEnd(node, match.index + match[0].length);
          ranges.push(range);
          if (current && node.parentElement?.closest<HTMLElement>("[data-search-id]")?.dataset.searchId === entries[current.index]?.id) activeRanges.push(range);
        }
      }
      CSS.highlights.set(highlightName, new Highlight(...ranges));
      if (!located && current && activeRanges.length) {
        const ordinal = matches.slice(0, Math.min(position, matches.length - 1)).filter(match => match.index === current.index).length;
        const rect = activeRanges[Math.min(ordinal, activeRanges.length - 1)].getBoundingClientRect();
        const viewport = root.getBoundingClientRect();
        if (rect.height > 0) {
          located = true;
          if (rect.top < Math.max(viewport.top, panel.current?.getBoundingClientRect().bottom ?? viewport.top) + 8 || rect.bottom > viewport.bottom) root.scrollTop += rect.top - viewport.top - root.clientHeight / 2;
        }
      }
    };
    let frame = requestAnimationFrame(update);
    const observer = new MutationObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(update);
    });
    observer.observe(root, { childList: true, subtree: true, characterData: true });
    return () => { observer.disconnect(); cancelAnimationFrame(frame); CSS.highlights.delete(highlightName); };
  }, [query, scrollRef, highlightName, current, entries, matches, position]);
  const move = (delta: number) => setPosition(value => matches.length ? (Math.min(value, matches.length - 1) + delta + matches.length) % matches.length : 0);
  return <div ref={panel} className="sv-search" role="search" onKeyDown={event => {
    event.stopPropagation();
    if (event.key === "Escape") { event.preventDefault(); onClose(); }
    if (event.key === "Enter" && !event.nativeEvent.isComposing) { event.preventDefault(); move(event.shiftKey ? -1 : 1); }
  }}>
    <style>{`::highlight(${highlightName}) { background: #f5ce58; color: #171717; }`}</style>
    {loadingDetails && <div role="status">{t("common.loading")}</div>}
    {detailsError && <div role="alert">{detailsError}</div>}
    <div className="sv-search-controls">
      <input ref={input} className="vlx-input" aria-label={t("archive.searchTranscript")} placeholder={t("doc.searchPlaceholder")} value={query} onChange={event => { setQuery(event.target.value); setPosition(0); }} />
      <span role="status" aria-label={current ? t("search.matchPosition", Math.min(position + 1, matches.length), matches.length) : undefined} title={current ? current.text.slice(Math.max(0, current.start - 70), current.start + current.length + 100) : undefined}>{query ? current ? `${Math.min(position + 1, matches.length)}/${matches.length}` : t("doc.searchNoMatch") : ""}</span>
      <button className="vlx-btn" disabled={!current} title={t("common.prev")} onClick={() => move(-1)}>↑</button>
      <button className="vlx-btn" disabled={!current} title={t("common.next")} onClick={() => move(1)}>↓</button>
      <button className="vlx-btn" title={t("common.close")} onClick={onClose}>✕</button>
    </div>

  </div>;
}
