# Session Commands

Created: 2026-09-25 10:12

> VelaTerm adds a set of short commands to every session it starts. You and your agents use them to start other sessions, open files, read and search other conversations, check which sessions are busy, send messages between sessions, and wait for long commands. This chapter is a reference for each command and for the matching agent skills.

## 1. Where the commands work

The commands are on the PATH of every session that VelaTerm starts: plain terminals and agent sessions, in both the terminal view and the conversation view, including sessions on a remote VelaTerm server. They need no installation. Outside such a session they stop with "not inside a VelaTerm session".

Run a command with `--help` to print its usage.

| Command | Purpose |
|---------|---------|
| `vspawn`, `vspawn-tree` | Start a child session with a task |
| `vopen` | Open files in document tabs and web addresses in browser tabs |
| `vrefer` | Read another session's conversation, or ask a question about it |
| `vsearch` | Search the conversations of all sessions |
| `vstat` | Show which sessions are working, asking or waiting |
| `vtell` | Send a message to another session |
| `vrun` | Run a long command and wait for it to finish |
| `vkb` | Query the knowledge base and the code graph |
| `vflow` | Control planning and execution workflows |

Several commands take a **session reference**: a full session ID, an ID prefix of at least 8 characters, the exact session name, or a unique part of the name. Put names that contain spaces in quotes. When a reference matches several sessions, the command lists the candidates.

## 2. Agent skills (Vela Skills)

Settings ▸ General ▸ "Vela Skills" installs matching skills into Claude Code and Codex; "Reinstall" refreshes them. Start a new Claude or Codex conversation afterwards so the agent picks them up. In Claude Code a skill is called with `/name`, in Codex with `$name`.

| Skill | Called by | What it does |
|-------|-----------|--------------|
| `vspawn`, `vspawn-tree` | You | Writes a self-contained task from the current conversation and starts a child session or a planning and execution workflow |
| `vopen` | You | Opens files or web addresses |
| `vkb` | You | Queries the knowledge base and the code graph |
| `vrefer`, `vsearch`, `vask` | You or the agent | Reads, searches or asks about other conversations |
| `vstat` | You or the agent | Reports which sessions are busy |
| `vtell` | You or the agent | Sends a message to another session |

Skills marked "You" run only when you call them; the others can also be used by the agent on its own when you mention another session or earlier work. `vask` exists only as a skill; it runs `vrefer <session> --ask "<question>"`.

## 3. vspawn and vspawn-tree

```text
vspawn [--worktree] [--cwd <path>] [--yes] [--claude|--codex|--copilot|--kiro] [--model <name>] [--effort <level>] <task description...>
```

Starts a new session under the current one and sends the task as its first message. `vspawn-tree` is the same command with a separate git worktree.

| Option | Meaning |
|--------|---------|
| `--worktree` | Give the child its own worktree and branch |
| `--cwd <path>` | Working directory and repository for the child |
| `--yes` | Start without the confirmation card |
| `--claude`, `--codex`, `--copilot`, `--kiro` | Agent of the child; by default the current session's agent (Claude Code for plain terminals) |
| `--model <name>`, `--effort <level>` | Model and reasoning effort of the child |
| `--plan-execute` and related options | Start a planning and execution workflow |

Details: [Session Spawning & Git Collaboration](session-spawning-and-git_20260709_2041.md) and [Planning and Execution](planning-and-execution_20260925_1012.md).

## 4. vopen

```text
vopen <file|url>...
```

Opens each argument in the center pane. Markdown files open in the Markdown editor, images in the image viewer, and other files in the code editor. Web addresses (`http` or `https`) open in a browser tab, which is available in the desktop app only. Relative paths are resolved against the current directory. See [Document & Browser Tabs](document-and-browser-tabs_20260709_2041.md).

## 5. vrefer and vask

```text
vrefer <session> [--last N] [--range A:B] [--json]
vrefer <session> --ask "<question>" [--with <agent>] [--timeout N]
vrefer --list [--json]
```

Prints another session's conversation. The other session does not need to be running, and nothing is sent to it.

| Option | Meaning |
|--------|---------|
| `--last N` | Only the last N messages |
| `--range A:B` | Messages A up to, but not including, B (counting from 0) |
| `--ask "<question>"` | Have an agent read the conversation and answer the question, instead of printing the conversation |
| `--with <agent>` | Agent that answers: `claude`, `codex`, `opencode`, `pi`, `omp`, `cursor`, `copilot` or `grok`; chosen automatically when omitted |
| `--timeout N` | Seconds allowed for the answer (default 120) |
| `--list` | List the sessions that can be read |
| `--json` | Machine-readable output |

`--ask` starts a separate agent process for the answer and brings back only the answer, so a long conversation does not fill the current context. Settings ▸ Behavior ▸ "Session reference context" chooses how the conversation is handed to the answering agent: "Full transcript" (default) or "Summarize first", which first condenses it with the "Summary agent", model and effort chosen there. If no answer can be produced, `vrefer` explains why and prints the conversation instead.

