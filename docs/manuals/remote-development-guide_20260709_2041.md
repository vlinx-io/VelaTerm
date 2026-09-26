# Remote Development & Management

Created: 2026-07-09 20:41

Updated: 2026-09-25 10:21

> This guide covers four things: opening your VelaTerm to other devices on your network, connecting to other machines over SSH or a pairing link, sharing conversations with your own devices through a VelaTerm account, and moving files to and from remote machines.

## 1. Entry points

The remote features live on the right side of the title bar:

![Title bar buttons](../assets/remote-guide/titlebar-buttons.png)

- The globe icon is **"Remote Access (Browser)"**. It lets a phone, tablet or another computer on your network use this VelaTerm in a web browser. While the server runs, the icon is highlighted in the accent color and its tooltip adds "Running · port N".
- The arrow icon is **"Connect to Remote Server"**. It makes this machine a client of another machine, over SSH, through a pairing link, or through your VelaTerm account.
- The person icon is the **"Account"** entry, marked "Experimental". It signs this computer in to a VelaTerm account and lets you share conversations with your other signed-in devices over the internet.

The "Share" button next to them opens "Share VelaTerm", which is for recommending VelaTerm to others; it does not share any sessions.

| What you want | Where to go |
|---------------|-------------|
| Use this machine's sessions from a phone, tablet or another computer on the same network | Globe button (§2) |
| Work on a remote Linux, macOS or Windows machine that only has SSH | Connect button → "SSH" (§3.1) |
| Connect to another VelaTerm that has Remote Access running | Connect button → "URL" (§3.2) |
| Continue your AI conversations from another device over the internet | Account button (§4), then Connect button → "Remote" on the other device |
| Use VelaTerm on a phone | §6 |

## 2. Opening this machine to other devices (Remote Access)

### 2.1 Start the server

Click the globe button. Choose a "Port" (8799 by default; the last port you used is remembered), an "IP" address for the pairing link ("Automatic (first LAN address)" or a specific interface; VPN interfaces are marked "VPN"), and an access password ("Set access password"), then click "Start Server".

![Remote Access panel · not running](../assets/remote-guide/remote-access-off.png)

The access password is required for every device that connects. While the server runs, the panel notes: "Remote access restarts automatically when the app is reopened. Stop Server turns this off." If the automatic start fails on a later launch, the panel shows "Automatic start failed:" followed by the reason.

The "Mirror layout across devices" checkbox is on by default. With it on, tabs, splits and the active session stay the same on every connected device, while keyboard focus stays independent on each one. Turn it off if each device should arrange its own tabs.

### 2.2 Connect a device

Once the server is running, the panel shows the status, the "Certificate fingerprint (SHA-256)", a pairing link and a QR code:

![Remote Access panel · running](../assets/remote-guide/remote-access-running.png)

To connect a device, scan the QR code with its camera, or send the pairing link to it (for example with AirDrop or a message to yourself), open the link in a browser and enter the access password. The pairing link and the password are both required; opening the plain address without a pairing link shows "This server requires a pairing link." Traffic between the browser and this machine is end-to-end encrypted.

Two points matter for security:

- The pairing link carries access credentials. Share it only with your own devices. If you think it has leaked, click "Regenerate link (disconnects all)": the old link stops working immediately and every connected device is disconnected until it opens the new link.
- On the first visit, the browser warns that the certificate is not trusted, which is expected for a self-signed certificate. Compare the fingerprint the browser shows with the fingerprint in the panel. If they match, you are connected to your own machine.

If the machine has several network interfaces (wired, Wi-Fi, VPN), the panel shows the preferred address and lists the others under "N more urls". Pick the address on the same network as the device you are connecting.

Pairing links remain valid when VelaTerm restarts. Only "Regenerate link (disconnects all)" replaces them.

### 2.3 What the browser client can do

The login page and the main interface on a connected device:

![Browser login page](../assets/remote-guide/login.png)

![Main UI in the browser](../assets/remote-guide/browser-main.png)

A browser client works with the same live sessions as the desktop app: the same project tree and the same terminal and conversation output, and either side can type. With "Mirror layout across devices" on, the host's title bar shows "Mirrored by N"; click it to see the "Attached clients" with their names, addresses and connection times. Client names are reported by the clients themselves, so treat them as labels rather than proof of identity.

When the same terminal is open on several devices, its size follows one of them. The other devices show a scaled copy with a "⤢ Mirror … · click to fit this window" badge in the top-right corner of the terminal; click it to size the terminal for the current window.

