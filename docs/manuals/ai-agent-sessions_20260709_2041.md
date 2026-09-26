# AI Agent Sessions

Created: 2026-07-09 20:41

Updated: 2026-09-25 10:21

> This chapter covers how VelaTerm hosts AI coding agents as typed sessions: supported agents, live status, automatic conversation resume, importing and forking conversations, permission modes, launch settings, install guidance and the Info panel. The conversation view has its own chapter: [Conversation View](conversation-view_20260925_1012.md).

## 1. Supported agents

VelaTerm can start these agents: **Claude Code** (shown as "Claude"), **Codex**, **OpenCode**, **Copilot**, **Cursor**, **Antigravity**, **Cline**, **Pi**, **OMP**, **Crush**, **Kimi Code (K3)**, **Kiro**, **Grok Build (Grok 4.5)** and **Zoo Code**. Each agent's own CLI must be installed and signed in.

Capabilities differ by agent:

| Agent | Status reporting | "Needs you" state | Resume | Fork | Permission control | Conversation view |
|-------|------------------|:-----------------:|:------:|:----:|--------------------|:-----------------:|
| Claude Code | Reported by the agent | ✅ | ✅ | ✅ | Five modes | ✅ |
| Codex | Reported by the agent | ✅ | ✅ | ✅ | Three modes | ✅ |
| OpenCode | Reported by the agent | ✅ | ✅ | ✗ | Two modes | ✅ |
| Copilot | Reported by the agent | ✅ | ✅ | ✗ | Skip switch | ✗ |
| Cursor | Reported by the agent | ✗ | ✅ | ✗ | Skip switch | ✗ |
| Antigravity | Reported by the agent | ✗ | ✅ | ✗ | Skip switch | ✗ |
| Cline | Reported by the agent | ✗ | ✅ | ✗ | Skip switch | ✗ |
| Pi | Reported by the agent | — | ✅ | ✅ | None (Pi does not ask) | ✅ |
| OMP | Reported by the agent | ✅ | ✅ | ✅ | Skip switch; two modes in the conversation view | ✅ |
| Crush | Partly reported, partly read from the screen | ✅ | ✅ | ✗ | Skip switch | ✗ |
| Kimi Code (K3) | Reported by the agent | ✅ | ✅ | ✗ | Skip switch | ✗ |
| Kiro | Reported by the agent | ✗ | ✅ | ✗ | Skip switch | ✗ |
| Grok Build | Reported by the agent | ✅ | ✅ | ✗ | Skip switch | ✗ |
| Zoo Code | Read from the screen | ✅ | ✅ | ✗ | Skip switch | ✗ |

- **Reported by the agent** means the agent itself tells VelaTerm when it starts working, needs you and finishes, so the status is exact. Codex reports every stage when its version supports lifecycle hooks; older Codex versions only report when a turn ends. Where a column shows ✗ under "Needs you", the agent does not report questions or permission prompts, so the session shows "working" until the turn ends.
- **Read from the screen** means VelaTerm recognizes the agent's state from its terminal interface.
- For Copilot, Cursor, Antigravity, Kimi Code, Kiro and Grok Build, VelaTerm adds its own entries to the agent's user configuration the first time you start such a session. These entries do nothing when the agent runs outside VelaTerm.
- Agents you start yourself in a plain terminal are not affected; status reporting applies only to agent sessions that VelaTerm starts.

## 2. Creating an agent session

- **Context menu.** Right-click a project, group or session. The first level of the menu offers three agents: Claude, Codex and OpenCode at first, and later the three agent types you created most recently. "More Agent Session" lists every agent, your saved presets, "New with launch args…", and "New agent session" (the searchable picker).
- **New agent session picker.** ⌘N on macOS (Ctrl+Alt+N on Windows, Linux and in regular browsers) opens a searchable list of agents and presets, with recent choices marked "Last used". See [Settings & Shortcuts](settings-and-shortcuts_20260709_2041.md) §9.
- **Child sessions.** A session's "New Child Session" submenu creates the new session under it. Sessions can also be started from inside a conversation; see [Session Spawning & Git Collaboration](session-spawning-and-git_20260709_2041.md).

