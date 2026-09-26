# Settings & Shortcuts

Created: 2026-07-09 20:41

Updated: 2026-09-25 10:21

> Settings open from the gear button in the title bar. On macOS the app menu also has "Settings…" (⌘,); on Windows and Linux, press Alt and choose File ▸ Settings…. The settings window has eight categories: General, Appearance, Terminal, Conversation view, Behavior, Advanced, Agents and Shortcuts. This chapter describes each item and ends with the default key bindings. Settings are shared between the desktop app and browser clients connected to it.

![Settings · Appearance](../assets/manuals/settings-appearance.png)

## 1. General

| Item | Description |
|------|-------------|
| Language | Interface language. "Auto (system)" follows the system language; eleven languages are available |
| Notification sound | Sound for notifications (on by default) |
| System notifications | Turns system notifications on or off (on by default). If the system has not allowed notifications yet, "Allow notifications" asks for permission; if they are blocked, the steps for your system are shown |
| Shell command | macOS desktop app only: "Install 'vela' command" adds `vela <project-path>` to your shell, which opens a project in VelaTerm. "Uninstall 'vela' command" removes it |
| Vela Skills | "Install" (later "Reinstall") installs the VelaTerm skills for Claude Code and Codex, such as `/vspawn` and `$vspawn`. Start a new Codex session if Codex does not list them. See [Session Commands](session-commands_20260925_1012.md) §2 |

## 2. Appearance

| Item | Description |
|------|-------------|
| Accent | Accent color: "Follow theme" or one of four colors |
| Density | "Compact", "Regular" (default) or "Comfy" |
| Panes | Pane style: "Flush" (default) or "Card" |
| Divider | "Subtle" (default) or "Visible" |
| Sidebar | "Tree" (default) or "Compact" |
| Interface font | Font of the interface |
| Interface size | Font size of the interface; "Auto" follows the density |

The light and dark themes are switched in the title bar (follow system, dark, light), not here. Running Claude sessions in the terminal view follow the theme change without a restart.

## 3. Terminal

| Item | Description |
|------|-------------|
| Image paste | "Paste file path" (default) saves a pasted image as a temporary file and inserts its path. "Native image paste" lets Claude Code or Codex read the image from the clipboard; it is available only in the local desktop app |
| Command suggestions | "Automatic", "On Tab" or "Off". Takes effect immediately, including in open terminals; hidden where it is not supported. See [Command suggestions](terminal-completion_20260908.md) |
| Default shell | Shell for new terminals. Shown on Windows, where several shells are available; macOS and Linux always use the system shell |
| Terminal font | Font of the terminal |
| Terminal size | Terminal font size (default 13). ⌘+ / ⌘- / ⌘0 change it while you work |
| Terminal line height | Line spacing of the terminal (default 1.2×) |

## 4. Conversation view

These settings apply to the conversation view of agent sessions; see [Conversation View](conversation-view_20260925_1012.md).

| Item | Description |
|------|-------------|
| Conversation font, Conversation font size, Conversation line height | Typography of the conversation, independent of the terminal settings (default size 13.5, line height 1.2×) |
| Composer toolbar | Which controls appear beside the message box, and in which order: Model, Thinking effort, Collaboration mode, Permission mode, Fast mode, Speed, Tone, MCP servers, Background tasks, Account and Codex reset credits. Each has an On/Off switch; the arrows reorder the controls that are on. Model, Thinking effort, Collaboration mode and Permission mode are on by default |

Controls that the current agent or model does not support are not shown. A supported control stays visible when its feature is temporarily unavailable, for example MCP servers and Background tasks while the agent process is stopped.

Controls that are switched off, and controls that do not fit beside the message box, are in the **More** menu, so you can use every supported control without first switching it on here. More is not shown when all supported controls are on and fit. A control that is switched off and on again moves to the end of the row. On mobile, controls that are off appear in a second row.

## 5. Behavior

| Item | Description |
|------|-------------|
| Tabs | "Single" (default) reuses the current tab when you open an agent session and keeps the previous tab running in the background. "Multi" opens each session in its own tab |
| Dynamic status filter additions | When a sidebar status filter is active, sessions that start to match are added to it (on by default) |
| Background limit | Shown in Single mode: the maximum number of background tabs (8, 16, 32 or 64; default 32). Past the limit, the oldest inactive background tab ends |
| Confirm before spawn | Show a confirmation card before a child session starts (on by default); see [Session Spawning & Git Collaboration](session-spawning-and-git_20260709_2041.md) §4 |
| Usage auto-refresh | Keep the account usage in the Info panel up to date (on by default) |
| Usage refresh | Refresh interval: 30 s, 1 min, 2 min or 5 min (default 5 min) |
| Continue after limit resets | Continue a Claude or Codex task automatically after a five-hour or weekly usage limit resets (on by default) |
| Auto-clean pasted images | Remove the temporary files of pasted images when the app exits, and files older than 24 hours at startup (on by default). "Clean now" removes them immediately |

