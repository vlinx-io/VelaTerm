//! Shared executable selection for terminal, chat, and background agent operations.

use crate::{host::AppCtx, models::SessionKind};
use std::path::Path;

pub fn command_name(kind: SessionKind) -> &'static str {
    match kind {
        SessionKind::Zcode => "zcode",
        SessionKind::Cursor => "cursor-agent",
        SessionKind::Antigravity => "agy",
        SessionKind::Kiro => "kiro-cli",
        SessionKind::Zoo => "roo",
        _ => kind.as_str(),
    }
}

fn configured_path(raw: Option<&str>) -> Option<String> {
    let path = raw?.trim();
    if path.is_empty() {
        return None;
    }
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        if let Some(home) = crate::host::home_dir() {
            return Some(home.join(rest).to_string_lossy().into_owned());
        }
    }
    Some(path.to_string())
}

/// Configured path precedence: the session override first, then the agent's global default.
fn configured_session(session: Option<&str>, global: Option<&str>) -> Option<String> {
    configured_path(session).or_else(|| configured_path(global))
}

/// Test-only helper retaining the original precedence and discovery shape.
#[cfg(test)]
fn select(
    session: Option<&str>,
    global: Option<&str>,
    discover: impl FnOnce() -> Option<String>,
) -> Option<String> {
    configured_session(session, global).or_else(discover)
}

/// How an agent session's executable was resolved.
#[derive(Debug, PartialEq, Eq)]
pub enum LaunchBinary {
    /// Launch this absolute path.
    Path(String),
    /// An installed command wrapper exists but the program it forwards to is gone. Resolving the
    /// command by name would find that same wrapper and print only cmd.exe's path error, so the
    /// launch reports the agent as missing and shows the installation guidance instead.
    Broken,
    /// No absolute path was found. The interactive shell resolves the command name, which keeps
    /// installations visible only to a login profile working.
    Unresolved,
}

/// Explicit paths remain authoritative even if missing; never silently launch a different installation.
pub fn resolve(app: &AppCtx, kind: SessionKind, session_path: Option<&str>) -> Option<String> {
    match resolve_binary(app, kind, session_path) {
        Ok(LaunchBinary::Path(path)) => Some(path),
        _ => None,
    }
}

pub fn for_session(app: &AppCtx, session: &crate::models::Session) -> String {
    resolve(app, session.kind, session.agent_path.as_deref())
        .unwrap_or_else(|| command_name(session.kind).to_string())
}

/// Session-scoped resolution that keeps a proven-broken installation distinguishable from a command
/// name the interactive shell should resolve itself.
pub fn resolve_launch_binary(
    app: &AppCtx,
    id: &str,
    kind: SessionKind,
) -> Result<LaunchBinary, String> {
    if matches!(kind, SessionKind::Terminal | SessionKind::Browser) {
        return Ok(LaunchBinary::Unresolved);
    }
    let path = {
        let conn = app.db().conn.lock().unwrap();
        crate::db::repo::get_agent_path(&conn, id)?
    };
    resolve_binary(app, kind, path.as_deref())
}

fn resolve_binary(
    app: &AppCtx,
    kind: SessionKind,
    session_path: Option<&str>,
) -> Result<LaunchBinary, String> {
    if matches!(kind, SessionKind::Terminal | SessionKind::Browser) {
        return Ok(LaunchBinary::Unresolved);
    }
    let settings = {
        let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
        crate::db::repo::get_app_settings(&conn)?.remove("vlx-settings")
    };
    let settings: serde_json::Value = settings
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let global = settings
        .get("agentDefaults")
        .and_then(|v| v.get(kind.as_str()))
        .and_then(|v| v.get("path"))
        .and_then(|v| v.as_str());
    match configured_session(session_path, global) {
        Some(path) => Ok(configured_binary(path)),
        None => Ok(discover(kind)),
    }
}

/// A configured path stays authoritative even when missing, so the launch guard can point at
/// Settings. A wrapper whose payload is gone is not a usable installation at all, so it takes the
/// broken-install path instead.
fn configured_binary(path: String) -> LaunchBinary {
    if shim_payload_missing(Path::new(&path)) {
        return LaunchBinary::Broken;
    }
    LaunchBinary::Path(path)
}

