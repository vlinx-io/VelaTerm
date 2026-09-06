import { useEffect, useMemo, useState } from "react";
import DOMPurify from "dompurify";
import { marked } from "marked";
import { dateLocale, t, type I18nKey } from "../../i18n";
import { memoryNavigate } from "./navigation";

const errors: Record<string, I18nKey> = {
  memory_conflict: "memory.conflict", memory_duplicate_title: "memory.duplicate", memory_busy: "memory.busy",
  memory_not_found: "memory.notFound", memory_no_transcript: "memory.noTranscript", memory_agent_unavailable: "memory.agentUnavailable",
  memory_invalid: "memory.invalid", memory_process_failed: "memory.processFailed", memory_timeout: "memory.timeout",
  memory_interrupted: "memory.interrupted", memory_source_too_large: "memory.tooLarge", memory_context_too_large: "memory.tooLarge",
  memory_output_too_large: "memory.tooLarge", memory_invalid_output: "memory.invalidOutput", memory_cancelled: "memory.cancelled",
};
export function memoryError(error: unknown) {
  const text = String(error);
  const code = text.match(/memory_[a-z_]+/)?.[0];
  return t(errors[code ?? ""] ?? "memory.loadError");
}
export function memoryTime(stamp: number) { return new Date(stamp).toLocaleString(dateLocale()); }
export function StateLabel({ value }: { value: string }) {
  const key = `memory.${value}`;
  const allowed = ["running", "completed", "failed", "cancelled", "extract", "merge", "commit", "done"];
  return <span>{allowed.includes(value) ? t(key as I18nKey) : value}</span>;
}
export function useMemoryLoad<T>(loader: () => Promise<T>, deps: unknown[]) {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState("");
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    let alive = true; setData(null); setError("");
    loader().then((v) => { if (alive) setData(v); }).catch((e) => { if (alive) setError(memoryError(e)); });
    return () => { alive = false; };
    // The caller explicitly declares resource identity; the closure itself changes every render.
  }, [...deps, revision]);
  return { data, error, reload: () => setRevision((v) => v + 1) };
}
export function LoadState({ error, reload }: { error: string; reload: () => void }) {
  return <div className="memory-empty" role={error ? "alert" : "status"}>{error || t("common.loading")}
    {error && <button className="btn" onClick={reload}>{t("common.retry")}</button>}
  </div>;
}
export function MemoryMarkdown({ content }: { content: string }) {
  const html = useMemo(() => DOMPurify.sanitize(marked.parse(content, { async: false }) as string, {
    FORBID_TAGS: ["img", "video", "audio", "iframe", "form", "input", "style"], FORBID_ATTR: ["style"],
  }), [content]);
  return <div className="memory-markdown" dangerouslySetInnerHTML={{ __html: html }} onClick={(e) => {
    const anchor = (e.target as HTMLElement).closest("a");
    if (!anchor) return;
    const url = new URL(anchor.href, window.location.href);
    if (url.origin === window.location.origin && url.searchParams.has("memory")) {
      if (!e.metaKey && !e.ctrlKey && !e.shiftKey && !e.altKey) { e.preventDefault(); memoryNavigate(`${url.pathname}${url.search}${url.hash}`); }
    } else if (url.protocol === "http:" || url.protocol === "https:") {
      anchor.target = "_blank"; anchor.rel = "noopener noreferrer";
    }
  }} />;
}
