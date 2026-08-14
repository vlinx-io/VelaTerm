//! Badge presentation for a `ChangedFile` status, shared by the Changes modal and the right panel's
//! Git tab so both render the same letter for the same status.

/** Map status to a badge letter and color. */
export const STATUS_META: Record<string, { letter: string; color: string }> = {
  added: { letter: "A", color: "var(--green, #3fb950)" },
  modified: { letter: "M", color: "var(--yellow, #d29922)" },
  deleted: { letter: "D", color: "var(--danger, #e05252)" },
  untracked: { letter: "U", color: "var(--text-muted)" },
  renamed: { letter: "R", color: "var(--cyan, #4aa)" },
};

/** Badge for one status. Falls back to modified so a status added on the Rust side still renders. */
export function statusMeta(status: string): { letter: string; color: string } {
  return STATUS_META[status] ?? STATUS_META.modified;
}
