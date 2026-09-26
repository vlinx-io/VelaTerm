# Planning and Execution

Created: 2026-09-25 10:12

> A planning and execution workflow splits one piece of work between two roles: a planner that prepares and reviews the work, and one or more executors that carry it out. This chapter covers starting a workflow from the menu or from a session, reviewing a task split, choosing working directories, and following the workflow's progress.

## 1. How the workflow runs

A workflow has two roles, each running as its own session in the sidebar:

- **Planning and review.** The planner studies the task and the project, writes the assignment for the executor, and reviews each result. It asks for corrections until the result is acceptable, and accepts it only after checking the actual changes and the verification evidence.
- **Execution.** The executor implements the assignment and reports back. Corrections go to the same executor, which keeps its context between rounds.

The executor appears as a child of the planner in the sidebar. Both sessions run in the conversation view. Messages that the roles send each other show the sender's agent icon, session name and role ("Planning" or "Execution"), so you can follow the exchange in either conversation.

Both roles need an agent that supports the conversation view: Claude Code, Codex, OpenCode, Pi or OMP. The planner and executor may use different agents, models and reasoning efforts.

You can read, answer or interrupt either session at any time. Permission requests from either role still wait for your answer.

## 2. Starting from the menu

Open the context menu of a project, group or session, or its "New Child Session" submenu, and choose "New Plan/Execute Session…". The dialog contains:

- "Create in": where the planner will be created. The executor is created under the planner.
- "Task instructions": describe the task, requirements and acceptance criteria. You can paste images; they are sent with the planner's task and the executor's first assignment.
- "Automatically split into multiple tasks": see §4.
- "Planning and review" and "Execution": the agent ("Session type"), "Model" and "Reasoning effort" for each role. The model and effort choices come from the selected agent. The dialog remembers your last choices for each role.
- "Working directory path" and the directory choice: see §5.

Choose "Create and start" to start the planner. It creates the executor when the plan is ready. Opening or cancelling the dialog creates nothing. The dialog has its own address, so you can copy the link, refresh the page, or use back and forward without losing it.

## 3. Starting from a session

Inside any session that VelaTerm starts, run:

```bash
vspawn --plan-execute \
  --plan-agent claude --plan-model opus --plan-effort high \
  --exec-agent codex --exec-model gpt-5.5 --exec-effort medium \
  "Implement the agreed changes and verify all acceptance criteria"
```

In Claude Code, `/vspawn --plan-execute <task>` does the same; in Codex, use `$vspawn --plan-execute <task>`. The skill writes a self-contained task from the current conversation before starting the workflow. These skills require Vela Skills (Settings ▸ General).

The new planner is created under the session that ran the command. The request first opens the confirmation card described in [Session Spawning & Git Collaboration](session-spawning-and-git_20260709_2041.md) §4, where you can review and change the task and both roles' settings.

| Option | Meaning |
|--------|---------|
| `--plan-agent`, `--plan-model`, `--plan-effort` | Agent, model and reasoning effort of the planner |
| `--exec-agent`, `--exec-model`, `--exec-effort` | Agent, model and reasoning effort of the executor |
| `--claude`, `--codex`, `--model`, `--effort` | Used for the planner when the role-specific options are absent |
| `--split-tasks` | Let the planner propose several tasks (§4) |
| `--worktree-mode none\|shared\|each` | Directory mode for all roles (§5); overrides `--worktree` |
| `--cwd <path>` | Working directory and repository for the workflow |
| `--yes` | Skip the initial confirmation card; a task split is still shown for review |

Without an explicit agent, a planner started from a Claude, Codex, OpenCode, Pi or OMP session uses the same agent; from any other session it uses Claude Code. A model or effort that the selected agent does not support is rejected before anything starts.

The new sessions do not see the initiating conversation. Include the decisions made so far, existing changes, constraints and the required delivery location in the task.

## 4. Splitting into several tasks

Turn on "Automatically split into multiple tasks" in the dialog, or add `--split-tasks` to `vspawn --plan-execute`. The planner then investigates the work and proposes between 1 and 12 independent tasks. No executor starts until you confirm them.

The proposal opens the "Review execution tasks" dialog:

- Select a task in the list to edit its "Session name", "Task instructions", agent, "Model" and "Reasoning effort". Switching between tasks keeps your edits.
- "Remove task" drops a task; "Undo removal" brings it back. At least one task must remain.
- The start button ("Start N sessions") creates one executor per task.
- Closing the dialog keeps the proposal waiting. A "Review execution tasks" link then stays on screen to reopen it. Cancelling the proposal stops the workflow without creating executors.
- The dialog has its own address and survives a refresh; edits that were not yet confirmed are lost on refresh.

When the skill is used with task splitting, it adds `--yes` so that only this final review appears. The review can change each executor's settings, but not the settings of the planner, which is already running.

One planner receives the reports of all executors and reviews each task separately, with its own correction rounds. The workflow as a whole is complete only after every task has been accepted.

Split tasks into pieces that do not edit the same files. Each executor runs its own agent process, so choose the number of tasks with the machine's resources in mind.

## 5. Working directories and worktrees

The directory choice applies to every role in the workflow:

| Choice | `--worktree-mode` | Result |
|--------|-------------------|--------|
| "Current directory" | `none` | All sessions work in the given directory |
| "Shared worktree" | `shared` | The planner and all executors share one new worktree and branch |
| "One worktree per session" | `each` | The planner and each executor get their own worktree and branch |

New worktrees start from the current commit and do not include uncommitted changes. If a worktree cannot be created, the affected session does not start; it does not fall back to another directory. On the command line, `--worktree` alone means `shared`.

The results stay in the worktrees until you merge them. Use "Git ▸ Merge…" on a session to merge a branch (see [Session Spawning & Git Collaboration](session-spawning-and-git_20260709_2041.md) §6). The workflow does not merge, push or delete worktrees on its own.

## 6. Following progress

- The planning conversation shows every report and correction as it happens. While executors work, the planner waits for their reports.
- `vflow status <workflow-id>` shows a workflow's state, its tasks, sessions and current round. The workflow ID appears in the workflow messages.
- `vflow stop <workflow-id>` stops further rounds and asks the running sessions to stop. Stopping the whole workflow stops its tasks; stopping one task does not affect the others.
- `vstat` shows which sessions are working, asking for permission or waiting. A waiting executor has not necessarily finished its task; only the planner's acceptance counts.
- If a role ends its turn without handing the work on, the workflow becomes blocked. Check the conversation, answer what is needed, and continue from the planning session.

The commands are described in [Session Commands](session-commands_20260925_1012.md).

## 7. Example project

The [plan-execute sample](../samples/plan-execute/README.md) contains a small demonstration project with both dialogs, review rounds and recovery scenarios.
