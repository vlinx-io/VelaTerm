import { useState, useEffect } from "react";
import { useT } from "../../i18n";
import { Backdrop } from "../../components/Backdrop";
import { PasswordField } from "../../components/PasswordField";
import { invoke, listen } from "../../ipc/transport";

//! Client panel for connecting to a remote service through one of two modes:
//! - SSH: enter user@host, verify new or changed host fingerprints, trust through known_hosts, then probe
//!   the system, provision vela-server, start serving, forward a port, and log in automatically. Backend
//!   `ssh://progress` events drive a progress bar. The four latest hosts appear inline and older entries
//!   under View All; selecting one fills the form without connecting. Password prompts optionally remember
//!   secrets in the system keyring and default to off.
//! - URL: paste a pairing link and password, verify the remote TLS fingerprint, then open a remote window
//!   with automatic login. URL history and optional keyring password storage behave the same way.

/** Matches backend `ssh_remote::HostKeyProbe` with serde camelCase. */
type HostKeyProbe = {
  status: "known" | "new" | "changed";
  target: string;
  keyType: string;
  fingerprint: string;
};

/** Matches backend `commands::UrlKeyProbe`: TOFU result for a remote URL's TLS certificate. `known`
 * connects directly, `new` requests initial confirmation, and `changed` warns about a changed fingerprint. */
type UrlKeyProbe = {
  status: "known" | "new" | "changed";
  fingerprint: string;
};

/** Matches backend `ssh_remote::SshHostInfo`: connection history, remembered-password state, and the last
 * remote-desktop database reuse choice for form restoration. */
type SshHostInfo = {
  target: string;
  label: string | null;
  lastConnectedAt: number;
  hasPassword: boolean;
  sharedDb: boolean;
  mirror: boolean;
};

/** Matches backend `command_core::UrlHostInfo`: URL pairing history and remembered-password state. */
type UrlHostInfo = {
  url: string;
  hasPassword: boolean;
};

type Mode = "ssh" | "url";

/** Maximum recent connections shown inline; remaining entries appear under View All. */
const RECENT_INLINE_MAX = 4;

/** Matches backend AUTH_REQUIRED_TAG; this public-key rejection prefix opens password input. */
const AUTH_REQUIRED_TAG = "__VLX_SSH_AUTH_REQUIRED__";

/** Whether to show the temporarily hidden shared-remote-database checkbox. Option/Alt-clicking the title
 * bar button enables it; otherwise connections always use an independent database. */
