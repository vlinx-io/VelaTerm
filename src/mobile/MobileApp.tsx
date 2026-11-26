//! Mobile root component with two-level navigation: session list to full-screen terminal.
//!
//! It shares the desktop data layer (`transport`, `wsClient`, `termStore`, and `usePtySession`),
//! while managing its own layout. Mirror mode shares the underlying session and terminal I/O,
//! not the client-specific layout. Startup mirrors App.tsx: apply appearance, load the tree, and
//! subscribe to global events. Differences:
//! - `useKeyboardShortcuts` is omitted because phones have no physical shortcut keys;
//! - `view://request` is ignored because mobile has no document tabs;
//! - navigation uses session URLs without mirroring tab, split, or keep-alive store state.

import { useCallback, useEffect } from "react";
import { isShareSurface } from "../ipc/shareBase";
import { navigateMobileSession, useMobileSessionNavigation } from "./sessionNavigation";
import { useSpawnNavigation } from "./useSpawnNavigation";
import { SplitTaskConfirmModal } from "../components/SplitTaskConfirmModal";
import { BindPlanExecuteModal } from "../components/BindPlanExecuteModal";
import { SpawnConfirmModal } from "../components/SpawnConfirmModal";
import { useNotifications } from "../hooks/useNotifications";
import {
  onSessionState,
  onTreeChanged,
} from "../ipc/events";
import { startSpawnRecovery } from "../ipc/spawnRecovery";
import { ConnectionBanner } from "../remote/ConnectionBanner";
import {
  refreshSettingsFromBackend,
  startSettingsWatch,
} from "../store/settingsWatch";
import { useTermStore } from "../store/termStore";
import { watchSystemTheme } from "../theme";
import { projectRoot, type SessionId } from "../types";
import { SessionListPage } from "./SessionListPage";
import { TerminalPage } from "./TerminalPage";
import "./mobile.css";
import { useMobileTree } from "./useMobileTree";
import { settleFirstMirrorAlign } from "../store/mirrorAlign";
import { bindMobilePush } from "./pushSubscription";

function MobileApp() {
  const projects = useTermStore((s) => s.projects);
  const sessions = useTermStore((s) => s.sessions);
  const loadTree = useTermStore((s) => s.loadTree);
  const loadMobileTree = useCallback(() => {
    // Mobile does not mirror desktop layout or wait for its initial alignment.
    settleFirstMirrorAlign(false);
    return loadTree().then(() => { void bindMobilePush() });
  }, [loadTree]);
  const treeRequest = useMobileTree(loadMobileTree);
  const applyAppearance = useTermStore((s) => s.applyAppearance);
  const clearNotification = useTermStore((s) => s.clearNotification);

  // The currently viewed session; null displays the list. TerminalPage owns mount/unmount behavior.
  const { sessionId: openId, view } = useMobileSessionNavigation();
  const treeLoaded=useTermStore(s=>s.treeLoaded);

  useNotifications();

  useEffect(() => {
    applyAppearance();

    // Phones share the same backend-authoritative preferences as the desktop, but this view never
    // reconciled them: it neither picked up a theme or language chosen elsewhere nor seeded its own.
    // Do the startup pass here too, then follow later changes over the broadcast.
    void refreshSettingsFromBackend();
    const stopSettingsWatch = startSettingsWatch();
    // Authoritative session records, same as the desktop: read the whole set, then follow the broadcast.
    // Without this the phone shows unread dots only for sessions it opened itself.
    void useTermStore.getState().syncSessionStates();
    const unlistenSessionState = onSessionState((batch) =>
      useTermStore.getState().applySessionStates(batch),
    );
    const unwatch = watchSystemTheme(() => {
      if (useTermStore.getState().theme === "system") applyAppearance();
    });
    const stopSpawnRecovery = startSpawnRecovery();
    // Synchronize the tree across clients: after any successful mutation, the backend broadcasts
    // `tree://changed`; debounce the event before reloading.
    let treeTimer: ReturnType<typeof setTimeout> | undefined;
    const unlistenTree = onTreeChanged(() => {
      clearTimeout(treeTimer);
      treeTimer = setTimeout(() => void treeRequest.refresh(), 300);
    });
    return () => {
      unwatch();
      stopSettingsWatch();
      stopSpawnRecovery();
      clearTimeout(treeTimer);
      void unlistenTree.then((fn) => fn());
      void unlistenSessionState.then((fn) => fn());
    };
    // Run only once on mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const open = useCallback(
    (id: SessionId) => {
      // Match desktop openSession behavior: opening a session clears its unread marker.
      clearNotification(id);
      navigateMobileSession(id);
    },
    [clearNotification],
  );
  const back = useCallback(() => navigateMobileSession(null), []);
  useSpawnNavigation(open);

  const session = openId ? (sessions.find((s) => s.id === openId) ?? null) : null;

  // Return to the list if the active session disappears after deletion or archiving.
  useEffect(() => {
    if (treeLoaded && !treeRequest.loading && !treeRequest.error && openId && !session) navigateMobileSession(null, "terminal", true);
  }, [treeLoaded, treeRequest.loading, treeRequest.error, openId, session]);

  const project = session ? projects.find((p) => p.id === session.projectId) : null;
  const cwd = session ? (session.cwd ?? projectRoot(project) ?? undefined) : undefined;

  return (
    <div className="m-app">
      {session ? (
        <TerminalPage session={session} cwd={cwd} onBack={back} view={view} />
      ) : (
        <SessionListPage onOpen={open} loading={treeRequest.loading} error={treeRequest.error} onRefresh={treeRequest.refresh} />
      )}
      <SpawnConfirmModal />
      {!isShareSurface && <SplitTaskConfirmModal />}
      {!isShareSurface && <BindPlanExecuteModal />}
      <ConnectionBanner />
    </div>
  );
}

export default MobileApp;
