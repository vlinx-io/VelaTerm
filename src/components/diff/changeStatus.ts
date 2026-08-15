//! Shared presentation for git change statuses: the single-letter badge and its color.
//! Used by both the Changes modal and the right panel's Git tab so the two stay consistent.

/** Map a backend `ChangedFile.status` to a badge letter and color. */
export const STATUS_META: Record<string, { letter: string; color: string }> = {
  added: { letter: "A", color: "var(--green, #3fb950)" },
  modified: { letter: "M", color: "var(--yellow, #d29922)" },
  deleted: { letter: "D", color: "var(--danger, #e05252)" },
  untracked: { letter: "U", color: "var(--text-muted)" },
  renamed: { letter: "R", color: "var(--cyan, #4aa)" },
};

/** Badge letter for a status, falling back to `?` for values this build does not know. */
export function statusLetter(status: string): string {
  return STATUS_META[status]?.letter ?? "?";
}
