import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { useTranslationSession } from "./state";

const mocks = vi.hoisted(() => ({ state: null as ReturnType<typeof useTranslationSession> | null, save: vi.fn(), refresh: vi.fn(), copy: vi.fn() }));
vi.mock("./state", () => ({ useConversationTranslation: () => mocks.state }));
vi.mock("./SettingsSection", () => ({ translationError: () => "Translation unavailable" }));
vi.mock("../platform", () => ({ env: { isBrowser: false, isRemoteWindow: false }, platform: { clipboard: { writeText: mocks.copy } } }));
import { setLang } from "../i18n";
import { TranslationComposerControl } from "./ComposerControl";
import { ReplyTranslation } from "./Message";
import { MessageBubble } from "../layout/CenterPane/session/rows";

beforeEach(() => {
  setLang("en"); vi.clearAllMocks(); window.history.replaceState(null, "", "/");
  mocks.save.mockResolvedValue(undefined); mocks.refresh.mockResolvedValue(undefined); mocks.copy.mockResolvedValue(undefined);
  mocks.state = { sessionId: "fixture", originals: [], refresh: mocks.refresh, save: mocks.save,
    settings: { config: { enabled: true, defaultEnabled: true, conversationKinds: ["claude", "codex"], engine: "agent", agent: "codex", model: "fixture-model", effort: "", serviceId: "", version: 1, generation: 1, enabledSince: 0 }, editable: true, conversationKinds: [{ id: "claude", label: "Claude" }, { id: "codex", label: "Codex" }],
      agents: [{ id: "codex", label: "Codex", available: true, acceptsTask: true, supportsPlanExecute: true, supportsReferSummary: true, effortFlag: "model_reasoning_effort", effortLevels: [] }],
      services: [], languages: [{ code: "en", name: "English" }, { code: "de", name: "Deutsch" }], engines: ["agent", "api"], protocols: ["openai"], displays: ["original", "translation", "both", "immersive"] },
    value: { enabled: true, version: 1, generation: 1, preferences: { outgoing: true, replies: true, source: "auto", target: "en", replySource: "en", replyTarget: "de", display: "immersive", repliesSince: 0 }, replies: [], messages: [], pending: [] } };
});
afterEach(cleanup);

it("toggles sending and reply translation together and opens languages from a compact settings link", async () => {
  render(<TranslationComposerControl />);
  expect(screen.getByText("Translation")).toBeTruthy();
  expect(screen.getByRole("link", { name: "Translation languages" }).textContent).toBe("");
  expect(screen.queryByText("Auto-detect → English")).toBeNull();
  fireEvent.click(screen.getByRole("switch", { name: "Translation" }));
  await waitFor(() => expect(mocks.save).toHaveBeenCalledWith(expect.objectContaining({ outgoing: false, replies: false, target: "en" })));
  expect(window.location.search).toBe(""); expect(screen.queryByRole("dialog")).toBeNull();
  fireEvent.click(screen.getByTitle("Translation languages"));
  expect(screen.getByRole("dialog", { name: "Translation languages" })).toBeTruthy();
  expect(new URL(location.href).searchParams.get("translation")).toBe("languages");
  expect(screen.getAllByRole("switch")).toHaveLength(2);
  fireEvent.click(screen.getByRole("link", { name: "More settings" }));
  expect(new URL(location.href).searchParams.get("settings")).toBe("translation");
  expect(new URL(location.href).searchParams.has("translation")).toBe(false);
});

it("hides both the switch and language direction when the global feature is disabled", () => {
  mocks.state!.settings!.config.enabled = false;
  const { container } = render(<TranslationComposerControl />);
  expect(container.childElementCount).toBe(0);
  expect(screen.queryByRole("switch")).toBeNull();
  expect(screen.queryByTitle("Translation languages")).toBeNull();
});

it("hides the switch, language link and open language panel for an excluded conversation type", () => {
  window.history.replaceState(null, "", "/?translation=languages&translationSession=fixture");
  const { rerender } = render(<TranslationComposerControl />);
  expect(screen.getByRole("dialog", { name: "Translation languages" })).toBeTruthy();
  mocks.state!.value!.enabled = false;
  rerender(<TranslationComposerControl />);
  expect(screen.queryByRole("switch")).toBeNull();
  expect(screen.queryByTitle("Translation languages")).toBeNull();
  expect(screen.queryByRole("dialog")).toBeNull();
  mocks.state!.value!.enabled = true;
  rerender(<TranslationComposerControl />);
  expect(screen.getByRole("switch", { name: "Translation" })).toBeTruthy();
});

