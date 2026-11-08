//! Mapping of stable backend error codes to localized UI messages.
//!
//! The Rust backend gates remote WebSocket clients (management plane, secret commands, protected
//! settings keys, data-dir file access) and reports those rejections as machine-readable
//! `code:detail` strings instead of raw English prose, so the UI can localize them. wsClient maps
//! every invoke rejection through here; unknown errors pass through unchanged.

import { t } from "../i18n";

/** Product names for the agents whose sign-in requirement the backend reports by kind. */
const SIGN_IN_AGENT_LABEL: Record<string, string> = { copilot: "Copilot", cursor: "Cursor", kimi: "Kimi Code", grok: "Grok Build" };

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
  const code = idx > 0 ? raw.slice(0, idx) : raw;
  const historyKey = {
    source_changed: "chat.history.contentChanged",
    cursor_expired: "chat.history.contentChanged",
    resync_required: "chat.history.contentChanged",
    source_missing: "chat.history.sourceMissing",
    index_incomplete: "chat.history.incomplete",
    index_busy: "chat.history.indexBusy",
    unsupported_record: "chat.history.unsupported",
    detail_unavailable: "chat.history.unavailable",
    storage_pressure: "chat.history.storagePressure",
    read_canceled: "chat.history.canceled",
    detail_requires_paging: "chat.history.pagedRequired",
    detail_requires_download: "chat.history.clipboardLimit",
    image_requires_download: "chat.history.imageDownloadRequired",
    history_protocol_unsupported: "chat.history.updateServer",
  } as const;
  if (Object.hasOwn(historyKey,code)) return `${code}: ${t(historyKey[code as keyof typeof historyKey])}`;
  if (idx <= 0) return raw;
  const detail = raw.slice(idx + 1);
  switch (raw.slice(0, idx)) {
    case "remote_cmd_forbidden":
      return t("transport.remoteCmdForbidden", detail);
    case "remote_setting_forbidden":
      return t("transport.remoteSettingForbidden", detail);
    case "remote_path_forbidden":
      return t("transport.remotePathForbidden", detail);
    // An ACP agent refused to open the conversation until its user signs in.
    case "agent_sign_in_required":
      return t("chat.signInRequired", SIGN_IN_AGENT_LABEL[detail] ?? detail);
    // The agent has no API key to call its model with; DeepSeek Harness is the one that reports this.
    case "agent_api_key_required":
      return t("chat.apiKeyRequired");
    // Signed in to ZCode, but the account holds no usable GLM Coding Plan on that platform
    // (`zcode_plan_unavailable:<zai|bigmodel>[:<provider message>]`). The provider's own words follow.
    case "zcode_plan_unavailable": {
      const sep = detail.indexOf(":");
      const platform = sep < 0 ? detail : detail.slice(0, sep);
      const said = sep < 0 ? "" : detail.slice(sep + 1).trim();
      if (platform !== "zai" && platform !== "bigmodel") return raw;
      const message = t(platform === "zai" ? "chat.auth.zcode.noPlan.zai" : "chat.auth.zcode.noPlan.bigmodel");
      return said ? `${message} (${said})` : message;
    }
    // Signed in to ZCode, but ZCode lists no model: no plan of that platform was found in the sign-in
    // (`zcode_no_models:<zai|bigmodel>`). Unlike the case above, the platform has not refused anything.
    case "zcode_no_models":
      if (detail !== "zai" && detail !== "bigmodel") return raw;
      return t(detail === "zai" ? "chat.auth.zcode.noModels.zai" : "chat.auth.zcode.noModels.bigmodel");
    default:
      return raw;
  }
}

/**
 * Stable code the chat backend returns instead of the raw OS error when the agent executable does
 * not exist (`agent_not_installed:<kind>`). The conversation view turns it into installation
 * guidance; the plain spawn message stays reserved for real launch failures.
 */
const AGENT_NOT_INSTALLED = "agent_not_installed";

/** Whether an invoke rejection or stored submission error is the missing-executable code. */
export function isAgentNotInstalledError(value: unknown): boolean {
  const raw =
    typeof value === "string" ? value : value instanceof Error ? value.message : "";
  // A rejection that was stringified carries the standard Error prefix (`String(new Error(code))`).
  const text = raw.startsWith("Error: ") ? raw.slice("Error: ".length) : raw;
  return text === AGENT_NOT_INSTALLED || text.startsWith(`${AGENT_NOT_INSTALLED}:`);
}
