//! Documents beside a conversation share the document-tab actions without becoming tabs.

import { beforeEach, describe, expect, it } from "vitest";

import { findDocTab, useTermStore } from "./termStore";

beforeEach(() => useTermStore.setState({ dockDocs: {}, docTabs: {}, openTabs: [] }));

describe("dockDocs", () => {
  it("takes the document actions and stays out of the tab strip", () => {
    const store = useTermStore.getState();
    const id = store.openDockDoc("/w/a.ts");
    expect(useTermStore.getState().openTabs).toEqual([]);
    store.setDocTabMode(id, "source");
    store.setDocTabDirty(id, true);
    store.refreshDocTab(id);
    expect(findDocTab(useTermStore.getState(), id)).toMatchObject({ kind: "code", mode: "source", dirty: true, reloadNonce: 1 });
  });

  it("asks before closing unsaved edits, and closes through closeTab like a tab", () => {
    const store = useTermStore.getState();
    const id = store.openDockDoc("/w/notes.md");
    store.setDocTabDirty(id, true);
    store.requestCloseDocTab(id);
    expect(useTermStore.getState().dockDocs[id].pendingClose).toBe(true);
    store.cancelCloseDocTab(id);
    expect(useTermStore.getState().dockDocs[id].pendingClose).toBe(false);
    store.closeTab(id);
    expect(useTermStore.getState().dockDocs).toEqual({});
  });
});
