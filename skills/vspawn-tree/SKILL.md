---
name: vspawn-tree
description: >-
  Same as /vspawn, but **opens a dedicated git worktree for the child session** (equivalent to vspawn --worktree).
  Only use when the user explicitly invokes /vspawn-tree or $vspawn-tree; never auto-trigger. This is a real session run by its
  own process in the vlx-term left-panel tree — not an in-process sub-agent, and not a background Task. Available only
  inside vlx-term-hosted sessions.
argument-hint: "[--plan-execute] [--review|--no-review] [--review-agent <agent>] [--review-model <model>] [--review-effort <level>] [--split-tasks] [--worktree-mode <shared|each>] [--plan-agent <agent>] [--plan-model <model>] [--plan-effort <level>] [--exec-agent <agent>] [--exec-model <model>] [--exec-effort <level>] [--cwd <path>] [--title <text>] [--yes] [--agent <agent>] [--model <name>] [--effort <level>] <task>"
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

## Planning and execution mode

When the user requests planning and execution, add `--plan-execute`. Keep this conversation as the
initiator; a new planner organizes work and dispatches to persistent execution sessions. Independent review is disabled by default; add `--review` when the user requests review, which then runs in a separate Review session. Plan only collects context, coordinates and summarizes in either mode.
Add `--split-tasks` when the user requests task decomposition. Do not plan, split or implement the task
in this conversation, and do not use in-process subagents.

For a skill invocation with both flags, **automatically add `--yes`** unless the user explicitly requests
the initial configuration dialog. The planner starts without a launch dialog, but its proposed tasks
still require the user's final review before any executor starts. Never confirm tasks for the user.
Without splitting, retain the ordinary confirmation behavior.

Pass only explicit role selections through `--plan-agent`, `--plan-model`, `--plan-effort`, `--exec-agent`,
`--exec-model` and `--exec-effort`. The backend resolves omitted values from the initiating session and
supported defaults. Only backend chat-capable agents can run the workflow. The final review can edit
each executor's settings, but cannot change the already-started planner's settings or directory mode.

By default, the planner and all executors share the one new worktree requested by this command. For
separate worktrees, pass `--worktree-mode each`: the planner gets its own worktree and each executor gets
another. `--worktree-mode shared` explicitly selects the shared mode. These options require
`--plan-execute` and override the wrapper's legacy `--worktree` flag. For all roles in the current
directory, use the `vspawn` skill with `--worktree-mode none` instead. Keep correction rounds in the same
sessions and worktrees. Include existing modifications and the required delivery location in the task;
new worktrees do not include uncommitted changes. The backend supplies the workflow protocol to both roles.

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

Detect a named agent ("run it in codex / pi / opencode") from the user input (or a leading `--agent <agent>` or
shortcut such as `--codex`) → add `--agent <agent>`, and **don't** write it into the prompt. Accepted names:
`claude`, `codex`, `opencode`, `copilot`, `cursor`, `antigravity`, `cline`, `pi`, `omp`, `crush`, `kimi`, `kiro`,
`grok`, `zoo`; `--claude`, `--codex`, `--copilot`, `--kiro` and `--pi` remain as shortcuts. The default follows the current session type. For planning
and execution, also detect the shared or separate worktree choice described above.

Also detect "no dialog / don't ask me / just start it" (or a leading `--yes` / `-y`) → add `--yes`, which starts
the child session immediately with the default settings instead of showing the initial confirmation card.
This never skips split-task review; split-task skill invocations add it automatically as described above.

Detect a requested session name ("call it X / title it X", or a leading `--title`) → add `--title "<text>"`, up to
80 characters on one line. With `--plan-execute` it names the planner. Without it, the name comes from the start of the task.

Detect a named model ("run it on opus / sonnet / gpt-5.5", or a leading `--model`) → add `--model <name>`, passing
the name through as the user wrote it; vlx-term turns it into the running agent's own model flag. Detect a named
reasoning effort ("think harder / low effort", or a leading `--effort`) → add `--effort <level>`, typically `low`,
`medium`, `high`, `xhigh`, or `max`. Without either flag the child inherits the current session's model and effort.

## Step 3: Run the command (always opens a worktree)

Choose the repository before spawning. If the task or conversation identifies one unambiguous repository
directory, add `--cwd <absolute-path>` so VelaTerm creates the worktree from the intended repository. Otherwise
omit it and use the command's current directory. Do not guess between multiple plausible repositories.

Pass the prompt you expanded in Step 1 **as a single argument** (escaping any quotes inside it correctly), and run
`vspawn-tree` from PATH:

```bash
vspawn-tree [--cwd <absolute-path>] [--title <text>] [--yes] [--agent <agent>] [--model <name>] [--effort <level>] "<expanded self-contained prompt>"
```

For a split-task workflow, use this form, adding explicit role settings when supplied:

```bash
vspawn-tree --plan-execute --split-tasks --yes [--worktree-mode <shared|each>] [--cwd <absolute-path>] "<expanded self-contained prompt>"
```

After it succeeds, give a one-line summary that the launch request was submitted, including the task,
workflow mode and worktree choice. Submission does not prove that the user confirmed the tasks or the
workflow passed. Leave this conversation free; do not poll after submitting.

## Notes

- Must run inside a **vlx-term-hosted session**: it relies on the injected `VLX_*` environment variables and the
  `vspawn-tree` on PATH; if missing it reports "not inside a vlx-term session" and exits.
