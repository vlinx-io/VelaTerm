//! Shell mode in the composer: a message starting with `!` is a command for the session's shell, not a
//! prompt. The pure parts live here so the routing can be tested without rendering the pane.

import type { I18nKey } from "../../../i18n";

/** What the agent is told when a command ends, read back from the tagged message. */
export interface ShellContext {
  command: string;
  stdout: string;
  stderr: string;
  exitCode?: number;
  cancelled: boolean;
}

/**
 * Read a composer submission as a shell command.
 *
 * `!` after optional leading whitespace, then optional spaces, then the command. `null` for anything
 * else; `{ command: "" }` for a bare `!`, so the caller can show the hint instead of sending nothing.
 */
export function parseShellSubmission(text: string): { command: string } | null {
  const match = /^\s*!\s*([\s\S]*)$/.exec(text);
  return match ? { command: match[1].trim() } : null;
}

const TRUNCATED_NOTE = "[VelaTerm: earlier output truncated]";
const CANCELLED_LINE = "Command cancelled by the user";

/**
 * Read a shell-mode context message, the mirror of the backend parser. Used for queued items, which
 * still carry the raw tagged text while they wait for the running turn to end.
 */
export function parseShellContext(text: string): ShellContext | null {
  // Mirror of the backend parser: first input seam, LAST stdout/stderr seam, closing tag at the end, so
  // command output that contains the tags itself (grepping this repository does) still reads back.
  let rest = text.trimStart();
  if (!rest.startsWith("<bash-input>")) return null;
  rest = rest.slice("<bash-input>".length);
  let seam = rest.indexOf("</bash-input>\n<bash-stdout>");
  let seamLength = "</bash-input>\n<bash-stdout>".length;
  if (seam < 0) { seam = rest.indexOf("</bash-input><bash-stdout>"); seamLength = "</bash-input><bash-stdout>".length; }
  if (seam < 0) return null;
  const command = rest.slice(0, seam);
  rest = rest.slice(seam + seamLength).trimEnd();
  let rawStdout: string;
  let rawStderr = "";
  if (rest.endsWith("</bash-stderr>")) {
    const body = rest.slice(0, -"</bash-stderr>".length);
    const split = body.lastIndexOf("</bash-stdout><bash-stderr>");
    if (split < 0) return null;
    rawStdout = body.slice(0, split);
    rawStderr = body.slice(split + "</bash-stdout><bash-stderr>".length);
  } else if (rest.endsWith("</bash-stdout>")) {
    rawStdout = rest.slice(0, -"</bash-stdout>".length);
  } else {
    return null;
  }
  const stdout = stripNote(rawStdout);
  let stderr = stripNote(rawStderr);
  let exitCode: number | undefined = 0;
  let cancelled = false;
  const cut = stderr.lastIndexOf("\n");
  const last = cut >= 0 ? stderr.slice(cut + 1) : stderr;
  const body = cut >= 0 ? stderr.slice(0, cut) : "";
  const exit = /^Exit code (-?\d+)$/.exec(last);
  if (last === CANCELLED_LINE) {
    cancelled = true;
    exitCode = undefined;
    stderr = body;
  } else if (exit) {
    exitCode = Number(exit[1]);
    stderr = body;
  }
  return { command, stdout, stderr, exitCode, cancelled };
}

function stripNote(stream: string): string {
  if (!stream.startsWith(TRUNCATED_NOTE)) return stream;
  const rest = stream.slice(TRUNCATED_NOTE.length);
  return rest.startsWith("\n") ? rest.slice(1) : rest;
}

/** The composer's wording for a stable backend refusal, or `null` for a message to show verbatim. */
export function shellErrorKey(error: unknown): I18nKey | null {
  const text = String(error);
  if (text.includes("chat_shell_running")) return "chat.shell.alreadyRunning";
  if (text.includes("chat_shell_empty")) return "chat.shell.emptyCommand";
  return null;
}