/// Discovery order: known install locations, then `PATH`, then an interactive login shell (Unix
/// only), and last a dead command wrapper. Checking the wrapper last keeps a working copy found
/// anywhere else in the lead.
fn discover(kind: SessionKind) -> LaunchBinary {
    if kind == SessionKind::Zcode {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let bin = parent.join("zcode").join(if cfg!(windows) { "zcode.exe" } else { "zcode" });
                if bin.is_file() { return LaunchBinary::Path(bin.to_string_lossy().into_owned()); }
            }
        }
    }
    let name = command_name(kind);
    if let Some(path) = super::install::locate_installed_bin(kind.as_str()) {
        return LaunchBinary::Path(path);
    }
    if let Some(path) = find_on_path(name) {
        return LaunchBinary::Path(path);
    }
    if let Some(path) = find_in_shell(name) {
        return LaunchBinary::Path(path);
    }
    if kind == SessionKind::Opencode {
        if let Some(path)=find_on_path("opencode2").or_else(||find_in_shell("opencode2")) {
            return LaunchBinary::Path(path);
        }
    }
    if super::install::dangling_npm_install(kind.as_str()) {
        return LaunchBinary::Broken;
    }
    LaunchBinary::Unresolved
}

/// Include the executable's directory so npm wrappers can find their adjacent Node runtime.
pub fn prepare_command(command: &mut std::process::Command, bin: &str) {
    let Some(parent) = Path::new(bin).parent().filter(|p| p.is_absolute()) else {
        return;
    };
    let existing = command
        .get_envs()
        .find(|(key, _)| *key == "PATH")
        .and_then(|(_, value)| value.map(|v| v.to_os_string()))
        .or_else(|| crate::appimage::clean_var("PATH").map(Into::into))
        .unwrap_or_default();
    let mut paths: Vec<_> = std::env::split_paths(&existing).collect();
    if !paths.iter().any(|p| p == parent) {
        paths.insert(0, parent.to_path_buf());
    }
    if let Ok(path) = std::env::join_paths(paths) {
        command.env("PATH", path);
    }
}

/// Match terminal startup when installation paths are set only in interactive shell profiles.
/// The command name comes from `command_name`; no session path is interpolated into shell code.
fn find_in_shell(bin: &str) -> Option<String> {
    #[cfg(windows)]
    {
        let _ = bin;
        None
    }
    #[cfg(not(windows))]
    {
        let shell = crate::appimage::clean_var("SHELL")
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "/bin/sh".into());
        for mode in shell_probe_modes(&shell) {
            let Some(output) = shell_probe_output(&shell, mode, &format!("command -v {bin}")) else {
                continue;
            };
            if let Some(path) = String::from_utf8_lossy(&output)
                .lines().rev().map(str::trim)
                .find(|p| Path::new(p).is_absolute() && is_executable_file(Path::new(p)))
            {
                return Some(path.to_string());
            }
        }
        None
    }
}

/// Bash's interactive login shell can omit .bashrc. Probe its ordinary interactive environment too;
/// Zsh and Fish already load their interactive configuration in a login shell.
#[cfg(unix)]
pub(crate) fn shell_probe_modes(shell: &str) -> &'static [&'static str] {
    if Path::new(shell).file_stem().is_some_and(|name| name == "bash") {
        &["-lic", "-ic"]
    } else {
        &["-lic"]
    }
}

/// A profile may prompt or leave stdout open in a background process. Both execution and output are bounded.
#[cfg(unix)]
pub(crate) fn shell_probe_output(shell: &str, mode: &str, script: &str) -> Option<Vec<u8>> {
    shell_probe_output_limited(shell, mode, script, 65536)
}

#[cfg(unix)]
pub(crate) fn shell_probe_output_limited(shell: &str, mode: &str, script: &str, limit: usize) -> Option<Vec<u8>> {
    shell_probe_output_in(shell, mode, script, limit, None)
}

