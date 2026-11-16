import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import en from "../../i18n/locales/en";
import type { LaunchModelCatalog, LaunchOption } from "../../ipc/launch";
import type { AgentPreset, Group, Project, Session, SessionKind } from "../../types";
import { NewSessionView } from "./NewSessionView";

const fixture = vi.hoisted(() => ({
  prepare: vi.fn(),
  create: vi.fn(),
  models: vi.fn(),
  savePermissionDefault: vi.fn(),
  useAgentPermissions: vi.fn(),
  openSession: vi.fn(),
  closeTab: vi.fn(),
  revealSession: vi.fn(),
  loadTree: vi.fn(),
  setCollapse: vi.fn(),
  setChatModel: vi.fn(),
  setChatEffort: vi.fn(),
  state: {
    projects: [] as Project[],
    groups: [] as Group[],
    sessions: [] as Session[],
    agentDefaults: {} as Record<string, { args?: string; permissionMode?: string }>,
    chatModel: "",
    chatModelByKind: {} as Record<string, string>,
    chatEffortByModel: {} as Record<string, string>,
    runtimes: {} as Record<string, { status: string }>,
    pendingPrompts: {} as Record<string, string>,
  },
}));

vi.mock("../../ipc/launch", () => ({
  prepareAgentSession: fixture.prepare,
  createAgentSession: fixture.create,
  launchModels: fixture.models,
  launchErrorText: (cause: unknown) => (cause instanceof Error ? cause.message : String(cause)),
}));
vi.mock("../../ipc/tree", () => ({ setCollapsed: fixture.setCollapse }));
vi.mock("../../ipc/transport", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../ipc/transport")>(),
  uploadImage: vi.fn(),
}));
vi.mock("../../hooks/useGitBranch", () => ({ useGitBranchInfo: () => ({ isRepo: false, branch: null }) }));
vi.mock("../../components/useTaskImages", () => ({
  useTaskImages: () => ({ attachments: [], busy: false, note: null, onPaste: vi.fn(), remove: vi.fn() }),
}));
vi.mock("../../hooks/useAgentPermissions", () => ({
  useAgentPermissions: fixture.useAgentPermissions,
  savePermissionDefault: fixture.savePermissionDefault,
}));
vi.mock("../../i18n", () => ({
  useT: () => (key: keyof typeof en, ...args: unknown[]) => {
    const entry = en[key];
    return typeof entry === "function" ? (entry as (...a: unknown[]) => string)(...args) : String(entry ?? key);
  },
}));
vi.mock("../../store/termStore", () => ({
  useTermStore: Object.assign(
    (selector: (state: typeof fixture.state) => unknown) => selector(fixture.state),
    {
      getState: () => ({
        ...fixture.state,
        openSession: fixture.openSession,
        closeTab: fixture.closeTab,
        revealSession: fixture.revealSession,
        loadTree: fixture.loadTree,
        setChatModel: fixture.setChatModel,
        setChatEffort: fixture.setChatEffort,
        retargetNewSessionTab: vi.fn(),
        importProject: vi.fn(),
        setCreateProjectModalOpen: vi.fn(),
        setCloneModalOpen: vi.fn(),
      }),
      setState: (update: (state: typeof fixture.state) => object) => Object.assign(fixture.state, update(fixture.state)),
      subscribe: () => () => {},
    },
  ),
}));

const project = { id: "p1", name: "Workspace", rootPath: "/tmp/project" } as Project;

const options: LaunchOption[] = [
  { id: "antigravity", label: "Antigravity", acceptsTask: true, effortFlag: "--effort", effortLevels: ["low", "medium", "high"], supportsPlanExecute: false, supportsReferSummary: false },
  { id: "claude", label: "Claude", acceptsTask: true, effortFlag: "--effort", effortLevels: ["low", "medium", "high", "xhigh", "max"], supportsPlanExecute: true, supportsReferSummary: true },
  { id: "zcode", label: "ZCode", acceptsTask: true, effortFlag: null, effortLevels: [], supportsPlanExecute: false, supportsReferSummary: false },
  { id: "pi", label: "Pi", acceptsTask: true, effortFlag: "--thinking", effortLevels: ["off", "low", "medium", "high"], supportsPlanExecute: false, supportsReferSummary: false },
];

const presets: AgentPreset[] = [
  { id: "zcode-preset", name: "ZCode Custom", baseKind: "zcode", execPath: "/opt/zcode/bin/zcode", sortOrder: 0, createdAt: 1 },
];

