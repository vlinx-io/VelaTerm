import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { TranslationSettings } from "../ipc/translation";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), refresh: vi.fn(), settings: null as TranslationSettings | null }));
vi.mock("../ipc/transport", async original => ({ ...await original<typeof import("../ipc/transport")>(), invoke: mocks.invoke }));
vi.mock("../ipc/launch", async original => ({ ...await original<typeof import("../ipc/launch")>(), launchModels: vi.fn().mockResolvedValue({ models: [
  { id: "fixture-model", label: "Fixture model", effortLevels: ["low", "high"] },
  { id: "other-model", label: "Other model", effortLevels: ["low"] },
], effortLevels: ["low", "high"] }) }));
vi.mock("./state", () => ({ refreshTranslationSettings: mocks.refresh, useTranslationSettings: () => ({ value: mocks.settings, error: null, refresh: mocks.refresh }) }));
import { setLang, t } from "../i18n";
import { TranslationSettingsSection, translationError } from "./SettingsSection";

beforeEach(() => {
  setLang("en");
  HTMLElement.prototype.scrollIntoView = vi.fn();
  vi.clearAllMocks(); window.history.replaceState(null, "", "/?settings=translation");
  mocks.settings = { config: { enabled: false, defaultEnabled: true, conversationKinds: ["claude", "codex"], engine: "agent", agent: "", model: "", effort: "", serviceId: "", version: 1, generation: 1, enabledSince: 0 }, editable: true, conversationKinds: [{ id: "claude", label: "Claude" }, { id: "codex", label: "Codex" }],
    agents: [{ id: "codex", label: "Codex", available: true, acceptsTask: true, supportsPlanExecute: true, supportsReferSummary: true, effortFlag: "model_reasoning_effort", effortLevels: [] }], services: [], languages: [], engines: ["agent", "api", "google", "bing"], protocols: ["openai"], displays: ["original", "translation", "both", "immersive"] };
  mocks.refresh.mockResolvedValue(mocks.settings); mocks.invoke.mockReset().mockResolvedValue(undefined);
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

it("saves the default switch immediately without enabling the feature, including on remote clients", async () => {
  mocks.settings!.editable = false;
  mocks.invoke.mockImplementation(async (command, args) => {
    if (command === "translation_set_default_enabled") mocks.settings = { ...mocks.settings!, config: { ...mocks.settings!.config, defaultEnabled: args.enabled } };
  });
  const view = render(<TranslationSettingsSection />);
  const toggle = screen.getByRole<HTMLInputElement>("checkbox", { name: "Enable translation by default" });
  expect(toggle.checked).toBe(true);
  fireEvent.click(toggle);
  await waitFor(() => expect(toggle.checked).toBe(false));
  expect(mocks.invoke).toHaveBeenCalledExactlyOnceWith("translation_set_default_enabled", { enabled: false });
  expect(screen.getByRole<HTMLInputElement>("checkbox", { name: "Enable translation" }).checked).toBe(false);
  view.unmount(); render(<TranslationSettingsSection />);
  expect(screen.getByRole<HTMLInputElement>("checkbox", { name: "Enable translation by default" }).checked).toBe(false);
});

it("preserves the confirmed default and allows retrying a failed save", async () => {
  mocks.invoke.mockRejectedValueOnce(new Error("translation:storage"));
  render(<TranslationSettingsSection />);
  const toggle = screen.getByRole<HTMLInputElement>("checkbox", { name: "Enable translation by default" });
  fireEvent.click(toggle);
  expect(await screen.findByText("Translation is unavailable. Check the selected agent or service.")).toBeTruthy();
  expect(toggle.checked).toBe(true);
  expect(toggle.disabled).toBe(false);
  mocks.invoke.mockImplementation(async (_command, args) => {
    mocks.settings = { ...mocks.settings!, config: { ...mocks.settings!.config, defaultEnabled: args.enabled } };
  });
  fireEvent.click(toggle);
  await waitFor(() => expect(toggle.checked).toBe(false));
});

it("keeps the dropdown usable during saves and persists the latest rapid selection after reopening", async () => {
  let finish!: () => void;
  mocks.settings!.editable = false;
  mocks.invoke.mockImplementationOnce(() => new Promise<void>(resolve => { finish = resolve; }));
  mocks.invoke.mockImplementation(async (command, args) => {
    if (command === "translation_set_conversation_kinds") mocks.settings!.config.conversationKinds = args.kinds;
  });
  const view = render(<TranslationSettingsSection />);
  fireEvent.click(screen.getByRole("combobox", { name: "Conversation agent types" }));
  fireEvent.click(screen.getByRole("option", { name: "Claude" }));
  expect(screen.getByRole("option", { name: "Claude" }).getAttribute("aria-selected")).toBe("false");
  fireEvent.click(screen.getByRole("option", { name: "Codex" }));
  fireEvent.click(screen.getByRole("option", { name: "Claude" }));
  expect(screen.getByRole("option", { name: "Claude" }).getAttribute("aria-selected")).toBe("true");
  expect(screen.getByRole("option", { name: "Codex" }).getAttribute("aria-selected")).toBe("false");
  expect(screen.getByRole<HTMLInputElement>("checkbox", { name: "Enable translation" }).disabled).toBe(false);
  expect(mocks.invoke).toHaveBeenCalledExactlyOnceWith("translation_set_conversation_kinds", { kinds: ["codex"] });
  view.unmount(); render(<TranslationSettingsSection />);
  expect(screen.getByRole("combobox", { name: "Conversation agent types" }).textContent).toContain("Claude");
  await act(async () => { finish(); });
  await waitFor(() => expect(mocks.invoke).toHaveBeenLastCalledWith("translation_set_conversation_kinds", { kinds: ["claude"] }));
  await waitFor(() => expect(screen.queryByText("Saving…")).toBeNull());
  expect(mocks.settings!.config.conversationKinds).toEqual(["claude"]);
});

it("preserves failed edits with an inline retry and confirms the retried values", async () => {
  mocks.invoke.mockRejectedValueOnce(new Error("translation:storage"));
  const { rerender } = render(<TranslationSettingsSection />);
  fireEvent.click(screen.getByRole("combobox", { name: "Conversation agent types" }));
  fireEvent.click(screen.getByRole("option", { name: "Claude" }));
  const error = await screen.findByRole("alert");
  expect(error.textContent).toContain("Could not save agent types. Your selection is still pending.");
  expect(screen.getByRole("option", { name: "Claude" }).getAttribute("aria-selected")).toBe("false");
  mocks.invoke.mockImplementation(async (_command, args) => { mocks.settings!.config.conversationKinds = args.kinds; });
  fireEvent.keyDown(screen.getByRole("combobox", { name: "Conversation agent types" }), { key: "Escape" });
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  await waitFor(() => expect(screen.queryByText("Saving…")).toBeNull());
  rerender(<TranslationSettingsSection />);
  expect(mocks.settings!.config.conversationKinds).toEqual(["codex"]);
  expect(screen.getByRole("combobox", { name: "Conversation agent types" }).textContent).toContain("Codex");
});

it("explains stored payment errors without mislabeling network failures", () => {
  expect(translationError("translation:http:402", t)).toBe("The translation service requires payment (HTTP 402). Check your API account balance or billing status.");
  for (const error of ["translation:http:503", "translation:network", "translation:timeout"]) {
    expect(translationError(error, t)).toBe("The translation service is unavailable. Please retry.");
  }
});

it("saves Google without credentials and tests the saved selection", async () => {
  mocks.invoke.mockImplementation(async (command, args) => {
    if (command === "translation_save") mocks.settings!.config = args.config;
  });
  render(<TranslationSettingsSection />);
  fireEvent.click(screen.getByRole("combobox", { name: "Translation engine" }));
  fireEvent.click(screen.getByRole("option", { name: "Google Translate (no API key)" }));
  expect(screen.getByText("Free translation with Google. No API key or Google account required.")).toBeTruthy();
  expect(screen.queryByRole("combobox", { name: "Agent" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "Saved service" })).toBeNull();
  expect(screen.getByRole<HTMLButtonElement>("button", { name: "Test connection" }).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Save configuration" }));
  await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith("translation_save", { config: expect.objectContaining({ engine: "google", agent: "", serviceId: "", enabled: false }) }));
  await waitFor(() => expect(screen.getByRole<HTMLButtonElement>("button", { name: "Test connection" }).disabled).toBe(false));
  mocks.invoke.mockRejectedValueOnce(new Error("translation:http:429"));
  fireEvent.click(screen.getByRole("button", { name: "Test connection" }));
  await waitFor(() => expect(screen.getByRole("status").textContent).toBe("Too many translation requests. Please try again later."));
  expect(mocks.invoke).toHaveBeenLastCalledWith("translation_test");
});

it("saves Bing without credentials and tests the saved selection", async () => {
  mocks.invoke.mockImplementation(async (command, args) => {
    if (command === "translation_save") mocks.settings!.config = args.config;
  });
  render(<TranslationSettingsSection />);
  fireEvent.click(screen.getByRole("combobox", { name: "Translation engine" }));
  fireEvent.click(screen.getByRole("option", { name: "Bing Translator (no API key)" }));
  expect(screen.getByText("Free translation with Bing. No API key or Microsoft account required.")).toBeTruthy();
  expect(screen.queryByRole("combobox", { name: "Agent" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "Saved service" })).toBeNull();
  expect(screen.getByRole<HTMLButtonElement>("button", { name: "Test connection" }).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Save configuration" }));
  await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith("translation_save", { config: expect.objectContaining({ engine: "bing", agent: "", serviceId: "", enabled: false }) }));
  await waitFor(() => expect(screen.getByRole<HTMLButtonElement>("button", { name: "Test connection" }).disabled).toBe(false));
  mocks.invoke.mockRejectedValueOnce(new Error("translation:http:429"));
  fireEvent.click(screen.getByRole("button", { name: "Test connection" }));
  await waitFor(() => expect(screen.getByRole("status").textContent).toBe("Too many translation requests. Please try again later."));
  expect(mocks.invoke).toHaveBeenLastCalledWith("translation_test");
});

it("explains payment failures when testing the saved API configuration", async () => {
  mocks.settings!.config = { ...mocks.settings!.config, engine: "api", serviceId: "fixture-profile" };
  mocks.settings!.services = [{ id: "fixture-profile", name: "Translation fixture", baseUrl: "https://fixture.example/v1", model: "fixture-model", hasKey: true }];
  mocks.invoke.mockRejectedValueOnce(new Error("translation:http:402"));
  render(<TranslationSettingsSection />);
  fireEvent.click(screen.getByRole("button", { name: "Test connection" }));
  expect(await screen.findByRole("status")).toHaveProperty("textContent", "The translation service requires payment (HTTP 402). Check your API account balance or billing status.");
  expect(mocks.invoke).toHaveBeenCalledExactlyOnceWith("translation_test");
  expect(screen.getByRole("button", { name: "Test connection" })).toHaveProperty("disabled", false);
});

it("saves a translation agent independently without enabling the feature or sending conversation content", async () => {
  render(<TranslationSettingsSection />);
  fireEvent.click(screen.getByRole("combobox", { name: "Agent" }));
  fireEvent.click(screen.getByRole("option", { name: "Codex" }));
  fireEvent.click(screen.getByLabelText("Model"));
  fireEvent.click(await screen.findByRole("option", { name: /Fixture model/ }));
  fireEvent.click(screen.getByLabelText("Reasoning effort"));
  fireEvent.click(screen.getByRole("option", { name: "High" }));
  fireEvent.click(screen.getByRole("button", { name: "Save configuration" }));
  await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith("translation_save", { config: expect.objectContaining({ agent: "codex", engine: "agent", model: "fixture-model", effort: "high", enabled: false }) }));
  expect(mocks.invoke).toHaveBeenCalledTimes(1);
});

it("clears an incompatible effort when switching models and preserves the default choice", async () => {
  mocks.settings!.config = { ...mocks.settings!.config, agent: "codex", model: "fixture-model", effort: "high" };
  render(<TranslationSettingsSection />);
  fireEvent.click(screen.getByLabelText("Model"));
  fireEvent.click(await screen.findByRole("option", { name: /Other model/ }));
  expect(screen.getByLabelText("Reasoning effort").textContent).toContain("Auto");
  fireEvent.click(screen.getByLabelText("Reasoning effort"));
  expect(screen.queryByRole("option", { name: "High" })).toBeNull();
  fireEvent.click(screen.getByRole("option", { name: "Auto" }));
  fireEvent.click(screen.getByRole("button", { name: "Save configuration" }));
  await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith("translation_save", { config: expect.objectContaining({ model: "other-model", effort: "" }) }));
});

