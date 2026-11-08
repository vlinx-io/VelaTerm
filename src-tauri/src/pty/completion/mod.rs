//! Native completion state belongs to the PTY, including clients sharing the same shell.
// The install half of this module is unused while Windows is excluded; remove together with the re-enable.
#![cfg_attr(windows, allow(dead_code))]

use std::path::{Path, PathBuf};
use std::time::Instant;

pub const MODE_KEY: &str = "terminal-completion-mode";
/// Tab only. Inline history hints stay opt-in until they are mature enough to be the default.
pub const DEFAULT_MODE: &str = "tab";
/// What Tab does while an inline history hint is on screen: `accept` takes the hint, `complete` always completes.
pub const TAB_BEHAVIOR_KEY: &str = "terminal-tab-behavior";
pub const DEFAULT_TAB_BEHAVIOR: &str = "complete";
/// Native completion stays disabled on Windows for now. Every request forks MSYS helper processes there, and
/// no Bash-family shell offers a startup-file hook, so the bootstrap command has to be typed into the terminal.
/// Re-enable once per-request process creation and typing latency are reduced on that platform.
pub const AVAILABLE: bool = !cfg!(windows);
pub const DEBOUNCE_MS: u64 = 25;
pub const QUERY_KEY: &[u8] = b"\x1b[24;5~";
pub const ACCEPT_KEY: &[u8] = b"\x1b[23;5~";
/// Same as `QUERY_KEY`, except that the shell accepts a visible hint instead of collecting candidates.
pub const QUERY_HINT_KEY: &[u8] = b"\x1b[21;5~";
const PREFIX: &[u8] = b"\x1b]6973;";
const LIMIT: usize = 64;

#[derive(Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub configured: bool,
    pub supported: bool,
    pub ready: bool,
    pub revision: u64,
    pub input_version: u64,
    pub items: Vec<Item>,
    pub query: String,
    pub pending: bool,
    /// The last Tab accepted the inline hint rather than collecting candidates; `undo` can still revert it.
    pub hint_accepted: bool,
    /// The candidates share text beyond the typed token, which the `extend` action inserts like native Tab.
    pub extendable: bool,
    /// Set only on a query's reply: the backend already inserted the single match or the shared text.
    pub applied: bool,
}

#[derive(Clone, serde::Serialize)]
pub struct Item {
    pub index: usize,
    pub label: String,
    pub description: String,
    pub matches: Vec<[usize; 2]>,
}

pub struct State {
    pub snapshot: Snapshot,
    nonce: String,
    pub selection_file: PathBuf,
    pub requested: Option<(u64, Instant)>,
    collecting: Vec<Item>,
    collecting_revision: Option<u64>,
    collecting_query: String,
    collecting_extendable: bool,
    collecting_hint_accepted: bool,
    /// The shell declared the `QUERY_HINT_KEY` binding. Sending it to any other shell would type garbage.
    pub hint_capable: bool,
    pub owner: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            snapshot: Snapshot::default(),
            nonce: String::new(),
            selection_file: PathBuf::new(),
            requested: None,
            collecting: Vec::new(),
            collecting_revision: None,
            collecting_query: String::new(),
            collecting_extendable: false,
            collecting_hint_accepted: false,
            hint_capable: false,
            owner: None,
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        if let Some(dir) = self.selection_file.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

impl State {
    pub fn input(&mut self, data: &str) {
        self.snapshot.input_version = self.snapshot.input_version.wrapping_add(1);
        self.snapshot.items.clear();
        self.snapshot.extendable = false;
        self.snapshot.hint_accepted = false;
        // The queued Enter must close the gate before its pre-execution marker reaches the reader.
        let paste = data.starts_with("\x1b[200~") && data.ends_with("\x1b[201~");
        if !paste
            && data
                .chars()
                .any(|c| matches!(c, '\r' | '\n' | '\u{3}' | '\u{4}' | '\u{1a}'))
        {
            self.snapshot.ready = false;
        }
    }

