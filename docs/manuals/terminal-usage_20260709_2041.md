# Terminal Usage

Created: 2026-07-09 20:41

Updated: 2026-09-25 10:21

> This chapter covers the terminal itself: input and output, in-terminal search, copy and paste, image paste, command suggestions, the built-in `v` commands, background commands started with `vrun`, shell selection on Windows, session recording, renderer settings and shortcuts.

## 1. Basics

A terminal session runs a real shell. On macOS and Linux it is the system default shell (`$SHELL`); on Windows it is cmd by default and can be changed (see §7). The working directory defaults to the project root. The session's edit form lets you change "Working directory (leave empty for project root)" and set a "Startup command (optional)", which runs every time the session starts (for example `pnpm dev`).

When several agents produce a lot of output at once, VelaTerm gives your typing and the visible tab priority and throttles output from background tabs. This behavior is controlled by Settings ▸ Advanced ▸ "Foreground-priority output" (on by default). If a full-screen program ever looks garbled after you switch tabs, click the "Redraw" button in the pane header to repaint it. Settings ▸ Advanced ▸ "Redraw on tab switch" does this automatically whenever a tab becomes visible.

## 2. In-terminal search (⌘F)

⌘F opens a search bar ("Search in terminal") for the current terminal's scrollback, with previous and next buttons:

![In-terminal search](../assets/manuals/terminal-search.png)

To search across sessions and history, use global search (⌘⇧F), described in [Interface & Session Management](interface-and-sessions_20260709_2041.md).

## 3. Copy and paste

- Select text with the mouse, then use ⌘C / ⌘V or the context menu ("Copy", "Paste", "Select All", "Clear", "Search…").
- Inside programs that capture the mouse (vim, htop, most agent interfaces), hold Option on macOS or Shift on Windows and Linux while dragging to force a normal text selection.
- Agent interfaces such as Claude Code and Codex can copy text themselves. When they have just done so and nothing is selected in the terminal, the context menu shows "Auto-copied N chars · ⌘V to paste".

## 4. Pasting images

Pasting or dropping an image into a terminal is how you give an image to a command-line agent. Settings ▸ Terminal ▸ "Image paste" offers two choices:

- **Paste file path** (default): the image is saved as a temporary file and its path is inserted at the prompt, which any agent that reads files can open. Codex receives it as `image_path: …`.
- **Native image paste**: Claude Code or Codex reads the image from the system clipboard and shows its own image placeholder. This option is available only in the local desktop app; remote sessions always paste a file path so that the agent can read the image on its own machine.

Settings ▸ Behavior ▸ "Auto-clean pasted images" removes these temporary files when the app exits and clears leftovers older than 24 hours on startup. "Clean now" removes them immediately. Images inserted into documents are never affected.

## 5. Command suggestions

In plain terminal sessions on macOS and Linux, VelaTerm can show a suggestion list built from your shell's own completion definitions. Settings ▸ Terminal ▸ "Command suggestions" chooses "Automatic" (default), "On Tab" or "Off". Use the arrow keys to select a candidate and Tab to insert it; Enter always runs the command as typed. When the selection cannot move any further, Up and Down go back to the shell, so Up on the first candidate recalls the previous command. The list is not available on Windows. Full details are in [Command suggestions](terminal-completion_20260908.md).

## 6. Built-in commands

Every session VelaTerm starts has a set of short commands on its PATH. They need no installation and work in plain terminals as well as agent sessions:

| Command | Purpose |
|---------|---------|
| `vopen <file or URL>...` | Open files in document tabs and web addresses in browser tabs (see below) |
| `vspawn <task>` / `vspawn-tree <task>` | Ask VelaTerm to start a child session; `vspawn-tree` gives it a separate worktree |
| `vrefer <session>` | Read another session's conversation |
| `vsearch <words...>` | Search the content of all sessions |
| `vstat` | Show which sessions are working or waiting |
| `vtell <session> <message>` | Send a message to another session |
| `vkb <subcommand>` | Query local knowledge bases, the session knowledge base and the code graph |
| `vrun <label> <command...>` | Run a long command and wait for it (see §6.1) |
| `vflow` | Used by the planner in planning and execution sessions |

Run a command with `--help` to see its options; [Session Commands](session-commands_20260925_1012.md) describes each one in detail. Most of these commands also exist as skills inside Claude Code (`/vopen`) and Codex (`$vopen`); `vrun` and `vflow` have no skill, and `vask` exists only as a skill. The skills are installed with the "Vela Skills" button in Settings ▸ General.

