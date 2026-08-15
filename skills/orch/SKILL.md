---
name: orch
description: >-
  Act as the lead agent for a multi-part task: decompose it, delegate work items to child sessions
  with routing profiles through the vagent command, then supervise the workers
  to completion (wait, read, follow up, cancel) and integrate the results. Only use when the user
  explicitly invokes /orch; never auto-trigger. Child sessions are real, visible, resumable
  sessions in the VelaTerm left-panel tree, each run by its own process. Available only inside
  VelaTerm-hosted sessions.
argument-hint: "<project or multi-part task>"
disable-model-invocation: true
allowed-tools: Bash(vagent:*) Bash(git:*)
---

# /orch

The user **explicitly invoked `/orch`** to make this session the **lead agent** for the task
below. You may autonomously spawn, inspect, wait on, read, prompt, and cancel **your own child
sessions** with `vagent`. No per-action approval is needed beyond VelaTerm's own spawn confirmation
card, which the user may have enabled.

User input:

$ARGUMENTS

## Your toolbox: vagent

All commands print JSON. Children are addressed by id or unique `--name` and must be your
descendants.

```bash
vagent config                            # profiles, limits, and your current child counts
vagent spawn --profile <p> --name <n> "<self-contained task>"
vagent spawn --agent <kind> --model <m> --effort <e> --name <n> [--worktree] \
             [--permission-mode default|skip|inherit] "<task>"
vagent spawn-status <requestId>          # collect a spawn that answered {"pending":true}
vagent list                              # children with {id,name,kind,model,effort,state}
vagent status <id|name>                  # one child's row
vagent wait <id|name>... [--any|--all] [--timeout <secs>]   # blocks while working/starting
vagent read <id|name> [--full]           # last assistant message (or whole conversation)
vagent prompt <id|name> "<follow-up>"    # sends into the child's conversation
vagent cancel <id|name>                  # interrupts the current turn; session stays alive
vagent cancel --all                      # interrupts every running descendant
vagent diff <id|name>                    # branch diff and commit list against its recorded base
vagent land <id|name> --message "<conventional-subject>"  # squash into the parent
vagent cleanup [--confirm]               # preview or remove verified worktrees and branches
```

For Codex children, use the current ChatGPT model identifiers `gpt-5.6-sol`, `gpt-5.6-terra`,
or `gpt-5.6-luna`. Do not use the short names `sol`, `terra`, or `luna`. Use `low`, `medium`,
`high`, `xhigh`, or `max` for reasoning effort. `ultra` is a lead-agent mode, not a child effort.

States: `working`/`asking`/`waiting` (live), `starting` (booting), `not-started`, `exited`.

Known behavior:

- `spawn` blocks until the child exists and returns its id. `{"pending":true}` means a confirmation
  card is open or the timeout passed. Keep working and collect the child with
  `vagent spawn-status <requestId>` using the id in that response. Never sit in a loop waiting for a
  card: only the user can answer it.
- Unknown model and effort values produce advisory warnings on the confirmation card. The installed
  CLI remains authoritative, so newer values stay launchable.
- `land` applies a direct child's net change to the parent's current branch as one commit. The Lead
  supplies the reviewed Conventional Commit subject. The worker's temporary commits stay private.
- `land` requires a clean worker with at least one commit ahead of its base. It does not rebase the
  worker. A conflict restores the parent and returns the conflicting paths.
- A successful landing stores its change fingerprint and target commit. A retry detects the same
  landing after a crash. The worker can rewrite its branch when the net change stays the same.
- `cleanup` only accepts a clean worker whose fingerprint and target commit still verify. With
  `--confirm`, it removes the worktree and deletes the disposable worker branch.
- `wait` returns a `blocked` array naming children stopped at a permission prompt. Those settled
  without finishing. Tell the user immediately, by session name, then wait again.
- `wait` returns a `failed` array with each errored session's name and provider text. Report every
  failed worker immediately and do not spawn dependent work from it. An empty `failed` array means
  no target's last completed turn reported an error.
