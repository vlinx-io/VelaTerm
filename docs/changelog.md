# Changelog

> Created: 2026-07-09 16:10 · Updated: 2026-08-26

All notable changes to VelaTerm are documented here, newest first.
v0.1.91 is the first public release; earlier version numbers were internal iterations and are not covered.

---

## v0.1.105 — 2026-08-26

### Workspace

- **Session status, unread marks and whether a session is still running are now decided by the backend, and every client sees the same answer.** Each client used to work these out from whatever it happened to observe, so "unread" really meant "not read in this window": reading a session in the browser left the desktop copy unread, and the dynamic status filter kept pushing that row back into the shared list, where even "Refresh status" could not remove it. Status events were also registered only once a client had spawned a session itself, so a browser that had just connected showed dots for the sessions it had opened and nothing for the rest. The backend now keeps one authoritative record per session — the agent, its state, whether its process is alive, and the unread flag — answers a bulk query when a client connects or reconnects, and broadcasts every change to everyone. Clients report what they observe; the backend draws the conclusions. Reading a session on a phone clears the mark on the desktop, a browser that has just connected shows the right dots for sessions it has never opened, and the arbitration rules that used to live in the frontend moved across with their reasons and their tests: a hook that has reported once locks out anything inferred from raw output, continuous output only implies busy for a session that has an agent, is not Codex and has no authoritative report yet, and a 1200ms hold keeps a task that has just started from being cancelled by a finish event arriving right behind it. Screen reading is the exception, because it needs the rendered grid and that only exists in a client: the client that owns the terminal size reports what it read, and the backend decides whether to accept it — a report from any other client is refused. If any of this goes wrong, setting `vlx-arbitration` to `frontend` in localStorage hands arbitration back to the old frontend chain.

- **Restarting a session no longer closes its tab on the other client.** `pty://killed` carried no payload, so the other side could not tell a restart from a close: it treated everything as a close, removed the pane, and then mirrored that layout back. The event now says which client killed the process and why, so a restart keeps the pane and waits for the new process to arrive. A missing or unrecognised reason still counts as a close — leaving a pane open for a session that is never coming back puts a dead terminal on screen, which is worse than closing one that was about to restart.

- **A browser connecting to a desktop that has just restored its workspace no longer starts every session for real.** Restoring a workspace renders placeholder cards instead of launching processes, but that decision only existed on the desktop; the browser followed the mirrored layout, could not tell "not running" from "running, just never opened here", and mounted the terminals — and mounting one is launching it. A leaf that arrives with someone else's layout now renders a placeholder when the backend says there is no process behind it. Opening a session yourself is still an intent to start it, and a terminal you are currently looking at is never replaced by a card when its process exits, because you may still want to read what it printed.

- **A settings change reaches the other clients immediately.** The backend has always held the authoritative settings, but it changed them without telling anyone, so the other client found out at its next launch. Over a remote connection that is more than a display mismatch: turning off "grow status filter dynamically" on one client did nothing while another one kept adding rows to the shared list, and the client with the smaller live-tab limit evicted background tabs out from under everyone else. Writing a setting now broadcasts which key changed, and each client re-reads it through the same path it uses at startup, so the rules that hide protected values from remote callers still apply — the broadcast carries key names only, never values. Phones, which previously neither sent nor received settings, now take part as well.

- **A browser waits for the mirrored layout before restoring its own.** A remote window has two sources of layout — the one in its own localStorage and the one the host pushes under mirror mode — and whichever arrived first was overwritten by the other. Putting the local one up first cost more than a flicker: mounting a terminal leaf launches a real process, and for a session whose process was already gone that meant starting a shell nobody would ever look at, which then stayed, because a browser detaches its terminals instead of killing them. The browser now waits for the first alignment to settle before restoring anything, for at most two seconds; if the backend is slow or unreachable it falls back to the local layout rather than sitting on an empty window. Phones, and any client with mirroring off, are let through immediately.

### Remote access

- **An SSH connection can turn mirror mode on for the service it starts.** The switch lives in the Remote access panel, but SSH starts a headless service on the far machine and there is no panel there to click. Hold Option while clicking "Connect remote" and the SSH form now offers a "Mirror UI across clients" checkbox, off by default. The value travels with the connection and is held in the service's memory rather than written to the far machine's database: when you also reuse the remote desktop's own database, one SSH connection should not quietly flip a switch on somebody's panel. The choice is remembered per host, so picking the same machine out of the history brings it back. Reusing a service that is already running now requires the version, the data mode and the mirror mode to all match — if any of the three differs the old service is replaced, which ends the sessions running on it, so this option sits behind Option alongside the database switch.

- **A client that is being mirrored says so.** Tabs and splits on a following client used to rearrange themselves with nothing on screen to explain where the change came from. The title bar now carries a Mirrored badge, with an explanation on hover. The host does not show one — it has the switch.

- **Files can be moved between your machine and the one running the terminal.** Remote access showed you the far machine's files but gave you no way to take one home or put one there — the only route was a shell command in a terminal. The Files panel now offers Download on a file's context menu and Upload in its header, and dragging files from your desktop onto a folder row sends them there. Both directions ride the same authenticated connection everything else uses, so this works from a browser on the local network, from a phone, and from a remote-connection window alike. Transfers move in chunks with a progress queue below the tree, and they keep running while you look at another panel. Download is an ordinary download link, so your browser's own download manager handles it: it streams to disk, shows speed and time remaining, and can be paused and resumed, at any file size and in every browser including a phone. The link carries a ticket minted for that one file and valid for a few minutes, because this server keeps credentials in a header and a browser fetching a link sends none. An upload writes to a temporary name and is renamed into place only once it is complete, so an interrupted transfer never leaves a half-written file where a whole one should be; a name that is already taken is refused before anything moves rather than after. Uploads show their speed and time remaining, and they survive the connection dropping: a failed chunk backs off and retries for about a minute, asking the server how far its temporary file actually got rather than resending a chunk that may already have landed. Giving up keeps those bytes — drop the same file on the same folder again and it continues from where it stopped, even after a reload, because only cancelling throws the partial file away.

