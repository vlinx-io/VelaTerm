//! What the composer offers to finish as you type.
//!
//! Four shapes, narrowest first: a path for an `@` mention, a value for a setting, a setting for `/config`,
//! or a command. Settings are worth completing because `/config` is how everything the agent has no
//! dedicated control for gets changed, and nobody remembers thirty key names. A path is worth completing
//! because typing one by hand is where typos come from; `@` itself is only a way to reach the list, and
//! what gets sent is the plain path.

import type { ChatCommand, ChatConfigKey } from "../../../ipc/chat";

/** One thing the composer is offering to finish. */
export interface Suggestion {
  id: string;
  label: string;
  /** Dimmed trailing text: a command's argument hint, or a setting's allowed values. */
  hint?: string;
  desc?: string;
  /** What the draft becomes when this is chosen. */
  insert: string;
  /** Where the caret goes afterwards; the end of the draft when absent. */
  caret?: number;
}

/** One path under the session's working directory, offered to finish an `@` mention. */
export interface FileCandidate {
  /** Path relative to the working directory, separated by `/` and without a trailing slash. */
  path: string;
  isDir: boolean;
}

/** Where an `@` mention sits in the draft, and what has been typed into it so far. */
export interface FileMention {
  /** Index of the `@`. */
  start: number;
  /** End of the query, which is where the caret is. */
  end: number;
  /** The text between the `@` and the caret. */
  query: string;
}

/** Most suggestions shown at once. Enough to choose from, few enough to read. */
const LIMIT = 8;

/**
 * Most paths shown at once. Higher than the others because this list is a directory being browsed rather
 * than a handful of near-misses, and eight entries would hide most folders. The list scrolls.
 */
const FILE_LIMIT = 20;

/**
 * What ends a mention. A path containing a space can still be chosen from the list; it just cannot be
 * typed past the space, because that is where the mention stops.
 */
const MENTION_END = /[\s"']/;

/**
 * Find the mention the caret is writing, or null when it is not writing one.
 *
 * A mention may sit mid-sentence, but only after whitespace, so `foo@bar.com` stays an address.
 */
export function findFileMention(text: string, cursor: number): FileMention | null {
  const caret = Math.max(0, Math.min(cursor, text.length));
  const before = text.slice(0, caret);
  for (
    let at = before.lastIndexOf("@");
    at >= 0;
    at = at === 0 ? -1 : before.lastIndexOf("@", at - 1)
  ) {
    if (at > 0 && !/\s/.test(text[at - 1])) continue;
    const query = before.slice(at + 1);
    if (MENTION_END.test(query)) continue;
    return { start: at, end: caret, query };
  }
  return null;
}

/** The directory a query points into, relative to the working directory: `src/lay` reads `src/`. */
export function mentionDir(query: string): string {
  const slash = query.lastIndexOf("/");
  return slash < 0 ? "" : query.slice(0, slash + 1);
}

/** Quote a path only when it needs quoting, so the ordinary case stays readable. */
function quotePath(path: string): string {
  return /[\s"]/.test(path) ? `"${path.replace(/"/g, '\\"')}"` : path;
}

/**
 * Build the list for the current draft, or null when there is nothing to offer.
 *
 * A list holding only what has already been typed offers nothing: after choosing `Concise` the draft ends
 * in `Concise`, which still looks like a value being typed, and the list would sit there repeating the
 * answer with no way to get rid of it.
 */
export function buildSuggestions({
  draft,
  caret = draft.length,
  commands,
  configKeys,
  files = [],
}: {
  draft: string;
  /** Caret position, which is what tells a mention being written from one already finished. */
  caret?: number;
  commands: ChatCommand[];
  configKeys: ChatConfigKey[];
  /** Children of the directory the current mention points into; empty until that listing arrives. */
  files?: FileCandidate[];
}): Suggestion[] | null {
  const take = (items: Suggestion[], limit = LIMIT): Suggestion[] | null =>
    items.length && !(items.length === 1 && items[0].insert === draft)
      ? items.slice(0, limit)
      : null;

  // A mention is caret-driven, so it is settled first: everything below reads the draft as a whole.
  const mention = findFileMention(draft, caret);
  if (mention) {
    const query = mention.query.toLowerCase();
    // Matching the whole relative path rather than the last segment keeps a listing that has not caught up
    // with the typing — the parent's children, while the child's are still being read — from offering
    // entries that do not belong under what was typed.
    return take(
      files
        .filter((f) => f.path.toLowerCase().startsWith(query))
        .map((f) => {
          // A directory keeps its trailing slash so the next keystroke lists what is inside it.
          const shown = f.isDir ? `${f.path}/` : f.path;
          const text = `@${quotePath(shown)}`;
          return {
            id: shown,
            label: shown,
            insert: draft.slice(0, mention.start) + text + draft.slice(mention.end),
            caret: mention.start + text.length,
          };
        }),
      FILE_LIMIT,
    );
  }

  const value = /^\/(?:config|settings)\s+([A-Za-z0-9_.-]+)=(\S*)$/.exec(draft);
  if (value) {
    const [, key, typed] = value;
    const spec = configKeys.find((k) => k.key === key);
    if (!spec) return null;
    const query = typed.toLowerCase();
    return take(
      spec.values
        .filter((v) => v.toLowerCase().startsWith(query))
        .map((v) => ({ id: v, label: v, insert: `/config ${key}=${v}` })),
    );
  }

  const key = /^\/(?:config|settings)\s+([A-Za-z0-9_.-]*)$/.exec(draft);
  if (key) {
    const query = key[1].toLowerCase();
    return take(
      configKeys
        .filter((k) => k.key.toLowerCase().startsWith(query))
        // A setting is completed up to the `=` so the next keystroke offers its values.
        .map((k) => ({
          id: k.key,
          label: k.key,
          hint: k.values.length ? k.values.join(" | ") : "<value>",
          insert: `/config ${k.key}=`,
        })),
    );
  }

  // Dollar mentions can appear within a Codex prompt; slash commands must start the draft.
  const before = draft.slice(0, caret);
  const dollar = /(?:^|\s)\$([^\s$]*)$/.exec(before);
  if (dollar) {
    const start = before.lastIndexOf("$");
    const query = dollar[1].toLowerCase();
    return take(
      commands
        .filter((c) => c.invocation && (start === 0 || c.invocation === "$")
          && c.name.toLowerCase().startsWith(query))
        .map((c) => {
          const end = caret + (draft.slice(caret).match(/^[^\s$]*/)?.[0].length ?? 0);
          const suffix = draft.slice(end);
          const hasSpace = /^\s/.test(suffix);
          const text = `${c.invocation}${c.name}${hasSpace ? "" : " "}`;
          return {
            id: `$${c.name}`,
            label: `$${c.name}`,
            hint: c.argumentHint,
            desc: c.description,
            insert: draft.slice(0, start) + text + suffix,
            caret: start + text.length + (hasSpace ? 1 : 0),
          };
        }),
      commands.length,
    );
  }

  const command = /^\/([^\s]*)$/.exec(draft);
  if (command) {
    const query = command[1].toLowerCase();
    // Local commands come first. A same-named skill remains accessible through `$`.
    const seen = new Set<string>();
    return take(
      commands
        .filter((c) => {
          if (!c.name.toLowerCase().startsWith(query) || seen.has(c.name)) return false;
          seen.add(c.name);
          return true;
        })
        .map((c) => ({
          id: c.name,
          label: `/${c.name}`,
          hint: c.argumentHint,
          desc: c.description,
          insert: `${c.invocation ?? "/"}${c.name} `,
        })),
      commands.length,
    );
  }
  return null;
}
