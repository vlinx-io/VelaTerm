# Interface & Session Management

Created: 2026-07-09 20:41

Updated: 2026-09-25 10:21

> This chapter covers VelaTerm's organizational model: the window layout, managing the tree, how tabs and split panes behave, status dots and filtering, archiving, global search, and saving the workspace on quit. The other chapters build on these.

## 1. The layout

![Main window](../assets/manuals/main-ui.png)

| Area | Contents |
|------|----------|
| Title bar (right side) | Theme switch (follow system, dark, light), Remote Access (Browser), Connect to Remote Server, Share, Settings, Account, Feedback, and the buttons that show or hide the sidebar and the info panel |
| Left sidebar | "Workspace": the project tree with a search box and filter; header buttons Create Project, Import Project, Clone from Git, Search All Sessions and Archived Sessions |
| Center pane | Tab bar and the open sessions, documents, browser pages and background tasks; each session tab can be split into panes |
| Right panel | Four tabs that follow the current session: Files, Info, Git, and Knowledge Base |
| Status bar | Session count, current session type, Git branch, permission mode, notification switch, the Working / Pending / Viewed counters, background tabs, and update notices |

Remote Access and Connect to Remote Server appear in the desktop app; see [Remote Development & Management](remote-development-guide_20260709_2041.md). Feedback opens the VelaTerm feedback page in your browser. On Windows and Linux, pressing Alt shows a menu bar with File, Terminal and Help.

Both side panels can be hidden with the title bar buttons and resized by dragging their edges.

## 2. The tree: projects, collections, groups, sessions

- A **project** is a directory on disk and the root of a subtree. Create one with "Create Project" (makes a new folder), "Import Project" (opens an existing folder, ⌘O), or "Clone from Git".
- A **collection** is a top-level container without a folder, for sessions that do not belong to one project. Create it with the "New Collection" button at the bottom of the sidebar.
- A **group** is an organizational node and can be nested at any depth.
- A **session** is the working unit: an agent, a browser page, or a terminal. Its working directory defaults to the project root and can be changed in the session's "Edit" form.

Hovering a row shows a ＋ button ("New Session" on projects and groups, "New Child Session" on sessions) and, on projects and groups, a button for a new group.

Most operations are in the context menus:

![Project context menu](../assets/manuals/project-menu.png)

**Project and group menus** start with the creation items:

- "New Browser Page" (desktop app) and "New Terminal", which opens a scratch terminal in that directory (§5).
- Three direct "New … Session" items for recently used agents, and "More Agent Session" with every agent, saved presets, "New agent session" (the searchable picker) and "New with launch args…".
- "New Worktree Session…" and "New Plan/Execute Session…"; see [Session Spawning & Git Collaboration](session-spawning-and-git_20260709_2041.md) and [Planning and Execution](planning-and-execution_20260925_1012.md).
- "Import Sessions…" (projects only) and "Resume Session…"; see [AI Agent Sessions](ai-agent-sessions_20260709_2041.md) §4.

A project menu then offers "New Group", "Mark", "Experimental" (Code Graph and Code audit), "Project Info" (or "Collection Info"), "Rename", and "Remove Project" (or "Delete Collection"). A group menu offers the "Git" submenu, "New Subgroup", "Move to Worktree…", "Mark", "Rename", "Archive Group" and "Delete Group".

**Session menus** show items that fit the session's type and state:

![Session context menu](../assets/manuals/session-menu.png)

- "Open", "Open in New Tab", "Open in Split Right", "Open in Split Down", "Open in Focused Pane" (§4).
- "New Child Session", "Fork Session", "Export Session…", "Organize into Session Knowledge Base", and the "Git" submenu.
- "Mark", "Session Info" (hold Option/Alt for the full launch command), "Edit", "Rename", "Move to…".
- "Kill Process" (while running), "Archive Session", "Delete Session".

Other tree operations:

- **Move**: drag nodes, or use "Move to…". A session can be moved under another session to become its child.
- **Rename**: "Rename" edits the name in place. A session you do not name takes its name from your first message.
- **Marks**: "Mark" adds an emoji marker to a project, group or session: 🔥 Urgent, ⭐ Important, 🐛 Bug, ✅ Done, 🚧 In progress, 📌 Pinned, 💡 Idea, ⚠️ Caution. Choosing the current marker again removes it.
- **Multi-select**: ⌘/Ctrl-click toggles sessions, Shift-click selects a range. The context menu then offers "Open Selected Sessions", "Tile Selected Sessions" (two to four sessions), "Move Selected to…", "Archive Selected Sessions" and "Delete N Selected Items". Dragging a selection moves all of it.
- **Collapse state** is kept across restarts.

## 3. Tabs

Settings ▸ Behavior ▸ "Tabs" chooses how sessions open:

- **Single** (default): clicking an agent session in the tree reuses the current session tab, like following a link in a browser. The tab you leave moves to the background with all its panes, and its processes keep running. Terminal sessions open in their own tabs.
- **Multi**: each session opens in its own tab.

"Open in New Tab" always opens a new tab. In both modes, switching to another tab does not stop anything; **closing a tab (⌘W or ×) ends its processes**. For agent sessions this is safe: the conversation is kept, and opening the node again resumes it.

Background tabs appear in the status bar as "Background N/M"; click it to bring a tab back or end it. In Single mode, "Background limit" (default 32) caps their number. Past the limit, the oldest inactive background tab ends automatically. If every background tab is working or waiting for you, VelaTerm asks which one to end.

Tab bar details:

- ⌘1–9 (Ctrl+1–9 on Windows and Linux) switches to a tab by position. A middle click closes a tab.
- The ＋ after the tabs opens a scratch terminal. The buttons on the right open a new terminal, a new document, a built-in browser tab (desktop app), and the Game Center.
- A tab's context menu has "Close Tab", "Close Other Tabs", "Close Tabs to the Right", "Close All Tabs", "Send to Background" (keeps the tab's processes running), and the same session items as the tree.

Document and browser tabs are covered in [Document & Browser Tabs](document-and-browser-tabs_20260709_2041.md).

## 4. Split panes

A tab can hold several panes:

- ⌘D splits right and ⌘⇧D splits down (Ctrl+Alt+D and Ctrl+Alt+E on Windows and Linux); the pane header has the same buttons. The new pane is a scratch terminal that starts in the current pane's working directory.
- To show an existing session in the current tab, use "Open in Split Right", "Open in Split Down" or "Open in Focused Pane" from its menu, or drag it from the sidebar onto the center pane.
- "Tile Selected Sessions" arranges two to four selected sessions in one tab.
- Sessions shown in another pane of the current tab are highlighted in the sidebar.

![Split panes](../assets/manuals/split-pane.png)

Dividers can be dragged. Background keep-alive applies to the **whole tab**, so leaving a tab and returning preserves its layout. ⌘W closes the current pane; closing the last pane closes the tab.

## 5. Scratch terminals

"New Terminal", ⌘T, and the ＋ in the tab bar open a scratch terminal. It exists only as a tab: it never becomes a tree node and is discarded when closed. The tab is marked "scratch". Use scratch terminals for quick commands; keep long-running work in agent sessions. A scratch terminal's menu is short: "Open", "Open in New Tab", the split items, "Rename", "Session Info" and "Close Scratch".

## 6. Status dots and filtering

The dot next to each agent session shows its state: green while working, yellow when it needs you, magenta after it replied and you have seen it. The full legend is in [AI Agent Sessions](ai-agent-sessions_20260709_2041.md) §3.

The status bar shows how many sessions are "Working", "Pending" (need you) and "Viewed"; a counter with no sessions is hidden. Click a counter to show only those sessions in the sidebar; click it again to clear the filter.

The sidebar has its own tools:

- The **search box** ("Search sessions / groups…") filters the tree by name.
- The **filter button** ("Filter by status") selects one or more states and, separately, one marker.
- A status filter keeps the sessions that matched when you set it, so the list does not jump while you work. With "Dynamic status filter additions" (Settings ▸ Behavior, on by default), sessions that start to match are added. "Refresh status filter" at the bottom of the sidebar checks every session again; "Refresh Status" in a session's menu checks only that session.
- "Split tree view down" at the bottom of the sidebar adds a second view of the tree with its own search and filters, for example one view for sessions that need you and one for the full tree. Each additional view can be closed.

## 7. Right panel

- **Files**: the file tree of the session's working directory. Double-click a file to open it in a document tab.
- **Info**: session details, account usage, the current turn and resource use; see [AI Agent Sessions](ai-agent-sessions_20260709_2041.md) §8.
- **Git**: branch, staged and unstaged changes, untracked files, committing, and the commit history.
- **Knowledge Base**: the Session Knowledge Base, archived sessions and your local knowledge bases; see [Local Knowledge Bases](knowledge-notebooks_20260910.md) and [Session Knowledge Base](global-memory_20260905_2027.md).

## 8. Archiving: put away, do not delete

Finished work does not need to be deleted. Right-click a session → "Archive Session" to hide it: the session and its children leave the tree and their processes end, but **all data is kept**. "Archive Group" archives every session in a group, and "Archive Selected Sessions" archives a selection.

Archived sessions are in the knowledge base. Open them with the "Archived Sessions" button in the sidebar header. Archived sessions are grouped by project, and the search box ("Search archived content…") searches their content. For each archived session:

- The conversation can be read. Agent sessions show their messages; when no conversation is available, the terminal recording is shown if one exists. A second tab lists the knowledge entries generated from the session.
- "Restore to normal session" returns it to its place in the tree, with its conversation ID intact, so opening it resumes the original conversation.
- "Organize into Session Knowledge Base" extracts reusable knowledge from it.
- "Export full context as Markdown" (Claude and Codex) writes the whole conversation to a file.
- "Delete permanently (with recording)" is the only action that removes the data.

## 9. Global search (⌘⇧F)

"Search All Sessions" (⌘⇧F, Ctrl+Alt+G on Windows and Linux) searches **the content of all sessions**: agent conversations and recorded terminal output, not only names.

![Global search](../assets/manuals/global-search.png)

- Several words can be combined in any order; partial words and Chinese, Japanese or Korean text are found as well. Results are ranked by relevance.
- Results are grouped by session on the left; the preview on the right highlights the current match. ↑/↓ and Enter step through matches across sessions.
- "Open session" opens the session in the workspace.
- Archived sessions are excluded unless you tick "Include archived".

Terminal output can be searched only when session recording is on (Settings ▸ Advanced ▸ "Record session logs", off by default). Agent conversations can always be searched.

## 10. Deleting safely

Deleting a session removes it permanently, together with its recording. When you delete a group or project that contains **archived** sessions, those archives are kept, and restoring one brings back its project and group. If in doubt, archive first: archived content can always be restored.

## 11. Quitting and restoring the workspace

Quitting asks "Quit VelaTerm?" and stops running terminal and agent sessions. With "Save workspace" checked, the next start reopens the same tabs and splits. Restored sessions do not start on their own: each pane shows "Restored from your saved workspace. No process is running yet." with a "Start" button. The checkbox remembers your last choice.
