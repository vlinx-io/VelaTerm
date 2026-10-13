import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";

const { env } = vi.hoisted(() => ({
  env: { isBrowser: false, isTauri: false, isElectron: false },
}));

vi.mock("../../platform", () => ({
  env,
  platform: { opener: { openExternal: vi.fn().mockResolvedValue(undefined) } },
}));
vi.mock("../../ipc/transport", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../ipc/transport")>(),
  invoke: vi.fn().mockResolvedValue({ linked: false }),
  isTauri: false,
}));
vi.mock("../../ipc/webServer", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../ipc/webServer")>(),
  webServerStatus: vi.fn().mockResolvedValue({ running: false, port: 0 }),
}));

import { StatusBar } from "./StatusBar";

afterEach(() => {
  cleanup();
  env.isBrowser = false;
});

it("renders Feedback and Account after Notify on the left side", () => {
  const { container } = render(<StatusBar />);
  const statusbar = container.querySelector(".statusbar") as HTMLElement;
  const feedback = screen.getByTitle("Feedback");
  const account = screen.getByTitle(/Account/);
  const notify = statusbar.querySelector('[title^="System notifications"]') as HTMLElement;

  expect(notify).toBeTruthy();
  expect(notify.compareDocumentPosition(feedback) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(feedback.compareDocumentPosition(account) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(account.compareDocumentPosition(statusbar.querySelector(".sp") as HTMLElement) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
});

it("hides Account in browser clients", () => {
  env.isBrowser = true;
  render(<StatusBar />);

  expect(screen.queryByTitle(/Account/)).toBeNull();
  expect(screen.getByTitle("Feedback")).toBeTruthy();
});
