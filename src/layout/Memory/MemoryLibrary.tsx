import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { memoryList } from "../../ipc/memory";
import { MemoryLink, memoryNavigate, memoryUrl, useMemoryLocation } from "./navigation";
import { LoadState, memoryTime, useMemoryLoad } from "./shared";

export function MemoryLibrary({ selected }: { selected: string }) {
  const t = useT(); const location = useMemoryLocation(); const params = new URLSearchParams(location);
  const query = params.get("memoryQuery") ?? ""; const tag = params.get("memoryTag") ?? "";
  const sort = params.get("memorySort") ?? "updated"; const page = Math.max(0, Number(params.get("memoryPage")) || 0);
  const [input, setInput] = useState(query);
  useEffect(() => setInput(query), [query]);
  useEffect(() => {
    if (input === query) return;
    const timer = setTimeout(() => memoryNavigate(memoryUrl(params.get("memory") || "library", { memoryQuery: input, memoryPage: null }), true), 250);
    return () => clearTimeout(timer);
  }, [input, query, location]);
  const { data, error, reload } = useMemoryLoad(() => memoryList({ query, tag, sort, page }), [query, tag, sort, page, selected]);
  useEffect(() => {
    window.addEventListener("memory:saved", reload);
    return () => window.removeEventListener("memory:saved", reload);
  }, [reload]);
  return <aside className="memory-sidebar">
    <div className="memory-search">
      <input className="input" aria-label={t("memory.search")} placeholder={t("memory.search")} value={input} onChange={(e) => setInput(e.target.value)} />
      <div className="memory-row">
        <select className="input" aria-label={t("memory.tags")} value={tag} onChange={(e) => memoryNavigate(memoryUrl("library", { memoryTag: e.target.value, memoryPage: null }))}>
          <option value="">{t("memory.allTags")}</option>
          {tag && !data?.tags.includes(tag) && <option>{tag}</option>}
          {data?.tags.map((value) => <option key={value}>{value}</option>)}
        </select>
        <select className="input" aria-label={t("memory.updated")} value={sort} onChange={(e) => memoryNavigate(memoryUrl("library", { memorySort: e.target.value, memoryPage: null }))}>
          <option value="updated">{t("memory.updated")}</option><option value="title">{t("memory.titleSort")}</option>
        </select>
        <button className="btn" title={t("common.refresh")} onClick={reload}>↻</button>
      </div>
    </div>
    {!data ? <LoadState error={error} reload={reload} /> : <>
      <div className="memory-count">{t("memory.entries")} · {data.total}</div>
      <div className="memory-results">
        {!data.entries.length && <p className="memory-empty">{t("memory.empty")}</p>}
        {data.entries.map((entry) => <MemoryLink key={entry.id} route={`entry/${entry.id}`} className={`memory-result${selected === entry.id ? " active" : ""}`}>
          <strong>{entry.title}</strong><p>{entry.summary}</p>
          <div className="memory-tags">{entry.tags.map((tag) => <span key={tag}>{tag}</span>)}</div>
          <small>{memoryTime(entry.updatedAt)} · {t("memory.sources")} {entry.sourceCount}</small>
        </MemoryLink>)}
      </div>
      {data.total > data.pageSize && <footer className="memory-pagination">
        {page > 0 ? <MemoryLink route="library" values={{ memoryPage: page - 1 }}>{t("common.prev")}</MemoryLink> : <span />}
        <span>{page + 1} / {Math.ceil(data.total / data.pageSize)}</span>
        {(page + 1) * data.pageSize < data.total && <MemoryLink route="library" values={{ memoryPage: page + 1 }}>{t("common.next")}</MemoryLink>}
      </footer>}
    </>}
  </aside>;
}
