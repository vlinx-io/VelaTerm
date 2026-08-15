# Session Spawning & Git Collaboration

Created: 2026-07-09 20:41
Updated: 2026-08-14

> Two features that work as a pair: spawning subtasks into independent child sessions with `vspawn` (optionally in isolated git worktrees), and a graphical merge to bring parallel branches back together. Together they make "several agents working the same repo in parallel" an everyday workflow.

## 1. What spawning is

From inside any session, split a subtask off: VelaTerm creates a new child session **under** the current node (the tree becomes hierarchical: parent → child → grandchild), optionally gives it an isolated git worktree, and feeds the task description in as the new session's **first message** — the new agent starts working the moment it boots, no re-explaining needed.

What you get is a real session in the tree — its own process, fully interactive, resumable — that you can watch or step into at any time. It is not a hidden background task.

## 2. Four entry points

| Entry | Who uses it | Worktree |
|-------|-------------|----------|
| Terminal command `vspawn "task"` | You, in any session | Off by default; `--worktree` enables |
| Terminal command `vspawn-tree "task"` | Same | Always on |
| `/vspawn task` in Claude or `$vspawn task` in Codex | The agent spawns it (and expands the task into a rich self-contained prompt) | Off by default |
| `/vspawn-tree task` in Claude or `$vspawn-tree task` in Codex | Same | Always on |

The two terminal commands are injected into every session's PATH automatically — zero install.

Both commands accept optional launch configuration for the child session:

- `--model <model>` selects the child's model (for example `vspawn --claude --model fable "task"`).
- `--effort <level>` selects the reasoning effort (for example `high` or `xhigh`).
- `--name <name>` sets the child session's name instead of deriving it from the task text.
- `--agent-args "<raw args>"` replaces the per-agent default launch arguments from Settings.

Model and effort persist on the child session, survive restart and resume, and are translated to
agent-specific flags at launch: Claude uses `--model` and `--effort`; Codex uses `-m` and
`-c model_reasoning_effort=`. Other agent types currently ignore these settings. When omitted, the
agent's own defaults apply.

> **Prerequisite for agent skills:** enable **Vela Skills** in Settings ▸ General. This installs `vspawn`, `vspawn-tree`, `vopen`, and `orch` into both `~/.claude/skills/` and the Codex skills directory (`$CODEX_HOME/skills` when set, otherwise `~/.codex/skills`); they're kept up to date automatically on app upgrades. After enabling, start a new Claude or Codex thread so the agent picks them up. Without this, the terminal commands still work, but the conversation skills are unavailable.

### Supervising children with `vagent`

