//! Regression coverage for the stable backend gating codes (FIX: raw English backend strings
//! reached the UI): every remote_*_forbidden code maps to its i18n key with the detail forwarded
//! as parameter, unknown errors pass through unchanged, and the E2EE handshake failure
//! classification distinguishes the server's rate_limited code from credential failures.

import { describe, expect, it, vi } from "vitest";

vi.mock("../i18n", () => ({
  // Echo key and parameters so assertions are locale-independent.
  t: (key: string, ...args: unknown[]) => [key, ...args].join("|"),
}));

import { handshakeFailureReason, isAgentNotInstalledError, mapBackendError } from "./backendError";

describe("mapBackendError", () => {
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

  it("passes unknown backend errors through unchanged", () => {
    expect(mapBackendError("Session has been deleted")).toBe("Session has been deleted");
    expect(mapBackendError("Failed to open file: no such file")).toBe(
      "Failed to open file: no such file",
    );
    expect(mapBackendError("")).toBe("");
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