export function ConnectRemotePanel({
  onClose,
  showSharedDb = false,
}: {
  onClose: () => void;
  showSharedDb?: boolean;
}) {
  const t = useT();
  const [mode, setMode] = useState<Mode>("ssh");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  // URL mode.
  const [pairingUrl, setPairingUrl] = useState("");
  // TLS TOFU result: null before/without confirmation; new or changed while awaiting confirmation.
  const [urlProbe, setUrlProbe] = useState<UrlKeyProbe | null>(null);
  // URL history and remembered-password flags, plus the current password and opt-in choice.
  const [savedUrls, setSavedUrls] = useState<UrlHostInfo[]>([]);
  const [showAllUrls, setShowAllUrls] = useState(false);
  const [urlPassword, setUrlPassword] = useState("");
  const [urlRemember, setUrlRemember] = useState(false);

  // SSH mode.
  const [sshHost, setSshHost] = useState("");
  const [probe, setProbe] = useState<HostKeyProbe | null>(null);
  // After public-key rejection, pwHost identifies the password prompt target; null hides the prompt.
  const [pwHost, setPwHost] = useState<string | null>(null);
  const [sshPassword, setSshPassword] = useState("");
  // SSH history, current remember-password opt-in, and View All visibility.
  const [savedHosts, setSavedHosts] = useState<SshHostInfo[]>([]);
  const [rememberPw, setRememberPw] = useState(false);
  const [showAllHosts, setShowAllHosts] = useState(false);
  // Database mode defaults to independent; opting in shares the remote desktop release's vlx-term.db.
  // The hidden checkbox keeps this false unless showSharedDb is enabled.
  const [sharedDb, setSharedDb] = useState(false);
  // Mirror mode for the service started on the remote machine. That machine is headless and has no panel
  // of its own, so the choice travels with the connection. Hidden behind the same Option/Alt reveal.
  const [mirror, setMirror] = useState(false);
  // SSH progress from backend `ssh://progress`: stage code and optional percentage.
  const [progress, setProgress] = useState<{ stage: string; percent: number | null } | null>(null);
  useEffect(() => {
    const un = listen<{ stage: string; percent: number | null }>(
      "ssh://progress",
      (p) => setProgress(p),
    );
    return () => {
      void un.then((f) => f());
    };
  }, []);

  // Load connection history for SSH mode.
  const loadHosts = () => {
    void invoke<SshHostInfo[]>("ssh_hosts_list")
      .then(setSavedHosts)
      .catch(() => setSavedHosts([]));
  };
  useEffect(() => {
    loadHosts();
  }, []);
  const forgetHost = async (target: string) => {
    try {
      await invoke("ssh_host_forget", { target });
    } catch {
      /* Forget failures must not interrupt the panel. */
    }
    loadHosts();
  };

  // Load recent URL connections.
  const loadUrls = () => {
    void invoke<UrlHostInfo[]>("url_hosts_list")
      .then(setSavedUrls)
      .catch(() => setSavedUrls([]));
  };
  useEffect(() => {
    loadUrls();
  }, []);
  const forgetUrl = async (url: string) => {
    try {
      await invoke("url_host_forget", { url });
    } catch {
      /* Ignore history-loading failures. */
    }
    loadUrls();
  };

  const resetTransient = () => {
    setError("");
    setUrlProbe(null);
    setProbe(null);
    setPwHost(null);
    setSshPassword("");
    setUrlPassword("");
    setUrlRemember(false);
    setShowAllHosts(false);
    setShowAllUrls(false);
    setSharedDb(false);
    setMirror(false);
  };

  // ── URL mode: probe certificate, confirm new/changed fingerprints, then open the window ──
  const urlStart = async () => {
    const u = pairingUrl.trim();
    if (u === "") return;
    setBusy(true);
    setError("");
    try {
      const p = await invoke<UrlKeyProbe>("probe_remote_fingerprint", { pairingUrl: u });
      if (p.status === "known") {
        await openUrl(u, urlPassword, urlRemember, null); // Known fingerprint: open without confirmation.
      } else {
        setUrlProbe(p); // New or changed fingerprint: request confirmation.
        setBusy(false);
      }
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };
  // Open a remote window with the form password injected for automatic login, record history, and store
  // the password only in the `{identifier}.url` keyring service when opted in. A confirmed trustFp is
  // persisted so an unchanged fingerprint at the same host and port connects directly next time.
  const openUrl = async (
    url: string,
    pw: string,
    remember: boolean,
    trustFp: string | null,
  ) => {
    setBusy(true);
    setError("");
    try {
      await invoke("open_remote_window", { pairingUrl: url, password: pw || null });
      if (trustFp) {
        void invoke("url_trust_fingerprint", { pairingUrl: url, fingerprint: trustFp }).catch(
          () => {},
        );
      }
      void invoke("url_host_record", {
        url,
        password: remember && pw ? pw : null,
        remember,
      }).catch(() => {});
      onClose();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };
  const urlConfirm = () =>
    openUrl(pairingUrl.trim(), urlPassword, urlRemember, urlProbe?.fingerprint ?? null);
  // Selecting URL history fills the form without connecting. Restore a remembered password and checkbox
  // when available. Reconnecting while unchecked tells the backend to delete the stored secret.
  const pickUrl = async (u: UrlHostInfo) => {
    setShowAllUrls(false);
    setPairingUrl(u.url);
    setUrlProbe(null);
    setUrlPassword("");
    setUrlRemember(false);
    if (u.hasPassword) {
      try {
        const pw = await invoke<string | null>("url_host_password", { url: u.url });
        if (pw) {
          setUrlPassword(pw);
          setUrlRemember(true);
        }
      } catch {
        /* Leave it empty for manual entry when retrieval fails. */
      }
    }
  };

  // ── SSH mode: probe host key, confirm new/changed keys, trust, and connect ──
  const sshStart = async () => {
    const h = sshHost.trim();
    if (h === "") return;
    setBusy(true);
    setError("");
    try {
      const p = await invoke<HostKeyProbe>("ssh_probe_host", { host: h });
      if (p.status === "known") {
        await sshConnect(false, undefined, h); // Known matching host key: connect directly.
      } else {
        setProbe(p); // New or changed host key: request confirmation.
        setBusy(false); // Re-enable confirmation instead of leaving it stuck on Connecting.
      }
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };
  const sshConnect = async (needTrust: boolean, pw?: string, hostArg?: string) => {
    const h = (hostArg ?? sshHost).trim();
    setBusy(true);
    setError("");
    setProgress(null);
    try {
      if (needTrust && probe) {
        await invoke("ssh_trust_host", {
          host: h,
          wasChanged: probe.status === "changed",
        });
      }
      await invoke<string>("ssh_connect", {
        host: h,
        password: pw ?? null,
        remember: rememberPw,
        // Hidden shared-database controls always use an independent database, ignoring restored history.
        sharedDb: showSharedDb && sharedDb,
        // Same for mirror mode: without the hidden controls the remote service starts with it off.
        mirror: showSharedDb && mirror,
      });
      onClose(); // Connection succeeded and the auto-login window is open.
    } catch (e) {
      const msg = String(e);
      if (msg.includes(AUTH_REQUIRED_TAG)) {
        // Public-key auth failed without a stored password: prompt for one and hide the low-level error.
        setPwHost(h);
        setError("");
      } else {
        // Show password-retry failures as-is and retain the prompt for another attempt.
        setError(msg);
      }
      setBusy(false);
    }
  };

  // Selecting SSH history fills the form and restores shared-database state only when visible. It never
  // connects immediately, and it closes View All so the user can verify and click Connect.
  const pickHost = (h: SshHostInfo) => {
    setShowAllHosts(false);
    setSshHost(h.target);
    setSharedDb(showSharedDb && h.sharedDb);
    setMirror(showSharedDb && h.mirror);
    setProbe(null);
    setError("");
    setPwHost(null);
    setSshPassword("");
  };
  // Password prompt submit reconnects without retrusting the already accepted fingerprint; cancel closes it.
  const submitPw = () => {
    if (sshPassword === "" || pwHost === null) return;
    void sshConnect(false, sshPassword, pwHost);
  };
  const cancelPw = () => {
    setPwHost(null);
    setSshPassword("");
    setError("");
    setBusy(false);
    setProgress(null);
  };

  // Map progress stages to labels, adding percentages for supply/transfer and grouping probe/connect.
  const stageText = (): string => {
    if (!progress) return t("connect.connecting");
    switch (progress.stage) {
      case "supply":
        return progress.percent != null
          ? `${t("connect.stagePreparing")} ${progress.percent}%`
          : t("connect.stagePreparing");
      case "transfer":
        return progress.percent != null
          ? `${t("connect.stageTransferring")} ${progress.percent}%`
          : t("connect.stageTransferring");
      case "start":
      case "forward":
        return t("connect.stageStarting");
      default:
        return t("connect.connecting");
    }
  };

  const primaryLabel = busy
    ? stageText()
    : mode === "url"
      ? urlProbe
        ? t("connect.confirmConnect")
        : t("connect.connect")
      : probe
        ? t("connect.confirmConnect")
        : t("connect.connect");

  const onPrimary = () => {
    if (mode === "url") {
      void (urlProbe ? urlConfirm() : urlStart());
      return;
    }
    void (probe ? sshConnect(true) : sshStart());
  };
  const onEnter = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") onPrimary();
  };

  const canStart =
    mode === "url" ? pairingUrl.trim() !== "" : sshHost.trim() !== "";
  const danger =
    (mode === "ssh" && probe?.status === "changed") ||
    (mode === "url" && urlProbe?.status === "changed");

  // Shared recent-host row: click to fill the form or × to forget the host and stored password.
  const renderHostRow = (h: SshHostInfo) => (
    <div key={h.target} style={savedRowStyle}>
      <button
        type="button"
        onClick={() => pickHost(h)}
        disabled={busy}
        className="connect-recent-item"
        title={h.target}
      >
        {h.hasPassword && (
          <span
            title={t("connect.savedHasPassword")}
            style={{ display: "inline-flex", flexShrink: 0, color: "var(--accent)" }}
          >
            <KeyGlyph />
          </span>
        )}
        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {h.target}
        </span>
      </button>
      <button
        type="button"
        onClick={() => void forgetHost(h.target)}
        disabled={busy}
        className="connect-recent-x"
        title={t("connect.forgetHost")}
        aria-label={t("connect.forgetHost")}
      >
        ×
      </button>
    </div>
  );

  // Recent URL rows display complete pairing links, including #pair, because one host may have multiple
  // tokens that would otherwise appear identical. Long links truncate with a full tooltip. A key marks
  // remembered passwords; selecting restores the form and × removes both history and stored password.
  const renderUrlRow = (u: UrlHostInfo) => {
    return (
      <div key={u.url} style={savedRowStyle}>
        <button
          type="button"
          onClick={() => void pickUrl(u)}
          disabled={busy}
          className="connect-recent-item"
          title={u.url}
        >
          {u.hasPassword && (
            <span
              title={t("connect.savedHasPassword")}
              style={{ display: "inline-flex", flexShrink: 0, color: "var(--accent)" }}
            >
              <KeyGlyph />
            </span>
          )}
          <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
            {u.url}
          </span>
        </button>
        <button
          type="button"
          onClick={() => void forgetUrl(u.url)}
          disabled={busy}
          className="connect-recent-x"
          title={t("connect.forgetHost")}
          aria-label={t("connect.forgetHost")}
        >
          ×
        </button>
      </div>
    );
  };

  return (
    <>
      <Backdrop onClose={onClose} zIndex={200} dim={false} center={false}>
        <div
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => {
            if (e.key === "Escape") onClose();
          }}
          style={{
            position: "fixed",
            top: 44,
            right: 12,
            width: 300,
            background: "var(--bg-2)",
            border: "1px solid var(--border-strong)",
            borderRadius: "var(--r-md)",
            boxShadow: "var(--shadow)",
            padding: 14,
          }}
        >
          <div style={sectionLabelStyle}>{t("connect.title")}</div>

          {/* Mode switch: SSH or URL. */}
          <div style={{ display: "flex", gap: 6, margin: "2px 0 10px" }}>
            {(["ssh", "url"] as Mode[]).map((m) => (
              <button
                key={m}
                onClick={() => {
                  setMode(m);
                  resetTransient();
                }}
                style={{
                  flex: 1,
                  padding: "6px 0",
                  border: "1px solid var(--border)",
                  borderRadius: "var(--r-sm, 6px)",
                  background: mode === m ? "var(--accent)" : "var(--bg-0)",
                  color: mode === m ? "var(--bg-0)" : "var(--text-dim)",
                  fontSize: 12,
                  fontWeight: 600,
                  cursor: "pointer",
                }}
              >
                {m === "ssh" ? "SSH" : "URL"}
              </button>
            ))}
          </div>

          {mode === "url" ? (
            <input
              type="text"
              value={pairingUrl}
              placeholder={t("connect.pairingPlaceholder")}
              autoFocus
              onChange={(e) => {
                setPairingUrl(e.target.value);
                setUrlProbe(null);
              }}
              onKeyDown={onEnter}
              style={inputStyle}
            />
          ) : (
            <input
              type="text"
              value={sshHost}
              placeholder="user@host[:port]"
              autoFocus
              onChange={(e) => {
                setSshHost(e.target.value);
                setProbe(null);
                setError("");
                setPwHost(null);
                setSshPassword("");
              }}
              onKeyDown={onEnter}
              style={inputStyle}
            />
          )}

          {/* URL mode: the login password, injected into the remote window on connect so the page does not ask again, plus Remember password, unchecked by default. */}
          {mode === "url" && (
            <>
              <PasswordField
                value={urlPassword}
                placeholder={t("connect.urlPasswordPlaceholder")}
                onChange={setUrlPassword}
                onKeyDown={onEnter}
                wrapStyle={{ marginTop: 8 }}
              />
              <label style={rememberRowStyle}>
                <input
                  type="checkbox"
                  checked={urlRemember}
                  onChange={(e) => setUrlRemember(e.target.checked)}
                  style={{ cursor: "pointer" }}
                />
                {t("connect.rememberPassword")}
              </label>
            </>
          )}

          {/* SSH mode: two switches for the service this connection starts on the remote machine. The data
              mode defaults to a separate database; when checked, the remote desktop release's database is
              reused instead (the same vlx-term.db). Mirror mode defaults off and, when checked, keeps tabs,
              splits, and the active session identical across every client of that remote service — the same
              switch the remote-access panel offers, which that headless machine has no way to show. Each one
              explains itself in a note once checked. Both are hidden and appear only when the panel is
              opened by Option/Alt-clicking the Connect remote button in the title bar. */}
          {mode === "ssh" && !probe && showSharedDb && (
            <>
              <label style={rememberRowStyle}>
                <input
                  type="checkbox"
                  checked={sharedDb}
                  onChange={(e) => setSharedDb(e.target.checked)}
                  style={{ cursor: "pointer" }}
                />
                {t("connect.shareDesktopDb")}
              </label>
              {sharedDb && <div style={hintStyle}>{t("connect.shareDesktopDbHint")}</div>}
              <label style={rememberRowStyle}>
                <input
                  type="checkbox"
                  checked={mirror}
                  onChange={(e) => setMirror(e.target.checked)}
                  style={{ cursor: "pointer" }}
                />
                {t("connect.mirror")}
              </label>
              {mirror && <div style={hintStyle}>{t("connect.mirrorHint")}</div>}
            </>
          )}

          {/* SSH mode: previously connected hosts. The panel lists only the four most recent; the rest go into the View all dialog. */}
          {mode === "ssh" && !probe && savedHosts.length > 0 && (
            <div style={{ marginTop: 10 }}>
              <div style={sectionLabelStyle}>{t("connect.savedHosts")}</div>
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                {savedHosts.slice(0, RECENT_INLINE_MAX).map(renderHostRow)}
              </div>
              {savedHosts.length > RECENT_INLINE_MAX && (
                <button
                  type="button"
                  onClick={() => setShowAllHosts(true)}
                  disabled={busy}
                  style={showAllBtnStyle}
                >
                  {t("connect.showAllHosts", savedHosts.length)}
                </button>
              )}
            </div>
          )}

          {/* URL mode: recent connections as pairing links. Clicking reconnects, logging in automatically if the password was remembered; × forgets the entry. */}
          {mode === "url" && !urlProbe && savedUrls.length > 0 && (
            <div style={{ marginTop: 10 }}>
              <div style={sectionLabelStyle}>{t("connect.savedHosts")}</div>
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                {savedUrls.slice(0, RECENT_INLINE_MAX).map(renderUrlRow)}
              </div>
              {savedUrls.length > RECENT_INLINE_MAX && (
                <button
                  type="button"
                  onClick={() => setShowAllUrls(true)}
                  disabled={busy}
                  style={showAllBtnStyle}
                >
                  {t("connect.showAllHosts", savedUrls.length)}
                </button>
              )}
            </div>
          )}

          {/* URL mode: certificate fingerprint check, a neutral notice for a new endpoint and a danger warning when the fingerprint changes. */}
          {mode === "url" && urlProbe && (
            <>
              <div style={{ ...sectionLabelStyle, margin: "10px 0 4px" }}>
                {t("remote.fingerprintLabel")}
              </div>
              <div style={fpBoxStyle(urlProbe.status === "changed")}>{urlProbe.fingerprint}</div>
              <div
                style={{
                  ...hintStyle,
                  color:
                    urlProbe.status === "changed" ? "var(--danger, #ff6b6b)" : "var(--text-dim)",
                }}
              >
                {urlProbe.status === "changed"
                  ? t("connect.urlCertChanged")
                  : t("remote.fingerprintHint")}
              </div>
            </>
          )}

          {/* SSH mode: host fingerprint check, neutral for a new host and dangerous when the key changes. */}
          {mode === "ssh" && probe && (
            <>
              <div style={{ ...sectionLabelStyle, margin: "10px 0 4px" }}>
                {t("connect.sshFingerprintLabel", probe.keyType)}
              </div>
              <div style={fpBoxStyle(danger)}>{probe.fingerprint}</div>
              <div style={{ ...hintStyle, color: danger ? "var(--danger, #ff6b6b)" : "var(--text-dim)" }}>
                {probe.status === "changed"
                  ? t("connect.sshHostChanged")
                  : t("connect.sshHostNew")}
              </div>
            </>
          )}

          <button
            onClick={onPrimary}
            disabled={busy || !canStart}
            style={{
              width: "100%",
              marginTop: 12,
              padding: "8px 0",
              border: "none",
              borderRadius: "var(--r-sm, 6px)",
              background: danger ? "var(--danger, #ff6b6b)" : "var(--accent)",
              color: "var(--bg-0)",
              fontSize: 12.5,
              fontWeight: 600,
              cursor: busy ? "default" : "pointer",
              opacity: busy || !canStart ? 0.6 : 1,
            }}
          >
            {primaryLabel}
          </button>

          {busy && progress?.percent != null && (
            <div
              style={{
                marginTop: 8,
                height: 4,
                borderRadius: 2,
                background: "var(--bg-active)",
                overflow: "hidden",
              }}
            >
              <div
                style={{
                  width: `${progress.percent}%`,
                  height: "100%",
                  background: "var(--accent)",
                  transition: "width 0.3s ease",
                }}
              />
            </div>
          )}

          {error && pwHost === null && <div style={errorStyle}>{error}</div>}
        </div>
      </Backdrop>

      {/* The View all dialog: every recent host, click to connect and × to forget. */}
      {showAllHosts && (
        <Backdrop onClose={() => setShowAllHosts(false)} zIndex={300} dim center>
          <div onClick={(e) => e.stopPropagation()} style={modalStyle}>
            <div style={sectionLabelStyle}>{t("connect.savedHostsAll")}</div>
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                gap: 4,
                maxHeight: 360,
                overflowY: "auto",
              }}
            >
              {savedHosts.map(renderHostRow)}
            </div>
            <button
              type="button"
              onClick={() => setShowAllHosts(false)}
              style={{ ...modalSecondaryBtnStyle, marginTop: 12, width: "100%" }}
            >
              {t("common.close")}
            </button>
          </div>
        </Backdrop>
      )}

      {/* The View all dialog for URL mode: every recent pairing link, click to reconnect and × to forget. */}
      {showAllUrls && (
        <Backdrop onClose={() => setShowAllUrls(false)} zIndex={300} dim center>
          <div onClick={(e) => e.stopPropagation()} style={modalStyle}>
            <div style={sectionLabelStyle}>{t("connect.savedHostsAll")}</div>
            <div
              style={{
                display: "flex",
                flexDirection: "column",
                gap: 4,
                maxHeight: 360,
                overflowY: "auto",
              }}
            >
              {savedUrls.map(renderUrlRow)}
            </div>
            <button
              type="button"
              onClick={() => setShowAllUrls(false)}
              style={{ ...modalSecondaryBtnStyle, marginTop: 12, width: "100%" }}
            >
              {t("common.close")}
            </button>
          </div>
        </Backdrop>
      )}

      {/* Password dialog: shown when public-key authentication is refused and no remembered password is available, falling back to the backend PTY authentication path. */}
      {pwHost !== null && (
        <Backdrop onClose={cancelPw} zIndex={300} dim center>
          <div onClick={(e) => e.stopPropagation()} style={modalStyle}>
            <div style={sectionLabelStyle}>{t("connect.sshPasswordLabel")}</div>
            <div
              style={{
                fontFamily: "var(--font-mono, monospace)",
                fontSize: 12,
                color: "var(--text)",
                margin: "0 0 8px",
                wordBreak: "break-all",
              }}
            >
              {pwHost}
            </div>
            <PasswordField
              value={sshPassword}
              placeholder={t("connect.sshPasswordPlaceholder")}
              autoFocus
              onChange={setSshPassword}
              onKeyDown={(e) => {
                if (e.key === "Enter") submitPw();
              }}
            />
            <label style={rememberRowStyle}>
              <input
                type="checkbox"
                checked={rememberPw}
                onChange={(e) => setRememberPw(e.target.checked)}
                style={{ cursor: "pointer" }}
              />
              {t("connect.rememberPassword")}
            </label>

            {error && <div style={errorStyle}>{error}</div>}

            <div style={{ display: "flex", gap: 8, marginTop: 12 }}>
              <button
                type="button"
                onClick={cancelPw}
                disabled={busy}
                style={{ ...modalSecondaryBtnStyle, flex: 1 }}
              >
                {t("common.cancel")}
              </button>
              <button
                type="button"
                onClick={submitPw}
                disabled={busy || sshPassword === ""}
                style={{
                  ...modalPrimaryBtnStyle,
                  cursor: busy ? "default" : "pointer",
                  opacity: busy || sshPassword === "" ? 0.6 : 1,
                }}
              >
                {busy ? stageText() : t("connect.connect")}
              </button>
            </div>
          </div>
        </Backdrop>
      )}
    </>
  );
}

