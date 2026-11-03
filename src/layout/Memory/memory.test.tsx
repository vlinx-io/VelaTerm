// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { setLang } from "../../i18n";
import { MemoryLibrary } from "./MemoryLibrary";
import { MemoryDirectory } from "./MemoryDirectory";
import { MemoryEditor } from "./MemoryDocument";
import { MemoryCompile } from "./MemoryTasks";
import { MemoryTab } from "./MemoryTab";
import { MemoryRoute } from "./MemoryRoute";
import { MemoryMarkdown } from "./shared";
import { memoryNavigate, memoryUrl } from "./navigation";

const api = vi.hoisted(() => ({ list: vi.fn(), collections: vi.fn(), get: vi.fn(), options: vi.fn(), save: vi.fn(), start: vi.fn(), models: vi.fn() }));
const launchSpec = (id: string, label: string) => ({ id, label, acceptsTask: true, effortFlag: "--effort", effortLevels: ["low", "high"], supportsPlanExecute: false, supportsReferSummary: false });
vi.mock("../../ipc/launch", () => ({
  launchOptions: async () => [launchSpec("claude", "Claude"), launchSpec("codex", "Codex")],
  launchModels: api.models,
}));
vi.mock("../../ipc/memory", () => ({
  memoryList: api.list, memoryCollections: api.collections,
  memoryGet: api.get, memoryOptions: api.options, memorySave: api.save, memoryStart: api.start,
  memoryDelete: vi.fn(), memoryRestore: vi.fn(), memorySource: vi.fn(), memoryJobs: vi.fn(), memoryCancel: vi.fn(), memoryRetry: vi.fn(),
}));
const store = vi.hoisted(() => ({
  memoryPrefs: {} as { agent?: string; model?: string; effort?: string },
  setMemoryPrefs: vi.fn(),
}));
vi.mock("../../store/termStore", () => ({
  useTermStore: Object.assign(
    (select: (s: unknown) => unknown) => select({ sessions: [{ id: "session", name: "Source conversation" }], archivedSessions: [] }),
    { getState: () => store },
  ),
}));

beforeEach(() => {
  setLang("en"); window.history.replaceState(null, "", "/?memory=library");
  store.memoryPrefs = {}; store.setMemoryPrefs.mockReset();
  api.list.mockReset().mockResolvedValue({ projects: [], selectedSessionId: null, entries: [], tags: [], total: 0, pageSize: 40 });
  api.get.mockReset(); api.options.mockReset(); api.save.mockReset(); api.start.mockReset(); api.models.mockReset();
  api.models.mockResolvedValue({ models: [{ id: "chosen-model", label: "Chosen model", effortLevels: ["low", "high"] }, { id: "simple-model", label: "Simple model", effortLevels: [] }], effortLevels: ["low", "high"] });
  api.options.mockResolvedValue({ agents: [{ id: "claude", label: "Claude", available: true }, { id: "codex", label: "Codex", available: true }], defaultAgent: "codex", catalog: [] });
});
afterEach(cleanup);

/** Commit one option on a shared `Select`, a combobox button plus a popup list. jsdom runs the
 *  wrapping label's activation behavior after the row click even though the list stops propagation,
 *  which reopens the popup; browsers do not, so the stray popup is toggled shut here. */
function choose(label: string, option: string) {
  const trigger = screen.getByRole("combobox", { name: label });
  fireEvent.click(trigger);
  fireEvent.click(screen.getByRole("option", { name: option }));
  if (screen.queryAllByRole("listbox").length) fireEvent.click(trigger);
}

/** Commit one catalogue entry on the shared launch fields, whose inputs list the model identifiers. */
async function pick(label: string, option: RegExp | string) {
  fireEvent.click(screen.getByRole("combobox", { name: label }));
  fireEvent.click(await screen.findByRole("option", { name: option }));
}
const field = (label: string) => screen.getByRole("combobox", { name: label }) as HTMLInputElement;

