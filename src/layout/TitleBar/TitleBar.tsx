//! Vela-style application header with branding, theme toggle, appearance settings, and panel controls.
//! The window retains native decorations; this row sits below the system title bar.

import { useEffect, useRef, useState } from "react";
import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import { getBackendVersion } from "../../ipc/commands";
import { invoke, isTauri } from "../../ipc/transport";
import { webServerStatus, type WebServerStatus } from "../../ipc/webServer";
import { env } from "../../platform";
import { useTermStore } from "../../store/termStore";
import { resolveTheme } from "../../theme";
import { ShareModal } from "../../components/ShareModal";
import { AppMenuBar } from "./AppMenuBar";
import { ConnectRemotePanel } from "./ConnectRemotePanel";
import { RemoteAccessPanel } from "./RemoteAccessPanel";
import { SettingsModal } from "./SettingsModal";

/** Format HH:mm:ss for the development badge's latest hot-update time. */
function fmtClock(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

/**
 * Development-only latest Vite HMR timestamp. It starts at module load and updates after every
 * `vite:afterUpdate`, making it easy to confirm that fresh code reached the client.
 */
function useHotReloadTime(): string {
  const [stamp, setStamp] = useState(() => fmtClock(new Date()));
  useEffect(() => {
    if (!import.meta.hot) return;
    const handler = () => setStamp(fmtClock(new Date()));
    import.meta.hot.on("vite:afterUpdate", handler);
    return () => {
      import.meta.hot?.off("vite:afterUpdate", handler);
    };
  }, []);
  return stamp;
}

export function TitleBar() {
  const t = useT();
  const hotReloadTime = useHotReloadTime();
  const theme = useTermStore((s) => s.theme);
  const setTheme = useTermStore((s) => s.setTheme);
  const leftCollapsed = useTermStore((s) => s.leftCollapsed);
  const toggleLeft = useTermStore((s) => s.toggleLeft);
  const rightCollapsed = useTermStore((s) => s.rightCollapsed);
  const toggleRight = useTermStore((s) => s.toggleRight);
  // Mirror mode is switched on the host only. Without a marker, a followed client sees its tabs and
  // splits rearrange with no visible cause; the badge names where those changes come from.
  const mirrorEnabled = useTermStore((s) => s.mirrorEnabled);
  // The host needs the same warning for the opposite reason: mirroring is two-way, so an attached client
  // rearranges the host's window too. The switch alone does not say whether anyone is actually on the
  // other end, so the host badge waits for a real connection. Zero clients means nothing is being driven
  // from anywhere else, and a badge then would be noise.
  const remoteClients = useTermStore((s) => s.remoteClients);
  // A count says somebody is there; the list says who, which is the next thing a host asks before letting
  // a peer move its tabs. The badge opens it.
  const remoteClientList = useTermStore((s) => s.remoteClientList);
  const [clientsOpen, setClientsOpen] = useState(false);
  const clientsRef = useRef<HTMLSpanElement | null>(null);
  // Dismiss the client list the way any popover should: a click anywhere else, or Escape.
  useEffect(() => {
    if (!clientsOpen) return;
    const onDown = (e: MouseEvent) => {
      if (!clientsRef.current?.contains(e.target as Node))
        setClientsOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setClientsOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [clientsOpen]);
  // The badge disappears once the last client leaves, taking the list with it. Reset the flag so the badge
  // does not come back already open when somebody else connects.
  useEffect(() => {
    if (remoteClients === 0) setClientsOpen(false);
  }, [remoteClients]);

  // Store-level settings visibility is shared by the gear and the macOS native Settings menu.
  const settingsOpen = useTermStore((s) => s.settingsOpen);
  const setSettingsOpen = useTermStore((s) => s.setSettingsOpen);
  // Share-dialog visibility is shared by the header action and macOS native Share menu.
  const shareOpen = useTermStore((s) => s.shareOpen);
  const setShareOpen = useTermStore((s) => s.setShareOpen);
  // Hidden error-log entry through Option/Alt-clicking the gear; a normal click opens settings.
  const setErrorLogOpen = useTermStore((s) => s.setErrorLogOpen);
  const [remoteOpen, setRemoteOpen] = useState(false);
  const [connectOpen, setConnectOpen] = useState(false);
  // Show the hidden remote-database reuse checkbox only when the remote-connect button is opened with
  // Option/Alt. Normal opening always uses an independent database; each click decides afresh.
  const [connectSharedDb, setConnectSharedDb] = useState(false);
  // Remote-access service state lights the globe. The backend remains authoritative; query the initial
  // value here and synchronize subsequent changes through the panel callback.
  const [remoteRunning, setRemoteRunning] = useState(false);
  const [remotePort, setRemotePort] = useState<number | null>(null);
  // Frontend/backend versions for the mismatch banner; null when equal or not yet checked.
  const [versionMismatch, setVersionMismatch] = useState<{
    frontend: string;
    backend: string;
  } | null>(null);
  const resolved = resolveTheme(theme);

  // Compare bundle __APP_VERSION__ from package.json with backend app_version from Cargo.toml at
  // startup. Release scripts update both, but remote deployments or manual edits can drift. Report
  // mismatches to the console and header rather than running incompatible ends silently.
  useEffect(() => {
    let alive = true;
    getBackendVersion()
      .then((backend) => {
        if (!alive) return;
        const frontend = __APP_VERSION__;
        if (backend !== frontend) {
          console.error(
            `[version mismatch] frontend v${frontend} != backend v${backend}: the two builds have drifted apart. Rebuild, or deploy both together.`,
          );
          setVersionMismatch({ frontend, backend });
        }
      })
      .catch(() => {
        // Old backends or connection failures cannot provide a version; do not block or show a false mismatch.
      });
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    // Tauri and Electron desktop expose remote access; browser clients are already remote and skip this.
    if (!isTauri && !env.isElectron) return;
    let alive = true;
    webServerStatus()
      .then((s) => {
        if (!alive) return;
        setRemoteRunning(s.running);
        setRemotePort(s.port);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  const syncRemoteStatus = (s: WebServerStatus | null) => {
    setRemoteRunning(s?.running ?? false);
    setRemotePort(s?.port ?? null);
  };

  const remoteInfo = (window as any).__VLX_REMOTE__ as
    { address: string } | undefined;

  return (
    <div className="titlebar">
      <div className="brand">
        <img
          className="logo"
          // CSS content on `.brand .logo` follows data-theme so system-mode changes do not require a
          // TitleBar rerender. This light src is only a fallback when CSS content is unavailable.
          src="/velaterm-light.svg"
          alt="VelaTerm"
          draggable={false}
        />
        <span className="v">Vela</span>
        <span className="sub">terminal · agents</span>
        {env.isDev || __DEV_BUILD__ ? (
          <span
            // Vite development for Tauri/browser shows HMR time. Electron loads packaged assets without
            // HMR, so it shows only "dev". Dev-server builds use their build timestamp.
            title={
              import.meta.hot
                ? t("titlebar.hotReloadedAt", hotReloadTime)
                : __DEV_BUILD__ && __BUILD_TIME__
                  ? t("titlebar.builtAt", __BUILD_TIME__)
                  : "dev"
            }
            style={{
              marginLeft: 2,
              fontSize: 9,
              fontWeight: 700,
              letterSpacing: 0.4,
              textTransform: "uppercase",
              padding: "1px 5px",
              borderRadius: 4,
              color: "var(--accent)",
              background: "var(--accent-soft)",
            }}
          >
            {import.meta.hot
              ? `dev · ${hotReloadTime}`
              : __DEV_BUILD__ && __BUILD_TIME__
                ? `dev · ${__BUILD_TIME__}`
                : "dev"}
          </span>
        ) : (
          <span
            title={
              __BUILD_TIME__ ? t("titlebar.builtAt", __BUILD_TIME__) : undefined
            }
            style={{
              marginLeft: 2,
              fontSize: 9.5,
              fontWeight: 600,
              padding: "1px 5px",
              borderRadius: 4,
              color: "var(--text-dim)",
              background: "var(--bg-active)",
            }}
          >
            v{__APP_VERSION__}
            {__BUILD_TIME__ ? ` · ${__BUILD_TIME__}` : ""}
          </span>
        )}
        {remoteInfo && (
          <span
            style={{
              marginLeft: 6,
              fontSize: 9.5,
              fontWeight: 700,
              letterSpacing: 0.4,
              textTransform: "uppercase",
              padding: "1px 6px",
              borderRadius: 4,
              color: "var(--bg-0)",
              background: "var(--accent)",
            }}
          >
            Remote · {remoteInfo.address}
          </span>
        )}
        {mirrorEnabled && (isTauri || env.isElectron) && remoteClients > 0 && (
          <span ref={clientsRef} style={{ position: "relative" }}>
            <button
              onClick={() => setClientsOpen((v) => !v)}
              title={t("titlebar.mirroredByHint", remoteClients)}
              style={{
                marginLeft: 6,
                fontSize: 9.5,
                fontWeight: 700,
                letterSpacing: 0.4,
                textTransform: "uppercase",
                padding: "1px 6px",
                borderRadius: 4,
                border: "none",
                cursor: "pointer",
                fontFamily: "inherit",
                color: "var(--bg-0)",
                background: "var(--yellow)",
              }}
            >
              ⤢ {t("titlebar.mirroredBy", remoteClients)}
            </button>
            {clientsOpen && (
              <div
                style={{
                  position: "absolute",
                  top: "calc(100% + 6px)",
                  left: 6,
                  zIndex: 200,
                  minWidth: 220,
                  maxWidth: 320,
                  padding: "10px 12px",
                  background: "var(--bg-2)",
                  border: "1px solid var(--border-strong)",
                  borderRadius: "var(--r-md)",
                  boxShadow: "var(--shadow)",
                  textAlign: "left",
                }}
              >
                <div
                  style={{
                    fontSize: 10,
                    letterSpacing: "1px",
                    textTransform: "uppercase",
                    fontWeight: 600,
                    color: "var(--text-dim)",
                    marginBottom: 6,
                  }}
                >
                  {t("titlebar.clientsTitle")}
                </div>
                {remoteClientList.length === 0 ? (
                  <div
                    style={{
                      fontSize: 11,
                      lineHeight: 1.5,
                      color: "var(--text-dim)",
                    }}
                  >
                    {t("titlebar.mirroredByHint", remoteClients)}
                  </div>
                ) : (
                  remoteClientList.map((c) => (
                    <div key={c.source} style={{ padding: "4px 0" }}>
                      <div
                        style={{
                          fontSize: 12,
                          color: "var(--text)",
                          whiteSpace: "nowrap",
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                        }}
                      >
                        {c.name || t("titlebar.clientUnnamed")}
                      </div>
                      <div style={{ fontSize: 10.5, color: "var(--text-dim)" }}>
                        {c.ip} ·{" "}
                        {t(
                          "titlebar.clientSince",
                          new Date(c.since * 1000).toLocaleTimeString(),
                        )}
                      </div>
                    </div>
                  ))
                )}
              </div>
            )}
          </span>
        )}
        {mirrorEnabled && !(isTauri || env.isElectron) && (
          <span
            title={t("titlebar.mirroredHint")}
            style={{
              marginLeft: 6,
              fontSize: 9.5,
              fontWeight: 700,
              letterSpacing: 0.4,
              textTransform: "uppercase",
              padding: "1px 6px",
              borderRadius: 4,
              color: "var(--accent)",
              background: "var(--accent-soft)",
            }}
          >
            ⤢ {t("titlebar.mirrored")}
          </span>
        )}
        {versionMismatch && (
          <span
            title={t(
              "titlebar.versionMismatch",
              versionMismatch.frontend,
              versionMismatch.backend,
            )}
            style={{
              marginLeft: 6,
              fontSize: 9.5,
              fontWeight: 700,
              letterSpacing: 0.3,
              padding: "1px 6px",
              borderRadius: 4,
              color: "#fff",
              background: "var(--danger, #e5484d)",
            }}
          >
            ⚠ v{versionMismatch.frontend} ≠ v{versionMismatch.backend}
          </span>
        )}
      </div>

      {/* Windows/Linux have no native menu bar; this one appears on a bare Alt press. */}
      <AppMenuBar />

      <span className="tb-spacer" />

      <div className="tb-seg">
        <button
          className={theme === "system" ? "on" : ""}
          title={t(
            "titlebar.themeSystem",
            resolved === "dark"
              ? t("titlebar.themeDark")
              : t("titlebar.themeLight"),
          )}
          onClick={() => setTheme("system")}
        >
          <Icons.monitor size={14} />
        </button>
        <button
          className={theme === "dark" ? "on" : ""}
          title={t("titlebar.themeDark")}
          onClick={() => setTheme("dark")}
        >
          <Icons.moon size={14} />
        </button>
        <button
          className={theme === "light" ? "on" : ""}
          title={t("titlebar.themeLight")}
          onClick={() => setTheme("light")}
        >
          <Icons.sun size={14} />
        </button>
      </div>

      {/* Remote access appears in Tauri/Electron desktop, whose sidecars can start a LAN instance.
          Browser clients are already remote. The button lights while the service runs. */}
      {(isTauri || env.isElectron) && (
        <button
          className={`tb-btn${remoteRunning ? " remote-on" : ""}`}
          title={
            remoteRunning
              ? `${t("titlebar.remoteAccess")} · ${t("remote.running", remotePort ?? 0)}`
              : t("titlebar.remoteAccess")
          }
          onClick={() => setRemoteOpen((o) => !o)}
        >
          <Icons.globe size={15} />
        </button>
      )}

      {/* Remote connect is Tauri-desktop-only, using a wry window and local TLS-termination tunnel.
          Electron lacks the equivalent capability and is intentionally excluded. */}
      {isTauri && (
        <button
          className="tb-btn"
          title={t("titlebar.connectRemote")}
          onClick={(e) => {
            // Option/Alt-click reveals the hidden remote desktop database reuse option.
            if (!connectOpen) setConnectSharedDb(e.altKey);
            setConnectOpen((o) => !o);
          }}
        >
          <Icons.connect size={15} />
        </button>
      )}

      {/* DevTools appears only in desktop development builds. The backend open_devtools command is
          also guarded by debug_assertions and unavailable in releases. */}
      {import.meta.env.DEV && isTauri && (
        <button
          className="tb-btn"
          title="DevTools"
          onClick={() => {
            void invoke("open_devtools").catch(() => {});
          }}
        >
          <Icons.code size={15} />
        </button>
      )}

      {/* Share appears on every platform and shares its dialog with the macOS native menu action. */}
      <button
        className="tb-btn"
        title={t("titlebar.share")}
        onClick={() => setShareOpen(!shareOpen)}
      >
        <Icons.share size={15} />
      </button>

      <button
        className="tb-btn"
        title={t("settings.title")}
        onClick={(e) => {
          // Hidden debug action: Option/Alt-click opens the error log; normal click opens settings.
          if (e.altKey) {
            setErrorLogOpen(true);
            return;
          }
          setSettingsOpen(!settingsOpen);
        }}
      >
        <Icons.gear size={15} />
      </button>

      {/* VS Code-style panel toggles sit at the far right and fill their corresponding side when open. */}
      <div className="tb-pair">
        <button
          className="tb-btn"
          title={
            leftCollapsed ? t("titlebar.showLeft") : t("titlebar.hideLeft")
          }
          onClick={toggleLeft}
        >
          {leftCollapsed ? (
            <Icons.panelLeft size={15} />
          ) : (
            <Icons.panelLeftFill size={15} />
          )}
        </button>

        <button
          className="tb-btn"
          title={
            rightCollapsed ? t("titlebar.showRight") : t("titlebar.hideRight")
          }
          onClick={toggleRight}
        >
          {rightCollapsed ? (
            <Icons.panel size={15} />
          ) : (
            <Icons.panelFill size={15} />
          )}
        </button>
      </div>

      {settingsOpen && <SettingsModal onClose={() => setSettingsOpen(false)} />}
      {shareOpen && <ShareModal onClose={() => setShareOpen(false)} />}
      {remoteOpen && (
        <RemoteAccessPanel
          onClose={() => setRemoteOpen(false)}
          onStatusChange={syncRemoteStatus}
        />
      )}
      {connectOpen && (
        <ConnectRemotePanel
          onClose={() => setConnectOpen(false)}
          showSharedDb={connectSharedDb}
        />
      )}
    </div>
  );
}
