//! Regression coverage for the stable backend gating codes (FIX: raw English backend strings
//! reached the UI): every remote_*_forbidden code maps to its i18n key with the detail forwarded
//! as parameter, unknown errors pass through unchanged, and the E2EE handshake failure
//! classification distinguishes the server's rate_limited code from credential failures.

import { describe, expect, it, vi } from "vitest";

vi.mock("../i18n", () => ({
  // Echo key and parameters so assertions are locale-independent.
  t: (key: string, ...args: unknown[]) => [key, ...args].join("|"),
}));

import { handshakeFailureReason, mapBackendError } from "./backendError";

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
