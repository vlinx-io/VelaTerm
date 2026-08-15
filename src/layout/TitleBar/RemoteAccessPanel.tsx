//! Desktop remote-access panel for controlling the web service, setting a password, and displaying
//! LAN addresses. Devices on the same network can open an address and authenticate to use the desktop UI.

import { useEffect, useRef, useState } from "react";
import { QRCodeSVG } from "qrcode.react";
import { useT } from "../../i18n";
import { Backdrop } from "../../components/Backdrop";
import {
  networkInterfacesList,
  webDeviceRevoke,
  webDevicesList,
  webPairingCreate,
  webServerStart,
  webServerStatus,
  webServerStop,
  type DeviceEntry,
  type NetworkInterface,
  type WebServerStatus,
} from "../../ipc/webServer";
import { copyText } from "../../ipc/info";
import { invoke } from "../../ipc/transport";

/** app_settings key persisting the selected advertised IP; empty string means automatic (backend default). */
const SHARE_IP_KEY = "vlx-share-ip";

/** Extract the host of a URL, exact up to `:` or `/` after the scheme, so `10.0.0.1` never matches `10.0.0.11`. */
function hostOf(u: string): string {
  const start = u.indexOf("://");
  if (start < 0) return "";
  const rest = u.slice(start + 3);
  const end = rest.search(/[/:]/);
  return end < 0 ? rest : rest.slice(0, end);
}

/** Move URLs whose host is exactly `ip` to the front, keeping backend order otherwise. */
export function orderUrlsBySelectedIp(urls: string[], ip: string): string[] {
  if (!ip) return urls;
  const hits = urls.filter((u) => hostOf(u) === ip);
  if (hits.length === 0) return urls;
  return [...hits, ...urls.filter((u) => hostOf(u) !== ip)];
}

/**
 * Display URLs for an explicitly selected IP. The backend URL list mirrors the interface snapshot
 * taken at server START, while the selector enumerates interfaces LIVE — an interface that appeared
 * afterwards (e.g. Tailscale connecting later) is selectable but absent from the snapshot. In that
 * case, synthesize its URL from the backend-reported scheme (explicit in the status since the
 * `scheme` field; inferring from the snapshot's first URL remains only as a fallback for a stale
 * backend without the field) and the live port, and put it first, so the primary copied link and
 * the QR carry the chosen IP without a restart.
 */
export function urlsForSelectedIp(
  urls: string[],
  ip: string,
  port: number | null,
  scheme: string | null,
): string[] {
  if (!ip) return urls;
  if (urls.some((u) => hostOf(u) === ip)) return orderUrlsBySelectedIp(urls, ip);
  if (urls.length === 0 || port == null) return urls;
  const s = scheme ?? (urls[0].startsWith("http://") ? "http" : "https");
  return [`${s}://${ip}:${port}`, ...urls];
}