Browser clients do not have browser tabs or the native image paste option. They can upload and download files in the Files tab (§5).

### 2.4 Paired devices

Click "Paired devices" in the running panel to see every device that has connected, with its "Last seen" time:

![Paired devices](../assets/remote-guide/remote-devices.png)

To block a device, click "Block" and then "Confirm block". The device is disconnected at once and cannot reconnect until it opens a new pairing link; other devices are not affected. To disconnect every device at once, use "Regenerate link (disconnects all)".

### 2.5 Stop the server

Click "Stop Server" at the bottom of the panel. Browser clients are disconnected, and running sessions continue because they belong to the desktop app. Stopping also turns off the automatic start; the next time you start the server, enter the access password again.

## 3. Connecting to other machines

Click the connect button in the title bar. The panel has three tabs: "SSH", "URL" and "Remote".

### 3.1 SSH: the remote machine only needs SSH

Use this for a remote machine without any VelaTerm components. The remote machine can run Linux (x86_64 or ARM64), macOS (Apple silicon or Intel) or Windows (x86_64); Windows machines can use Microsoft's OpenSSH server or a Cygwin or MSYS2 SSH server. Connecting runs the whole setup automatically: VelaTerm detects the remote system, copies the matching VelaTerm server to it, starts the server so that it keeps running on its own, forwards a local port to it, and opens a remote window that signs in automatically.

Enter `user@host[:port]` and click "Connect". Details worth knowing:

**Host key verification.** The first connection to a host shows its "SSH host key fingerprint" with the note "First time connecting to this host — verify the fingerprint before continuing." Check it and click "Fingerprint matches, connect"; the key is saved to `known_hosts` and you are not asked again. If a known host's key changes, a red warning appears: "⚠ This host's key has changed — could be a server reinstall or a man-in-the-middle attack. Continue only if you're sure." Find out why before you confirm.

![First-connect fingerprint verification](../assets/remote-guide/connect-ssh-fingerprint.png)

**Authentication.** Your existing SSH setup (ssh-agent, `~/.ssh/config` and keys) is used as it is. An "SSH password" prompt appears only when key authentication fails. "Remember password" stores the password in the system keychain, never in the database, and uses it next time.

**Mirror the remote desktop app.** This option is off by default and remembered per host. Turn it on when the remote machine also runs the VelaTerm desktop app and you want to see exactly what it shows: the same tabs, splits and active session, with changes on either side appearing on both. If the desktop app is not running on the remote machine, VelaTerm opens its database directly, or a separate database when there is none. Without this option, the remote server uses its own data directory (`~/.velaterm/data`, or `%USERPROFILE%\.velaterm\data` on Windows), separate from any desktop app installed there.

Holding Option (Alt on Windows and Linux) while clicking the connect button shows one more option when mirroring is off, "Use remote desktop app's database". It lets the remote server open the remote desktop app's database without attaching to the running app. Keeping both on the same VelaTerm version is recommended.

**Recent hosts.** Hosts you have connected to are listed under "Recent hosts" ("Show all (N)" shows the full list). Clicking an entry only fills in the form; review it, then click "Connect". A key icon means a saved password, and × ("Forget this host") removes the host and its saved password.

**Progress.** The first connection copies the server (tens of MB), and the button shows "Preparing server…", "Transferring server…" and "Starting server…" with a percentage. Later connections reuse the server that is already on the remote machine and are much faster.

### 3.2 URL: the other machine has Remote Access running

Use this when another machine's VelaTerm has Remote Access running (§2) and you have its pairing link. Compared with opening the link in a web browser, you get a dedicated window, automatic sign-in with a password saved in the system keychain, and fingerprint checks handled by the app.

![Connect to Remote · URL mode](../assets/remote-guide/connect-url.png)

Paste the pairing link, enter the "Login password" set on the other machine, and click "Connect". The first connection shows the other machine's TLS certificate fingerprint; compare it with the "Certificate fingerprint (SHA-256)" shown in that machine's panel. Later connections with an unchanged fingerprint go straight through, and a changed fingerprint produces a red warning. Recent pairing links are listed below the form, like SSH hosts.

### 3.3 Remote: your devices signed in to the same account

The "Remote" tab lists the devices signed in to your VelaTerm account (§4). Each device shows "Online" or "Offline" and what it shares, for example "Entire workspace", a project or a conversation, or "Nothing is shared on this device." When a device is online and sharing, "View shared content →" opens its shared content in a new window. If you are not signed in, the tab shows a "Sign in to VelaTerm" link.

