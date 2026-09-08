//! Tool-call cards for the session view.
//!
//! A transcript records a tool call as a name plus a blob of JSON, which is exactly the shape a reader does
//! not want. What matters differs per tool: for Bash it is the command line, for Edit it is what changed, for
//! Read it is which file. So each known tool gets a one-line summary shown on the collapsed card, and a body
//! that renders the fields that tool actually uses. Unknown tools — plugins, MCP servers, or a tool added
//! after this code was written — fall back to formatted JSON, which is unglamorous but never wrong.

import { useState, type ReactNode } from "react";

import { useT } from "../../../i18n";
import { highlight } from "../../RightPanel/highlight";
import { Markdown } from "./markdown";

/** Longest output shown before the card offers to expand. Keeps a 200 KB build log from freezing a pane. */
const OUTPUT_CLAMP = 2000;

/** Read a string field from a tool's input, which is untyped JSON from the recording. */
function str(input: unknown, key: string): string | undefined {
  if (!input || typeof input !== "object") return undefined;
  const v = (input as Record<string, unknown>)[key];
  return typeof v === "string" ? v : undefined;
}

function arr(input: unknown, key: string): unknown[] | undefined {
  if (!input || typeof input !== "object") return undefined;
  const v = (input as Record<string, unknown>)[key];
  return Array.isArray(v) ? v : undefined;
}

/** Shorten an absolute path against the session's working directory, so cards read like an editor's tab bar. */
export function relPath(path: string, cwd?: string): string {
  if (cwd && path.startsWith(cwd)) {
    const rest = path.slice(cwd.length).replace(/^[/\\]/, "");
    if (rest) return rest;
  }
  return path;
}

