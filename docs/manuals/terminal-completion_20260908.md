# Command suggestions

Created: 2026-09-08

Updated: 2026-11-08 09:33

Command suggestions help in a plain terminal session when you press Tab: the shell's own completion definitions provide the candidates, and a list opens when there is more than one choice. Optionally, Zsh can also show the latest matching command from your history dimmed after the cursor while you type. The command stays in the terminal's normal input line, and accepting a suggestion inserts text without running it. This feature does not use an AI model.

Suggestions are available on macOS and Linux. On Windows the feature is currently turned off (see [Windows](#windows)).

## Settings and controls

Settings ▸ Terminal ▸ "Command suggestions" offers three modes:

- **Tab only** (default): no hints while you type; Tab works as described below.
- **Inline + Tab**: history hints while you type, and the Tab behavior described below.
- **Off**: no hints and no list; Tab keeps the shell's native behavior.

Typing never opens the list in any mode. A change takes effect in open terminals immediately; Zsh picks up a change to history hints at its next prompt. The choice is shared by every client connected to the same VelaTerm backend.

### History hints

When the cursor is at the end of the line in Zsh, VelaTerm looks up the most recent history entry that starts with what you typed and shows the rest of it in dim text. Press Right arrow or End, or Ctrl+E with Emacs key bindings, to accept it (or Tab, if you choose that below); any other key keeps typing normally. While a hint is showing, the terminal's header names the key to press. Elsewhere on the line, those keys keep the behavior they had before. Hints skip multi-line history entries and pause while you type faster than the shell redraws, during incremental history search and in vi command mode.

If zsh-autosuggestions is loaded, VelaTerm leaves the hints to it. Fish has its own inline suggestions, and PSReadLine can show history predictions through its `PredictionSource` option, so VelaTerm adds none to those shells. Bash has no way to draw a hint after the cursor and shows none.

### Tab key behavior

With **Inline + Tab**, Settings ▸ Terminal ▸ "Tab key" decides what Tab does while a history hint is showing:

- **Complete command** (default): Tab always completes the command, as described below. Right arrow or End accepts the hint.
- **Accept suggestion**: Tab accepts the hint. Press Tab again right away to undo that and list the matches for what you typed.

Without a hint on screen, Tab behaves the same with either choice.

### Tab

Tab first asks the shell for candidates and then behaves like the shell's own completion:

- With a single candidate, Tab inserts it directly.
- When all candidates share text beyond what you typed, Tab inserts that shared text, for example `cd D` becomes `cd Do` when `Documents/` and `Downloads/` are the only matches.
- When more than one choice remains, Tab opens the list. Press Tab again to insert the highlighted candidate.

Ctrl+Space always opens the list, even for a single candidate.

Keys and mouse in the list:

- Up and Down select a candidate; Page Up and Page Down move through longer lists.
- Tab, or a mouse click, inserts the selected candidate.
- Enter always goes to the shell and runs the current command as typed, without applying a suggestion.
- Escape closes the list.
- When the selection cannot move any further, Up and Down go back to the shell. Pressing Up on the first candidate therefore recalls the previous command from history.

Candidates are ordered as exact matches, prefix matches, substring matches and then other matches from the shell. Within exact and prefix matches, case-sensitive matches come first; within each group, shorter candidates come first, and ties are ordered alphabetically.

The list highlights the part of each candidate that matches what you typed and shows the shell's description when one is available; hover over a candidate to read a long description. While the list is open, typing more letters refreshes it and keeps its position and selection; typing a space closes it. Accepting a suggestion closes the list, including when the candidate is a directory ending in `/`. Press Tab again to see the next directory level.

If you change the input while an insertion is pending, VelaTerm waits for a fresh result from the shell and inserts the candidate only if it is still offered.

## Shell support

| Shell | Suggestions |
| --- | --- |
| Zsh | Native completion widgets and `compadd`; your existing completion definitions are used. |
| Bash 4 or later | Registered completion functions and specifications; command and file names when nothing is registered. |
| Fish | Native `complete -C` candidates and descriptions. |
| PowerShell (pwsh) with PSReadLine | Native `CommandCompletion.CompleteInput`. |
| Bash 3.2 and other shells | Native Tab behavior only; no suggestion list. |

The Bash that ships with macOS is version 3.2. To use suggestions with Bash on macOS, set a newer Bash (for example one installed with Homebrew) as your login shell.

Suggestions are available only in plain terminal sessions. Agent sessions are not changed.

## How the integration is loaded

VelaTerm loads its shell integration when a terminal starts. Your shell startup files are never modified, and a custom startup command still runs after the integration is loaded. Terminals that were already running before you installed a version with this feature need to be restarted.

- **Zsh** loads the integration through temporary startup files after your own login files, and restores `ZDOTDIR` afterwards. Nothing is typed into the terminal.
- **Bash** starts with a temporary `--rcfile` that first reads your login files in Bash's usual order (`/etc/profile`, then the first of `~/.bash_profile`, `~/.bash_login` and `~/.profile`) and then loads the integration. Nothing is typed into the terminal. Two differences from a normal login shell remain: `shopt -q login_shell` reports false, and `~/.bash_logout` does not run when the shell exits.
- **Fish** and **PowerShell** have no comparable startup hook, so VelaTerm sends one command that loads the integration when the shell starts; that line appears at the top of the new terminal.

PowerShell's execution policy still applies to the integration script; VelaTerm does not bypass it.

Suggestions pause while a command is running, while a full-screen program is open, during input method (IME) composition and during history navigation (Ctrl+R, Ctrl+S and the arrow keys). The arrow keys close an open list and go to the shell, so repeated Up presses keep moving through earlier commands. When no integration is active or no candidates were found, Tab keeps its native behavior.

Starting SSH or another nested shell inside the terminal does not install the integration in that environment.

## Windows

On Windows no integration is loaded, whatever the setting says, and the "Command suggestions" setting is hidden. cmd, Windows PowerShell, pwsh, Git Bash and WSL terminals keep their native Tab behavior. The feature stays off because loading it there would require typing a command into every new terminal and each request would start helper processes, which slows down typing.

## Limits

- Tab candidates depend on the completion definitions installed for your shell. VelaTerm does not provide its own command catalog. History hints use only the Zsh history of the session.
- Some complex Bash completion scripts rely on Readline internals that the integration cannot reproduce exactly.
- Completion functions can run external programs and may be slow. The list stops waiting after two seconds, but a shell function that is still running is not killed; press Ctrl+C to interrupt it as usual.
- The integration reserves Ctrl+F12 for collecting candidates and Ctrl+F11 for inserting a selection. A shell configuration that rebinds these keys after the terminal starts can disable suggestions. With Bash 4.0 to 4.3, the integration also wraps Enter and Ctrl+J. In Zsh, Right arrow, End and Ctrl+E are bound to accept hints and fall back to the widget each key had when the integration loaded; a plugin that rebinds them later takes them over.
- Descriptions come from the shell. Many Bash completion definitions provide none, so their candidates show no description.

## Testing

The feature has been tested on macOS with Zsh and Bash 5.2, and on Linux with Bash, Zsh, Fish and PowerShell 7.4. The PTY checks in the repository can be repeated with:

```sh
python3 scripts/test-terminal-completion.py --zsh /bin/zsh --bash /path/to/bash
python3 scripts/test-terminal-completion.py --zsh /bin/zsh --zsh-user-config
python3 scripts/test-terminal-completion.py --fish /usr/bin/fish --pwsh /path/to/pwsh
```
