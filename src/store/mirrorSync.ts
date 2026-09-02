//! Mirror mode wiring: publish this client's layout, follow what other clients publish.
//!
//! The terminal stream was already shared — one PTY, one byte stream, size arbitrated by the owner model
//! (communication doc §6.3). What was not shared is the arrangement around it, which lived only in each
//! client's `localStorage`. This module closes that gap so a browser opened over the LAN shows the same
//! tabs and splits as the desktop it connects to.
//!
//! Three rules keep two-way sync from turning into a fight:
//!
//! 1. **Last writer wins.** A push always takes effect; `rev` only orders frames and drops late ones. Two
//!    people rearranging at the same moment converge on whoever moved last, exactly like typing into a
//!    shared session.
//! 2. **No echo.** The backend stamps each push with the publisher's connection ID, so a client recognizes
//!    and drops its own broadcast. Belt and braces, every publish is also compared byte-for-byte against the
//!    last synced serialization, so an apply can never bounce straight back out as a fresh push.
//! 3. **No focus theft.** Applying a peer's layout bumps `mirrorFocusSeq`, and terminal views skip their one
//!    automatic focus for that update. The window rearranges under you; the keyboard stays where it was.
//!
//! Phones do not participate at all: the narrow two-level navigation is a different shape of UI, and copying
//! a desktop split tree onto it helps nobody.

import { onMirrorLayout, onMirrorMode, onRemoteClients } from "../ipc/events";
import { mirrorGet, mirrorPush } from "../ipc/mirror";
import { getClientSource, isTauri } from "../ipc/transport";
import { wsClient } from "../ipc/wsClient";
import { isMobileView } from "../mobile/detect";
import { markMirrorDetach } from "../ipc/commands";
import { settleFirstMirrorAlign } from "./mirrorAlign";
import {
  buildMirrorLayout,
  layoutSessionIds,
  sanitizeMirrorLayout,
} from "./mirrorLayout";
import { useTermStore } from "./termStore";

/** Coalescing window for local edits. Long enough to swallow a drag's worth of resize events, short enough
 * that a peer sees a tab switch as immediate. */
const PUSH_DEBOUNCE_MS = 150;

/** Retry budget for the initial alignment, which commonly runs before the socket is ready. */
const ALIGN_RETRIES = 5;

/** First retry delay; each further attempt doubles it. */
const ALIGN_RETRY_BASE_MS = 500;

/** Store fields whose change can alter the published snapshot; anything else never schedules a push. */
const WATCHED = [
  "openTabs",
  "liveTabs",
  "pinnedTabs",
  "activeTabId",
  "lastActiveSessionTabId",
  "activeSessionId",
  "focusedPaneId",
  "paneTrees",
  "ephemeralSessions",
  "docTabs",
  "browserTabs",
  "selection",
  "inspectTarget",
  "leftCollapsed",
  "rightCollapsed",
  "inspectorTab",
  "sidebarTreeViews",
  "sidebarTreeTabs",
  "primarySidebarTreeViewId",
  "activeSidebarTreeViewId",
] as const;

/** Serialize the arrangement this client would publish right now. */
function currentJson(): string {
  return JSON.stringify(buildMirrorLayout(useTermStore.getState()));
}

/**
 * Start following and publishing the shared layout. Returns a disposer.
 *
 * Safe to call on any client: phones opt out here rather than at every call site.
 */
