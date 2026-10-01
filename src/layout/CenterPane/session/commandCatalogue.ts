//! Every slash command the composer knows for a session, and what happens when one is sent.
//!
//! Two layers meet here. The live layer is what the agent itself reports (for Claude, the handshake's list
//! and every later `commands_changed`), and what it reports it can run headless. The manifest is what the
//! live layer cannot say: commands the agent has only in its terminal interface, commands VelaTerm answers
//! itself, and names that mean nothing in a conversation view. Each entry carries one treatment, and the
//! composer routes by it instead of by a list of names per agent.
//!
//! Manifests are per agent kind, so a Codex session never sees a Claude command. The Claude manifest is
//! pinned to what Claude Code 2.1.283 offers; descriptions of Claude's own commands are Claude's text,
//! passed through untranslated like the descriptions the live layer brings.

import type { ChatCommand } from "../../../ipc/chat";
import type { Dict, I18nKey } from "../../../i18n";
import type { SessionKind } from "../../../types";

/**
 * - `passthrough`: sent to the agent as typed; it answers the command itself.
 * - `native`: VelaTerm does it; nothing is sent to the agent unless the action says so.
 * - `picker`: opens VelaTerm's picker of earlier conversations.
 * - `terminal`: only the agent's terminal interface has it; the session moves there with the command typed.
 * - `unavailable`: has no meaning here; never listed, explained when typed, never sent.
 */
export type Treatment = "passthrough" | "native" | "picker" | "terminal" | "unavailable";

/** What a native entry does. `backend` is sent as a message and answered by VelaTerm's backend. */
export type NativeAction = "clear" | "rewind" | "fork" | "export" | "compact" | "review" | "backend";

/** A key whose text is a plain string, usable as a description or hint. */
type TextKey = { [K in I18nKey]: Dict[K] extends string ? K : never }[I18nKey];

export interface CatalogueEntry extends ChatCommand {
  treatment: Treatment;
  action?: NativeAction;
  /** Why an unavailable entry cannot run: it only changes the terminal, or VelaTerm manages it. */
  reason?: "terminal" | "session";
  /** Routed when typed, but not offered in the menu. */
  hidden?: boolean;
  /**
   * The baseline rule of the older, per-agent commands: with images, while steering, or (for an action
   * that takes none) with arguments, the text is sent as a message instead.
   */
  optOut?: boolean;
  /** For entries without `optOut`: whether text after the name is accepted or refused with a note. */
  args?: "reject" | "accept";
  /** VelaTerm's own description, translated; otherwise `description` is the agent's text. */
  descriptionKey?: TextKey;
  argumentHintKey?: TextKey;
}

type Manifest = CatalogueEntry[];

/** What every chat agent has, and had before this catalogue existed. */
const COMMON: Manifest = [
  { name: "clear", treatment: "native", action: "clear", optOut: true, descriptionKey: "chat.command.clearDescription" },
  // Routed as before, but never listed: `/clear` is the one the menu offers.
  { name: "new", treatment: "native", action: "clear", optOut: true, hidden: true, descriptionKey: "chat.command.clearDescription" },
  { name: "rewind", treatment: "native", action: "rewind", optOut: true, hidden: true, descriptionKey: "chat.command.rewindDescription" },
];

const CODEX: Manifest = [
  { name: "compact", treatment: "native", action: "compact", optOut: true, args: "accept", descriptionKey: "chat.command.compactDescription" },
  {
    name: "review", treatment: "native", action: "review", optOut: true, args: "accept",
    descriptionKey: "chat.command.reviewDescription", argumentHintKey: "chat.command.reviewHint",
  },
];

const PI: Manifest = [
  { name: "compact", treatment: "native", action: "compact", optOut: true, args: "accept", descriptionKey: "chat.command.compactDescription" },
];

// OpenCode's backend answers these without a turn when they are sent as a message.
const OPENCODE: Manifest = [
  { name: "compact", treatment: "native", action: "backend", optOut: true, descriptionKey: "chat.command.compactDescription" },
  { name: "undo", treatment: "native", action: "backend", optOut: true, descriptionKey: "chat.command.undoDescription" },
  { name: "redo", treatment: "native", action: "backend", optOut: true, descriptionKey: "chat.command.redoDescription" },
  { name: "share", treatment: "native", action: "backend", optOut: true, descriptionKey: "chat.command.shareDescription" },
  { name: "unshare", treatment: "native", action: "backend", optOut: true, descriptionKey: "chat.command.unshareDescription" },
];

/** A Claude command that only its terminal interface has, with the description Claude Code gives it. */
const terminal = (name: string, description?: string, aliases?: string[]): CatalogueEntry =>
  ({ name, description, aliases, treatment: "terminal", args: "accept" });

