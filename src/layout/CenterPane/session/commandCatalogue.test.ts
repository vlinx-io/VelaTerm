import { describe, expect, it } from "vitest";

import type { ChatCommand } from "../../../ipc/chat";
import type { SessionKind } from "../../../types";
import fixture from "../../../../src-tauri/src/agent/chat/fixtures/claude-initialize-commands.json";
import { buildCatalogue, isImmediate, isListed, resolveSlash, type CatalogueEntry } from "./commandCatalogue";

/** What a real Claude Code handshake reported, as the backend hands it on (`invocation` set). */
const live: ChatCommand[] = (fixture.commands as ChatCommand[]).map((c) => ({ ...c, invocation: "/" }));
const claude = buildCatalogue("claude", live);
const listedNames = (catalogue: CatalogueEntry[]) => catalogue.filter(isListed).map((e) => e.name);
const resolve = (catalogue: CatalogueEntry[], text: string) => resolveSlash(catalogue, text)?.entry;

/** Names only the Claude manifest brings (everything but what every agent has). */
const CLAUDE_ONLY = buildCatalogue("claude", [])
  .map((e) => e.name)
  .filter((name) => !["clear", "new", "rewind"].includes(name));

describe("the Claude catalogue over a real handshake", () => {
  it("keeps every built-in the CLI reported exactly once", () => {
    for (const command of live) {
      expect(claude.filter((e) => e.name === command.name)).toHaveLength(1);
    }
  });

  it("never turns a command the CLI runs headless into a terminal handoff or picker", () => {
    // A manifest entry the live list reports would be one Claude can answer itself; it must stay passthrough.
    for (const command of live) {
      const entry = claude.find((e) => e.name === command.name)!;
      expect(["terminal", "picker", "unavailable"]).not.toContain(entry.treatment);
    }
  });

  it("lists the terminal-only commands next to the live ones", () => {
    const listed = listedNames(claude);
    for (const name of ["resume", "status", "memory", "hooks", "permissions", "plugin", "help", "config", "compact", "model"]) {
      expect(listed).toContain(name);
    }
    expect(listed).not.toContain("theme");
    expect(listed).not.toContain("exit");
    expect(resolve(claude, "/theme")?.treatment).toBe("unavailable");
    expect(resolve(claude, "/exit")?.reason).toBe("session");
  });

  it("orders VelaTerm's own entries first and the terminal handoffs last", () => {
    const listed = claude.filter(isListed);
    const firstTerminal = listed.findIndex((e) => e.treatment === "terminal");
    expect(listed[0].treatment).not.toBe("passthrough");
    expect(listed.slice(firstTerminal).every((e) => e.treatment === "terminal")).toBe(true);
  });

  it("finds and routes aliases", () => {
    expect(resolve(claude, "/reset")?.action).toBe("clear");
    expect(resolve(claude, "/new")?.action).toBe("clear");
    expect(resolve(claude, "/checkpoint")?.action).toBe("rewind");
    expect(resolve(claude, "/undo")?.action).toBe("rewind");
    expect(resolve(claude, "/continue")?.treatment).toBe("picker");
    expect(resolve(claude, "/allowed-tools")).toMatchObject({ name: "permissions", treatment: "terminal" });
    expect(resolve(claude, "/quit")).toMatchObject({ treatment: "unavailable", reason: "session" });
    expect(resolve(claude, "/settings")).toMatchObject({ name: "config", treatment: "passthrough" });
    expect(resolve(claude, "/cost")).toMatchObject({ name: "usage", treatment: "passthrough" });
  });

  it("lets a user's own command of the same name run instead of the manifest entry", () => {
    const catalogue = buildCatalogue("claude", [{ name: "status", description: "mine", invocation: "/" }]);
    expect(catalogue.filter((e) => e.name === "status")).toHaveLength(1);
    expect(resolve(catalogue, "/status")).toMatchObject({ treatment: "passthrough", description: "mine" });
  });

  it("splits the arguments from the name", () => {
    expect(resolveSlash(claude, "/resume  flaky test ")).toMatchObject({ name: "resume", args: "flaky test" });
    expect(resolveSlash(claude, "/status")?.args).toBe("");
    expect(resolveSlash(claude, "explain /status")).toBeNull();
  });

  it("covers every built-in the terminal interface of Claude Code 2.1.283 can show", () => {
    // Every built-in 2.1.283 defines that is not constantly hidden or switched off, read from the CLI.
    const inventory = [
      "add-dir", "advisor", "artifacts", "auto-mode-setup", "autocompact", "autofix-pr", "branch", "bug", "cd",
      "chrome", "clear", "cloud-plugins", "color", "compact", "config", "context", "copy", "design-login",
      "desktop", "diff", "effort", "exit", "export", "fast", "feedback", "focus", "help", "hooks", "ide",
      "import", "init", "insights", "install-github-app", "install-slack-app", "keybindings", "login", "mcp",
      "memory", "mobile", "model", "output-style", "passes", "permissions", "plan", "plugin", "powerup",
      "privacy-settings", "radio", "release-notes", "reload-plugins", "reload-skills", "remote-env", "rename",
      "resume", "rewind", "sandbox", "scroll-speed", "session", "setup-bedrock", "setup-vertex", "skills",
      "status", "statusline", "stickers", "tasks", "teleport", "terminal-setup", "theme", "tui", "ultraplan",
      "ultrareview", "upgrade", "usage", "usage-credits", "voice",
      // Defined outside the table above, in the terminal interface's own chunks.
      "background", "brief", "btw", "daemon", "fork", "logout", "quit", "remote", "remote-control", "stop",
      "subtask", "web-setup", "workflows",
    ];
    // Deliberately not in the manifest, each for a reason.
    const excluded: Record<string, string> = {
      ultraplan: "removed from Claude Code",
      // Account- or feature-gated, and runnable headless: when on, the live list brings them.
      advisor: "live layer", import: "live layer", ultrareview: "live layer", "usage-credits": "live layer",
    };
    for (const name of inventory) {
      if (excluded[name]) continue;
      expect(resolve(claude, `/${name}`), name).toBeDefined();
    }
  });

  it("keeps the rule Claude's /clear, /new and /rewind always had", () => {
    for (const name of ["clear", "new", "reset", "rewind", "checkpoint"]) {
      expect(resolve(claude, `/${name}`)).toMatchObject({ treatment: "native", optOut: true });
      expect(resolve(claude, `/${name}`)?.args).toBeUndefined();
    }
  });

  it("runs a handoff on the first Enter, and completes an agent command first", () => {
    expect(isImmediate(claude, "/status")).toBe(true);
    expect(isImmediate(claude, "/theme ")).toBe(true);
    expect(isImmediate(claude, "/model")).toBe(false);
    expect(isImmediate(claude, "/status now")).toBe(false);
  });
});

