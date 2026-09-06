# VelaTerm User Manuals — Overview

Created: 2026-07-09 20:41

> `docs/manuals/` is the VelaTerm user manual series. This page is the table of contents and reading guide: new users should read the first three in order; dip into the rest as needed.

## Reading order

| # | Manual | What it covers | Who it's for |
|---|--------|----------------|--------------|
| 1 | [Getting Started](getting-started_20260709_2041.md) | Installation, importing a project, your first terminal and first agent session, UI tour | All new users — start here |
| 2 | [Interface & Session Management](interface-and-sessions_20260709_2041.md) | Three-pane layout, project tree, browser-style tabs with background keep-alive, split panes, status dots and filtering, archiving, global search | All users |
| 3 | [AI Agent Sessions](ai-agent-sessions_20260709_2041.md) | Capability differences across the nine supported agents, status awareness, auto-resume, forking, permission modes, install guidance, usage panel | Anyone using AI coding agents (the core chapter) |
| 4 | [Session Spawning & Git Collaboration](session-spawning-and-git_20260709_2041.md) | Spawning sub-sessions with vspawn, parallel work in isolated worktrees, graphical merge | Parallel tasks / multi-agent workflows |
| 5 | [Terminal Usage](terminal-usage_20260709_2041.md) | In-terminal search, copy/paste and image paste, vopen, shell selection on Windows, session recording, shortcut table | All users |
| 6 | [Document & Browser Tabs](document-and-browser-tabs_20260709_2041.md) | Built-in Markdown / source editor, PDF export, embedded browser tabs | As needed |
| 7 | [Settings & Shortcuts](settings-and-shortcuts_20260709_2041.md) | Item-by-item reference for the seven settings categories, default key bindings and rebinding | Reference |
| 8 | [Remote Development & Management](remote-development-guide_20260709_2041.md) | Browser remote access, SSH remote development, URL pairing, disconnect semantics and state retention, security model | Remote / mobile access users |

## Three mental models worth knowing first

1. **Each agent session node in the tree = one ongoing conversation.** Close it and reopen it — the conversation resumes automatically. Want a fresh conversation? Create a new node.
2. **Tabs behave like a browser**: a tab you switch away from stays alive in the background (its process keeps running); closing a tab is what actually ends the process — and even then, an agent conversation can be resumed.
3. **Archiving is not deleting**: archive finished work to tidy the tree; transcripts stay readable, restorable, and exportable at any time.

Additional manuals for the experimental features: [Global Memory](global-memory_20260905_2027.md) and
[Code Graph & Memory References](codegraph_20260905_2027.md).

## About the screenshots

Manual screenshots live in `docs/assets/manuals/` (the remote guide's are in `docs/assets/remote-guide/`). They were captured against a development environment with demo data, in English UI (the product default) and dark theme; account usage figures, paths, and similar details in some screenshots are sample values.