it("shows the enabled default in each conversation and hides both controls when Settings disables it", () => {
  const { rerender } = render(<TranslationComposerControl />);
  expect(screen.getByRole<HTMLInputElement>("switch", { name: "Translation" }).checked).toBe(true);
  mocks.state = { ...mocks.state!, sessionId: "new-session", value: { ...mocks.state!.value!, preferences: { ...mocks.state!.value!.preferences } } };
  rerender(<TranslationComposerControl />);
  expect(screen.getByRole<HTMLInputElement>("switch", { name: "Translation" }).checked).toBe(true);
  expect(screen.getByTitle("Translation languages")).toBeTruthy();
  mocks.state!.value!.enabled = false;
  rerender(<TranslationComposerControl />);
  expect(screen.queryByRole("switch")).toBeNull();
  expect(screen.queryByTitle("Translation languages")).toBeNull();
});

it.each(["agent", "api", "google"] as const)("switches on without navigation even before choosing languages with the %s engine", async engine => {
  mocks.state!.settings!.config.engine = engine;
  if (engine === "api") {
    mocks.state!.settings!.config.serviceId = "api-service";
    mocks.state!.settings!.services = [{ id: "api-service", name: "API fixture", baseUrl: "https://api.example.com", model: "fixture-model", hasKey: true }];
  }
  Object.assign(mocks.state!.value!.preferences, { outgoing: false, replies: false, target: "" });
  mocks.save.mockImplementation(async prefs => { mocks.state!.value!.preferences = prefs; });
  render(<TranslationComposerControl />);
  const toggle = screen.getByRole<HTMLInputElement>("switch", { name: "Translation" });
  expect(toggle.disabled).toBe(false);
  fireEvent.click(toggle);
  await waitFor(() => expect(toggle.checked).toBe(true));
  expect(mocks.save).toHaveBeenCalledWith(expect.objectContaining({ outgoing: true, replies: true, target: "" }));
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(window.location.search).toBe("");
  expect(screen.getByRole("status").textContent).toBe("Select a target language before translating.");
  fireEvent.click(toggle);
  await waitFor(() => expect(toggle.checked).toBe(false));
  expect(screen.queryByRole("status")).toBeNull();
});

it("recognizes Google as configured without an installed agent or saved API service", () => {
  mocks.state!.settings!.config.engine = "google";
  mocks.state!.settings!.agents = [];
  render(<TranslationComposerControl />);
  expect(screen.queryByText("Configure translation in Settings.")).toBeNull();
  fireEvent.click(screen.getByTitle("Translation languages"));
  expect(screen.queryByText("Configure translation in Settings.")).toBeNull();
});

it.each([false, true])("edits the %s reply direction without enabling translation", async reply => {
  Object.assign(mocks.state!.value!.preferences, { outgoing: false, replies: false, target: "", replyTarget: "" });
  render(<TranslationComposerControl />);
  fireEvent.click(screen.getByTitle("Translation languages"));
  fireEvent.click(screen.getByRole("combobox", { name: reply ? "Reply target language" : "Target language" }));
  fireEvent.click(screen.getByRole("option", { name: "English" }));
  await waitFor(() => expect(mocks.save).toHaveBeenCalledWith(expect.objectContaining({ outgoing: false, replies: false, [reply ? "replyTarget" : "target"]: "en" })));
});

it("shows an explicit settings link without navigating when the engine is unconfigured", async () => {
  mocks.state!.settings!.config.engine = "api";
  Object.assign(mocks.state!.value!.preferences, { outgoing: false, replies: false });
  mocks.save.mockImplementation(async prefs => { mocks.state!.value!.preferences = prefs; });
  render(<TranslationComposerControl />);
  fireEvent.click(screen.getByRole("switch", { name: "Translation" }));
  await waitFor(() => expect(screen.getByRole("status").textContent).toBe("Configure translation in Settings."));
  expect(window.location.search).toBe("");
  expect(screen.queryByRole("dialog")).toBeNull();
  fireEvent.click(screen.getByRole("link", { name: "Configure translation in Settings." }));
  expect(new URL(location.href).searchParams.get("settings")).toBe("translation");
});