const catalogs: Record<string, LaunchModelCatalog> = {
  antigravity: {
    models: [
      { id: "gemini-4-carbon-1m", label: "Gemini 4 Carbon (1M)", description: "Gemini 4 Carbon (1M) · Extended 1M context window", largeContext: true, effortLevels: ["low", "medium", "high"] },
      { id: "gemini-4-carbon", label: "Gemini 4 Carbon", description: "Gemini 4 Carbon · Flagship reasoning model", effortLevels: ["low", "medium", "high"] },
      { id: "gemini-3.8-flash", label: "Gemini 3.8 Flash", description: "Gemini 3.8 Flash · Fast non-thinking model", effortLevels: [] },
    ],
    effortLevels: ["low", "medium", "high"],
  },
  claude: {
    models: [
      { id: "claude-opus-4-7", label: "Claude Opus 4.7", description: "Claude Opus 4.7 · Deepest reasoning", effortLevels: ["low", "medium", "high", "xhigh", "max"] },
      { id: "claude-sonnet-4-6", label: "Claude Sonnet 4.6", description: "Claude Sonnet 4.6 · Fast daily driver", effortLevels: ["low", "medium", "high"] },
    ],
    effortLevels: ["low", "medium", "high", "xhigh", "max"],
  },
  zcode: {
    models: [
      { id: "glm-4.7", label: "GLM 4.7", description: "GLM 4.7 · ZCode default", effortLevels: ["low", "medium", "high"] },
    ],
    effortLevels: ["low", "medium", "high"],
  },
  pi: {
    models: [
      { id: "pi-fast", label: "Pi Fast", effortLevels: ["low", "high"] },
    ],
    effortLevels: ["low", "high"],
  },
};

const renderView = () =>
  render(<NewSessionView tab={{ id: "tab-1", projectId: "p1", groupId: null, parentSessionId: null }} hidden={false} />);

const selectCombobox = (name: string) => screen.getByRole("combobox", { name }) as HTMLButtonElement;

async function switchAgent(label: string) {
  fireEvent.click(selectCombobox("Agent"));
  const popup = await screen.findByRole("listbox");
  fireEvent.click(within(popup).getByRole("option", { name: new RegExp(label) }));
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  fixture.state = {
    projects: [project],
    groups: [],
    sessions: [],
    agentDefaults: {},
    chatModel: "",
    chatModelByKind: {},
    chatEffortByModel: {},
    runtimes: {},
    pendingPrompts: {},
  };
  fixture.prepare.mockResolvedValue({ options, presets, locationNames: ["Workspace"] });
  fixture.create.mockResolvedValue({ id: "s-new", projectId: "p1", name: "New", kind: "antigravity", engine: "chat" });
  fixture.models.mockImplementation(async (kind: string) => catalogs[kind] ?? { models: [], effortLevels: [] });
  fixture.useAgentPermissions.mockImplementation((kind: SessionKind, value?: string | null) => ({
    catalog: {
      modes: kind === "antigravity" ? ["default", "bypassPermissions"] : ["default", "acceptEdits", "plan", "bypassPermissions"],
      selected: value === "skip" ? "bypassPermissions" : value || "default",
    },
    error: null,
  }));
  fixture.savePermissionDefault.mockResolvedValue(undefined);
  fixture.setCollapse.mockResolvedValue(undefined);
  fixture.loadTree.mockResolvedValue(undefined);
});

afterEach(cleanup);

