//! Text for paths dropped from the OS file manager onto a terminal or the chat composer.

const isWindows =
  typeof navigator !== "undefined" && /Win/i.test(navigator.userAgent) && !/Mac/i.test(navigator.userAgent);

/**
 * Quote a path for a shell prompt. Unix shells get Terminal.app's form, a backslash before every character the
 * shell would treat specially, while letters and digits of any script (CJK names included) stay readable.
 * Windows shells take a path containing such characters in double quotes instead.
 */
export function shellQuotePath(path: string, windows = isWindows): string {
  if (windows) return /[\s&()^%!,;=']/.test(path) ? `"${path}"` : path;
  return path.replace(/[^\p{L}\p{N}_\-.,/:@+%~]/gu, "\\$&");
}

/** Terminal input for dropped paths: each quoted, separated by spaces, with a trailing space to keep typing. */
export function terminalDropText(paths: string[], windows = isWindows): string {
  return paths.map((p) => shellQuotePath(p, windows)).join(" ") + " ";
}

/** Composer text for dropped paths: plain paths, in double quotes only where whitespace would split one. */
export function chatDropText(paths: string[]): string {
  return paths.map((p) => (/\s/.test(p) ? `"${p}"` : p)).join(" ");
}

/** The dropped paths other than the given files, matched by name; used to leave images to the image flow. */
export function pathsExcept(paths: string[], files: File[]): string[] {
  const names = new Set(files.map((f) => f.name));
  return paths.filter((p) => !names.has(p.split(/[\\/]/).pop() ?? p));
}