export function startMirrorSync(): () => void {
  if (isMobileView()) {
    // Nothing will ever align here, so release the initial layout restore instead of making it wait out
    // its whole budget before falling back to local storage.
    settleFirstMirrorAlign(false);
    return () => {};
  }

  const store = useTermStore;
  /** Serialization of the last arrangement known to be in sync; a push that matches it is skipped. */
  let synced: string | null = null;
  /** Highest revision seen, so a frame that arrives out of order cannot undo a newer one. */
  let lastRev = 0;
  /** True while a peer's layout is being written into the store, so that write does not schedule a push. */
  let applying = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let stopped = false;
  /** True between sending a push and learning its revision. */
  let pushing = false;
  /** A peer frame that arrived while our own push was in flight, held until the push reports its revision. */
  let deferred: { state: unknown; rev: number } | null = null;
  let alignTimer: ReturnType<typeof setTimeout> | undefined;
  let alignAttempt = 0;

  const publish = () => {
    if (stopped || !store.getState().mirrorEnabled) return;
    const json = currentJson();
    if (json === synced) return;
    synced = json;
    pushing = true;
    mirrorPush(JSON.parse(json)).then(
      (snap) => {
        pushing = false;
        lastRev = Math.max(lastRev, snap.rev);
        // A frame held during the push is only newer than our own arrangement if the service ordered it
        // after us. Applying an older one would leave the two clients holding each other's layout, with
        // both baselines matching so neither ever publishes again.
        const held = deferred;
        deferred = null;
        if (held && held.rev > snap.rev) apply(held.state, held.rev);
      },
      () => {
        pushing = false;
        deferred = null;
        // The service may be down or the socket reconnecting. Forget the baseline so the next local
        // change retries instead of assuming this arrangement was published.
        synced = null;
      },
    );
  };

  const schedule = () => {
    clearTimeout(timer);
    timer = setTimeout(publish, PUSH_DEBOUNCE_MS);
  };

  const apply = (raw: unknown, rev: number) => {
    const layout = sanitizeMirrorLayout(raw);
    if (!layout) return;
    lastRev = Math.max(lastRev, rev);
    // Sessions this window is about to stop showing leave by mirror, not by anyone's decision to close
    // them. Desktop unmount otherwise means kill, which would let a remote client end a process merely by
    // closing its own tab; the mark downgrades that unmount to a detach.
    const incoming = layoutSessionIds(layout);
    const leaving = [
      ...layoutSessionIds(buildMirrorLayout(store.getState())),
    ].filter((id) => !incoming.has(id));
    if (leaving.length) markMirrorDetach(leaving);
    applying = true;
    try {
      store.getState().applyMirrorLayout(layout);
    } finally {
      applying = false;
    }
    // Take the baseline from our own serialization of the result, not from the peer's bytes: two clients
    // can order map keys differently, and comparing against their ordering would push a pointless echo.
    synced = currentJson();
  };

  /** Align with whatever is already published; publish our own arrangement when nothing is yet. */
  const align = () => {
    // Start a fresh revision epoch. The service may have restarted with its counter back at zero while
    // this page stayed open, and a client that kept its old high-water mark would dismiss every frame
    // that follows as stale and silently stop mirroring.
    lastRev = 0;
    clearTimeout(alignTimer);
    mirrorGet().then(
      (status) => {
        if (stopped) {
          settleFirstMirrorAlign(false);
          return;
        }
        alignAttempt = 0;
        // Before the enabled check: the host badge needs the count even in the moment mirroring is off,
        // and this reply is the only place a freshly started client learns it.
        store
          .getState()
          .setRemoteClients(status.clients ?? 0, status.clientList ?? []);
        store.getState().setMirrorEnabled(status.enabled);
        if (!status.enabled) {
          settleFirstMirrorAlign(false);
          return;
        }
        // A broadcast can overtake this reply. Applying the snapshot unconditionally would then roll the
        // client back to the older arrangement and, because the baseline follows, leave it there.
        let applied = false;
        if (status.state) {
          if (status.rev >= lastRev) {
            apply(status.state, status.rev);
            applied = true;
          }
        } else publish();
        // Tell the initial layout restore whether an arrangement has landed. Reporting after the apply
        // keeps the restore from reading a half-written store.
        settleFirstMirrorAlign(applied);
      },
      () => {
        // Usually a socket that is not ready yet at mount. Nothing else retries: pushes are gated on
        // mirrorEnabled, which stays false here, so without a retry this page never mirrors again.
        if (stopped || alignAttempt >= ALIGN_RETRIES) {
          settleFirstMirrorAlign(false);
          return;
        }
        const delay = ALIGN_RETRY_BASE_MS * 2 ** alignAttempt;
        alignAttempt += 1;
        alignTimer = setTimeout(align, delay);
      },
    );
  };

  const unsubStore = store.subscribe((state, prev) => {
    if (applying || stopped || !state.mirrorEnabled) return;
    if (WATCHED.every((k) => state[k] === prev[k])) return;
    schedule();
  });

  const unlisteners: Array<() => void> = [];
  void onMirrorLayout((snap) => {
    if (stopped || !store.getState().mirrorEnabled) return;
    if (snap.source === getClientSource()) return; // Our own broadcast.
    if (snap.rev <= lastRev || !snap.state) return;
    // Our own push has not reported its revision yet, so we cannot tell whether this frame is newer or
    // older than what we just sent. Hold it and decide once the push resolves.
    if (pushing) {
      if (!deferred || snap.rev > deferred.rev)
        deferred = { state: snap.state, rev: snap.rev };
      return;
    }
    apply(snap.state, snap.rev);
  }).then((un) => (stopped ? un() : unlisteners.push(un)));
  // Who else is attached. It rides along here rather than in its own module because it exists for the same
  // reason mirroring does: on a host, a peer's rearranging is invisible until you know a peer is there.
  void onRemoteClients((count, clients) => {
    if (stopped) return;
    store.getState().setRemoteClients(count, clients);
  }).then((un) => (stopped ? un() : unlisteners.push(un)));
  void onMirrorMode((enabled) => {
    if (stopped) return;
    store.getState().setMirrorEnabled(enabled);
    if (enabled) align();
    else synced = null;
  }).then((un) => (stopped ? un() : unlisteners.push(un)));

  // Reconnecting means the backend may be a different process, with its revision counter back at zero.
  // Realigning is what tells this page to follow again instead of dropping every frame as stale while
  // still publishing its own. Desktop IPC has no such reconnect.
  if (!isTauri) {
    unlisteners.push(
      wsClient.onConnState((state) => {
        if (state === "online" && !stopped) align();
      }),
    );
  }

  align();

  return () => {
    stopped = true;
    clearTimeout(timer);
    clearTimeout(alignTimer);
    unsubStore();
    for (const un of unlisteners) un();
  };
}
