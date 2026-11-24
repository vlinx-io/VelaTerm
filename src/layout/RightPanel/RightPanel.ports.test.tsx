// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { setLang } from "../../i18n";
import { useTermStore } from "../../store/termStore";
import { RightPanel } from "./RightPanel";

vi.mock("./FilesTab", () => ({ FilesTab: () => <div>File explorer</div> }));
vi.mock("./InfoTab", () => ({ InfoTab: () => <div>Session info</div> }));
vi.mock("./git/GitTab", () => ({ GitTab: () => <div>Git changes</div> }));
vi.mock("../../remote/ports/PortsTab", () => ({ PortsTab: () => <div>Ports panel</div> }));
vi.mock("../../remote/ports/portsClient", () => ({ portsSupported: true }));

const original = useTermStore.getState().setInspectorTab;
beforeEach(() => {
  setLang("en");
  window.history.replaceState(null, "", "/");
  useTermStore.setState({ inspectorTab: "info", setInspectorTab: (tab) => useTermStore.setState({ inspectorTab: tab }) });
});
afterEach(() => {
  cleanup();
  useTermStore.setState({ setInspectorTab: original });
});

it("shows a Ports tab in SSH remote windows and opens the panel", () => {
  render(<RightPanel />);
  fireEvent.click(screen.getByRole("link", { name: "Ports" }));
  expect(screen.getByText("Ports panel")).toBeTruthy();
  expect(new URLSearchParams(location.search).get("inspector")).toBe("ports");
});

it("restores the Ports tab from the stored selection", () => {
  useTermStore.setState({ inspectorTab: "ports" });
  render(<RightPanel />);
  expect(screen.getByText("Ports panel")).toBeTruthy();
});