**Session reference context** decides how `vrefer --ask` (and the `vask` skill) hands another session's conversation to the agent that answers. "Full transcript" (default) passes the whole conversation. "Summarize first" condenses it first with the "Summary agent", model and effort selected here. See [Session Commands](session-commands_20260925_1012.md) §5.

## 6. Advanced

| Item | Description |
|------|-------------|
| Terminal renderer | "DOM" (default) or "WebGL". Keep the default unless you have a specific reason to change it |
| Redraw on tab switch | Redraws a terminal completely when you return to its tab (off by default). Turn it on if terminal interfaces occasionally look garbled after switching |
| Foreground-priority output | Keeps typing responsive while agents in other tabs produce a lot of output (on by default) |
| Record session logs | Saves the terminal output of agent sessions to a log file (off by default). When on, archived sessions can be replayed and global search also covers terminal output. Plain terminal sessions are never recorded |

## 7. Agents

"New session defaults" apply to **newly created** sessions of the selected agent. Settings made for an individual session take precedence, except for the executable path, which always applies to the whole agent type.

![Settings · Agents](../assets/manuals/settings-agents.png)

| Item | Description |
|------|-------------|
| Agent type | The agent whose defaults you are editing |
| Executable path (optional) | Full path to the agent's executable. When empty, the command is looked up on PATH. Filled in automatically after a successful one-click installation |
| Launch args (optional) | Default command-line arguments, for example `--model opus` |
| Permission | Default permission for new sessions. Claude, Codex and OpenCode offer their named modes; most other agents offer "Default" and "YOLO" (skip all permission confirmations); Pi has no permission prompts. See [AI Agent Sessions](ai-agent-sessions_20260709_2041.md) §6 |
| Default view | For Claude, Codex, OpenCode, Pi and OMP: "Conversation view" (default) or "Terminal view" for new sessions. Existing sessions keep their view |

## 8. Shortcuts

![Settings · Shortcuts](../assets/manuals/settings-shortcuts.png)

Click an action's key combination, then press the new combination; Escape cancels. A combination consists of a letter with ⌘ (on the macOS desktop app) or Ctrl, optionally with Shift and Option/Alt. If the combination is taken, the message names the action that uses it. "Restore defaults" resets all bindings.

| Action | macOS desktop app | Windows, Linux |
|--------|-------------------|----------------|
| Open project | ⌘O | Ctrl+Alt+O |
| New terminal | ⌘T | Ctrl+Alt+T |
| New agent session | ⌘N | Ctrl+Alt+N |
| New browser tab (desktop app only) | ⌘⇧B | Ctrl+Alt+B |
| Split right | ⌘D | Ctrl+Alt+D |
| Split down | ⌘⇧D | Ctrl+Alt+E |
| Close pane / tab | ⌘W | Ctrl+Alt+W |
| Find in terminal | ⌘F | Ctrl+Alt+F |
| Search all sessions | ⌘⇧F | Ctrl+Alt+G |
| Save document | ⌘S | Ctrl+S |

**Regular browsers.** When you use VelaTerm in a regular browser (URL remote access), the browser itself uses many ⌘ and Ctrl combinations, so the Windows and Linux bindings above apply on every system, and the settings page shows them that way. The exception is a browser on macOS, where split right and split down stay ⌘D and ⌘⇧D. Remote-connection windows of the desktop app use the desktop bindings.

**Fixed keys** that cannot be changed: ⌘1–9 switches tabs, and ⌘+ / ⌘- / ⌘0 changes or resets the terminal font size (Ctrl on Windows, Linux and in browsers).

## 9. New agent session picker

"New agent session" opens a searchable list of agent types and saved presets ("Search agents and presets"). Recently used choices come first and are marked "Last used".

The picker shows where the session will be created. When a saved session is active, choose "Same level" to create the new session beside it, or "Child session" to create it under it. While the search field has focus:

- Tab switches between "Same level" and "Child session".
- ↑ and ↓ select a result; Enter creates the session.
- Shift+Tab moves focus to the previous control; Escape closes the picker.

You can also click a result and choose "Create". If no project is available, select one or use "Open Project" first.

The picker has its own address, which keeps the target, position, search text and selection. Copying the link, refreshing, or going back and forward restores the picker without creating a session. If creating fails, the picker stays open so you can retry.

## 10. Other keys

| Where | Key | Action |
|-------|-----|--------|
| Conversation view | Enter / Shift+Enter | Send / new line |
| Conversation view, while the agent works | ⌥Enter (Alt+Enter) | Add the message to the running turn |
| Conversation view, while the agent works | ⌘Enter (Ctrl+Enter) | Stop the turn and send this message |
| Conversation view, while the agent works | Esc | Stop the turn |
| Spawn confirmation card | ⌘Enter (Ctrl+Enter) | Start the session |
| Tab bar | Middle click | Close the tab |
| Sidebar | ⌘/Ctrl-click, Shift-click | Select several sessions, or a range |
