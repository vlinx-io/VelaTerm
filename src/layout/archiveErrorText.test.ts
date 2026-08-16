//! The backend reports blocked archives as a machine envelope; the dialog must render it in the
//! user's locale and fall back to the raw reason for codes it does not know.

import { describe, expect, it, vi } from "vitest";

vi.mock("../ipc/commands", () => ({}));
vi.mock("../store/termStore", () => ({ useTermStore: { getState: () => ({}) } }));

import { archiveErrorText } from "./sessionMenuDialogs";

describe("archiveErrorText", () => {
  it("passes a plain rejection string through verbatim", () => {
    expect(archiveErrorText(new Error("database lock is unavailable"))).toBe(
      "database lock is unavailable",
    );
  });

  it("localizes coded blocker rows and appends the suffix", () => {
    const envelope =
      "archive_blocked:" +
      JSON.stringify([
        {
          name: "worker-a",
          reason: "worktree has no verified landing",
          code: "noVerifiedLanding",
        },
      ]);
    expect(archiveErrorText(new Error(envelope))).toBe(
      '"worker-a" still holds a worktree: the worktree has no verified landing ' +
        "Land or retire the worker first.",
    );
  });

  it("keeps the raw reason for rows without a known code and joins rows", () => {
    const envelope =
      "archive_blocked:" +
      JSON.stringify([
        {
          name: "worker-a",
          reason: "worktree has uncommitted changes",
          code: "uncommittedChanges",
        },
        { name: "worker-b", reason: "exotic git failure", code: null },
      ]);
    const text = archiveErrorText(envelope);
    expect(text).toContain(
      '"worker-a" still holds a worktree: the worktree has uncommitted changes',
    );
    expect(text).toContain('"worker-b" still holds a worktree: exotic git failure');
    expect(text).toContain("Land or retire the worker first.");
  });

  it("returns the raw message when the envelope payload is not valid JSON", () => {
    expect(archiveErrorText("archive_blocked:not-json")).toBe(
      "archive_blocked:not-json",
    );
  });
});
