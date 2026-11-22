//! Regression coverage for the stable backend gating codes (FIX: raw English backend strings
//! reached the UI): every remote_*_forbidden code maps to its i18n key with the detail forwarded
//! as parameter, unknown errors pass through unchanged, and the E2EE handshake failure
//! classification distinguishes the server's rate_limited code from credential failures.

import { describe, expect, it, vi } from "vitest";

vi.mock("../i18n", () => ({
  // Echo key and parameters so assertions are locale-independent.
  t: (key: string, ...args: unknown[]) => [key, ...args].join("|"),
}));

import { formatSubmissionError, handshakeFailureReason, isAgentNotInstalledError, mapBackendError, stripAnsi } from "./backendError";

describe("mapBackendError", () => {
  it("localizes history errors while retaining the code used for revision recovery", () => {
    expect(mapBackendError("source_changed: Original path and system error")).toBe("source_changed: chat.history.contentChanged");
    expect(mapBackendError("cursor_expired: Session generation changed")).toBe("cursor_expired: chat.history.contentChanged");
    expect(mapBackendError("index_incomplete: Unconfirmed branch")).toBe("index_incomplete: chat.history.incomplete");
    expect(mapBackendError("read_canceled")).toBe("read_canceled: chat.history.canceled");
    expect(mapBackendError("storage_pressure: Private native location")).toBe("storage_pressure: chat.history.storagePressure");
    expect(mapBackendError("image_requires_download: Excessive dimensions")).toBe("image_requires_download: chat.history.imageDownloadRequired");
    expect(mapBackendError(mapBackendError("index_busy: Another reader"))).toBe("index_busy: chat.history.indexBusy");
  });
  it("names the agent an ACP sign-in requirement belongs to", () => {
    expect(mapBackendError("agent_sign_in_required:copilot")).toBe("chat.signInRequired|Copilot");
    expect(mapBackendError("agent_sign_in_required:grok")).toBe("chat.signInRequired|Grok Build");
    expect(mapBackendError("agent_sign_in_required:future")).toBe("chat.signInRequired|future");
  });

  it("explains how to give DeepSeek an API key", () => {
    expect(mapBackendError("agent_api_key_required:deepseek")).toBe("chat.apiKeyRequired");
  });

  it("names the platform a ZCode account without a usable plan signed in to, keeping the provider's words", () => {
    expect(mapBackendError("zcode_plan_unavailable:zai")).toBe("chat.auth.zcode.noPlan.zai");
    expect(mapBackendError("zcode_plan_unavailable:bigmodel:[1113][Insufficient balance][id]"))
      .toBe("chat.auth.zcode.noPlan.bigmodel ([1113][Insufficient balance][id])");
    expect(mapBackendError("zcode_plan_unavailable:other")).toBe("zcode_plan_unavailable:other");
  });

  it("asks a ZCode account ZCode lists no model for to sign in again, without claiming the quota is spent", () => {
    expect(mapBackendError("zcode_no_models:bigmodel")).toBe("chat.auth.zcode.noModels.bigmodel");
    expect(mapBackendError("zcode_no_models:zai")).toBe("chat.auth.zcode.noModels.zai");
    expect(mapBackendError("zcode_no_models:other")).toBe("zcode_no_models:other");
  });

  it("maps the command gate code to its i18n key with the command as detail", () => {
    expect(mapBackendError("remote_cmd_forbidden:web_server_start")).toBe(
      "transport.remoteCmdForbidden|web_server_start",
    );
  });

  it("maps the settings ACL code to its i18n key with the key as detail", () => {
    expect(mapBackendError("remote_setting_forbidden:gitea.token")).toBe(
      "transport.remoteSettingForbidden|gitea.token",
    );
  });

  it("maps the data-dir path ACL code and keeps colons inside the detail intact", () => {
    // Windows-style paths contain a colon of their own; only the first colon splits code/detail.
    expect(mapBackendError("remote_path_forbidden:C:\\data\\vlx-web-access.json")).toBe(
      "transport.remotePathForbidden|C:\\data\\vlx-web-access.json",
    );
  });

  it("maps the listen-address codes to their i18n keys with the value as detail", () => {
    expect(mapBackendError("remote_bind_invalid:host.example")).toBe(
      "remote.bindInvalid|host.example",
    );
    expect(mapBackendError("remote_bind_unavailable:192.0.2.10")).toBe(
      "remote.bindUnavailable|192.0.2.10",
    );
    expect(mapBackendError("remote_pairing_host_invalid:x:8799")).toBe(
      "remote.pairingHostInvalid|x:8799",
    );
  });

  it("maps Codex active-writer lock conflicts from both stable codes and raw ANSI stderr", () => {
    expect(mapBackendError("codex_thread_active_writer:01a1d769-830a-75a1-a110-67e01b00387e")).toBe("chat.codex.activeWriter");
    expect(mapBackendError("codex_thread_active_writer")).toBe("chat.codex.activeWriter");
    const ansiStderr =
      "\x1b[2m2026-11-22T06:13:11.211255Z\x1b[0m \x1b[31mERROR\x1b[0m \x1b[2mcodex_core::session::session\x1b[0m\x1b[2m:\x1b[0m failed to initialize thread persistence: thread-store conflict: thread 01a1d769-830a-75a1-a110-67e01b00387e already has an active writer";
    expect(mapBackendError(ansiStderr)).toBe("chat.codex.activeWriter");
  });

  it("strips ANSI escape codes from unknown errors without rewriting plain prose", () => {
    expect(stripAnsi("\x1b[2m2026-11-22\x1b[0m \x1b[31mERROR\x1b[0m boom")).toBe("2026-11-22 ERROR boom");
    expect(mapBackendError("\x1b[31mUnexpected CLI failure\x1b[0m")).toBe("Unexpected CLI failure");
    expect(mapBackendError("Session has been deleted")).toBe("Session has been deleted");
    expect(mapBackendError("Failed to open file: no such file")).toBe(
      "Failed to open file: no such file",
    );
    expect(mapBackendError("")).toBe("");
  });
});