/** The paths a unified diff touches, in order of first appearance, from its `+++` headers. */
export function diffFiles(diff: string): string[] {
  const files: string[] = [];
  for (const line of diff.split("\n")) {
    if (!line.startsWith("+++ ")) continue;
    const path = line.slice(4).trim().replace(/^b\//, "");
    if (path && path !== "/dev/null" && !files.includes(path)) files.push(path);
  }
  return files;
}

/**
 * The one line shown beside the tool name on a collapsed card.
 *
 * Returns undefined when the tool has nothing worth summarizing, in which case the card shows only its name.
 */
export function toolSummary(tool: string | undefined, input: unknown, cwd?: string): string | undefined {
  if (!tool) return undefined;
  const path = str(input, "file_path") ?? str(input, "path") ?? str(input, "notebook_path");
  switch (tool) {
    case "Bash":
    case "BashOutput":
    case "shell":
      return str(input, "command") ?? str(input, "description");
    case "Read":
    case "Write":
    case "Edit":
    case "MultiEdit":
    case "NotebookEdit":
    case "FileChange":
      return path ? relPath(path, cwd) : undefined;
    case "Glob":
      return str(input, "pattern");
    case "Grep":
      return str(input, "pattern");
    case "WebFetch":
      return str(input, "url");
    case "WebSearch":
      return str(input, "query");
    // The subagent tool has been called both things across CLI versions; 2.1.258 reports `Agent`.
    case "Task":
    case "Agent":
      return str(input, "description") ?? str(input, "subagent_type");
    case "Skill":
      return str(input, "skill") ?? str(input, "name");
    case "TodoWrite":
      return undefined;
    case "ExitPlanMode":
      return undefined;
    // Codex's aggregate of every file changed in a turn: the files it touches are the summary.
    case "Diff":
      return diffFiles(str(input, "diff") ?? "").join(", ") || undefined;
    default:
      return path ? relPath(path, cwd) : str(input, "command") ?? str(input, "description");
  }
}

/** The expanded body of a tool card. */
export function ToolBody({
  tool,
  input,
  output,
  isError,
}: {
  tool?: string;
  input: unknown;
  output?: string;
  isError: boolean;
}): ReactNode {
  const t = useT();
  const sections: ReactNode[] = [];

  switch (tool) {
    case "Bash":
    case "shell": {
      const cmd = str(input, "command");
      if (cmd) sections.push(<CodeSection key="cmd" text={cmd} />);
      break;
    }
    case "Edit": {
      sections.push(
        <Diff key="diff" before={str(input, "old_string") ?? ""} after={str(input, "new_string") ?? ""} />,
      );
      break;
    }
    case "MultiEdit": {
      const edits = arr(input, "edits") ?? [];
      edits.forEach((e, i) =>
        sections.push(
          <Diff key={`d${i}`} before={str(e, "old_string") ?? ""} after={str(e, "new_string") ?? ""} />,
        ),
      );
      break;
    }
    case "Write": {
      const content = str(input, "content");
      if (content) sections.push(<CodeSection key="content" text={content} />);
      break;
    }
    case "Diff": {
      const diff = str(input, "diff");
      if (diff) sections.push(<CodeSection key="diff" text={diff} />);
      break;
    }
    case "FileChange": {
      const changes = arr(input, "changes") ?? [];
      changes.forEach((change, i) => {
        const path = str(change, "path");
        const diff = str(change, "diff");
        if (!path && !diff) return;
        sections.push(
          <div className="sv-tool-section" key={`change-${i}`}>
            {path ? <div className="sv-tool-text">{path}</div> : null}
            {diff ? <CodeSection text={diff} /> : null}
          </div>,
        );
      });
      break;
    }
    case "TodoWrite": {
      const todos = arr(input, "todos") ?? [];
      sections.push(
        <div className="sv-todos" key="todos">
          {todos.map((todo, i) => {
            const status = str(todo, "status") ?? "pending";
            return (
              <div className={`sv-todo sv-todo-${status}`} key={i}>
                <span className="sv-todo-mark">
                  {status === "completed" ? "☑" : status === "in_progress" ? "▸" : "☐"}
                </span>
                <span>{str(todo, "content") ?? str(todo, "activeForm") ?? ""}</span>
              </div>
            );
          })}
        </div>,
      );
      break;
    }
    case "ExitPlanMode": {
      const plan = str(input, "plan");
      if (plan)
        sections.push(
          <div className="sv-plan" key="plan">
            <Markdown text={plan} />
          </div>,
        );
      break;
    }
    case "Task":
    case "Agent": {
      const prompt = str(input, "prompt");
      if (prompt) sections.push(<TextSection key="prompt" text={prompt} />);
      break;
    }
    case "Read":
    case "Glob":
    case "Grep":
    case "WebFetch":
    case "WebSearch":
      // The summary line already carries the whole request; only the result is worth showing.
      break;
    default: {
      // An unfamiliar tool: show its input as formatted JSON rather than guessing at its fields.
      const json = safeJson(input);
      if (json && json !== "{}") sections.push(<CodeSection key="json" text={json} />);
    }
  }

  if (output !== undefined && output !== "") {
    sections.push(
      <Output key="out" text={output} isError={isError} label={isError ? t("session.toolFailed") : undefined} />,
    );
  }

  if (sections.length === 0) return <div className="sv-tool-empty">{t("session.toolNoDetail")}</div>;
  return <>{sections}</>;
}

/** Pretty-print tool input, tolerating values that cannot be serialized. */
function safeJson(v: unknown): string {
  try {
    return JSON.stringify(v, null, 2) ?? "";
  } catch {
    return String(v);
  }
}

function CodeSection({ text }: { text: string }): ReactNode {
  return (
    <div className="sv-code sv-tool-section">
      <pre>{highlight(text)}</pre>
    </div>
  );
}

function TextSection({ text }: { text: string }): ReactNode {
  return <div className="sv-tool-text sv-tool-section">{text}</div>;
}

/** Tool output, clamped until the reader asks for the rest. */
function Output({ text, isError, label }: { text: string; isError: boolean; label?: string }): ReactNode {
  const t = useT();
  const [expanded, setExpanded] = useState(false);
  const long = text.length > OUTPUT_CLAMP;
  const shown = expanded || !long ? text : text.slice(0, OUTPUT_CLAMP);
  return (
    <div className={"sv-tool-section sv-out" + (isError ? " sv-out-error" : "")}>
      {label ? <div className="sv-out-label">{label}</div> : null}
      <pre>{shown}</pre>
      {long ? (
        <button className="sv-more" onClick={() => setExpanded((v) => !v)}>
          {expanded ? t("session.showLess") : t("session.showMore", text.length - OUTPUT_CLAMP)}
        </button>
      ) : null}
    </div>
  );
}

/** One line of a rendered diff. */
interface DiffLine {
  sign: " " | "+" | "-";
  text: string;
}

/**
 * Line diff between the text an edit replaced and the text it wrote.
 *
 * Longest-common-subsequence over whole lines: with old and new strings both usually a handful of lines, the
 * quadratic table costs nothing, and unlike a naive prefix/suffix trim it survives edits in the middle.
 */
export function diffLines(before: string, after: string): DiffLine[] {
  const a = before === "" ? [] : before.split("\n");
  const b = after === "" ? [] : after.split("\n");
  // Guard against a pathological case (a whole file rewritten through Edit): fall back to a flat replacement.
  if (a.length * b.length > 250_000) {
    return [
      ...a.map((text) => ({ sign: "-" as const, text })),
      ...b.map((text) => ({ sign: "+" as const, text })),
    ];
  }
  const lcs: number[][] = Array.from({ length: a.length + 1 }, () => new Array(b.length + 1).fill(0));
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      out.push({ sign: " ", text: a[i] });
      i++;
      j++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      out.push({ sign: "-", text: a[i++] });
    } else {
      out.push({ sign: "+", text: b[j++] });
    }
  }
  while (i < a.length) out.push({ sign: "-", text: a[i++] });
  while (j < b.length) out.push({ sign: "+", text: b[j++] });
  return out;
}

function Diff({ before, after }: { before: string; after: string }): ReactNode {
  const lines = diffLines(before, after);
  return (
    <div className="sv-diff sv-tool-section">
      {lines.map((l, i) => (
        <div className={`sv-diff-line sv-diff-${l.sign === "+" ? "add" : l.sign === "-" ? "del" : "ctx"}`} key={i}>
          <span className="sv-diff-sign">{l.sign}</span>
          <span className="sv-diff-text">{l.text || " "}</span>
        </div>
      ))}
    </div>
  );
}