### AI agents

- **Antigravity and Copilot sessions are named after their first message.** Both were missing from automatic renaming, which left rows of "Antigravity 1, 2, 3" in the sidebar. Antigravity's hook events carry no user text at all, only a conversation id and a transcript path, so the first message is read out of the transcript instead; the metadata block that follows it is left out of the title. Copilot's events carry no event name either, and are told apart by shape, so a body with a prompt and no tool name is now taken as a submission — which leaves the prompt a session is started with, and tool calls, correctly out of it.

- **An Antigravity session reopens with its history.** Resuming needs the conversation id, and the parser that pulls a session id out of the launch arguments did not recognise Antigravity's spelling of it, so `--conversation=<id>` never had an anchor to point at and every reopened session came up empty.

- **`vspawn --yes` spawns without the confirmation card.** Anyone who keeps "confirm before spawning" on had to click through a card for every child session in a run. The flag — also spelled `-y` or `--no-confirm` — skips the card for that one call and starts the session with the default settings. It does not change the setting itself, so the next spawn without it asks again.

- **The model field on the spawn card takes anything you type.** It was a plain dropdown, so only the models in the list could be chosen — and an agent understands far more identifiers than that: dated names like `claude-opus-4-6`, vendor-prefixed names, aliases configured locally. It is now a text field with the known models hanging off a dropdown beside it as shortcuts. The list is a suggestion, not a whitelist: whatever you type is what gets passed, an empty field means no `--model` at all, and the list filters as you type and folds away when a custom identifier matches nothing.

### Interface

- **One dropdown, used everywhere.** The dropdowns around the app had been copied from the same code more than a dozen times and then drifted apart: three panel backgrounds, four shadows, three hover colours, trigger heights of 26, 28 and 32 pixels, and ticks on selected rows that only a multi-select list needs. A single component now backs the branch pickers on merge, the language, default shell and font pickers in settings, the agent picker, the worktree dialogs, the agent type on new and restored sessions, and the last native select in the form modal. It also brings keyboard control, which none of them had: arrow keys to move, Enter to choose, Escape to close without closing the dialog behind it, Home and End to jump. The two menus in the status bar behave as they did, and the status filter in the sidebar keeps its ticks, because it really is multi-select.

- **The password field in the Remote access panel can be revealed.** It was a bare password input, so you could not see what you had typed; the eye button existed, but only inside the connect panel's own file. Both now share one component, and the revealed state resets when the panel closes.

- **The IP picker no longer looks like a system control.** It was a native select, which WKWebView dresses in system chrome that sits badly on a dark panel — the same complaint as the dropdowns replaced above. It now uses the shared component, and its label is shortened to "IP", since the text beside it already says what it is for.

### Fixed

- **Checking for updates asks the server every time.** A client left running was pinned to the first version it ever saw: having found 0.1.101 it went on offering 0.1.101 after 0.1.104 shipped, and "Check for updates" only reopened the same dialog, because the old code returned early whenever a notice was already pending. Every check is now a real request. A newer version replaces the notice on screen, the same version or a download in progress leaves it alone, and a server that reports no update at all takes down a notice that has gone stale — the version was pulled, or you installed it yourself in the meantime. The "Download manually" button now opens the download page on the website; it used to hand over the updater's own package, which unpacks in place and cannot be installed by hand.

- **A full-screen error overlay no longer appears when terminals are torn down quickly.** xterm's viewport schedules a scroll-area sync when it is constructed and another when it resets, and cancels neither on dispose, so a terminal opened and closed within the same task — which is exactly what rebuilding the session tree during a remote connection does — still ran those callbacks, found a renderer that had already been cleared, and threw. The throw comes from a timer, where neither try/catch nor an error boundary can reach it, so it is caught globally and matched narrowly: only a stack or message naming that sync, together with a mention of the renderer or its dimensions, is swallowed as harmless and written to the request log. Real crashes still raise the overlay.

---

## v0.1.104 — 2026-08-25

### AI agents

- **The child-task card now offers each agent's real models, and the effort flag that agent actually understands.** The card built its launch arguments with `--model` and `--effort` for everything, but only Claude, Kiro and Antigravity spell reasoning effort that way — Grok and Zoo call it `--reasoning-effort`, Cline calls it `--thinking`. Picking an effort level for any of the others handed the CLI a flag it had never heard of, and the session failed to start. Every agent now contributes its own flag names and its own values. The model control follows what each CLI can tell us: the ones that can list their catalogue (OpenCode, Grok, Crush, Antigravity, Cursor, pi, Kiro) are asked for it and offer the real list, the ones with a fixed set (Claude, Codex, Kimi Code) offer that set, and the rest give you a text field with an example of the shape they expect. An agent that is not installed or not signed in says so instead of spinning forever. Choosing "Default" now clears an inherited override rather than leaving the old value in place, and picking an effort level no longer drops the model inherited from the parent session.

- **A child task spawned from a Kimi Code session stays Kimi Code.** Kimi Code was missing from the list the spawn path uses to inherit the parent's agent, so its child sessions quietly came back as the default agent instead.

