import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const { codexHooksListMock, codexHookUpdateMock } = vi.hoisted(() => ({
  codexHooksListMock: vi.fn(),
  codexHookUpdateMock: vi.fn(),
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: string, enabled?: number, total?: number) =>
    key === "settings.codexHooksEnabledCount" ? `Enabled ${enabled} / ${total}` : key,
}));

vi.mock("../../ipc/commands", () => ({
  codexHooksList: codexHooksListMock,
  codexHookUpdate: codexHookUpdateMock,
}));

import { CodexHooksPanel } from "./CodexHooksPanel";

const USER_HOOK = {
  key: "/Users/me/.codex/hooks.json:post_tool_use:0:0",
  eventName: "postToolUse",
  handlerType: "command",
  matcher: "Edit|Write",
  command: "python3 /Users/me/format.py",
  timeoutSec: 5,
  statusMessage: "Formatting",
  sourcePath: "/Users/me/.codex/hooks.json",
  source: "user",
  pluginId: null,
  displayOrder: 0,
  enabled: true,
  currentHash: "sha256:user",
  trustStatus: "untrusted",
  isManaged: false,
};

const MANAGED_HOOK = {
  ...USER_HOOK,
  key: "/Library/Application Support/Codex/hooks.json:session_start:0:0",
  eventName: "sessionStart",
  command: "/usr/local/bin/company-codex-start",
  sourcePath: "/Library/Application Support/Codex/hooks.json",
  source: "mdm",
  currentHash: "sha256:managed",
  trustStatus: "managed",
  isManaged: true,
};

const VELATERM_HOOK = {
  ...MANAGED_HOOK,
  key: "/<session-flags>/config.toml:session_start:0:0",
  command: '"$VLX_EXE" --codex-hook ready',
  sourcePath: "VelaTerm session hooks",
  source: "velaterm",
  trustStatus: "trusted",
};

