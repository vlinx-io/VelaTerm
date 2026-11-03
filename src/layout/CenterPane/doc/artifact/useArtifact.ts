//! State behind one artifact preview: its grant, reloads on change, the console, and version history.
//!
//! The grant is a lease (see src-tauri/src/artifact.rs), renewed every minute and revoked on unmount. Changes
//! arrive from the same directory watcher the Files tab uses; platforms without one (Linux) poll the page's
//! mtime instead, so only the page itself is watched there. Every reload caused by a change also saves a
//! snapshot, which is where the version list comes from.

import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";

import {
  artifactGrant,
  artifactRenew,
  artifactRevoke,
  artifactSnapshot,
  artifactUrl,
  artifactVersions,
  type ArtifactGrant,
  type ArtifactVersion,
} from "../../../../ipc/artifact";
import { onFsChanged } from "../../../../ipc/events";
import { fsWatch, statFile } from "../../../../ipc/info";
import { parseBridgeMessage, postToFrame, pushConsole, type ArtifactOption, type ConsoleEntry } from "./artifactBridge";
import { artifactReview, reviewMarks, setDraft, setPicking, subscribeArtifactReview } from "./artifactReview";
import { isNetAllowed, setNetAllowed } from "./artifactPrefs";
import { artifactWriter, subscribeArtifactWriters } from "./artifactWriters";

/** Lease renewal for both the grant and the directory watch. */
const RENEW_MS = 60_000;
/** Mtime polling where the platform has no directory watcher. */
const POLL_MS = 2000;
/** Several writes in a burst (an agent saving page and stylesheet) cost one reload. */
const CHANGE_DEBOUNCE_MS = 300;
/** A hidden preview keeps its page this long, so flipping between tabs does not restart it. */
const UNLOAD_AFTER_MS = 30_000;

/** Directories whose churn never affects a page: dependency and build folders, and dot-folders such as .git. */
function ignoredDir(dir: string): boolean {
  const first = dir.split(/[\\/]/)[0];
  return first.startsWith(".") || first === "node_modules" || first === "target" || first === "dist" || first === "build";
}

/** Whether a batch of changed directories (relative to the watched root) can affect the page. */
export function changeAffectsPage(dirs: string[] | null): boolean {
  if (dirs === null) return true;
  return dirs.some((d) => d === "" || (!ignoredDir(d) && d.split(/[\\/]/).length <= 2));
}

export interface ArtifactState {
  /** The iframe URL, or null while no grant is held or the preview is unloaded. */
  url: string | null;
  /** Message about why there is nothing to show, such as a deleted file. */
  error: string | null;
  /** Whether the file is gone (deleted or moved), as opposed to some other failure. */
  missing: boolean;
  /** Whether changes are picked up automatically. */
  live: boolean;
  /** When the page last reloaded because it changed. */
  updatedAt: number | null;
  net: boolean;
  setNet: (allowed: boolean) => void;
  console: ConsoleEntry[];
  errorCount: number;
  warnCount: number;
  clearConsole: () => void;
  /** Requests blocked by the offline policy since the last load. */
  blocked: number;
  reload: () => void;
  /** Saved versions, oldest first; empty until the first snapshot. */
  versions: ArtifactVersion[];
  /** The session that last wrote this page, when known. */
  writer: string | undefined;
  /** Design options the page marks with `data-vlx-option`, in page order. */
  options: ArtifactOption[];
  /** Scroll the page to one of its options and flash it. */
  revealOption: (id: string) => void;
  /** Scroll the page to a commented element and flash it. */
  revealSelector: (selector: string) => void;
  /** Attach to the iframe so messages from other frames are ignored. */
  frameRef: React.RefObject<HTMLIFrameElement | null>;
}