#[cfg(unix)]
pub(crate) fn shell_probe_output_in(shell: &str, mode: &str, script: &str, limit: usize, cwd: Option<&Path>) -> Option<Vec<u8>> {
    use std::io::Read;
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let mut command = crate::host::command(shell);
    crate::login_env::probe_environment(&mut command);
    if let Some(cwd) = cwd { command.current_dir(cwd); }
    let mut child = command.args([mode, script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let fd = stdout.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    let mut output = Vec::new();
    let drain = |stdout: &mut std::process::ChildStdout, output: &mut Vec<u8>| {
        let mut buffer = [0u8; 4096];
        // Keep the final bounded capture while draining banners too, so a full pipe cannot stall the shell.
        for _ in 0..32 {
            let Ok(count) = stdout.read(&mut buffer) else { break; };
            if count == 0 { break; }
            output.extend_from_slice(&buffer[..count]);
            if output.len() > limit { output.drain(..output.len() - limit); }
        }
    };
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        drain(&mut stdout, &mut output);
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20))
            }
            _ => {
                // The probe owns its process group. A stalled npm or background profile job must not survive it.
                unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL); }
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    if !child.wait().ok()?.success() {
        return None;
    }
    // Read buffered output without waiting for a background job that still owns the pipe.
    drain(&mut stdout, &mut output);
    Some(output)
}

/// Discover all supported installations once per terminal launch, sharing npm prefix probes across agents.
/// Include valid user defaults and presets without allowing them to replace an explicit session launch path.
pub(crate) fn terminal_environment(app: &AppCtx, shell: &str, cwd: Option<&Path>, agent: bool) -> (Vec<String>, Option<crate::login_env::SessionEnvironment>) {
    let mut binaries = Vec::new();
    if let Ok(conn) = app.db().conn.lock() {
        if let Ok(settings) = crate::db::repo::get_app_settings(&conn) {
            let settings: serde_json::Value = settings.get("vlx-settings")
                .and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();
            if let Some(defaults) = settings.get("agentDefaults").and_then(|v| v.as_object()) {
                for option in super::launch_options::catalog() {
                    if let Some(path) = configured_path(defaults.get(option.id.as_str())
                        .and_then(|v| v.get("path")).and_then(|v| v.as_str()))
                    {
                        binaries.push(path);
                    }
                }
            }
        }
        if let Ok(presets) = crate::db::repo::list_agent_presets(&conn) {
            binaries.extend(presets.into_iter().filter_map(|preset| configured_path(preset.exec_path.as_deref())));
        }
    }
    let (installed, path) = super::install::locate_installed_bins(shell, cwd, agent);
    binaries.extend(installed);
    (binaries, path)
}

/// Only real executable files contribute directories; stale settings and partial installs add nothing.
pub(crate) fn binary_dirs(binaries: &[String]) -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    for bin in binaries {
        let path = Path::new(bin);
        if !path.is_absolute() || !is_executable_file(path) { continue; }
        if let Some(parent) = path.parent() {
            if !dirs.iter().any(|dir| dir == parent) { dirs.push(parent.to_path_buf()); }
        }
    }
    dirs
}

/// Preserve the shell's inherited command precedence while exposing newly installed agents to every shell.
pub(crate) fn prepare_pty(command: &mut portable_pty::CommandBuilder, dirs: &[std::path::PathBuf]) {
    let existing = command.get_env("PATH").map(|path| path.to_os_string())
        .or_else(|| crate::appimage::clean_var("PATH").map(Into::into)).unwrap_or_default();
    let mut paths: Vec<_> = std::env::split_paths(&existing).collect();
    for dir in dirs {
        if !paths.contains(dir) { paths.push(dir.clone()); }
    }
    if let Ok(path) = std::env::join_paths(paths) { command.env("PATH", path); }
}

/// Reapply directories after a POSIX shell's profiles have rebuilt PATH, keeping its existing order.
/// Only quoted configuration and backend-validated directory literals enter this startup script.
pub(crate) fn path_startup_script(dirs: &[std::path::PathBuf]) -> String {
    if dirs.is_empty() { return String::new(); }
    let dirs = dirs.iter().map(|dir| format!("'{}'", posix_dir(dir).replace('\'', "'\\''")))
        .collect::<Vec<_>>().join(" ");
    format!(
        "for _vlx_path_bin in {dirs}; do case \":${{PATH-}}:\" in \
         *\":$_vlx_path_bin:\"*) ;; *) export PATH=\"${{PATH:+$PATH:}}$_vlx_path_bin\" ;; \
         esac; done; unset _vlx_path_bin;\n"
    )
}

/// Environment variable through which a typed launch receives the directories `PATH_APPEND_COMMAND` reapplies.
pub(crate) const PATH_APPEND_VAR: &str = "VLX_PATH_APPEND";

