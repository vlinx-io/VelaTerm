// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { setLang, t } from "../../i18n";
import { assistClaim, assistGet, assistMark, assistPending, assistRun, assistRunAll, assistSkipAll, type AssistAction, type AssistPlan } from "../../ipc/assist";
import { takeCloneUrl } from "../../remote/dialogNavigation";
import { useTermStore } from "../../store/termStore";
import type { Session } from "../../types";
import { AssistRequestListener, AssistRequests, cardGroups } from "./AssistRequests";
import { describeAction, performAction } from "./actions";

vi.mock("../../ipc/assist", () => ({ assistClaim: vi.fn(), assistGet: vi.fn(), assistMark: vi.fn(), assistRun: vi.fn(), assistRunAll: vi.fn(), assistSkipAll: vi.fn(), assistPending: vi.fn() }));
const requests: ((request: { id: string; sessionId: string; noConfirm?: boolean }) => void)[] = [];
vi.mock("../../ipc/events", async importOriginal => ({
  ...await importOriginal<typeof import("../../ipc/events")>(),
  onAssistRequest: (cb: (request: { id: string; sessionId: string; noConfirm?: boolean }) => void) => { requests.push(cb); return Promise.resolve(() => {}); },
  onAssistChanged: () => Promise.resolve(() => {}),
}));

const action = (fields: Partial<AssistAction> & { domain: string; kind: string }): AssistAction => ({ status: "pending", backend: false, ...fields });
const source = { sessionId: "s1", sessionName: "Planner", agent: "codex" };
const plan = (actions: AssistAction[], extra: Partial<AssistPlan> = {}): AssistPlan => ({ id: "plan-1", createdAt: 0, source, summary: "Open the terminal settings", actions, ...extra });

beforeEach(() => {
  setLang("en");
  for (const mock of [assistClaim, assistGet, assistRun, assistRunAll, assistSkipAll, assistMark, assistPending]) vi.mocked(mock).mockReset();
  window.history.replaceState(null, "", "/?session=keep");
});
afterEach(() => cleanup());

