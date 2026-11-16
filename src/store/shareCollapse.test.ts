//! A share visitor's expand and collapse choices must work without an error and survive tree refreshes.

import { beforeEach, describe, expect, it, vi } from "vitest";

const rpc = vi.hoisted(() => ({
  listTree: vi.fn(),
  setCollapsed: vi.fn().mockResolvedValue(null),
}));
vi.mock("../ipc/shareBase", () => ({ isShareSurface: true, shareBasePath: "/r/abc", apiUrl: (path: string) => path }));
vi.mock("../ipc/tree", () => rpc);

import { useTermStore } from "./termStore";

const hostTree = () => ({
  projects: [{ id: "p1", collapsed: true }],
  groups: [{ id: "g1", collapsed: false }],
  sessions: [],
});

beforeEach(() => {
  rpc.listTree.mockResolvedValue(hostTree());
  useTermStore.setState(hostTree() as never);
  useTermStore.setState({ treeMutationError: null });
});

describe("collapse on the public share surface", () => {
  it("expands a project without an error and keeps it across a refresh from the host", async () => {
    const store = useTermStore.getState();
    await store.toggleCollapsed("project", "p1");
    expect(useTermStore.getState().treeMutationError).toBeNull();
    expect(useTermStore.getState().projects[0].collapsed).toBe(false);

    await store.loadTree();
    expect(useTermStore.getState().projects[0].collapsed).toBe(false);
  });

  it("keeps a group's choice across a refresh from the host", async () => {
    const store = useTermStore.getState();
    await store.toggleCollapsed("group", "g1");
    await store.loadTree();
    expect(useTermStore.getState().groups[0].collapsed).toBe(true);
  });
});