/** 12px Lucide key icon for history entries with remembered passwords, inheriting currentColor. */
function KeyGlyph() {
  return (
    <svg
      width="12"
      height="12"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M2.586 17.414A2 2 0 0 0 2 18.828V21a1 1 0 0 0 1 1h3a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h1a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h.172a2 2 0 0 0 1.414-.586l.814-.814a6.5 6.5 0 1 0-4-4z" />
      <circle cx="16.5" cy="7.5" r=".5" fill="currentColor" />
    </svg>
  );
}

const sectionLabelStyle: React.CSSProperties = {
  fontSize: 10.5,
  letterSpacing: "1px",
  textTransform: "uppercase",
  color: "var(--text-dim)",
  fontWeight: 600,
  marginBottom: 8,
};

const hintStyle: React.CSSProperties = {
  fontSize: 11,
  color: "var(--text-dim)",
  lineHeight: 1.5,
  margin: "4px 0 0",
};

const errorStyle: React.CSSProperties = {
  marginTop: 10,
  fontSize: 11,
  color: "var(--danger, #ff6b6b)",
  lineHeight: 1.4,
};

const fpBoxStyle = (danger: boolean): React.CSSProperties => ({
  padding: "7px 10px",
  border: `1px solid ${danger ? "var(--danger, #ff6b6b)" : "var(--border)"}`,
  borderRadius: "var(--r-sm, 6px)",
  background: "var(--bg-active)",
  color: "var(--text)",
  fontFamily: "var(--font-mono, monospace)",
  fontSize: 11,
  lineHeight: 1.5,
  wordBreak: "break-all",
});