The `vagent` terminal command (also injected into every session's PATH) lets a parent session
supervise the children it spawned. All output is JSON, and a session can only reach its own live
descendants, addressed by id or unique name:

```text
vagent spawn --profile <p> --name <n> "<task>"
vagent spawn --agent <kind> --model <m> --effort <e> --name <n> [--worktree] "<task>"
vagent config
vagent spawn-status <requestId>
vagent list
vagent status <id|name>
vagent wait <id|name>...
vagent read <id|name>
vagent prompt <id|name> "<follow-up>"
vagent cancel <id|name> | --all
vagent diff <id|name>
vagent land <id|name> --message "<conventional-subject>"
vagent cleanup [--confirm]
```

`spawn` blocks until the child exists (respecting the confirmation card) and returns its session
id. If the confirmation card remains open, collect the request later with `spawn-status`. Unknown
model or effort values produce a warning, but the installed CLI remains authoritative and can still
launch newer values.

`wait` blocks while a child is working. Its result includes `blocked` permission prompts and
`failed` workers. A failed worker includes its provider error text. `status` and each wait row expose
the last turn outcome as `ok`, `error`, or `unknown`. `read` returns the child's last assistant reply
(Claude, Codex, and Grok), or an error-role response when the last turn failed. `prompt` sends a
follow-up into the child's conversation. `cancel` interrupts its current turn without closing the
session, and `cancel --all` interrupts every running descendant.

`config` prints the routing profiles, the spawn limits, and the caller's current child counts, all
from Settings > Orchestration. A lead agent reads each profile description before it routes work.
`--profile database` then launches a child with that profile's agent, model, effort, and worktree
choice; an explicit flag still overrides the profile.

`cleanup` lists the worktrees of children that have finished and hold no uncommitted changes;
`cleanup --confirm` removes only verified landed worktrees and their disposable branches. A running
child, an uncommitted worktree, or an unverified landing is blocked.

`land` requires a Lead-written Conventional Commit subject. It applies the direct child's net
change to the parent's current branch as one commit. Temporary worker commits do not enter the
parent history.

The worker must have a clean worktree and at least one commit ahead of its base. Landing does not
rebase the worker. A conflict restores the parent and reports the conflicting paths.

VelaTerm stores the worker change fingerprint and the resulting parent commit. This record makes
landing retries and cleanup safe after a crash. Nested workers use the same boundary at each level.

### Orchestration settings

Settings ▸ Orchestration is one place for everything a lead agent needs:

- **Profiles**: named bundles of description, agent, model, effort, worktree, and permission mode. Four ship by
  default: `database`, `frontend`, `quick-edits`, and `tests`. Add, edit, and delete them freely.
- **Limits**, enforced by the app on every spawn rather than by prompt text: `maxChildren` live
  children per lead, `maxParallel` children working at once, `maxDepth` for child-of-child nesting,
  the child count above which the confirmation card appears even when confirmation is off, and the
  default `vagent wait` timeout. A spawn that would cross a limit fails with a message naming the
  limit and the current count, so the agent waits and retries instead of failing blind.
- **Worktree copy patterns**: globs such as `docs/plans/**` for untracked or ignored files copied
  from the repository root into each new worktree, so a worker can see local-only notes. Build
  output such as `node_modules` is never copied, and workers still build from scratch.

### The `/orch` skill

`/orch <task>` turns the current agent session into a lead agent: it reads the configured
profiles and limits with `vagent config`, decomposes the task, routes each work item to a profile,
waits on and reviews each result, follows up where needed, and integrates the outcome. `/vspawn`
stays manual and single-shot; only `/orch` manages workers autonomously.

## 3. Confirm before spawn

By default every spawn first shows a confirmation card (top-right, non-modal, doesn't steal focus):

![Spawn confirmation card](../assets/manuals/spawn-confirm.png)

- Editable fields: the **prompt** (multi-line), the **agent type** (defaults to the parent's), **separate git worktree**, and the child's **model** and **effort** (empty keeps the agent's own defaults).
- "Launch" starts the child session; "Cancel" drops the request. When an agent spawns several at once, cards are handled one at a time (the remaining count shows on the card).
- If you'd rather skip confirmation entirely, turn off "Confirm before spawn" in Settings ▸ Behavior.

## 4. Worktrees: parallel without stepping on each other

With the worktree option on, the child session works on a new branch in its own working directory — isolated from the main workspace and from other children. That's the right shape for multi-agent parallelism. Management lives in the session context menu under "Worktree ▸": view changes, copy / open the worktree folder, delete the worktree (with optional force to discard uncommitted changes).

Spawning from a non-git directory still works; it just falls back to sharing the parent's directory, with no worktree.

## 5. Merge: graphical Git merge

When the work is done, bring it home. Any session whose working directory is a git repo (worktree or not): right-click → "Git ▸ Merge…" opens the merge dialog:

- **You pick both source and target branches**, and can swap direction — merge a child's branch back into main, or pull main into the child to refresh its baseline, all from the same dialog.
- The merge executes in the working tree where the target branch is checked out; if the source tree has uncommitted changes, they're committed first so nothing is lost.
- On conflict, the scene is left in place for you (or an agent) to resolve in the terminal and continue.

Merging does not auto-delete the worktree — clean it up afterwards via "Worktree ▸ Delete worktree…" once you're sure.

## 6. A typical workflow

1. In the main session, have claude analyze the task, then `/vspawn-tree refactor the checkout module` to spawn it (requires Vela Skills — see §2).
2. Review the prompt on the confirmation card and hit Launch — the child starts in its own worktree, visible in the sidebar under its parent, dot turning green.
3. You keep working in the main session; when the child asks something or finishes, its dot turns yellow and a notification fires.
4. Step in to review, then right-click → "Git ▸ Merge…" to merge its branch back into main.
5. "Worktree ▸ Delete worktree…" to clean up, then archive the child session for the record.
