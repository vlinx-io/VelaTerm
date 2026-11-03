import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ChatPermission } from "../ipc/chat";
import type { QuestionTranslation } from "../ipc/translation";
import type { useTranslationSession } from "./state";

const mocks = vi.hoisted(() => ({ state: null as ReturnType<typeof useTranslationSession> | null, translate: vi.fn() }));
vi.mock("./state", () => ({ useConversationTranslation: () => mocks.state }));
vi.mock("./SettingsSection", () => ({ translationError: () => "Translation unavailable" }));
vi.mock("../ipc/translation", () => ({ translateQuestion: mocks.translate }));
import { setLang } from "../i18n";
import { PermissionCard } from "../layout/CenterPane/session/permissionCards";

const input = { questions: [{ header: "Plan", question: "Approve this plan?", options: [
  { label: "Additive zip fields", description: "Keep install.sh compatible." }, { label: "Zip only" },
], multiSelect: false }] };
const request: ChatPermission = { id: "ask-1", tool_name: "AskUserQuestion", input };
const translated: QuestionTranslation = { input: { questions: [{ ...input.questions[0], header: "方案", question: "批准此方案吗？",
  options: [{ label: "新增 zip 字段", description: "保留 install.sh 的兼容性。" }, { label: "仅发布 zip" }],
}] }, originalInput: input, version: 1, generation: 1, source: "en", target: "zh-CN", durationMs: 100 };

beforeEach(() => {
  setLang("en"); vi.clearAllMocks(); mocks.translate.mockResolvedValue(translated);
  mocks.state = { sessionId: "fixture", originals: [], refresh: vi.fn(), save: vi.fn(),
    settings: { config: { enabled: true, defaultEnabled: true, conversationKinds: ["claude"], engine: "google", agent: "", model: "", effort: "", serviceId: "", version: 1, generation: 1, enabledSince: 0 },
      editable: true, conversationKinds: [], agents: [], services: [], languages: [], engines: ["google"], protocols: [], displays: ["original", "translation", "both", "immersive"] },
    value: { enabled: true, version: 1, generation: 1, preferences: { outgoing: true, replies: true, source: "auto", target: "en", replySource: "en", replyTarget: "zh-CN", display: "immersive", repliesSince: 0 },
      replies: [], messages: [], pending: [] } };
});
afterEach(cleanup);

it("translates questions and options while submitting original option indices", async () => {
  const answer = vi.fn(); render(<PermissionCard request={request} onAnswer={answer} />);
  expect(await screen.findByText("批准此方案吗？")).toBeTruthy();
  expect(screen.getByText("Approve this plan?")).toBeTruthy();
  expect(screen.getByText("保留 install.sh 的兼容性。")).toBeTruthy();
  fireEvent.click(screen.getByRole("radio", { name: /新增 zip 字段/ }));
  fireEvent.click(screen.getByRole("button", { name: "Submit" }));
  await waitFor(() => expect(answer).toHaveBeenCalledWith(request, { allow: true, questionAnswers: [{ selected: [0], text: "" }], sendOriginal: false }));
  expect(input.questions[0].question).toBe("Approve this plan?");
});

it("retains typed answers after failure and lets the user explicitly send the original", async () => {
  const answer = vi.fn().mockRejectedValueOnce("translation:network").mockResolvedValue(undefined);
  render(<PermissionCard request={request} onAnswer={answer} />);
  fireEvent.change(screen.getByRole("textbox"), { target: { value: "请保留原来的字段" } });
  fireEvent.click(screen.getByRole("button", { name: "Submit" }));
  expect(await screen.findByRole("alert")).toBeTruthy();
  expect(screen.getByRole<HTMLInputElement>("textbox").value).toBe("请保留原来的字段");
  fireEvent.click(screen.getByRole("button", { name: "Send original" }));
  await waitFor(() => expect(answer).toHaveBeenLastCalledWith(request, { allow: true,
    questionAnswers: [{ selected: [], text: "请保留原来的字段" }], sendOriginal: true }));
});

