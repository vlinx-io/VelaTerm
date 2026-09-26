# Session Spawning & Git Collaboration

Created: 2026-07-09 20:41

Updated: 2026-09-25 10:21

> Two features that work as a pair: spawning subtasks into independent child sessions (optionally in isolated git worktrees), and a graphical merge that brings parallel branches back together. Together they let several agents work on the same repository in parallel.

## 1. What spawning is

From inside any session, you can split off a subtask. VelaTerm creates a new child session **under** the current node (the tree becomes hierarchical: parent → child → grandchild), optionally gives it an isolated git worktree, and sends the task description as the new session's **first message**. The new agent starts working as soon as it launches, without further explanation.

The result is a real session in the tree, with its own process, that you can watch, answer, reopen and resume at any time. It is not a hidden background task.

For work that should be planned, carried out and reviewed by separate agents, use a planning and execution workflow instead; see [Planning and Execution](planning-and-execution_20260925_1012.md).

## 2. Ways to spawn

| Entry | Who uses it | Worktree |
|-------|-------------|----------|
| Command `vspawn "task"` | You or a script, in any session | Off by default; `--worktree` turns it on |
| Command `vspawn-tree "task"` | Same | Always on |
| `/vspawn task` in Claude Code, `$vspawn task` in Codex | You ask the agent; it expands the task into a self-contained prompt first | Off by default |
| `/vspawn-tree task` in Claude Code, `$vspawn-tree task` in Codex | Same | Always on |

The two commands are available in every session that VelaTerm starts. The agent skills require **Vela Skills**: choose "Install" in Settings ▸ General, then start a new Claude Code or Codex conversation so the agent picks them up. Agents call these skills only when you ask for them.

A new session does not see the conversation it came from. When you use the skill, the agent writes the goal, relevant files, conclusions so far, constraints and acceptance criteria into the task. When you run the command yourself, include them in the task text.

## 3. Directory, agent and model

- **Directory.** The child starts in the directory where the command runs, which is also the repository used for a worktree. Pass `--cwd <path>` to use another project, for example `vspawn-tree --cwd /work/project "fix the parser"`. This matters in a collection, which has no project folder of its own. The skills add `--cwd` when the conversation clearly identifies one repository.
- **Agent.** The child uses the same agent as the current session; a plain terminal spawns Claude Code. `--claude`, `--codex`, `--copilot` or `--kiro` choose another agent, and the confirmation card offers every agent type.
- **Model and effort.** `--model <name>` and `--effort <level>` set the child's model and reasoning effort, for example `vspawn --model opus --effort high "port the parser"`. Model names belong to the agent that will run. A setting the agent does not support is rejected before the session is created. Without these options, a child of the same agent type inherits the parent's launch arguments and permission setting; a child of another type uses that agent's defaults from Settings ▸ Agents.
- **View.** A child that supports the conversation view opens in the same view as a parent that supports it, and otherwise in the agent's default view.

All options are listed in [Session Commands](session-commands_20260925_1012.md) §3.

## 4. Confirm before spawn

By default, every spawn request first shows a confirmation card, titled "Start child session". The card does not take focus from the session you are working in, and a system notification ("Child session awaiting confirmation") tells you it is waiting.

- The card shows the requesting session and its directory ("Requested by"), the "Task instructions" that become the child's first message, the "Run settings" ("Session type", "Model", "Reasoning effort"), and the "Working directory" choice: "Current directory" or "Separate worktree".
- Model suggestions come from the selected agent. If no list is available, you can type a model identifier. Leaving a field on "Agent default" uses the agent's own setting.
- A plain terminal opens in the working directory; it does not run the task instructions.
- "Start session" (or ⌘Enter / Ctrl+Enter) starts the child; "Cancel" discards the request. Further requests wait in a queue, and the card shows how many remain ("N more to review").

To start requests without the card, turn off "Confirm before spawn" in Settings ▸ Behavior, or add `--yes` to a single command.

## 5. Worktrees: parallel work without conflicts

With a worktree, the child session works on a new branch in its own directory, isolated from the main workspace and from other children. This is the recommended setup when several agents change the same repository at once. New worktrees start from the current commit and do not include uncommitted changes. If a worktree cannot be created for a spawned session, the session uses the original directory.

A session that works in a worktree shows a small badge on its icon in the sidebar; hover it to see the worktree name and path.

Other ways to work with worktrees:

- **"New Worktree Session…"** in the project, group or session menu opens the create dialog with the "Worktree" option set to "New". Enter a "Worktree name", which is used for the directory and the branch. The dialog can also attach the session to an "Existing" worktree, or use "None".
- **"Move to Worktree…"** in a group's menu binds the group to a new or existing worktree. Sessions created in the group from then on use that worktree; sessions already in it keep their directory. A group with a worktree also offers "Group Info".

The **Git** submenu of a session or group whose directory is a git repository contains:

| Item | What it does |
|------|--------------|
| "View changes…" | Shows the working tree's changes with a file list and diff |
| "Merge…" | Opens the merge dialog (§6) |
| "Copy worktree path", "Open worktree folder" | Available for sessions and groups bound to a worktree |
| "Delete worktree…" | Choose a worktree of this repository and delete its directory from disk; "Force delete (discard uncommitted changes)" removes it even with uncommitted changes |

After a bound worktree has been deleted, the session or group menu offers "Convert to normal session" or "Convert to normal group", which removes the binding so the session starts in the project directory again.

When you delete a session that has worktrees in its subtree, the confirmation offers to remove those worktrees as well.

## 6. Merge: graphical Git merge

When the work is done, merge it. For any session or group whose directory is a git repository, with or without a worktree, choose "Git ▸ Merge…" to open the "Merge branches" dialog:

- **Choose both branches.** Pick a "Source branch" and a "Target branch"; "Swap direction" reverses them. The same dialog merges a child's branch back into the main branch, or brings the main branch into a child to update it.
- The dialog lists the changes the merge brings into the target branch before you confirm.
- The merge runs in the working tree where the target branch is checked out. If the target branch is not checked out anywhere, the merge cannot run; check it out first.
- If the source branch's working tree has uncommitted changes, they are committed first with the message you enter ("Commit & merge").
- If the merge has conflicts, VelaTerm leaves them in place and tells you where to resolve them. Resolve them in that worktree's terminal (or ask an agent to), then commit.

Merging does not delete the worktree. Remove it afterwards with "Git ▸ Delete worktree…" once you no longer need it.

## 7. A typical workflow

1. In the main session, discuss the task with Claude Code, then run `/vspawn-tree refactor the checkout module` (requires Vela Skills).
2. Review the task on the confirmation card and choose "Start session". The child starts in its own worktree and appears in the sidebar under its parent, with a green status dot while it works.
3. Continue in the main session. When the child asks something or finishes, its dot changes and a notification appears.
4. Review the child's work, then choose "Git ▸ Merge…" to merge its branch into the main branch.
5. Remove the worktree with "Git ▸ Delete worktree…", then archive the child session to keep a record.