/// Typed launch prefix that appends each `PATH_APPEND_VAR` entry missing from PATH. The directories travel in
/// the environment rather than on the command line because macOS discards terminal input beyond 1024 bytes on
/// one line before the shell's line editor starts, and a long PATH would cut the launch command short.
/// Splitting by parameter expansion behaves the same in sh, Bash and zsh, which does not word-split `$VAR`.
pub(crate) const PATH_APPEND_COMMAND: &str = "_vlx_path_rest=${VLX_PATH_APPEND-}; \
    while [ -n \"$_vlx_path_rest\" ]; do _vlx_path_bin=${_vlx_path_rest%%:*}; \
    case \"$_vlx_path_rest\" in *:*) _vlx_path_rest=${_vlx_path_rest#*:} ;; *) _vlx_path_rest= ;; esac; \
    case \":${PATH-}:\" in *\":$_vlx_path_bin:\"*) ;; *) export PATH=\"${PATH:+$PATH:}$_vlx_path_bin\" ;; esac; \
    done; unset _vlx_path_bin _vlx_path_rest;";

/// Value for `PATH_APPEND_VAR`: POSIX directory paths joined by `:`. A directory containing `:` cannot be a
/// PATH entry, so it is skipped.
pub(crate) fn path_append_value(dirs: &[std::path::PathBuf]) -> String {
    dirs.iter().map(|dir| posix_dir(dir)).filter(|dir| !dir.is_empty() && !dir.contains(':'))
        .collect::<Vec<_>>().join(":")
}

/// A directory as the POSIX shell spells it: Git Bash takes Windows paths in its own drive syntax.
fn posix_dir(dir: &Path) -> String {
    let path = dir.to_string_lossy();
    #[cfg(windows)]
    let path = posix_windows_path(&path);
    path.to_string()
}

#[cfg(any(windows, test))]
fn posix_windows_path(path: &str) -> String {
    let path = path.strip_prefix(r"\\?\").unwrap_or(path).replace('\\', "/");
    if let Some(rest) = path.strip_prefix("UNC/") { return format!("//{rest}"); }
    let bytes = path.as_bytes();
    if bytes.len() > 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/' {
        return format!("/{}{}", (bytes[0] as char).to_ascii_lowercase(), &path[2..]);
    }
    path
}

/// First executable of this name on PATH.
///
/// The agent may have been installed by a package manager none of the fixed-location probes know about,
/// so PATH is the last word before declaring it absent.
pub(crate) fn find_on_path(bin: &str) -> Option<String> {
    #[cfg(windows)]
    if let Some(path) = crate::login_env::latest_path() {
        if let Some(found) = find_on_path_in(bin, &path) { return Some(found); }
    }
    let path = std::env::var_os("PATH")?;
    find_on_path_in(bin, &path)
}

pub(crate) fn find_on_path_in(bin: &str, path: &std::ffi::OsStr) -> Option<String> {
    for dir in std::env::split_paths(path) {
        for name in exe_names(bin) {
            let candidate = dir.join(&name);
            if is_executable_file(&candidate) {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
    }
    None
}

/// Filenames to try for one command, covering Windows's extension-based lookup.
fn exe_names(bin: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        return ["exe", "cmd", "bat"]
            .iter()
            .map(|ext| format!("{bin}.{ext}"))
            .collect();
    }
    #[cfg(not(windows))]
    {
        vec![bin.to_string()]
    }
}

pub(crate) fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    // A generated Windows command wrapper counts as executable only while the program it forwards to
    // is present; otherwise discovery would hand out a wrapper that fails with an opaque cmd.exe
    // path error instead of letting the install guidance appear.
    #[cfg(windows)]
    let shim_dead = shim_payload_missing(path);
    #[cfg(not(windows))]
    let shim_dead = false;
    if shim_dead {
        return false;
    }
    // Unix installers download to a temporary file or set the permission bit afterwards, but Windows
    // installers such as OMP's stream straight into the final `.exe`; a file still being written is not
    // an installation yet, and restarting the session on it would kill the installer mid-download.
    if being_written(path) {
        return false;
    }
    // Windows has no permission bit to read here; existing as a file is as far as this check goes.
    #[cfg(unix)]
    let runnable = {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    };
    #[cfg(not(unix))]
    let runnable = true;
    runnable
}

