//! Tests for reading the standing rules an agent offers with a permission question.
//!
//! Every payload here was captured from claude 2.1.258 answering `--permission-prompt-tool stdio`.

import { describe, expect, it } from "vitest";

import { parsePermissionSuggestions } from "./permissions";

/** What a Write or Edit question offers: accept file edits for the rest of the conversation. */
const setMode = { type: "setMode", mode: "acceptEdits", destination: "session" };
/** What a Bash question offers: remember this exact command in the project's own settings. */
const addRules = {
  type: "addRules",
  rules: [{ toolName: "Bash", ruleContent: "touch probe-a.txt" }],
  behavior: "allow",
  destination: "localSettings",
};
/** What a write outside the working directory offers: work in that directory too. */
const addDirectories = {
  type: "addDirectories",
  directories: ["/Users/someone/Temp/work"],
  destination: "session",
};

describe("parsePermissionSuggestions", () => {
  it("reads a mode switch and says it lasts only for this conversation", () => {
    expect(parsePermissionSuggestions([setMode])).toEqual([
      { suggestion: setMode, kind: "mode", session: true, mode: "acceptEdits" },
    ]);
  });

  it("writes a rule out as the tool and what it covers", () => {
    const [offer] = parsePermissionSuggestions([addRules]);
    expect(offer.kind).toBe("rules");
    expect(offer.subject).toBe("Bash(touch probe-a.txt)");
    // localSettings is a file on disk, so this one outlives the conversation.
    expect(offer.session).toBe(false);
  });

  it("keeps a rule that names only a tool", () => {
    const [offer] = parsePermissionSuggestions([
      { type: "addRules", rules: [{ toolName: "WebSearch" }], behavior: "allow" },
    ]);
    expect(offer.subject).toBe("WebSearch");
  });

  it("reads the directories a question offers to open up", () => {
    const [offer] = parsePermissionSuggestions([addDirectories]);
    expect(offer.kind).toBe("directories");
    expect(offer.subject).toBe("/Users/someone/Temp/work");
    expect(offer.session).toBe(true);
  });

  /** Codex's own rules: a session grant, an execpolicy prefix for the rules file, and a host rule. */
  it("reads the rules a Codex command approval offers", () => {
    const offers = parsePermissionSuggestions([
      { type: "codexAcceptForSession", destination: "session", subject: "cargo test --all" },
      { type: "codexExecpolicyAmendment", destination: "userSettings", subject: "cargo test", amendment: ["cargo", "test"] },
      { type: "codexNetworkPolicyAmendment", destination: "userSettings", subject: "crates.io", amendment: { action: "allow", host: "crates.io" } },
    ]);
    expect(offers.map((o) => [o.kind, o.session, o.subject])).toEqual([
      ["rules", true, "cargo test --all"],
      ["rules", false, "cargo test"],
      ["network", false, "crates.io"],
    ]);
  });

  it("drops a Codex amendment that has nothing to send back", () => {
    expect(
      parsePermissionSuggestions([
        { type: "codexExecpolicyAmendment", subject: "cargo test" },
        { type: "codexNetworkPolicyAmendment", subject: "crates.io", amendment: "allow" },
      ]),
    ).toEqual([]);
  });

  it("keeps every offer a single question carries, in the order it sent them", () => {
    const offers = parsePermissionSuggestions([addRules, addDirectories, setMode]);
    expect(offers.map((o) => o.kind)).toEqual(["rules", "directories", "mode"]);
  });

  /** The button says "allow", so a proposal that would refuse or re-ask must not wear it. */
  it("drops a rule that does not allow", () => {
    expect(
      parsePermissionSuggestions([
        { type: "addRules", rules: [{ toolName: "Bash" }], behavior: "deny" },
        { type: "addRules", rules: [{ toolName: "Bash" }], behavior: "ask" },
      ]),
    ).toEqual([]);
  });

  /** Narrowing permissions is not something an allow button can offer. */
  it("drops the proposals that take permissions away", () => {
    expect(
      parsePermissionSuggestions([
        { type: "removeRules", rules: [{ toolName: "Bash" }], behavior: "allow" },
        { type: "replaceRules", rules: [{ toolName: "Bash" }], behavior: "allow" },
        { type: "removeDirectories", directories: ["/tmp"] },
      ]),
    ).toEqual([]);
  });

  /** A mode with no name in this view could be neither labelled nor followed by its own control. */
  it("drops a mode it has no name for", () => {
    expect(parsePermissionSuggestions([{ type: "setMode", mode: "dontAsk" }])).toEqual([]);
  });

  it("survives anything that is not a list of proposals", () => {
    expect(parsePermissionSuggestions(undefined)).toEqual([]);
    expect(parsePermissionSuggestions(null)).toEqual([]);
    expect(parsePermissionSuggestions("setMode")).toEqual([]);
    expect(parsePermissionSuggestions([null, 3, "x", [], {}])).toEqual([]);
    expect(parsePermissionSuggestions([{ type: "addRules", behavior: "allow" }])).toEqual([]);
    expect(parsePermissionSuggestions([{ type: "addDirectories", directories: [] }])).toEqual([]);
  });
});
