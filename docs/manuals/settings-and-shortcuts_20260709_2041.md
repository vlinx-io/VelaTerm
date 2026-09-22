# Settings & Shortcuts

Created: 2026-07-09 20:41

> Settings open from the gear button in the title bar (on macOS also the app menu, "Settings… ⌘,"). Eight categories on the left; this chapter is an item-by-item reference, ending with the default key bindings and how to rebind. Settings are shared between the desktop app and browser remote clients (a few purely per-client view options aside).

![Settings · Appearance](../assets/manuals/settings-appearance.png)

## 1. General

| Item | Description |
|------|-------------|
| Language | UI language; Auto (follow system) by default, or pin one of eleven languages |
| System notifications | Notification permission state and toggle, with per-platform steps when the OS has denied it |
| Notification sound | Sound for notifications |
| Vela Skills | One-click install of the `vspawn`, `vspawn-tree`, and `vopen` skills for both Claude and Codex (installed and removed as a bundle, refreshed on upgrade) |

## 2. Appearance

| Item | Description |
|------|-------------|
| Accent | Accent color: follow theme, or one of four fixed colors |
| Density | UI density: Compact / Regular / Comfy |
| Panes | Pane style: Flush / Card |
| Divider | Divider style: Subtle / Visible |
| Sidebar | Sidebar style: Tree / Compact |
| Interface font / size | UI font and size (Auto or manual stepping) |

Light/dark themes are switched from the title bar, not here (follow-system / dark / light). Running claude sessions re-skin instantly on switch.

## 3. Terminal

| Item | Description |
|------|-------------|
| Terminal font / size | Terminal font and size (⌘+ / ⌘- / ⌘0 adjust on the fly) |
| Terminal renderer | Rendering backend; keep the default (DOM) |
| Redraw on tab switch | Force a full repaint when switching tabs — enable if TUIs occasionally look glitched |
| Default shell (Windows only) | Default shell for new terminal sessions |

## 4. Conversation view

Applies to agent conversation views (the chat layout), not to terminal sessions.

| Item | Description |
|------|-------------|
| Conversation font / size / line height | Font, size and line height of the conversation view, independent of the terminal font |
| Composer toolbar | Which chips sit beside the message input, and in which order: Model, Thinking effort, Collaboration mode, Permission mode, Fast mode, Speed, Tone, MCP servers, Background tasks, Account, Codex reset credits. Each chip has an On/Off switch; the arrows move a chip that is on up or down. Model, Thinking effort, Collaboration mode and Permission mode are on by default. A chip that is on is always in the toolbar and a chip that is off never is, regardless of what the agent is doing right now: when a chip's feature is momentarily unavailable (no running agent process for MCP servers and Background tasks, an empty task list, an unresolved sign-in for Account) the chip stays in place, shown empty or dimmed and not openable; while the agent process is not running, MCP servers and Background tasks say so in their tooltip. Only a chip the current agent kind does not have at all is left out (for example Codex reset credits in a Claude session, or Fast mode when the model does not offer it) |

The **More** menu in the composer appears only while the chips that are on do not fit the row. It then holds the chips that overflowed plus the chips that are off, so nothing you turned off becomes unreachable: switch it back on here, or open it from More whenever that menu is shown. A chip you switch off and on again returns at the end of the inline order, even while it has nothing to show (for example Background tasks with an empty task list). On the mobile layout all chips that are off stay visible in a second row.

## 5. Behavior

| Item | Description |
|------|-------------|
| Tabs | Tab mode: Multi (default) / Single (single reused tab) |
| Background limit | Cap on background keep-alive tabs (default 32); past it, the oldest inactive tab is ended automatically |
| Sort by activity | Keep the most recently active projects and sessions at the top of the sidebar tree (off by default, same switch as in the sidebar filter menu); off restores the manual order. See [Interface & Session Management](interface-and-sessions_20260709_2041.md) |
| Confirm before spawn | Show the confirmation card before spawning child sessions (on by default) — see [Session Spawning & Git Collaboration](session-spawning-and-git_20260709_2041.md) |
| Usage refresh | Refresh interval for the Usage quota in the Info panel |
| Image paste | Image paste behavior: Upload as file (materialize to a path) / Agent default |
| Auto-clean pasted images | Periodically clean up pasted temp images, with a "Clean now" button |
| System notifications | Shortcut to the notification permission controls (same as General) |

## 6. Advanced

| Item | Description |
|------|-------------|
| Foreground-priority output | Output scheduling that protects typing latency while agents flood output (on by default); turn off to compare if you suspect display issues |
| Record session logs | Session recording (off by default). When on, terminal content is recorded in full — archives get replay, global search covers terminal output |

## 7. Agents

Configured per type (Claude / Codex / OpenCode / …), applying to **newly created** sessions of that type; per-session settings in the edit form override these defaults.

![Settings · Agents](../assets/manuals/settings-agents.png)

| Item | Description |
|------|-------------|
| Executable path | Full path to the executable. Empty = look up the command on PATH; set it when the agent lives outside PATH. Auto-filled after a successful one-click install |
| Launch args | Default launch-argument template for the type (e.g. `--model opus`) |
| Permission | Default permission level: Default (step-by-step confirmation) / YOLO (skip all permission prompts) |

## 8. Shortcuts

![Settings · Shortcuts](../assets/manuals/settings-shortcuts.png)

Click an action's binding, then press the new combination (must include ⌘/Ctrl). Conflicts name the current owner; "Restore defaults" resets everything. Defaults:

| Action | macOS | Windows / Linux |
|--------|-------|-----------------|
| Open project | ⌘O | Ctrl+Alt+O |
| New scratch terminal | ⌘T | Ctrl+Alt+T |
| New browser tab | ⌘⇧B | Ctrl+Alt+B |
| Close pane / tab | ⌘W | Ctrl+Alt+W |
| Split right | ⌘D | Ctrl+Alt+D |
| Split down | ⌘⇧D | Ctrl+Alt+E |
| Find in terminal | ⌘F | Ctrl+Alt+F |
| Search all sessions | ⌘⇧F | Ctrl+Alt+G |
| Save document | ⌘S | Ctrl+S |

Fixed, non-rebindable keys: ⌘1–9 (switch tabs), ⌘+ / ⌘- / ⌘0 (terminal font size).

When you open VelaTerm in a regular browser (URL remote access), the browser itself consumes ⌘/Ctrl letter combos (⌘D bookmarks, ⌘T opens a tab…), so the Windows / Linux Ctrl+Alt bindings above apply on every OS, and the settings page shows them that way. Desktop apps and remote-connection windows keep the macOS ⌘ bindings.