describe("Knowledge Base interactions", () => {
  it.each(["tauri://localhost", "tauri://localhost/"])("builds an explicit close URL for %s", (base) => {
    vi.stubGlobal("window", { location: { href: `${base}?memory=library&memoryQuery=test` } });
    try {
      expect(memoryUrl("")).toBe(base);
      expect(new URL(memoryUrl("")).search).toBe("");
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("closes the library tab and clears its route", async () => {
    window.history.replaceState(null, "", "/?memory=library&memoryQuery=test&session=keep#terminal");
    render(<><MemoryTab /><MemoryRoute /></>);
    expect(screen.getByRole("tabpanel").querySelector(".memory-close")).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("tabpanel")).toBeNull());
    expect(document.querySelector(".memory-open-tab")).toBeNull();
    expect(location.search).toBe("?session=keep");
    expect(location.hash).toBe("#terminal");
  });

  it("uses the backend agent default and submits the user's selected model", async () => {
    api.start.mockResolvedValue({ id: "job-1", reused: false });
    render(<MemoryCompile sessionId="session" />);
    const agents = await screen.findByRole("combobox", { name: "Agent" });
    await waitFor(() => expect(agents.textContent).toContain("Codex"));
    choose("Agent", "Claude");
    await screen.findByRole("combobox", { name: "Model" });
    await pick("Model", /chosen-model/);
    await pick("Reasoning effort", "high");
    fireEvent.click(screen.getByRole("button", { name: "Organize and save" }));
    await waitFor(() => expect(api.start).toHaveBeenCalledWith("session", "claude", "chosen-model", "high"));
    await waitFor(() => expect(new URLSearchParams(location.search).get("memory")).toBe("job/job-1"));
  });

  it("clears an effort the new model lacks and clears both overrides when changing agents", async () => {
    render(<MemoryCompile sessionId="session" />);
    await screen.findByRole("combobox", { name: "Model" });
    await pick("Model", /chosen-model/);
    await pick("Reasoning effort", "high");
    await pick("Model", /simple-model/);
    expect(field("Model").value).toBe("simple-model");
    expect(field("Reasoning effort").value).toBe("");
    await pick("Model", /chosen-model/);
    await pick("Reasoning effort", "low");
    choose("Agent", "Claude");
    await waitFor(() => expect(field("Model").value).toBe(""));
    expect(field("Reasoning effort").value).toBe("");
  });

  it("restores the remembered agent, model and effort on reopen", async () => {
    store.memoryPrefs = { agent: "claude", model: "chosen-model", effort: "high" };
    render(<MemoryCompile sessionId="session" />);
    const agents = await screen.findByRole("combobox", { name: "Agent" });
    await waitFor(() => expect(agents.textContent).toContain("Claude"));
    await waitFor(() => expect(field("Model").value).toBe("chosen-model"));
    expect(field("Reasoning effort").value).toBe("high");
  });

  it("remembers each selection for the next visit", async () => {
    render(<MemoryCompile sessionId="session" />);
    await screen.findByRole("combobox", { name: "Model" });
    choose("Agent", "Claude");
    expect(store.setMemoryPrefs).toHaveBeenCalledWith({ agent: "claude", model: null, effort: null });
    await screen.findByRole("combobox", { name: "Model" });
    await pick("Model", /chosen-model/);
    expect(store.setMemoryPrefs).toHaveBeenCalledWith({ model: "chosen-model" });
    await pick("Reasoning effort", "high");
    expect(store.setMemoryPrefs).toHaveBeenCalledWith({ effort: "high" });
  });

  it("falls back to the backend default when the remembered agent is no longer available", async () => {
    store.memoryPrefs = { agent: "claude", model: "chosen-model", effort: "high" };
    api.options.mockResolvedValue({ agents: [{ id: "claude", label: "Claude", available: false }, { id: "codex", label: "Codex", available: true }], defaultAgent: "codex", catalog: [] });
    render(<MemoryCompile sessionId="session" />);
    const agents = await screen.findByRole("combobox", { name: "Agent" });
    await waitFor(() => expect(agents.textContent).toContain("Codex"));
    await screen.findByRole("combobox", { name: "Model" });
    expect(field("Model").value).toBe("");
  });

  it("keeps the agent default usable when the model list fails and retries it", async () => {
    api.models.mockRejectedValueOnce(new Error("Provider catalogue unavailable"));
    render(<MemoryCompile sessionId="session" />);
    await screen.findByText("Provider catalogue unavailable");
    // The shared fields accept a typed identifier or the agent default, so a missing list does not block.
    expect((screen.getByRole("button", { name: "Organize and save" }) as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    fireEvent.click(screen.getByRole("combobox", { name: "Model" }));
    await screen.findByRole("option", { name: /chosen-model/ });
    expect(api.start).not.toHaveBeenCalled();
  });

  it("submits edits with the backend version and preserves the form after a conflict", async () => {
    api.get.mockResolvedValue({ entry: { id: "entry", version: 7, title: "Original", summary: "Summary", content: "Body", tags: ["Tag"], related: [], sources: [] }, catalog: [] });
    api.save.mockRejectedValue(new Error("memory_conflict"));
    render(<MemoryEditor id="entry" />);
    fireEvent.change(await screen.findByLabelText("Title"), { target: { value: "Edited" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(api.save).toHaveBeenCalledWith({ id: "entry", version: 7, title: "Edited", summary: "Summary", content: "Body", tags: ["Tag"], related: [] }));
    expect(await screen.findByRole("alert")).toHaveProperty("textContent", expect.stringContaining("changed during"));
    expect((screen.getByLabelText("Title") as HTMLInputElement).value).toBe("Edited");
  });

  it("requires an explicit discard before leaving an edited form", async () => {
    window.history.replaceState(null, "", "/?memory=new");
    render(<MemoryEditor />);
    fireEvent.change(await screen.findByLabelText("Title"), { target: { value: "Unsaved" } });
    fireEvent.click(screen.getByRole("link", { name: "Cancel" }));
    expect(await screen.findByRole("alertdialog")).toHaveProperty("textContent", expect.stringContaining("Discard unsaved changes?"));
    expect(new URLSearchParams(location.search).get("memory")).toBe("new");
    fireEvent.click(screen.getByRole("button", { name: "OK" }));
    expect(new URLSearchParams(location.search).get("memory")).toBe("library");
  });

  it("renders Markdown without executable HTML or remote images", () => {
    const { container } = render(<MemoryMarkdown content={'# Knowledge\n\n**Safe**\n\n<script>alert(1)</script><img src="https://example.com/tracker" onerror="alert(1)"><a href="javascript:alert(1)">unsafe</a>'} />);
    expect(container.querySelector("h1")?.textContent).toBe("Knowledge");
    expect(container.querySelector("strong")?.textContent).toBe("Safe");
    expect(container.querySelector("script,img")).toBeNull();
    expect(container.querySelector("a")?.getAttribute("href")).toBeNull();
  });

  it("uses stable URLs for library filters and source routes", () => {
    const href = memoryUrl("source/source-id", { memoryQuery: "中文搜索", memoryTag: "技术", memoryPage: 2 });
    memoryNavigate(href);
    const params = new URLSearchParams(location.search);
    expect(params.get("memory")).toBe("source/source-id");expect(params.get("memoryQuery")).toBe("中文搜索");
    expect(params.get("memoryTag")).toBe("技术");expect(params.get("memoryPage")).toBe("2");
  });
});

it("navigates project and session groups through shareable URLs and loads scoped entries", async () => {
  api.list.mockImplementation(async (args) => ({
    projects: [{ id: "project", name: "Project A", kind: "project", count: 1, sessions: [{ id: "session", name: "Session A", kind: "session", count: 1 }] }],
    selectedSessionId: args.sessionId ?? null, total: 1, pageSize: 40, tags: [],
    entries: [{ id: "entry", title: "Saved topic", summary: "Snapshot", tags: [], updatedAt: 0, sourceCount: 1 }],
  }));
  render(<><MemoryLibrary selected="" /><aside className="col-right"><MemoryDirectory /></aside></>);
  await screen.findByRole("link",{name:/Project A/});
  const directory = document.querySelector(".memory-directory") as HTMLElement;
  const project = await within(directory).findByRole("link", { name: /Project A/ });
  expect(project.getAttribute("href")).toContain("memoryProject=project");
  expect(within(directory).queryByText("Saved topic")).toBeNull();
  expect(screen.getByRole("main").contains(screen.getByLabelText("Search entry titles and content…"))).toBe(true);
  fireEvent.click(project);
  fireEvent.click(await within(directory).findByRole("link", { name: /Session A/ }));
  await within(screen.getByRole("main")).findByText("Saved topic");
  expect(location.search).toContain("memorySession=session");
  expect(api.list).toHaveBeenCalledWith(expect.objectContaining({ sessionId: "session", projectId: "project" }));
  expect(within(directory).getByRole("link", { name: /Saved topic/ }).getAttribute("href")).toContain("memory=entry%2Fentry");
  expect(within(directory).queryByText("Snapshot")).toBeNull();
  expect(within(screen.getByRole("main")).getByText("Saved topic")).toBeTruthy();
  choose("Recently updated", "Title");
  await waitFor(() => expect(new URLSearchParams(location.search).get("memorySort")).toBe("title"));
  expect(new URLSearchParams(location.search).get("memorySession")).toBe("session");
});

it("shows related entries on a search result and links to them", async () => {
  api.list.mockResolvedValue({
    projects: [], selectedSessionId: null, total: 1, pageSize: 40, tags: [],
    entries: [{ id: "entry", title: "Saved topic", summary: "Snapshot", tags: [], updatedAt: 0, sourceCount: 1, related: [{ id: "neighbor", title: "Neighbor topic" }] }],
  });
  render(<MemoryLibrary selected="" />);
  const related = await screen.findByRole("link", { name: "Neighbor topic" });
  expect(related.getAttribute("href")).toContain("memory=entry%2Fneighbor");
  expect(screen.getByText("Related entries")).toBeTruthy();
});

it("keeps failure feedback and retry in the main panel", async () => {
  api.list.mockRejectedValueOnce(new Error("memory_invalid"));
  render(<MemoryLibrary selected="" />);
  const alert = await screen.findByRole("alert");
  expect(screen.getByRole("main").contains(alert)).toBe(true);
  expect(document.querySelector(".memory-workspace .memory-directory")).toBeNull();
  fireEvent.click(within(alert).getByRole("button", { name: "Retry" }));
  await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  expect(screen.getByLabelText("Search entry titles and content…")).toBeTruthy();
});

it("keeps later entries reachable and locates a selected entry beyond the first tree page", async () => {
  window.history.replaceState(null,"","/?memory=entry/late&memoryProject=p&memorySession=s");
  api.list.mockImplementation(async args => ({
    projects:[{id:"p",name:"Project",kind:"project",count:3,sessions:[{id:"s",name:"Session",kind:"session",count:3}]}],
    selectedSessionId:args.selectedId?"s":args.sessionId??null,total:3,pageSize:2,tags:[],
    entries:args.page===1?[{id:"late",title:"Late entry"}]:[{id:"first",title:"First entry"},{id:"second",title:"Second entry"}],
  }));
  api.get.mockResolvedValue({entry:{id:"late",title:"Late entry"}});
  render(<MemoryDirectory/>);
  const late=await screen.findByRole("link",{name:"Late entry"});
  expect(late.getAttribute("aria-current")).toBe("page");
  expect(late.closest('li[data-depth="2"]')?.querySelector('.nb-tree-row>a')?.textContent).toContain("Session");
  fireEvent.click(screen.getByRole("button",{name:"Load more"}));
  await waitFor(()=>expect(api.list).toHaveBeenCalledWith(expect.objectContaining({projectId:"p",sessionId:"s",page:1})));
  await waitFor(()=>expect(screen.queryByRole("button",{name:"Load more"})).toBeNull());
  expect(screen.getAllByRole("link",{name:"Late entry"})).toHaveLength(1);
});