- `status` and each `wait.sessions` row include `lastTurnOutcome` (`ok`, `error`, or `unknown`) and
  `lastTurnError` when the agent supplied failure text. `unknown` is honest for unsupported transcript
  formats and must not be treated as success.
- `read` returns a provider failure as an error-role response when the last turn has no assistant reply.
- After `cancel` or a finished turn, the state can lag a few seconds; trust `wait`, not an
  immediate `status`.
- `read` supports Claude, Codex, and Grok children. For other kinds, ask the worker to write its
  summary to a file and read that.
- `diff` also returns `commits`, `commitCount`, and `commitsTruncated` for the child branch. Use the
  commit list to review what the child did before you land it.
- `merge` squashes the child branch into one commit and cherry-picks that commit onto the base
  branch. The base branch gains one commit for each child and no merge commit. `--message` supplies
  the squashed commit message and is required when the branch holds more than one commit. A message
  that names the child branch is refused. A rewritten branch keeps its previous tip under
  `refs/vlx/presquash/<branch>`, returned as `backupRef`.

Never poll with a shell loop (`until ...; do sleep N; done`) or a background job. `vagent wait`
already blocks, and its `blocked` array carries the one case a loop was covering.

## Configuration and limits

Run `vagent config` first. It returns what the user configured in Settings > Orchestration:

- `profiles`: named description/agent/model/effort/worktree bundles, for example `database`,
  `frontend`, `quick-edits`, and `tests`. Read every profile description before routing work. A profile launch
  applies its settings without repeating launch flags. Explicit flags override profile values.
- `limits`: `maxChildren`, `maxParallel`, `maxDepth`, `requireConfirmationAbove`,
  `defaultTimeoutSecs`, and `worktreeCopyPatterns`.
- `counts`: your current live children, how many are `working`, and your own depth.

The limits are enforced by the backend on every spawn, not by this text. A spawn that would cross
one fails with HTTP 429 and a structured body naming `limit`, `limitValue`, and `current`. Nothing
is queued: `vagent wait` for a slot, then spawn again.

## Delegate by profile

Use the profile name on each `vagent spawn` command. Do not put the profile choice in the task text.

For example, after `/orch Implement the billing changes`, route work like this:

```bash
vagent spawn --profile database --name billing-data "Implement the billing persistence changes"
vagent spawn --profile tests --name billing-tests "Add focused tests for the billing changes"
```

An explicit user profile request is authoritative. Use that profile for the requested work item.

Match each work item against every profile description from `vagent config`. Use the best matching
profile when one clearly applies. Ask the user when multiple profiles match equally. Use a generic
fallback only when no profile matches. The fallback can use explicit `--agent`, `--model`, and
`--effort` flags when necessary.

## Worker worktrees start cold

A `--worktree` child gets a fresh checkout. Dependencies are not installed and compiled artifacts do
not exist, so the first build in a worker is slow. Untracked files matching the configured
`worktreeCopyPatterns` are copied in automatically; nothing else local is visible to the worker.
Tell each worker to validate only what it changed, and run the full suites yourself after
integration.

A fresh worktree also has no session-local permission configuration. Use a profile permission mode or
pass `--permission-mode` on every worktree spawn. `inherit` maps the lead's abstract permission level
to the child agent's equivalent. It uses the child agent's configured or native default when the lead
has no stored level. Never select `skip` unless the user selected it in the profile or requested it.

## Land one commit for each child

## Cross-review critical work

Use this review pattern for critical work:

1. Assign implementation to a Fable worker with high effort.
2. Review the result with Sol at xhigh effort. Use `vagent read` and `vagent diff`.
3. Spawn an Opus 5 reviewer with high effort. Name the worker branch in the review task.
4. Reconcile both reviews. Send required fixes through `vagent prompt`.
5. Run `vagent land` with a Lead-written Conventional Commit subject after the worker validates.

## Spawn before you survey

Your reading is not free: while you explore, every worker slot sits idle, and the workers repeat most
of your reading inside their own worktrees anyway. Get them started, then catch up in parallel.

Read only what you need to split the work: the plan document, the file list, and enough structure to
know which files belong to which item. That is usually two or three commands. Spawn, then do your own
reading while the workers run.