const inputStyle: React.CSSProperties = {
  width: "100%",
  boxSizing: "border-box",
  padding: "8px 10px",
  border: "1px solid var(--border)",
  borderRadius: "var(--r-sm, 6px)",
  background: "var(--bg-0)",
  color: "var(--text)",
  fontSize: 13,
  outline: "none",
};

const savedRowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 4,
};

const showAllBtnStyle: React.CSSProperties = {
  width: "100%",
  marginTop: 6,
  padding: "6px 0",
  border: "1px dashed var(--border)",
  borderRadius: "var(--r-sm, 6px)",
  background: "transparent",
  color: "var(--text-dim)",
  fontSize: 11.5,
  fontWeight: 600,
  cursor: "pointer",
};

const rememberRowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 7,
  marginTop: 8,
  fontSize: 12,
  color: "var(--text-dim)",
  cursor: "pointer",
};

const modalStyle: React.CSSProperties = {
  width: "min(360px, calc(100vw - 32px))",
  background: "var(--bg-2)",
  border: "1px solid var(--border-strong)",
  borderRadius: "var(--r-md)",
  boxShadow: "var(--shadow)",
  padding: 16,
};

const modalPrimaryBtnStyle: React.CSSProperties = {
  flex: 1,
  padding: "8px 0",
  border: "none",
  borderRadius: "var(--r-sm, 6px)",
  background: "var(--accent)",
  color: "var(--bg-0)",
  fontSize: 12.5,
  fontWeight: 600,
};

const modalSecondaryBtnStyle: React.CSSProperties = {
  padding: "8px 0",
  border: "1px solid var(--border)",
  borderRadius: "var(--r-sm, 6px)",
  background: "var(--bg-0)",
  color: "var(--text-dim)",
  fontSize: 12.5,
  fontWeight: 600,
  cursor: "pointer",
};