it("saves an API profile on the host, keeps its key out of global configuration, and closes its edit URL", async () => {
  window.history.replaceState(null, "", "/?settings=translation&translationService=new");
  mocks.invoke.mockImplementation(async command => command === "translation_service_save" ? "fixture-profile" : undefined);
  render(<TranslationSettingsSection />);
  fireEvent.change(screen.getByLabelText(/Service name/), { target: { value: "Translation fixture" } });
  fireEvent.change(screen.getByLabelText(/Base URL/), { target: { value: "https://fixture.example/v1" } });
  fireEvent.change(screen.getByLabelText(/^Model\s+\*/), { target: { value: "fixture-model" } });
  fireEvent.change(screen.getByLabelText("API key"), { target: { value: "synthetic-test-key" } });
  fireEvent.click(screen.getByRole("button", { name: /^Save$/ }));
  await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith("translation_save", { config: expect.objectContaining({ engine: "api", serviceId: "fixture-profile", enabled: false }) }));
  expect(mocks.invoke).toHaveBeenCalledWith("translation_service_save", { service: { id: undefined, name: "Translation fixture", baseUrl: "https://fixture.example/v1", model: "fixture-model", apiKey: "synthetic-test-key", clearKey: false } });
  expect(mocks.invoke.mock.calls.find(([command]) => command === "translation_save")?.[1].config).not.toHaveProperty("apiKey");
  await waitFor(() => expect(new URL(location.href).searchParams.has("translationService")).toBe(false));
});