describe("action cards", () => {
  /** Every kind the host can return, with the fields its validator produces. */
  const kinds: AssistAction[] = [
    action({ domain: "projects", kind: "importProject", candidates: ["/p/a"] }), action({ domain: "projects", kind: "importProject", candidates: ["/p/a", "/q/a"], collectionId: "c1", collectionName: "Clients" }),
    action({ domain: "projects", kind: "createProject", name: "a" }), action({ domain: "projects", kind: "cloneProject", url: "https://x/y.git" }),
    action({ domain: "projects", kind: "createCollection", name: "c", parentId: "c1", parentName: "Clients" }), action({ domain: "projects", kind: "moveToCollection", projectName: "p", collectionId: null }),
    action({ domain: "projects", kind: "createGroup", name: "g", where: "p" }), action({ domain: "projects", kind: "newSession", sessionKind: "codex", projectName: "p" }),
    action({ domain: "projects", kind: "openSession", sessionName: "s", where: "p / s" }), action({ domain: "projects", kind: "revealProject", projectName: "p" }),
    action({ domain: "projects", kind: "rename", currentName: "a", name: "b", where: "p / a" }), action({ domain: "projects", kind: "move", nodeName: "s", to: "p / g" }),
    action({ domain: "projects", kind: "sort", by: "name", where: "p / g" }), action({ domain: "projects", kind: "sort", by: "order", where: "p" }),
    action({ domain: "projects", kind: "archiveSession", nodeName: "s", where: "p / s" }), action({ domain: "projects", kind: "deleteNode", nodeName: "a", where: "p / a" }),
    action({ domain: "files", kind: "openFile", path: "/f/a" }), action({ domain: "files", kind: "reveal", path: "/f/a" }),
    ...["openVault", "search", "openNote", "createNote", "createFolder", "moveNote", "deleteNote", "favorite", "importFiles", "registerVault", "createVault", "openCollections"]
      .map(kind => action({ domain: "knowledge", kind, vaultName: "Notes", path: "a.md", query: "x", favorite: true })),
    action({ domain: "settings", kind: "openSettings", page: "terminal" }), action({ domain: "settings", kind: "set", setting: "theme", value: "dark" }),
    action({ domain: "settings", kind: "setFontSize", target: "interface", size: 14 }), action({ domain: "settings", kind: "setFontFamily", target: "terminal", family: "" }),
  ];

  it.each(kinds.map(a => [`${a.domain}.${a.kind}`, a] as const))("%s has a written title", (_, value) => {
    const card = describeAction(value, t);
    expect(card.title).not.toBe(`${value.domain} · ${value.kind}`);
    expect(card.title).not.toContain("undefined");
    card.details.forEach(line => expect(line).not.toContain("undefined"));
  });

  it("shows validated fields rather than the agent's wording", () => {
    const card = describeAction(action({ domain: "settings", kind: "set", setting: "theme", value: "dark", summary: "Delete everything" }), t);
    expect(card.title).toBe("Set the theme to Dark");
    expect(describeAction(action({ domain: "settings", kind: "openSettings", page: "agents" }), t).details).toEqual([t("settings.catAgents")]);
    expect(describeAction(action({ domain: "projects", kind: "deleteNode", nodeName: "Work" }), t).danger).toBe(true);
    expect(describeAction(action({ domain: "projects", kind: "importProject", candidates: ["/p/shop"] }), t).title).toBe(t("assist.projects.importFolder", "/p/shop"));
  });

  it("names the Projects collection when a project moves out of its collection", () => {
    expect(describeAction(action({ domain: "projects", kind: "moveToCollection", projectName: "Shop", collectionId: null }), t).title)
      .toBe(t("assist.projects.moveToCollection", "Shop", t("systemCollection.projects")));
  });

  it("keeps a cloned repository's address out of the URL", async () => {
    await performAction(action({ domain: "projects", kind: "cloneProject", url: "https://user:secret@x/y.git", folderName: "y" }));
    expect(location.href).not.toContain("secret");
    expect(new URLSearchParams(location.search).get("projectDialog")).toBe("clone");
    expect(new URLSearchParams(location.search).get("projectName")).toBe("y");
    expect(takeCloneUrl()).toBe("https://user:secret@x/y.git");
    expect(takeCloneUrl()).toBe("");
  });
});

describe("card groups", () => {
  it("gathers consecutive host actions and keeps window actions and choices on their own cards", () => {
    const host = (fields: Partial<AssistAction> = {}) => action({ domain: "projects", kind: "rename", backend: true, ...fields });
    const window = action({ domain: "settings", kind: "openSettings" });
    expect(cardGroups(plan([host(), host(), window, host(), host({ status: "done" }), host()]))).toEqual([[0, 1], [2], [3, 5]]);
    expect(cardGroups(plan([host(), host({ kind: "importProject", candidates: ["/a", "/b"] }), host()]))).toEqual([[0], [1], [2]]);
    expect(cardGroups(plan([host({ kind: "importProject", candidates: ["/a"] }), host()]))).toEqual([[0, 1]]);
  });
});