**New with launch args…** opens a dialog with the agent type ("Agent type"), an optional name ("Session name (leave empty to auto-name)"), "Launch args (optional)", "Executable (optional)" for a replacement executable used by this session only, the permission setting, "Opens in" ("Terminal view" or "Conversation view") for agents that support both, and a "Worktree" choice. "Save as a preset" stores the settings under a name, with an optional icon. Presets appear in the menu and in the picker and start with their saved executable, arguments and permission setting.

Claude, Codex, OpenCode, Pi and OMP sessions open in the conversation view by default; the other agents open in the terminal view. Settings ▸ Agents ▸ "Default view" changes this per agent.

## 3. Status dots: who is working, who needs you

The dot next to each agent session updates live:

| Dot | Meaning |
|-----|---------|
| Green, pulsing | Working |
| Yellow, pulsing | Needs you: a question or a permission request |
| Yellow, rippling, with a yellow bar on the row | Unread: the agent finished or asked while you were elsewhere |
| Magenta | Replied, and you have seen it |
| Blue | Running, with no agent activity reported |
| Red | Error |
| Gray | Status unavailable |

When an agent stops for you (a question, a permission request or the end of a turn), you get a system notification, the session is marked unread, and the Dock badge counts it. If you are already looking at that session, no notification appears. On the signed macOS build, clicking a notification opens the session. The "Notify: On/Off" item in the status bar turns system notifications on or off; unread marks and badges remain. When badges exist, the sidebar header shows a button that clears all of them.

The status bar's "Working", "Pending" and "Viewed" counters filter the sidebar to sessions in that state; see [Interface & Session Management](interface-and-sessions_20260709_2041.md) §6.

## 4. Auto-resume: close it, reopen it, the conversation continues

The mental model in one line: **each agent session node in the tree is one ongoing conversation.**

- On the first run, VelaTerm remembers the agent's own conversation ID.
- After that, whether you closed the tab or quit the app, opening the node again continues the same conversation. Before resuming, VelaTerm checks that the conversation still exists; if it was deleted, the session starts fresh instead of failing.
- For a fresh conversation, create a new session node. In the conversation view, `/clear` replaces the session with a new empty one in the same place.

**Resume Session…** at the end of the New Session menu adds a conversation you already have, for example one you ran in a plain terminal. Pick the "Agent type", paste the agent's conversation ID, and choose "Resume & Open". Every agent type in §1 can be resumed this way.

**Import Sessions…** in a project's context menu lists existing Codex, Claude, OpenCode and Kiro conversations whose working directory is exactly the project directory. Search by title, agent or session ID ("Search by title, agent or session ID"), select rows, and choose "Import". The conversations join the project tree; open one to continue it. Importing does not copy or change the agent's own history. Conversations already in VelaTerm, including archived ones, are marked "Already imported". Conversations in subdirectories or separate worktrees appear only when that directory is opened as its own project. Kiro history can be viewed for text-only records. When connected to a remote VelaTerm, the list comes from the remote computer.

## 5. Fork: branch off a conversation

For Claude, Codex, Pi and OMP sessions that already have a conversation, right-click → "Fork Session". This creates a sibling node that starts from the source conversation's current history and leaves the source unchanged, similar to a git branch. Use it to try two approaches from the same context.

## 6. Permissions

Permission settings decide how much the agent may do without asking you.

- **Claude, Codex and OpenCode** have named modes. Claude: "Plan Mode", "Always Ask", "Accept File Edits", "Auto mode", "Bypass". Codex: "Read Only", "Auto mode", "Full Access". OpenCode: "Always Ask", "Bypass".
- **Copilot, Cursor, Antigravity, Cline, OMP, Crush, Kimi Code, Kiro, Grok Build and Zoo Code** have a single switch, "Skip all permission confirmations", which launches the agent with its own bypass option. In the conversation view, OMP offers "Always Ask" and "Bypass".
- **Pi** runs tools without asking, so it has no permission setting.

