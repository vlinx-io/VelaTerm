import type { AnchorHTMLAttributes } from "react";
import { memoryNavigate, useMemoryLocation } from "../Memory/navigation";
export { useMemoryLocation as useKnowledgeLocation };
export function knowledgeUrl(projectId: string, values: Record<string, string | number | null> = {}) {
  const url = new URL(window.location.href);
  for (const key of [...url.searchParams.keys()]) if (key.startsWith("memory") || (!projectId && key.startsWith("knowledge"))) url.searchParams.delete(key);
  if (projectId) {
    if (url.searchParams.get("knowledge") !== projectId) for (const key of [...url.searchParams.keys()]) if (key.startsWith("knowledge")) url.searchParams.delete(key);
    url.searchParams.set("knowledge", projectId);
  }
  for (const [key,value] of Object.entries(values)) {
    if (value == null || value === "") url.searchParams.delete(key); else url.searchParams.set(key,String(value));
  }
  return `${url.pathname}${url.search}${url.hash}`;
}
export function KnowledgeLink({ projectId, values, ...props }: AnchorHTMLAttributes<HTMLAnchorElement> & { projectId: string; values?: Record<string, string | number | null> }) {
  const href = knowledgeUrl(projectId, values);
  return <a {...props} href={href} onClick={(e) => {
    props.onClick?.(e);
    if (e.defaultPrevented || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button !== 0) return;
    e.preventDefault(); memoryNavigate(href);
  }} />;
}
