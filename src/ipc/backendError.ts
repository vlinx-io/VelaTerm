//! Mapping of stable backend error codes to localized UI messages.
//!
//! The Rust backend gates remote WebSocket clients (management plane, secret commands, protected
//! settings keys, data-dir file access) and reports those rejections as machine-readable
//! `code:detail` strings instead of raw English prose, so the UI can localize them. wsClient maps
//! every invoke rejection through here; unknown errors pass through unchanged.

import { t } from "../i18n";

/** Product names for the agents whose sign-in requirement the backend reports by kind. */
const SIGN_IN_AGENT_LABEL: Record<string, string> = { antigravity: "Antigravity", copilot: "Copilot", cursor: "Cursor", kimi: "Kimi Code", grok: "Grok Build" };

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

/** Strip ANSI/VT escape sequences so raw CLI tracing logs never leak control codes into UI errors. */
export function stripAnsi(text: string): string {
  if (!text.includes("\x1b")) return text;
  return text
    // eslint-disable-next-line no-control-regex
    .replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, "")
    // eslint-disable-next-line no-control-regex
    .replace(/\x1b\[[0-9;?]*[ -/]*[@-~]/g, "")
    // eslint-disable-next-line no-control-regex
    .replace(/\x1b[@-Z\\-_]/g, "");
}

/** Whether a Codex handshake or stderr message reports an active writer lock conflict on the thread. */
function isCodexActiveWriterError(text: string): boolean {
  return /\bthread-store conflict\b|\balready has an active writer\b/i.test(text);
}

/**
 * Map a backend invoke error to a localized message. Stable codes carry their detail after the
 * first colon (`remote_cmd_forbidden:web_server_start`); anything unrecognized is returned as-is
 * so genuine backend prose and legacy errors keep surfacing unchanged.
 */
export function mapBackendError(raw: string): string {
  const clean = stripAnsi(raw);
  const idx = clean.indexOf(":");
  const code = idx > 0 ? clean.slice(0, idx) : clean;
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
  // DeepSeek's API panel validates the endpoint and key, with no detail worth echoing.
  if (clean === "deepseek_invalid_url" || clean === "deepseek_invalid_key") return t("settings.deepseekInvalid");
  // Codex refuses to resume a thread whose writer lock is still held by another process. Match both
  // the stable backend code and raw CLI/stderr output so older remote servers render cleanly too.
  if (clean === "codex_thread_active_writer" || code === "codex_thread_active_writer" || isCodexActiveWriterError(clean)) {
    return t("chat.codex.activeWriter");
  }
  if (idx <= 0) return clean;
  const detail = clean.slice(idx + 1);
  switch (code) {
    case "remote_cmd_forbidden":
      return t("transport.remoteCmdForbidden", detail);
    case "remote_setting_forbidden":
      return t("transport.remoteSettingForbidden", detail);
    case "remote_path_forbidden":
      return t("transport.remotePathForbidden", detail);
    // An ACP agent refused to open the conversation until its user signs in.
    case "agent_sign_in_required":
      return t("chat.signInRequired", SIGN_IN_AGENT_LABEL[detail] ?? detail);
    // The agent has no API key to call its model with; DeepSeek through OpenCode is the one that reports this.
    case "agent_api_key_required":
      return t("chat.apiKeyRequired");
    // Signed in to ZCode, but the account holds no usable GLM Coding Plan on that platform
    // (`zcode_plan_unavailable:<zai|bigmodel>[:<provider message>]`). The provider's own words follow.
    case "zcode_plan_unavailable": {
      const sep = detail.indexOf(":");
      const platform = sep < 0 ? detail : detail.slice(0, sep);
      const said = sep < 0 ? "" : detail.slice(sep + 1).trim();
      if (platform !== "zai" && platform !== "bigmodel") return clean;
      const message = t(platform === "zai" ? "chat.auth.zcode.noPlan.zai" : "chat.auth.zcode.noPlan.bigmodel");
      return said ? `${message} (${said})` : message;
    }
    // Signed in to ZCode, but ZCode lists no model: no plan of that platform was found in the sign-in
    // (`zcode_no_models:<zai|bigmodel>`). Unlike the case above, the platform has not refused anything.
    case "zcode_no_models":
      if (detail !== "zai" && detail !== "bigmodel") return clean;
      return t(detail === "zai" ? "chat.auth.zcode.noModels.zai" : "chat.auth.zcode.noModels.bigmodel");
    case "remote_bind_invalid":
      return t("remote.bindInvalid", detail);
    case "remote_bind_unavailable":
      return t("remote.bindUnavailable", detail);
    case "remote_pairing_host_invalid":
      return t("remote.pairingHostInvalid", detail);
    case "ports_missing_port":
      return t("ports.missingPort");
    case "ports_invalid_port":
      return t("ports.invalidPort", detail);
    case "ports_unknown_action":
      return t("ports.unknownAction", detail);
    case "ports_limit":
      return t("ports.limit", detail);
    case "ports_not_forwarded":
      return t("ports.notForwarded", detail);
    default:
      return clean;
  }
}

/**
 * Format an outbox submission failure for inline display. Strips ANSI control codes and drops the
 * JS `Error: ` wrapper whenever the underlying message matches a localized backend error.
 */
export function formatSubmissionError(raw: string): string {
  const clean = stripAnsi(raw);
  const unwrapped = clean.startsWith("Error: ") ? clean.slice("Error: ".length) : clean;
  const mapped = mapBackendError(unwrapped);
  return mapped !== unwrapped ? mapped : clean;
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
