//! Mobile root component with two-level navigation: session list to full-screen terminal.
//!
//! It shares the desktop data layer (`transport`, `wsClient`, `termStore`, and `usePtySession`),
//! while managing its own layout. Mirror mode shares the underlying session and terminal I/O,
//! not the client-specific layout. Startup mirrors App.tsx: apply appearance, load the tree, and
//! subscribe to global events. Differences:
//! - `useKeyboardShortcuts` is omitted because phones have no physical shortcut keys;
//! - `view://request` is ignored because mobile has no document tabs;
//! - navigation uses local `openId` state without touching tab, split, or keep-alive store state.

import { useCallback, useEffect, useState } from "react";
import { SpawnConfirmModal } from "../components/SpawnConfirmModal";
import { useNotifications } from "../hooks/useNotifications";
import { onSpawnRequest, onSpawnResolved, onTreeChanged } from "../ipc/events";
import { getClientSource } from "../ipc/transport";
import { ConnectionBanner } from "../remote/ConnectionBanner";
import { useTermStore } from "../store/termStore";
import { watchSystemTheme } from "../theme";
import type { SessionId } from "../types";
import { SessionListPage } from "./SessionListPage";
import { TerminalPage } from "./TerminalPage";
import "./mobile.css";

function MobileApp() {
  const projects = useTermStore((s) => s.projects);
  const sessions = useTermStore((s) => s.sessions);
  const loadTree = useTermStore((s) => s.loadTree);
  const handleSpawnRequest = useTermStore((s) => s.handleSpawnRequest);
  const applyAppearance = useTermStore((s) => s.applyAppearance);
  const clearNotification = useTermStore((s) => s.clearNotification);

  // The currently viewed session; null displays the list. TerminalPage owns mount/unmount behavior.
  const [openId, setOpenId] = useState<SessionId | null>(null);

  useNotifications();

  useEffect(() => {
    applyAppearance();
    void loadTree();
    const unwatch = watchSystemTheme(() => {
      if (useTermStore.getState().theme === "system") applyAppearance();
    });
    // Handle child-task requests normally. The new session appears in the tree and receives its
    // prompt through usePtySession when opened.
    const unlistenSpawn = onSpawnRequest((req) => void handleSpawnRequest(req));
    // Another client answering the card must clear it here too. Without this the phone keeps showing a
    // request the desktop already confirmed, and tapping Confirm launches the same task a second time.
    const unlistenResolved = onSpawnResolved((ev) => {
      if (ev.source === getClientSource()) return;
      useTermStore.getState().handleSpawnResolved(ev.parentSessionId, ev.prompt);
    });
    // Synchronize the tree across clients: after any successful mutation, the backend broadcasts
    // `tree://changed`; debounce the event before reloading.
    let treeTimer: ReturnType<typeof setTimeout> | undefined;
    const unlistenTree = onTreeChanged(() => {
      clearTimeout(treeTimer);
      treeTimer = setTimeout(() => void useTermStore.getState().loadTree(), 300);
    });
    return () => {
      unwatch();
      void unlistenSpawn.then((fn) => fn());
      void unlistenResolved.then((fn) => fn());
      clearTimeout(treeTimer);
      void unlistenTree.then((fn) => fn());
    };
    // Run only once on mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const open = useCallback(
    (id: SessionId) => {
      // Match desktop openSession behavior: opening a session clears its unread marker.
      clearNotification(id);
      setOpenId(id);
    },
    [clearNotification],
  );
  const back = useCallback(() => setOpenId(null), []);

  const session = openId ? (sessions.find((s) => s.id === openId) ?? null) : null;

  // Return to the list if the active session disappears after deletion or archiving.
  useEffect(() => {
    if (openId && !session) setOpenId(null);
  }, [openId, session]);

  const project = session ? projects.find((p) => p.id === session.projectId) : null;
  const cwd = session ? (session.cwd ?? project?.rootPath ?? undefined) : undefined;

  return (
    <div className="m-app">
      {session ? (
        <TerminalPage session={session} cwd={cwd} onBack={back} />
      ) : (
        <SessionListPage onOpen={open} />
      )}
      <SpawnConfirmModal />
      <ConnectionBanner />
    </div>
  );
}

export default MobileApp;
