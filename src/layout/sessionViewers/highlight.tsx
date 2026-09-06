//! Case-insensitive keyword highlighting that wraps matching text fragments in <mark>.
//! Shared by transcript navigation and global-search snippets for a consistent appearance.
//!
//! Two backgrounds matching xterm recording search (see SEARCH_OPTS in RecordingViewer):
//! - match: amber #d29922, indicating another match that is not currently selected.
//! - active: brighter orange #f0883e, indicating the match currently being viewed.
//! Left-tree snippets and non-current matching messages use match; the current message in the right preview uses active.

import React from "react";

/** Regular-match background, matching xterm recording search's matchBackground #d29922. */
const MATCH_STYLE: React.CSSProperties = {
  backgroundColor: "rgba(210,153,34,0.40)",
  borderRadius: 2,
};

/** More prominent current-match background, matching xterm recording search's activeMatchBackground #f0883e. */
const ACTIVE_STYLE: React.CSSProperties = {
  backgroundColor: "rgba(240,136,62,0.62)",
  borderRadius: 2,
};

/**
 * Wrap every case-insensitive occurrence of any of `terms` in `text` with a highlighted <mark>, preserving all
 * other text. A single string is treated as one term. Search results pass the literals the backend actually
 * matched (see SearchMatch.matched), so what is marked is what the index matched. Overlapping candidates
 * resolve to the earliest occurrence, and the longest term at that position. Return the original text when
 * no term is nonempty. `active=true` uses the more prominent current-match background.
 */
export function highlightMatches(
  text: string,
  terms: string | string[],
  active = false,
): React.ReactNode {
  const list = (Array.isArray(terms) ? terms : [terms])
    .map((t) => t.trim().toLowerCase())
    .filter((t) => t.length > 0)
    // Longest first so a longer term wins when two start at the same position.
    .sort((a, b) => b.length - a.length);
  if (list.length === 0) return text;
  const style = active ? ACTIVE_STYLE : MATCH_STYLE;
  const lower = text.toLowerCase();
  const out: React.ReactNode[] = [];
  let i = 0;
  let key = 0;
  while (i < text.length) {
    let pos = -1;
    let len = 0;
    for (const term of list) {
      const p = lower.indexOf(term, i);
      if (p >= 0 && (pos < 0 || p < pos)) {
        pos = p;
        len = term.length;
      }
    }
    if (pos < 0) {
      out.push(text.slice(i));
      break;
    }
    if (pos > i) out.push(text.slice(i, pos));
    out.push(
      <mark key={key++} style={style}>
        {text.slice(pos, pos + len)}
      </mark>,
    );
    i = pos + len;
  }
  return out;
}
