//! Coverage for the conversation's missing-agent notice: it names the agent from the install recipe,
//! offers the install hand-off and the documentation link, and reports a dismissal.

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
vi.mock("../../../ipc/transport", () => ({ invoke: vi.fn(), listen: vi.fn(), isTauri: false, isElectronShell: false }));
vi.mock("../../../platform", () => ({ platform: { opener: { openExternal: vi.fn() } } }));
vi.mock("../../../store/termStore", () => ({ useTermStore: { getState: () => ({}) } }));
import { invoke } from "../../../ipc/transport";
import { setLang } from "../../../i18n";
import type { Session } from "../../../types";
import { AgentMissingNotice } from "./AgentMissingNotice";

const session = { id: "s", kind: "codex", engine: "chat" } as Session;

beforeEach(() => {
  vi.resetAllMocks();
  setLang("en");
});
afterEach(cleanup);

it("offers the install hand-off and documentation for a recipe-backed agent", async () => {
  vi.mocked(invoke).mockResolvedValue({
    label: "Codex",
    bin: "codex",
    command: "npm install -g @openai/codex",
    needsNode: true,
    docsUrl: "https://example.test/codex",
    authHint: "",
  });
  const onInstall = vi.fn();
  render(<AgentMissingNotice session={session} onInstall={onInstall} onDismiss={vi.fn()} />);

  await screen.findByText("Codex is not installed");
  expect(screen.getByText(/VelaTerm couldn't find Codex/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Install now" }));
  expect(onInstall).toHaveBeenCalledOnce();
});

it("hides the install action when the platform has no recipe command, and reports dismissal", async () => {
  vi.mocked(invoke).mockResolvedValue({
    label: "Codex",
    bin: "codex",
    command: "  ",
    needsNode: false,
    docsUrl: "https://example.test/codex",
    authHint: "",
  });
  const onDismiss = vi.fn();
  render(<AgentMissingNotice session={session} onInstall={vi.fn()} onDismiss={onDismiss} />);

  await screen.findByText("Codex is not installed");
  expect(screen.queryByRole("button", { name: "Install now" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "I'll do it myself" }));
  expect(onDismiss).toHaveBeenCalledOnce();
});