it("locks a pending submission against double clicks and keeps multi-select indices intact", async () => {
  let finish!: () => void;
  const answer = vi.fn(() => new Promise<void>(resolve => { finish = resolve; }));
  const multi = { ...request, input: { questions: [{ ...input.questions[0], multiSelect: true }] } };
  mocks.state!.value!.preferences.replies = false;
  render(<PermissionCard request={multi} onAnswer={answer} />);
  fireEvent.click(screen.getByRole("checkbox", { name: /Additive/ })); fireEvent.click(screen.getByRole("checkbox", { name: /Zip only/ }));
  const submit = screen.getByRole("button", { name: "Submit" });
  fireEvent.click(submit); fireEvent.click(submit);
  expect(answer).toHaveBeenCalledTimes(1);
  expect(answer.mock.calls[0]).toEqual([multi, { allow: true, questionAnswers: [{ selected: [0, 1], text: "" }], sendOriginal: false }]);
  expect(screen.getByRole<HTMLInputElement>("textbox").disabled).toBe(true);
  expect(screen.getByRole<HTMLButtonElement>("button", { name: "Dismiss" }).disabled).toBe(true);
  await act(async () => finish());
});

it("discards a late translation after the reply language changes or translation is disabled", async () => {
  let finish!: (value: QuestionTranslation) => void;
  mocks.translate.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  const { rerender } = render(<PermissionCard request={request} onAnswer={vi.fn()} />);
  mocks.state!.value!.preferences.replyTarget = "de";
  mocks.translate.mockResolvedValue({ ...translated, target: "de", input });
  rerender(<PermissionCard request={request} onAnswer={vi.fn()} />);
  await act(async () => finish(translated));
  expect(screen.queryByText("批准此方案吗？")).toBeNull();
  mocks.state!.value!.enabled = false;
  rerender(<PermissionCard request={request} onAnswer={vi.fn()} />);
  expect(screen.queryByRole("combobox")).toBeNull();
  expect(screen.getByText("Approve this plan?")).toBeTruthy();
});

it("offers an on-demand translation when automatic reply translation is off", async () => {
  mocks.state!.value!.preferences.replies = false;
  mocks.state!.value!.preferences.display = "original";
  render(<PermissionCard request={request} onAnswer={vi.fn()} />);
  expect(mocks.translate).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Translation" }));
  expect(await screen.findByText("批准此方案吗？")).toBeTruthy();
  expect(screen.getByText("Approve this plan?")).toBeTruthy();
  expect(mocks.translate).toHaveBeenCalledWith("fixture", "ask-1", true);
});

it("offers no translation button when translation is off in Settings", () => {
  mocks.state!.value!.preferences.replies = false;
  mocks.state!.settings!.config.enabled = false;
  render(<PermissionCard request={request} onAnswer={vi.fn()} />);
  expect(screen.queryByRole("button", { name: "Translation" })).toBeNull();
});

it("keeps the question usable when display translation fails and supports retry", async () => {
  mocks.translate.mockRejectedValueOnce("translation:network");
  render(<PermissionCard request={request} onAnswer={vi.fn()} />);
  expect(await screen.findByRole("alert")).toBeTruthy();
  fireEvent.click(screen.getByRole("radio", { name: /Additive/ }));
  fireEvent.click(screen.getByRole("button", { name: "Retry translation" }));
  expect(await screen.findByText("批准此方案吗？")).toBeTruthy();
  expect(screen.getByRole("radio", { name: /新增 zip 字段/ }).getAttribute("aria-checked")).toBe("true");
});

it("preserves dismiss semantics and does not submit while confirming an IME composition", () => {
  mocks.state = null;
  const answer = vi.fn(); render(<PermissionCard request={request} onAnswer={answer} />);
  fireEvent.change(screen.getByRole("textbox"), { target: { value: "自定义回答" } });
  fireEvent.keyDown(screen.getByRole("textbox"), { key: "Enter", isComposing: true });
  expect(answer).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
  expect(answer).toHaveBeenCalledWith(request, { allow: false, message: "Dismissed by user" });
  expect(mocks.translate).not.toHaveBeenCalled();
});

it("lets option text be selected without picking the option, and still picks from the keyboard", () => {
  mocks.state = null;
  render(<PermissionCard request={request} onAnswer={vi.fn()} />);
  const option = screen.getByRole("radio", { name: /Zip only/ });
  const range = document.createRange(); range.selectNodeContents(option);
  window.getSelection()!.addRange(range);
  fireEvent.click(option);
  expect(option.getAttribute("aria-checked")).toBe("false");
  window.getSelection()!.removeAllRanges();
  fireEvent.keyDown(option, { key: " " });
  expect(option.getAttribute("aria-checked")).toBe("true");
});
