# Command suggestions

Created: 2026-09-08

Updated: 2026-09-25 10:21

Command suggestions show a list of candidates taken from your shell's own completion definitions while you type in a plain terminal session. The command stays in the terminal's normal input line, and choosing a suggestion inserts text without running it. This feature does not use an AI model.

Suggestions are available on macOS and Linux. On Windows the feature is currently turned off (see [Windows](#windows)).

## Settings and controls

Settings ▸ Terminal ▸ "Command suggestions" offers three modes:

- **Automatic** (default): VelaTerm collects your input for about 25 ms and then asks the shell for suggestions; typing more does not restart that wait. If you keep typing while a request is running, only your latest input is used for the next request, and outdated results are discarded. An exact match stays in the list.
- **On Tab**: suggestions appear only when you press Tab.
- **Off**: no suggestion list is shown, and Tab keeps the shell's native behavior.

A change takes effect in open terminals immediately, and the choice is shared by every client connected to the same VelaTerm backend.

Keys and mouse:

- Up and Down select a candidate; Page Up and Page Down move through longer lists.
- Tab, or a mouse click, inserts the selected candidate.
- Enter always goes to the shell and runs the current command as typed, without applying a suggestion.
- Escape closes the list.
- Ctrl+Space requests suggestions manually.
- When the selection cannot move any further, Up and Down go back to the shell. Pressing Up on the first candidate therefore recalls the previous command from history.

Candidates are ordered as exact matches, prefix matches, substring matches and then other matches from the shell. Within exact and prefix matches, case-sensitive matches come first; within each group, shorter candidates come first, and ties are ordered alphabetically.

The list highlights the part of each candidate that matches what you typed and shows the shell's description when one is available; hover over a candidate to read a long description. The list keeps its position and selection while results refresh. Accepting a suggestion closes the list, including when the candidate is a directory ending in `/`. Press Tab again to see the next directory level. In Automatic mode, further typing opens the list again.

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

Suggestions pause while a command is running, while a full-screen program is open, during input method (IME) composition and during history navigation (Ctrl+R, Ctrl+S and the arrow keys). The arrow keys never start an automatic request, so repeated Up presses keep moving through earlier commands. When no integration is active or no candidates were found, Tab keeps its native behavior.

Starting SSH or another nested shell inside the terminal does not install the integration in that environment.

## Windows

On Windows no integration is loaded, whatever the setting says, and the "Command suggestions" setting is hidden. cmd, Windows PowerShell, pwsh, Git Bash and WSL terminals keep their native Tab behavior. The feature stays off because loading it there would require typing a command into every new terminal and each request would start helper processes, which slows down typing.

## Limits

- Candidates depend on the completion definitions installed for your shell. VelaTerm does not provide its own command catalog or history-based predictions.
- Some complex Bash completion scripts rely on Readline internals that the integration cannot reproduce exactly.
- Completion functions can run external programs and may be slow. The list stops waiting after two seconds, but a shell function that is still running is not killed; press Ctrl+C to interrupt it as usual.
- The integration reserves Ctrl+F12 for collecting candidates and Ctrl+F11 for inserting a selection. A shell configuration that rebinds these keys after the terminal starts can disable suggestions. With Bash 4.0 to 4.3, the integration also wraps Enter and Ctrl+J.
- Descriptions come from the shell. Many Bash completion definitions provide none, so their candidates show no description.

## Testing

The feature has been tested on macOS with Zsh and Bash 5.2, and on Linux with Bash, Zsh, Fish and PowerShell 7.4. The PTY checks in the repository can be repeated with:

```sh
python3 scripts/test-terminal-completion.py --zsh /bin/zsh --bash /path/to/bash
python3 scripts/test-terminal-completion.py --zsh /bin/zsh --zsh-user-config
python3 scripts/test-terminal-completion.py --fish /usr/bin/fish --pwsh /path/to/pwsh
```
