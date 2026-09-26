# Runtime Logs and Privacy

Created: 2026-09-09

Updated: 2026-09-25 10:21

Runtime logs help you find out where an operation failed or what it was waiting for. They do not record terminal input or output, conversation text, file contents, credentials or full URLs.

## Files and settings

The desktop app, the Electron backend and the headless server each write to `logs/runtime-<process ID>-<start ID>.log` in their application data directory.

- Requests from a remote window are logged by the backend on the remote machine. Setting up an SSH connection is logged by the local desktop app.
- Split events follow the same rule: native windows log to the local desktop app, and browser clients log to the backend they are connected to.
- A VelaTerm server started on a remote machine over SSH also writes its console output to `~/.velaterm/server.log` there (`server.log` and `server.err.log` in `%USERPROFILE%\.velaterm` on Windows).

Environment variables control where and how much is logged. They are read when a process starts, so restart the process after changing them.

- `VLX_LOG_DIR` sets the log directory for everything.
- When `VLX_LOG_DIR` is not set, the older variables `VLX_SPLIT_LOG_DIR`, `VLX_MEMORY_LOG_DIR`, `VLX_KNOWLEDGE_LOG_DIR` and `VLX_SECURITY_LOG_DIR` still set the directory for their own events. All directories use the `runtime-*.log` file names; older log files are neither moved nor deleted.
- `VLX_LOG_LEVEL` sets the level: `TRACE`, `DEBUG`, `INFO` (default), `WARN`, `ERROR` or `OFF`, in any letter case. An unrecognized value counts as `INFO`.
- The module variables `VLX_SPLIT_LOG_LEVEL`, `VLX_MEMORY_LOG_LEVEL`, `VLX_KNOWLEDGE_LOG_LEVEL`, `VLX_SECURITY_LOG_LEVEL` and `VLX_MODEL_CATALOG_LOG_LEVEL` filter their own events further. They cannot show more than `VLX_LOG_LEVEL` allows.

Each log file grows to about 10 MiB; each process keeps its current file and up to four older ones. While writing, VelaTerm removes its own runtime logs that are older than 7 days or that push the directory above about 100 MiB in total. This cleanup runs periodically rather than as a strict limit, and it never touches session recordings or other data. On macOS and Linux the log directory is readable only by you (`0700`, files `0600`), and VelaTerm refuses to write when the directory or file is a symbolic link. On Windows the files inherit the directory's permissions; VelaTerm does not change them.

The Electron shell writes its own events, such as a failed start or a backend restart, to `logs/electron-*.log` in its user data directory, or to `VLX_LOG_DIR`. These files follow the same limits (about 10 MiB per file, at most five files, 7 days and about 100 MiB). The shell does not copy the backend's output; read the backend's runtime log for that.

## When the window freezes

If the desktop app stops responding, look for `event=main_thread_stall` in the runtime log.

- When the app's main thread has been blocked for at least one second, VelaTerm writes `main_thread_stall` with `"status":"blocked"` and the time blocked so far (`blockedMs`), even while the freeze is still in progress.
- If the freeze continues, the line is repeated every five seconds.
- When the app responds again, VelaTerm writes `"status":"recovered"` with the total time.

A stall line says that the app froze, not why. Look at lines with the same timestamp to find the cause:

| Event | Meaning |
|-------|---------|
| `ui_thread_slow` | A command that runs on the main thread (for example terminal input, resizing a terminal, a system notification or a browser tab action) took more than 50 ms; the line names the command |
| `pty_lock` | Waiting for a terminal session took more than 50 ms |
| `database_lock` | Waiting for the database took more than 100 ms |
| `pty_resize_slow` | Resizing a terminal took more than 50 ms |
| `pty_fanout_slow` | Delivering a burst of terminal output took more than 50 ms |
| `rpc_queue` | A desktop request waited at least 250 ms before it started |
| `runtime_panic` | An internal error occurred; later `pty_lock` or `database_lock` lines with `"status":"poisoned"` mean the error left that resource unusable |