Where to change it:

- **Per session**: the session's "Edit" form ("Permission", or the "Skip all permission confirmations" checkbox); in the terminal view, the permission item in the status bar ("Perms: Ask" / "Perms: Skip", or the current mode name); in the conversation view, the permission control under the message box.
- **Default for new sessions**: Settings ▸ Agents ▸ "Permission". The status bar and conversation view menus also offer "Set as default".

Some changes apply only after the agent restarts. VelaTerm then offers "Restart now" or "Later"; restarting resumes the conversation but interrupts a running task. "Bypass", "Full Access" and "Skip all permission confirmations" skip every confirmation, so use them with care.

## 7. Launch settings and install guidance

**Launch args.** The session's "Edit" form has "Launch args (optional)", which adds command-line arguments for that session. Settings ▸ Agents holds a default for each agent type.

**Executable path.** If an agent is installed outside your PATH, set its "Executable path (optional)" in Settings ▸ Agents. When it is empty, the command is looked up on PATH.

![Settings · Agents](../assets/manuals/settings-agents.png)

**Not installed?** Starting an agent that is not installed shows a card in the session: "<Agent> is not installed", with the recommended install command for your system.

- "Install now" runs the command in the session. When the installed program is found, its path is saved to Settings, and a dialog offers "Relaunch now" or "Later".
- "Retry launch" starts the session again, for example after you installed the agent yourself.
- If the agent is already installed outside PATH, enter the full path in "Executable path" and choose "Use this path", or use "Browse…".
- "Install docs" opens the agent's installation documentation; "I'll do it myself" closes the card.

Each agent still needs its own sign-in or API key after installation.

## 8. The Info panel: usage, context and resources

With an agent session open, the Info tab of the right panel shows:

![Info panel](../assets/manuals/agent-info.png)

- **Agent**: the agent type, session name, working directory, Git branch, start time and uptime. For Kiro sessions it also links to a read-only view of the conversation history.
- **Usage** (Claude, Codex, Grok Build): account quota usage, for example the five-hour and seven-day windows, with their reset times. The ↻ button refreshes immediately. Automatic refresh and its interval are set in Settings ▸ Behavior ("Usage auto-refresh", "Usage refresh"). When a refresh fails, the last values stay visible with a "stale" mark.
- **This turn** (Claude, Codex, Grok Build, OpenCode, Pi, OMP, Kiro): the model, how full the context window is, and the current turn's token counts and tools.
- **Resources**: CPU and memory of the session's processes; the "system" option adds figures for the whole computer.

## 9. Transcripts, export and archiving

- Right-click → "Export Session…" (Claude and Codex, once a conversation exists) writes the full conversation to a Markdown file, including reasoning and every tool call with its input and result.
- "Organize into Session Knowledge Base" in the session menu has an agent extract reusable knowledge from the conversation; see [Session Knowledge Base](global-memory_20260905_2027.md).
- Archived sessions can be read in the knowledge base's "Archived Sessions" view. Conversations of Claude, Codex, OpenCode, Pi, OMP, Grok Build and Kiro show as messages; for other agents, the terminal recording is shown when one exists. Restoring an archived session keeps its conversation ID, so it resumes as usual. See [Interface & Session Management](interface-and-sessions_20260709_2041.md) §8.

## 10. Other behavior

- **Automatic names**: a session you did not name takes its name from your first message.
- **Theme changes**: switching between light and dark updates running Claude sessions in the terminal view without a restart.
- **Vela Skills**: Settings ▸ General ▸ "Vela Skills" installs skills for Claude Code and Codex that start child sessions, open files, and read or message other sessions; see [Session Commands](session-commands_20260925_1012.md).
- **Windows**: Claude Code and Codex are fully supported (they run in PowerShell); the other agents are provided on a best-effort basis.
