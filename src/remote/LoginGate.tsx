//! Login gate for browser remote access:
//! - Desktop Tauri clients render the application directly.
//! - Browser clients use token authentication. A successful response supplies a session token stored in
//!   this window's sessionStorage, sent as `?token=` for WebSocket and an Authorization header for HTTP.
//!   Cookies are intentionally unused because they are shared across windows by domain, ignore ports,
//!   and previously caused one SSH window to overwrite another's credentials. `/api/me` likewise accepts
//!   only this window's token.
//! - If credentials expire and a submitted or injected password is available, silently log in again and
//!   reconnect. Return to the login page only when the password is rejected, usually after a server restart.
//!
//! This gate must wrap App because App calls loadTree over WebSocket as soon as it mounts, while the
//! WebSocket upgrade requires authentication. Mount App only after obtaining a token.

import { useEffect, useRef, useState, type ReactNode } from "react";
import { useT } from "../i18n";
import { recordRequestError } from "../ipc/reqLog";
import { isTauri } from "../ipc/transport";
import { wsClient } from "../ipc/wsClient";

type Phase =
  | "checking"
  | "need-login"
  | "need-pairing"
  | "auth-failed"
  | "ready";

/** Extract a session token from a login response and give it to wsClient. It is this window's sole
 * credential, persisted in sessionStorage across refreshes and sent in WebSocket URLs and HTTP headers.
 * Response-body tokens avoid unreadable HttpOnly cookies, cross-window sharing, and WKWebView cookie
 * timing races. A 200 response without a token is a login failure because no cookie fallback remains. */
async function captureSessionToken(r: Response): Promise<boolean> {
  try {
    const body = (await r.json()) as { token?: string };
    if (body && typeof body.token === "string" && body.token) {
      wsClient.setSessionToken(body.token);
      return true;
    }
  } catch {
    /* Fall through to common failure handling. */
  }
  // A successful response without a token indicates an outdated or faulty server. Since tokens are the
  // only credential, log the diagnostic and keep the caller on the login page.
  recordRequestError("login", "server did not return a session token (server too old?)");
  return false;
}

