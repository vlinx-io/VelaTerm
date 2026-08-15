//! Mapping of stable backend error codes to localized UI messages.
//!
//! The Rust backend gates remote WebSocket clients (management plane, secret commands, protected
//! settings keys, data-dir file access) and reports those rejections as machine-readable
//! `code:detail` strings instead of raw English prose, so the UI can localize them. wsClient maps
//! every invoke rejection through here; unknown errors pass through unchanged.

import { t } from "../i18n";

/** Reason the E2EE WebSocket handshake failed, derived from the server's `e2ee_error` code. */
export type HandshakeFailure = "rate_limited" | "unauthorized";

/**
 * Classify a decrypted handshake frame that is not `e2ee_authenticated`. The server distinguishes
 * throttling (`rate_limited`, sent before any credential work) from bad credentials
 * (`unauthorized`); everything else — unknown codes included — is treated as an auth failure.
 */
export function handshakeFailureReason(msg: { type?: string; code?: string }): HandshakeFailure {
  return msg.type === "e2ee_error" && msg.code === "rate_limited" ? "rate_limited" : "unauthorized";
}

/**
 * Map a backend invoke error to a localized message. Stable codes carry their detail after the
 * first colon (`remote_cmd_forbidden:web_server_start`); anything unrecognized is returned as-is
 * so genuine backend prose and legacy errors keep surfacing unchanged.
 */
export function mapBackendError(raw: string): string {
  const idx = raw.indexOf(":");
  if (idx <= 0) return raw;
  const detail = raw.slice(idx + 1);
  switch (raw.slice(0, idx)) {
    case "remote_cmd_forbidden":
      return t("transport.remoteCmdForbidden", detail);
    case "remote_setting_forbidden":
      return t("transport.remoteSettingForbidden", detail);
    case "remote_path_forbidden":
      return t("transport.remotePathForbidden", detail);
    default:
      return raw;
  }
}