    pub fn message(&mut self, payload: &[u8]) {
        let Ok(text) = std::str::from_utf8(payload) else {
            return;
        };
        let mut fields = text.split(';');
        if self.nonce.is_empty() || fields.next() != Some(self.nonce.as_str()) {
            return;
        }
        match fields.next() {
            Some("U") => {
                self.snapshot = Snapshot::default();
                self.hint_capable = false;
                self.requested = None;
                self.collecting_revision = None;
                self.collecting.clear();
            }
            Some("P") => {
                self.snapshot.supported = true;
                self.snapshot.ready = true;
                self.snapshot.items.clear();
                self.snapshot.extendable = false;
                self.snapshot.hint_accepted = false;
                self.requested = None;
                self.snapshot.pending = false;
                self.collecting_revision = None;
            }
            Some("X") => {
                self.snapshot.ready = false;
                self.snapshot.items.clear();
                self.snapshot.extendable = false;
                self.snapshot.hint_accepted = false;
                self.requested = None;
                self.snapshot.pending = false;
                self.collecting_revision = None;
            }
            Some("A") => {
                self.collecting.clear();
                self.collecting_query.clear();
                self.collecting_extendable = false;
                self.collecting_hint_accepted = false;
                self.collecting_revision = fields.next().and_then(|s| s.parse().ok());
            }
            Some("H") => self.hint_capable = true,
            Some("Y") if self.collecting_revision.is_some() => self.collecting_hint_accepted = true,
            Some("E") if self.collecting_revision.is_some() => self.collecting_extendable = true,
            Some("T") if self.collecting_revision.is_some() => {
                self.collecting_query = fields.next().and_then(unhex).unwrap_or_default();
            }
            Some("C") if self.collecting_revision.is_some() && self.collecting.len() < LIMIT => {
                if let Some(label) = fields.next().and_then(unhex) {
                    let description = fields.next().and_then(unhex).unwrap_or_default();
                    let description = description.trim();
                    let description = if description == label {
                        ""
                    } else {
                        description
                            .strip_prefix(&format!("{label} "))
                            .map(|text| text.trim_start().strip_prefix("--").unwrap_or(text).trim())
                            .unwrap_or(description)
                    }
                    .to_string();
                    self.collecting.push(Item {
                        index: self.collecting.len(),
                        label,
                        description,
                        matches: Vec::new(),
                    });
                }
            }
            Some("Z") => {
                if let (Some(revision), Some((version, _))) =
                    (self.collecting_revision.take(), self.requested.take())
                {
                    self.snapshot.revision = revision;
                    if self.snapshot.ready && version == self.snapshot.input_version {
                        self.snapshot.query = self.collecting_query.clone();
                        self.snapshot.items = std::mem::take(&mut self.collecting);
                        self.snapshot.extendable = self.collecting_extendable && self.snapshot.items.len() > 1;
                        self.snapshot.hint_accepted = self.collecting_hint_accepted;
                        for item in &mut self.snapshot.items {
                            item.matches = match_ranges(&item.label, &self.snapshot.query);
                        }
                        rank_items(&mut self.snapshot.items, &self.snapshot.query);
                    }
                }
                self.snapshot.pending = false;
                self.collecting.clear();
            }
            _ => {}
        }
    }
}

// Keep native indices intact: accepting a candidate still addresses the shell's original list.
fn rank_items(items: &mut [Item], query: &str) {
    if query.is_empty() { return; }
    items.sort_by_cached_key(|item| {
        let token = if !item.label.trim_end_matches(['/', '\\']).contains(['/', '\\']) {
            query.rsplit(['/', '\\']).next().unwrap_or(query)
        } else { query };
        let folded_label = item.label.to_lowercase();
        let folded_token = token.to_lowercase();
        let category = if item.label == token { 0 }
            else if folded_label == folded_token { 1 }
            else if item.label.starts_with(token) { 2 }
            else if folded_label.starts_with(&folded_token) { 3 }
            else if folded_label.contains(&folded_token) { 4 }
            else { 5 };
        (category, item.label.chars().count(), folded_label)
    });
}

// Highlight the shell's current completion token, using UTF-16 offsets for DOM text.
fn match_ranges(label: &str, query: &str) -> Vec<[usize; 2]> {
    let query = if !label.trim_end_matches(['/', '\\']).contains(['/', '\\']) {
        query.rsplit(['/', '\\']).next().unwrap_or(query)
    } else {
        query
    };
    let mut wanted = query.chars().peekable();
    let mut offset = 0;
    let mut ranges: Vec<[usize; 2]> = Vec::new();
    for ch in label.chars() {
        let end = offset + ch.len_utf16();
        if wanted
            .peek()
            .is_some_and(|next| ch.to_lowercase().eq(next.to_lowercase()))
        {
            wanted.next();
            if let Some(last) = ranges.last_mut().filter(|last| last[1] == offset) {
                last[1] = end;
            } else {
                ranges.push([offset, end]);
            }
        }
        offset = end;
    }
    if wanted.peek().is_some() {
        Vec::new()
    } else {
        ranges
    }
}

fn unhex(s: &str) -> Option<String> {
    if s.len() > 32768 || s.len() % 2 != 0 {
        return None;
    }
    let bytes: Option<Vec<u8>> = s
        .as_bytes()
        .chunks_exact(2)
        .map(|c| Some((c[0] as char).to_digit(16)? as u8 * 16 + (c[1] as char).to_digit(16)? as u8))
        .collect();
    let value = String::from_utf8(bytes?).ok()?;
    Some(
        value
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect(),
    )
}

/// Remove private metadata before replay, recording and rendering. Other bytes remain unchanged.
#[derive(Default)]
pub struct Filter {
    held: Vec<u8>,
    overflow: bool,
}

impl Filter {
    pub fn feed(&mut self, data: &[u8], state: &mut State) -> Vec<u8> {
        // A shell without integration never emits the private protocol, so pass its output straight through
        // instead of scanning it byte by byte on every read.
        if state.nonce.is_empty() {
            return data.to_vec();
        }
        let mut out = Vec::with_capacity(data.len());
        for &byte in data {
            if self.held.is_empty() && byte != 0x1b {
                out.push(byte);
                continue;
            }
            self.held.push(byte);
            if self.held.len() <= PREFIX.len() {
                if !PREFIX.starts_with(&self.held) {
                    out.append(&mut self.held);
                }
                continue;
            }
            if byte == 7 || self.held.ends_with(b"\x1b\\") {
                if !self.overflow {
                    let end = self.held.len() - if byte == 7 { 1 } else { 2 };
                    state.message(&self.held[PREFIX.len()..end]);
                }
                self.held.clear();
                self.overflow = false;
            } else if self.held.len() > 65536 {
                self.overflow = true;
                self.held.truncate(PREFIX.len() + 1);
            }
        }
        out
    }
}

/// Install only in application data; shell profiles are never edited.
pub fn configure_zsh_startup(
    state: &State,
    cmd: &mut portable_pty::CommandBuilder,
    binaries: &[String],
) -> Result<(), String> {
    let dir = state
        .selection_file
        .parent()
        .ok_or("Missing completion directory")?;
    let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let initial = match cmd.get_env("ZDOTDIR") {
        Some(value) => format!("export ZDOTDIR={}\n", quote(&value.to_string_lossy())),
        None => "unset ZDOTDIR\n".to_string(),
    };
    let restore = "if (( _vlxc_startup_zdotdir_set )); then export ZDOTDIR=$_vlxc_startup_zdotdir; else unset ZDOTDIR; fi\n";
    let redirect = format!(
        "typeset -g _vlxc_startup_zdotdir_set=${{+ZDOTDIR}} _vlxc_startup_zdotdir=${{ZDOTDIR-}}\nexport ZDOTDIR={}\n",
        quote(&dir.to_string_lossy())
    );
    let finish = format!(
        "unset _vlxc_startup_zdotdir_set _vlxc_startup_zdotdir\n{}builtin source {}\n",
        startup_path(binaries),
        quote(&dir.join("integration.zsh").to_string_lossy())
    );
    for file in [".zshenv", ".zprofile", ".zshrc", ".zlogin"] {
        let mut body = String::from("# Load the user's startup file with its original ZDOTDIR.\n");
        body.push_str(if file == ".zshenv" { &initial } else { restore });
        if file == ".zshrc" {
            // /etc/zshrc on macOS runs before this file and sets HISTFILE=${ZDOTDIR:-$HOME}/.zsh_history while
            // ZDOTDIR still points here; repeat that assignment with the user's own ZDOTDIR.
            body.push_str(&format!(
                "[[ ${{HISTFILE-}} != {} ]] || HISTFILE=${{ZDOTDIR:-$HOME}}/.zsh_history\n",
                quote(&dir.join(".zsh_history").to_string_lossy())
            ));
        }
        body.push_str(&format!("[[ ! -r ${{ZDOTDIR:-$HOME}}/{file} ]] || builtin source \"${{ZDOTDIR:-$HOME}}/{file}\"\n"));
        match file {
            ".zlogin" => body.push_str(&finish),
            ".zshrc" => body.push_str(&format!(
                "if [[ -o login ]]; then\n{redirect}else\n{finish}fi\n"
            )),
            _ => body.push_str(&redirect),
        }
        std::fs::write(dir.join(file), body).map_err(|e| e.to_string())?;
    }
    cmd.env("ZDOTDIR", dir);
    Ok(())
}

/// Install only in application data; shell profiles are never edited.
/// Bash has no `ZDOTDIR` equivalent. `--rcfile` is its only startup hook, and it replaces `~/.bashrc`, which
/// a login shell never reads, so the caller starts Bash without `-l` and this file replays the login sequence
/// itself. Two differences from a real login shell remain: `shopt -q login_shell` reports false, and
/// `~/.bash_logout` does not run on exit.
/// Append verified binary directories after profiles load, preserving PATH precedence and avoiding a
/// second .bashrc evaluation when the user's login profile already sources it.
pub fn configure_bash_startup(state: &State, binaries: &[String]) -> Result<PathBuf, String> {
    let dir = state
        .selection_file
        .parent()
        .ok_or("Missing completion directory")?;
    let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let mut body = String::from(
        "# Load the user's login startup files in Bash's own order, then the integration.\n\
         [[ ! -r /etc/profile ]] || builtin source /etc/profile\n\
         for _vlxc_startup_profile in \"$HOME/.bash_profile\" \"$HOME/.bash_login\" \"$HOME/.profile\"; do\n\
         \t[[ -r $_vlxc_startup_profile ]] || continue\n\
         \tbuiltin source \"$_vlxc_startup_profile\"\n\
         \tbreak\n\
         done\n\
         unset _vlxc_startup_profile\n"
    );
    body.push_str(&startup_path(binaries));
    body.push_str(&format!("builtin source {}\n", quote(&dir.join("integration.bash").to_string_lossy())));
    let path = dir.join("bashrc");
    std::fs::write(&path, body).map_err(|e| e.to_string())?;
    Ok(path)
}

fn startup_path(binaries: &[String]) -> String {
    crate::agent::executable::path_startup_script(&crate::agent::executable::binary_dirs(binaries))
}

/// Share the mode with running shells, which read it at each prompt to decide whether to draw history hints.
pub fn write_mode(data_dir: &Path, mode: &str) -> Result<(), String> {
    let dir = data_dir.join("terminal-completion");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("mode"), mode).map_err(|e| e.to_string())
}

