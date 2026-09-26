# Split Diagnostics

Created: 2026-09-05 20:27

Updated: 2026-09-25 10:21

Split commands from the application menu, including ⌘D and ⌘⇧D on macOS, go only to the window that has focus. When no window has focus, nothing happens. A local window, an SSH window and a pairing-link window therefore never split at the same time because of one menu command.

"Mirror layout across devices" still synchronizes layouts as configured. A client with mirroring on can receive a split that another client created; such events are recorded with the source `mirror`.

## Where the log is written

Split events go to `logs/runtime-*.log` in the application data directory and share the background writer used by the other diagnostics. Each process writes its own file, which is rotated by size and age. Locations, permissions and limits are described in [Runtime logs and privacy](runtime-diagnostics_20260909.md).

SSH and pairing-link windows log to the desktop app that hosts them. Browser clients and the Electron app log to the data directory of the backend they are connected to. `VLX_LOG_DIR` sets the directory for all logs; when it is not set, `VLX_SPLIT_LOG_DIR` still sets the directory for split events. `VLX_SPLIT_LOG_LEVEL` filters split events further.

## What is recorded

The console and the file use the same format:

```
yyyy-MM-dd HH:mm:ss [INFO ] [requestId or system] event=split {...}
```

The timestamp at the start of the line is the server time. The `clientAtMs` field is the client's Unix time in milliseconds when the split was triggered, which helps compare events across windows.

A split line contains the client identifier, `clientAtMs`, the source, the session IDs, the parent session ID, the tab ID, the split direction and the number of sessions. Identifiers that are not in a valid format are left out. The `source` field has these values:

| `source` | Meaning |
| --- | --- |
| `shortcut` | Keyboard shortcut in the interface |
| `menu` | Application menu, including its keyboard shortcuts |
| `pane-button` | Split button in the pane header |
| `sidebar` | Split item in a session's context menu |
| `drop` | A session dragged onto a pane |
| `tile` | Several sessions tiled at once |
| `mirror` | A split received through layout mirroring |
| `unknown` | An internal call without a source |

The log does not contain terminal input or output, session names, working directories, server addresses or credentials. Mirror events record only the session IDs that arrived; they do not invent a local parent session or direction.

The last 200 split records of the current window are also available in the developer tools as `window.__vlxSplitLog`. Each record's `persistence` value is one of:

- `pending`: being sent to the backend;
- `saved`: the backend accepted it into its write queue;
- `disabled`: the backend is configured not to log `INFO` events (`VLX_SPLIT_LOG_LEVEL` or `VLX_LOG_LEVEL` is set above `INFO`);
- `failed`: the request failed or the write queue was full.

Reloading the window clears these in-memory records; files already written are not affected. A failed log request never blocks the split: the console reports the failure, and the request is not retried. `saved` does not guarantee that the line reached the disk, because disk errors are reported through the backend's diagnostic state. Requests still pending when a window closes may not be written.

## Regression checks

```sh
cargo test --manifest-path src-tauri/Cargo.toml --lib --features native-menu-tests native_menu::tests
cargo test --manifest-path src-tauri/Cargo.toml --lib --no-default-features split_trace::tests
pnpm exec vitest run src/store/splitTrace.test.ts src/hooks/shortcutRegistry.test.ts src/hooks/useKeyboardShortcuts.test.tsx src/layout/TitleBar/AppMenuBar.test.tsx src/layout/CenterPane/keepAlive.test.tsx
pnpm exec tsc --noEmit
```

The menu test uses Tauri's mock runtime with three windows. It checks that a command reaches only the intended window and that nothing is sent to other windows when there is no focused window or the focused tab no longer exists. It does not open real desktop windows.