describe("formatSubmissionError", () => {
  it("drops the Error prefix for mapped codes and strips ANSI from unrecognized failures", () => {
    expect(formatSubmissionError("Error: codex_thread_active_writer:01a1d769")).toBe("chat.codex.activeWriter");
    expect(formatSubmissionError("Error: \x1b[31mthread-store conflict: thread 01a1 already has an active writer\x1b[0m"))
      .toBe("chat.codex.activeWriter");
    expect(formatSubmissionError("Error: \x1b[31mSomething broke\x1b[0m")).toBe("Error: Something broke");
    expect(formatSubmissionError("Error: Submission failed")).toBe("Error: Submission failed");
  });
});

describe("handshakeFailureReason", () => {
  it("recognizes the server's rate_limited handshake error", () => {
    expect(handshakeFailureReason({ type: "e2ee_error", code: "rate_limited" })).toBe(
      "rate_limited",
    );
  });

  it("treats unauthorized, unknown codes, and malformed frames as auth failures", () => {
    expect(handshakeFailureReason({ type: "e2ee_error", code: "unauthorized" })).toBe(
      "unauthorized",
    );
    expect(handshakeFailureReason({ type: "e2ee_error", code: "something_new" })).toBe(
      "unauthorized",
    );
    expect(handshakeFailureReason({})).toBe("unauthorized");
    // A rate_limited code without the e2ee_error type must not unlock the retry path.
    expect(handshakeFailureReason({ type: "other", code: "rate_limited" })).toBe("unauthorized");
  });
});

describe("isAgentNotInstalledError", () => {
  it("recognizes the missing-executable code with and without its agent detail", () => {
    expect(isAgentNotInstalledError("agent_not_installed")).toBe(true);
    expect(isAgentNotInstalledError("agent_not_installed:codex")).toBe(true);
    expect(isAgentNotInstalledError(new Error("agent_not_installed:codex"))).toBe(true);
    // The outbox stores `String(error)`, which adds the standard Error prefix.
    expect(isAgentNotInstalledError("Error: agent_not_installed:codex")).toBe(true);
  });

  it("leaves other failures alone", () => {
    expect(isAgentNotInstalledError(undefined)).toBe(false);
    expect(isAgentNotInstalledError("")).toBe(false);
    expect(isAgentNotInstalledError("Failed to start the agent \"codex\": program not found")).toBe(false);
    expect(isAgentNotInstalledError("agent_not_installed_elsewhere")).toBe(false);
  });
});
