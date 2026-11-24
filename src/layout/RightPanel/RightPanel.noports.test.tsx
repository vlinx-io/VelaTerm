// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { setLang } from "../../i18n";
import { useTermStore } from "../../store/termStore";
import { RightPanel } from "./RightPanel";

vi.mock("./FilesTab", () => ({ FilesTab: () => <div>File explorer</div> }));
vi.mock("./InfoTab", () => ({ InfoTab: () => <div>Session info</div> }));
vi.mock("./git/GitTab", () => ({ GitTab: () => <div>Git changes</div> }));
vi.mock("../../remote/ports/PortsTab", () => ({ PortsTab: () => <div>Ports panel</div> }));
vi.mock("../../remote/ports/portsClient", () => ({ portsSupported: false }));

beforeEach(() => {
  setLang("en");
  window.history.replaceState(null, "", "/");
});
afterEach(cleanup);

it("falls back to a valid tab when a stored Ports selection has no panel outside SSH windows", () => {
  useTermStore.setState({ inspectorTab: "ports" });
  render(<RightPanel />);
  expect(screen.queryByText("Ports panel")).toBeNull();
  expect(screen.queryByRole("link", { name: "Ports" })).toBeNull();
  expect(screen.getByRole("link", { name: "Info" }).getAttribute("aria-current")).toBe("page");
});