describe("NewSessionView aligned controls", () => {
  it("renders conversation-style model rows with human labels, 1M last, multi-word search, and custom ID entry", async () => {
    fixture.state.chatModelByKind = { antigravity: "gemini-4-carbon" };
    renderView();

    await waitFor(() => expect(fixture.models).toHaveBeenCalledWith("antigravity", { parentSessionId: null, cwd: "/tmp/project", inheritArgs: false }));
    const modelBtn = selectCombobox("Model");
    await waitFor(() => expect(modelBtn.textContent).toContain("Gemini 4 Carbon"));

    fireEvent.click(modelBtn);
    const popup = await screen.findByRole("listbox");
    const rows = within(popup).getAllByRole("option");
    expect(rows.map((r) => r.textContent)).toEqual([
      expect.stringContaining("Use Antigravity default"),
      expect.stringContaining("Gemini 4 Carbon"),
      expect.stringContaining("Gemini 3.8 Flash"),
      expect.stringContaining("Gemini 4 Carbon (1M)"),
    ]);
    // Description strips the repeated `<label> · ` prefix and tags the saved default.
    expect(rows[1].textContent).toContain("Flagship reasoning model");
    expect(rows[1].textContent).not.toContain("Gemini 4 Carbon · Flagship reasoning model");
    expect(rows[1].textContent).toContain("App default");

    // Multi-word search spans label, description, and model ID.
    const filter = within(popup).getByPlaceholderText("Filter");
    fireEvent.change(filter, { target: { value: "flash fast" } });
    expect(within(popup).getAllByRole("option")).toHaveLength(1);
    expect(within(popup).getAllByRole("option")[0].textContent).toContain("Gemini 3.8 Flash");

    // Typing a spaceless uncatalogued model ID offers custom entry.
    fireEvent.change(filter, { target: { value: "gemini-custom-preview" } });
    const customRow = within(popup).getByRole("option", { name: /Use “gemini-custom-preview”/ });
    fireEvent.click(customRow);
    expect(modelBtn.textContent).toContain("gemini-custom-preview");
  });

  it("aligns effort options per agent/model, disables effort on non-thinking models, and shows Claude Off/Ultra Code", async () => {
    renderView();
    await waitFor(() => expect(fixture.models).toHaveBeenCalledWith("antigravity", expect.anything()));

    // Antigravity non-thinking model disables Reasoning effort and resets incompatible choice.
    fireEvent.click(selectCombobox("Model"));
    fireEvent.click(within(await screen.findByRole("listbox")).getByRole("option", { name: /Gemini 4 Carbon(?! \(1M\))/ }));
    fireEvent.click(selectCombobox("Reasoning effort"));
    fireEvent.click(within(await screen.findByRole("listbox")).getByRole("option", { name: "High" }));
    expect(selectCombobox("Reasoning effort").textContent).toContain("High");

    fireEvent.click(selectCombobox("Model"));
    fireEvent.click(within(await screen.findByRole("listbox")).getByRole("option", { name: /Gemini 3\.8 Flash/ }));
    const effortBtn = selectCombobox("Reasoning effort");
    expect(effortBtn.disabled).toBe(true);
    expect(effortBtn.textContent).toContain("Auto");

    // Switching to Claude exposes Off ("No extended thinking") and Ultra Code when xhigh exists.
    await switchAgent("Claude");
    await waitFor(() => expect(fixture.models).toHaveBeenCalledWith("claude", expect.anything()));
    fireEvent.click(selectCombobox("Model"));
    fireEvent.click(within(await screen.findByRole("listbox")).getByRole("option", { name: /Claude Opus 4\.7/ }));
    fireEvent.click(selectCombobox("Reasoning effort"));
    const claudeEfforts = await screen.findByRole("listbox");
    expect(within(claudeEfforts).getByRole("option", { name: /Off/ }).textContent).toContain("No extended thinking");
    expect(within(claudeEfforts).getByRole("option", { name: "Ultra Code" })).toBeTruthy();
  });

  it("prefills remembered model/effort, updates effort on model change, saves via Set as default, and sends empty string to clear", async () => {
    fixture.state.chatModelByKind = { antigravity: "gemini-4-carbon" };
    fixture.state.chatEffortByModel = {
      "antigravity:gemini-4-carbon": "high",
      "antigravity:gemini-4-carbon-1m": "low",
    };
    renderView();
    await waitFor(() => expect(fixture.models).toHaveBeenCalledWith("antigravity", expect.anything()));

    expect(selectCombobox("Model").textContent).toContain("Gemini 4 Carbon");
    expect(selectCombobox("Reasoning effort").textContent).toContain("High");

    // Switching models pulls that model's remembered effort and persists when "Set as default" is checked.
    fireEvent.click(selectCombobox("Model"));
    const modelPopup = await screen.findByRole("listbox");
    fireEvent.click(within(modelPopup).getByRole("checkbox", { name: "Set as default" }));
    fireEvent.click(within(modelPopup).getByRole("option", { name: /Gemini 4 Carbon \(1M\)/ }));
    expect(selectCombobox("Reasoning effort").textContent).toContain("Low");
    expect(fixture.setChatModel).toHaveBeenCalledWith("antigravity", "gemini-4-carbon-1m");
    expect(fixture.setChatEffort).toHaveBeenCalledWith("antigravity:gemini-4-carbon-1m", "low");

    // Picking "Use Antigravity default" clears both in the launch request (`""`).
    fireEvent.click(selectCombobox("Model"));
    fireEvent.click(within(await screen.findByRole("listbox")).getByRole("option", { name: /Use Antigravity default/ }));
    fireEvent.click(screen.getByRole("button", { name: "Start" }));
    await waitFor(() => expect(fixture.create).toHaveBeenCalledOnce());
    expect(fixture.create.mock.calls[0][0]).toMatchObject({
      kind: "antigravity",
      model: "",
      effort: "",
    });
  });

  it("shows Model and Reasoning effort for ZCode and forwards preset execPath to model discovery", async () => {
    renderView();
    await waitFor(() => expect(fixture.prepare).toHaveBeenCalled());
    await switchAgent("ZCode Custom");

    await waitFor(() => expect(fixture.models).toHaveBeenCalledWith("zcode", {
      parentSessionId: null,
      cwd: "/tmp/project",
      execPath: "/opt/zcode/bin/zcode",
      inheritArgs: false,
    }));
    expect(selectCombobox("Model")).toBeTruthy();
    await waitFor(() => expect(selectCombobox("Reasoning effort").disabled).toBe(false));
  });

  it("hides Permission for Pi and renders Agent Default + full-width Set as default for Antigravity", async () => {
    renderView();
    await waitFor(() => expect(fixture.models).toHaveBeenCalledWith("antigravity", expect.anything()));

    const permBtn = selectCombobox("Permission");
    expect(permBtn.textContent).toContain("Agent Default");
    fireEvent.click(permBtn);
    const permPopup = await screen.findByRole("listbox");
    fireEvent.click(within(permPopup).getByRole("checkbox", { name: "Set as default" }));
    fireEvent.click(within(permPopup).getByRole("option", { name: /Bypass/ }));
    expect(fixture.savePermissionDefault).toHaveBeenCalledWith("antigravity", "bypassPermissions");

    await switchAgent("Pi");
    expect(screen.queryByRole("combobox", { name: "Permission" })).toBeNull();
  });
});
