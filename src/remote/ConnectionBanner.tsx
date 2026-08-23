//! Browser connection-loss banner with a clear message and immediate reconnect action.
//!
//! wsClient broadcasts status and shows this only after at least two consecutive failures, avoiding
//! transient flicker. It disappears immediately on recovery; reconnectNow bypasses backoff. Tauri IPC
//! desktop never renders it because it does not use WebSockets.
//!
//! SSH remote windows connect through a local forwarded port, so WebSocket retries cannot recover a
//! dead tunnel. ssh_remote::watch_tunnel reports rebuilding/failure through `ssh://tunnel-state`.
//! Manual reconnect also emits `vlx://ssh-reconnect` for the host, since remote contexts cannot invoke
//! app commands directly. After the tunnel returns, forceReconnect replaces any half-open old socket.

import { useEffect, useState } from "react";
import { useT } from "../i18n";
import {
  emitNative,
  isTauri,
  isRemoteWindow,
  listenNative,
  remoteSshSession,
} from "../ipc/transport";
import { wsClient } from "../ipc/wsClient";

/** Failure count before showing the banner; the first may be transient, the second is considered disconnected. */
const SHOW_AFTER_ATTEMPTS = 2;

/** Host tunnel status for SSH remote windows: reconnecting during automatic recovery, down after
 * recovery is abandoned, or null when healthy/not SSH. */
type TunnelState = "reconnecting" | "down" | null;

export function ConnectionBanner() {
  const t = useT();
  const [visible, setVisible] = useState(false);
  // Disable the manual action and show progress until the attempt either connects or fails again.
  const [retrying, setRetrying] = useState(false);
  const [tunnel, setTunnel] = useState<TunnelState>(null);

  useEffect(() => {
    if (isTauri) return;
    return wsClient.onConnState((state, attempts) => {
      setVisible(state === "offline" && attempts >= SHOW_AFTER_ATTEMPTS);
      // Any resulting status settles the manual attempt, whether connected or disconnected again.
      setRetrying(false);
    });
  }, []);

  // SSH remote windows subscribe to app-wide host tunnel events and filter by their connection session.
  useEffect(() => {
    if (!isRemoteWindow || !remoteSshSession) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void (async () => {
      try {
        // Native rather than the regular transport listen: this window's WebSocket goes to the remote
        // server, which knows nothing about the local tunnel carrying it.
        const un = await listenNative<{ session: string; state: string }>(
          "ssh://tunnel-state",
          (payload) => {
            if (payload.session !== remoteSshSession) return;
            const s = payload.state;
            if (s === "up") {
              setTunnel(null);
              // Replace the old potentially half-open socket immediately after tunnel recovery.
              wsClient.forceReconnect();
            } else if (s === "reconnecting" || s === "down") {
              setTunnel(s);
            }
            setRetrying(false);
          },
        );
        if (disposed) un();
        else unlisten = un;
      } catch {
        /* If the event channel is unavailable, retain the basic WebSocket banner. */
      }
    })();
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  // Tunnel failures must show the banner even before a half-open WebSocket detects them.
  if (!visible && tunnel === null) return null;

  const onReconnect = () => {
    setRetrying(true);
    // Ask the host to rebuild an SSH tunnel; healthy tunnels safely ignore the request.
    if (isRemoteWindow && remoteSshSession) {
      void (async () => {
        try {
          await emitNative("vlx://ssh-reconnect", { session: remoteSshSession });
        } catch {
          /* Continue with WebSocket reconnection if emitting fails. */
        }
      })();
    }
    wsClient.reconnectNow();
  };

  const message =
    tunnel === "down"
      ? t("conn.sshDown")
      : tunnel === "reconnecting"
        ? t("conn.sshReconnecting")
        : t("conn.reconnecting");

  return (
    <div
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        right: 0,
        zIndex: 10000,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        gap: 12,
        padding: "6px 12px",
        fontSize: 12.5,
        fontWeight: 600,
        color: "var(--bg-0)",
        background: "var(--danger, #d84840)",
      }}
    >
      <span>{message}</span>
      <button
        onClick={onReconnect}
        disabled={retrying}
        style={{
          border: "1px solid var(--bg-0)",
          borderRadius: 5,
          padding: "2px 10px",
          fontSize: 11.5,
          fontWeight: 700,
          color: "var(--bg-0)",
          background: "transparent",
          cursor: retrying ? "default" : "pointer",
          opacity: retrying ? 0.6 : 1,
          whiteSpace: "nowrap",
        }}
      >
        {retrying ? t("conn.retrying") : t("conn.reconnectNow")}
      </button>
    </div>
  );
}