### Workspace

- **A group can be moved to a worktree after it was created.** The worktree was chosen when the group was created and fixed from then on; changing your mind meant deleting the group and building it again. Right-click a group and pick "Move to Worktree…" to create a new worktree, bind an existing one, or re-point a group that is already bound. Only the group itself changes: sessions already inside keep the directory they were created with — a running session cannot be moved to another directory underneath itself — while sessions created afterwards start in the worktree.

- **Mirror mode now covers the whole sidebar tree.** It shared the selection and the collapsed panels; the search box and the status and marker filters stayed local, on the theory that syncing them interrupts whoever is looking something up. That reasoning was backwards: mirroring means both windows hold the same state, not that one replays the other's keystrokes — a filter that is on here is on there. What actually interrupts people is the two sides showing different trees. Every sidebar projection now travels: the split layout, each projection's name, its search text, its status and marker filters, and its own collapse state. The snapshot format moved to version 2, and a client running an older version stops mirroring rather than applying half of a frame, so reload any window you left open through the upgrade.

### Interface

- **Closing the window on macOS asks the same question as quitting.** ⌘Q and the menu item went through the app's own confirmation, but the red close button destroyed the window outright — and that window holds the webview the confirmation dialog lives in. You either got no confirmation at all or the stripped-down native fallback, with no "save workspace" checkbox and untranslated text. All three platforms now hold the window open until you answer.

- **"Save workspace" is on by default, and it stays where you put it.** Losing a layout costs more than an unwanted snapshot, so the box starts checked. It also used to forget: the setting was written to the database through a 400ms debounce, and quitting killed the process inside that window, so the next launch reconciled against the old value and put your change back. The write is now flushed before exit, with a 600ms ceiling so a stalled backend cannot leave the confirm button spinning. (Contributed by FarhadGSRX.)

- **Passwords in the remote-connection panel can be revealed.** Both the URL password and the SSH password have an eye button that switches between masked and plain text. The revealed state is local to the field and resets when the panel closes, so a password is never left sitting on screen.

- **The sidebar filter badge counts every filter that is on.** A marker filter only lit the button up without saying anything, so the badge could read 1 while two filters were active. It now counts statuses and markers together and matches the ticks in the dropdown; a single status filter keeps its coloured dot.

### Fixed

- **Automatic updates on macOS work again.** The v0.1.103 packages carried AppleDouble companion entries (`._VelaTerm.app`), which the updater strips the first path component from — leaving an empty path — and it then refused to unpack the archive at all. Both architectures were affected, so every macOS user on v0.1.103 was stuck there. Packaging no longer writes those entries.

- **Native controls follow the app's theme when it differs from the system's.** Applying a theme set the app's own colours but never updated `color-scheme`, which was seeded once at startup from the system preference and never changed again, so checkboxes, dropdowns and scrollbars stayed dark under a light app on a dark system. (Contributed by FarhadGSRX.)

- **A fresh clone builds again.** The Rust crate embeds `../dist` at compile time, and the dev command does not produce it, so a newly cloned repository failed to compile before it could run. The build script now creates the directory when it is missing. (Contributed by FarhadGSRX.)

---

## v0.1.103 — 2026-08-24

### Fixed

- **Codex sessions on Windows no longer refuse to start.** Every Codex session failed immediately with `unexpected argument '--codex-hook'` because the lifecycle-hook TOML table passed on the command line contains spaces and double quotes, and npm-installed `codex.cmd` re-parses that through cmd.exe, which strips the quotes and splits the value into separate arguments. Hook injection is now skipped on Windows; status detection falls back to the existing notify / screen / busy heuristics, which still report idle and busy states — just less precisely than hooks. macOS and Linux are unaffected and continue to use hooks.

- **Rolled back the Windows IME pre-edit fix from v0.1.102.** The fix that restored the composition overlay for Chinese, Japanese and Korean input also gave it a visible background, border and rounded corners, which drew a small box around the pre-edit text inside the terminal — something that should not appear there. Because the overlay sizing, the helper-container geometry and the textarea cleanup were all interdependent, the entire change had to be reverted together. The underlying issue — typing CJK blind on Windows — remains open and tracked in issue #6.

---

## v0.1.102 — 2026-08-23

### AI agents

- **Agent presets: run several compatible CLIs side by side.** Every agent kind was hard-wired to one executable, so a fork, a nightly build or a second CLI that speaks the same protocol had no way in — you edited the launch arguments of an existing kind and lost the original. A preset now names its own executable, icon and launch arguments, and appears in the new-session menu next to the built-in kinds. Sessions record which preset created them, so forking one keeps the same executable, and a preset created on the desktop shows up on paired browsers and remote clients as well, icon included, because the icon travels as data rather than as a path on one machine. Existing sessions are untouched: a database from an earlier version starts exactly as it did before.

- **The child-task card picks the model and the effort level, and one answer settles it everywhere.** When an agent asks to spawn a child task, the confirmation card now offers the model and — where the agent supports it — the reasoning effort, prefilled from the parent's own launch arguments so the common case is a single click. Switching the card to a different agent re-derives both, so a model name from one CLI can no longer end up on another's command line. The card appears on every connected client, and answering it on one now dismisses it on the others; the first answer also claims the task on the server, so confirming on a phone and a desktop within the same second creates one worktree and one child session instead of two.

### Workspace

