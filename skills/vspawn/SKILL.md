---
name: vspawn
description: >-
  Explicitly spawn a standalone child session under the current vlx-term session, passing the task in as its
  first message (mirrors spawn_task). **Runs in the current directory by default, without a git worktree**
  (use vspawn-tree for a worktree). Only use when the user explicitly invokes /vspawn or $vspawn; never auto-trigger.
  This is a real session run by its own process in the vlx-term left-panel tree — not an in-process sub-agent,
  and not a background Task. Available only inside vlx-term-hosted sessions.
argument-hint: "[--worktree] [--yes] [--claude|--codex] <task>"
disable-model-invocation: true
allowed-tools: Bash(vspawn:*)
---

# /vspawn

The user **explicitly invoked `/vspawn` (Claude) or `$vspawn` (Codex)** to request spawning a **standalone child
session** under the current **vlx-term** session,
passing the task in as its first message. The new child session runs as a **brand-new claude/codex process**, and
**by default runs in the current directory without a worktree** (to open a dedicated git worktree for the child
task, use `/vspawn-tree`, or add `--worktree` to this command).

> ⚠️ This is **not** an in-process sub-agent, and **not** a background Task: it is a real, visible, interactive,
> resumable session tab in the vlx-term left-panel tree, run by its own process, leaving the current session
> undisturbed.

User input:

$ARGUMENTS

## Step 1: Expand the user input into a "self-contained" rich prompt (critical)

The new child session is a **brand-new conversation with no memory of this one**. **Do not** forward the user's
one-liner verbatim — as if handing the job to someone with zero context, **use what is known from the current
conversation** to expand it into a task description that stands on its own. Write whichever of the following
**apply — don't pad**:

- **Goal**: what needs to be achieved.
- **Relevant files and paths**: spell them out (absolute or relative), don't make them guess.
- **Established facts / conclusions**: key information from this conversation that they wouldn't know (e.g. an
  already-located root cause, an already-agreed convention).
- **Constraints**: tech stack, things that must not change, code-style requirements.
- **Acceptance criteria**: what counts as done.

Don't write "see above / as mentioned / continuing from earlier" — the new session can't see any of this.
(If the user's input is already complete, or they explicitly say "keep it short," don't over-expand.)

## Step 2: Detect worktree / type options

Detect the following **intents** from the user input; when detected, turn them into the corresponding command
flag, and **do not** write that instruction into the prompt:

- "want a worktree / open a separate workspace / a separate branch," or a leading `--worktree` → add `--worktree`
  to the command.
- Specifying claude / codex, or a leading `--claude` / `--codex` → add the matching flag.
- "no dialog / don't ask me / just start it / start it straight away," or a leading `--yes` (`-y`) → add `--yes`.
  The child session then starts immediately with the default settings, with no confirmation card to click.

Defaults: **no** worktree (run in the current directory), **follow** the current session type, and **show** the
confirmation card when the user has "Confirm before spawn" turned on.

## Step 3: Run the command

Pass the prompt you expanded in Step 1 **as a single argument** (escaping any quotes inside it correctly), and run
`vspawn` from PATH:

```bash
vspawn [--worktree] [--yes] [--claude|--codex] "<expanded self-contained prompt>"
```

After it succeeds, give the user a one-line summary: "Spawned a child session in vlx-term: <brief task summary>".

## Notes

- Must run inside a **vlx-term-hosted session**: it relies on the injected `VLX_*` environment variables and the
  `vspawn` on PATH; if missing it reports "not inside a vlx-term session" and exits.
