//! Tests for what the composer offers to finish.

import { describe, expect, it } from "vitest";

import { buildSuggestions, findFileMention, mentionDir } from "./completion";
import type { ChatCommand } from "../../../ipc/chat";

const commands = [
  { name: "config", description: "Set a setting by key", argumentHint: "key=value" },
  { name: "compact", description: "Compact the conversation" },
  { name: "effort", argumentHint: "<low|medium|high>" },
];
const configKeys = [
  { key: "autoCompact", values: ["true", "false"] },
  { key: "autoScroll", values: ["true", "false"] },
  { key: "outputStyle", values: ["default", "Concise", "Explanatory"] },
  { key: "language", values: [] },
];
const build = (draft: string) => buildSuggestions({ draft, commands, configKeys });

describe("provider skill completion", () => {
  const catalogue: ChatCommand[] = [
    { name: "compact" },
    { name: "vspawn", invocation: "$", description: "Create a session" },
    { name: "compact", invocation: "$", description: "A same-named skill" },
    { name: "plugin:review", invocation: "$" },
  ];
  const suggest = (draft: string) => buildSuggestions({ draft, commands: catalogue, configKeys: [] });

  it("accepts both triggers and inserts the Codex native prefix", () => {
    for (const draft of ["/vsp", "$vsp"]) {
      expect(suggest(draft)?.[0].insert).toBe("$vspawn ");
      expect(suggest(draft)?.[0].desc).toBe("Create a session");
    }
    expect(suggest("$plugin:")?.[0].insert).toBe("$plugin:review ");
  });

  it("keeps slash commands first while dollar reaches a same-named skill", () => {
    expect(suggest("/compact")).toHaveLength(1);
    expect(suggest("/compact")?.[0].insert).toBe("/compact ");
    expect(suggest("$compact")?.[0].insert).toBe("$compact ");
    expect(suggest("$")).toHaveLength(3);
  });

  it("preserves the native Claude slash when either alias is selected", () => {
    const commands: ChatCommand[] = [{ name: "vspawn", invocation: "/" }];
    for (const draft of ["/vsp", "$vsp"]) {
      expect(buildSuggestions({ draft, commands, configKeys: [] })?.[0].insert).toBe("/vspawn ");
    }
    expect(buildSuggestions({ draft: "explain $vsp", commands, configKeys: [] })).toBeNull();
  });

  it("completes a Codex skill at the caret without removing surrounding text", () => {
    const hits = buildSuggestions({ draft: "use $vsp later", caret: 8, commands: catalogue, configKeys: [] });
    expect(hits?.[0].insert).toBe("use $vspawn later");
    expect(hits?.[0].caret).toBe(12);
    expect(suggest("cost$vsp")).toBeNull();
    expect(suggest("$vspawn task")).toBeNull();
  });

  it("keeps the full provider catalogue available for scrolling", () => {
    const commands: ChatCommand[] = Array.from({ length: 30 }, (_, i) => ({ name: `skill-${i}`, invocation: "$" }));
    expect(buildSuggestions({ draft: "$", commands, configKeys: [] })).toHaveLength(30);
    expect(buildSuggestions({ draft: "/", commands, configKeys: [] })).toHaveLength(30);
  });
});

const files = [
  { path: "src", isDir: true },
  { path: "scripts", isDir: true },
  { path: "README.md", isDir: false },
  { path: "my notes.md", isDir: false },
];
const nested = [
  { path: "src/layout", isDir: true },
  { path: "src/main.tsx", isDir: false },
];
/** Complete a mention with the caret at the end of the draft, which is where typing leaves it. */
const mention = (draft: string, list = files) =>
  buildSuggestions({ draft, commands, configKeys, files: list });