`vopen` chooses the view by file type:

```bash
vopen README.md          # Markdown → Markdown editor
vopen src/main.rs        # source code → code editor with syntax highlighting
vopen diagram.png        # image → image viewer
vopen https://crates.io  # web address → browser tab (desktop app only)
```

Relative paths are resolved against the current directory, and several files can be opened at once. Document tabs are covered in [Document & Browser Tabs](document-and-browser-tabs_20260709_2041.md); `vspawn` is covered in [Session Spawning & Git Collaboration](session-spawning-and-git_20260709_2041.md); `vkb` is covered in [Code Graph & Knowledge Base](codegraph_20260905_2027.md) and [Local Knowledge Bases](knowledge-notebooks_20260910.md).

### 6.1 Background commands (`vrun`)

`vrun <label> <command...>` starts a command, waits for it to finish, and then prints the exit code, the elapsed time and the end of its log. An agent that starts a long build or test with `vrun` therefore learns about the result as soon as it is available. Closing the session does not stop the command. `vrun --status <label>` reports on a command started earlier, and `-t <seconds>` sets how long to wait (1800 seconds by default); when the wait times out, `vrun` exits with code 124 and leaves the command running.

While such commands run, a "Background commands" row appears above the terminal. Each entry shows its label and "Running for …", a "Log" button that opens the command's output, and a "Stop" button; click "Confirm stop" to end the command. The row disappears when nothing is running.

## 7. Shell selection (Windows only)

On Windows, terminal sessions can use any detected shell: PowerShell, pwsh, cmd, Git Bash, and every installed WSL distribution. Choose it from the "New Terminal" submenu, from "Shell (leave empty for system default)" in the session edit form, or from the right-click "Shell" menu. A running session restarts immediately with the new shell. Settings ▸ Terminal ▸ "Default shell" sets the shell for new terminals that have no explicit choice.

WSL entries appear as `WSL: <distribution>` only when WSL reports an installed distribution. Choosing one starts that distribution and maps the session's Windows project directory into it. WSL is available for plain terminal sessions only; agent sessions continue to use the Windows host shell. The built-in `v` commands are not available inside WSL terminals.

About the bundled Git Bash: the full installer includes the complete Git Bash (with git, ssh and perl). The min installer includes a core subset; when you run a missing command, it offers to download the full version, which is also available from the right-click "Download full Git Bash" item. Agent sessions are not affected; they use PowerShell.

macOS and Linux have no shell picker; sessions use the system default shell.

## 8. Session recording

Settings ▸ Advanced ▸ "Record session logs" is off by default. When it is on, VelaTerm saves the terminal output of agent sessions running in the terminal view to a local log file, up to 50 MB per session. Plain terminal sessions are never recorded.

A recording lets you replay an archived session as it appeared on screen and makes its screen text searchable. VelaTerm uses a recording only when the agent's own conversation record cannot be read; otherwise the conversation record is used for viewing and search. Recording files are deleted together with their session.

## 9. Renderer

Settings ▸ Advanced ▸ "Terminal renderer" offers DOM (default) and WebGL. DOM is the recommended choice in the desktop app. Earlier versions also offered a Canvas renderer; that option no longer exists, and a saved Canvas preference now uses DOM.

## 10. Common shortcuts

The table lists the defaults of the macOS desktop app. The Windows and Linux apps use Ctrl+Alt instead of ⌘ for most actions (split down is Ctrl+Alt+E, search all sessions is Ctrl+Alt+G, and save stays Ctrl+S). Browser clients use Ctrl-based defaults that avoid the browser's own shortcuts. Settings ▸ Shortcuts shows the exact bindings on your system, and every action except tab switching and font size can be changed there.

| Action | Shortcut |
|--------|----------|
| Open project | ⌘O |
| New terminal | ⌘T |
| New agent session | ⌘N |
| New browser tab | ⌘⇧B |
| Close pane / tab | ⌘W |
| Split right / split down | ⌘D / ⌘⇧D |
| Find in terminal | ⌘F |
| Search all sessions | ⌘⇧F |
| Save document | ⌘S |
| Go to tab N | ⌘1–9 (fixed; Ctrl+1–9 outside the macOS desktop app) |
| Terminal font larger / smaller / reset | ⌘+ / ⌘- / ⌘0 (fixed; Ctrl+ / Ctrl- / Ctrl+0 outside the macOS desktop app) |