export function RemoteAccessPanel({
  onClose,
  onStatusChange,
}: {
  onClose: () => void;
  /** Notify the title bar so its globe button reflects whether the service is running. */
  onStatusChange?: (status: WebServerStatus | null) => void;
}) {
  const t = useT();
  const [status, setStatus] = useState<WebServerStatus | null>(null);
  const [password, setPassword] = useState("");
  // Listening port defaults to 8799 and remains editable in case it is occupied. The first status load
  // prefills the last persisted port unless the user already typed one.
  const [port, setPort] = useState("8799");
  const portTouched = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [copiedUrl, setCopiedUrl] = useState<string | null>(null);
  const [copiedFp, setCopiedFp] = useState(false);
  // Show only the preferred address by default and collapse the rest for hosts with many interfaces.
  const [showAllUrls, setShowAllUrls] = useState(false);
  // Copyable E2EE pairing link containing a token and server public key in the URL fragment.
  const [pairUrl, setPairUrl] = useState<string | null>(null);
  const [pairBusy, setPairBusy] = useState(false);
  // Paired devices, loaded while the service runs and refreshed after link generation or revocation.
  const [devices, setDevices] = useState<DeviceEntry[]>([]);
  // Paired-devices dialog visibility.
  const [showDevices, setShowDevices] = useState(false);
  // Device awaiting revocation confirmation to prevent accidental denial.
  const [confirmId, setConfirmId] = useState<string | null>(null);
  // Device currently being revoked, used to disable duplicate actions.
  const [blockBusy, setBlockBusy] = useState(false);
  // Selectable interface candidates for the advertised-IP selector.
  const [ifaces, setIfaces] = useState<NetworkInterface[]>([]);
  // Persisted advertised-IP selection; empty string means automatic (backend picks the first LAN address).
  const [selectedIp, setSelectedIp] = useState("");
  // Monotonic pairing-request sequence: only the latest request may apply its result, so an older
  // response resolving late can never overwrite a newer link (in-flight guard for genPairing).
  const pairSeq = useRef(0);
  // Address the latest pairing request was issued with; null until the first request. Drives the
  // regeneration effect below independently of whether that request has already resolved.
  const pairRequestedIp = useRef<string | null>(null);

  useEffect(() => {
    webServerStatus()
      .then((s) => {
        setStatus(s);
        // Restore the last used port after restart/remount instead of hardcoding 8799.
        if (s.savedPort != null && !portTouched.current) {
          setPort(String(s.savedPort));
        }
      })
      .catch(() => setStatus(null));
    networkInterfacesList()
      .then(setIfaces)
      .catch(() => setIfaces([]));
    // Restore the persisted selection; a missing key or failure keeps automatic.
    invoke<Record<string, string>>("get_app_settings")
      .then((s) => setSelectedIp(s[SHARE_IP_KEY] ?? ""))
      .catch(() => {});
  }, []);

  // A persisted IP that is currently absent (e.g. VPN down) falls back to automatic without erasing
  // the stored value; only an explicit re-selection overwrites it.
  const effectiveIp = ifaces.some((i) => i.ip === selectedIp) ? selectedIp : "";

  // Report status after initial discovery or service changes, only when running/port actually changes.
  useEffect(() => {
    onStatusChange?.(status);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status?.running, status?.port]);

  // Invalidate in-flight pairing responses and reset the COMPLETE pairing state; used when the
  // service stops or its pairing link can no longer be trusted. Bumping pairSeq strands in-flight
  // requests, whose `finally` therefore skips its own busy reset — so pairBusy must be cleared
  // here, or an invalidated in-flight request would leave the regenerate button disabled forever.
  const resetPairing = () => {
    pairSeq.current++;
    pairRequestedIp.current = null;
    setPairUrl(null);
    setPairBusy(false);
  };

  // While running, load registered devices and automatically show a pairing link; clear them on stop.
  useEffect(() => {
    if (status?.running) {
      webDevicesList()
        .then(setDevices)
        .catch(() => setDevices([]));
      void genPairing(false);
    } else {
      setDevices([]);
      resetPairing();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status?.running]);

  // Regenerate the pairing link when the effective selection changes while running, so the link, QR,
  // and primary URL follow it. Keyed on the address the latest pairing request used (not on pairUrl),
  // so a persisted IP arriving while the automatic first pairing is still in flight reliably re-pairs.
  useEffect(() => {
    if (
      status?.running &&
      pairRequestedIp.current !== null &&
      pairRequestedIp.current !== effectiveIp
    )
      void genPairing(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [effectiveIp]);

  // Persist the selection and apply it immediately; the effect above regenerates the link if running.
  const chooseIp = (ip: string) => {
    setSelectedIp(ip);
    void invoke("set_app_settings", {
      entries: { [SHARE_IP_KEY]: ip },
    }).catch(() => {});
  };

  const start = async () => {
    if (!password.trim()) {
      setError(t("remote.needPassword"));
      return;
    }
    const portNum = Number(port.trim());
    if (!Number.isInteger(portNum) || portNum < 1 || portNum > 65535) {
      setError(t("remote.portInvalid"));
      return;
    }
    setBusy(true);
    setError("");
    try {
      setStatus(await webServerStart(password.trim(), portNum));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const stop = async () => {
    setBusy(true);
    setError("");
    try {
      await webServerStop();
      // The service is down: invalidate the pairing state HERE and not only via the status-driven
      // effect, which never fires when the status query below throws (status would stay "running"
      // and a late in-flight pairing response could re-populate the dead link).
      resetPairing();
      setStatus(await webServerStatus());
      setPassword("");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  // With rotate=false, fetch the current link after startup. With rotate=true, issue a new token,
  // invalidate the old link, clear registrations, and disconnect all devices.
  const genPairing = async (rotate: boolean) => {
    // Overlapping calls are possible (e.g. the automatic first pairing racing a re-pair for a
    // late-arriving persisted IP); the sequence guard lets only the latest request take effect.
    const seq = ++pairSeq.current;
    pairRequestedIp.current = effectiveIp;
    setPairBusy(true);
    setError("");
    try {
      const info = await webPairingCreate(effectiveIp || undefined, rotate);
      if (seq !== pairSeq.current) return;
      setPairUrl(info.url);
      if (rotate) {
        webDevicesList()
          .then(setDevices)
          .catch(() => setDevices([]));
      }
    } catch (e) {
      if (seq === pairSeq.current) setError(String(e));
    } finally {
      if (seq === pairSeq.current) setPairBusy(false);
    }
  };

  // Refresh devices when opening the dialog because clients may have connected since the panel opened.
  const openDevices = () => {
    setConfirmId(null);
    setError("");
    setShowDevices(true);
    webDevicesList()
      .then(setDevices)
      .catch(() => {});
  };

  // Revoke a device in the backend, preventing reconnection and dropping any active connection, then refresh.
  const blockDevice = async (deviceId: string) => {
    setBlockBusy(true);
    setError("");
    try {
      await webDeviceRevoke(deviceId);
      setDevices(await webDevicesList());
    } catch (e) {
      setError(String(e));
    } finally {
      setBlockBusy(false);
      setConfirmId(null);
    }
  };

  const running = status?.running ?? false;
  // Prefer the backend's multi-interface URL list and fall back to its single URL. The selected IP's
  // URL moves to the front — derived from scheme and port when the IP is missing from the start-time
  // snapshot — so the primary displayed/copied link and the QR carry the chosen host.
  const baseUrls = status?.urls?.length
    ? status.urls
    : status?.url
      ? [status.url]
      : [];
  const urls = urlsForSelectedIp(
    baseUrls,
    effectiveIp,
    status?.port ?? null,
    status?.scheme ?? null,
  );

  // Advertised-IP selector, shown while stopped (below the port field) and while running (above the
  // pairing block); one persisted selection drives both.
  const ipSelector = (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        width: "100%",
        boxSizing: "border-box",
        marginBottom: 8,
        border: "1px solid var(--border)",
        borderRadius: "var(--r-sm, 6px)",
        background: "var(--bg-0)",
        overflow: "hidden",
      }}
    >
      <span
        style={{
          padding: "8px 10px",
          fontSize: 13,
          color: "var(--text-dim)",
          whiteSpace: "nowrap",
          borderRight: "1px solid var(--border)",
        }}
      >
        {t("remote.ipLabel")}
      </span>
      <select
        aria-label={t("remote.ipLabel")}
        value={effectiveIp}
        onChange={(e) => chooseIp(e.target.value)}
        style={{
          flex: 1,
          minWidth: 0,
          padding: "8px 6px",
          border: "none",
          background: "transparent",
          color: "var(--text)",
          fontSize: 13,
          outline: "none",
        }}
      >
        <option value="">{t("remote.ipAuto")}</option>
        {ifaces.map((i) => (
          <option key={`${i.name}-${i.ip}`} value={i.ip}>
            {i.ip} · {i.name}
            {i.vpn ? ` (${t("remote.ipVpn")})` : ""}
          </option>
        ))}
      </select>
    </div>
  );
  // Pairing fragments are interface-independent. Reuse the first link's `#pair=...` fragment with each host URL.
  const pairFragment = pairUrl ? pairUrl.slice(pairUrl.indexOf("/#") + 1) : "";
  const pairUrls =
    pairUrl && pairFragment && urls.length
      ? urls.map((u) => `${u.replace(/\/+$/, "")}/${pairFragment}`)
      : pairUrl
        ? [pairUrl]
        : [];

  return (
    <>
    <Backdrop onClose={onClose} zIndex={200} dim={false} center={false}>
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          position: "fixed",
          top: 44,
          right: 12,
          width: 280,
          // Cap to the viewport (44px top offset + 12px bottom margin) and scroll, so long content
          // such as the pairing QR code stays reachable instead of overflowing off-screen.
          maxHeight: "calc(100vh - 56px)",
          overflowY: "auto",
          background: "var(--bg-2)",
          border: "1px solid var(--border-strong)",
          borderRadius: "var(--r-md)",
          boxShadow: "var(--shadow)",
          padding: 14,
        }}
      >
        <div
          style={{
            fontSize: 10.5,
            letterSpacing: "1px",
            textTransform: "uppercase",
            color: "var(--text-dim)",
            fontWeight: 600,
            marginBottom: 4,
          }}
        >
          {t("remote.title")}
        </div>
        <div
          style={{
            fontSize: 11,
            color: "var(--text-dim)",
            lineHeight: 1.5,
            marginBottom: 12,
          }}
        >
          {t("remote.desc")}
        </div>

        {running ? (
          <>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                marginBottom: 10,
              }}
            >
              <span
                style={{
                  width: 7,
                  height: 7,
                  borderRadius: "50%",
                  background: "var(--accent)",
                  boxShadow: "0 0 6px var(--accent)",
                }}
              />
              <span style={{ fontSize: 12, color: "var(--text)" }}>
                {t("remote.running", status?.port ?? 0)}
              </span>
            </div>

            <div
              style={{
                fontSize: 11,
                color: "var(--text-dim)",
                lineHeight: 1.5,
                marginBottom: 8,
              }}
            >
              {t("remote.autoRestartHint")}
            </div>

            {status && status.fingerprint && (
              <>
                <div style={{ height: 4 }} />
                <div
                  style={{
                    fontSize: 10.5,
                    letterSpacing: "0.5px",
                    textTransform: "uppercase",
                    color: "var(--text-dim)",
                    fontWeight: 600,
                    marginBottom: 4,
                  }}
                >
                  {t("remote.fingerprintLabel")}
                </div>
                <button
                  onClick={() => {
                    void copyText(status.fingerprint ?? "");
                    setCopiedFp(true);
                    setTimeout(() => setCopiedFp(false), 1500);
                  }}
                  title={t("common.copy")}
                  style={{
                    width: "100%",
                    textAlign: "left",
                    padding: "7px 10px",
                    marginBottom: 6,
                    border: "1px solid var(--border)",
                    borderRadius: "var(--r-sm, 6px)",
                    background: "var(--bg-active)",
                    color: "var(--text)",
                    fontFamily: "var(--font-mono, monospace)",
                    fontSize: 11,
                    lineHeight: 1.5,
                    wordBreak: "break-all",
                    cursor: "pointer",
                  }}
                >
                  {status.fingerprint}
                  <span
                    style={{
                      display: "block",
                      marginTop: 4,
                      color: "var(--text-dim)",
                      fontSize: 10.5,
                    }}
                  >
                    {copiedFp ? t("common.copied") : t("common.copy")}
                  </span>
                </button>
                <div
                  style={{
                    fontSize: 11,
                    color: "var(--text-dim)",
                    lineHeight: 1.5,
                    marginBottom: 4,
                  }}
                >
                  {t("remote.fingerprintHint")}
                </div>
              </>
            )}

            <div style={{ height: 6 }} />

            {ipSelector}

            <button
              onClick={() => void genPairing(true)}
              disabled={pairBusy}
              style={btnStyle("accent", pairBusy)}
            >
              {pairBusy
                ? t("remote.pairingCreating")
                : t("remote.pairingRegenerate")}
            </button>

            {pairUrl && (
              <>
                <div
                  style={{
                    fontSize: 11,
                    color: "var(--text-dim)",
                    lineHeight: 1.5,
                    margin: "8px 0 4px",
                  }}
                >
                  {t("remote.pairingHint")}
                </div>
                {(showAllUrls ? pairUrls : pairUrls.slice(0, 1)).map((pu, i) => (
                  <button
                    key={pu}
                    onClick={() => {
                      void copyText(pu);
                      setCopiedUrl(pu);
                      setTimeout(() => setCopiedUrl(null), 1500);
                    }}
                    title={t("remote.copyUrl")}
                    style={{
                      width: "100%",
                      textAlign: "left",
                      padding: "7px 10px",
                      marginBottom: 6,
                      border:
                        i === 0
                          ? "1px solid var(--accent)"
                          : "1px solid var(--border)",
                      borderRadius: "var(--r-sm, 6px)",
                      background: "var(--bg-active)",
                      color: "var(--accent)",
                      fontFamily: "var(--font-mono, monospace)",
                      fontSize: 11,
                      lineHeight: 1.5,
                      wordBreak: "break-all",
                      cursor: "pointer",
                    }}
                  >
                    {pu}
                    <span
                      style={{
                        display: "block",
                        marginTop: 4,
                        color: "var(--text-dim)",
                        fontSize: 10.5,
                      }}
                    >
                      {copiedUrl === pu ? t("common.copied") : t("common.copy")}
                    </span>
                  </button>
                ))}
                {pairUrls.length > 1 && (
                  <button
                    onClick={() => setShowAllUrls((v) => !v)}
                    style={{
                      width: "100%",
                      textAlign: "left",
                      padding: "4px 10px",
                      marginBottom: 6,
                      border: "none",
                      background: "transparent",
                      color: "var(--text-dim)",
                      fontSize: 11.5,
                      cursor: "pointer",
                    }}
                  >
                    <span style={{ marginRight: 5 }}>
                      {showAllUrls ? "▾" : "▸"}
                    </span>
                    {showAllUrls
                      ? t("remote.lessUrls")
                      : t("remote.moreUrls", pairUrls.length - 1)}
                  </button>
                )}
                {pairUrls.length > 0 && (
                  <>
                    {/* White padded background keeps the QR scannable in the dark theme. */}
                    <div
                      style={{
                        display: "flex",
                        justifyContent: "center",
                        padding: 10,
                        marginBottom: 4,
                        background: "#fff",
                        borderRadius: "var(--r-sm, 6px)",
                      }}
                    >
                      <QRCodeSVG
                        value={pairUrls[0]}
                        size={160}
                        level="M"
                        marginSize={1}
                      />
                    </div>
                    <div
                      style={{
                        fontSize: 11,
                        color: "var(--text-dim)",
                        lineHeight: 1.5,
                        marginBottom: 6,
                      }}
                    >
                      {t("remote.qrHint")}
                    </div>
                  </>
                )}
              </>
            )}

            <div style={{ height: 6 }} />

            <button
              onClick={openDevices}
              style={{
                width: "100%",
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                gap: 8,
                padding: "8px 10px",
                border: "1px solid var(--border)",
                borderRadius: "var(--r-sm, 6px)",
                background: "var(--bg-active)",
                color: "var(--text)",
                fontSize: 12.5,
                cursor: "pointer",
              }}
            >
              <span>{t("remote.devicesLabel")}</span>
              <span style={{ color: "var(--text-dim)" }}>
                {devices.length} ›
              </span>
            </button>

            <div style={{ height: 6 }} />

            <button
              onClick={stop}
              disabled={busy}
              style={btnStyle("danger", busy)}
            >
              {t("remote.stop")}
            </button>
          </>
        ) : (
          <>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                width: "100%",
                boxSizing: "border-box",
                marginBottom: 8,
                border: "1px solid var(--border)",
                borderRadius: "var(--r-sm, 6px)",
                background: "var(--bg-0)",
                overflow: "hidden",
              }}
            >
              <span
                style={{
                  padding: "8px 10px",
                  fontSize: 13,
                  color: "var(--text-dim)",
                  whiteSpace: "nowrap",
                  borderRight: "1px solid var(--border)",
                }}
              >
                {t("remote.portLabel")}
              </span>
              <input
                type="text"
                inputMode="numeric"
                value={port}
                placeholder="8799"
                onChange={(e) => {
                  portTouched.current = true;
                  setPort(e.target.value.replace(/[^0-9]/g, ""));
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void start();
                }}
                style={{
                  flex: 1,
                  minWidth: 0,
                  padding: "8px 10px",
                  border: "none",
                  background: "transparent",
                  color: "var(--text)",
                  fontSize: 13,
                  outline: "none",
                }}
              />
            </div>
            {ipSelector}
            <input
              type="password"
              value={password}
              placeholder={t("remote.passwordPlaceholder")}
              autoFocus
              onChange={(e) => setPassword(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void start();
              }}
              style={{
                width: "100%",
                boxSizing: "border-box",
                padding: "8px 10px",
                marginBottom: 12,
                border: "1px solid var(--border)",
                borderRadius: "var(--r-sm, 6px)",
                background: "var(--bg-0)",
                color: "var(--text)",
                fontSize: 13,
                outline: "none",
              }}
            />
            <button
              onClick={start}
              disabled={busy}
              style={btnStyle("accent", busy)}
            >
              {busy ? t("remote.starting") : t("remote.start")}
            </button>
          </>
        )}

        {status?.autostartError && (
          <div
            style={{
              marginTop: 10,
              fontSize: 11,
              color: "var(--danger, #ff6b6b)",
              lineHeight: 1.4,
            }}
          >
            {t("remote.autostartFailed")} {status.autostartError}
          </div>
        )}

        {error && (
          <div
            style={{
              marginTop: 10,
              fontSize: 11,
              color: "var(--danger, #ff6b6b)",
              lineHeight: 1.4,
            }}
          >
            {error}
          </div>
        )}
      </div>
    </Backdrop>

      {showDevices && (
        <Backdrop
          onClose={() => {
            setShowDevices(false);
            setConfirmId(null);
          }}
          zIndex={300}
        >
          <div
            onClick={(e) => e.stopPropagation()}
            style={{
              width: 340,
              maxWidth: "90vw",
              maxHeight: "70vh",
              overflowY: "auto",
              background: "var(--bg-2)",
              border: "1px solid var(--border-strong)",
              borderRadius: "var(--r-md)",
              boxShadow: "var(--shadow)",
              padding: 16,
            }}
          >
            <div
              style={{
                fontSize: 10.5,
                letterSpacing: "1px",
                textTransform: "uppercase",
                color: "var(--text-dim)",
                fontWeight: 600,
                marginBottom: 4,
              }}
            >
              {t("remote.devicesLabel")}
            </div>
            <div
              style={{
                fontSize: 11,
                color: "var(--text-dim)",
                lineHeight: 1.5,
                marginBottom: 12,
              }}
            >
              {t("remote.deviceBlockHint")}
            </div>

            {devices.length === 0 ? (
              <div
                style={{
                  fontSize: 12,
                  color: "var(--text-dim)",
                  padding: "16px 0",
                  textAlign: "center",
                }}
              >
                {t("remote.devicesEmpty")}
              </div>
            ) : (
              devices.map((d) => (
                <div
                  key={d.deviceId}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                    padding: "8px 0",
                    borderTop: "1px solid var(--border)",
                  }}
                >
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div
                      style={{
                        fontSize: 12.5,
                        color: "var(--text)",
                        whiteSpace: "nowrap",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                      }}
                    >
                      {d.name}
                    </div>
                    <div style={{ fontSize: 10.5, color: "var(--text-dim)" }}>
                      {t("remote.lastSeen")}:{" "}
                      {new Date(d.lastSeenAt * 1000).toLocaleString()}
                    </div>
                  </div>
                  {confirmId === d.deviceId ? (
                    <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
                      <button
                        onClick={() => void blockDevice(d.deviceId)}
                        disabled={blockBusy}
                        style={{
                          padding: "5px 10px",
                          border: "none",
                          borderRadius: "var(--r-sm, 6px)",
                          background: "var(--danger, #ff6b6b)",
                          color: "var(--bg-0)",
                          fontSize: 11.5,
                          fontWeight: 600,
                          cursor: blockBusy ? "default" : "pointer",
                          opacity: blockBusy ? 0.6 : 1,
                        }}
                      >
                        {t("remote.deviceBlockConfirm")}
                      </button>
                      <button
                        onClick={() => setConfirmId(null)}
                        disabled={blockBusy}
                        style={{
                          padding: "5px 10px",
                          border: "1px solid var(--border)",
                          borderRadius: "var(--r-sm, 6px)",
                          background: "transparent",
                          color: "var(--text-dim)",
                          fontSize: 11.5,
                          cursor: "pointer",
                        }}
                      >
                        {t("common.cancel")}
                      </button>
                    </div>
                  ) : (
                    <button
                      onClick={() => setConfirmId(d.deviceId)}
                      style={{
                        flexShrink: 0,
                        padding: "5px 10px",
                        border: "1px solid var(--danger, #ff6b6b)",
                        borderRadius: "var(--r-sm, 6px)",
                        background: "transparent",
                        color: "var(--danger, #ff6b6b)",
                        fontSize: 11.5,
                        cursor: "pointer",
                      }}
                    >
                      {t("remote.deviceBlock")}
                    </button>
                  )}
                </div>
              ))
            )}

            {error && (
              <div
                style={{
                  marginTop: 10,
                  fontSize: 11,
                  color: "var(--danger, #ff6b6b)",
                  lineHeight: 1.4,
                }}
              >
                {error}
              </div>
            )}

            <div style={{ height: 12 }} />
            <button
              onClick={() => {
                setShowDevices(false);
                setConfirmId(null);
              }}
              style={{
                width: "100%",
                padding: "8px 0",
                border: "1px solid var(--border)",
                borderRadius: "var(--r-sm, 6px)",
                background: "transparent",
                color: "var(--text)",
                fontSize: 12.5,
                cursor: "pointer",
              }}
            >
              {t("common.close")}
            </button>
          </div>
        </Backdrop>
      )}
    </>
  );
}

function btnStyle(
  kind: "accent" | "danger",
  busy: boolean,
): React.CSSProperties {
  return {
    width: "100%",
    padding: "8px 0",
    border: "none",
    borderRadius: "var(--r-sm, 6px)",
    background:
      kind === "danger" ? "var(--danger, #ff6b6b)" : "var(--accent)",
    color: "var(--bg-0)",
    fontSize: 12.5,
    fontWeight: 600,
    cursor: busy ? "default" : "pointer",
    opacity: busy ? 0.6 : 1,
  };
}
