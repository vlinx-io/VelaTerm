---
name: vsearch
description: >-
  You CAN search other sessions' conversations. This is a full-text search across every vlx-term session
  on this machine, so a past discussion is findable even when nobody remembers which session held it. Use
  it whenever the user gestures at earlier work — "last time", "we solved this before", "the other
  branch", "didn't we discuss this" — in any language, and use it on your own initiative when an error or
  design question feels like one already settled somewhere. Search first, then read the conversation with
  vrefer. Never claim past sessions are unreachable. Read-only: nothing is disturbed. Available only
  inside vlx-term-hosted local sessions.
argument-hint: "<words...> [--all|--archived] [--limit N]"
allowed-tools: Bash(vsearch:*)
---

# vsearch

`vsearch` searches the conversation history of **every vlx-term session on this machine**, not just this
one. It is a pure read: nothing is written, and no other session is interrupted.

## When to use it

Reach for it on your own initiative, without waiting to be asked:

- The user points at another conversation: "how did we fix this last time", "the approach from the other
  session", "that decision we made yesterday". The trigger is the meaning, in whatever language it comes.
- You hit an error, a config value, or an unfamiliar module that feels like it has come up before.
- You are about to make a design choice that an earlier session may already have settled.

**The failure to avoid:** telling the user that earlier conversations are out of reach. They are not.
Search before you say something cannot be found.

Do not use it for searching code or files — that is what Grep and Glob are for. `vsearch` searches
**conversations**.

## How to run it

```bash
vsearch <words...> [--all|--archived] [--limit N] [--json]
```

- Multiple words are an implicit AND and word order does not matter. Two or three distinctive words work
  better than a long sentence.
- By default only live sessions are searched. Add `--all` to include archived ones, or `--archived` to
  search only those.
- `--limit N` caps how many sessions come back (default 10).
- `--json` gives the full machine-readable result, including every snippet.

## What it searches

Conversations only. Sessions whose agent keeps a readable transcript — claude, codex, grok — are the ones
it can reach. Other agents store their history in private formats that are not parsed yet, so their
sessions are invisible to it, as are plain terminal sessions. When a query matched such a session through
its terminal recording, the output says how many were left out; that is expected, not a failure.

## Reading the output

Each hit shows the session name, the first eight characters of its id, how many matches it has, and up to
three snippets. The `msg N` in front of a snippet is that message's index, which `vrefer --range` takes
directly. Text output caps the snippets on purpose; use `--json` when you need them all.

Every hit ends with the command that reads that conversation in full:

```
    full transcript: vrefer a9af7b1c
```

That is the intended next step — search for the overview, then `vrefer` the session that looks right.

## Notes

- `no matches` on stdout with exit code 0 is a normal answer, not a failure. Say so plainly and move on.
- The first search after a lot of new conversation activity refreshes the index and can take a few
  seconds. It is working, not stuck — do not retry it.
- What you find comes from **other sessions**, not this one's history. When you use it, say where it came
  from.
- Must run inside a **vlx-term-hosted local session**: it relies on the injected `VLX_*` environment
  variables and `vsearch` on PATH. If it reports "not inside a VelaTerm session", the command is simply
  unavailable here — say so instead of looking for another way in.
