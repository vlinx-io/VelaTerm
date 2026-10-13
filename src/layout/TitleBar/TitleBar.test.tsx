import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { useTermStore } from "../../store/termStore";

vi.mock("../../ipc/commands", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../ipc/commands")>(),
  getBackendVersion: vi.fn().mockResolvedValue("0.0.0"),
}));
vi.mock("../../ipc/transport", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../ipc/transport")>(),
  invoke: vi.fn().mockResolvedValue({}),
  isTauri: false,
}));
vi.mock("../../ipc/webServer", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../ipc/webServer")>(),
  webServerStatus: vi.fn().mockResolvedValue({ running: false, port: 0 }),
}));
vi.mock("./AppMenuBar", () => ({ AppMenuBar: () => null }));

import { TitleBar } from "./TitleBar";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  useTermStore.setState({
    theme: "system",
    themeSwitcherInTitleBar: false,
    leftCollapsed: false,
    rightCollapsed: false,
  });
});

it("hides theme, Feedback, Account, and open-panel controls by default", () => {
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  render(<TitleBar />);

  expect(document.querySelectorAll(".tb-seg[aria-pressed], .tb-seg button[aria-pressed]")).toHaveLength(0);
  expect(screen.queryByTitle("Feedback")).toBeNull();
  expect(screen.queryByTitle(/Account/)).toBeNull();
  expect(screen.queryByTitle("Hide sidebar")).toBeNull();
  expect(screen.queryByTitle("Hide info panel")).toBeNull();
});

it("shows the original three theme buttons when the setting is on", () => {
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  useTermStore.setState({ themeSwitcherInTitleBar: true });
  render(<TitleBar />);

  expect(document.querySelectorAll(".tb-seg button[aria-pressed]")).toHaveLength(3);
});

it("keeps panel controls out of the header even while a panel is collapsed", () => {
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  useTermStore.setState({ leftCollapsed: true, rightCollapsed: true });
  render(<TitleBar />);

  expect(screen.queryByTitle("Show sidebar")).toBeNull();
  expect(screen.queryByTitle("Show info panel")).toBeNull();
});