/// Install only in application data; shell profiles are never edited.
pub fn install(data_dir: &Path, shell: &str) -> Result<Option<(State, String)>, String> {
    let name = Path::new(shell)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let (extension, body, powershell) = match name.trim_end_matches(".exe") {
        "zsh" => ("zsh", include_str!("integration.zsh"), false),
        "bash" => ("bash", include_str!("integration.bash"), false),
        "fish" => ("fish", include_str!("integration.fish"), false),
        "pwsh" | "powershell" => ("ps1", include_str!("integration.ps1"), true),
        _ => return Ok(None),
    };
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let dir = data_dir.join("terminal-completion").join(&nonce);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    let selection_file = dir.join("selection");
    std::fs::write(&selection_file, "0 0\n").map_err(|e| e.to_string())?;
    let script = dir.join(format!("integration.{extension}"));
    let path = selection_file.to_string_lossy().replace('\\', "/");
    let quoted_path = if powershell {
        path.replace('\'', "''")
    } else {
        path.replace('\'', "'\\''")
    };
    let rendered = body
        .replace("@NONCE@", &nonce)
        .replace("@SELECTION@", &quoted_path);
    // Windows PowerShell 5.1 otherwise interprets non-ASCII user paths using the system code page.
    let rendered = if powershell {
        format!("\u{feff}{rendered}")
    } else {
        rendered
    };
    std::fs::write(&script, rendered).map_err(|e| e.to_string())?;
    let path = script.to_string_lossy().replace('\\', "/");
    let launch = if powershell {
        format!(". '{}'", path.replace('\'', "''"))
    } else {
        format!("source '{}'", path.replace('\'', "'\\''"))
    };
    let mut state = State::default();
    state.nonce = nonce;
    state.selection_file = selection_file;
    state.snapshot.configured = true;
    Ok(Some((state, launch)))
}

