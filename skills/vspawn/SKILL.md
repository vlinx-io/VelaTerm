---
name: vspawn
description: >-
  Explicitly spawn a standalone child session under the current vlx-term session, passing the task in as its
  first message (mirrors spawn_task). With --plan-execute, create a planner, persistent executors and an optional separate reviewer; --split-tasks enables user-confirmed task decomposition. **Runs in the current directory by default, without a git worktree**
  (use vspawn-tree for a worktree). Only use when the user explicitly invokes /vspawn or $vspawn; never auto-trigger.
  This is a real session run by its own process in the vlx-term left-panel tree — not an in-process sub-agent,
  and not a background Task. Available only inside vlx-term-hosted sessions.
argument-hint: "[--plan-execute] [--review|--no-review] [--review-agent <agent>] [--review-model <model>] [--review-effort <level>] [--split-tasks] [--plan-agent <agent>] [--plan-model <model>] [--plan-effort <level>] [--exec-agent <agent>] [--exec-model <model>] [--exec-effort <level>] [--worktree-mode <none|shared|each>] [--worktree] [--cwd <path>] [--title <text>] [--yes] [--agent <agent>] [--model <name>] [--effort <level>] <task>"
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

## Planning and execution mode

When the user requests this workflow or supplies `--plan-execute`, keep the
current session as the initiator. Pass the expanded task to `vspawn --plan-execute`; VelaTerm creates a new
planner, which later dispatches work to its persistent executor. Independent review is disabled by default. Pass `--review` when the user requests review; it then runs in a separate Review session. Plan only collects context, coordinates and summarizes in both modes. Add `--split-tasks` when the user requests automatic decomposition into multiple tasks. The new planner then proposes the split for a separate user review; do not split or execute the task in the initiating conversation. Do not perform the planning
or execution here, and do not use in-process subagents.

For a skill invocation that uses both `--plan-execute` and `--split-tasks`, **automatically add `--yes`**
unless the user explicitly requests the initial configuration dialog. This skips launch configuration and
keeps only the final task-proposal review; the user does not need to supply `--yes` themselves. Never approve
the proposal on the user's behalf. Without task splitting, keep the ordinary confirmation rules below.

Each role has independent `--plan-agent`, `--plan-model`, `--plan-effort` and `--exec-agent`, `--exec-model`,
`--exec-effort` parameters. Optional Review also has `--review-agent`, `--review-model` and `--review-effort`; `--review` or `--no-review` selects whether it exists. Pass only explicit user selections; the backend resolves omitted choices using
the initiating session and supported defaults. With split-task skill invocation, the planner starts with
those settings, and the final review lets the user edit each executor's settings. It does not reconfigure
the already-started planner. Both launch dialogs also offer automatic task splitting. Do not guess a
different model. Only agents supported by the backend chat engine can run this workflow. Agent changes
reset that role's model and effort draft in the dialogs. Ordinary
`--model`, `--effort` and agent flags serve as planner defaults when role-specific flags are absent.

Keep the cwd rules below. Map an explicit directory choice to `--worktree-mode none` (all roles in the
current directory), `--worktree-mode shared` (one new worktree shared by the planner and all executors), or
`--worktree-mode each` (one worktree for the planner and a separate worktree for each executor). This option
requires `--plan-execute` and overrides the legacy `--worktree` flag. Without an explicit mode, `--worktree`
means shared; otherwise all workflow sessions use the requested directory. A displayed launch dialog can
change the mode; the final task review retains it. The selected mode is saved for
both single-task and split-task execution and is retained through retries and correction rounds.
Include existing user modifications and the required delivery location in the task. The planner may edit
its plan and delivery-summary documents; only the executor edits implementation files.

The backend supplies all roles with [the workflow protocol](references/plan-execute.md). Read it when
explaining or diagnosing dispatch, result delivery, correction rounds, cancellation or recovery. In split mode, the user edits and confirms task prompts and execution settings before any executor starts; `--yes` skips only the initial launch confirmation. Each task has its own workflow ID and round. With review enabled, reports go to the shared reviewer with progress notices to Plan; otherwise the full reports go to Plan. Workflow
messages use `vflow` actions and `vtell --report`; they appear with their sender identity in the destination chat. A successful
`vspawn` invocation records a launch request; it does not prove the user confirmed it or the workflow passed.
Report that the request was submitted and leave this session free. Do not poll after submitting.

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
- Naming an agent ("run it in codex / pi / opencode"), or a leading `--agent <agent>` or shortcut such as `--codex`
  → add `--agent <agent>`. Accepted names: `claude`, `codex`, `opencode`, `copilot`, `cursor`, `antigravity`,
  `cline`, `pi`, `omp`, `crush`, `kimi`, `kiro`, `grok`, `zoo`; `--claude`, `--codex`, `--copilot`, `--kiro` and
  `--pi` remain as shortcuts.
- Naming the session ("call it X / title it X"), or a leading `--title` → add `--title "<text>"`, up to 80 characters
  on one line. With `--plan-execute` it names the planner. Without it, the name comes from the start of the task.
- "no dialog / don't ask me / just start it / start it straight away," or a leading `--yes` (`-y`) → add `--yes`.
  This skips the initial launch card, never the split-task proposal review. Split-task skill invocations
  add it automatically as described above.
- Naming a model ("run it on opus / sonnet / gpt-5.5"), or a leading `--model` → add `--model <name>`. Pass the
  name through as the user wrote it; model names belong to the agent that will run, and vlx-term turns it into
  that agent's own model flag.
- Naming a reasoning effort ("think harder / low effort / high effort"), or a leading `--effort` → add
  `--effort <level>`. Levels are typically `low`, `medium`, `high`, `xhigh`, `max`; explicit effort selections are rejected if the agent does not expose that setting.

Defaults: **no** worktree (run in the current directory), **follow** the current session type, **inherit** the
current session's model and effort, and **show** the confirmation card when the user has "Confirm before spawn"
turned on, except for the split-task skill rule above.

## Step 3: Run the command

Choose the command directory before spawning. If the task or conversation identifies one unambiguous project or
repository directory, add `--cwd <absolute-path>` so the child starts there and a requested worktree is created
from that repository. Otherwise omit it and let `vspawn` use the command's current directory. Do not guess between
multiple plausible repositories.

Pass the prompt you expanded in Step 1 **as a single argument** (escaping any quotes inside it correctly), and run
`vspawn` from PATH:

```bash
vspawn [--worktree] [--cwd <absolute-path>] [--title <text>] [--yes] [--agent <agent>] [--model <name>] [--effort <level>] "<expanded self-contained prompt>"
```

For a split-task skill invocation, use this form, adding only the user's explicit role and directory choices:

```bash
vspawn --plan-execute --split-tasks --yes [--worktree-mode <none|shared|each>] [--cwd <absolute-path>] [--plan-agent <agent>] [--exec-agent <agent>] "<expanded self-contained prompt>"
```

After it succeeds, give a one-line summary that the launch request was submitted, including the task and whether it uses planning and execution mode. A submitted request may still be awaiting confirmation.

## Notes

- Must run inside a **vlx-term-hosted session**: it relies on the injected `VLX_*` environment variables and the
  `vspawn` on PATH; if missing it reports "not inside a vlx-term session" and exits.
