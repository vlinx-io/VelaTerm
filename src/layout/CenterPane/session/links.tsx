import { createContext, useContext, useState, type MouseEvent, type ReactNode } from "react";

import { platform } from "../../../platform";
import { useTermStore } from "../../../store/termStore";

/** Resolve relative transcript links against the session, including panes that are not focused. */
export const SessionLinkDirectory = createContext<string | undefined>(undefined);

type LinkTarget =
  | { kind: "file"; path: string }
  | { kind: "external"; url: string }
  | { kind: "anchor"; url: string };

export function resolveSessionLink(href: string, cwd?: string): LinkTarget | null {
  const value = href.trim();
  if (!value || /[\u0000-\u001f\u007f]/.test(value)) return null;
  if (value.startsWith("#")) return { kind: "anchor", url: value };
  if (/^(https?:|mailto:)/i.test(value) || value.startsWith("//")) {
    try {
      const url = new URL(value.startsWith("//") ? `https:${value}` : value);
      return { kind: "external", url: url.href };
    } catch { return null; }
  }

  let path = value;
  if (/^file:/i.test(path)) {
    try {
      const url = new URL(path);
      path = url.hostname && url.hostname !== "localhost"
        ? `//${url.hostname}${url.pathname}`
        : url.pathname.replace(/^\/([a-z]:\/)/i, "$1");
    } catch { return null; }
  } else if (/^[a-z][a-z\d+.-]*:/i.test(path) && !/^[a-z]:[/\\]/i.test(path)) {
    // Agent-generated links must never execute script/data URLs or launch arbitrary URI handlers.
    return null;
  }
  try { path = decodeURIComponent(path); } catch { /* A literal percent sign is valid in a filename. */ }
  if (/[\u0000-\u001f\u007f]/.test(path)) return null;
  // Source citations often append a line/column location; it is not part of the filename.
  path = path.replace(/(?::\d+(?::\d+)?|#L\d+(?:C\d+)?(?:-L?\d+(?:C\d+)?)?)$/, "");
  const windows = /^[a-z]:[/\\]/i.test(path) || path.startsWith("\\\\") || /^[a-z]:[/\\]/i.test(cwd ?? "") || cwd?.startsWith("\\\\");
  if (windows) path = path.replace(/\\/g, "/");
  if (!path.startsWith("/") && !/^[a-z]:\//i.test(path)) {
    if (!cwd) return null;
    path = `${windows ? cwd.replace(/\\/g, "/") : cwd}/${path}`;
  }
  const prefix = path.startsWith("//") ? "//" : path.startsWith("/") ? "/" : "";
  const parts: string[] = [];
  for (const part of path.slice(prefix.length).split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") {
      if (parts.length && !/^[a-z]:$/i.test(parts.at(-1)!)) parts.pop();
    } else parts.push(part);
  }
  return { kind: "file", path: prefix + parts.join("/") };
}

/** Files use the existing backend-backed viewer; URLs use the native opener on desktop. */
export function SessionLink({ href, children }: { href: string; children: ReactNode }) {
  const cwd = useContext(SessionLinkDirectory);
  const target = resolveSessionLink(href, cwd);
  const [error, setError] = useState<string | null>(null);
  if (!target) return <span>{children}</span>;
  const activate = (event: MouseEvent<HTMLAnchorElement>) => {
    if (event.button !== 0 && event.button !== 1) return;
    if (target.kind === "anchor") return;
    event.preventDefault();
    setError(null);
    if (target.kind === "file") {
      useTermStore.getState().openDocTab(target.path);
    } else {
      void platform.opener.openExternal(target.url).catch((err: unknown) => setError(String(err)));
    }
  };
  return <>
    <a className="sv-link" href={target.kind === "file" ? target.path : target.url}
      target={target.kind === "anchor" ? undefined : "_blank"} rel="noreferrer"
      onClick={activate} onAuxClick={activate}>{children}</a>
    {error && <span role="alert"> {error}</span>}
  </>;
}