- **Mirror mode: one shared layout across every client.** The terminal stream was always shared — one PTY, one byte stream — but the arrangement around it lived only in each client's browser storage, so a browser opened over the LAN showed its own tabs and splits and rearranging one screen did nothing to the other. With mirror mode on, tabs, splits, the active session, the sidebar selection and the collapsed panels are published to every client and followed by all of them. The host controls the switch from the remote-access panel. Rearranging on either side takes effect on the other; a session that leaves this window's layout is detached rather than killed, so following a peer never ends someone's process; and applying a peer's layout does not steal the keyboard from whoever is typing locally. Phones stay out of it — the two-level phone navigation is a different shape of UI, and copying a desktop split tree onto it helps nobody.

- **The Git tab in the right sidebar became a usable Git client.** It only listed changed files before. It now stages and unstages individual files or whole groups, discards changes, writes a commit (with amend), and shows the commit history with each commit's files and diffs — grouped into staged, changed, untracked and committed sections that fold away. Paths are handled from the repository root, so a session opened in a subdirectory operates on the files it says it does, and a detached HEAD is labelled as such instead of showing a branch called HEAD.

### Interface

- **⌘Q now asks the same question closing the window does.** The Quit item in the application menu was the system's own, which terminates the process outright: pressing ⌘Q skipped the "save workspace" confirmation that the close button shows, so the same intent behaved differently depending on how you expressed it. Both paths now go through one confirmation. If the window that shows it has reloaded or crashed in the meantime, pressing ⌘Q again re-asks and falls back to a native dialog rather than leaving the app unquittable.

- **Shortcut hints show the keys that actually work.** The defaults differ per platform, and a browser reserves ⌘/Ctrl letter combinations for itself — ⌘D bookmarks, ⌘T opens a tab — so on macOS the app's own ⌘ shortcuts never reached the page when VelaTerm was opened as a URL. Plain browser tabs now use the Ctrl+Alt bindings on every operating system, while desktop apps and remote-connection windows keep ⌘. Tooltips and the empty-tab hint render whatever binding is in effect, including one you rebound yourself, instead of a hardcoded ⌘ combination; and the terminal blocks exactly the combinations the app has claimed, so rebinding an action moves that key with it.

- **Font presets cover Nerd Fonts and CJK, and a custom font that is not installed says so.** The preset list gained the common Nerd Font and CJK families, and a font typed in by hand is echoed back and checked: if the system does not have it, the settings page says so instead of silently falling back to a default that looks nothing like what you asked for.

- **Text fields on macOS no longer capitalise or correct what you type.** The system's automatic capitalisation, autocorrect and spellcheck applied to every input in the app, including session names and command fields, where "npm" became "Npm". They are off everywhere now.

### Windows

- **Typing Chinese, Japanese or Korean shows the pre-edit text and the candidate window again.** Both were invisible — you typed blind and only saw the result after pressing Enter. Two of our own CSS rules were responsible: the container holding the pre-edit overlay collapsed to zero width, and the overlay's `right` offset resolved inside it to nothing at all. The candidate window followed, because the operating system positions it from the overlay's rectangle. The overlay is drawn again and themed with the app's colours, and the invisible input element it sits on releases its geometry once composing ends, so clicking and dragging over that patch reaches the terminal rather than an empty element that used to keep covering it.

- **The native title bar follows the light/dark setting.** The app keeps the system title bar, and Windows paints it light until told otherwise, so a dark interface carried a white strip above it. It now matches the app, including windows opened later such as SSH and remote-connection windows. Choosing "follow system" hands control back to the OS instead of pinning a value.

- **The stray square on cold start is gone.** The single-instance plugin creates a hidden message window and never gave it the transparency its own style promised, so Windows occasionally grew the zero-sized window to its minimum and painted a small square during startup. It is properly transparent now; single-instance behaviour is unchanged.

### Performance

- **Remote access loads far less on first paint.** Static assets are now compressed on demand and served with cache validators, so a second visit revalidates instead of re-downloading, and the language packs and the terminal's optional renderers load only when something needs them rather than being part of the first payload. Together the initial transfer drops to roughly a fifth of what it was.

### Fixed

- **A spawned child now starts like the session that asked for it.** Children inherited neither the parent's permission mode nor its launch arguments, so a child of a session running with confirmations skipped came up asking for them, and a model pinned on the parent was dropped. Both are inherited now, falling back to the agent kind's global defaults — the same ones the "new agent session" menu applies.

- **Reconnecting a remote window no longer reports a false "authentication failed".** When the window reconnected, the new WebSocket and the one it replaced raced each other; the loser's teardown was reported as an authentication failure, and the banner accused a perfectly valid pairing of being rejected.

- **A group can no longer be dragged into its own subtree.** Dropping a group onto one of its own descendants detached that whole branch from the tree, and the sessions inside it disappeared from the sidebar until the database was repaired by hand. The move is now rejected.

- **Agent output keeps its colours when VelaTerm is launched from another tool.** A terminal inherits the environment of whatever started it, so launching from an IDE or an agent harness that exports `NO_COLOR`, `CI` or `FORCE_COLOR=0` made every agent TUI render monochrome inside VelaTerm, even though the terminal advertises full colour. Those inherited values are dropped when a session starts; the same variables exported from your own shell profile still apply, because that profile runs inside the session.

---

## v0.1.101 — 2026-08-15

### Remote access

