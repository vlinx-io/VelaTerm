import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../ipc/transport")>(),
  invoke: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("../../ipc/commands", async (original) => ({
  ...await original<typeof import("../../ipc/commands")>(),
  spawnSkillsInstalled: vi.fn().mockResolvedValue(false),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../../ipc/settingsSync", () => ({ pushSetting: vi.fn() }));

import { setLang } from "../../i18n";
import { pushSetting } from "../../ipc/settingsSync";
import { loadSettings, SETTINGS_KEY, visualOf } from "../../store/settings";
import { useTermStore } from "../../store/termStore";
import { applyVisual, fontStack, uiFontStack } from "../../theme";
import { SettingsModal } from "./SettingsModal";

beforeEach(() => {
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  vi.stubGlobal("OffscreenCanvas", class {
    getContext() { return { font: "", measureText: () => ({ fontBoundingBoxAscent: 13, fontBoundingBoxDescent: 4 }) }; }
  });
  localStorage.removeItem(SETTINGS_KEY);
  useTermStore.setState(loadSettings());
  applyVisual(visualOf(loadSettings()));
  setLang("en");
  vi.clearAllMocks();
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

it("migrates older preferences to independent conversation defaults", () => {
  localStorage.setItem(SETTINGS_KEY, JSON.stringify({ termFontFamily: "Menlo", termFontSize: 18 }));
  const settings = loadSettings();
  expect(settings.termFontSize).toBe(18);
  expect(settings.termLineHeight).toBe(1.2);
  expect(settings.chatFontFamily).toBeNull();
  expect(settings.chatFontSize).toBe(13.5);
  expect(settings.chatLineHeight).toBe(1.2);
});

it("preserves quoted custom families and explicit fallback lists when applying typography", () => {
  useTermStore.getState().setChatFontFamily('"User Installed Font"');
  expect(document.documentElement.style.getPropertyValue("--chat-font")).toMatch(/^"User Installed Font", /);
  useTermStore.getState().setChatFontFamily('"User Installed Font", monospace');
  expect(document.documentElement.style.getPropertyValue("--chat-font")).toBe('"User Installed Font", monospace');
});

it("saves and restores conversation typography without changing terminal typography", () => {
  const store = useTermStore.getState();
  store.setChatFontFamily("Menlo");
  store.setChatFontSize(12.5);
  store.setChatLineHeight(1.5);
  const saved = loadSettings();
  expect(saved).toMatchObject({ chatFontFamily: "Menlo", chatFontSize: 12.5, chatLineHeight: 1.5, termFontFamily: null, termFontSize: 13, termLineHeight: 1.2 });
  expect(pushSetting).toHaveBeenLastCalledWith(SETTINGS_KEY, expect.stringContaining('"chatFontSize":12.5'));
  expect(document.documentElement.style.getPropertyValue("--chat-font")).toBe(fontStack("Menlo"));
  expect(document.documentElement.style.getPropertyValue("--chat-fs")).toBe("12.5px");
  expect(document.documentElement.style.getPropertyValue("--chat-line-height")).toBe("25px");
  useTermStore.setState({ chatFontSize: 20, chatLineHeight: 2 });
  store.hydrateSettingsFromCache();
  expect(useTermStore.getState().chatFontSize).toBe(12.5);
  store.setTermFontFamily("Consolas");
  store.setTermFontSize(16.5);
  store.setUiFontSize(12.5);
  store.setTermLineHeight(1.8);
  expect(loadSettings()).toMatchObject({ termFontSize: 16.5, uiFontSize: 12.5 });
  expect(document.documentElement.style.getPropertyValue("--ui-fs")).toBe("12.5px");
  expect(useTermStore.getState()).toMatchObject({ chatFontFamily: "Menlo", chatFontSize: 12.5, chatLineHeight: 1.5 });
});

it("provides separate controls and resets only the selected conversation value", async () => {
  await act(async () => render(<SettingsModal onClose={() => {}} />));
  useTermStore.getState().setUiFontSize(null);
  document.documentElement.style.setProperty("--ui-fs", "12.5px");
  const interfaceSize = within(screen.getByRole("group", { name: "Interface size" }));
  fireEvent.click(interfaceSize.getByTitle("Larger"));
  expect(useTermStore.getState().uiFontSize).toBe(13);
  fireEvent.click(interfaceSize.getByTitle("Smaller"));
  expect(useTermStore.getState().uiFontSize).toBe(12.5);
  fireEvent.click(interfaceSize.getByTitle("Reset"));
  expect(useTermStore.getState().uiFontSize).toBeNull();
  fireEvent.click(screen.getByRole("link", { name: "Terminal" }));
  const terminalSize = within(screen.getByRole("group", { name: "Terminal size" }));
  expect(terminalSize.getByTitle("Reset").textContent).toBe("13px");
  fireEvent.click(terminalSize.getByTitle("Larger"));
  expect(useTermStore.getState().termFontSize).toBe(13.5);
  fireEvent.click(terminalSize.getByTitle("Reset"));
  fireEvent.click(screen.getByRole("link", { name: "Conversation view" }));
  const conversationSize = within(screen.getByRole("group", { name: "Conversation font size" }));
  fireEvent.click(conversationSize.getByTitle("Smaller"));
  expect(conversationSize.getByTitle("Reset").textContent).toBe("13px");
  expect(useTermStore.getState().termFontSize).toBe(13);
  fireEvent.click(within(screen.getByRole("group", { name: "Conversation line height" })).getByTitle("Larger"));
  expect(useTermStore.getState().chatLineHeight).toBe(1.3);
  expect(useTermStore.getState().termLineHeight).toBe(1.2);
  fireEvent.click(conversationSize.getByTitle("Reset"));
  expect(useTermStore.getState().chatFontSize).toBe(13.5);
  expect(useTermStore.getState().chatLineHeight).toBe(1.3);
});

it("normalizes malformed or out-of-range saved typography", () => {
  localStorage.setItem(SETTINGS_KEY, JSON.stringify({ chatFontSize: 999, chatLineHeight: null, chatFontFamily: 123, termLineHeight: 0 }));
  expect(loadSettings()).toMatchObject({ chatFontSize: 24, chatLineHeight: 1.2, chatFontFamily: null, termLineHeight: 1 });
});

it("applies interface fonts to the base stacks and restores them together", () => {
  const store = useTermStore.getState();
  const root = document.documentElement.style;
  store.setUiFontFamily("Avenir Next");
  store.setUiMonoFontFamily("Menlo");
  store.setUiFontSize(14);
  expect(root.getPropertyValue("--font-ui")).toBe(uiFontStack("Avenir Next"));
  expect(root.getPropertyValue("--font-ui")).toMatch(/^"Avenir Next", .*sans-serif/);
  expect(root.getPropertyValue("--font-mono")).toBe(fontStack("Menlo"));
  // The default conversation text follows the interface face, so Inter's tracking no longer applies.
  expect(root.getPropertyValue("--chat-tracking")).toBe("0px");
  expect(loadSettings()).toMatchObject({ uiFontFamily: "Avenir Next", uiMonoFontFamily: "Menlo", uiFontSize: 14 });
  store.resetUiTypography();
  expect(loadSettings()).toMatchObject({ uiFontFamily: null, uiMonoFontFamily: null, uiFontSize: null });
  expect(root.getPropertyValue("--font-ui")).toBe("");
  expect(root.getPropertyValue("--font-mono")).toBe("");
  expect(root.getPropertyValue("--chat-tracking")).toBe("");
});

it("applies the conversation code font and letter spacing, and resets every conversation value", () => {
  const store = useTermStore.getState();
  const root = document.documentElement.style;
  store.setChatCodeFontFamily("Fira Code");
  store.setChatLetterSpacing(0.55);
  store.setChatFontSize(16);
  expect(root.getPropertyValue("--chat-code-font")).toBe(fontStack("Fira Code"));
  expect(root.getPropertyValue("--chat-tracking")).toBe("0.6px");
  store.setChatLetterSpacing(9);
  expect(useTermStore.getState().chatLetterSpacing).toBe(2);
  store.setChatLetterSpacing(null);
  expect(root.getPropertyValue("--chat-tracking")).toBe("");
  store.resetChatTypography();
  expect(loadSettings()).toMatchObject({ chatFontFamily: null, chatCodeFontFamily: null, chatFontSize: 13.5, chatLineHeight: 1.2, chatLetterSpacing: null });
  expect(root.getPropertyValue("--chat-code-font")).toBe("");
});

it("steps letter spacing from the automatic value and offers group resets in the dialog", async () => {
  await act(async () => render(<SettingsModal onClose={() => {}} />));
  fireEvent.click(screen.getByRole("link", { name: "Appearance" }));
  expect(screen.getByRole("combobox", { name: "Interface font" })).toBeTruthy();
  expect(screen.getByRole("combobox", { name: "Monospace font" })).toBeTruthy();
  fireEvent.click(screen.getByRole("link", { name: "Conversation view" }));
  document.documentElement.style.setProperty("--chat-tracking", "0.2px");
  const spacing = within(screen.getByRole("group", { name: "Conversation letter spacing" }));
  expect(spacing.getByTitle("Reset").textContent).toBe("Auto");
  fireEvent.click(spacing.getByTitle("Larger"));
  expect(useTermStore.getState().chatLetterSpacing).toBe(0.3);
  useTermStore.getState().setChatFontSize(18);
  fireEvent.click(screen.getByRole("button", { name: "Restore default fonts" }));
  expect(useTermStore.getState()).toMatchObject({ chatLetterSpacing: null, chatFontSize: 13.5 });
  expect(screen.getAllByRole("group", { name: "Preview" })).toHaveLength(1);
});

it("drops malformed saved interface and code fonts", () => {
  localStorage.setItem(SETTINGS_KEY, JSON.stringify({ uiFontFamily: "  ", uiMonoFontFamily: 7, chatCodeFontFamily: " Menlo ", chatLetterSpacing: "wide" }));
  expect(loadSettings()).toMatchObject({ uiFontFamily: null, uiMonoFontFamily: null, chatCodeFontFamily: "Menlo", chatLetterSpacing: null });
});
