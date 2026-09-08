//! Where the paths behind an `@` mention come from: one directory at a time, read on demand.
//!
//! A mention is answered by listing the directory it points into, not by indexing the whole tree. A tree
//! walk would have to be built before the first keystroke and rebuilt as files change, and on a repository
//! of any size it costs far more than it saves — the directory being typed is the only one whose contents
//! the list can show anyway.
//!
//! Listings are kept for a few seconds so that walking back up and down a tree does not re-read the same
//! folders, and long enough is short enough that a file written during the conversation still turns up.

import { useEffect, useRef, useState } from "react";

import { listDir } from "../../../ipc/info";
import type { FileCandidate } from "./completion";

/** Never offered: version-control bookkeeping and build output nobody means to point an agent at. */
const SKIP = new Set([".git", "node_modules", "target", ".vlx-worktrees"]);

/** How long a listing is reused before the directory is read again. */
const FRESH_MS = 10_000;

/** Shared empty result, so a render with no mention keeps the same array identity. */
const NONE: FileCandidate[] = [];

interface Listing {
  at: number;
  entries: FileCandidate[];
}

/** Join the working directory with a relative directory that is either empty or ends in `/`. */
function under(cwd: string, dir: string): string {
  const base = cwd.replace(/[\\/]+$/, "");
  const rel = dir.replace(/\/+$/, "");
  return rel ? `${base}/${rel}` : base;
}

/**
 * Children of `dir` under `cwd`, or an empty list while they are being read.
 *
 * `dir` is relative to `cwd` and either empty or ends in `/`; null means no mention is being written, which
 * is what keeps an idle composer from reading directories nobody asked about.
 */
export function useMentionFiles(cwd: string | undefined, dir: string | null): FileCandidate[] {
  const cache = useRef(new Map<string, Listing>());
  const [, bump] = useState(0);

  // Listings are keyed by a path relative to the working directory, so they mean nothing once it changes.
  useEffect(() => {
    cache.current.clear();
  }, [cwd]);

  useEffect(() => {
    if (!cwd || dir === null) return;
    const held = cache.current.get(dir);
    if (held && Date.now() - held.at < FRESH_MS) return;
    let cancelled = false;
    const remember = (entries: FileCandidate[]) => {
      if (cancelled) return;
      cache.current.set(dir, { at: Date.now(), entries });
      bump((n) => n + 1);
    };
    void listDir(under(cwd, dir))
      .then((entries) =>
        remember(
          entries
            .filter((e) => !SKIP.has(e.name))
            .map((e) => ({ path: dir + e.name, isDir: e.isDir })),
        ),
      )
      // A directory that cannot be read offers nothing, and remembering that is what stops the next
      // keystroke from asking again.
      .catch(() => remember([]));
    return () => {
      cancelled = true;
    };
  }, [cwd, dir]);

  return dir === null ? NONE : (cache.current.get(dir)?.entries ?? NONE);
}
