import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { PortsState } from "../../remote/ports/portsClient";

const ports = vi.hoisted(() => ({
  state: { snapshot: null, unavailable: false, error: null } as PortsState,
}));
vi.mock("../../remote/ports/portsClient", () => ({
  portsSupported: true,
  usePorts: () => ports.state,
}));
vi.mock("../../ipc/transport", async (original) => ({
  ...(await original<typeof import("../../ipc/transport")>()),
  invoke: vi.fn(),
  listen: vi.fn().mockResolvedValue(() => {}),
  onTransportReconnect: () => () => {}, onTransportDisconnect: () => () => {},
}));

import { PortsSeg } from "./StatusBar";
import { setLang } from "../../i18n";
import { useTermStore } from "../../store/termStore";

beforeEach(() => {
  setLang("en");
  ports.state = { snapshot: null, unavailable: false, error: null };
  useTermStore.setState({ inspectorTab: "info", rightCollapsed: true });
});
afterEach(cleanup);

it("stays hidden without forwards", () => {
  const { container } = render(<PortsSeg />);
  expect(container.textContent).toBe("");
});

it("shows the forward count and opens the Ports tab", () => {
  ports.state = {
    snapshot: { session: "s1", detected: [], detectionAvailable: true, forwards: [
      { rport: 3000, lport: 3000, source: "detected" },
      { rport: 8080, lport: 8080, source: "manual" },
    ] },
    unavailable: false,
    error: null,
  };
  render(<PortsSeg />);
  fireEvent.click(screen.getByText("2 forwarded ports"));
  expect(useTermStore.getState().inspectorTab).toBe("ports");
  expect(useTermStore.getState().rightCollapsed).toBe(false);
});