- **Pick which address the share link uses — Tailscale addresses now show up.** The address list only accepted the classic private IPv4 ranges, so VPN meshes such as Tailscale, which assign addresses from the carrier-grade NAT range (100.64.0.0/10), were silently dropped from the remote-access panel and the pairing link — even though the server was already reachable through them. These addresses are now listed, with VPN tunnels ranked last so they never become the default. A new IP selector in the panel — shown before starting and while running — lists every candidate with its interface name and marks VPN tunnels; choosing one moves its URL to the front and regenerates the pairing link with exactly that host, so the link you copy works on a device that can only reach this machine over the VPN, without editing the URL by hand. A QR code under the pairing link lets a phone scan it directly. The choice is remembered; if the chosen interface disappears, the panel falls back to automatic without forgetting it. The server itself is unchanged and keeps listening on all interfaces. Choosing an address that only appeared after the server started — a VPN that connected later, say — now also updates the copied URL and the QR code immediately, instead of only the pairing link until the next restart; VPN tunnels are ranked behind LAN addresses on every platform, an address picked while the server is stopped drives the very first pairing link after it starts, and overlapping link regenerations can no longer overwrite a newer link with an older one.

- **Sharing survives a restart.** The pairing token used to be regenerated every time the server started, so quitting and reopening VelaTerm silently invalidated every shared link, and every phone had to be paired again. The token, the paired devices and the device blocklist are now saved to an owner-only file in the data directory: a previously paired device reconnects with its saved URL after a restart — the access password stays a required second factor — and a revoked device stays revoked. VelaTerm also remembers that sharing was on: quit while the server is running and the next launch brings it back on the same port, in the desktop app and on a headless `--serve` server alike; stop it yourself and nothing starts automatically. If the automatic start fails, because the port is taken for example, the app starts normally and the remote-access panel shows the reason. The port field now remembers the port you actually used instead of resetting to the default, and "Regenerate link" remains the explicit kill switch: it issues a new token at once, invalidates every old link, and overwrites the saved state. The access password itself is never written to disk — only a memory-hard hash (Argon2id) is stored.

### Security

- **A paired device can no longer manage the sharing itself.** Any paired browser could invoke the same management commands as the desktop app — create a new pairing link (which also clears the device blocklist), list and revoke other devices, or stop and reconfigure the server — and the settings store handed every client the complete settings map, including the memory-hard hash of the access password and the autostart settings the next launch reads. Management commands are now reserved for the desktop app and the Electron shell; the settings API filters the remote-access keys and the Gitea token out of every read from a paired device and rejects writes to them. A paired device keeps what pairing is for — its terminal sessions with full shell access — but it can no longer read the password verifier, invite or evict other devices, or redirect the port the next start uses. Commands that read, write or delete stored secrets — the Gitea token and remembered host passwords — are refused for a paired device as well, and the path-taking commands — reading, previewing, writing, creating, renaming and deleting, and likewise showing a file's git diff or picking the folder a repository is cloned into — resolve symbolic links first and reject paths inside VelaTerm's own data directory, where the pairing state and keys live, while every other path keeps working so remote file browsing and editing stay intact. A test enumerates every remote command that accepts a path, so a new command cannot slip past this check unnoticed. When one of these protections rejects a request, the browser now shows a properly translated message instead of a raw English error.

- **Revoking a device or regenerating the link now also survives the dual-instance setup.** On a headless `--serve` server with automatic start enabled, two server instances each held their own copy of the saved pairing state and wrote it back whole: a revocation or a fresh pairing link made through one could be silently undone by the other. All instances in one process now share a single pairing state per data directory: revocation and rotation take effect everywhere immediately, and exactly one writer persists the file, which remains the source of truth across real restarts.

- **Repeated failed logins are throttled.** Checking the access password uses Argon2id, deliberately expensive — and anyone who can reach the port can try. After five failed attempts from one address, further attempts are rejected for a minute before any hashing work happens, and the hashing itself now runs outside the server's event loop with a hard cap on concurrent verifications: a flood of wrong passwords can no longer saturate the server with memory-hard hashing or slow it down for devices that are already connected. The throttle lives in memory and resets with the server; the pairing token and the password remain the actual barrier. The limit is now shared by every server instance using the same data directory, so the dual-instance `--serve` setup no longer doubles the attempt budget, and an attempt is reserved before the password check starts, so parallel requests from one address cannot slip under the limit. A throttled browser now sees a dedicated rate-limit message on the login screen instead of being told the password was wrong, and being throttled is no longer remembered like a wrong password: once the pause is over, the next attempt goes through again without reloading the page. An attempt abandoned midway — the tab closed while the password was still being checked — now frees its reserved slot immediately instead of counting against the address for the rest of the minute, and a successful login releases only its own reservation instead of clearing the address's whole record: behind a shared network address, one person signing in correctly no longer resets an attacker's attempt budget, and recorded failures expire only with their minute.

- **Secrets on disk and in logs are handled more carefully.** The file with the pairing state and the end-to-end encryption key are now created owner-only readable from the start instead of being restricted after the first write, and the session database — which holds the password hash — is restricted to the owner as well. A headless `--serve` no longer prints the pairing link's long-lived secret into logs: when output is not a terminal the link is withheld and a hint is printed instead; `--print-pairing` explicitly opts back in. The device registry is capped at 32 entries with length-limited names so a paired client cannot grow the saved file without bound, and when saving a revocation or a new pairing link fails, the error now reaches the caller instead of a log line. Automatic start no longer replaces a server already started by hand, and a stale autostart error clears once you stop the server yourself.

### Fixed

- **Pairing can be managed from the Electron shell.** Creating a pairing link, listing paired devices and revoking a device existed only as desktop (Tauri) commands; the WebSocket dispatcher used by the Electron shell and browser clients answered "Unknown command", leaving the remote-access panel broken there. All three commands now go through the same core functions on both transports, so the two cannot drift apart, and regression tests cover the new dispatch routes — including creating a real pairing link against a running local server.