## 4. Sharing through your VelaTerm account (experimental)

Account sharing lets you continue AI conversations running on one computer from your other devices over the internet, without opening ports or being on the same network. The shared content travels through velaterm.com end-to-end encrypted. This feature is experimental.

### 4.1 Sign in

Click the account button in the title bar. The "Account" dialog shows "Sign in to VelaTerm"; click it to continue in your web browser. While VelaTerm waits, it shows "Complete sign-in in your browser, then return to VelaTerm." You can expand "Sign-in request details" to compare the request with what the website shows.

Once signed in, the dialog shows your name and "Account ID:", an "Account" button that opens your account page on velaterm.com, and "Sign out". Signing out disconnects this computer and stops everything it shares.

### 4.2 Share content

Under "Start sharing", choose a "Scope": "Entire workspace", "Project" or "Session". For a project or session, pick it from the list, then click "Start sharing". Each shared item appears under "Sharing active" with its status ("Sharing active" or "Shared content not ready") and a "Stop sharing" button. You can share several items at once. Shares remain in place after a restart, and they are available only while this computer runs VelaTerm and is online.

Only AI conversations are shared. Plain terminals and browser pages are never shared, and a device viewing shared content cannot open terminals, files, settings or new tabs.

### 4.3 Open shared content from another device

On another device signed in to the same account, open the connect button's "Remote" tab (§3.3) and click "View shared content →". You can read the shared conversations. You can also send messages, interrupt the agent and answer its permission requests, but only for sessions that the sharing computer shows in the conversation view; for other sessions the window explains that the host must switch the session to the conversation view first.

Keep in mind that an AI agent acts with the permissions of the user on the sharing computer: it can run commands and change files there. Anyone who can continue a shared conversation can direct the agent, so share only with devices you control, and use "Stop sharing" when you no longer need access.

## 5. Transferring files

In browser clients and in URL and SSH remote windows, the right panel's Files tab can move files between your device and the machine that runs the sessions. The local desktop app does not need this and does not show these actions.

- **Upload:** right-click a folder, or a file inside it, and choose "Upload Files…"; click the upload button in the Files header ("Upload files to this folder"); or drag files onto the file tree. Folders cannot be uploaded.
- **Download:** right-click a file and choose "Download". In a web browser, the file goes through the browser's own download manager. In a URL or SSH remote window, a save dialog asks where to store the file (the Downloads folder by default).

Uploads, and downloads in remote windows, appear in the "Transfers" list with their size, speed and remaining time. Click the cancel button to stop an item, and "Clear" to remove finished items. If the connection drops during an upload, the item shows "Reconnecting…" and VelaTerm keeps retrying for about a minute. An interrupted upload never leaves a partial file at its destination, and uploading the same file again continues where it stopped.

## 6. Using VelaTerm on a phone

Open a pairing link (§2.2) in the phone's browser, for example by scanning the QR code in the Remote Access panel. On a phone, VelaTerm switches to a layout designed for small touch screens. The layout is chosen when the page loads: it is used on touch devices whose shorter screen side is below 768 pixels. Add `?view=mobile` or `?view=desktop` to the address to choose a layout yourself (a tablet can use `?view=mobile`), or choose "Switch to desktop" in the "More actions" (•••) menu; the choice is remembered.

The mobile layout has two levels:

- **Session list:** the project tree with a search box ("Search sessions / groups…") and status filters ("Working", "Pending", "Viewed"). Sessions are created on the desktop app or in a computer browser and appear here automatically.
- **Session page:** tap a session to open it, and use "‹ Back" to return to the list. Sessions in the conversation view show the full conversation. Claude Code, Codex, OpenCode, Pi and OMP sessions that run in the terminal view show their conversation with a message box that types into the terminal. Other sessions show the terminal with a key bar: Esc, Tab, Ctrl (which opens a row of Ctrl combinations such as ^C, ^D, ^R and ^W), the arrow keys, ^C, a keyboard button and a camera button for sending an image. Swipe to scroll, tap to show the keyboard, and long-press to select a word and "Copy" it.

The mobile layout does not show browser tabs, document tabs or split panes, has no keyboard shortcuts, and keeps its own view instead of following "Mirror layout across devices". Kiro sessions offer their history as a read-only view.

A VelaTerm app for iOS and Android is in development and has not been published yet.

## 7. Disconnecting, closing, and what survives

