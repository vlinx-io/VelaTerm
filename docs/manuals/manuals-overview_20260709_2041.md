# VelaTerm User Manuals — Overview

Created: 2026-07-09 20:41

Updated: 2026-09-25 10:22

> `docs/manuals/` contains the VelaTerm user manuals. This page is the table of contents and reading guide. New users should read the first four manuals in order and consult the others as needed.

## Reading order

| # | Manual | What it covers | Who it is for |
|---|--------|----------------|---------------|
| 1 | [Getting Started](getting-started_20260709_2041.md) | Installation, adding a project, the first terminal and the first agent session, a tour of the window | All new users; start here |
| 2 | [Interface & Session Management](interface-and-sessions_20260709_2041.md) | Window layout, the project tree and its menus, collections, marks, tabs and background tabs, split panes and tiling, status filters, archiving, global search, saving the workspace | All users |
| 3 | [AI Agent Sessions](ai-agent-sessions_20260709_2041.md) | The fourteen supported agents and their capabilities, status dots and notifications, resume, importing and forking conversations, permissions, presets, install guidance, the Info panel | Anyone using AI coding agents |
| 4 | [Conversation View](conversation-view_20260925_1012.md) | Switching between terminal and conversation view, sending while the agent works (queue, steer, interrupt), model, effort and permission controls, images, commands, rewinding, background tasks, automatic continuation after usage limits, signing in | Users of Claude Code, Codex, OpenCode, Pi and OMP |
| 5 | [Session Spawning & Git Collaboration](session-spawning-and-git_20260709_2041.md) | Starting child sessions with `vspawn`, the confirmation card, worktrees, the Git menu and graphical merge | Parallel work with several agents |
| 6 | [Planning and Execution](planning-and-execution_20260925_1012.md) | Planner and executor workflows, splitting work into several reviewed tasks, directory modes, following progress | Larger tasks that need planning and review |
| 7 | [Session Commands](session-commands_20260925_1012.md) | Reference for the ten built-in commands (`vspawn`, `vspawn-tree`, `vopen`, `vrefer`, `vsearch`, `vstat`, `vtell`, `vrun`, `vkb`, `vflow`), the Vela Skills including `vask`, and the `vela` shell command | Reference for users and scripts |
| 8 | [Terminal Usage](terminal-usage_20260709_2041.md) | Terminal basics, in-terminal search, copy and paste, image paste, command suggestions, the built-in commands and background `vrun` commands, shell selection on Windows, session recording, renderer options, common shortcuts | All users |
| 9 | [Command suggestions](terminal-completion_20260908.md) | Suggestions from the shell's own completions in plain terminals: settings and controls, supported shells, how the integration is loaded, the current state on Windows, limits | Terminal users on macOS and Linux |
| 10 | [Document & Browser Tabs](document-and-browser-tabs_20260709_2041.md) | Document tabs (opening files, the Markdown editor, editing and saving, new documents and PDF export), browser tabs in the desktop app, and how the tab kinds coexist | As needed |
| 11 | [Local Knowledge Bases](knowledge-notebooks_20260910.md) | Markdown folders as knowledge bases: opening or creating one, importing files and folders, writing and organizing notes, searching, links, copying from the Session Knowledge Base, storage | Note-taking and documentation |
| 12 | [Session Knowledge Base](global-memory_20260905_2027.md) | Organizing knowledge from sessions, processing history, browsing and maintaining entries, archived sessions, links to the code graph | Keeping decisions and lessons from agent work |
| 13 | [Code Graph & Knowledge Base](codegraph_20260905_2027.md) | Indexing a project's code, viewing symbols and their relationships, linking and reviewing knowledge entries, queries from a session, where data is stored (experimental) | Code exploration |
| 14 | [Code audits](code-audits_20260908.md) | Security audits of a project with Codex or Claude Code: models and thinking effort, Codex Security integration, scope, reports, exports (experimental) | Security reviews |
| 15 | [Settings & Shortcuts](settings-and-shortcuts_20260709_2041.md) | Every item in the eight settings categories, default key bindings and rebinding, the new agent session picker | Reference |
| 16 | [Remote Development & Management](remote-development-guide_20260709_2041.md) | Remote Access for other devices, connecting to other machines over SSH (including Windows hosts and mirroring the remote desktop app), a URL or your account, sharing through a VelaTerm account (experimental), file transfer, using VelaTerm on a phone, disconnecting and what survives, security | Remote and mobile use |
| 17 | [Model Catalog Sync](model-catalog-sync_20260909.md) | Where the model lists come from, how the Claude catalog is updated, checking the catalog | Reference |
| 18 | [Runtime Logs and Privacy](runtime-diagnostics_20260909.md) | Runtime log files and settings, investigating frozen windows, slow shell switching and SSH connections, what the logs record and what they leave out | Troubleshooting |
| 19 | [Split Diagnostics](split-diagnostics_20260905_2027.md) | How split commands reach windows, the split events in the runtime log, regression checks | Troubleshooting |

## Four ideas worth knowing first

1. **Each agent session in the tree is one ongoing conversation.** Close it and reopen it, and the conversation continues. For a fresh conversation, create a new session.
2. **Tabs behave like a browser.** By default, opening an agent session reuses the current tab, and the tab you leave keeps running in the background. Closing a tab ends its processes, and even then an agent conversation can be resumed.
3. **One conversation, two views.** Claude Code, Codex, OpenCode, Pi and OMP can run in the conversation view or in their own terminal interface. Switching views continues the same conversation.
4. **Archiving is not deleting.** Archive finished work to keep the tree tidy. Archived sessions stay readable in the knowledge base and can be restored or exported at any time.

## About the screenshots

The manual screenshots are in `docs/assets/manuals/`; those of the remote guide are in `docs/assets/remote-guide/`. They were taken in a development environment with demo data, in the English interface (the product default) and the dark theme. Some screenshots predate recent interface changes, such as the Knowledge Base tab in the right panel, the Feedback button and the Conversation view settings category; where a screenshot and the text differ, the text describes the current version. Account usage figures, paths and similar details are sample values.
