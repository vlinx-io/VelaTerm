# VelaTerm

English | [简体中文](README.zh-CN.md)

**The best ADE.** Not just a terminal. Not just an IDE.

Manage agent sessions like Codex. Split terminals like iTerm2. VelaTerm keeps both in one native app,
and brings them to your browser and your phone.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Website](https://img.shields.io/badge/Website-velaterm.com-0b7285.svg)](https://velaterm.com)
[![X](https://img.shields.io/badge/X-@vlinx__soft-000000.svg)](https://x.com/vlinx_soft)
[![YouTube](https://img.shields.io/badge/YouTube-@vlinx__soft-FF0000.svg)](https://www.youtube.com/@vlinx_soft)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2.svg)](https://discord.gg/gaD4NBzggU)

<a href="https://www.youtube.com/watch?v=X665-GPmaKw"><img src="docs/assets/velaterm-intro-cover-3a-equation-en.jpg" alt="Codex session management plus iTerm2 split panes equals VelaTerm, the agentic development environment. Click to watch the intro video." width="100%"></a>

**[Watch the intro video (2:55)](https://www.youtube.com/watch?v=X665-GPmaKw)** ·
**[Download](https://velaterm.com/download)** ·
**[User manual](https://velaterm.com/docs)**

## Workspace

### Every major coding agent

Claude Code, Codex, OpenCode, Copilot, Cursor, Antigravity, Cline, Pi and more. Each one runs as a
managed session, with live status, resume and custom launch arguments.

<img src="docs/assets/readme/window.webp" alt="Claude Code, Codex, a dev server and a shell side by side in one VelaTerm window" width="100%">

### A full agent dev environment

Follow an agent's plan, edits, commands and test results as a conversation, or switch the same
session to the terminal view and work in its TUI.

<img src="docs/assets/readme/views-conversation.webp" alt="An agent session in the conversation view, with the plan, edits, commands and test results" width="100%">

### Sessions in a tree

Projects → groups → nested sub-groups → sessions. However many sessions you run, they stay in order.
Every session is a real pseudo-terminal that keeps running in the background, and sessions can be
split side by side.

<p align="center"><img src="docs/assets/readme/tree.webp" alt="Projects, groups, nested groups and sessions in the session tree" width="440"></p>

### Commands and paths, as you type

Command and path suggestions appear as you type. Choose with the arrow keys, then press Tab to accept.

<img src="docs/assets/readme/suggest-commands.webp" alt="Command suggestions appearing while typing a git command" width="100%">

### Know the moment an agent needs you

Session states follow the conversation live, in the session tree and the status bar, with a desktop
notification when an agent finishes and waits for you.

<img src="docs/assets/readme/status.webp" alt="Session states in the session tree, the status bar and a notification" width="100%">

### Usage and load, live

While the agent works, the right panel keeps plan quota, context, tokens and system load in view.

<p align="center"><img src="docs/assets/readme/info-panel.webp" alt="The info panel with plan quota, context, tokens and system load" width="440"></p>

## Agents together

### One command, one sub-session

An agent can hand a side task to a child session with `vspawn`: pick the agent, model and effort,
and give it its own worktree when needed. The child appears under its parent in the tree.

<img src="docs/assets/readme/vspawn.webp" alt="An agent starting a child session with vspawn" width="100%">

### Agents talk across sessions

Sessions running Claude, Codex, OpenCode or Pi can search each other's conversations, ask about them,
and send each other messages.

| Command | What it does |
|---------|--------------|
| `vsearch` | Search every session |
| `vrefer` | Read a conversation, or ask about it |
| `vtell` | Message another session; with `--steer` the message joins the turn that is already running |

<img src="docs/assets/readme/vrefer.webp" alt="A Claude session asking about a Codex session with vsearch and vrefer" width="100%">

<img src="docs/assets/readme/vtell.webp" alt="A Claude session steering a Codex session with vtell" width="100%">

### One big task, a team of sessions

Plan / Execute mode splits a large task across several agent sessions, each with one job. The
**planner** writes the plan, splits the work and reviews every result; the **executors** each build
one part in its own worktree and report back.

1. **Plan.** Pick agents, models and reasoning effort for planning and execution separately.
2. **Execute.** The planner proposes the split; you edit tasks and models before anything starts.
   Executors then work in parallel, one worktree each, as children of the planner in the session tree.
3. **Review.** Reports come back to the planner automatically; anything short of the bar goes back to
   the same session, with its context intact.

<img src="docs/assets/readme/pe-team.webp" alt="A planner session dispatching two executor sessions" width="100%">

<img src="docs/assets/readme/parallel.webp" alt="Executor sessions working in parallel under the planner" width="100%">

### Keep what your sessions learn

Organize a session into the Session Knowledge Base, keep Markdown notes in local knowledge bases, and
search both at once. With `vkb`, an agent looks things up before it acts:

| Command | Looks up |
|---------|----------|
| `vkb memories` | Session knowledge |
| `vkb notes` | Local notes |
| `vkb explore` | The code graph, queried locally without calling a model |

See the [notebook guide](docs/manuals/knowledge-notebooks_20260910.md).

<img src="docs/assets/readme/kb-sources.webp" alt="A knowledge base entry with its source sessions" width="100%">

<img src="docs/assets/readme/vkb.webp" alt="An agent looking up knowledge, notes and the code graph with vkb" width="100%">

## Built-in tools

### Editors included

A WYSIWYG Markdown editor, a code editor and an image viewer, plus a built-in browser on desktop. All
of them work on remote machines too.

<img src="docs/assets/readme/editor-markdown.webp" alt="The WYSIWYG Markdown editor with a file tree" width="100%">

### A little break between tasks

Open Game Center from your workspace and play Pixel Wing with a keyboard, touch controls or a
controller.

<p align="center"><img src="docs/assets/readme/game.webp" alt="Pixel Wing in Game Center" width="520"></p>

### Also included

- **Code audits.** Bundled Codex Security workflows using your logged-in local Codex or Claude Code,
  with source evidence, coverage limits and Markdown/JSON reports. See the
  [code audit guide](docs/manuals/code-audits_20260908.md).
- **Git integration.** Branch, ahead/behind and change counts per session, plus common actions.
- **Themes and languages.** Light and dark themes that follow the system, and a fully translated
  interface.

## Anywhere

### Run it anywhere

Remote access is built in: connect over SSH, or serve the app over HTTPS with end-to-end encryption.

- **Desktop.** Native app for macOS, Windows and Linux. Connect to remote machines over SSH from
  inside the app.
- **Browser.** Open a URL on any machine. No install needed. Served over HTTPS with end-to-end
  encryption; no readable data crosses the network.
- **Phone.** A native app for iOS and Android. Scan a QR code to carry the same session tree with you.

<p><img src="docs/assets/readme/anywhere-desktop.webp" alt="The desktop app connected over SSH" width="42%"> <img src="docs/assets/readme/anywhere-browser.webp" alt="The same workspace in a browser" width="42%"> <img src="docs/assets/readme/anywhere-phone.webp" alt="The same session tree on a phone" width="13%"></p>

### Connect remotely, three ways

Connect over SSH, open a pairing link, or sign in to your account and pick one of your devices.

<img src="docs/assets/readme/remote-ssh.webp" alt="Connecting to a remote server over SSH" width="100%">

### Your sessions, in your pocket

Scan a QR code to connect. Browse the tree, read agent replies as they stream and answer them, and get
a push notification when an agent needs you.

<p align="center"><img src="docs/assets/readme/mobile.webp" alt="The iOS and Android app showing the session tree and an agent reply" width="560"></p>

### macOS, Windows and Linux

One native app on every desktop. On Windows, Git Bash comes bundled with the full installer. Built on
Tauri 2: a small install that stays smooth under heavy terminal load.

| Platform | Architectures | Shells |
|----------|---------------|--------|
| macOS | Apple Silicon, Intel | zsh |
| Windows | x64, arm64 | PowerShell, Git Bash, WSL |
| Linux | x86_64, aarch64 | bash |

<p><img src="docs/assets/readme/platform-macos.webp" alt="VelaTerm on macOS" width="32%"> <img src="docs/assets/readme/platform-windows.webp" alt="VelaTerm on Windows with Git Bash" width="32%"> <img src="docs/assets/readme/platform-linux.webp" alt="VelaTerm on Linux" width="32%"></p>

## Download

From the first command to the final review. Get the installer for your platform at
[velaterm.com/download](https://velaterm.com/download), then start with the
[getting started guide](docs/manuals/getting-started_20260709_2041.md).

## Documentation

- [Manuals overview](docs/manuals/manuals-overview_20260709_2041.md) — start here
- [Getting started](docs/manuals/getting-started_20260709_2041.md)
- [AI agent sessions](docs/manuals/ai-agent-sessions_20260709_2041.md)
- [Remote development guide](docs/manuals/remote-development-guide_20260709_2041.md)
- [Changelog](docs/changelog.md)

## Community

- **[X](https://x.com/vlinx_soft)** — release announcements and short demos.
- **[YouTube](https://www.youtube.com/@vlinx_soft)** — demos and guided tours of the application.
- **[Discord](https://discord.gg/gaD4NBzggU)** — questions, bug reports and everyday discussion.
- **[velaterm.com](https://velaterm.com)** — downloads, manuals and the changelog.

## Development

### Tech stack

| Layer | Choice |
|-------|--------|
| Desktop shell | Tauri 2.x (Rust backend + system WebView); an Electron shell also lives in `electron/` |
| PTY | `portable-pty` (wezterm) |
| Frontend | React 19 + TypeScript + Vite |
| Terminal | xterm.js with the fit, web-links, search, image, and unicode11 addons |
| State | Zustand |
| Persistence | SQLite via `rusqlite` (bundled) |
| Styling | Tailwind v4 with CSS-variable themes |

### Build from source

Prerequisites: Node.js, [pnpm](https://pnpm.io/), the [Rust toolchain](https://rustup.rs/), and git.
Tauri also needs its platform dependencies — see the
[Tauri prerequisites guide](https://tauri.app/start/prerequisites/).

```bash
pnpm install          # install frontend dependencies
pnpm dev:desktop      # build the Rust backend and open the desktop window
```

Other development modes:

```bash
pnpm dev:web          # headless backend + Vite, driven from a normal browser
pnpm dev:electron     # the Electron shell instead of Tauri
pnpm dev:ls           # list running dev instances
pnpm dev:stop <label> # stop one instance by label
```

Every dev instance picks a random port and carries a label, so several can run side by side. `dev:web`
binds `0.0.0.0`, so it also prints a LAN address that another computer or a phone can open, and it
defaults to an isolated database under `.dev-data/`, leaving your real session tree untouched. On a
machine that must not expose ports to its network, `VLX_DEV_BIND=loopback pnpm dev:web` binds
`127.0.0.1` only (see the [remote development guide](docs/manuals/remote-development-guide_20260709_2041.md#81-for-contributors-a-dev-instance-that-stays-off-the-lan)).

Build and test:

```bash
pnpm build                                        # type-check and bundle the frontend
pnpm tauri build                                  # build the desktop application
pnpm test                                         # frontend tests (vitest)
pnpm lint                                         # eslint
cargo test --manifest-path src-tauri/Cargo.toml   # backend tests
```

### Project layout

```
src/              React frontend
  layout/         three-column + bottom-bar regions
  store/          Zustand state
  ipc/            invoke / listen wrappers
  terminal/       xterm instance registry
  i18n/           translations, with English as the key source
  remote/         browser remote access and pairing
  mobile/         phone browser layout
src-tauri/src/    Rust backend
  pty/            PTY manager
  db/             SQLite persistence
  agent/          agent detection, status, transcripts, spawning
  web/            embedded web server and command dispatch
  git.rs          git status probing
electron/         Electron shell
skills/           agent skills exposed inside VelaTerm sessions
docs/manuals/     user manuals
```

### Contributing

Three conventions matter most in this codebase:

1. **All user-facing strings are English and go through i18n** (`src/i18n/`). English is the key
   source; a missing key fails the type check. This includes strings returned from the Rust backend,
   which surface directly in the UI.
2. **All code comments are written in English.**
3. **Please do not run `cargo fmt`.** The Rust sources are hand-formatted and this project does not
   use rustfmt. Running it rewrites large parts of files a change never touched, which buries the
   actual diff. Match the style of the surrounding code instead.

Any command touching the network or the filesystem must be asynchronous — synchronous Tauri commands
run on the main thread and freeze the UI.

## License

Copyright (c) 2026 VLINX Software. Released under the [MIT License](LICENSE).

You may use, copy, modify, merge, publish, distribute, sublicense and sell copies of VelaTerm, for
any purpose, as long as the copyright notice and the licence text travel with it.