/// Reads the program a generated Windows command wrapper forwards to, if the file has that shape.
///
/// npm, pnpm, and yarn place `.cmd`/`.bat` wrappers beside the real programs. Generated wrappers
/// reference their payload as a quoted `%dp0%`-relative path, and the last such path is the one
/// executed; earlier ones belong to interpreter probing such as `IF EXIST "%dp0%\node.exe"`. Files
/// without that shape, oversized files, and non-UTF-8 files return None: their format is unknown
/// rather than wrong, so callers leave them alone.
fn shim_payload(path: &Path) -> Option<std::path::PathBuf> {
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    if !name.ends_with(".cmd") && !name.ends_with(".bat") {
        return None;
    }
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    let mut text = String::new();
    // Generated wrappers are a few hundred bytes; anything larger is not one.
    file.take(64 * 1024).read_to_string(&mut text).ok()?;
    let dir = path.parent()?;
    let mut payload = None;
    for segment in text.split('"').skip(1).step_by(2) {
        let Some(rest) = ["%dp0%", "%~dp0"]
            .iter()
            .find_map(|prefix| segment.strip_prefix(prefix))
        else {
            continue;
        };
        let relative = rest
            .trim_start_matches(['\\', '/'])
            .replace('\\', std::path::MAIN_SEPARATOR_STR);
        if !relative.is_empty() {
            payload = Some(dir.join(relative));
        }
    }
    payload
}

/// Whether another process currently holds the file open for writing, such as a download or copy in progress.
///
/// Opening for reading while denying write sharing fails with a sharing violation exactly when a writer
/// already has the file open. A running executable holds no write handle, so an agent in use elsewhere still
/// counts as installed. Any other failure is not treated as writing, leaving the decision to the other checks.
#[cfg(windows)]
fn being_written(path: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_DELETE: u32 = 0x4;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    match std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
        .open(path)
    {
        Ok(_) => false,
        Err(e) => e.raw_os_error() == Some(ERROR_SHARING_VIOLATION),
    }
}

/// Unix installers signal completion through the permission bit instead; see `is_executable_file`.
#[cfg(not(windows))]
fn being_written(_path: &Path) -> bool {
    false
}

/// Whether a wrapper payload is present and, for `.exe` payloads, is a Windows executable.
///
/// A half-finished install can leave the wrapper while its payload is missing, or leave a text
/// placeholder where the real binary belongs; both fail later with an opaque cmd.exe path error.
fn payload_live(path: &Path) -> bool {
    if !path.is_file() || being_written(path) {
        return false;
    }
    if !path.to_string_lossy().to_ascii_lowercase().ends_with(".exe") {
        return true;
    }
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 2];
    file.read_exact(&mut magic).is_ok() && magic == *b"MZ"
}

/// Whether a generated command wrapper is present without the program it forwards to.
///
/// A failed, interrupted, or quarantined install leaves the wrapper while removing the package it
/// points at, and running the wrapper then prints only cmd.exe's bare "The system cannot find the
/// path specified." Discovery treats that state as not installed so the installation guidance
/// appears instead.
pub(crate) fn shim_payload_missing(path: &Path) -> bool {
    match shim_payload(path) {
        Some(payload) => !payload_live(&payload),
        None => false,
    }
}