function response(userHook = USER_HOOK) {
  return {
    data: [
      {
        cwd: "/work/one",
        hooks: [userHook, MANAGED_HOOK, VELATERM_HOOK],
        warnings: [],
        errors: [],
      },
      {
        cwd: "/work/two",
        hooks: [userHook],
        warnings: [],
        errors: [],
      },
    ],
  };
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Codex hook settings", () => {
  it("places refresh beside the wrapping description", async () => {
    codexHooksListMock.mockResolvedValue(response());

    render(<CodexHooksPanel cwds={[]} codexPath="" />);

    const description = screen.getByText("settings.codexHooksDesc");
    const refresh = screen.getByRole("button", { name: "common.refresh" });
    expect(description.parentElement).toBe(refresh.parentElement);
  });

  it("collapses groups by default and shows each enabled count", async () => {
    codexHooksListMock.mockResolvedValue(response({ ...USER_HOOK, enabled: false }));

    render(
      <CodexHooksPanel
        cwds={["/work/one", "/work/two"]}
        codexPath="/opt/bin/codex"
      />,
    );

    const userGroup = await screen.findByRole("button", {
      name: /settings\.codexHooksUser.*Enabled 0 \/ 1/,
    });
    expect(userGroup.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByText(USER_HOOK.command)).toBeNull();

    fireEvent.click(userGroup);

    expect(await screen.findByText(USER_HOOK.command)).toBeTruthy();
    expect(screen.getAllByText(USER_HOOK.command)).toHaveLength(1);
    expect(userGroup.getAttribute("aria-expanded")).toBe("true");
    expect(codexHooksListMock).toHaveBeenCalledWith({
      cwds: ["/work/one", "/work/two"],
      codexPath: "/opt/bin/codex",
    });
  });

  it("shows disabled state and keeps the toggle on the hook header", async () => {
    const trustedHook = { ...USER_HOOK, trustStatus: "trusted" };
    codexHooksListMock.mockResolvedValue(response(trustedHook));
    codexHookUpdateMock.mockResolvedValue(response({ ...trustedHook, enabled: false }));

    render(<CodexHooksPanel cwds={["/work/one"]} codexPath="" />);

    fireEvent.click(await screen.findByRole("button", { name: /settings\.codexHooksUser/ }));
    const toggle = await screen.findByRole("switch", { name: "Post Tool Use hook" });
    expect(toggle.getAttribute("aria-checked")).toBe("true");
    fireEvent.click(toggle);

    await waitFor(() =>
      expect(codexHookUpdateMock).toHaveBeenCalledWith({
        cwds: ["/work/one"],
        codexPath: undefined,
        key: USER_HOOK.key,
        enabled: false,
      }),
    );
    await waitFor(() => expect(toggle.getAttribute("aria-checked")).toBe("false"));
    expect(screen.queryByText("settings.codexHooksTrusted")).toBeNull();
    expect(screen.getByText("settings.codexHooksDisabled")).toBeTruthy();
    const hookTitle = screen.getByText("Post Tool Use");
    expect(hookTitle.parentElement).toBe(toggle.parentElement?.parentElement);
  });

  it("shows the complete hook definition", async () => {
    codexHooksListMock.mockResolvedValue(response());

    render(<CodexHooksPanel cwds={["/work/one"]} codexPath="" />);

    fireEvent.click(await screen.findByRole("button", { name: /settings\.codexHooksUser/ }));
    expect(screen.getByText("settings.codexHooksHandler")).toBeTruthy();
    expect(screen.getByText("settings.codexHooksCommand")).toBeTruthy();
    expect(screen.getByText("settings.codexHooksMatcher")).toBeTruthy();
    expect(screen.getByText("settings.codexHooksTimeout")).toBeTruthy();
    expect(screen.getByText("settings.codexHooksStatusMessage")).toBeTruthy();
    expect(screen.getByText("Formatting")).toBeTruthy();
  });

  it("trusts reviewed definitions and keeps managed hooks locked", async () => {
    codexHooksListMock.mockResolvedValue(response());
    codexHookUpdateMock.mockResolvedValue(
      response({ ...USER_HOOK, trustStatus: "trusted" }),
    );

    render(<CodexHooksPanel cwds={[]} codexPath="" />);

    fireEvent.click(await screen.findByRole("button", { name: /settings\.codexHooksUser/ }));
    fireEvent.click(await screen.findByRole("button", { name: "settings.codexHooksTrust" }));
    await waitFor(() =>
      expect(codexHookUpdateMock).toHaveBeenCalledWith({
        cwds: [],
        codexPath: undefined,
        key: USER_HOOK.key,
        trustedHash: USER_HOOK.currentHash,
      }),
    );

    fireEvent.click(screen.getByRole("button", { name: /settings\.codexHooksManagedSource/ }));
    fireEvent.click(screen.getByRole("button", { name: /VelaTerm/ }));
    const managedToggles = screen.getAllByRole("switch", { name: "Session Start hook" });
    expect(managedToggles.every((toggle) => (toggle as HTMLButtonElement).disabled)).toBe(true);

    const velatermCommand = screen.getByText(VELATERM_HOOK.command);
    expect(velatermCommand.closest('[aria-disabled="true"]')).toBeTruthy();
  });

  it("keeps the newest response when an earlier request resolves last", async () => {
    const slow = response({ ...USER_HOOK, command: "stale command" });
    const fast = response({ ...USER_HOOK, command: "fresh command" });
    let releaseSlow = () => {};
    codexHooksListMock
      .mockImplementationOnce(
        () => new Promise((resolve) => (releaseSlow = () => resolve(slow))),
      )
      .mockResolvedValueOnce(fast);

    const { rerender } = render(<CodexHooksPanel cwds={["/work/one"]} codexPath="" />);
    rerender(<CodexHooksPanel cwds={["/work/two"]} codexPath="" />);

    fireEvent.click(await screen.findByRole("button", { name: /settings\.codexHooksUser/ }));
    expect(await screen.findByText("fresh command")).toBeTruthy();

    releaseSlow();
    await waitFor(() => expect(codexHooksListMock).toHaveBeenCalledTimes(2));
    expect(screen.queryByText("stale command")).toBeNull();
    expect(screen.getByText("fresh command")).toBeTruthy();
  });
});
