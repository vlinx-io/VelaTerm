import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
vi.mock("../../../ipc/transport", () => ({ invoke: vi.fn(), listen: vi.fn() }));
import { invoke, listen } from "../../../ipc/transport";
import { setLang } from "../../../i18n";
import { ModelCatalogStatus } from "./ModelCatalogStatus";

const cached = { source: "cache", revision: 2, checkedAt: null, error: "downloadFailed", refreshing: false };
let receive: (value: unknown) => void;
const stop = vi.fn();
beforeEach(() => {
  vi.resetAllMocks(); setLang("en");
  vi.mocked(invoke).mockResolvedValue(cached);
  vi.mocked(listen).mockImplementation((_name, callback) => { receive = callback; return Promise.resolve(stop); });
});
afterEach(cleanup);

it("shows restored cache and switches to the new website revision on a backend event", async () => {
  const changed = vi.fn();
  const view = render(<ModelCatalogStatus onChanged={changed} />);
  await screen.findByText("Cached model catalog");
  expect(screen.getByText("Update failed. The previous catalog is still available.")).toBeTruthy();
  await act(async () => receive({ ...cached, source: "website", revision: 3, error: null }));
  expect(screen.getByText("Website model catalog")).toBeTruthy();
  expect(screen.getByText("· v3")).toBeTruthy();
  expect(screen.queryByText("Update failed. The previous catalog is still available.")).toBeNull();
  expect(changed).toHaveBeenCalledTimes(2);
  view.unmount();
  await waitFor(() => expect(stop).toHaveBeenCalledOnce());
});

it("names the installed CLI with its version when the list comes from a probe or a conversation", async () => {
  const changed = vi.fn();
  vi.mocked(invoke).mockResolvedValue({ source: "cli", revision: null, cliVersion: "2.1.280", checkedAt: 1, error: null, refreshing: false });
  render(<ModelCatalogStatus onChanged={changed} />);
  await screen.findByText("Installed Claude CLI");
  expect(screen.getByText("· 2.1.280")).toBeTruthy();
  expect(screen.queryByText(/· v/)).toBeNull();
  expect(screen.getByText(/Last checked:/)).toBeTruthy();
  expect(screen.queryByText("Update failed. The previous catalog is still available.")).toBeNull();
  // A failed re-probe keeps the CLI list and shows the existing failure text.
  await act(async () => receive({ source: "cli", revision: null, cliVersion: "2.1.280", checkedAt: 1, error: "probeFailed:timeout", refreshing: false }));
  expect(screen.getByText("Installed Claude CLI")).toBeTruthy();
  expect(screen.getByText("Update failed. The previous catalog is still available.")).toBeTruthy();
  // A newer CLI version is a change worth re-reading the catalogue for.
  await act(async () => receive({ source: "cli", revision: null, cliVersion: "2.1.300", checkedAt: 1, error: null, refreshing: false }));
  expect(screen.getByText("· 2.1.300")).toBeTruthy();
  expect(changed).toHaveBeenCalledTimes(2);
});

it("keeps the cached revision visible when manual refresh cannot reach the backend", async () => {
  render(<ModelCatalogStatus onChanged={vi.fn()} />);
  await screen.findByText("Cached model catalog");
  vi.mocked(invoke).mockRejectedValueOnce(new Error("disconnected"));
  fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
  await waitFor(() => expect((screen.getByRole("button", { name: "Refresh" }) as HTMLButtonElement).disabled).toBe(false));
  expect(invoke).toHaveBeenCalledWith("model_catalog_refresh");
  expect(screen.getByText("· v2")).toBeTruthy();
  expect(screen.getByText("Update failed. The previous catalog is still available.")).toBeTruthy();
});