it("toggles replies independently without navigating or changing either direction", async () => {
  Object.assign(mocks.state!.value!.preferences, { replies: false, replyTarget: "" });
  render(<TranslationComposerControl />);
  fireEvent.click(screen.getByTitle("Translation languages"));
  const url = location.href;
  fireEvent.click(screen.getByRole("switch", { name: "Translate replies" }));
  await waitFor(() => expect(mocks.save).toHaveBeenCalledWith(expect.objectContaining({ replies: true, replyTarget: "", outgoing: true, target: "en" })));
  expect(location.href).toBe(url);
});

it("shows save errors in place and keeps the saved switch value after failure", async () => {
  mocks.save.mockRejectedValueOnce(new Error("translation:storage"));
  render(<TranslationComposerControl />);
  fireEvent.click(screen.getByRole("switch", { name: "Translation" }));
  await waitFor(() => expect(screen.getByRole("alert").textContent).toBe("Translation unavailable"));
  const toggle = screen.getByRole<HTMLInputElement>("switch", { name: "Translation" });
  expect(toggle.disabled).toBe(false);
  expect(toggle.checked).toBe(true);
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(window.location.search).toBe("");
});

it("retains saved directions through off/on and displays the next conversation's own preferences", async () => {
  mocks.save.mockImplementation(async prefs => { mocks.state!.value!.preferences = prefs; });
  const { rerender } = render(<TranslationComposerControl />);
  const toggle = screen.getByRole<HTMLInputElement>("switch", { name: "Translation" });
  fireEvent.click(toggle);
  await waitFor(() => expect(toggle.checked).toBe(false));
  fireEvent.click(toggle);
  await waitFor(() => expect(toggle.checked).toBe(true));
  expect(mocks.save).toHaveBeenLastCalledWith(expect.objectContaining({ outgoing: true, replies: true, target: "en", replyTarget: "de" }));
  mocks.state = { ...mocks.state!, sessionId: "another", value: { ...mocks.state!.value!, preferences: { ...mocks.state!.value!.preferences, outgoing: false, replies: false, target: "de" } } };
  rerender(<TranslationComposerControl />);
  expect(toggle.checked).toBe(false);
  expect(screen.queryByRole("dialog")).toBeNull();
  fireEvent.click(screen.getByRole("link", { name: "Translation languages" }));
  expect(screen.getByRole("combobox", { name: "Target language" }).textContent).toContain("Deutsch");
});

it("pairs complete Markdown blocks and copies both documents without splitting their structure", async () => {
  const text = "First line.\nSecond line.";
  mocks.state!.value!.replies = [{ id: "reply", original: text, source: "en", target: "de", version: 1, generation: 1, state: "ready", error: null,
    result: { sourceLanguage: "en", formatVersion: 2, pairs: [{ original: "First line.", translation: "Erste Zeile." }, { original: "Second line.", translation: "Zweite Zeile." }] } }];
  const { container } = render(<ReplyTranslation rowId="reply" text={text} original={<div data-testid="complete-original">{text}</div>} />);
  const pairs = Array.from(container.querySelectorAll(".translation-pair"));
  expect(pairs.map(pair => Array.from(pair.children).map(row => row.textContent))).toEqual([["First line.\nSecond line.", "Erste Zeile.\nZweite Zeile."]]);
  const bilingual = screen.getByRole("button", { name: "Copy original and translation" });
  const translation = screen.getByRole("button", { name: "Copy translation" });
  expect(bilingual.closest(".translation-copy-actions")).toBe(translation.closest(".translation-copy-actions"));
  expect(bilingual.closest(".translation-copy-actions")?.previousElementSibling?.contains(screen.getByRole("combobox", { name: "Reply display" }))).toBe(true);
  expect(document.getElementById(bilingual.getAttribute("aria-describedby")!)?.textContent).toBe("Copy original and translation");
  expect(document.getElementById(translation.getAttribute("aria-describedby")!)?.textContent).toBe("Copy translation");
  fireEvent.click(bilingual);
  await waitFor(() => expect(mocks.copy).toHaveBeenCalledWith("First line.\nSecond line.\n\nErste Zeile.\nZweite Zeile.", { reportFailure: true }));
  await waitFor(() => expect(document.getElementById(bilingual.getAttribute("aria-describedby")!)?.textContent).toBe("Original and translation copied"));
  fireEvent.click(translation);
  await waitFor(() => expect(mocks.copy).toHaveBeenLastCalledWith("Erste Zeile.\nZweite Zeile.", { reportFailure: true }));
  await waitFor(() => expect(document.getElementById(translation.getAttribute("aria-describedby")!)?.textContent).toBe("Translation copied"));
  fireEvent.click(screen.getByRole("combobox", { name: "Reply display" }));
  fireEvent.click(screen.getByRole("option", { name: "Both" }));
  expect(container.querySelector(".translation-pairs")).toBeNull();
  expect(screen.getByTestId("complete-original")).toBeTruthy();
  expect(container.querySelectorAll(".translation-reply-blocks > div")).toHaveLength(2);
  fireEvent.click(screen.getByRole("button", { name: "Copy original and translation" }));
  await waitFor(() => expect(mocks.copy).toHaveBeenLastCalledWith("First line.\nSecond line.\n\nErste Zeile.\nZweite Zeile.", { reportFailure: true }));
});