---

## v0.1.100 — 2026-08-10

### AI agents

- **Kiro CLI is now a first-class session type.** Kiro sessions get their own node in the tree, an authoritative Working/Waiting status dot driven by Kiro's own lifecycle hooks, notifications when a turn ends, automatic resume of the same conversation when you reopen the node, launch arguments and a skip-confirmations toggle, and spawning through vspawn — everything the other agents already had. VelaTerm clones your default Kiro agent into its own `vlx-term` agent, adds observe-only lifecycle hooks to the copy, and launches that — your own agent file is never edited, and your prompt, tools and MCP servers come along unchanged. Kiro has no permission-request hook, so the dot stays Working while it waits for your approval.

### Fixed

- **Programs started from the terminal no longer inherit the AppImage's own environment (Linux).** The AppImage launcher points `PYTHONHOME`, `PYTHONPATH`, `PERLLIB`, `QT_PLUGIN_PATH` and the GStreamer plugin paths at the bundle's temporary mount directory, and puts bundle directories ahead of everything else in `PATH` and `LD_LIBRARY_PATH`. A terminal hands its whole environment to the shell it starts, so the system `python3` looked for its standard library inside the bundle and refused to run at all, and other dynamically linked programs loaded the bundle's copy of a library in preference to the system one. VelaTerm now strips those bundle paths before starting a shell or an external tool, and leaves values you set yourself untouched. `APPDIR` and `APPIMAGE` stay visible, so programs that check whether they are running from an AppImage still get their answer. Only AppImage builds were affected; the deb package, macOS and Windows behave as before.

---

## v0.1.99 — 2026-08-09

### Terminal

- **Shift+Enter writes a new line instead of submitting.** Terminals have no encoding for Enter with a modifier, so agent CLIs such as Claude Code and Codex received a plain carriage return and sent the prompt while you were still writing it. VelaTerm now emits ESC+CR, the same sequence those tools expect from an iTerm2 key mapping, so multi-line prompts work — including on macOS, where the custom key handler was previously not installed at all. Composition in an input method is left alone, so Enter still confirms a candidate word.

### Projects and organisation

- **Refresh the status of a single session.** Sessions in a status-filtered pane gain a Refresh status action that re-evaluates only that session against the pane's own conditions, adding or removing it while every other session keeps its place. The action belongs to the pane its menu was opened from, so recursive splits never borrow another pane's filter, and the result is stored per pane and restored after restart.
- **Clearing a mark takes one click.** Choosing the emoji already applied now removes it, so the separate entry for clearing a mark and its divider are gone. The filter button also drops its emoji badge: the highlight already says a mark filter is active, and the menu says which one.

### Fixed

- **Linux AppImage desktop integration installs on any machine.** The bundled icon was a symbolic link to an absolute path on the build machine, so tools such as Gear Lever and AppImageLauncher failed to extract it, even though the application itself ran normally. The link is now relative. The published glibc requirement was also corrected to 2.35 after measuring the bundled libraries rather than the executable alone, which makes Ubuntu 22.04 the oldest supported release for the desktop application.

---

## v0.1.98 — 2026-08-02

### AI agents

- **Grok Build joins VelaTerm as a first-class agent.** Install, start and resume Grok 4.5 with stable session IDs, official lifecycle hooks, accurate working and permission states, merged transcripts, usage details and a theme-aware official icon across desktop, browser and mobile views.

### Projects and organisation

- **Split the project sidebar into independent working views.** Any tree pane can split downward again, with its own search, status and emoji filters, collapse state and resize ratio restored after restart. Every pane remains a projection of the same backend-owned project tree, so edits stay in sync without duplicating business data.
- **Mark nodes and filter without losing context.** Projects, groups and sessions can carry emoji markers. A marked container keeps its full subtree visible, status membership remains stable while you work, dynamic additions and manual refresh are both available, and status plus emoji conditions combine as a union.
- **Create an empty project in place.** Choose a parent directory, validate the name and create then import the folder in one flow. Partial failures retry the import without creating duplicate directories.

### Interface

- **Share VelaTerm where your community is.** The share dialog now covers WeChat Moments, Weibo, Xiaohongshu, X, Reddit, Hacker News, LinkedIn, Facebook, Telegram and WhatsApp, with a QR flow for WeChat and a share prompt in the update dialog.
- **Small interactions feel more deliberate.** Temporary terminal tabs can be renamed before they become saved sessions, and ordinary inputs disable automatic capitalisation on mobile keyboards without changing terminal input.

---

## v0.1.97 — 2026-07-25

### AI agents

- **Sessions no longer stay stuck on “working”.** Codex reported tool activity and turn completion through separate short-lived processes whose callbacks could arrive out of order, leaving a finished turn shown as still working. Mid-turn reports that arrive after their own turn ended are now discarded, and an additional session-end hook covers sessions that exit without a completion event.
- **Interrupted turns settle within seconds.** Pressing Esc, or a stream error, ends a Claude or Codex turn without any completion callback at all. Six seconds of terminal silence now quietly corrects such a session to waiting, without raising a “replied” notification.

### Interface

- **Reliable split shortcuts on macOS.** Split Right (Cmd+D) and Split Down (Cmd+Shift+D) are now registered as native Terminal menu commands, so macOS no longer intercepts the key combination before VelaTerm receives it.
- **Documents save exactly once per keystroke.** Cmd+S was handled by both the global shortcut and the focused editor, which could write the same file twice in a single keypress.