describe("other agents", () => {
  const skills: ChatCommand[] = [{ name: "vspawn", invocation: "$" }, { name: "compact", invocation: "$" }];

  it.each<SessionKind>(["codex", "pi", "omp", "opencode"])("%s shows no Claude command", (kind) => {
    const catalogue = buildCatalogue(kind, kind === "codex" ? skills : []);
    for (const name of CLAUDE_ONLY) {
      expect(catalogue.some((e) => e.name === name || e.aliases?.includes(name))).toBe(false);
      expect(resolve(catalogue, `/${name}`)).toBeUndefined();
    }
  });

  it("keep the entries they had, name for name", () => {
    const listed = (kind: SessionKind) => buildCatalogue(kind, []).filter(isListed)
      .map((e) => [e.name, e.descriptionKey, e.argumentHintKey ?? null]);
    expect(listed("codex")).toEqual([
      ["clear", "chat.command.clearDescription", null],
      ["compact", "chat.command.compactDescription", null],
      ["review", "chat.command.reviewDescription", "chat.command.reviewHint"],
    ]);
    expect(listed("pi")).toEqual([["clear", "chat.command.clearDescription", null], ["compact", "chat.command.compactDescription", null]]);
    expect(listed("omp")).toEqual(listed("pi"));
    expect(listed("opencode").map(([name]) => name)).toEqual(["clear", "compact", "undo", "redo", "share", "unshare"]);
    expect(listed("cursor").map(([name]) => name)).toEqual(["clear"]);
  });

  it("route as before", () => {
    const codex = buildCatalogue("codex", skills);
    expect(resolve(codex, "/compact")).toMatchObject({ action: "compact", optOut: true });
    expect(resolve(codex, "/review branch main")).toMatchObject({ action: "review", optOut: true });
    expect(resolve(codex, "/new")).toMatchObject({ action: "clear", optOut: true, hidden: true });
    expect(resolve(codex, "/rewind")).toMatchObject({ action: "rewind", optOut: true });
    // A skill keeps its `$` and is not what a slash command resolves to.
    expect(codex.filter((e) => e.invocation === "$").map((e) => e.name)).toEqual(["vspawn", "compact"]);
    expect(resolve(codex, "/vspawn")).toBeUndefined();
    expect(resolve(buildCatalogue("opencode", []), "/undo")).toMatchObject({ action: "backend" });
    expect(resolve(buildCatalogue("pi", []), "/review")).toBeUndefined();
  });

  it("keep clear, new and rewind even without any catalogue", () => {
    for (const name of ["clear", "new", "rewind"]) expect(resolve([], `/${name}`)?.name).toBe(name);
  });
});
