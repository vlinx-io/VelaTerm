//! Messages from the script the backend injects into every artifact page.
//!
//! The page is untrusted, so its messages are accepted only from the iframe we created and only in the
//! shapes listed here, with every field type-checked and clipped. They feed displays (error count, console,
//! blocked-request banner, scroll restore, the picked element and the option list) and never trigger an action
//! by themselves: a picked element only opens the comment box, and nothing reaches a session until the user sends it.

/** Source tag the injected script puts on every message. */
export const BRIDGE_SOURCE = "vlx-artifact";

/** Source tag on messages this window posts to the page's review script. */
export const HOST_SOURCE = "vlx-artifact-host";

/** An element the user picked in the page to comment on. */
export interface PickedTarget {
  /** CSS selector the page script built for it; used to pin and reveal it again. */
  selector: string;
  tag: string;
  /** Its visible text, or its label, clipped. */
  text: string;
  /** The design option it sits in, if any. */
  option: string | null;
}

/** A design option the page marks with `data-vlx-option`. */
export interface ArtifactOption {
  id: string;
  label: string;
}

/** Longest message text kept; the script already clips, this guards against a page posting its own. */
const MAX_TEXT = 2000;

export type BridgeMessage =
  | { type: "ready" }
  | { type: "console"; level: "error" | "warn"; text: string }
  | { type: "error"; text: string; line: number }
  | { type: "blocked"; uri: string; directive: string }
  | { type: "scroll"; y: number }
  | ({ type: "picked" } & PickedTarget)
  | { type: "pickCancel" }
  | { type: "options"; options: ArtifactOption[] };

function clip(v: unknown, max = MAX_TEXT): string | null {
  return typeof v === "string" ? v.slice(0, max) : null;
}

function int(v: unknown): number | null {
  return typeof v === "number" && Number.isFinite(v) && v >= 0 ? Math.min(Math.floor(v), 10_000_000) : null;
}

/** Validate one posted message; null for anything that is not a well-formed bridge message. */
export function parseBridgeMessage(data: unknown): BridgeMessage | null {
  if (!data || typeof data !== "object") return null;
  const m = data as Record<string, unknown>;
  if (m.source !== BRIDGE_SOURCE) return null;
  switch (m.type) {
    case "ready":
      return { type: "ready" };
    case "console": {
      const text = clip(m.text);
      if (text === null || (m.level !== "error" && m.level !== "warn")) return null;
      return { type: "console", level: m.level, text };
    }
    case "error": {
      const text = clip(m.text);
      if (text === null) return null;
      return { type: "error", text, line: int(m.line) ?? 0 };
    }
    case "blocked": {
      const uri = clip(m.uri, 300);
      const directive = clip(m.directive, 64);
      if (uri === null || directive === null) return null;
      return { type: "blocked", uri, directive };
    }
    case "scroll": {
      const y = int(m.y);
      return y === null ? null : { type: "scroll", y };
    }
    case "picked": {
      const selector = clip(m.selector, 500);
      const tag = clip(m.tag, 40);
      const text = clip(m.text, 120);
      if (!selector || tag === null || text === null) return null;
      const option = m.option === null || m.option === undefined ? null : clip(m.option, 40);
      return { type: "picked", selector, tag, text, option: option || null };
    }
    case "pickCancel":
      return { type: "pickCancel" };
    case "options": {
      if (!Array.isArray(m.options)) return null;
      const options: ArtifactOption[] = [];
      for (const raw of m.options.slice(0, 40)) {
        const o = raw as Record<string, unknown> | null;
        const id = clip(o?.id, 40);
        const label = clip(o?.label, 80);
        if (id && label !== null && !options.some((x) => x.id === id)) options.push({ id, label });
      }
      return { type: "options", options };
    }
    default:
      return null;
  }
}

/** Messages to the page's review script; posted to our own frame only. */
export type HostMessage =
  | { type: "pick"; on: boolean }
  | { type: "marks"; marks: { n: number; selector: string }[] }
  | { type: "reveal"; option?: string; selector?: string };

export function postToFrame(frame: HTMLIFrameElement | null, message: HostMessage): void {
  // The frame has an opaque origin, so "*" is the only target that reaches it; the message carries no secrets.
  frame?.contentWindow?.postMessage({ ...message, source: HOST_SOURCE }, "*");
}

/** One line in the console drawer. */
export interface ConsoleEntry {
  id: number;
  level: "error" | "warn";
  text: string;
  line?: number;
}

/** Entries kept per page load; the oldest are dropped first. */
export const MAX_CONSOLE = 200;

/** Append an entry, keeping the newest `MAX_CONSOLE`. */
export function pushConsole(list: ConsoleEntry[], entry: ConsoleEntry): ConsoleEntry[] {
  const next = list.length >= MAX_CONSOLE ? list.slice(list.length - MAX_CONSOLE + 1) : list.slice();
  next.push(entry);
  return next;
}

/** Text handed to an agent: the page path and its errors, so it can fix them without asking. */
export function consoleReport(path: string, entries: ConsoleEntry[]): string {
  const lines = entries
    .filter((e) => e.level === "error")
    .slice(-20)
    .map((e) => `- ${e.text}${e.line ? ` (line ${e.line})` : ""}`);
  return `The page ${path} reports these errors in the preview:\n${lines.join("\n")}\nPlease fix them.`;
}
