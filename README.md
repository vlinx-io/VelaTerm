# VelaTerm

A terminal manager built for the AI-agent era. VelaTerm organizes scattered terminal sessions into a
**project → group → session** tree, treats coding agents such as Claude Code and Codex as first-class
citizens, and lets you take over any session from a browser or another machine.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Website](https://img.shields.io/badge/Website-velaterm.com-0b7285.svg)](https://velaterm.com)
[![X](https://img.shields.io/badge/X-@vlinx__soft-000000.svg)](https://x.com/vlinx_soft)
[![YouTube](https://img.shields.io/badge/YouTube-@vlinx__soft-FF0000.svg)](https://www.youtube.com/@vlinx_soft)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2.svg)](https://discord.gg/gaD4NBzggU)

![VelaTerm main window](docs/assets/manuals/main-ui.png)

## Why

Working with coding agents breaks the assumptions traditional terminals were built on:

- **Session sprawl.** Several agents run at once — one refactoring, one running tests, one waiting for
  approval. A flat row of tabs stops scaling quickly.
- **No visibility into agent state.** A plain terminal cannot tell you whether an agent is working,
  blocked on a question, or finished.
- **Lost context.** Closing a terminal usually throws away the agent conversation with it.
- **Tied to one desk.** Long-running tasks keep going after you walk away, with no way to check in.

VelaTerm addresses each of these directly: a persistent session tree, live per-session agent status,
transcripts that survive restarts, and remote access from a browser or over SSH.

## Features

- **Session tree** — projects, arbitrarily nested groups, and sessions, with drag-and-drop reordering,
  search, and persisted collapse state.
- **Real PTYs** — every session is a full pseudo-terminal with complete input, output, and resize
  handling, kept alive in the background while you work elsewhere.
- **Agent awareness** — per-session status for supported agents (working, waiting for input, done),
  driven by the agents' own hook mechanisms, plus desktop notifications and session resumption.
- **Session spawning** — start a child session from inside a session, optionally in its own git
  worktree, and merge it back when the work is done.
- **Split panes** — horizontal and vertical splits, with keyboard shortcuts for switching sessions.
- **Document, image, and browser tabs** — open Markdown in a WYSIWYG editor, source files with syntax
  highlighting, images in a viewer, and URLs in a built-in browser tab.
- **Git integration** — branch, ahead/behind, and change counts per session, plus common actions.
- **Remote access** — reach your sessions from a browser with end-to-end encrypted device pairing, or
  connect to a remote machine over SSH and run sessions there.
- **Mobile view** — a browser layout tuned for phones, served by the same remote access stack.
- **Themes and i18n** — light/dark themes that follow the system, and a fully translated interface.

## Platforms

macOS (Apple Silicon and Intel), Windows (x64 and arm64), and Linux (x86_64 and aarch64).

## Tech stack

| Layer | Choice |
|-------|--------|
| Desktop shell | Tauri 2.x (Rust backend + system WebView); an Electron shell also lives in `electron/` |
| PTY | `portable-pty` (wezterm) |
| Frontend | React 19 + TypeScript + Vite |
| Terminal | xterm.js with the fit, web-links, search, image, and unicode11 addons |
| State | Zustand |
| Persistence | SQLite via `rusqlite` (bundled) |
| Styling | Tailwind v4 with CSS-variable themes |

## Getting started

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
pnpm dev:mobile       # same, with the mobile layout
pnpm dev:electron     # the Electron shell instead of Tauri
pnpm dev:ls           # list running dev instances
pnpm dev:stop <label> # stop one instance by label
```

Every dev instance picks a random port and carries a label, so several can run side by side. The web
and mobile modes default to an isolated database under `.dev-data/`, leaving your real session tree
untouched.

Build and test:

```bash
pnpm build                                        # type-check and bundle the frontend
pnpm tauri build                                  # build the desktop application
pnpm test                                         # frontend tests (vitest)
pnpm lint                                         # eslint
cargo test --manifest-path src-tauri/Cargo.toml   # backend tests
```

## Project layout

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

## Contributing

Two conventions matter most in this codebase:

1. **All user-facing strings are English and go through i18n** (`src/i18n/`). English is the key
   source; a missing key fails the type check. This includes strings returned from the Rust backend,
   which surface directly in the UI.
2. **All code comments are written in English.**

Any command touching the network or the filesystem must be asynchronous — synchronous Tauri commands
run on the main thread and freeze the UI.

## License

Copyright (c) 2026 VLINX Software. Released under the [MIT License](LICENSE).

You may use, copy, modify, merge, publish, distribute, sublicense and sell copies of VelaTerm, for
any purpose, as long as the copyright notice and the licence text travel with it.
