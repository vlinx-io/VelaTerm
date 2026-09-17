# AI Agent Sessions

Created: 2026-07-09 20:41

> This chapter covers VelaTerm's core differentiator: hosting AI coding agents as **typed sessions** — live status, automatic conversation resume, forking, and permission control.

## 1. Supported agents

The New Session menus offer these local agent types: **Claude Code**, **Codex**, **OpenCode**, **Copilot CLI**, **Cursor CLI**, **Antigravity**, **Cline**, **Pi**, **Crush**, and **Kiro**. Capabilities differ slightly:

| Agent | Status awareness | Auto-resume | Fork | "Skip permissions" toggle |
|-------|-----------------|:-----------:|:----:|:------------------------:|
| Claude Code | Authoritative (incl. asking) | ✅ | ✅ | ✅ |
| Codex | Weakly authoritative + screen detection | ✅ | ✅ | ✅ |
| OpenCode | Authoritative (incl. asking) | ✅ | ✗ | ✗ (own config) |
| Copilot CLI | Authoritative (incl. asking) | ✅ | ✗ | ✅ |
| Cursor CLI | Authoritative | ✅ | ✗ | ✅ |
| Antigravity | Authoritative | ✅ | ✗ | ✅ |
| Cline | Authoritative | ✅ | ✗ | ✅ (explicit both ways) |
| Pi | Authoritative | ✅ | ✅ | ✗ (no permission system) |
| Crush | Partially authoritative + screen detection | ✅ | ✗ | ✅ |
| Kiro | Authoritative | ✅ | ✗ | ✅ |

"Authoritative" means VelaTerm injects the agent's official callback mechanism (hooks / plugin / extension) at launch, so state changes are **actively reported** by the agent rather than guessed from terminal output. For agents that don't report everything (codex, crush), a screen-detection fallback fills the gaps. Agents you run yourself in a plain terminal are untouched — injection only applies to sessions VelaTerm launches.

## 2. Status dots: who's working, who's waiting on me

The dot next to each agent session updates live: green = working; yellow = needs you (question, permission prompt, or unread notification); magenta = replied and seen. The status bar's three counters are clickable filters — with many agents in flight, that's how you find the ones waiting for you.

Paired with **system notifications**: when an agent stops for you (a question, or end of turn) you get a notification, the session gets an unread badge, and the Dock badge counts it; if you're already looking at that session, nothing fires. On the signed macOS build, clicking a notification jumps straight to the session. The "Notify" item in the status bar is the global toggle.

## 3. Auto-resume: close it, reopen it, the conversation is still there

The mental model in one line: **each agent session node in the tree = one ongoing conversation.**

- On first run, VelaTerm remembers the agent's own session id automatically.
- After that — whether you closed the tab or quit the app — reopening the node relaunches the agent with its resume flag (e.g. `claude --resume <id>`) and the context comes right back. Before resuming, VelaTerm verifies the conversation still exists; if it was deleted, it silently falls back to a fresh start instead of hanging.
- Want a fresh conversation? Create a new node. The whole mechanism is automatic — no switches, no cleanup.

**Manual resume**: if you have an agent session id from elsewhere (say, a conversation you ran in a plain terminal), use "Resume Session…" at the bottom of the New Session menu — pick the type, paste the id, and that conversation joins the tree as a proper session node.

**Import existing sessions**: open a project directory, right-click the project, then choose **Import Sessions**. VelaTerm lists Codex, Claude and OpenCode conversations whose working directory matches that project. Search by title, agent name or native session ID, select individual rows or all visible available rows, then click **Import**. Importing adds the conversations to the project tree; open a node to continue it with the original agent session ID. It does not copy or rewrite the native history.

Sessions already present anywhere in VelaTerm, including archived sessions, are marked **Already imported** and cannot be selected again. If a history source cannot be read, the dialog displays a warning while retaining results from the other sources. The dialog and search have a copyable URL and survive refresh. Native histories are read on the machine running the VelaTerm backend; when connected remotely, this is the remote machine. Subdirectories and separate worktrees must be opened as their own projects to import their sessions.