export function useArtifact(path: string, opts: { hidden: boolean; version?: number | null }): ArtifactState {
  const version = opts.version ?? null;
  const [grant, setGrant] = useState<ArtifactGrant | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [missing, setMissing] = useState(false);
  const [nonce, setNonce] = useState(0);
  const [net, setNetState] = useState(() => isNetAllowed(path));
  const [entries, setEntries] = useState<ConsoleEntry[]>([]);
  const [blocked, setBlocked] = useState(0);
  const [live, setLive] = useState(false);
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);
  const [versions, setVersions] = useState<ArtifactVersion[]>([]);
  const [unloaded, setUnloaded] = useState(false);
  const frameRef = useRef<HTMLIFrameElement | null>(null);
  const scrollRef = useRef(0);
  const entryId = useRef(0);
  const [grantEpoch, setGrantEpoch] = useState(0);
  const [options, setOptions] = useState<ArtifactOption[]>([]);
  const pathRef = useRef(path);
  pathRef.current = path;

  const writer = useSyncExternalStore(subscribeArtifactWriters, () => artifactWriter(path));

  // Mint a grant for this page, version and network policy; revoke it when any of them change.
  useEffect(() => {
    let disposed = false;
    let held: ArtifactGrant | null = null;
    setError(null);
    artifactGrant(path, { net, version: version ?? undefined })
      .then((g) => {
        if (disposed) {
          void artifactRevoke(g.grantId).catch(() => {});
          return;
        }
        held = g;
        setMissing(false);
        setGrant(g);
      })
      .catch((e) => {
        if (disposed) return;
        setGrant(null);
        setMissing(true);
        setError(String(e));
      });
    const timer = setInterval(() => {
      if (!held) return;
      // A lapsed lease (the machine slept past it) needs a fresh grant rather than a dead frame.
      void artifactRenew(held.grantId).then((ok) => { if (!ok && !disposed) setGrantEpoch((n) => n + 1); }).catch(() => {});
    }, RENEW_MS);
    return () => {
      disposed = true;
      clearInterval(timer);
      if (held) void artifactRevoke(held.grantId).catch(() => {});
    };
  }, [path, net, version, grantEpoch]);

  // Each new page load starts with an empty console and no blocked requests. Blocked requests count once
  // per address: the header and the meta copy of the policy each report the same violation.
  const blockedUris = useRef(new Set<string>());
  useEffect(() => {
    setEntries([]);
    setBlocked(0);
    blockedUris.current = new Set();
  }, [grant, nonce]);

  // Messages from the injected script, accepted only from our own frame.
  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      if (!frameRef.current || event.source !== frameRef.current.contentWindow) return;
      const msg = parseBridgeMessage(event.data);
      if (!msg) return;
      // "ready" opens every page load, including ones the frame starts by itself when it is remounted
      // (switching between preview and split), so it is where the previous load's messages are dropped.
      if (msg.type === "ready") {
        setEntries([]);
        setBlocked(0);
        setOptions([]);
        blockedUris.current = new Set();
        // A reload starts a fresh page: hand it the pins and pick mode it had.
        const review = artifactReview(pathRef.current);
        postToFrame(frameRef.current, { type: "marks", marks: reviewMarks(review) });
        postToFrame(frameRef.current, { type: "pick", on: review.picking });
      } else if (msg.type === "picked") {
        const { type: _type, ...target } = msg;
        setDraft(pathRef.current, target);
      } else if (msg.type === "pickCancel") setPicking(pathRef.current, false);
      else if (msg.type === "options") setOptions(msg.options);
      else if (msg.type === "scroll") scrollRef.current = msg.y;
      else if (msg.type === "blocked") {
        if (blockedUris.current.has(msg.uri)) return;
        blockedUris.current.add(msg.uri);
        setBlocked(blockedUris.current.size);
      }
      else if (msg.type === "console") setEntries((list) => pushConsole(list, { id: ++entryId.current, level: msg.level, text: msg.text }));
      else if (msg.type === "error") setEntries((list) => pushConsole(list, { id: ++entryId.current, level: "error", text: msg.text, line: msg.line || undefined }));
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, []);

  // Keep the page's pins and pick mode in step with the review draft.
  const review = useSyncExternalStore(subscribeArtifactReview, () => artifactReview(path));
  useEffect(() => {
    postToFrame(frameRef.current, { type: "marks", marks: reviewMarks(review) });
    postToFrame(frameRef.current, { type: "pick", on: review.picking });
  }, [review]);

  const snapshot = useCallback(() => {
    if (version !== null) return;
    void artifactSnapshot(path).then(setVersions).catch(() => {});
  }, [path, version]);

  // The version list, and a first snapshot so the page as opened is version 1.
  useEffect(() => {
    void artifactVersions(path).then(setVersions).catch(() => {});
    snapshot();
  }, [path, snapshot]);

  const reload = useCallback(() => setNonce((n) => n + 1), []);

  // Watch for changes: the directory watcher where there is one, mtime polling where there is not.
  useEffect(() => {
    if (!grant || version !== null) return;
    let disposed = false;
    let watchedRoot: string | null = null;
    let debounce: ReturnType<typeof setTimeout> | null = null;
    let pollTimer: ReturnType<typeof setInterval> | null = null;
    let lastMtime: number | null = null;
    const changed = () => {
      if (debounce) clearTimeout(debounce);
      debounce = setTimeout(() => {
        debounce = null;
        if (disposed) return;
        setUpdatedAt(Date.now());
        setNonce((n) => n + 1);
        snapshot();
      }, CHANGE_DEBOUNCE_MS);
    };
    const poll = () => {
      void statFile(path)
        .then((st) => {
          if (disposed) return;
          if (lastMtime !== null && st.mtimeMs !== lastMtime) changed();
          lastMtime = st.mtimeMs;
        })
        .catch(() => {});
    };
    const watch = () => {
      void fsWatch(grant.root)
        .then((info) => {
          if (disposed) return;
          watchedRoot = info.supported ? info.root : null;
          setLive(true);
          if (!info.supported && !pollTimer) {
            poll();
            pollTimer = setInterval(poll, POLL_MS);
          }
        })
        .catch(() => {
          if (disposed || pollTimer) return;
          setLive(true);
          poll();
          pollTimer = setInterval(poll, POLL_MS);
        });
    };
    watch();
    const renew = setInterval(() => { if (watchedRoot) watch(); }, RENEW_MS);
    const unlisten = onFsChanged(({ root, dirs }) => {
      if (disposed || root !== watchedRoot) return;
      if (changeAffectsPage(dirs)) changed();
    });
    return () => {
      disposed = true;
      setLive(false);
      if (debounce) clearTimeout(debounce);
      if (pollTimer) clearInterval(pollTimer);
      clearInterval(renew);
      void unlisten.then((stop) => stop());
    };
  }, [grant, path, version, snapshot]);

  // Unload a preview that stays hidden, and bring it back when shown again.
  useEffect(() => {
    if (!opts.hidden) {
      setUnloaded(false);
      return;
    }
    const timer = setTimeout(() => setUnloaded(true), UNLOAD_AFTER_MS);
    return () => clearTimeout(timer);
  }, [opts.hidden]);

  const setNet = useCallback(
    (allowed: boolean) => {
      setNetAllowed(path, allowed);
      setNetState(allowed);
    },
    [path],
  );

  // Fixed per load: the scroll position is read once, when a reload builds the next URL, so later scrolling
  // (which updates the ref on every message) cannot change the src and reload the page by accident.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const url = useMemo(() => (grant && !unloaded ? artifactUrl(grant, nonce, scrollRef.current) : null), [grant, nonce, unloaded]);
  return {
    url,
    error,
    missing,
    live,
    updatedAt,
    net,
    setNet,
    console: entries,
    errorCount: entries.filter((e) => e.level === "error").length,
    warnCount: entries.filter((e) => e.level === "warn").length,
    clearConsole: () => setEntries([]),
    blocked,
    reload,
    versions,
    writer,
    options,
    revealOption: (id: string) => postToFrame(frameRef.current, { type: "reveal", option: id }),
    revealSelector: (selector: string) => postToFrame(frameRef.current, { type: "reveal", selector }),
    frameRef,
  };
}
