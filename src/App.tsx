//! Vlinx application root: title bar, project/terminal/info columns, and status bar.

import { useEffect } from "react";
import { GitbashDownloadBanner } from "./components/GitbashDownloadBanner";
import { ErrorLogModal } from "./components/ErrorLogModal";
import { Splitter } from "./components/Splitter";
import { MergeModal } from "./components/MergeModal";
import { ChangesModal } from "./components/diff/ChangesModal";
import { NotifyGuideModal } from "./components/NotifyGuideModal";
import { QuitConfirmModal } from "./components/QuitConfirmModal";
import { SpawnConfirmModal } from "./components/SpawnConfirmModal";
import { UpdateModal } from "./components/UpdateModal";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { useNotifications } from "./hooks/useNotifications";
import { CenterPane } from "./layout/CenterPane/CenterPane";
import { LeftSidebar } from "./layout/LeftSidebar/LeftSidebar";
import { RightPanel } from "./layout/RightPanel/RightPanel";
import { StatusBar } from "./layout/StatusBar/StatusBar";
import { TitleBar } from "./layout/TitleBar/TitleBar";
import { listShells } from "./ipc/commands";
import {
  onMenuAction,
  onSpawnRequest,
  onSpawnResolved,
  onPresetsChanged,
  onSessionState,
  onTreeChanged,
  onViewRequest,
} from "./ipc/events";
import {
  refreshSettingsFromBackend,
  startSettingsWatch,
} from "./store/settingsWatch";
import { getClientSource, isTauri } from "./ipc/transport";
import { wsClient } from "./ipc/wsClient";
import { checkForUpdates, startUpdateSchedule } from "./ipc/updater";
import { env, platform } from "./platform";
import { ConnectionBanner } from "./remote/ConnectionBanner";
import { CloneProjectModal } from "./remote/CloneProjectModal";
import { CreateProjectModal } from "./remote/CreateProjectModal";
import { DirectoryPickerModal } from "./remote/DirectoryPickerModal";
import { SaveAsModal } from "./remote/SaveAsModal";
import { startMirrorSync } from "./store/mirrorSync";
import { useTermStore } from "./store/termStore";
import { watchSystemTheme } from "./theme";

/** Isolated notification side-effect host. Changes to activeSessionId, notifications, and
 * windowFocused rerender only this null component instead of the entire App tree, avoiding stalls
 * during session switches, notifications, and focus changes. */
function NotificationsManager() {
  useNotifications();
  return null;
}

