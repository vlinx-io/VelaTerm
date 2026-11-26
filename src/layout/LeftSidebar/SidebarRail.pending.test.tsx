//! Activity bar pending count badges per sidebar section.

import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../ipc/transport", async (original) => ({
  ...await original<typeof import("../../ipc/transport")>(),
  invoke: vi.fn().mockResolvedValue({ linked: false }),
  listen: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("../../ipc/commands", async (original) => ({
  ...await original<typeof import("../../ipc/commands")>(),
  listShells: vi.fn().mockResolvedValue([]),
}));
vi.mock("../../ipc/settingsSync", () => ({ pushSetting: vi.fn() }));

import { setLang } from "../../i18n";
import { loadSettings, SETTINGS_KEY } from "../../store/settings";
import { useTermStore } from "../../store/termStore";
import type { Session, SessionRuntime } from "../../types";
import { SidebarRail } from "./SidebarRail";
import { SYSTEM_CHAT, SYSTEM_PROJECTS, SYSTEM_TASKS } from "./systemCollections";

const mkSession = (id: string, projectId: string, kind: Session["kind"] = "claude"): Session => ({
  id,
  projectId,
  name: id,
  kind,
  collapsed: false,
  sortOrder: 0,
  createdAt: 0,
});

beforeEach(() => {
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  localStorage.removeItem(SETTINGS_KEY);
  setLang("en");
  useTermStore.setState({
    ...loadSettings(),
    projects: [
      { id: SYSTEM_PROJECTS, name: "Projects", rootPath: "", collapsed: false, collectionId: null, sortOrder: 0, createdAt: 0 },
      { id: SYSTEM_TASKS, name: "Tasks", rootPath: "", collapsed: false, collectionId: null, sortOrder: 1, createdAt: 0 },
      { id: SYSTEM_CHAT, name: "Chat", rootPath: "", collapsed: false, collectionId: null, sortOrder: 2, createdAt: 0 },
      { id: "proj-1", name: "App", rootPath: "/tmp/app", collapsed: false, collectionId: SYSTEM_PROJECTS, sortOrder: 3, createdAt: 0 },
    ],
    sessions: [],
    ephemeralSessions: {},
    runtimes: {},
    notifications: {},
  });
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  localStorage.removeItem(SETTINGS_KEY);
});

describe("activity bar pending badges", () => {
  it("shows pending count badges on projects, tasks, and chat while keeping only running count on terminals", async () => {
    const runtimes: Record<string, SessionRuntime> = {
      p1: { status: "running", agent: "claude", agentState: "asking" },
      p2: { status: "running", agent: "claude", agentState: "working" },
      t1: { status: "running", agent: "codex", agentState: "asking" },
      t2: { status: "running", agent: "claude", agentState: "waiting" },
      c1: { status: "running", agent: "claude", agentState: "waiting" },
      term1: { status: "running", agent: "claude", agentState: "asking" },
      term2: { status: "running" },
    };
    useTermStore.setState({
      sessions: [
        mkSession("p1", "proj-1"),
        mkSession("p2", "proj-1"),
        mkSession("t1", SYSTEM_TASKS),
        mkSession("t2", SYSTEM_TASKS),
        mkSession("c1", SYSTEM_CHAT),
        mkSession("term1", "proj-1", "terminal"),
      ],
      ephemeralSessions: {
        term2: mkSession("term2", "proj-1", "terminal"),
      },
      runtimes,
      notifications: {
        t2: 1,
        c1: 2,
      },
    });

    await act(async () => render(<SidebarRail />));

    const projectsBtn = screen.getByRole("button", { name: "Projects (2 Pending)" });
    expect(projectsBtn.querySelector(".sidebar-rail-pending")?.textContent).toBe("2");
    expect(projectsBtn.getAttribute("title")).toBe("Projects · 2 Pending");

    const terminalsBtn = screen.getByRole("button", { name: "Terminals (2)" });
    expect(terminalsBtn.querySelector(".sidebar-rail-pending")).toBeNull();
    expect(terminalsBtn.querySelector(".sidebar-rail-count")?.textContent).toBe("2");

    const tasksBtn = screen.getByRole("button", { name: "Tasks (2 Pending)" });
    expect(tasksBtn.querySelector(".sidebar-rail-pending")?.textContent).toBe("2");

    const chatBtn = screen.getByRole("button", { name: "Chats (1 Pending)" });
    expect(chatBtn.querySelector(".sidebar-rail-pending")?.textContent).toBe("1");
  });

  it("caps displayed pending counts at 99+ and omits the badge when zero", async () => {
    const sessions = Array.from({ length: 105 }, (_, i) => mkSession(`task-${i}`, SYSTEM_TASKS));
    const notifications = Object.fromEntries(sessions.map((s) => [s.id, 1]));
    useTermStore.setState({ sessions, notifications });

    await act(async () => render(<SidebarRail />));

    const tasksBtn = screen.getByRole("button", { name: "Tasks (105 Pending)" });
    expect(tasksBtn.querySelector(".sidebar-rail-pending")?.textContent).toBe("99+");

    const projectsBtn = screen.getByRole("button", { name: "Projects" });
    expect(projectsBtn.querySelector(".sidebar-rail-pending")).toBeNull();
  });
});