describe("requests in the session view", () => {
  it("runs host actions with the chosen folder and keeps other page state", async () => {
    const pending = plan([action({ domain: "projects", kind: "importProject", backend: true, candidates: ["/p/a", "/p/b"] })]);
    vi.mocked(assistPending).mockResolvedValue([pending]);
    vi.mocked(assistRun).mockResolvedValue({ ...pending, actions: [{ ...pending.actions[0], status: "done" }] });
    render(<AssistRequests sessionId="s1" className="lane" />);
    await screen.findByText("Open the terminal settings");
    fireEvent.click(screen.getByRole("combobox", { name: t("assist.chooseFolder") }));
    fireEvent.click(await screen.findByRole("option", { name: "/p/b" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
    // Once every action is decided the request leaves the lane; the agent reports the outcome itself.
    await waitFor(() => expect(document.querySelector(".lane")).toBeNull());
    expect(assistRun).toHaveBeenCalledWith("plan-1", 0, "/p/b");
    expect(assistPending).toHaveBeenCalledWith("s1");
    expect(new URLSearchParams(location.search).get("session")).toBe("keep");
  });

  it("performs window actions and records them", async () => {
    const pending = plan([action({ domain: "settings", kind: "openSettings", page: "terminal" })]);
    vi.mocked(assistPending).mockResolvedValue([pending]);
    vi.mocked(assistMark).mockResolvedValue({ ...pending, actions: [{ ...pending.actions[0], status: "done" }] });
    render(<AssistRequests sessionId="s1" className="lane" />);
    fireEvent.click(await screen.findByRole("button", { name: "Confirm" }));
    await waitFor(() => expect(assistMark).toHaveBeenCalledWith("plan-1", 0, "done"));
    expect(new URLSearchParams(location.search).get("settings")).toBe("terminal");
  });

  it("lets the user skip a host action without running it", async () => {
    const pending = plan([action({ domain: "projects", kind: "deleteNode", backend: true, nodeName: "Work" })]);
    vi.mocked(assistPending).mockResolvedValue([pending]);
    vi.mocked(assistMark).mockResolvedValue({ ...pending, actions: [{ ...pending.actions[0], status: "dismissed" }] });
    render(<AssistRequests sessionId="s1" className="lane" />);
    fireEvent.click(await screen.findByRole("button", { name: t("assist.skip") }));
    await waitFor(() => expect(assistMark).toHaveBeenCalledWith("plan-1", 0, "dismissed"));
    expect(assistRun).not.toHaveBeenCalled();
  });

  it("keeps decided actions out of view and lists failed ones by name", async () => {
    vi.mocked(assistPending).mockResolvedValue([plan([
      action({ domain: "settings", kind: "openSettings", page: "terminal", status: "dismissed" }),
      action({ domain: "projects", kind: "rename", backend: true, status: "failed", error: "No such item", currentName: "a", name: "b" }),
      action({ domain: "settings", kind: "openSettings", page: "agents" }),
    ])]);
    render(<AssistRequests sessionId="s1" className="lane" />);
    await screen.findByText(t("assist.failed", 1));
    screen.getByText(t("assist.projects.rename", "a", "b"));
    // The host's message goes to the agent, not into the view.
    expect(screen.queryByText("No such item")).toBeNull();
    expect(document.querySelectorAll(".assist-card")).toHaveLength(1);
    screen.getByText(t("assist.confirmHint"));
  });

  it("confirms consecutive host changes together, in order", async () => {
    const changes = [
      action({ domain: "projects", kind: "createGroup", backend: true, name: "Lessons", where: "Course", ref: "$lessons" }),
      action({ domain: "projects", kind: "move", backend: true, nodeName: "Lesson 2", to: "Course / Lessons", groupId: "$lessons" }),
      action({ domain: "projects", kind: "sort", backend: true, by: "name", where: "Course / Lessons", groupId: "$lessons" }),
    ];
    const pending = plan(changes);
    vi.mocked(assistPending).mockResolvedValue([pending]);
    vi.mocked(assistRunAll).mockResolvedValue({ ...pending, actions: changes.map(a => ({ ...a, status: "done" as const })) });
    render(<AssistRequests sessionId="s1" className="lane" />);
    await screen.findByText(t("assist.batchTitle", 3));
    screen.getByText(t("assist.projects.move", "Lesson 2", "Course / Lessons"));
    // References inside the same card are resolved as it runs, so nothing waits.
    expect(screen.queryByText(t("assist.waitsForEarlier"))).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: t("assist.applyAll", 3) }));
    await waitFor(() => expect(document.querySelector(".lane")).toBeNull());
    expect(assistRunAll).toHaveBeenCalledWith("plan-1", [0, 1, 2]);
  });

  it("skips a whole card, and holds back changes that need an unconfirmed earlier one", async () => {
    const pending = plan([
      action({ domain: "projects", kind: "importProject", backend: true, candidates: ["/p/a", "/p/b"], ref: "$shop" }),
      action({ domain: "projects", kind: "createGroup", backend: true, name: "Docs", where: "shop", projectId: "$shop" }),
      action({ domain: "projects", kind: "rename", backend: true, currentName: "a", name: "b" }),
    ]);
    vi.mocked(assistPending).mockResolvedValue([pending]);
    vi.mocked(assistSkipAll).mockResolvedValue({ ...pending, actions: pending.actions.map(a => ({ ...a, status: "dismissed" as const })) });
    render(<AssistRequests sessionId="s1" className="lane" />);
    await screen.findByText(t("assist.waitsForEarlier"));
    expect((screen.getByRole("button", { name: t("assist.applyAll", 2) }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: t("assist.skipAll") }));
    await waitFor(() => expect(assistSkipAll).toHaveBeenCalledWith("plan-1", [1, 2]));
  });

  it("shows nothing without waiting requests, and reads again when the agent submits more", async () => {
    vi.mocked(assistPending).mockResolvedValue([]);
    const openSession = vi.fn();
    useTermStore.setState({ sessions: [{ id: "s1" } as Session], activeSessionId: null, openSession });
    render(<><AssistRequestListener /><AssistRequests sessionId="s1" className="lane" /></>);
    await waitFor(() => expect(assistPending).toHaveBeenCalledTimes(1));
    expect(document.querySelector(".lane")).toBeNull();
    vi.mocked(assistPending).mockResolvedValue([plan([action({ domain: "settings", kind: "openSettings", page: "terminal" })])]);
    await waitFor(() => expect(requests.length).toBeGreaterThan(0));
    act(() => requests[requests.length - 1]({ id: "plan-1", sessionId: "s1" }));
    await screen.findByRole("button", { name: "Confirm" });
    // The session the request came from comes into view.
    expect(openSession).toHaveBeenCalledWith("s1");
  });

  it("claims and executes window actions for noConfirm requests without stealing session focus", async () => {
    const autoPlan = plan([action({ domain: "settings", kind: "openSettings", page: "terminal" })], { noConfirm: true });
    const runningPlan = { ...autoPlan, actions: [{ ...autoPlan.actions[0], status: "running" as const }] };
    const donePlan = { ...autoPlan, actions: [{ ...autoPlan.actions[0], status: "done" as const }] };
    vi.mocked(assistPending).mockResolvedValue([]);
    vi.mocked(assistGet).mockResolvedValue(autoPlan);
    vi.mocked(assistClaim).mockResolvedValue({ claimed: true, plan: runningPlan });
    vi.mocked(assistMark).mockResolvedValue(donePlan);
    const openSession = vi.fn();
    useTermStore.setState({ sessions: [{ id: "s1" } as Session], activeSessionId: "other", openSession });
    render(<AssistRequestListener />);
    await waitFor(() => expect(requests.length).toBeGreaterThan(0));
    act(() => requests[requests.length - 1]({ id: "plan-1", sessionId: "s1", noConfirm: true }));
    await waitFor(() => expect(assistMark).toHaveBeenCalledWith("plan-1", 0, "done"));
    expect(assistClaim).toHaveBeenCalledWith("plan-1", 0);
    expect(new URLSearchParams(location.search).get("settings")).toBe("terminal");
    expect(openSession).not.toHaveBeenCalled();
  });

  it("does not run a noConfirm window action when another client already claimed it, and focuses when a folder choice remains", async () => {
    const mixed = plan([
      action({ domain: "projects", kind: "importProject", backend: true, candidates: ["/p/a", "/p/b"] }),
      action({ domain: "settings", kind: "openSettings", page: "terminal" }),
    ], { noConfirm: true });
    vi.mocked(assistGet).mockResolvedValue(mixed);
    vi.mocked(assistClaim).mockResolvedValue({ claimed: false, plan: { ...mixed, actions: [mixed.actions[0], { ...mixed.actions[1], status: "running" }] } });
    const openSession = vi.fn();
    useTermStore.setState({ sessions: [{ id: "s1" } as Session], activeSessionId: "other", openSession });
    render(<AssistRequestListener />);
    await waitFor(() => expect(requests.length).toBeGreaterThan(0));
    act(() => requests[requests.length - 1]({ id: "plan-1", sessionId: "s1", noConfirm: true }));
    await waitFor(() => expect(openSession).toHaveBeenCalledWith("s1"));
    expect(assistMark).not.toHaveBeenCalled();
    expect(new URLSearchParams(location.search).get("settings")).toBeNull();
  });
});