Conversations can be read for Claude Code, Codex, OpenCode, Pi, OMP, Grok Build and Kiro (text records only). Other session types, and sessions whose agent conversation has not been recorded yet, cannot be read.

## 6. vsearch

```text
vsearch <words...> [--session <session>] [--all|--archived] [--limit N] [--json]
```

Searches the conversations of all sessions on the connected VelaTerm. All words must match, in any order.

| Option | Meaning |
|--------|---------|
| `--session <session>` | Search only this session |
| `--all` | Include archived sessions (by default only sessions that are not archived) |
| `--archived` | Search archived sessions only |
| `--limit N` | Maximum number of matching sessions (default 10) |
| `--json` | Machine-readable output |

Each result shows the session name, its ID prefix, the number of matches, a few excerpts with message numbers, and the `vrefer` command that reads the whole conversation. A message number can be used with `vrefer --range`. Results come from agent conversations only; matches that exist only in terminal recordings are not returned. "no matches" is a normal result.

## 7. vstat

```text
vstat [--wait] [--follow] [--timeout N] [--json]
```

Shows the state of each session: `working`, `asking` (waiting for a permission or an answer), `waiting` (finished its turn) or `unknown`. The state shows activity, not whether a task was completed.

| Option | Meaning |
|--------|---------|
| `--wait` | Wait for the next change instead of answering at once |
| `--follow` | Keep printing changes until every agent is idle |
| `--timeout N` | Seconds one wait may take (default 60, maximum 300) |
| `--json` | Machine-readable output |

An optional first argument filters by an orchestration ID from older VelaTerm versions; planning and execution workflows use `vflow status` instead.

## 8. vtell

```text
vtell <session> [--steer] [--message-id msg-UUID] [message...]
vtell [planner-session] --report --round N [--message-id msg-UUID] [message...]
```

Sends a message to another session. Without message arguments, the text is read from standard input. The recipient sees the sender's agent icon and session name in its conversation, also after reopening it.

- The recipient must be a Claude Code, Codex, OpenCode, Pi or OMP session in the conversation view. Archived sessions and the sending session itself are rejected.
- An idle recipient starts a new turn. A busy recipient queues the message until its turn ends; `--steer` adds it to the running turn instead.
- A message can contain up to 65,536 bytes of text.
- The command prints a receipt: `sent`, `queued` or `steered`. `blocked` means the recipient is waiting for a permission or an answer. A receipt confirms delivery, not that the recipient finished the work.
- The command prints a message ID. To retry a delivery whose outcome is uncertain, repeat the command with `--message-id` and the same text; the message is not delivered twice.
- `--report --round N` is used by executors in a planning and execution workflow to submit their result for review. It goes to the executor's planner. See [Planning and Execution](planning-and-execution_20260925_1012.md).

## 9. vrun

```text
vrun [-t <seconds>] <label> <command> [arguments...]
vrun --status <label>
```

Starts a command, waits until it finishes, and then prints the exit code, the elapsed time and the end of its log. The command's exit code is passed through. `vrun` is meant for long builds and tests: an agent that runs them with `vrun` learns the result as soon as it is available.

- `-t <seconds>` sets how long `vrun` waits (default 1800). When the time runs out, `vrun` exits with code 124 and the command keeps running.
- `--status <label>` reports on a command started earlier with that label.
- While a command runs, the session keeps its "working" status. In the terminal view, a "Background commands" row above the terminal lists running commands with "Log" and "Stop"; see [Terminal Usage](terminal-usage_20260709_2041.md) §6.1.

## 10. vkb

```text
vkb explore <query> | search <query> | node <symbol-id> | callers <id> [depth] | callees <id> [depth]
    | impact <id> [depth] | path <from-id> <to-id> | files [path]
    | memories <query> | memory <entry-id> | notes <query> | note <vault-id> <path> | status
```

Queries the code graph of the current checkout and the knowledge base of the connected VelaTerm. Code queries need indexing to be enabled on the project's Code Graph page. `vkb` only reads; it does not change any knowledge entry. See [Code Graph & Knowledge Base](codegraph_20260905_2027.md), [Session Knowledge Base](global-memory_20260905_2027.md) and [Local Knowledge Bases](knowledge-notebooks_20260910.md).

## 11. vflow

```text
vflow status|stop|propose|dispatch|accept|block <workflow-id> [--round N --message-id msg-UUID] < message.txt
```

Controls planning and execution workflows. The planner uses `propose`, `dispatch`, `accept` and `block` as part of the workflow. You will mostly need two actions:

- `vflow status <workflow-id>` shows the workflow's state, its tasks, sessions and current round.
- `vflow stop <workflow-id>` stops further rounds and asks the running sessions to stop.

## 12. The `vela` command

On macOS, Settings ▸ General ▸ "Shell command" offers "Install 'vela' command". It adds `vela <project-path>` to your shell, similar to the `code` command of VS Code. Running it in any terminal opens the project in VelaTerm, or switches to it if it is already open. VelaTerm does not overwrite an existing `vela` command that it did not install.
