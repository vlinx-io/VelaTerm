// @vitest-environment jsdom
import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { setLang } from "../../i18n";
import { useTermStore } from "../../store/termStore";
import type { Project, Session } from "../../types";
import { RightPanel } from "./RightPanel";

const mocks = vi.hoisted(() => ({
  listDir: vi.fn(),
  fsWatch: vi.fn(),
  getSessionCwd: vi.fn(),
}));

vi.mock("../../ipc/info", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../ipc/info")>();
  return {
    ...actual,
    listDir: mocks.listDir,
    fsWatch: mocks.fsWatch,
    clipboardFileState: vi.fn().mockResolvedValue({ supported: false, hasFiles: false, mode: null, sequence: 0 }),
  };
});

vi.mock("../../ipc/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../ipc/commands")>();
  return {
    ...actual,
    getSessionCwd: mocks.getSessionCwd,
  };
});

vi.mock("../../ipc/events", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../ipc/events")>();
  return {
    ...actual,
    onFsChanged: () => Promise.resolve(() => {}),
  };
});

vi.mock("../../remote/ports/portsClient", () => ({ portsSupported: false }));

const project: Project = {
  id: "p1",
  name: "demo.com",
  rootPath: "/Users/test/Projects/中文项目/demo.com",
  color: null,
  sortOrder: 0,
  collapsed: false,
  createdAt: 0,
};

function makeSession(overrides: Partial<Session>): Session {
  return {
    id: "s1",
    projectId: "p1",
    groupId: null,
    name: "Session",
    kind: "terminal",
    shell: null,
    cwd: null,
    envJson: null,
    initCmd: null,
    hotkey: null,
    parentSessionId: null,
    collapsed: false,
    worktreePath: null,
    sortOrder: 0,
    createdAt: 0,
    ...overrides,
  };
}

describe("RightPanel missing session cwd fallback", () => {
  beforeEach(() => {
    setLang("en");
    window.history.replaceState(null, "", "/?inspector=files");
    vi.resetAllMocks();
    mocks.fsWatch.mockResolvedValue({ root: project.rootPath, supported: false });
    mocks.getSessionCwd.mockResolvedValue(null);
  });

  afterEach(() => {
    cleanup();
    useTermStore.setState({
      projects: [],
      groups: [],
      sessions: [],
      ephemeralSessions: {},
      runtimes: {},
      activeSessionId: null,
      inspectTarget: null,
    });
  });

  it("falls back to project.rootPath and heals ephemeral split cwd when session.cwd is missing on disk", async () => {
    const badPath = "/Users/test/Projects/\\xe4\\xb8\\xad\\xe6\\x96\\x87/demo.com";
    const split = makeSession({ id: "eph-split", name: "Split", cwd: badPath });
    mocks.listDir.mockImplementation(async (path: string) => {
      if (path === badPath) {
        throw new Error("Failed to read directory: No such file or directory (os error 2)");
      }
      if (path === project.rootPath) {
        return [{ name: "package.json", isDir: false, isHidden: false, gitBadge: null }];
      }
      return [];
    });

    useTermStore.setState({
      inspectorTab: "files",
      projects: [project],
      groups: [],
      sessions: [],
      ephemeralSessions: { "eph-split": split },
      runtimes: { "eph-split": { status: "running" } },
      activeSessionId: "eph-split",
      inspectTarget: null,
    });

    const view = render(<RightPanel />);
    await act(async () => {});
    await act(async () => {});

    expect(screen.getByText("package.json")).toBeTruthy();
    expect(view.container.querySelector(".files-err")).toBeNull();
    expect(useTermStore.getState().ephemeralSessions["eph-split"]?.cwd).toBeNull();
  });
});