export function LoginGate({ children }: { children: ReactNode }) {
  const t = useT();
  // Desktop clients are immediately ready. E2EE pairing links carry a token and public key rather than
  // a session token, so proceed to the password page for second-factor handshake verification.
  const [phase, setPhase] = useState<Phase>(
    isTauri ? "ready" : wsClient.isPairingMode() ? "need-login" : "checking",
  );
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [submitting, setSubmitting] = useState(false);
  // Refs expose current relogin state to the mount-time onAuthLost callback: passwordRef tracks the
  // injected or submitted password, reloginBusy prevents concurrent attempts, and reloginRejected
  // suppresses retries after explicit rejection until a manual login succeeds.
  const passwordRef = useRef("");
  const reloginBusy = useRef(false);
  const reloginRejected = useRef(false);
  useEffect(() => {
    passwordRef.current = password;
  }, [password]);

  // Handle lost credentials or E2EE authentication failure in three ways. E2EE `e2ee_error` cannot
  // distinguish a bad password, expired pairing token, or disabled device, so show a terminal message
  // requesting a new pairing link rather than prompting endlessly. Token sessions with a password
  // silently relogin and reconnect, unless the password is rejected. Without a password, return to login.
  useEffect(() => {
    if (isTauri) return;
    return wsClient.onAuthLost((reason) => {
      if (wsClient.isPairingMode()) {
        // Server-side login throttling is temporary, not a credential failure: return to the
        // password form with the rate-limit message (the same key the HTTP 429 path uses below)
        // instead of the terminal "link expired" guidance.
        if (reason === "rate_limited") {
          setError(t("login.rateLimited"));
          setPhase("need-login");
          return;
        }
        setPhase("auth-failed");
        return;
      }
      const pw = passwordRef.current;
      if (pw && !reloginBusy.current && !reloginRejected.current) {
        reloginBusy.current = true;
        void (async () => {
          try {
            const r = await fetch("/api/login", {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({ password: pw }),
            });
            if (r.ok && (await captureSessionToken(r))) {
              wsClient.reconnectNow();
              setPhase("ready");
              return;
            }
            if (r.status === 429) {
              // Rate limiting is temporary, not a credential failure: show the rate-limit message
              // and do NOT latch reloginRejected, so a later attempt can succeed after it expires.
              setError(t("login.rateLimited"));
            } else {
              // Explicit rejection usually means the server restarted with another password; stop retrying.
              reloginRejected.current = true;
            }
          } catch {
            /* Let reconnect backoff handle network failure; recovery will invoke onAuthLost again. */
          } finally {
            reloginBusy.current = false;
          }
          setPhase((prev) => (prev === "need-pairing" ? prev : "need-login"));
        })();
        return;
      }
      // need-pairing is terminal for a pairing-required server without pairing data. Preserve it instead
      // of flashing its guidance and falling through to a password page after a stray WebSocket rejection.
      setPhase((prev) => (prev === "need-pairing" ? prev : "need-login"));
    });
  }, []);

  useEffect(() => {
    if (isTauri) return;
    // E2EE pairing has no session token to probe; proceed directly to the second-factor password page.
    if (wsClient.isPairingMode()) return;

    // For a bare address, first ask whether this server requires pairing. LanTls browser access shows
    // pairing-link guidance without a password form; plaintext loopback/LAN modes use password login.
    const proceedPlaintext = () => {
      // Session restoration recognizes only this window's sessionStorage token. Enter immediately when
      // the server accepts it; otherwise log in, possibly through the next effect's injected password.
      // Never admit a window through shared cookies that another window can overwrite.
      const token = wsClient.getSessionToken();
      if (!token) {
        setPhase("need-login");
        return;
      }
      fetch("/api/me", { headers: wsClient.authHeaders() })
        .then((r) => setPhase(r.ok ? "ready" : "need-login"))
        .catch(() => setPhase("need-login"));
    };

    fetch("/api/mode")
      .then((r) => (r.ok ? r.json() : { requirePairing: false }))
      .then((cfg: { requirePairing?: boolean }) => {
        if (cfg.requirePairing) setPhase("need-pairing");
        else proceedPlaintext();
      })
      .catch(() => proceedPlaintext());
  }, []);

  // `__VLX_AUTOLOGIN__` avoids manual entry. SSH windows receive a one-time random password over the
  // tunnel and loopback token flow; URL remote windows receive the connection-panel password for the
  // second E2EE handshake factor.
  useEffect(() => {
    if (isTauri) return;
    const auto = (window as { __VLX_AUTOLOGIN__?: { password?: string } })
      .__VLX_AUTOLOGIN__;
    const pw = auto && typeof auto.password === "string" ? auto.password : "";
    if (!pw) return;
    setPassword(pw);
    // For URL-based E2EE pairing, pass the password to wsClient as the second factor and mount App so
    // connection triggers handshake verification. onAuthLost reports failure through the same path as
    // manual submission. There is no session token or `/api/login` request.
    if (wsClient.isPairingMode()) {
      wsClient.setPairingPassword(pw);
      setPhase("ready");
      return;
    }
    void (async () => {
      try {
        const r = await fetch("/api/login", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ password: pw }),
        });
        if (r.ok && (await captureSessionToken(r))) {
          reloginRejected.current = false;
          setPhase("ready");
        }
      } catch {
        /* Stay on the password page for manual recovery when automatic login fails. */
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (phase === "ready") return <>{children}</>;

  if (phase === "checking") {
    return (
      <div style={screenStyle}>
        <div style={{ color: "var(--text-dim)", fontSize: 13 }}>{t("login.connecting")}</div>
      </div>
    );
  }

  // A pairing-required LanTls server without link data shows pairing guidance, not a password form.
  if (phase === "need-pairing") {
    return (
      <div style={screenStyle}>
        <div style={cardStyle}>
          <div
            style={{
              fontWeight: 700,
              fontSize: 15,
              color: "var(--text)",
              marginBottom: 8,
            }}
          >
            VelaTerm
          </div>
          <div
            style={{ fontSize: 13, color: "var(--text-dim)", lineHeight: 1.6 }}
          >
            {t("login.pairingRequired")}
          </div>
        </div>
      </div>
    );
  }

  // Treat E2EE handshake failure as terminal guidance; the password may be correct but the link expired.
  if (phase === "auth-failed") {
    return (
      <div style={screenStyle}>
        <div style={cardStyle}>
          <div
            style={{
              fontWeight: 700,
              fontSize: 15,
              color: "var(--text)",
              marginBottom: 8,
            }}
          >
            VelaTerm
          </div>
          <div
            style={{ fontSize: 13, color: "var(--text-dim)", lineHeight: 1.6 }}
          >
            {t("login.authFailed")}
          </div>
        </div>
      </div>
    );
  }

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!password) return;
    setSubmitting(true);
    setError("");
    // In E2EE pairing, give wsClient the password as a second factor. Mounting App initiates handshake;
    // onAuthLost returns here on failure. No `/api/login` or cookie is involved.
    if (wsClient.isPairingMode()) {
      wsClient.setPairingPassword(password);
      setSubmitting(false);
      setPhase("ready");
      return;
    }
    try {
      const r = await fetch("/api/login", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ password }),
      });
      if (r.ok) {
        if (await captureSessionToken(r)) {
          reloginRejected.current = false;
          setPhase("ready");
        } else setError(t("login.failed"));
      } else if (r.status === 429) {
        // Rate-limited by the backend after repeated failures; "wrong password" would mislead here.
        setError(t("login.rateLimited"));
      } else setError(t("login.wrongPassword"));
    } catch {
      setError(t("login.failed"));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div style={screenStyle}>
      <form onSubmit={submit} style={cardStyle}>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 10,
            marginBottom: 6,
          }}
        >
          <span
            style={{
              width: 30,
              height: 30,
              borderRadius: 8,
              display: "grid",
              placeItems: "center",
              background: "var(--accent)",
              color: "var(--bg-0)",
              fontWeight: 800,
            }}
          >
            V
          </span>
          <div>
            <div style={{ fontWeight: 700, fontSize: 15, color: "var(--text)" }}>
              VelaTerm
            </div>
            <div style={{ fontSize: 11, color: "var(--text-dim)" }}>
              {t("login.remoteAccess")}
            </div>
          </div>
        </div>

        <div style={{ fontSize: 12, color: "var(--text-dim)", marginBottom: 14 }}>
          {t("login.desc")}
        </div>

        <input
          type="password"
          value={password}
          placeholder={t("login.passwordPlaceholder")}
          autoFocus
          onChange={(e) => setPassword(e.target.value)}
          style={{
            width: "100%",
            boxSizing: "border-box",
            padding: "10px 12px",
            marginBottom: 12,
            border: "1px solid var(--border)",
            borderRadius: 8,
            background: "var(--bg-0)",
            color: "var(--text)",
            fontSize: 14,
            outline: "none",
          }}
        />

        <button
          type="submit"
          disabled={submitting || !password}
          style={{
            width: "100%",
            padding: "10px 0",
            border: "none",
            borderRadius: 8,
            background: "var(--accent)",
            color: "var(--bg-0)",
            fontSize: 13.5,
            fontWeight: 600,
            cursor: submitting ? "default" : "pointer",
            opacity: submitting || !password ? 0.6 : 1,
          }}
        >
          {submitting ? t("login.connecting") : t("login.connect")}
        </button>

        {error && (
          <div
            style={{
              marginTop: 12,
              fontSize: 12,
              color: "var(--danger, #ff6b6b)",
              textAlign: "center",
            }}
          >
            {error}
          </div>
        )}
      </form>
    </div>
  );
}

const screenStyle: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  display: "grid",
  placeItems: "center",
  background: "var(--bg-app, var(--bg-0))",
};

const cardStyle: React.CSSProperties = {
  // Avoid overflow below a 352px mobile viewport or under zoom; retain a fixed 320px desktop width.
  width: "min(320px, calc(100vw - 32px))",
  padding: 24,
  background: "var(--bg-2)",
  border: "1px solid var(--border-strong)",
  borderRadius: 14,
  boxShadow: "var(--shadow)",
};