## 4. Fork: branch off the current conversation

Right-click a claude / codex / pi session that has a conversation → "Fork Session". You get a sibling node that branches off the **current history** of the source conversation, leaving the source untouched — think git branch. Great for "same context, try two approaches".

## 5. Permission modes and launch arguments

**Two-level permissions**: each supported session can run in "Default" (step-by-step confirmation) or "skip all permission confirmations" — a.k.a. YOLO mode, which launches the agent with its corresponding flag (e.g. claude's `--dangerously-skip-permissions`). Toggle per session via "Skip all permission confirmations" in the session's edit form; set the per-type global default in Settings ▸ Agents.

**Custom launch args**: the session edit form's "Launch args" appends extra command-line arguments for that session; Settings ▸ Agents holds a per-type default template, and "New with launch args…" in the New Session menu is a one-off parameterized create.

**Executable path**: if an agent is installed outside PATH, set its "Executable path" per type in Settings ▸ Agents; leave empty to look the command up on PATH.

![Settings · Agents](../assets/manuals/settings-agents.png)

## 6. Not installed? Install guidance

Launching an agent that isn't installed doesn't dead-end in `command not found`: an install-guide card appears in the session with the recommended install command for your OS — copy it, or run it in place with one click. After install, the binary's location is auto-detected and filled into the path setting, and a retry button relaunches the session. Remember each agent still needs its own login / API key setup; the card links to the docs.

## 7. The Info panel: model, usage, resources

With an agent session open, the right panel's Info tab shows its runtime details:

![Info panel](../assets/manuals/agent-info.png)

- **AGENT**: session name, type, run state, working directory, Git branch, start time, uptime.
- **MODEL / This turn** (claude): current model, context usage, tool in flight.
- **USAGE** (claude / codex): official quota usage (5-hour and 7-day windows); refresh interval is configurable (Usage refresh).
- **RESOURCES**: measured CPU / memory of the session's process tree.

## 8. Transcripts, export, and archiving

- Right-click → "Export Session…" (claude / codex, shown once a conversation has been captured) writes the full context to Markdown — including assistant thinking and every tool call with its inputs and results.
- Archived agent sessions are readable as parsed transcripts in the archive panel (no terminal replay needed); restoring re-enables resume as usual. See [Interface & Session Management](interface-and-sessions_20260709_2041.md) §7.

## 9. Odds and ends

- **Auto-naming**: unnamed sessions take their name from your first message (claude and others).
- **Live theme following**: switching light/dark re-skins running claude sessions instantly, no restart.
- **Vela Skills**: the "Vela Skills" toggle in Settings ▸ General installs `vspawn`, `vspawn-tree`, and `vopen` for both Claude and Codex, letting either agent spawn sub-sessions and open documents from inside a conversation (see [Session Spawning & Git Collaboration](session-spawning-and-git_20260709_2041.md)).
- **Windows**: claude / codex fully supported (via PowerShell); the other types are best-effort.


## 10. Sign in and out from a Claude or Codex conversation

Open **Claude account** or **Codex account** in the composer controls to access **Sign in again** and **Sign out**, even when no authentication error has occurred. The menu closes when you click outside it or press Escape. Account actions remain hidden until needed; only unresolved authentication or an operation in progress appears below the conversation. After sign-in succeeds or is canceled, that panel closes automatically. Wait for the current turn to finish before changing accounts. Claude also requires any background tasks and permission requests to finish. Read-only conversation views do not expose account controls.

A sign-in prompt appears when a Codex request reports an authentication error, such as a revoked refresh token, `unauthorized`, or a requirement to sign in. The prompt does not open the browser automatically. Select **Sign in again**, open the authorization link, and enter the displayed device code. The conversation shows the result automatically, and you can send another message after sign-in succeeds. The existing conversation and its native thread ID are retained.

The device code remains available when you switch panes or reload the page while the backend and Codex process are running. **Cancel** stops the pending login; an unsuccessful attempt can be retried.

**Sign out** asks for confirmation before clearing the shared Codex account credentials on the connected host. This affects other sessions that use the same credential store; conversation history is kept. After Codex confirms sign-out, the view offers **Sign in**. If the result cannot be confirmed, the view reports that outcome and allows a retry. Signing out pauses automatic delivery of queued messages.

This uses [Codex-managed device code authentication](https://learn.chatgpt.com/docs/app-server). Enable device code login in your [ChatGPT security settings or workspace permissions](https://learn.chatgpt.com/docs/auth), and use a Codex CLI version that supports it. Authentication changes apply to the Codex credential store on the connected host, including other sessions that share that store. VelaTerm displays the temporary device code; Codex manages the account credentials.

For Claude, **Sign in again** opens a sign-in prompt with an authorization link. Open that link, sign in on the official page, and paste the full authorization code, including the part after `#`, into **Authorization code**. Select **Submit code** to finish. The backend checks that the code belongs to the current attempt; the code is never sent as a chat message or retained in the conversation. Pending authorization remains available across page reloads while the backend and Claude process are running. **Cancel** waits for the native flow to end before closing the prompt; cancellation is unavailable while Claude verifies a submitted code.

Claude manages OAuth and credential storage through its native control protocol. Its temporary callback listener uses an automatically assigned port on the connected host's loopback address. The manual authorization link and code submission work when the browser is on another device. The CLI must support `claude_authenticate`, `claude_oauth_callback`, and `claude_oauth_wait_for_completion`; an unsupported CLI or an unsuccessful attempt produces a retryable sign-in error.

For Claude, confirmed **Sign out** runs `claude auth logout` with the session's executable, working directory and configuration-source arguments. The current idle process is then released to discard cached credentials. The next operation resumes the native conversation and restores the visible history and queued messages. Sign-out clears the saved account credentials; configured API keys and other authentication methods are unchanged. Other running sessions can retain cached credentials until they restart.

## 11. Shell mode with `!` in the conversation view

Type `!` followed by a command into the composer, for example `! az login --tenant <id>`, and VelaTerm runs the command itself instead of sending it to the agent as text. This mirrors the terminal UI's shell mode: the agent neither interprets nor approves the command. Leading whitespace and spaces after the `!` are ignored; `!` alone does nothing and shows a hint. Send behaviors (queue, steer and so on) do not apply, and a shell command cannot carry images: with an attachment present the composer keeps your draft and asks you to remove it or send it as a message.

The command runs in the session's shell as a login shell (the same shell resolution the terminal view uses; PowerShell or cmd on Windows), in the agent's working directory and with the environment the agent process receives. The text after `!` is passed to the shell as one string, exactly as typed. If no agent process is running yet, VelaTerm starts it first, as it does for any message, so the agent can react afterwards.

While the command runs, a dedicated row appears in the timeline: the command line, its live stdout and stderr, a running indicator and a **Cancel** button. It is neither a user bubble nor a tool call. Cancel kills the command together with every process still in its process group; a process that detached itself into its own group keeps running, as it would in the terminal. There is no timeout, so a command that waits for a browser login keeps running until it finishes or you cancel it. When the command ends, the row shows the exit code, or that you cancelled it. Very long output is trimmed to its last part and the row says so.

When the command has ended, the agent receives the command with its stdout, stderr and exit state (or the cancellation) as one user message and responds to it like it would in the terminal UI. This message is shown only as the command row, never as raw text, also after the conversation is reloaded, and mirrored or remote views show the same row. Only one shell command runs per conversation at a time; a second `!` while one is running is refused with a hint. Shell mode works the same way for every agent kind, because the command runs locally.

Limits: there is no interactive input. A command that prompts on stdin ends without input or waits until you cancel it, as in the terminal UI's shell mode. `!` is recognized only at the start of the message, and there is no history or path completion.