/// The executable a generated wrapper forwards to, when that payload is present and runnable.
///
/// Launching the payload directly skips cmd.exe's re-parsing of the wrapper's arguments, which can
/// mangle structured values such as JSON.
pub(crate) fn shim_payload_exe(path: &Path) -> Option<std::path::PathBuf> {
    let payload = shim_payload(path)?;
    if payload.to_string_lossy().to_ascii_lowercase().ends_with(".exe") && payload_live(&payload) {
        Some(payload)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn shell_probe_drains_large_banners_and_keeps_the_final_capture() {
        let output = shell_probe_output("/bin/bash", "-c", "printf '%70000s' ''; printf '\\nFINAL_CAPTURE\\n'").unwrap();
        assert!(output.len() <= 65536);
        assert!(output.ends_with(b"\nFINAL_CAPTURE\n"));
    }

    #[test]
    fn gitbash_path_entries_use_posix_drive_and_unc_syntax() {
        assert_eq!(posix_windows_path(r"C:\Users\用户\new tool"), "/c/Users/用户/new tool");
        assert_eq!(posix_windows_path(r"\\?\D:\tools\bin"), "/d/tools/bin");
        assert_eq!(posix_windows_path(r"\\?\UNC\host\share\bin"), "//host/share/bin");
        assert_eq!(posix_windows_path(r"\\host\share\bin"), "//host/share/bin");
    }

    #[cfg(unix)]
    #[test]
    fn typed_path_append_stays_short_and_appends_missing_dirs_in_every_shell() {
        // Many long directories must not lengthen the typed command past the terminal's line limit.
        let mut dirs: Vec<_> = (0..64).map(|i| std::path::PathBuf::from(format!("/opt/long directory/it's {i}/bin"))).collect();
        dirs.push("/usr/bin".into());
        dirs.push("/bad:dir".into());
        assert!(PATH_APPEND_COMMAND.len() < 512);
        let value = path_append_value(&dirs);
        assert!(!value.contains("/bad"));
        let expected = format!("/usr/bin:/bin:{}", dirs[..64].iter().map(|dir| dir.to_string_lossy())
            .collect::<Vec<_>>().join(":"));
        // zsh reads `.zshenv` even for `-c`; `-f` keeps the user's startup files out of the comparison.
        for (shell, flag) in [("/bin/sh", "-c"), ("/bin/bash", "-c"), ("/bin/zsh", "-fc")] {
            if !Path::new(shell).exists() { continue; }
            let output = std::process::Command::new(shell)
                .env_clear().env("PATH", "/usr/bin:/bin").env(PATH_APPEND_VAR, &value)
                .args([flag, &format!("{PATH_APPEND_COMMAND} printf '%s' \"$PATH\"")]).output().unwrap();
            assert!(output.status.success(), "{shell}");
            assert_eq!(String::from_utf8(output.stdout).unwrap(), expected, "{shell}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn terminal_paths_include_all_agent_defaults_and_valid_presets() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("vlx-configured-paths-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let app = AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(
            dir.clone(), crate::db::Db::open(&dir.join("test.db")).unwrap(),
        )));
        let mut defaults = serde_json::Map::new();
        let mut expected = Vec::new();
        for option in super::super::launch_options::catalog() {
            if matches!(option.id, SessionKind::Terminal | SessionKind::Browser) { continue; }
            let parent = dir.join(option.id.as_str());
            std::fs::create_dir_all(&parent).unwrap();
            let bin = parent.join(command_name(option.id));
            std::fs::write(&bin, "#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
            defaults.insert(option.id.as_str().into(), serde_json::json!({"path":bin}));
            expected.push(parent);
        }
        assert_eq!(expected.len(), 14);
        let preset_dir = dir.join("preset");
        std::fs::create_dir_all(&preset_dir).unwrap();
        let preset_bin = preset_dir.join("custom-agent");
        std::fs::write(&preset_bin, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&preset_bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let missing = dir.join("missing/bin/claude");
        {
            let conn = app.db().conn.lock().unwrap();
            crate::db::repo::set_app_settings(&conn, &std::collections::HashMap::from([
                ("vlx-settings".into(), serde_json::json!({"agentDefaults":defaults}).to_string()),
            ])).unwrap();
            crate::db::repo::create_agent_preset(&conn, "Valid", SessionKind::Claude,
                preset_bin.to_str(), None, None, None).unwrap();
            crate::db::repo::create_agent_preset(&conn, "Missing", SessionKind::Claude,
                missing.to_str(), None, None, None).unwrap();
        }
        expected.push(preset_dir);
        let dirs = binary_dirs(&terminal_environment(&app, &super::super::install::probe_shell(), None, false).0);
        for parent in &expected { assert!(dirs.contains(parent), "missing {}", parent.display()); }
        assert!(!dirs.contains(&missing.parent().unwrap().to_path_buf()));
        let mut command = portable_pty::CommandBuilder::new("/bin/bash");
        command.env("PATH", "/usr/bin:/bin");
        prepare_pty(&mut command, &dirs);
        prepare_pty(&mut command, &dirs);
        let paths: Vec<_> = std::env::split_paths(command.get_env("PATH").unwrap()).collect();
        assert_eq!(&paths[..2], &[std::path::PathBuf::from("/usr/bin"), std::path::PathBuf::from("/bin")]);
        for parent in &expected { assert_eq!(paths.iter().filter(|path| *path == parent).count(), 1); }
        // The terminal fallback must never change explicit-path authority for a typed session.
        assert_eq!(resolve_binary(&app, SessionKind::Claude, missing.to_str()).unwrap(),
            LaunchBinary::Path(missing.to_string_lossy().into_owned()));
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn terminal_and_chat_read_the_same_registered_path_and_updates() {
        let dir = std::env::temp_dir().join(format!("vlx-executable-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let app = AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(
            dir.clone(),
            db,
        )));
        let mut session = {
            let conn = app.db().conn.lock().unwrap();
            let project =
                crate::db::repo::create_virtual_project(&conn, "Executable test").unwrap();
            crate::db::repo::create_session(
                &conn,
                &project.id,
                None,
                "Codex",
                SessionKind::Codex,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap()
        };
        for path in ["/registered/first/codex", "/registered/second/codex"] {
            {
                let conn = app.db().conn.lock().unwrap();
                crate::db::repo::set_app_settings(
                    &conn,
                    &std::collections::HashMap::from([(
                        "vlx-settings".into(),
                        serde_json::json!({"agentDefaults":{"codex":{"path":path}}}).to_string(),
                    )]),
                )
                .unwrap();
            }
            assert_eq!(for_session(&app, &session), path);
            assert_eq!(
                resolve_launch_binary(&app, &session.id, session.kind).unwrap(),
                LaunchBinary::Path(path.into())
            );
        }
        session.agent_path = Some("/session/codex".into());
        {
            let conn = app.db().conn.lock().unwrap();
            conn.execute(
                "UPDATE sessions SET agent_path = ?1 WHERE id = ?2",
                rusqlite::params![session.agent_path, session.id],
            )
            .unwrap();
        }
        assert_eq!(for_session(&app, &session), "/session/codex");
        assert_eq!(
            resolve_launch_binary(&app, &session.id, session.kind).unwrap(),
            LaunchBinary::Path("/session/codex".into())
        );
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn direct_launch_finds_an_adjacent_runtime_with_minimal_path() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("vlx-agent-runtime-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("agent");
        let runtime = dir.join("vlx-test-runtime");
        std::fs::write(&bin, "#!/usr/bin/env vlx-test-runtime\n").unwrap();
        std::fs::write(&runtime, "#!/bin/sh\nprintf runtime-ok\n").unwrap();
        for path in [&bin, &runtime] {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut command = std::process::Command::new(&bin);
        command.env("PATH", "/usr/bin:/bin");
        prepare_command(&mut command, bin.to_str().unwrap());
        let output = command.output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"runtime-ok");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn interactive_shell_returns_an_executable_path() {
        let path = find_in_shell("sh").expect("the login shell should resolve sh");
        assert!(Path::new(&path).is_absolute());
        assert!(is_executable_file(Path::new(&path)));
    }

    #[test]
    fn explicit_paths_win_without_discovery() {
        assert_eq!(
            select(Some(" /missing/session "), Some("/global"), || panic!(
                "unexpected discovery"
            )),
            Some("/missing/session".into())
        );
        assert_eq!(
            select(Some("  "), Some(" /global "), || panic!(
                "unexpected discovery"
            )),
            Some("/global".into())
        );
    }

    #[test]
    fn empty_paths_use_discovery() {
        assert_eq!(
            select(None, Some(" "), || Some("/discovered".into())),
            Some("/discovered".into())
        );
        assert_eq!(select(None, None, || None), None);
    }

    #[test]
    fn configured_home_is_expanded() {
        if let Some(home) = crate::host::home_dir() {
            assert_eq!(
                configured_path(Some(" ~/.local/bin/codex ")),
                Some(home.join(".local/bin/codex").to_string_lossy().into_owned())
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn runtime_path_preserves_existing_entries() {
        let mut command = std::process::Command::new("/opt/agent/bin/codex");
        command.env("PATH", "/usr/bin:/bin");
        prepare_command(&mut command, "/opt/agent/bin/codex");
        let path = command
            .get_envs()
            .find(|(key, _)| *key == "PATH")
            .unwrap()
            .1
            .unwrap();
        assert_eq!(
            std::env::split_paths(path).collect::<Vec<_>>(),
            vec![
                std::path::PathBuf::from("/opt/agent/bin"),
                "/usr/bin".into(),
                "/bin".into()
            ]
        );
    }

    /// Temp directory holding command-wrapper fixtures for one test.
    fn shim_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vlx-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Path text with forward slashes so expectations read the same on every platform.
    fn normalized(path: &Path) -> String {
        path.to_string_lossy().replace('\\', "/")
    }

    #[test]
    fn shim_payload_reads_the_last_dp0_target() {
        let dir = shim_dir("shim-payload");
        // A generated JavaScript wrapper probes `%dp0%\node.exe` first and forwards to the package script.
        let js = dir.join("cli.cmd");
        std::fs::write(
            &js,
            concat!(
                "IF EXIST \"%dp0%\\node.exe\" (\r\n",
                "  SET \"_prog=%dp0%\\node.exe\"\r\n",
                ")\r\n",
                "endLocal & \"%_prog%\"  \"%dp0%\\node_modules\\pkg\\cli.js\" %*\r\n"
            ),
        )
        .unwrap();
        assert_eq!(
            normalized(&shim_payload(&js).unwrap()),
            format!("{}/node_modules/pkg/cli.js", normalized(&dir))
        );
        // A package that ships a native executable forwards straight to it.
        let exe = dir.join("opencode.cmd");
        std::fs::write(
            &exe,
            "\"%dp0%\\node_modules\\opencode-ai\\bin\\opencode.exe\"   %*\r\n",
        )
        .unwrap();
        assert_eq!(
            normalized(&shim_payload(&exe).unwrap()),
            format!("{}/node_modules/opencode-ai/bin/opencode.exe", normalized(&dir))
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn shim_payload_ignores_files_without_a_generated_shape() {
        let dir = shim_dir("shim-shape");
        let plain = dir.join("plain.cmd");
        std::fs::write(&plain, "@echo off\r\necho hello\r\n").unwrap();
        assert_eq!(shim_payload(&plain), None);
        // Only batch wrappers carry the pattern; a shell script named like one is left alone.
        let script = dir.join("bash.cmd");
        std::fs::write(&script, "#!/bin/sh\nexec bash\n").unwrap();
        assert_eq!(shim_payload(&script), None);
        let sh = dir.join("run.sh");
        std::fs::write(&sh, "#!/bin/sh\nexec \"$basedir/x\"\n").unwrap();
        assert_eq!(shim_payload(&sh), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn payload_liveness_requires_presence_and_exe_magic() {
        let dir = shim_dir("shim-live");
        assert!(!payload_live(&dir.join("missing.exe")));
        // The placeholder a stalled OpenCode install leaves behind is a text file using the .exe name.
        let placeholder = dir.join("placeholder.exe");
        std::fs::write(&placeholder, "echo not installed\n").unwrap();
        assert!(!payload_live(&placeholder));
        let real = dir.join("real.exe");
        std::fs::write(&real, b"MZ\x90\x00rest").unwrap();
        assert!(payload_live(&real));
        // Script payloads only need to exist; the interpreter is resolved separately.
        let script = dir.join("cli.js");
        std::fs::write(&script, "console.log('ok')\n").unwrap();
        assert!(payload_live(&script));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn a_file_still_being_written_is_not_executable() {
        let dir = shim_dir("being-written");
        let exe = dir.join("omp.exe");
        // A download in progress keeps a write handle open on the final file.
        let writer = std::fs::File::create(&exe).unwrap();
        assert!(being_written(&exe));
        assert!(!is_executable_file(&exe), "a half-written file is not an installation");
        drop(writer);
        assert!(is_executable_file(&exe));
        // The test binary itself is running, which must not count as being written.
        assert!(!being_written(&std::env::current_exe().unwrap()));
        // A wrapper whose payload is still being copied is not usable yet either.
        let wrapper = dir.join("opencode.cmd");
        std::fs::write(&wrapper, "\"%dp0%\\payload.exe\"   %*\r\n").unwrap();
        let payload = dir.join("payload.exe");
        let mut copying = std::fs::File::create(&payload).unwrap();
        std::io::Write::write_all(&mut copying, b"MZ\x90\x00").unwrap();
        assert!(shim_payload_missing(&wrapper));
        drop(copying);
        assert!(!shim_payload_missing(&wrapper));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn dangling_shim_is_reported_missing() {
        let dir = shim_dir("shim-dangling");
        let wrapper = dir.join("opencode.cmd");
        std::fs::write(
            &wrapper,
            "\"%dp0%\\node_modules\\opencode-ai\\bin\\opencode.exe\"   %*\r\n",
        )
        .unwrap();
        assert!(shim_payload_missing(&wrapper));
        assert_eq!(shim_payload_exe(&wrapper), None);
        // Putting the real program back restores both liveness and the payload-exe preference.
        let bin = dir.join("node_modules").join("opencode-ai").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("opencode.exe"), b"MZ\x90\x00rest").unwrap();
        assert!(!shim_payload_missing(&wrapper));
        assert_eq!(shim_payload_exe(&wrapper), Some(bin.join("opencode.exe")));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