const unavailable = (reason: "terminal" | "session", name: string, aliases?: string[]): CatalogueEntry =>
  ({ name, aliases, treatment: "unavailable", reason });

const CLAUDE: Manifest = [
  // `/clear`, `/new` and `/rewind` keep the rule every agent has had; their aliases follow it.
  { name: "clear", aliases: ["reset", "new"], treatment: "native", action: "clear", optOut: true, descriptionKey: "chat.command.clearDescription" },
  { name: "rewind", aliases: ["checkpoint", "undo"], treatment: "native", action: "rewind", optOut: true, descriptionKey: "chat.command.rewindDescription" },
  { name: "fork", treatment: "native", action: "fork", args: "reject", descriptionKey: "tree.forkSession" },
  { name: "branch", treatment: "native", action: "fork", args: "reject", descriptionKey: "tree.forkSession" },
  { name: "export", treatment: "native", action: "export", args: "reject", descriptionKey: "tree.exportSession" },
  // With images or while steering, `/resume` goes out as a message, as it did before the catalogue.
  { name: "resume", aliases: ["continue"], treatment: "picker", optOut: true, args: "accept", descriptionKey: "chat.command.resumeDescription" },
  terminal("status", "Show Claude Code status including version, model, account, API connectivity, and tool statuses"),
  terminal("memory", "Edit CLAUDE.md files and memory settings"),
  terminal("hooks", "View hook configurations for tool events"),
  terminal("permissions", "Manage allow and deny tool permission rules", ["allowed-tools"]),
  terminal("plugin", "Manage Claude Code plugins", ["plugins", "marketplace"]),
  terminal("help", "Show help and available commands"),
  terminal("ide", "Manage IDE integrations and show status"),
  terminal("login", "Sign in with your Anthropic account"),
  terminal("logout", "Sign out from your Anthropic account"),
  terminal("copy", "Copy Claude's last response to clipboard (or /copy N for the Nth-latest)"),
  terminal("diff", "View uncommitted changes and per-turn diffs"),
  terminal("plan", "Enable plan mode or view the current session plan"),
  terminal("tasks", "View and manage everything running in the background", ["bashes"]),
  terminal("skills", "List available skills"),
  terminal("sandbox"),
  terminal("release-notes", "View release notes"),
  terminal("privacy-settings", "View and update your privacy settings"),
  terminal("powerup", "Discover Claude Code features through quick interactive lessons"),
  terminal("feedback", "Send feedback to Anthropic or report a bug"),
  terminal("bug", "Report a bug or share your conversation", ["share"]),
  terminal("upgrade", "Upgrade to Max for higher rate limits and more Opus"),
  terminal("passes", "Share a free week of Claude Code with friends"),
  terminal("btw", "Ask a quick side question without interrupting the main conversation"),
  terminal("background", "Send this session to the background and free the terminal", ["bg"]),
  terminal("add-dir", "Add a new working directory"),
  terminal("install-github-app", "Set up Claude GitHub Actions for a repository"),
  terminal("install-slack-app", "Install the Claude Slack app"),
  // Shown by the terminal interface only with the matching provider or feature; listed like the other
  // gated entries, and the terminal answers for itself where it is off.
  terminal("setup-bedrock", "Reconfigure Amazon Bedrock authentication, region, or model pins"),
  terminal("setup-vertex", "Reconfigure Google Vertex AI authentication, project, region, or model pins"),
  terminal("cloud-plugins", "Choose whether cloud sessions use the plugins enabled on this machine"),
  terminal("session", "Show cloud session URL and QR code", ["remote"]),
  terminal("brief", "Toggle brief-only mode"),
  terminal("daemon", "Manage background services and routines"),
  terminal("chrome", "Open Claude in Chrome settings"),
  terminal("remote-control", "Control this session from your phone or claude.ai/code", ["rc"]),
  terminal("teleport", "Send this session to the cloud, or resume one from claude.ai", ["tp"]),
  terminal("remote-env", "Choose the default environment for cloud agents"),
  terminal("web-setup", "Set up cloud sessions with your GitHub account"),
  terminal("artifacts", "Browse your published and shared artifacts"),
  terminal("autofix-pr", "Monitor and autofix any issues with the current PR"),
  terminal("radio", "Listen to Claude FM lo-fi radio"),
  terminal("stickers", "Order Claude Code stickers"),
  terminal("workflows", "Browse running and completed workflows"),
  terminal("subtask", "Send a subagent off with your full context; its result comes back here"),
  terminal("design-login", "Authorize design-system access for /design-sync with your claude.ai account"),
  ...["theme", "tui", "vim", "color", "focus", "scroll-speed", "terminal-setup", "statusline", "keybindings", "voice"]
    .map((name) => unavailable("terminal", name)),
  unavailable("session", "exit", ["quit"]),
  unavailable("session", "desktop", ["app"]),
  unavailable("session", "mobile", ["ios", "android"]),
  unavailable("session", "cd"),
  unavailable("session", "stop"),
];