Keep each worker prompt under about 2 KB. A long prompt costs a minute of your time to write and
tells the worker things it can read for itself. Name the goal, the exact files, the constraints it
cannot discover, the definition of done, and the path to the plan document. Do not paste the plan.

## Protocol

1. **Read the policy.** Run `vagent config` and read every profile description and returned limit.
2. **Decompose** the task into work items and identify dependencies between them. Read only enough
   to draw the boundaries.
3. **Route** each item by user request first, then by the best description match. Ask about equal
   matches. Use the generic fallback only when no profile matches, and say why. Never write launch
   configuration into the prompt text.
4. **Write short self-contained prompts.** Each child is a brand-new conversation with no memory of
   this one: include the goal, exact file paths, constraints it cannot discover, and the definition
   of done. Point at documents by path instead of pasting them. Instruct code workers to run the
   tests for what they changed and finish with a short summary, naming their branch and commit.
   A worker may make several commits on its own branch, but it must leave nothing uncommitted.
   Tell each worker to keep its branch name, its session name, and the orchestration run out of every
   commit message. Each work item lands on the base branch as exactly one commit. Ask the user before
   you land any work item as more than one commit.
5. **Spawn now, read later.** Start the independent items before you continue your own survey. Then
   do your remaining reading while they work.
6. **Isolate file editors.** Use `--worktree` for any item that edits files; never let two workers
   share a working tree. Split the work by file so two workers never touch the same file.
   Read-only research items can omit the worktree.
7. **Stay out of the shared index.** While workers run, do not `git add`, `git commit`, or `git
   stash` in the main tree. You and the workers share one index there, and a concurrent stage
   silently captures their files. Do your own edits, and stage only after every worker has finished.
8. **Parallelize independent items; gate dependent ones.** Start independent items together, then
   `vagent wait` on prerequisites before spawning dependents. Stay within `maxParallel` from
   `vagent config`, and give each child a distinct `--name`. In everything the user reads, refer
   to each worker by that exact session name (for example `phase4-frontend`), never by internal
   aliases such as "Worker A" or "Worker B": the user identifies sessions by the sidebar names.
9. **Escalate a blocked or failed worker at once.** When `vagent wait` reports a name in `blocked`,
   tell the user in your next message which session is waiting and for what. When it reports a name
   in `failed`, tell the user the provider error text and stop dependent work. Do not wait again on a
   blocked worker until the user has had a chance to answer; you cannot clear the prompt yourself.
10. **Inspect every result** with `vagent read` before using it. If output is incomplete or
    questionable, follow up with `vagent prompt` rather than redoing or silently accepting it.
11. **Never duplicate delegated work.** If a worker fails or stalls past its deadline, `cancel`,
    read what it did, and either re-prompt it or spawn a replacement, never both.
12. **Report failures.** Tell the user what failed and what you did about it; do not substitute
    lower-quality work without saying so.
13. **Integrate only after validation.** Require the worker to commit all changes. Run `vagent land`
    with the Lead's reviewed message. Do not cherry-pick a worker's temporary commits.
14. **Land through each parent.** A nested worker lands into its immediate parent. The top Lead then
    lands its complete net change into the branch that started the orchestration.
15. **Preview cleanup before deletion.** After integration and validation, run `vagent cleanup` without
    `--confirm`. Record each candidate's child name, worktree path, branch, and target commit. Show
    the user the exact deletion list. Report each blocked worktree and its reason.
16. **Require confirmation before cleanup.** Ask the user to confirm the exact cleanup list. Run
    `vagent cleanup --confirm` only after confirmation. It removes each verified worktree and branch.
17. **Verify cleanup.** Re-run `git worktree list` and `git branch --list`. Report what remains.
18. **Give the final report.** State what each worker did, what landed, what cleanup the user
    approved, which worktrees and branches were removed, and what remains.

## Tell the user not to type into worker panes

Say this once, when you report the workers are running: while the run is in progress, slash commands
and unrelated instructions typed into a worker's pane replace its assigned task. A worker cannot hold
two jobs. If the user wants something else done, they should tell you, and you will route it.

Answering a permission prompt in a worker pane is fine and is often exactly what is needed.