---

## v0.1.96 — 2026-07-23

### AI agents

- **Trust Codex lifecycle hooks, not terminal guesses.** Modern Codex sessions now use official lifecycle hooks as their only activity source. A `SessionStart` handshake verifies the bridge, missing callbacks are shown as “Status unavailable,” and terminal text or output activity can no longer overwrite working, confirmation or completed states.
- **Fresher Codex usage after every turn.** The Info panel shows the local rollout snapshot immediately, reconciles it with live rate limits, refreshes again after Codex finishes writing its final token snapshot, and ignores late responses from an older session.

### Interface

- **Reliable project-tree targeting on macOS.** Virtual rows no longer depend on compositor transforms, preventing stale WKWebView hit-test coordinates from sending hover, click or drag actions to a different row after scrolling or tree updates.

---

## v0.1.95 — 2026-07-21

### AI agents

- **Kimi Code and Zoo Code join the session tree.** VelaTerm can now launch, resume, install and configure both agents. Kimi uses its official lifecycle hooks for authoritative working, permission and waiting states; Zoo Code keeps a stable task identity and uses terminal detection where external hooks are not available.
- **Live Codex usage refresh.** The Info panel can actively query Codex's app server for current rate limits, while retaining the local rollout snapshot as a compatibility fallback.

### Projects and terminals

- **Open projects with `vela <path>`.** Packaged builds can install a VS Code-style shell command. A second invocation forwards the project to the existing VelaTerm window instead of opening a duplicate app instance.
- **Visible, cancellable Git clones.** Clone Project now reports Git stages, percentage and elapsed time, warns when progress stalls, and can cancel the entire Git process tree without leaving a half-cloned target. Credentials and query tokens are redacted from displayed errors and audit logs.
- **WSL terminals on Windows.** Every installed WSL distribution is detected and offered alongside PowerShell, cmd and Git Bash for plain terminal sessions. Agent sessions remain on the Windows host shell so hooks and executable paths stay reliable.

### Interface and reliability

- **Clearer background-session control.** Background-tab menus now show every session's live state, and the over-limit dialog can end several selected tabs at once.
- **Safer app lifecycle and release notes.** Quitting now confirms before stopping live sessions; Codex lifecycle identity wins over ambiguous rollout scans; and automatic-update notes can be delivered in every bundled interface language.

---

## v0.1.94 — 2026-07-12

### Localization

- **Vietnamese interface.** Tiếng Việt is now available in the language picker and is selected automatically for Vietnamese system locales.

### Browser

- **Faster starts in the built-in browser.** Every browser tab now has one-click shortcuts for ChatGPT, Claude, Gemini and Google. Project and group context menus can also create a permanent browser page directly in that part of the session tree.

### Images and documents

- **Reliable image-path paste on macOS.** When WebKit does not expose a copied image as a file, VelaTerm now reads it from the native clipboard and still uploads it as a file path instead of silently falling through to an agent's native image placeholder. Remote windows always show the image-paste setting, explain why file-path mode is required, and disable the unavailable native option.
- **Paste images into source documents.** The source editor now accepts clipboard images. Saved Markdown documents store them beside the document under `assets/` and insert portable Markdown image syntax; unsaved drafts embed the image data so it is not lost when temporary files are cleaned up.

### Interface

- **Context menus stay visible and target the right item.** Menus opened near the right edge are measured and shifted correctly. Right-clicking a tree node now highlights only the menu target without changing the existing selection, and group menus include a terminal scoped to that group.
- **Cleaner editing and status labels.** Source text no longer renders arrow-like font ligatures for sequences such as HTML comments, usage percentages are explicitly labelled as used, and the host WebView's unrelated native context menu no longer appears behind VelaTerm's own menus.

### Fixed

- **Codex stays in normal terminal history.** Codex sessions launched by VelaTerm now use inline terminal mode, so pressing Esc to interrupt or step back no longer switches terminal screen buffers and jumps the scrollback viewport to the top. Your own Codex configuration is left untouched.

---

## v0.1.93 — 2026-07-11

### Updates

- **See what changed before you install.** When a new version is available, a status-bar chip and a dedicated dialog now show this version's release notes. You can update now, skip this version, keep downloading in the background, or open the installer download manually; progress is shown live and the app restarts to finish. If you skipped several versions, the notes for every one in between are shown together, not just the latest.

### AI agents

- **Codex sessions name themselves.** A Codex session is renamed from your first message the moment you send it — the same way Claude sessions already were — so the tree no longer fills up with "Codex 1", "Codex 2". A name you set by hand is never overwritten.
- **vspawn and vopen now work in Codex too.** The built-in `vspawn`, `vspawn-tree` and `vopen` skills are installed into Codex's skill directory as well as Claude's, so `$vspawn <task>` and `$vopen <file>` work from a Codex session.
- **More reliable Codex resume.** Capturing a Codex conversation for later resume no longer gives up on a fixed timer, so it still works if you pause before typing the first message; and two Codex sessions started in the same folder can no longer end up wired to each other's conversation.
- **Model and context usage for Codex.** The Info panel now shows the current model, context-window usage and the tool in flight for Codex sessions, the same as it already did for Claude, read live from the session's rollout.

### Remote and terminal

- **Rebuilt remote file browser.** The directory picker for remote sessions gains search-as-you-type, jump-to-path (with `~`), Up / Home / Recent navigation, and inline New Folder.
- **Links open on your own machine.** Clicking a link in the terminal — including an agent's login link — now opens in your local browser, from remote windows too.
- **The "reuse remote database" option is tucked away.** It appears only when you Option/Alt-click Connect to Remote; a normal click always connects with an independent database.