describe("buildSuggestions", () => {
  it("offers commands for a bare slash, narrowing as it is typed", () => {
    expect(build("/")?.map((s) => s.label)).toEqual(["/config", "/compact", "/effort"]);
    expect(build("/comp")?.map((s) => s.label)).toEqual(["/compact"]);
    expect(build("/nothing-like-this")).toBeNull();
  });

  it("completes a setting up to its equals sign, so values come next", () => {
    const hits = build("/config auto");
    expect(hits?.map((s) => s.label)).toEqual(["autoCompact", "autoScroll"]);
    expect(hits?.[0].insert).toBe("/config autoCompact=");
    expect(hits?.[0].hint).toBe("true | false");
  });

  it("shows a free-text setting as taking a value rather than a choice", () => {
    expect(build("/config lang")?.[0].hint).toBe("<value>");
  });

  it("offers the values of the setting being written", () => {
    expect(build("/config outputStyle=")?.map((s) => s.label)).toEqual([
      "default",
      "Concise",
      "Explanatory",
    ]);
    expect(build("/config outputStyle=Con")?.map((s) => s.label)).toEqual(["Concise"]);
  });

  it("stops offering once the draft already holds the only answer", () => {
    // This is what a chosen value looks like: without this the list would sit there forever, repeating
    // the choice that was just made, with no way to dismiss it.
    expect(build("/config outputStyle=Concise")).toBeNull();
    expect(build("/compact ")).toBeNull();
  });

  it("says nothing about a setting the agent never mentioned", () => {
    expect(build("/config madeUpKey=")).toBeNull();
  });

  it("stays out of the way of ordinary messages", () => {
    expect(build("")).toBeNull();
    expect(build("what does /config do?")).toBeNull();
    expect(build("/config outputStyle=Concise and then some")).toBeNull();
  });

  it("accepts the alias the agent reports for the same command", () => {
    expect(build("/settings auto")?.map((s) => s.label)).toEqual(["autoCompact", "autoScroll"]);
  });
});

describe("findFileMention", () => {
  it("reads the mention the caret is writing, at the start or mid-sentence", () => {
    expect(findFileMention("@src", 4)).toEqual({ start: 0, end: 4, query: "src" });
    expect(findFileMention("read @src/ma", 12)).toEqual({ start: 5, end: 12, query: "src/ma" });
  });

  it("leaves an address alone, because its @ does not follow whitespace", () => {
    expect(findFileMention("write to foo@bar.com", 20)).toBeNull();
  });

  it("stops at whitespace and quotes, so a finished mention is not still being written", () => {
    expect(findFileMention("@src/main.tsx and then", 22)).toBeNull();
    expect(findFileMention('@"my notes.md"', 14)).toBeNull();
  });

  it("reads from the caret rather than the end of the text", () => {
    expect(findFileMention("@src and more", 4)).toEqual({ start: 0, end: 4, query: "src" });
    expect(findFileMention("@src and more", 13)).toBeNull();
  });
});

describe("mentionDir", () => {
  it("names the directory a query points into", () => {
    expect(mentionDir("")).toBe("");
    expect(mentionDir("sr")).toBe("");
    expect(mentionDir("src/")).toBe("src/");
    expect(mentionDir("src/lay")).toBe("src/");
    expect(mentionDir("src/layout/Center")).toBe("src/layout/");
  });
});

describe("buildSuggestions for @ mentions", () => {
  it("offers the working directory for a bare @, narrowing as it is typed", () => {
    expect(mention("@")?.map((s) => s.label)).toEqual([
      "src/",
      "scripts/",
      "README.md",
      "my notes.md",
    ]);
    expect(mention("@s")?.map((s) => s.label)).toEqual(["src/", "scripts/"]);
    expect(mention("@nothing-like-this")).toBeNull();
  });

  it("completes a directory up to its slash, so its contents come next", () => {
    const hits = mention("@sr");
    expect(hits?.[0].insert).toBe("@src/");
    expect(hits?.[0].caret).toBe(5);
    expect(mention("@src/", nested)?.map((s) => s.label)).toEqual(["src/layout/", "src/main.tsx"]);
  });

  it("quotes a path with a space in it, and leaves the rest bare", () => {
    expect(mention("@my")?.[0].insert).toBe('@"my notes.md"');
    expect(mention("@READ")?.[0].insert).toBe("@README.md");
  });

  it("replaces only the mention, leaving the sentence around it", () => {
    const hits = buildSuggestions({
      draft: "please read @READ and summarise",
      caret: 17,
      commands,
      configKeys,
      files,
    });
    expect(hits?.[0].insert).toBe("please read @README.md and summarise");
    expect(hits?.[0].caret).toBe(22);
  });

  it("says nothing until the listing for the typed directory arrives", () => {
    // The parent's children are what is still in hand while the child's are being read; none of them
    // belong under `src/`, so the list stays empty rather than offering the wrong folder.
    expect(mention("@src/")).toBeNull();
  });

  it("stops offering once the draft already holds the only answer", () => {
    expect(mention("@README.md")).toBeNull();
  });

  it("stays out of the way of an address and of a mention already written", () => {
    expect(mention("mail foo@bar.com")).toBeNull();
    expect(mention("@README.md and more")).toBeNull();
  });
});
