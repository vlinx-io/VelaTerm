---
name: vspawn-tree
description: >-
  Same as /vspawn, but **opens a dedicated git worktree for the child session** (equivalent to vspawn --worktree).
  Only use when the user explicitly invokes /vspawn-tree or $vspawn-tree; never auto-trigger. This is a real session run by its
  own process in the vlx-term left-panel tree — not an in-process sub-agent, and not a background Task. Available only
  inside vlx-term-hosted sessions.
argument-hint: "[--claude|--codex] [--model <model>] [--effort <level>] [--name <name>] <task>"
disable-model-invocation: true
allowed-tools: Bash(vspawn-tree:*)
---

# /vspawn-tree

The user **explicitly invoked `/vspawn-tree` (Claude) or `$vspawn-tree` (Codex)** to request spawning a **standalone
child session** under the current **vlx-term** session,
passing the task in as its first message. The only difference from `/vspawn`: it **opens a dedicated git worktree
for the child session** (a separate workspace/branch), suited to cases where you don't want to pollute the current
workspace and want the child task to run on an isolated branch.

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

## Step 2: Detect type options

Detect "specifying claude / codex" from the user input (or a leading `--claude` / `--codex`) → add the matching
flag, and **don't** write it into the prompt. The default follows the current session type. (This command always
opens a worktree, so there's nothing to detect there.)

Also detect launch configuration: naming a model ("use fable", "with opus") → add `--model <model>`; naming a
reasoning effort ("high effort", "xhigh") → add `--effort <level>`; asking for a session name → add
`--name <name>`. These persist on the child session and map to agent-specific launch flags; do not write them
into the prompt. Defaults keep the agent's own model/effort.

## Step 3: Run the command (always opens a worktree)

Pass the prompt you expanded in Step 1 **as a single argument** (escaping any quotes inside it correctly), and run
`vspawn-tree` from PATH:

```bash
vspawn-tree [--claude|--codex] [--model <model>] [--effort <level>] [--name <name>] "<expanded self-contained prompt>"
```

After it succeeds, give the user a one-line summary: "Spawned a child session in vlx-term (dedicated worktree):
<brief task summary>".

## Notes

- Must run inside a **vlx-term-hosted session**: it relies on the injected `VLX_*` environment variables and the
  `vspawn-tree` on PATH; if missing it reports "not inside a vlx-term session" and exits.
