---
name: vorch
description: >-
  Orchestrate a task across several child sessions at once: split it into self-contained subtasks, propose
  them in one call, and let the user review every entry in a dialog before any of them starts. Each child is
  a real, visible vlx-term session run by its own process, and a coordinator tab follows their progress so
  this session stays free. Only use when the user explicitly invokes /vorch or $vorch; never auto-trigger.
  Available only inside vlx-term-hosted sessions.
argument-hint: "<task to split across several sessions>"
disable-model-invocation: true
allowed-tools: Bash(vorch:*)
---

# /vorch

The user **explicitly invoked `/vorch` (Claude) or `$vorch` (Codex)** to have one task carried out by
**several child sessions in parallel** under the current **vlx-term** session. You split the task, propose
the pieces in one call, and the user confirms them in a dialog. Nothing starts until they do, and they can
edit or drop any entry there.

> ⚠️ These are **not** in-process sub-agents and **not** background Tasks: every entry becomes a real,
> visible, interactive session tab in the vlx-term left-panel tree, run by its own process. The current
> session is not occupied by any of it.

User input:

$ARGUMENTS

## Step 1: Split the task into self-contained subtasks (critical)

Each child session is a **brand-new conversation with no memory of this one**. Whatever it needs to know
must be in its prompt. For every subtask write, as applicable:

- **Goal**: what this one piece must achieve, and where its boundary is.
- **Relevant files and paths**: spell them out; do not make it search for what you already know.
- **Established facts / conclusions**: what this conversation already found (root causes, agreed
  conventions, decisions) that the child could not know.
- **Constraints**: what must not change, style rules, interfaces it must keep.
- **Acceptance criteria**: what counts as done, including how to verify it.

Never write "see above", "as discussed", or "continuing from earlier": the child cannot see any of this.

How to split well:

- Pieces should be **independent**: each one can finish without waiting on another, and two of them do not
  edit the same file. If two pieces must touch one file, merge them into one entry.
- Pieces should be **comparable in size** and each worth a whole session. Three to six is typical; a piece
  that takes two minutes belongs inside a larger one, and a piece that would take a day should be split.
- Do not add an "integration" or "review" entry that depends on the others finishing. Integration is done
  afterwards, by the user or by this session, once the results exist.

A poor split: "1. backend, 2. frontend, 3. tests" for a feature whose tests cover both — the test entry
cannot start until the others finish, and all three edit the same files. A good split: one entry per
independent module, each with its own tests, and a note in every prompt about the shared interface.

## Step 2: Choose settings

Detect these intents in the user input and set the matching fields; do not write them into the prompts:

- **worktreeMode**: `each` (default) gives every child its own git worktree; `shared` puts them all in one
  new worktree; `none` runs them in the current directory. Use `none` when the task does not touch a git
  repository or when the pieces only read.
- **defaults.kind**: `claude`, `codex`, `opencode`, `copilot`, `cursor`, and so on. Defaults to this
  session's own type.
- **defaults.model** / **defaults.effort**: only when the user asked for a specific model or effort. Leave
  them out otherwise; the dialog shows the agent's own default.
- Per-entry overrides (`kind`, `model`, `effort`, `worktree`) only when the user asked for one piece to
  differ.

## Step 3: Run the command

The proposal is JSON on **stdin**, never command-line arguments: prompts are multi-line prose. Use a
quoted heredoc so nothing inside is expanded:

```bash
vorch <<'JSON'
{
  "title": "Short name for the whole run",
  "worktreeMode": "each",
  "defaults": { "kind": "claude" },
  "agents": [
    { "name": "Short tab label", "prompt": "Self-contained task text…" },
    { "name": "Another label",   "prompt": "…", "worktree": false }
  ]
}
JSON
```

`title` names the group the sessions are placed in, and `name` labels each tab; keep both short. At most
twelve entries are accepted.

After it succeeds, tell the user in one line that the proposal is waiting in the vlx-term dialog, with the
number of entries. Then stop: do not poll, and do not wait for the children.

## Afterwards

- A coordinator tab named "Progress" follows the run and prints each status change; when every child has
  stopped it prints a summary and posts a notification. You do not need to watch anything.
- To check on the run yourself when the user asks, run `vstat latest`.
- To use a child's result, read its conversation with `vrefer <session>`.

## Notes

- Must run inside a **vlx-term-hosted session**: it relies on the injected `VLX_*` environment variables
  and `vorch` on PATH; if missing it reports "not inside a VelaTerm session" and exits.
- The command exits as soon as the proposal is recorded. A cancelled dialog is not an error you will see.
