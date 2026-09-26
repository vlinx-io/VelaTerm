# Getting Started with VelaTerm

Created: 2026-07-09 20:41

Updated: 2026-09-25 10:21

> This guide takes you from installation to your first terminal and your first AI agent session. It is enough to start working; each feature is described in detail in the other manuals (see the [Manuals Overview](manuals-overview_20260709_2041.md)).

## 1. What VelaTerm is

VelaTerm is a terminal manager built for working with AI coding agents. It organizes terminal and agent sessions in a **project → group → session** tree and runs coding agents such as Claude Code and Codex as dedicated sessions: you can see at any time whether an agent is working, needs your input or has finished, and reopening a session continues its conversation. Agents can run in their own terminal interface or in a conversation view with messages, tool cards and permission buttons. Browser remote access and SSH remote development let you continue your sessions from other devices.

## 2. Installation

Download the package for your platform:

| Platform | Package | Notes |
|----------|---------|-------|
| macOS | `.dmg` (separate Apple Silicon and Intel builds) | Open it and drag VelaTerm into Applications. The app is notarized by Apple |
| Windows | `-setup.exe` installer, in min and full variants | full (about 360 MB) includes a complete Git Bash; min (about 25 MB) downloads missing commands when needed. Both install to the same location, so choose one |
| Linux | `.AppImage` (x86_64 and aarch64) | Make it executable and run it; no installation is needed |

VelaTerm checks for updates automatically; "Check for Updates…" in the app menu checks immediately.

## 3. First launch: add a project

On first launch the sidebar is empty and offers three ways to add a project:

- "Create Project" creates a new folder and adds it.
- "Open Project" (⌘O, Ctrl+Alt+O on Windows and Linux) adds an existing folder.
- "Clone from Git" clones a repository and adds it.

The same actions are available as buttons in the sidebar header. Inside a project you can create **groups**, nested as deeply as you like (for example frontend, backend, testing), and **sessions** inside them:

![Project tree in the left sidebar](../assets/manuals/left-tree.png)

## 4. Open your first terminal

Any of these opens a scratch terminal:

1. Press ⌘T (Ctrl+Alt+T on Windows and Linux).
2. Hover a project, group or session in the sidebar, click the ＋ button, and choose "New Terminal". The terminal starts in that node's directory. Project and group context menus have the same item.
3. When the center pane is empty, click "Create Terminal".

![Center pane with no session open](../assets/manuals/empty-state.png)

Scratch terminals exist only as tabs: they never become tree nodes and are discarded when closed, which makes them suitable for quick commands.

## 5. Open your first AI agent session

Right-click a project or group and choose "New Claude Session". The first level of the menu offers Claude, Codex and OpenCode; "More Agent Session" lists all fourteen supported agents. You can also press ⌘N (Ctrl+Alt+N on Windows, Linux and in regular browsers) to open a searchable list of agents.

VelaTerm starts the agent in the project directory. Claude, Codex, OpenCode, Pi and OMP sessions open in the **conversation view**: type your message in the box at the bottom and press Enter. The other agents open in the **terminal view**, which shows the agent's own terminal interface. The switch button in the pane header moves a session between the two views; see [Conversation View](conversation-view_20260925_1012.md).

![A running Claude session with the Info panel](../assets/manuals/agent-info.png)

From then on you get three things:

- **Status dots**: the dot next to the session in the sidebar shows the agent's state. Green means working, yellow means it needs you (a question or a permission request), and magenta means it has replied and you have seen it.
- **System notifications**: when the agent stops and waits for you, you receive a notification. If you are already looking at the session, no notification appears.
- **Automatic resume**: close the tab, or quit the app, and the next time you open the session the conversation continues where it stopped. For a new conversation, create a new session.

If the agent's CLI is not installed yet, the session shows an install card with the recommended install command for your system and an "Install now" button.

## 6. A quick tour

![Main window](../assets/manuals/main-ui.png)

- **Left sidebar**: the project tree with a search box and a status filter. Header buttons: Create Project, Import Project, Clone from Git, Search All Sessions, and Archived Sessions.
- **Center pane**: the tab bar and the open sessions. By default, clicking an agent session in the tree reuses the current tab, and the previous tab keeps running in the background. Closing a tab ends its processes. Tabs can be split into panes (⌘D right, ⌘⇧D down).
- **Right panel**: follows the current session, with four tabs: Files, Info (session details, account usage, context and resource use), Git (branch, changes and commits), and Knowledge Base.
- **Status bar**: session count, current session type, Git branch, permission mode, notification switch, and the Working / Pending / Viewed counters. Click a counter to show only those sessions in the sidebar.
- **Title bar, right side**: theme switch, Remote Access, Connect to Remote Server, Share, Settings, Account, Feedback, and the buttons that show or hide the side panels.

## 7. Where to go next

- Trees, tabs, split panes, archiving and global search: [Interface & Session Management](interface-and-sessions_20260709_2041.md).
- Agent capabilities, resume, fork and permissions: [AI Agent Sessions](ai-agent-sessions_20260709_2041.md).
- Working in the conversation view: [Conversation View](conversation-view_20260925_1012.md).
- Letting agents start child sessions in separate worktrees: [Session Spawning & Git Collaboration](session-spawning-and-git_20260709_2041.md).
- Terminal features (search, image paste, command suggestions): [Terminal Usage](terminal-usage_20260709_2041.md).
- Using VelaTerm from a phone or another computer: [Remote Development & Management](remote-development-guide_20260709_2041.md).