### 7.1 Three layers of state

Whether "everything is still there after a disconnect" depends on the layer:

| Layer | Where it lives | When it is lost |
|-------|----------------|-----------------|
| Window layout (open tabs, splits) | Your window | Practically never; reopening restores it |
| Project tree (projects, groups, sessions) | The database on the remote machine's disk | Never; it survives reboots |
| Running sessions (live terminals, agents in the middle of a task) | The memory of the remote VelaTerm server process | When that server process ends |

Only the third layer needs attention: as long as the remote server process runs, your running sessions keep running.

### 7.2 Closing a remote window

**SSH:** the remote server was started by this connection, so closing the window opens "Disconnect remote" with three choices. "Stop server" shuts the server down on the remote machine and ends its running sessions. "Keep running" only disconnects this window; the next connection to that machine reuses the server with every session intact. "Cancel" keeps the window open. Choose "Keep running" if agents should keep working while you are away.

**SSH attached to the remote desktop app:** when "Mirror the remote desktop app" attached the window to a running desktop app, closing the window simply disconnects; the desktop app keeps running.

**URL:** you were a guest of a VelaTerm that was already running. Closing the window only means you leave; the other side keeps running, and everything is there when you come back.

### 7.3 Connection drops

When the network drops, a red bar appears at the top of the remote window with "Connection lost, reconnecting…" and VelaTerm retries automatically. Running sessions are not affected; in SSH mode the remote server runs independently of the SSH connection.

In SSH windows, VelaTerm also rebuilds the SSH tunnel on its own ("SSH link lost, rebuilding the tunnel…"). If it gives up, the bar reads "SSH link is down — press Reconnect now to try again"; click "Reconnect now". If the remote server itself has stopped, reconnecting from the window cannot help: go back to the main window and connect again. The connection checks whether the remote server is still running, reuses it if it is (with all sessions), and starts a new one only if it is not.

A remote machine that sleeps, logs out or reboots ends an SSH-mode server and its running sessions. The project tree is stored on disk and is not affected.

### 7.4 Quick reference

| Question | SSH | URL |
|----------|-----|-----|
| Who owns the remote server | This connection (you started it and manage it), or the remote desktop app when mirroring attaches to it | A VelaTerm that was already running over there |
| Closing the window | "Disconnect remote" dialog (no dialog when attached to the remote desktop app) | You leave; the other side keeps running |
| Window layout | Saved per window and restored on reopen | Same |
| Project tree | A separate database, or the remote desktop app's database when mirroring | The other VelaTerm's database |
| Running sessions | Alive as long as the server runs | Alive as long as the other VelaTerm runs |

## 8. Security at a glance

- Passwords (SSH passwords and remote login passwords) are stored only in the system keychain, never in the database, and only when you choose "Remember password". The Remote Access password is stored only as a hash, so that the server can restart automatically.
- LAN browser access uses TLS with a self-signed certificate. Pairing links carry end-to-end encryption credentials, and the server rejects connections without a pairing link.
- SSH host keys and remote TLS certificates are trusted on first use: known and unchanged targets connect directly, new targets ask for one confirmation, and changed fingerprints show a red warning. Investigate before you confirm.
- Individual devices can be blocked at any time (§2.4), and the pairing link can be regenerated at will.
- Account sharing is limited to AI conversations, but the agents in those conversations act with your permissions on the sharing computer (§4.3).

If the title bar shows a "⚠ vX ≠ vY" badge, the interface and the VelaTerm service it is connected to report different versions, and some features may not work together. Update VelaTerm so that both run the same version.

## 9. FAQ

**The window is connected, but the sidebar stays empty or the red bar never goes away.** The remote server has probably stopped. Go back to the main window and connect again (§7.3).

**The server does not start on port 8799.** Another program is using the port. Choose another port in the panel and start again.

**The browser says the certificate is not trusted.** This is expected for a self-signed certificate. Check the fingerprint and continue (§2.2).

**An agent on a remote machine should keep working while I am away.** In SSH mode, choose "Keep running" when you close the window. The session continues on the remote machine, and you can reconnect later and pick up where it was.

**A terminal looks tiny on my phone.** It is a scaled copy that follows another device's size. Tap "fit this window" in the badge at the top-right of the terminal to size it for your phone.

**"View shared content" does not appear for my computer.** The button appears only when that computer is online, signed in to the same account and sharing something. Open VelaTerm on it and check "Sharing active" in its "Account" dialog.
