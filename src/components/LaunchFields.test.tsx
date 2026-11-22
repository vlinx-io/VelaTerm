import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { LaunchModelCatalog, LaunchOption } from "../ipc/launch";

const lookup = vi.hoisted(() => vi.fn());
const listeners = vi.hoisted(() => new Map<string, Set<() => void>>());
vi.mock("../ipc/launch", () => ({ launchModels: lookup }));
vi.mock("../ipc/events", () => ({
  onAntigravityModelsChanged: (cb: () => void) => {
    let set = listeners.get("antigravity://models-changed");
    if (!set) {
      set = new Set();
      listeners.set("antigravity://models-changed", set);
    }
    set.add(cb);
    return Promise.resolve(() => { set?.delete(cb); });
  },
}));
vi.mock("../i18n", () => ({
  t: (key: string) => key,
  useT: () => (key: string, ...args: unknown[]) => args.length ? `${key}:${args.join(",")}` : key,
}));
import { ModelEffortFields } from "./LaunchFields";

const catalog: LaunchModelCatalog = {
  models: [
    { id: "provider-a/shared-model", label: "Shared model", effortLevels: ["low", "xhigh"] },
    { id: "provider-b/shared-model", label: "Shared model", effortLevels: ["off"] },
    { id: "provider-c/non-thinking", label: "Non-thinking model", effortLevels: [] },
  ], effortLevels: ["low", "xhigh", "off"],
};

function Harness({ kind = "codex", cwd = "/repo" }: { kind?: string; cwd?: string }) {
  const [draft, setDraft] = useState({ model: "provider-a/shared-model", effort: "xhigh" });
  return <><ModelEffortFields spec={{ id: kind, effortFlag: "effort", effortLevels: ["invented"] } as LaunchOption}
    {...draft} context={{ parentSessionId: "parent", cwd }} onChange={(model, effort) => setDraft({ model, effort })} />
    <output>{JSON.stringify(draft)}</output></>;
}

beforeEach(() => { lookup.mockReset().mockResolvedValue(catalog); });
afterEach(cleanup);

it.each(["claude", "codex", "opencode", "pi", "omp"])("reads %s model-specific effort levels and retains the selected provider", async kind => {
  render(<Harness kind={kind} />);
  const effort = screen.getByLabelText("spawn.effortLabel");
  fireEvent.click(effort);
  await screen.findByRole("option", { name: "chat.effort.low" });
  expect(screen.queryByRole("option", { name: "invented" })).toBeNull();
  expect(screen.queryByRole("option", { name: /chat\.effort\.off/ })).toBeNull();
  expect(lookup).toHaveBeenCalledWith(kind, { parentSessionId: "parent", cwd: "/repo", inheritArgs: false });
  fireEvent.keyDown(effort, { key: "Escape" });
  fireEvent.click(screen.getByLabelText("spawn.modelLabel"));
  fireEvent.click(screen.getByRole("option", { name: /provider-b\/shared-model/ }));
  expect(screen.getByRole("status").textContent).toBe(JSON.stringify({ model: "provider-b/shared-model", effort: "" }));
  fireEvent.click(effort);
  const effortOptions = screen.getAllByRole("option").map(option => option.textContent);
  expect(effortOptions).toEqual([expect.stringContaining("chat.effort.off"), "chat.effort.auto"]);
  fireEvent.click(screen.getByRole("option", { name: /chat\.effort\.off/ }));
  expect(screen.getByRole("status").textContent).toBe(JSON.stringify({ model: "provider-b/shared-model", effort: "off" }));
});

it("disables reasoning effort on non-thinking models and accepts spaceless custom model IDs", async () => {
  render(<Harness />);
  const modelBtn = screen.getByLabelText("spawn.modelLabel");
  const effortBtn = screen.getByLabelText("spawn.effortLabel") as HTMLButtonElement;
  fireEvent.click(modelBtn);
  fireEvent.click(await screen.findByRole("option", { name: /Non-thinking model/ }));
  expect(effortBtn.disabled).toBe(true);
  expect(screen.getByRole("status").textContent).toBe(JSON.stringify({ model: "provider-c/non-thinking", effort: "" }));

  fireEvent.click(modelBtn);
  const popup = await screen.findByRole("listbox");
  const filter = within(popup).getByRole("textbox");
  fireEvent.change(filter, { target: { value: "two words" } });
  expect(within(popup).queryByRole("option", { name: /settings\.fontUseCustom/ })).toBeNull();
  fireEvent.change(filter, { target: { value: "custom-model-1" } });
  fireEvent.click(within(popup).getByRole("option", { name: "settings.fontUseCustom:custom-model-1" }));
  expect(modelBtn.textContent).toContain("custom-model-1");
  expect(effortBtn.disabled).toBe(false);
});

it("shows the lookup failure, preserves the draft and retries both model and effort choices", async () => {
  lookup.mockRejectedValueOnce(new Error("Provider catalogue unavailable"));
  render(<Harness />);
  await screen.findByText("Provider catalogue unavailable");
  expect(screen.getByLabelText("spawn.modelLabel").textContent).toContain("provider-a/shared-model");
  expect(screen.getByLabelText("spawn.effortLabel").textContent).toContain("chat.effort.xhigh");
  fireEvent.click(screen.getByRole("button", { name: "common.retry" }));
  fireEvent.click(screen.getByLabelText("spawn.effortLabel"));
  await screen.findByRole("option", { name: "chat.effort.low" });
  expect(lookup).toHaveBeenCalledTimes(2);
});

it("ignores a previous directory's late catalogue", async () => {
  let resolveOld!: (catalog: LaunchModelCatalog) => void;
  lookup.mockImplementationOnce(() => new Promise<LaunchModelCatalog>(resolve => { resolveOld = resolve; }));
  const view = render(<Harness cwd="/old" />);
  await waitFor(() => expect(lookup).toHaveBeenCalledOnce());
  view.rerender(<Harness cwd="/new" />);
  fireEvent.click(screen.getByLabelText("spawn.effortLabel"));
  await screen.findByRole("option", { name: "chat.effort.low" });
  await act(async () => resolveOld({ models: [], effortLevels: ["wrong-directory"] }));
  expect(screen.queryByRole("option", { name: "wrong-directory" })).toBeNull();
  expect(screen.getByRole("option", { name: "chat.effort.low" })).toBeTruthy();
});

it("refreshes Antigravity launch models when background discovery updates the catalogue", async () => {
  lookup.mockResolvedValueOnce({
    models: [{ id: "gemini-4-hydrogen", label: "Gemini 4 Hydrogen", effortLevels: ["high"] }],
    effortLevels: ["high"],
  });
  render(<Harness kind="antigravity" />);
  const modelBtn = screen.getByLabelText("spawn.modelLabel");
  fireEvent.click(modelBtn);
  await screen.findByRole("option", { name: /Gemini 4 Hydrogen/ });
  expect(screen.queryByRole("option", { name: /Gemini 4 Neon/ })).toBeNull();

  lookup.mockResolvedValueOnce({
    models: [
      { id: "gemini-4-hydrogen", label: "Gemini 4 Hydrogen", effortLevels: ["high"] },
      { id: "gemini-4-neon", label: "Gemini 4 Neon", effortLevels: ["high", "xhigh"] },
    ],
    effortLevels: ["high", "xhigh"],
  });
  await act(async () => {
    listeners.get("antigravity://models-changed")?.forEach((cb) => cb());
  });
  await screen.findByRole("option", { name: /Gemini 4 Neon/ });
});