#[cfg(windows)]
pub fn install_wsl(data_dir: &Path, distro: &str) -> Result<Option<(State, String)>, String> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    fn run(distro: &str, arguments: &[&str]) -> Result<String, String> {
        let mut child = Command::new("wsl.exe")
            .args(["--distribution", distro, "--exec"])
            .args(arguments)
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err("WSL completion initialization timed out".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err("WSL completion initialization failed".into());
        }
        String::from_utf8(output.stdout)
            .map(|s| s.trim().to_string())
            .map_err(|e| e.to_string())
    }
    let shell = run(distro, &["sh", "-c", "entry=$(getent passwd \"$(id -u)\" 2>/dev/null); if [ -n \"$entry\" ]; then printf '%s' \"${entry##*:}\"; else printf '%s' \"${SHELL:-/bin/bash}\"; fi"])?;
    if !matches!(shell.rsplit('/').next(), Some("bash" | "zsh" | "fish")) {
        return Ok(None);
    }
    let Some((state, _)) = install(data_dir, &shell)? else {
        return Ok(None);
    };
    let dir = state
        .selection_file
        .parent()
        .ok_or("Missing completion directory")?;
    let linux_dir = run(distro, &["wslpath", "-u", &dir.to_string_lossy()])?;
    let entry = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .find(|e| e.file_name().to_string_lossy().starts_with("integration."))
        .ok_or("Missing completion integration")?;
    let script = entry.path();
    let body = std::fs::read_to_string(&script).map_err(|e| e.to_string())?;
    let host_path = state
        .selection_file
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "'\\''");
    std::fs::write(
        &script,
        body.replace(
            &host_path,
            &format!("{linux_dir}/selection").replace('\'', "'\\''"),
        ),
    )
    .map_err(|e| e.to_string())?;
    let launch = format!(
        "source '{}/{}'",
        linux_dir.replace('\'', "'\\''"),
        entry.file_name().to_string_lossy()
    );
    Ok(Some((state, launch)))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
mod install_tests;
