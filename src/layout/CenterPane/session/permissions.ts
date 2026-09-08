//! The permission vocabulary a chat session speaks: the modes it runs in, and the standing rules the
//! agent offers alongside a question about a tool.
//!
//! A permission question can arrive with `permission_suggestions` — the agent's own proposals for what
//! would save it having to ask again, such as "accept file edits for the rest of this session" or "always
//! allow this command". Taking one is an ordinary allow that hands the proposal straight back in
//! `updatedPermissions`; nothing is composed here, so the object that arrives is the object that returns.
//!
//! Shapes verified against claude 2.1.258. `setMode`, `addRules` and `addDirectories` are what the CLI
//! actually sends. Three more exist in its schema — `replaceRules`, `removeRules`, `removeDirectories` —
//! and every one of them narrows permissions rather than widens them, so a button reading "allow" would
//! misdescribe what it does; they are left out, as is an `addRules` whose behavior is not `allow`.

import type { SessionKind } from "../../../types";

/** Permission modes the agent accepts, listed the way its own interface lists them. */
export const CLAUDE_MODES = ["plan", "default", "acceptEdits", "auto", "bypassPermissions"] as const;
export const CODEX_MODES = ["read-only", "auto", "full-access"] as const;
/** OpenCode either asks, or answers every permission itself the way its own `--auto` flag does. */
export const OPENCODE_MODES = ["default", "bypassPermissions"] as const;
export type Mode = (typeof CLAUDE_MODES)[number] | (typeof CODEX_MODES)[number];

export function modesFor(kind: SessionKind): readonly Mode[] {
  if (kind === "codex") return CODEX_MODES;
  if (kind === "opencode") return OPENCODE_MODES;
  return CLAUDE_MODES;
}

export function isMode(value: string): value is Mode {
  return ([...CLAUDE_MODES, ...CODEX_MODES] as readonly string[]).includes(value);
}

/** A standing rule the agent offered, exactly as it arrived; it travels back unchanged. */
export type PermissionSuggestion = Record<string, unknown>;

/** A suggestion the card can offer, reduced to what its button needs. */
export interface SuggestionOffer {
  /** The proposal itself, to hand back when the button is pressed. */
  suggestion: PermissionSuggestion;
  /** `network` is a Codex rule for one host; the others are what Claude proposes. */
  kind: "mode" | "rules" | "directories" | "network";
  /** True when it lasts only for this conversation, false when it is written to a settings file. */
  session: boolean;
  /** The mode a `setMode` proposal switches to, so the view's own control can follow it. */
  mode?: Mode;
  /** What a rule or directory proposal covers, written out for the button. */
  subject?: string;
}

/** Destinations that outlive the conversation: taking one of these edits a settings file on disk. */
const DURABLE = ["userSettings", "projectSettings", "localSettings"];

function readString(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key];
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

/** Describe one rule the way the agent writes it: the tool, and what of it the rule covers. */
function ruleText(rule: Record<string, unknown>): string | null {
  const toolName = readString(rule, "toolName");
  if (!toolName) return null;
  const content = readString(rule, "ruleContent");
  return content ? `${toolName}(${content})` : toolName;
}

/**
 * Read the suggestions a permission question carries, keeping only the ones a button can honestly offer.
 *
 * Anything unrecognized is dropped rather than guessed at. A suggestion is a permission change, and one
 * described by a label the user cannot check is worse than one that is never offered.
 */
export function parsePermissionSuggestions(value: unknown): SuggestionOffer[] {
  if (!Array.isArray(value)) return [];
  const offers: SuggestionOffer[] = [];
  for (const entry of value as unknown[]) {
    if (typeof entry !== "object" || entry === null || Array.isArray(entry)) continue;
    const raw = entry as Record<string, unknown>;
    const session = !DURABLE.includes(readString(raw, "destination") ?? "session");
    switch (raw.type) {
      case "setMode": {
        // A mode the view has no name for cannot be labelled, and its own control could not follow the
        // switch either, so it is left alone.
        const mode = readString(raw, "mode");
        if (!mode || !isMode(mode)) continue;
        offers.push({ suggestion: raw, kind: "mode", session, mode });
        break;
      }
      case "addRules": {
        if (raw.behavior !== "allow" || !Array.isArray(raw.rules)) continue;
        const texts: string[] = [];
        for (const rule of raw.rules as unknown[]) {
          if (typeof rule !== "object" || rule === null) continue;
          const text = ruleText(rule as Record<string, unknown>);
          if (text) texts.push(text);
        }
        if (texts.length === 0) continue;
        offers.push({ suggestion: raw, kind: "rules", session, subject: texts.join(", ") });
        break;
      }
      case "addDirectories": {
        if (!Array.isArray(raw.directories)) continue;
        const dirs = (raw.directories as unknown[]).filter(
          (d): d is string => typeof d === "string" && d.length > 0,
        );
        if (dirs.length === 0) continue;
        offers.push({ suggestion: raw, kind: "directories", session, subject: dirs.join(", ") });
        break;
      }
      case "codexAcceptForSession": {
        offers.push({
          suggestion: raw,
          kind: "rules",
          session: true,
          subject: readString(raw, "subject") ?? "this action",
        });
        break;
      }
      // Codex's execpolicy amendment: a command prefix written to the user's rules, so it outlives the
      // conversation the way Claude's settings-file rules do.
      case "codexExecpolicyAmendment": {
        const subject = readString(raw, "subject");
        if (!subject || !Array.isArray(raw.amendment)) continue;
        offers.push({ suggestion: raw, kind: "rules", session: false, subject });
        break;
      }
      // A standing network rule for the host a command wants to reach.
      case "codexNetworkPolicyAmendment": {
        const subject = readString(raw, "subject");
        if (!subject || typeof raw.amendment !== "object" || raw.amendment === null) continue;
        offers.push({ suggestion: raw, kind: "network", session: false, subject });
        break;
      }
      // OpenCode's "always": the patterns it would stop asking about, for the rest of the session and
      // beyond, which is why the button reads as a standing rule rather than a session one.
      case "opencodeAlways": {
        offers.push({
          suggestion: raw,
          kind: "rules",
          session: false,
          subject: readString(raw, "subject") ?? "this action",
        });
        break;
      }
      default:
        break;
    }
  }
  return offers;
}
