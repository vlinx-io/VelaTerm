import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../../ipc/transport")>(),
  invoke: vi.fn(),
}));

import { invoke } from "../../../ipc/transport";
import { setLang } from "../../../i18n";
import { DeepSeekApiChip } from "./DeepSeekApiChip";

beforeEach(() => {
  setLang("en");
  vi.mocked(invoke).mockReset();
});
afterEach(cleanup);

function status(overrides: { baseUrl?: string | null; hasKey?: boolean; editable?: boolean } = {}) {
  return { baseUrl: null, hasKey: false, editable: true, ...overrides };
}

it("opens the API form beside the composer and saves the address and key", async () => {
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === "deepseek_get_status") return Promise.resolve(status()) as Promise<never>;
    if (command === "deepseek_set_config") {
      return Promise.resolve(status({ baseUrl: "https://proxy.example/v1", hasKey: true })) as Promise<never>;
    }
    return Promise.resolve(undefined) as Promise<never>;
  });

  render(<DeepSeekApiChip />);
  fireEvent.click(screen.getByRole("button", { name: "API" }));
  expect(await screen.findByLabelText("API address")).toBeTruthy();
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === "deepseek_get_status")).toBe(true);

  fireEvent.change(screen.getByLabelText("API address"), {
    target: { value: "https://proxy.example/v1" },
  });
  fireEvent.change(screen.getByLabelText("API key"), { target: { value: "sk-test" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));

  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith("deepseek_set_config", {
      baseUrl: "https://proxy.example/v1",
      apiKey: "sk-test",
      clearKey: false,
    }),
  );
  expect(await screen.findByText("Saved.")).toBeTruthy();
  expect((screen.getByLabelText("API address") as HTMLInputElement).value).toBe("https://proxy.example/v1");
});

it("offers removal when a key is stored and reports it to the backend", async () => {
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === "deepseek_get_status") return Promise.resolve(status({ hasKey: true })) as Promise<never>;
    if (command === "deepseek_set_config") return Promise.resolve(status()) as Promise<never>;
    return Promise.resolve(undefined) as Promise<never>;
  });

  render(<DeepSeekApiChip />);
  fireEvent.click(screen.getByRole("button", { name: "API" }));
  const remove = await screen.findByRole("button", { name: "Remove key" });
  fireEvent.click(remove);

  await waitFor(() =>
    expect(invoke).toHaveBeenCalledWith("deepseek_set_config", {
      baseUrl: "",
      apiKey: "",
      clearKey: true,
    }),
  );
});

it("keeps the form read-only where the host owns the credentials", async () => {
  vi.mocked(invoke).mockImplementation((command) => {
    if (command === "deepseek_get_status") {
      return Promise.resolve(status({ editable: false, hasKey: true })) as Promise<never>;
    }
    return Promise.resolve(undefined) as Promise<never>;
  });

  render(<DeepSeekApiChip />);
  fireEvent.click(screen.getByRole("button", { name: "API" }));
  expect(await screen.findByText(/stored on the host device/)).toBeTruthy();
  expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByLabelText("API address") as HTMLInputElement).disabled).toBe(true);
});