These lines are written at the `WARN` level, so they appear with the default settings; `VLX_LOG_LEVEL=ERROR` or `OFF` hides them. `main_thread_stall` and `ui_thread_slow` are recorded only by the desktop app. On macOS, holding a menu open or dragging or resizing a window for more than a second also produces a stall line; this is expected.

## When switching a terminal's shell is slow

Start with `event=client_shell_switch` and follow later lines with the same `operationId` and `sessionId`:

1. `rpc`, `rpc_queue`, `client_request`: how long requests such as updating the session or stopping the old process took to run, how long they waited, and the total time seen by the client.
2. `pty_kill`: locking the session, stopping the process and releasing its resources. `pty_kill_failed` records a failed stop.
3. `client_restart`, `client_pty_spawn`: the client restarting the view and requesting a new terminal.
4. `pty_prepare`, `pty_spawn`, `pty_shell`: preparing the session, reserving a start slot, allocating the terminal, choosing the shell and starting the process. `pty_shell_fallback` means the saved shell could not be found and the default shell was started instead.
5. `pty_first_output`, `client_pty_output`: the first output leaving the backend and reaching the client.

Each step has a start and a completion line with `durationMs` for the step and `elapsedMs` for the whole operation. First output only means that bytes arrived; it does not guarantee that the shell's startup files have finished or that the screen has been drawn.

## SSH connections

`event=ssh_connect` records the connection steps in order: `connect`, `probe`, `supply`, `prepare` (which includes copying the server to the remote machine), `start` and `forward`. When "Mirror the remote desktop app" attaches to a desktop app that is already running, only `connect`, `probe` and `forward` appear.

Other requests are recorded as `rpc` lines with their start, success or failure (`rpc_failure`). Keystrokes and terminal resizes are not logged individually. Logging never changes how requests are retried or canceled.

## What is recorded

The console and the file use the same format:

```
yyyy-MM-dd HH:mm:ss [LEVEL] [requestId or system] event=<name> {...}
```

The level is padded to five characters (`[INFO ]`, `[WARN ]`). The structured fields at the end of the line are limited to an allowed set: internal identifiers, fixed states and methods, counts, sizes, durations, error categories and selected AI usage figures. Explicit model names are replaced with `model=redacted` plus an anonymous `modelRef` that is valid only within the current process; `configured_default` means the configured default was used, not that the provider's actual model was observed. AI requests are described by their size and SHA-256 digest, and previews are replaced by a fixed placeholder. A digest is still diagnostic information about the content, not proof of anonymity.

The interface keeps only fixed error categories in its own diagnostic buffer. Error messages shown in the interface still contain the full detail so that you can act on them; that detail is not copied into the log. The browser sends at most 24 diagnostic requests at a time, and the backend accepts at most 120 per client per minute; excess events are dropped rather than retried.

HTTP lines record the route category, method, status and the time taken to prepare the response. They do not record query strings, headers, bodies or full paths. For downloads, WebSocket connections and other long-lived responses, the HTTP line does not mean that the transfer has finished.

## Limits

Log lines are written by a background thread with a queue of 2048 lines. When the queue is full, lines are dropped and counted; repeated disk errors produce a rate-limited warning. An internal error (panic) is recorded only with a fixed event, the source file name and the line number, never with its message. A forced exit, a disk failure or a long backlog can still leave gaps, because accepting a line into the queue does not mean it has been written.

The local desktop app can report the logger's state (`initialized`, `droppedCount`, `writeFailures`) through the `diagnostic_health` command; remote clients cannot call it. `droppedCount` also includes interface events dropped by the rate limit.

Logs stay on your machine. Session recordings, conversation history, the database, knowledge base source snapshots, audit reports and normal command output are separate data and are not part of the runtime log. Logs written by third-party tools are not managed by VelaTerm; do not attach them to a report without checking their contents.

Builds of the VelaTerm mobile app record their native SSH connection steps in the platform log (the `VelaTerm` tag on Android, the system log on iOS) as `event=mobile_connection` with the step, the previous step and its duration. These lines are not copied to a computer or uploaded.

See also: [Split diagnostics](split-diagnostics_20260905_2027.md).