### Fixed

- **Tools are found when launched from the Dock.** Starting VelaTerm from the Dock or Finder no longer left it with a stripped-down PATH, so git and other Homebrew-installed tools that agents rely on are located correctly.
- **New terminals open in the right folder.** Creating a terminal from a project or group in the sidebar now opens it in that node's directory, instead of reusing the last active session's folder.

---

## v0.1.92 — 2026-07-09

### Fixed

- **No more false "SSH link lost" warnings.** The tunnel monitor used to treat the exit of the port-forwarding child process as a dropped link, showing the red banner and rebuilding the tunnel. In reality the forwarding listener lives in the ControlMaster process, so forwarding keeps working after the child exits — the terminal stayed perfectly usable while the UI claimed the link was down, and the rebuild would tear down a healthy connection for nothing.

  The link is now checked in two stages: when the child process exits, VelaTerm first probes whether the local forwarding port still has a listener. If it does, nothing happens. Only when the port is gone as well is the link considered dead and rebuilt. The red banner is also debounced — it appears only after the first rebuild attempt fails, so a hiccup that heals within a few seconds no longer flashes a warning.

  Genuine disconnects still recover on their own. If the ControlMaster process is killed or the network drops for an extended period, the tunnel rebuilds automatically; once retries are exhausted the banner switches to "SSH link is down", and Reconnect now restores the session — remote processes are still running and pick up where they left off.

---

## v0.1.91 — 2026-07-09 00:00

**First public release.** Everything below shipped in this version.

### AI agents as first-class citizens

- Built-in support for nine agents — Claude Code, Codex, OpenCode, Copilot CLI, Cursor CLI, Cline, Pi, Antigravity (Gemini CLI) and crush. Pick a type when creating a session and it launches ready to go.
- Agent status is reported, not guessed. VelaTerm injects each vendor's official callback mechanism at launch (hooks for Claude, notify for Codex, a plugin for OpenCode, and so on), so every state change is pushed back immediately. The session tree shows a status dot for working, waiting on you, and finished-but-unseen; the sidebar filters by status and the status bar keeps a live count.
- System notifications fire when an agent needs a permission decision or finishes a task, and clicking one jumps straight to that session. Screen detection acts as a fallback for anything without a callback.
- Conversations survive everything. Each agent's own session id is captured and stored, so closing a session, restarting the app, or restoring an archived session all resume the original conversation — after verifying it still exists. Claude Code, Codex and Pi can also fork the current conversation into a parallel session.
- Sessions spawn sessions. Subtasks become child sessions, spawned by the agent itself or by you, optionally in an isolated git worktree so several agents can work on the same repository at once. A graphical merge dialog brings the branches back together when they are done.
- Missing agents get an install card with a recommended command and one-click install, with automatic path discovery afterwards. A side panel shows Claude and Codex usage quotas, and any conversation can be viewed in place or exported as a full Markdown transcript.

### Session organisation

- A three-level tree — project, group, session — with unlimited group nesting and child sessions beneath sessions. Nodes drag freely and collapse state persists across restarts.
- Any subtree can be archived: hidden into the archive panel without deleting data, restorable at any time, and still able to resume its conversation.
- Global content search (⌘⇧F) is backed by a SQLite FTS5 index and covers both agent transcripts and terminal output, with a result tree on the left, a linked preview on the right, and step-through navigation across sessions.

### A proper terminal, first

- Real PTYs (portable-pty, from wezterm), so vim, htop and the agent TUIs themselves all run exactly as they should.
- Multiple tabs with arbitrary splits — each tab holds its own split tree, divided horizontally or vertically however you like.
- Browser-style tab lifecycle: background tabs stay alive and their processes keep running, the live-tab budget is configurable, and exceeding it retires the oldest inactive session. Closing a tab is what ends a process.
- Typing stays responsive under load, thanks to input-priority scheduling and output frame coalescing — an agent flooding the screen no longer costs you keystrokes.
- In-terminal search (⌘F), session recording and replay, paste or drag images straight to an agent, and free switching between cmd, PowerShell and Git Bash on Windows.

### Beyond the terminal

- Run `vopen <file>` in any session to open a document tab in place, alongside your terminals: Markdown opens as a WYSIWYG editor and exports to vector PDF, images open in the built-in viewer, and source files get syntax highlighting.
- A built-in browser tab can be used as a scratch tab or pinned into the project tree as a permanent node.

### Remote and multi-device

- The desktop app serves the same interface over the web, sharing one live set of sessions. Output streams to every client in real time and any of them can type — desktop and browser see and drive the same terminal.
- End-to-end encryption with per-frame NaCl box, pairing by QR code or link, and per-device management including individual revocation.
- Headless mode: `--serve` runs the same service on a server with no GUI, ready for a browser to connect.
- SSH remote connections put remote machines in your tree. VelaTerm connects directly from the desktop, deploys a slim vela-server on the far side, and remote sessions behave like local ones. The tunnel heals and reconnects on its own.
- Mobile: the service ships a mobile web view, and a native app shell (xterm rendering with native input and an accessory key bar) is available as well.

### Platform and polish

- macOS, Linux and Windows.
- Automatic updates, with notarised macOS builds.
- Light and dark themes follow the system, and running agent sessions are told about the switch through the standard terminal notification (DEC 2031) — TUI colours change instantly, no restart needed.
- English by default, with 10 bundled languages following the system locale.