function App() {
  const leftCollapsed = useTermStore((s) => s.leftCollapsed);
  const rightCollapsed = useTermStore((s) => s.rightCollapsed);
  const resizeLeft = useTermStore((s) => s.resizeLeft);
  const resizeRight = useTermStore((s) => s.resizeRight);
  const loadTree = useTermStore((s) => s.loadTree);
  const handleSpawnRequest = useTermStore((s) => s.handleSpawnRequest);
  const applyAppearance = useTermStore((s) => s.applyAppearance);

  useKeyboardShortcuts();

  // Apply appearance, load the SQLite tree, and register global child-task listeners at startup.
  // Reapply appearance on system theme changes while following the system.
  useEffect(() => {
    applyAppearance();
    // Retry the idempotent initial tree read three times with backoff in case transport is not ready.
    // If all attempts fail, transport's centralized handling displays an error instead of leaving a
    // silently empty tree.
    void (async () => {
      const delays = [1000, 2000, 4000];
      for (let attempt = 0; ; attempt++) {
        try {
          await loadTree();
          // Presets only feed the new-session menu, so a failure here must not retry the tree load.
          void useTermStore.getState().loadAgentPresets();
          return;
        } catch {
          if (attempt >= delays.length) return; // Central handling has already recorded and displayed the error.
          await new Promise((r) => setTimeout(r, delays[attempt]));
        }
      }
    })();
    // Reload the tree after WebSocket recovery because reconnect restores transport and PTY attachment,
    // not tree changes made while disconnected. Desktop IPC does not need this subscription.
    const offConnState = isTauri
      ? undefined
      : wsClient.onConnState((state) => {
          if (state !== "online") return;
          void loadTree().catch(() => {});
          // Broadcasts that landed while the socket was down are not replayed, so re-read the
          // authoritative session records rather than carrying a stale set of dots forward.
          void useTermStore.getState().syncSessionStates();
        });
    // Detect and cache available shells here for the inline selector. Module-level detection would
    // race WebSocket connection before remote login completes. On failure, leave an empty list so the
    // selector hides automatically.
    void listShells()
      .then((shells) => useTermStore.setState({ shells }))
      .catch(() => {});
    // Reconcile cross-shell preferences with the shared backend as authority, applying differences once
    // and seeding missing backend keys from local values. Enable outbound sync only afterward. The same
    // helper handles the runtime broadcast below, so startup and mid-run reconciliation cannot drift.
    void refreshSettingsFromBackend();
    const unwatch = watchSystemTheme(() => {
      if (useTermStore.getState().theme === "system") applyAppearance();
    });
    const unlisten = onSpawnRequest((req) => void handleSpawnRequest(req));
    const unlistenResolved = onSpawnResolved((ev) => {
      // Skip our own echo — we already removed the card locally.
      if (ev.source === getClientSource()) return;
      useTermStore.getState().handleSpawnResolved(ev.parentSessionId, ev.prompt);
    });
    const consumeOpenProject = () => {
      void platform.window
        .takeOpenProjectRequest()
        .then((path) => {
          if (path) return useTermStore.getState().openProjectPath(path);
        })
        .catch(() => {
          // Platform/import layers already report errors; prevent an unhandled rejection in the event callback.
        });
    };
    // Fetch pending requests only after listener registration. Earlier requests remain queued and later
    // requests trigger events, covering both timing windows.
    const unlistenOpenProject = platform.window
      .onOpenProjectRequest(consumeOpenProject)
      .then((unlisten) => {
        consumeOpenProject();
        return unlisten;
      });
    // `view` opens files in document tabs and HTTP(S) URLs in built-in browser tabs. Remote browser
    // clients ignore URL requests because child WebViews are desktop-native.
    const unlistenView = onViewRequest((req) => {
      if (req.isUrl) {
        // The built-in browser requires a Tauri child or Electron WebContentsView, so only desktop opens it.
        if (isTauri || env.isElectron) useTermStore.getState().openBrowserTab(req.path);
        return;
      }
      useTermStore.getState().openDocTab(req.path);
    });
    // Synchronize the tree across clients by debouncing backend `tree://changed` broadcasts. Bursts
    // from compound actions merge; local actions may cause one harmless extra idempotent reconciliation.
    let treeTimer: ReturnType<typeof setTimeout> | undefined;
    const unlistenTree = onTreeChanged(() => {
      clearTimeout(treeTimer);
      treeTimer = setTimeout(() => void useTermStore.getState().loadTree(), 300);
    });
    // Agent presets are edited far less often than the tree and are broadcast separately, so reload the
    // list directly without debouncing.
    const unlistenPresets = onPresetsChanged(() => {
      void useTermStore.getState().loadAgentPresets();
    });
    // Handle macOS native menu actions, including split accelerators that may be consumed before
    // WKWebView dispatches keydown. Re-dispatch those key equivalents through the shared shortcut
    // matcher so user overrides retain the same semantics as ordinary DOM keyboard events.
    const unlistenMenu = onMenuAction((action) => {
      if (action === "settings") {
        useTermStore.getState().setSettingsOpen(true);
      } else if (action === "check-update") {
        void checkForUpdates({ manual: true });
      } else if (action === "share") {
        useTermStore.getState().setShareOpen(true);
      } else if (action === "split-right" || action === "split-down") {
        document.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: action === "split-down" ? "D" : "d",
            code: "KeyD",
            metaKey: true,
            shiftKey: action === "split-down",
            bubbles: true,
            cancelable: true,
          }),
        );
      }
    });
    // Silently check for desktop updates shortly after startup and periodically thereafter. New versions
    // light the status bar; no-update and error results remain quiet.
    const stopUpdateSchedule = startUpdateSchedule();
    // Mirror mode: follow and publish the shared layout so a remote browser renders this same arrangement.
    // Phones opt out inside startMirrorSync; when mirror mode is off it only listens for the switch.
    const stopMirrorSync = startMirrorSync();
    // Preferences are backend-authoritative but were reconciled only at startup, so a change made in
    // another shell stayed invisible here until the next launch. Follow the broadcast instead.
    const stopSettingsWatch = startSettingsWatch();
    // Authoritative session records: read the whole set once, then follow the broadcast. This is a
    // connection-level subscription, so a session this client has never opened still shows its state.
    void useTermStore.getState().syncSessionStates();
    const unlistenSessionState = onSessionState((batch) =>
      useTermStore.getState().applySessionStates(batch),
    );
    return () => {
      unwatch();
      offConnState?.();
      void unlisten.then((fn) => fn());
      void unlistenResolved.then((fn) => fn());
      void unlistenOpenProject.then((fn) => fn());
      void unlistenView.then((fn) => fn());
      clearTimeout(treeTimer);
      void unlistenTree.then((fn) => fn());
      void unlistenPresets.then((fn) => fn());
      void unlistenSessionState.then((fn) => fn());
      void unlistenMenu.then((fn) => fn());
      stopUpdateSchedule();
      stopMirrorSync();
      stopSettingsWatch();
    };
    // Run once on mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Hidden debug shortcut Cmd/Ctrl+Opt+E toggles the error log like Option-clicking the settings gear.
  // Use e.code because macOS Option+E is a dead key and e.key is unreliable.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.altKey && e.code === "KeyE") {
        e.preventDefault();
        const s = useTermStore.getState();
        s.setErrorLogOpen(!s.errorLogOpen);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="app">
      <TitleBar />
      <div className="body">
        {!leftCollapsed && (
          <>
            <LeftSidebar />
            <Splitter onDrag={resizeLeft} />
          </>
        )}

        <CenterPane />

        {!rightCollapsed && (
          <>
            <Splitter onDrag={resizeRight} />
            <RightPanel />
          </>
        )}
      </div>
      <StatusBar />
      <DirectoryPickerModal />
      <CreateProjectModal />
      <CloneProjectModal />
      <SaveAsModal />
      <SpawnConfirmModal />
      <QuitConfirmModal />
      <MergeModal />
      <ChangesModal />
      <NotifyGuideModal />
      <UpdateModal />
      <ConnectionBanner />
      <ErrorLogModal />
      <NotificationsManager />
      <GitbashDownloadBanner />
    </div>
  );
}

export default App;