function manifestFor(kind: SessionKind): Manifest {
  switch (kind) {
    case "claude": return CLAUDE;
    case "codex": return [...COMMON, ...CODEX];
    case "pi":
    case "omp": return [...COMMON, ...PI];
    case "opencode": return [...COMMON, ...OPENCODE];
    default: return COMMON;
  }
}

const names = (entry: ChatCommand) => [entry.name, ...(entry.aliases ?? [])];
const union = (a?: string[], b?: string[]) => {
  const all = [...new Set([...(a ?? []), ...(b ?? [])])];
  return all.length ? all : undefined;
};

/** Menu order: what VelaTerm does first, then the agent's own commands, then the terminal handoffs. */
function rank(entry: CatalogueEntry): number {
  if (entry.treatment === "native" || entry.treatment === "picker") return 0;
  if (entry.treatment === "passthrough") return entry.builtin ? 1 : 2;
  if (entry.treatment === "terminal") return 3;
  return 4;
}

/**
 * Merge what the agent reported with what its manifest adds.
 *
 * A VelaTerm action wins over a same-named live command, as `/clear` always did. A Claude command
 * defined by a user, project or plugin wins over a manifest entry of its name, because that one really
 * runs. A live built-in never becomes unavailable or a terminal handoff: if the CLI reports it, the CLI
 * can answer it here, and it stays passthrough.
 */
export function buildCatalogue(kind: SessionKind, live: ChatCommand[]): CatalogueEntry[] {
  const manifest = manifestFor(kind);
  const taken = new Set<CatalogueEntry>();
  const out: CatalogueEntry[] = [];
  for (const command of live) {
    // Codex skills stay as they are, next to any same-named command.
    if (command.invocation === "$") {
      out.push({ ...command, treatment: "passthrough" });
      continue;
    }
    const match = manifest.find((m) => names(m).includes(command.name));
    if (!match || taken.has(match)) {
      out.push({ ...command, treatment: "passthrough" });
      continue;
    }
    if (match.treatment === "native" || match.treatment === "picker") {
      taken.add(match);
      out.push({ ...match, invocation: command.invocation, aliases: union(match.aliases, command.aliases), builtin: command.builtin });
      continue;
    }
    // A user's, project's or plugin's own command of that name, or a built-in the CLI can run itself.
    out.push({ ...command, treatment: "passthrough" });
    if (command.name === match.name) taken.add(match);
  }
  for (const entry of manifest) if (!taken.has(entry)) out.push(entry);
  return out
    .map((entry, index) => ({ entry, index }))
    .sort((a, b) => rank(a.entry) - rank(b.entry) || a.index - b.index)
    .map(({ entry }) => entry);
}

/** Whether the menu offers this entry. */
export const isListed = (entry: CatalogueEntry) => entry.treatment !== "unavailable" && !entry.hidden;

export interface ResolvedSlash {
  entry: CatalogueEntry;
  /** The name as typed, which may be an alias. */
  name: string;
  /** Text after the name, trimmed; empty when there is none. */
  args: string;
}

/**
 * Find the entry a submitted `/name args` means, by name first and then by alias. Codex `$` skills are
 * never matched here. `/clear`, `/new` and `/rewind` resolve even in an empty catalogue, so a missing
 * list never takes away what the composer always did.
 */
export function resolveSlash(catalogue: CatalogueEntry[], text: string): ResolvedSlash | null {
  const match = /^\/(\S+)(?:\s+([\s\S]*))?$/.exec(text.trim());
  if (!match) return null;
  const [, name, rest] = match;
  const args = rest?.trim() ?? "";
  const slash = catalogue.filter((entry) => entry.invocation !== "$");
  const entry = slash.find((e) => e.name === name) ?? slash.find((e) => e.aliases?.includes(name))
    ?? COMMON.find((e) => e.name === name);
  return entry ? { entry, name, args } : null;
}

/**
 * Whether Enter on this exact draft should run it rather than complete it. Completion would add its
 * trailing space and make `/clear` or `/status` need two submissions.
 */
export function isImmediate(catalogue: CatalogueEntry[], draft: string): boolean {
  if (!/^\/\S+\s*$/.test(draft)) return false;
  const resolved = resolveSlash(catalogue, draft);
  return !!resolved && resolved.entry.treatment !== "passthrough";
}