it("keeps an earlier translation visible after the engine configuration changes and offers a fresh one", () => {
  const text = "Hello";
  mocks.state!.value!.version = 2; mocks.state!.value!.generation = 3;
  mocks.state!.value!.replies = [{ id: "reply", original: text, source: "en", target: "de", version: 1, generation: 1, state: "ready", error: null,
    result: { sourceLanguage: "en", formatVersion: 2, pairs: [{ original: text, translation: "Hallo" }] } }];
  const { container } = render(<ReplyTranslation rowId="reply" text={text} original={<div>{text}</div>} />);
  expect(container.querySelector(".translation-pair-target")?.textContent).toBe("Hallo");
  expect(screen.getByText("This translation uses earlier text, language settings or an engine configuration.")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Retry translation" })).toBeTruthy();
});

it("hides an earlier translation once the reply target language changes", () => {
  const text = "Hello";
  mocks.state!.value!.preferences.replyTarget = "en";
  mocks.state!.value!.replies = [{ id: "reply", original: text, source: "en", target: "de", version: 1, generation: 1, state: "ready", error: null,
    result: { sourceLanguage: "en", formatVersion: 2, pairs: [{ original: text, translation: "Hallo" }] } }];
  const { container } = render(<ReplyTranslation rowId="reply" text={text} original={<div>{text}</div>} />);
  expect(container.querySelector(".translation-pairs")).toBeNull();
  expect(screen.getByRole("button", { name: "Translation" })).toBeTruthy();
});

it("confirms copying only after clipboard success and reports failures without a success message", async () => {
  const text = "Hello";
  mocks.state!.value!.replies = [{ id: "reply", original: text, source: "en", target: "de", version: 1, generation: 1, state: "ready", error: null,
    result: { sourceLanguage: "en", formatVersion: 2, pairs: [{ original: text, translation: "Hallo" }] } }];
  let rejectCopy!: (error: Error) => void;
  mocks.copy.mockReturnValueOnce(new Promise<void>((_, reject) => { rejectCopy = reject; }));
  render(<ReplyTranslation rowId="reply" text={text} original={<div>{text}</div>} />);
  const button = screen.getByRole("button", { name: "Copy translation" });
  fireEvent.click(button);
  expect(button.getAttribute("aria-busy")).toBe("true");
  expect(screen.queryByRole("status")).toBeNull();
  fireEvent.click(button);
  expect(mocks.copy).toHaveBeenCalledTimes(1);
  rejectCopy(new Error("Clipboard unavailable"));
  await waitFor(() => expect(document.getElementById(button.getAttribute("aria-describedby")!)?.textContent).toBe("Could not copy. Please try again."));
  expect(screen.getByRole("status").textContent).toBe("Could not copy. Please try again.");
  fireEvent.click(button);
  await waitFor(() => expect(screen.getByRole("status").textContent).toBe("Translation copied"));
});

it("presents translating as a status rather than a disabled action", () => {
  mocks.state!.value!.replies = [{ id: "reply", original: "Pending.", source: "en", target: "de", version: 1, generation: 1, state: "pending", error: null, result: null }];
  render(<ReplyTranslation rowId="reply" text="Pending." original={<div>Pending.</div>} />);
  expect(screen.getByRole("status").textContent).toBe("Translating…");
  expect(screen.queryByRole("button", { name: "Translating…" })).toBeNull();
});


it("keeps the original, status and ready translation in the same message through confirmation", () => {
  const view = (text: string, pending: boolean) => <MessageBubble key="user" rowId="user" who="You" isUser text={text} footnote={pending ? <div role="status">Translating…</div> : undefined} />;
  const { container, rerender } = render(view("Original", true));
  const bubble = container.querySelector(".sv-msg-user");
  expect(bubble?.querySelector(".sv-message-content")?.contains(screen.getByRole("status"))).toBe(true);
  const message = { id: "user", original: "Original", source: "en", target: "de", accepted: false, durationMs: 1530, result: { sourceLanguage: "en", formatVersion: 2, pairs: [{ original: "Original", translation: "Übersetzung" }] } };
  mocks.state!.originals = [message]; mocks.state!.value!.messages = [message];
  rerender(view("Original", false));
  expect(screen.getByText("Original")).toBeTruthy(); expect(screen.getByText("Übersetzung")).toBeTruthy();
  expect(screen.getByText("Translation time: 1.5 sec")).toBeTruthy();
  expect(container.querySelector(".translation-sent button")).toBeNull();
  expect(container.querySelector(".translation-pair-original")?.textContent).toBe("Original");
  expect(container.querySelector(".translation-pair-target")?.textContent).toBe("Übersetzung");
  rerender(view("Übersetzung", false));
  expect(container.querySelector(".sv-msg-user")).toBe(bubble);
  expect(screen.getAllByText("Original")).toHaveLength(1); expect(screen.getAllByText("Übersetzung")).toHaveLength(1);
});

it("uses the same line pairs for sent originals and translations with one duration below the whole group", () => {
  const message = { id: "user", original: "One\nTwo", target: "de", accepted: true, durationMs: 1530,
    result: { sourceLanguage: "en", formatVersion: 2, pairs: [{ original: "One", translation: "Eins" }, { original: "Two", translation: "Zwei" }] } };
  mocks.state!.originals = [message]; mocks.state!.value!.messages = [message];
  const { container, rerender } = render(<MessageBubble rowId="user" who="You" isUser text="Eins\nZwei" />);
  expect(Array.from(container.querySelectorAll(".translation-pair")).map(pair => Array.from(pair.children).map(line => line.textContent))).toEqual([["One\nTwo", "Eins\nZwei"]]);
  expect(screen.getAllByText("Translation time: 1.5 sec")).toHaveLength(1);
  expect(container.querySelector(".translation-pairs")!.compareDocumentPosition(screen.getByText("Translation time: 1.5 sec")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  mocks.state!.settings!.config.enabled = false;
  rerender(<MessageBubble rowId="user" who="You" isUser text="Eins\nZwei" />);
  expect(container.querySelector(".translation-pairs")).toBeNull();
  expect(container.textContent).toContain("One");
  expect(container.textContent).not.toContain("Eins");
});

it("places progress below the original even before a streamed reply reaches its first stable line", () => {
  render(<ReplyTranslation rowId="reply" text="Pending." streaming original={<div>Pending.</div>} />);
  const original = screen.getByText("Pending."); const status = screen.getByRole("status");
  expect(original.compareDocumentPosition(status) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Translation" })).toBeNull();
});

it("retains the untranslated tail while the next streamed batch is pending", () => {
  const text = "First line.\nSecond line.";
  mocks.state!.value!.replies = [{ id: "reply", original: text, source: "en", target: "de", version: 1, generation: 1, state: "pending", error: null,
    result: { sourceLanguage: "en", formatVersion: 2, pairs: [{ original: "First line.", translation: "Erste Zeile." }] } }];
  const { container } = render(<ReplyTranslation rowId="reply" text={text} original={<div>{text}</div>} />);
  expect(container.querySelector(".translation-pair-original")?.textContent).toBe(text);
  expect(container.querySelector(".translation-pair-target")?.textContent).toBe("Erste Zeile.\nSecond line.");
  expect(container.querySelector(".translation-pairs")!.compareDocumentPosition(screen.getByRole("status")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
});

it("shows the backend's default checked reply switch and preserves an explicit opt-out", () => {
  const { rerender } = render(<TranslationComposerControl />);
  fireEvent.click(screen.getByTitle("Translation languages"));
  expect(screen.getByRole<HTMLInputElement>("switch", { name: "Translate replies" }).checked).toBe(true);
  mocks.state!.value!.preferences.replies = false;
  rerender(<TranslationComposerControl />);
  expect(screen.getByRole<HTMLInputElement>("switch", { name: "Translate replies" }).checked).toBe(false);
});
