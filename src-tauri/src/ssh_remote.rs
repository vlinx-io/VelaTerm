//! Client-side SSH remote orchestration for GUI builds.
//!
//! Connect by SSH, detect the remote system, provision `vela-server`, start `--serve --local-http`,
//! establish `ssh -L` forwarding, and open an auto-login client.
//!
//! Linux, macOS, and Windows remotes are all supported. The remote command language is decided by the
//! probe, not by the OS: `uname` answers on Linux, macOS, and a Cygwin/MSYS sshd, while Microsoft's
//! OpenSSH service has no POSIX tools and is driven through PowerShell instead. See [`RemoteShell`].
//!
//! Design: use OpenSSH and its agent/config ecosystem rather than implementing SSH. Windows prefers
//! bundled OpenSSH over unreliable PATH installs. Unix reuses a ControlMaster socket so authentication
//! occurs once; broken Windows multiplexing uses independent connections. Probe host keys with
//! ssh-keyscan, compare SHA-256 against known_hosts, and let the frontend confirm instead of terminal prompts.
//!
//! `commands.rs` exposes this orchestration to the frontend through `ssh_*` commands.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::agent::inject::{split_ssh_target, valid_ssh_target};
use crate::host;

/// Progress callback `(phase, optional 0..=100 percent)`, emitted as ssh://progress. Phases are connect,
/// probe, supply, transfer, start, and forward; only supply/transfer carry percentages.
pub type Progress<'a> = &'a (dyn Fn(&str, Option<u8>) + Sync);

/// Stable prefix marking rejected key authentication so the frontend can prompt for a password and retry.
pub const AUTH_REQUIRED_TAG: &str = "__VLX_SSH_AUTH_REQUIRED__";

/// Remote no-op used to verify authentication before the remote system is known.
///
/// It must succeed under every login shell we may land in. `true` does not: a Windows remote running
/// Microsoft's OpenSSH executes the command through cmd.exe, which reports `'true' is not recognized`
/// and returns 1, so the connection failed before anything could be probed. `exit 0` is a builtin in sh,
/// cmd.exe, and PowerShell alike.
const NOOP_REMOTE_CMD: &str = "exit 0";

// ─────────────────────────── OpenSSH tool resolution (bundled first) ───────────────────────────

/// Process-stable application data directory used by Windows to locate bundled OpenSSH.
static SSH_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Set data_dir once at startup for bundled SSH tool lookup; meaningful only on Windows.
pub fn set_data_dir(dir: PathBuf) {
    let _ = SSH_DATA_DIR.set(dir);
}

/// Resolve an OpenSSH executable. Windows prefers bundled full then minimal Git Bash tools to avoid
/// missing/broken system clients, falling back to PATH. Other platforms always use PATH.
///
/// Return an OsString suitable for Command constructors.
fn ssh_tool(tool: &str) -> std::ffi::OsString {
    #[cfg(windows)]
    if let Some(dir) = SSH_DATA_DIR.get() {
        if let Some(exe) = crate::agent::gitbash::bundled_tool(dir, tool) {
            return exe.into_os_string();
        }
    }
    tool.into()
}

/// Build a host command using ssh_tool resolution while preserving platform launch behavior.
fn ssh_command(tool: &str) -> Command {
    host::command(ssh_tool(tool))
}

// ─────────────────────────── Host history and keyring passwords ───────────────────────────

/// SSH history row augmented with whether the keyring contains a remembered password.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshHostInfo {
    /// `user@host[:port]`, also used as the keyring account.
    pub target: String,
    /// Reserved user alias; currently always None.
    pub label: Option<String>,
    /// Most recent successful connection timestamp in seconds.
    pub last_connected_at: i64,
    /// Whether a password is remembered in the system keyring.
    pub has_password: bool,
    /// Previous choice to reuse the remote desktop database, restored by reconnect UI.
    pub shared_db: bool,
    /// Previous choice to mirror the UI layout across the service's clients, restored by reconnect UI.
    pub mirror: bool,
}

/// Keyring service name isolates dev/release identifiers and SSH versus URL password kinds.
fn keyring_service(identifier: &str, kind: &str) -> String {
    format!("{identifier}.{kind}")
}

/// Store a password without trimming. Return false when secure storage is unavailable; never use plaintext fallback.
pub fn store_password(identifier: &str, kind: &str, account: &str, password: &str) -> bool {
    keyring::Entry::new(&keyring_service(identifier, kind), account)
        .and_then(|e| e.set_password(password))
        .is_ok()
}

/// Read a remembered password, or None when absent/unavailable.
pub fn load_password(identifier: &str, kind: &str, account: &str) -> Option<String> {
    keyring::Entry::new(&keyring_service(identifier, kind), account)
        .ok()
        .and_then(|e| e.get_password().ok())
        .filter(|s| !s.is_empty())
}

/// Delete a remembered password; missing entries are no-ops.
pub fn delete_password(identifier: &str, kind: &str, account: &str) {
    if let Ok(e) = keyring::Entry::new(&keyring_service(identifier, kind), account) {
        let _ = e.delete_credential();
    }
}

/// Whether an account has a keyring password, used for list indicators.
pub fn has_password(identifier: &str, kind: &str, account: &str) -> bool {
    keyring::Entry::new(&keyring_service(identifier, kind), account)
        .ok()
        .and_then(|e| e.get_password().ok())
        .map(|s| !s.is_empty())
        .unwrap_or(false)
}

/// Command language of the remote login shell, which decides how every remote command is written.
///
/// A Windows remote can answer with either one: Microsoft's OpenSSH service runs cmd/PowerShell and has
/// no POSIX tools, while a Cygwin/MSYS sshd answers `uname` and offers a full POSIX toolbox. Detection
/// therefore records the shell separately from the OS instead of deriving one from the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteShell {
    /// sh-compatible login shell (Linux, macOS, and Cygwin/MSYS on Windows).
    Posix,
    /// Windows PowerShell, driven through `-EncodedCommand` so no quoting reaches cmd.exe.
    Powershell,
}

/// Detected remote system information.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSystem {
    /// Normalized OS: linux, macos, or windows.
    pub os: String,
    /// Normalized architecture: x86_64 or aarch64.
    pub arch: String,
    /// Raw probe output for diagnostics.
    pub raw: String,
    /// Command language used for every remote command on this host.
    pub shell: RemoteShell,
}

impl RemoteSystem {
    /// Whether the remote runs Windows, where the server artifact carries an `.exe` suffix.
    fn is_windows(&self) -> bool {
        self.os == "windows"
    }

    /// Remote binary path written in the detected shell's own syntax.
    fn bin_path(&self, version: &str) -> String {
        let exe = if self.is_windows() { ".exe" } else { "" };
        match self.shell {
            RemoteShell::Posix => format!("$HOME/.velaterm/versions/{version}/vela-server{exe}"),
            RemoteShell::Powershell => {
                format!("$env:USERPROFILE\\.velaterm\\versions\\{version}\\vela-server{exe}")
            }
        }
    }

    /// Remote service state path (`run.json`) written in the detected shell's own syntax.
    fn run_json(&self) -> String {
        match self.shell {
            RemoteShell::Posix => "$HOME/.velaterm/run.json".to_string(),
            RemoteShell::Powershell => "$env:USERPROFILE\\.velaterm\\run.json".to_string(),
        }
    }
}

/// Fallback system used by entry points that only carry `(host, session)` and never saw a probe result.
/// Linux plus a POSIX shell reproduces the behavior that existed before Windows remotes were supported.
fn default_system() -> RemoteSystem {
    RemoteSystem {
        os: "linux".to_string(),
        arch: "x86_64".to_string(),
        raw: String::new(),
        shell: RemoteShell::Posix,
    }
}

/// Per-session detected system, so disconnect/kill/tunnel-rebuild paths can speak the right shell without
/// re-probing. Registered right after detection and removed on disconnect.
static SESSION_SYS: OnceLock<Mutex<HashMap<String, RemoteSystem>>> = OnceLock::new();

fn session_sys_map() -> &'static Mutex<HashMap<String, RemoteSystem>> {
    SESSION_SYS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Record a session's detected system for later shell-specific commands.
fn set_session_sys(session: &str, sys: &RemoteSystem) {
    session_sys_map()
        .lock()
        .unwrap()
        .insert(session.to_string(), sys.clone());
}

/// Read a session's detected system, falling back to Linux/POSIX when it was never registered.
fn session_sys(session: &str) -> RemoteSystem {
    session_sys_map()
        .lock()
        .unwrap()
        .get(session)
        .cloned()
        .unwrap_or_else(default_system)
}

/// Forget a session's detected system on disconnect.
fn clear_session_sys(session: &str) {
    session_sys_map().lock().unwrap().remove(session);
}

/// What the tunnel of a session points at: the headless service this client started, or the remote
/// desktop app's own local link (see `web::local_link`). The two differ in who owns the process, so the
/// tunnel-rebuild check and the disconnect semantics differ too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServiceTarget {
    /// `vela-server --serve` started (or reused) by this client and recorded in run.json.
    Headless,
    /// The desktop app running on the remote machine, reached through its local-link port. The process is
    /// not ours: disconnecting only drops the tunnel, never the desktop.
    DesktopLink,
}

/// Per-session service target, registered when the connection is established and read by tunnel rebuild
/// and disconnect. Sessions never registered default to `Headless`, which reproduces earlier behavior.
static SESSION_TARGET: OnceLock<Mutex<HashMap<String, ServiceTarget>>> = OnceLock::new();

fn session_target_map() -> &'static Mutex<HashMap<String, ServiceTarget>> {
    SESSION_TARGET.get_or_init(|| Mutex::new(HashMap::new()))
}

fn set_session_target(session: &str, target: ServiceTarget) {
    session_target_map()
        .lock()
        .unwrap()
        .insert(session.to_string(), target);
}

fn session_target(session: &str) -> ServiceTarget {
    session_target_map()
        .lock()
        .unwrap()
        .get(session)
        .copied()
        .unwrap_or(ServiceTarget::Headless)
}

fn clear_session_target(session: &str) {
    session_target_map().lock().unwrap().remove(session);
}

/// Wrap a PowerShell script as `-EncodedCommand` so the command line carries only base64 characters.
///
/// The remote login shell may be cmd.exe or PowerShell and each quotes differently; base64 passes through
/// both untouched. PowerShell requires UTF-16LE before base64.
fn ps_encoded(script: &str) -> String {
    format!(
        "powershell -NoProfile -NonInteractive -EncodedCommand {}",
        ps_b64(script)
    )
}

/// Encode a PowerShell script as the base64 UTF-16LE blob `-EncodedCommand` expects.
fn ps_b64(script: &str) -> String {
    use base64::Engine;
    let mut bytes = Vec::with_capacity(script.len() * 2);
    for u in script.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}

/// Run a script written for the detected shell, encoding it first when that shell is PowerShell.
fn shell_exec(t: &dyn SshTransport, sys: &RemoteSystem, script: &str) -> Result<String, String> {
    match sys.shell {
        RemoteShell::Posix => t.exec(script),
        RemoteShell::Powershell => t.exec(&ps_encoded(script)),
    }
}

/// Host-key comparison against known_hosts.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HostKeyStatus {
    /// Existing matching key; connect without prompting.
    Known,
    /// Unseen host; prompt for confirmation.
    New,
    /// Previously known but changed key; show a danger warning.
    Changed,
}

/// Fingerprint probe result for frontend verification.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostKeyProbe {
    pub status: HostKeyStatus,
    /// Display string `user@host:port`.
    pub target: String,
    /// Key type such as ssh-ed25519, ecdsa, or ssh-rsa.
    pub key_type: String,
    /// SSH-compatible `SHA256:...` fingerprint for exact comparison.
    pub fingerprint: String,
}

// ─────────────────────────── ControlMaster connection reuse ───────────────────────────

/// Process-private, per-session control-socket directory cleaned after connection.
fn control_dir() -> PathBuf {
    std::env::temp_dir().join("vlx-ssh-cm")
}

/// Control-socket path for a caller-supplied connection/session isolation key.
pub fn control_path(session: &str) -> PathBuf {
    control_dir().join(format!("cm-{session}.sock"))
}

/// Build common SSH arguments: Unix ControlMaster reuse, optional port, quiet/timeout settings, and
/// strict known_hosts checking after confirmation versus accept-new for a pre-probed first connection.
///
/// Windows disables ControlMaster because Microsoft/MSYS2 clients fail multiplexing. Each SSH/SCP
/// connects independently; key/agent authentication makes that transparent. `sock` is Unix-only.
#[cfg_attr(windows, allow(unused_variables))]
fn ssh_common_args(sock: &Path, port: Option<&str>, strict: bool) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    // Control options apply only on Unix; Windows skips them entirely.
    #[cfg(not(windows))]
    {
        a.push("-o".into());
        a.push("ControlMaster=auto".into());
        a.push("-o".into());
        a.push(format!("ControlPath={}", sock.display()));
        a.push("-o".into());
        a.push("ControlPersist=120".into());
    }
    a.push("-o".into());
    a.push(format!(
        "StrictHostKeyChecking={}",
        if strict { "yes" } else { "accept-new" }
    ));
    a.push("-o".into());
    a.push("ConnectTimeout=15".into());
    // Application keepalive declares failure after three unanswered 15-second probes. Whichever process
    // becomes ControlMaster carries it; its forwarded sessions then exit within ~45 seconds after network
    // loss, allowing the tunnel monitor's process-plus-port check to recover instead of hanging half-open.
    a.push("-o".into());
    a.push("ServerAliveInterval=15".into());
    a.push("-o".into());
    a.push("ServerAliveCountMax=3".into());
    a.push("-o".into());
    a.push("TCPKeepAlive=yes".into());
    if let Some(p) = port {
        a.push("-p".into());
        a.push(p.to_string());
    }
    a
}

/// Establish/reuse a noninteractive key/agent connection. Unix leaves a control socket for subsequent
/// commands; Windows merely runs `true` to verify auth and reconnects independently afterward.
pub fn open_master(host: &str, session: &str, strict: bool) -> Result<(), String> {
    if !valid_ssh_target(host) {
        return Err(format!("Invalid SSH target: {host}"));
    }
    let (target, port) = split_ssh_target(host);
    std::fs::create_dir_all(control_dir())
        .map_err(|e| format!("failed to create control socket dir: {e}"))?;
    let sock = control_path(session);

    let mut cmd = ssh_command("ssh");
    cmd.args(ssh_common_args(&sock, port, strict));
    // BatchMode disables prompts in the non-TTY core path; PTY handles interactive authentication.
    cmd.args(["-o", "BatchMode=yes"]);
    cmd.arg(target);
    // Run a no-op only to prove authentication; ControlPersist keeps the resulting master in the
    // background. `exit 0` rather than `true` because a Windows remote runs this through cmd.exe, which
    // has no `true` and would fail the whole connection before the system is ever probed.
    cmd.arg(NOOP_REMOTE_CMD);
    run_capture(cmd, "establish SSH control connection")?;
    Ok(())
}

/// Establish a password-authenticated master through a PTY by answering one prompt. ControlPersist then
/// makes it equivalent to open_master for subsequent reuse. Used only after key auth is rejected.
///
/// The password lives only here and is written to the PTY, never argv, logs, progress, or run.json.
///
/// Full Windows OpenSSH password orchestration is unsupported; this is currently Unix/test-only.
#[cfg_attr(windows, allow(dead_code))]
pub fn open_master_pty(
    host: &str,
    session: &str,
    strict: bool,
    password: &str,
) -> Result<(), String> {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    use std::io::{Read, Write};

    if !valid_ssh_target(host) {
        return Err(format!("Invalid SSH target: {host}"));
    }
    let (target, port) = split_ssh_target(host);
    std::fs::create_dir_all(control_dir())
        .map_err(|e| format!("failed to create control socket dir: {e}"))?;
    let sock = control_path(session);

    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("failed to open pty for ssh auth: {e}"))?;

    let mut cmd = CommandBuilder::new(ssh_tool("ssh"));
    // Preserve HOME/PATH/SSH_AUTH_SOCK and supply TERM to the PTY.
    for (k, v) in std::env::vars() {
        cmd.env(k, v);
    }
    cmd.env("TERM", "xterm-256color");
    for a in ssh_common_args(&sock, port, strict) {
        cmd.arg(a);
    }
    // Force password/keyboard-interactive after proven key rejection and allow only one prompt.
    for a in [
        "-o",
        "PubkeyAuthentication=no",
        "-o",
        "PreferredAuthentications=password,keyboard-interactive",
        "-o",
        "NumberOfPasswordPrompts=1",
    ] {
        cmd.arg(a);
    }
    cmd.arg(target);
    // Run a no-op only to prove authentication; ControlPersist retains the master. See NOOP_REMOTE_CMD.
    cmd.arg(NOOP_REMOTE_CMD);

    let mut child = pair.slave.spawn_command(cmd).map_err(|e| {
        format!("establish SSH control connection failed to run (ssh missing locally?): {e}")
    })?;
    // The child owns the slave fd; release the parent copy so readers receive EOF on exit.
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("failed to read ssh pty: {e}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("failed to write ssh pty: {e}"))?;

    // Reader accumulates output, answers the first password prompt once, and returns output at EOF.
    let pw = password.to_string();
    let pump = std::thread::spawn(move || {
        let mut writer = writer;
        let mut acc: Vec<u8> = Vec::new();
        let mut buf = [0u8; 1024];
        let mut sent = false;
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    acc.extend_from_slice(&buf[..n]);
                    if !sent && String::from_utf8_lossy(&acc).contains("assword") {
                        let _ = writer.write_all(pw.as_bytes());
                        let _ = writer.write_all(b"\n");
                        let _ = writer.flush();
                        sent = true;
                    }
                }
                Err(_) => break,
            }
        }
        String::from_utf8_lossy(&acc).to_string()
    });

    // Wait up to 30 seconds in addition to SSH's 15-second connect timeout.
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                return Err(format!("establish SSH control connection failed: {e}"));
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            return Err(
                "establish SSH control connection failed: password auth timed out".to_string(),
            );
        }
        std::thread::sleep(Duration::from_millis(150));
    };

    // On Windows ConPTY, drop the PTY master after SSH exits before joining the reader; otherwise its
    // output pipe never closes and the reader blocks forever. Unix receives EOF from slave closure.
    drop(pair.master);
    let output = pump.join().unwrap_or_default();
    if status.success() {
        Ok(())
    } else if output.contains("Permission denied") {
        Err("establish SSH control connection failed: password authentication failed (wrong password?)".to_string())
    } else {
        // Return only the final nonempty line for concise diagnostics.
        let tail = output
            .lines()
            .rev()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("");
        Err(format!("establish SSH control connection failed: {tail}"))
    }
}

/// Close a ControlMaster with `-O exit` and remove its socket, ignoring already-disconnected errors.
/// Windows has no master and returns immediately; forwarding is terminated separately.
pub fn close_master(host: &str, session: &str) {
    // Windows has no ControlPath/master to close.
    #[cfg(windows)]
    {
        let _ = (host, session);
        return;
    }
    #[cfg(not(windows))]
    {
        let (target, port) = split_ssh_target(host);
        let sock = control_path(session);
        let mut cmd = ssh_command("ssh");
        cmd.args(ssh_common_args(&sock, port, true));
        cmd.args(["-O", "exit"]);
        cmd.arg(target);
        let _ = cmd.output();
        let _ = std::fs::remove_file(&sock);
    }
}

/// Run a remote command over the established connection and return trimmed stdout.
pub fn run_remote(host: &str, session: &str, remote_cmd: &str) -> Result<String, String> {
    let (target, port) = split_ssh_target(host);
    let sock = control_path(session);
    let mut cmd = ssh_command("ssh");
    cmd.args(ssh_common_args(&sock, port, true));
    cmd.args(["-o", "BatchMode=yes"]);
    cmd.arg(target);
    cmd.arg(remote_cmd);
    run_capture(cmd, "remote command")
}

/// Run a command and capture stdout; include stderr on nonzero exit.
fn run_capture(mut cmd: Command, what: &str) -> Result<String, String> {
    let out = cmd
        .output()
        .map_err(|e| format!("{what} failed to run (ssh missing locally?): {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("{what} failed: {}", err.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

// ─────────────────────────── Remote system detection ───────────────────────────

/// Detect the remote system, trying the POSIX probe first and the PowerShell probe second.
///
/// `uname -sm` answers on Linux, macOS, and a Cygwin/MSYS sshd on Windows, and its output already names
/// the shell we should keep using. A Microsoft OpenSSH host has no `uname`: the attempt fails (cmd.exe
/// reports an unknown command), so the fallback asks PowerShell for the OS and processor architecture.
pub fn probe_system(t: &dyn SshTransport) -> Result<RemoteSystem, String> {
    let posix = t.exec("uname -sm");
    if let Ok(raw) = &posix {
        if let Some(sys) = parse_uname(raw) {
            return Ok(sys);
        }
    }
    // PowerShell reports the native architecture in PROCESSOR_ARCHITEW6432 when itself running under WOW64.
    let script = "$ErrorActionPreference='Stop'; \
                  $a=$env:PROCESSOR_ARCHITECTURE; \
                  if($env:PROCESSOR_ARCHITEW6432){$a=$env:PROCESSOR_ARCHITEW6432}; \
                  Write-Output \"Windows_NT $a\"";
    let win = t.exec(&ps_encoded(script)).map_err(|e| {
        // Report both failures: neither shell answered, so the host is not a supported remote.
        let posix_err = posix.err().unwrap_or_default();
        format!("cannot detect the remote system (uname: {posix_err}; powershell: {e})")
    })?;
    parse_windows_probe(&win)
        .ok_or_else(|| format!("cannot parse remote PowerShell probe output: {win:?}"))
}

/// Parse the PowerShell probe (`Windows_NT <PROCESSOR_ARCHITECTURE>`) into a normalized system.
fn parse_windows_probe(raw: &str) -> Option<RemoteSystem> {
    let mut it = raw.split_whitespace();
    let sys = it.next()?;
    if !sys.eq_ignore_ascii_case("Windows_NT") {
        return None;
    }
    let arch = match it.next()?.to_ascii_lowercase().as_str() {
        "amd64" | "x86_64" => "x86_64".to_string(),
        "arm64" | "aarch64" => "aarch64".to_string(),
        // Preserve unknown architectures (x86 for instance) so the supply step reports them clearly.
        other => other.to_string(),
    };
    Some(RemoteSystem {
        os: "windows".to_string(),
        arch,
        raw: raw.to_string(),
        shell: RemoteShell::Powershell,
    })
}

/// Parse uname output into normalized OS/architecture. A Cygwin/MSYS answer means Windows reached through
/// a POSIX shell, which keeps every POSIX command available.
fn parse_uname(raw: &str) -> Option<RemoteSystem> {
    let mut it = raw.split_whitespace();
    let sys = it.next()?;
    let machine = it.next()?;
    let os = match sys.to_ascii_lowercase().as_str() {
        "linux" => "linux",
        "darwin" => "macos",
        s if s.contains("mingw") || s.contains("msys") || s.contains("cygwin") => "windows",
        _ => return None,
    };
    let machine_lc = machine.to_ascii_lowercase();
    let arch = match machine_lc.as_str() {
        "x86_64" | "amd64" => "x86_64",
        "arm64" | "aarch64" => "aarch64",
        other => other, // Preserve unknown architectures for upper-layer support decisions.
    };
    Some(RemoteSystem {
        os: os.to_string(),
        arch: arch.to_string(),
        raw: raw.to_string(),
        shell: RemoteShell::Posix,
    })
}

// ─────────────────────────── Host fingerprints (TOFU) ───────────────────────────

/// Probe with ssh-keyscan, calculate SHA-256, and compare known_hosts without modifying it. trust_host
/// writes only after frontend confirmation.
pub fn probe_host_key(host: &str) -> Result<HostKeyProbe, String> {
    if !valid_ssh_target(host) {
        return Err(format!("Invalid SSH target: {host}"));
    }
    let (target, port) = split_ssh_target(host);
    // ssh-keyscan accepts hostnames without a user@ prefix.
    let hostname = target.rsplit('@').next().unwrap_or(target);

    let mut cmd = ssh_command("ssh-keyscan");
    cmd.args(["-T", "8"]);
    if let Some(p) = port {
        cmd.args(["-p", p]);
    }
    // Prefer stronger key types among all results.
    cmd.args(["-t", "ed25519,ecdsa,rsa"]);
    cmd.arg(hostname);
    let scanned = run_capture(cmd, "ssh-keyscan host key probe")?;
    let (key_type, key_b64) = first_host_key(&scanned)
        .ok_or_else(|| format!("ssh-keyscan returned no usable host key: {scanned:?}"))?;
    let fingerprint = sha256_fingerprint(&key_b64)?;

    let status = known_hosts_status(hostname, port, &fingerprint);
    Ok(HostKeyProbe {
        status,
        target: host.to_string(),
        key_type,
        fingerprint,
    })
}

/// Parse the first usable `(key_type, key_base64)` from keyscan output, skipping comments.
fn first_host_key(scanned: &str) -> Option<(String, String)> {
    for line in scanned.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut f = line.split_whitespace();
        let _host = f.next()?;
        let kt = f.next()?;
        let kb = f.next()?;
        return Some((kt.to_string(), kb.to_string()));
    }
    None
}

/// Calculate an SSH-compatible unpadded-base64 SHA-256 fingerprint from a host-key blob.
fn sha256_fingerprint(key_b64: &str) -> Result<String, String> {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    let raw = base64::engine::general_purpose::STANDARD
        .decode(key_b64.trim())
        .map_err(|e| format!("host key base64 decode failed: {e}"))?;
    let digest = Sha256::digest(&raw);
    let b64 = base64::engine::general_purpose::STANDARD_NO_PAD.encode(digest);
    Ok(format!("SHA256:{b64}"))
}

/// Query known_hosts with ssh-keygen and classify the fingerprint as Known/New/Changed.
fn known_hosts_status(hostname: &str, port: Option<&str>, fingerprint: &str) -> HostKeyStatus {
    // known_hosts represents nondefault ports as `[host]:port`.
    let needle = match port {
        Some(p) => format!("[{hostname}]:{p}"),
        None => hostname.to_string(),
    };
    // `-F` finds entries and `-l` prints their SHA-256 fingerprints.
    let mut cmd = ssh_command("ssh-keygen");
    cmd.args(["-F", &needle, "-l"]);
    let out = match cmd.output() {
        Ok(o) => o,
        Err(_) => return HostKeyStatus::New, // Without ssh-keygen, treat it as new and request confirmation.
    };
    if !out.status.success() {
        return HostKeyStatus::New; // No known_hosts match.
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // A matching stored SHA-256 is known; any different stored key is a dangerous change.
    for line in text.lines() {
        if line.contains(fingerprint) {
            return HostKeyStatus::Known;
        }
    }
    HostKeyStatus::Changed
}

// ─────────────────────────── Remote integrity checks ───────────────────────────

/// Calculate a lowercase SHA-256 remotely using Linux sha256sum, macOS shasum, or PowerShell Get-FileHash;
/// missing file/tool is Err.
pub fn remote_sha256(
    t: &dyn SshTransport,
    sys: &RemoteSystem,
    remote_path: &str,
) -> Result<String, String> {
    let cmd = match sys.shell {
        // Double-quote paths to allow $HOME expansion without word splitting; single-quote awk's `$1`.
        RemoteShell::Posix => format!(
            "sha256sum \"{p}\" 2>/dev/null | awk '{{print $1}}' || shasum -a 256 \"{p}\" | awk '{{print $1}}'",
            p = remote_path
        ),
        RemoteShell::Powershell => format!(
            "$ErrorActionPreference='Stop'; (Get-FileHash -Algorithm SHA256 -LiteralPath \"{p}\").Hash",
            p = remote_path
        ),
    };
    let out = shell_exec(t, sys, &cmd)?;
    let hex = out.split_whitespace().next().unwrap_or("").to_string();
    if hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(hex.to_ascii_lowercase())
    } else {
        Err(format!(
            "remote produced no valid SHA-256 (file missing or no hash tool): {out:?}"
        ))
    }
}

// ─────────────────────────── Trust, remote layout, and provisioning ───────────────────────────

/// Validate semver-safe characters before interpolating a version into remote commands/paths.
fn valid_version(v: &str) -> bool {
    !v.is_empty() && v.len() <= 32 && v.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Local `~/.ssh/known_hosts` path.
fn known_hosts_file() -> Result<PathBuf, String> {
    Ok(host::home_dir()
        .ok_or("cannot locate local home dir")?
        .join(".ssh")
        .join("known_hosts"))
}

/// After confirmation, write the host key to known_hosts. Remove a changed old entry first so later
/// connections can use StrictHostKeyChecking=yes without prompts.
pub fn trust_host(host: &str, was_changed: bool) -> Result<(), String> {
    if !valid_ssh_target(host) {
        return Err(format!("Invalid SSH target: {host}"));
    }
    let (target, port) = split_ssh_target(host);
    let hostname = target.rsplit('@').next().unwrap_or(target);

    if was_changed {
        let needle = match port {
            Some(p) => format!("[{hostname}]:{p}"),
            None => hostname.to_string(),
        };
        let _ = ssh_command("ssh-keygen").args(["-R", &needle]).output();
    }

    let mut cmd = ssh_command("ssh-keyscan");
    cmd.args(["-T", "8"]);
    if let Some(p) = port {
        cmd.args(["-p", p]);
    }
    cmd.args(["-t", "ed25519,ecdsa,rsa"]);
    cmd.arg(hostname);
    let scanned = run_capture(cmd, "ssh-keyscan (trust write)")?;

    let kh = known_hosts_file()?;
    if let Some(dir) = kh.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("failed to create ~/.ssh: {e}"))?;
    }
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&kh)
        .map_err(|e| format!("failed to open known_hosts: {e}"))?;
    writeln!(f, "{}", scanned.trim()).map_err(|e| format!("failed to write known_hosts: {e}"))?;
    Ok(())
}

/// Create the remote `~/.velaterm/{versions/<version>,data,bin}` layout.
pub fn ensure_remote_layout(
    t: &dyn SshTransport,
    sys: &RemoteSystem,
    version: &str,
) -> Result<(), String> {
    if !valid_version(version) {
        return Err(format!("Invalid version: {version}"));
    }
    let cmd = match sys.shell {
        RemoteShell::Posix => format!(
            "mkdir -p \"$HOME/.velaterm/versions/{version}\" \"$HOME/.velaterm/data\" \"$HOME/.velaterm/bin\""
        ),
        RemoteShell::Powershell => format!(
            "$ErrorActionPreference='Stop'; \
             $b=\"$env:USERPROFILE\\.velaterm\"; \
             foreach($p in @(\"$b\\versions\\{version}\",\"$b\\data\",\"$b\\bin\")){{ \
               New-Item -ItemType Directory -Force -Path $p | Out-Null }}"
        ),
    };
    shell_exec(t, sys, &cmd).map(|_| ())
}

/// Whether the remote version has an integrity-valid binary with the expected SHA-256.
pub fn remote_binary_ok(
    t: &dyn SshTransport,
    sys: &RemoteSystem,
    version: &str,
    expected_sha256: &str,
) -> bool {
    if !valid_version(version) {
        return false;
    }
    match remote_sha256(t, sys, &sys.bin_path(version)) {
        Ok(h) => h.eq_ignore_ascii_case(expected_sha256),
        Err(_) => false,
    }
}

/// Upload through the transport, verify remote SHA-256, atomically rename, and chmod executable. Delete
/// mismatched temporary files so partial data never appears installed. This integrity flow is transport-neutral.
pub fn push_binary(
    t: &dyn SshTransport,
    sys: &RemoteSystem,
    version: &str,
    local_bin: &Path,
    expected_sha256: &str,
    progress: Progress,
) -> Result<(), String> {
    if !valid_version(version) {
        return Err(format!("Invalid version: {version}"));
    }
    if !local_bin.is_file() {
        return Err(format!("local binary not found: {}", local_bin.display()));
    }
    ensure_remote_layout(t, sys, version)?;

    // Upload paths are home-relative and bypass shell expansion. Forward slashes are correct for both
    // sftp-server implementations, including the Windows one.
    let tmp_rel = format!(".velaterm/versions/{version}/.vela-server.tmp");
    let tmp_abs = match sys.shell {
        RemoteShell::Posix => format!("$HOME/{tmp_rel}"),
        RemoteShell::Powershell => format!(
            "$env:USERPROFILE\\.velaterm\\versions\\{version}\\.vela-server.tmp"
        ),
    };

    // The transport owns byte transfer and transfer progress.
    t.upload(local_bin, &tmp_rel, progress)?;

    // Verify the temporary file remotely.
    let got = remote_sha256(t, sys, &tmp_abs)?;
    if !got.eq_ignore_ascii_case(expected_sha256) {
        let _ = shell_exec(t, sys, &remove_file_cmd(sys, &tmp_abs));
        return Err(format!(
            "remote hash mismatch (refusing to place): expected {expected_sha256}, got {got}"
        ));
    }

    // Atomically install; only POSIX needs an executable bit.
    let final_abs = sys.bin_path(version);
    let install = match sys.shell {
        RemoteShell::Posix => format!("mv -f \"{tmp_abs}\" \"{final_abs}\" && chmod +x \"{final_abs}\""),
        RemoteShell::Powershell => format!(
            "$ErrorActionPreference='Stop'; Move-Item -Force -LiteralPath \"{tmp_abs}\" -Destination \"{final_abs}\""
        ),
    };
    shell_exec(t, sys, &install)?;
    Ok(())
}

/// Delete one remote file, ignoring a missing path, in the detected shell's own syntax.
fn remove_file_cmd(sys: &RemoteSystem, path: &str) -> String {
    match sys.shell {
        RemoteShell::Posix => format!("rm -f \"{path}\""),
        RemoteShell::Powershell => {
            format!("Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath \"{path}\"")
        }
    }
}

/// Verify the remote binary with --version, briefly retrying ETXTBSY (POSIX) or a still-held file handle
/// (Windows) right after the upload closes.
fn wait_remote_executable(
    t: &dyn SshTransport,
    sys: &RemoteSystem,
    version: &str,
) -> Result<(), String> {
    let bin = sys.bin_path(version);
    let cmd = match sys.shell {
        RemoteShell::Posix => format!("\"{bin}\" --version"),
        // A native command's nonzero exit is not a PowerShell error, so surface it explicitly.
        RemoteShell::Powershell => format!(
            "$ErrorActionPreference='Stop'; & \"{bin}\" --version; if($LASTEXITCODE -ne 0){{ exit $LASTEXITCODE }}"
        ),
    };
    let mut last = String::new();
    for _ in 0..8 {
        match shell_exec(t, sys, &cmd) {
            Ok(_) => return Ok(()),
            Err(e) => last = e,
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err(format!(
        "remote vela-server never became executable (ETXTBSY?): {last}"
    ))
}

// ─────────────────────────── Persistent serve and port forwarding ───────────────────────────

/// Per-session registry of persistent serve/forwarding children, terminated by stop_serve.
static SERVE_PROCS: OnceLock<Mutex<HashMap<String, Child>>> = OnceLock::new();

/// Deterministically derive the remote loopback port from session ID via FNV-1a in 40000..=59999.
fn remote_serve_port(session: &str) -> u16 {
    let mut h: u32 = 2166136261;
    for b in session.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    40000 + (h % 20000) as u16
}

/// Obtain a free local port by binding :0 then releasing it, accepting the small use race.
fn free_local_port() -> Result<u16, String> {
    let l = std::net::TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| format!("failed to bind local port: {e}"))?;
    l.local_addr()
        .map(|a| a.port())
        .map_err(|e| format!("failed to get local port: {e}"))
}

/// Deterministically derive a host-local port via FNV-1a in 20000..=39999.
fn stable_local_port(host: &str) -> u16 {
    let mut h: u32 = 2166136261;
    for b in host.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    20000 + (h % 20000) as u16
}

/// Prefer a stable host-derived forwarding port so window origin/localStorage layout persists across
/// reconnects. Fall back to a random port when occupied.
pub(crate) fn pick_local_port(host: &str) -> Result<u16, String> {
    let p = stable_local_port(host);
    if std::net::TcpListener::bind(("127.0.0.1", p)).is_ok() {
        return Ok(p);
    }
    free_local_port()
}

/// Remote state persisted in run.json for reconnect reuse/reclamation.
#[derive(Debug, Clone, serde::Deserialize)]
struct RunState {
    pid: u32,
    port: u16,
    password: String,
    version: String,
    /// Whether the service reuses the desktop database. Reconnect requires the same data mode; legacy
    /// run.json defaults false for an isolated database.
    #[serde(default)]
    shared_db: bool,
    /// Whether the service runs with UI mirror mode on. Reconnect requires the same mode; legacy run.json
    /// defaults false, which matches the checkbox shipping unchecked.
    #[serde(default)]
    mirror: bool,
}

/// Data-directory expressions by mode/OS/shell, returned as `(create, argument)`.
///
/// Both halves normally hold the same text. They differ only for Windows reached through a POSIX shell
/// (Cygwin/MSYS): the shell creates the directory under its own POSIX path, while `vela-server.exe` is a
/// native Windows program that understands only `C:\…`, so the argument runs through `cygpath -w`.
///
/// Isolated mode uses `~/.velaterm/data`; shared mode explicitly targets the remote desktop release
/// directory, which the server's own identifier would never resolve to:
///   - Linux: `${XDG_DATA_HOME:-$HOME/.local/share}/io.vlinx.vlxterm.release`
///   - macOS: `$HOME/Library/Application Support/io.vlinx.vlxterm.release`
///   - Windows: `%APPDATA%\io.vlinx.vlxterm.release`
fn serve_data_dir_expr(os: &str, shell: RemoteShell, shared_db: bool) -> (String, String) {
    let same = |s: &str| (s.to_string(), s.to_string());
    match (shell, os, shared_db) {
        (RemoteShell::Powershell, _, false) => same("$env:USERPROFILE\\.velaterm\\data"),
        (RemoteShell::Powershell, _, true) => same("$env:APPDATA\\io.vlinx.vlxterm.release"),
        (RemoteShell::Posix, "windows", false) => (
            "$HOME/.velaterm/data".to_string(),
            "$(cygpath -w \"$HOME/.velaterm/data\")".to_string(),
        ),
        (RemoteShell::Posix, "windows", true) => (
            // %APPDATA% is already a Windows path here, so create it through cygpath and pass it verbatim.
            "$(cygpath -u \"$APPDATA\")/io.vlinx.vlxterm.release".to_string(),
            "$APPDATA\\io.vlinx.vlxterm.release".to_string(),
        ),
        (RemoteShell::Posix, _, false) => same("$HOME/.velaterm/data"),
        (RemoteShell::Posix, "macos", true) => {
            same("$HOME/Library/Application Support/io.vlinx.vlxterm.release")
        }
        // Linux and other Unix fallback.
        (RemoteShell::Posix, _, true) => {
            same("${XDG_DATA_HOME:-$HOME/.local/share}/io.vlinx.vlxterm.release")
        }
    }
}

/// Shell-native path of the remote desktop release data directory, for file probes run by the shell
/// itself (as opposed to the argument handed to `vela-server`, see `serve_data_dir_expr`).
fn desktop_data_dir_expr(sys: &RemoteSystem) -> String {
    serve_data_dir_expr(&sys.os, sys.shell, true).0
}

/// Join a file name onto a shell-native directory expression with that shell's separator.
fn shell_join(sys: &RemoteSystem, dir: &str, name: &str) -> String {
    match sys.shell {
        RemoteShell::Posix => format!("{dir}/{name}"),
        RemoteShell::Powershell => format!("{dir}\\{name}"),
    }
}

/// Remote command that prints a file's content, or nothing when it does not exist.
fn read_file_cmd(sys: &RemoteSystem, path: &str) -> String {
    match sys.shell {
        RemoteShell::Posix => format!("cat \"{path}\" 2>/dev/null"),
        RemoteShell::Powershell => format!(
            "$ErrorActionPreference='SilentlyContinue'; Get-Content -Raw -LiteralPath \"{path}\""
        ),
    }
}

/// Remote command that prints `alive` when `pid` is a running process of the given kind. Windows reuses
/// PIDs quickly, so there the process name must also match one of `names` (case-insensitive prefixes).
fn pid_alive_cmd(sys: &RemoteSystem, pid: u32, names: &[&str]) -> String {
    match sys.shell {
        RemoteShell::Posix => format!("kill -0 {pid} 2>/dev/null && echo alive"),
        RemoteShell::Powershell => {
            let cond = names
                .iter()
                .map(|n| format!("$q.ProcessName -like '{n}*'"))
                .collect::<Vec<_>>()
                .join(" -or ");
            format!(
                "$ErrorActionPreference='SilentlyContinue'; \
                 $q=Get-Process -Id {pid} -ErrorAction SilentlyContinue; \
                 if($q -and ({cond})){{ Write-Output 'alive' }}"
            )
        }
    }
}

/// Read the remote desktop app's local link (see `web::local_link`) and confirm its process is alive.
///
/// None when the desktop app is not installed, not running, or too old to publish a link; the caller
/// then falls back to a headless service. The forwarded port is health-checked afterwards, which also
/// covers a stale record whose PID happens to be reused by an unrelated process on POSIX.
fn read_local_link(t: &dyn SshTransport, sys: &RemoteSystem) -> Option<crate::web::local_link::LocalLink> {
    let path = shell_join(sys, &desktop_data_dir_expr(sys), crate::web::local_link::FILENAME);
    let out = shell_exec(t, sys, &read_file_cmd(sys, &path)).ok()?;
    let link = crate::web::local_link::parse(&out)?;
    let alive = shell_exec(t, sys, &pid_alive_cmd(sys, link.pid, &["velaterm", "vlx-term"]))
        .map(|o| o.trim() == "alive")
        .unwrap_or(false);
    if alive {
        Some(link)
    } else {
        None
    }
}

/// Remote command that prints `yes` when the desktop release database exists.
fn desktop_db_probe_cmd(sys: &RemoteSystem) -> String {
    let db = shell_join(sys, &desktop_data_dir_expr(sys), "vlx-term.db");
    match sys.shell {
        RemoteShell::Posix => format!("test -f \"{db}\" && echo yes"),
        RemoteShell::Powershell => format!(
            "$ErrorActionPreference='SilentlyContinue'; \
             if(Test-Path -LiteralPath \"{db}\" -PathType Leaf){{ Write-Output 'yes' }}"
        ),
    }
}

/// Whether the remote desktop app has a database to share. Mirror mode reuses it when present so the
/// session tree matches the desktop's; otherwise the service falls back to its isolated database.
fn desktop_db_exists(t: &dyn SshTransport, sys: &RemoteSystem) -> bool {
    shell_exec(t, sys, &desktop_db_probe_cmd(sys))
        .map(|o| o.trim() == "yes")
        .unwrap_or(false)
}

/// Start detached remote `vela-server --serve --local-http` and persist pid/port/password/version/shared_db/
/// mirror in run.json so the service survives client disconnect and can be reused consistently.
///
/// Pass the alphanumeric random password through VELA_SERVE_PASSWORD, never remote process argv.
fn start_detached_serve(
    t: &dyn SshTransport,
    sys: &RemoteSystem,
    version: &str,
    password: &str,
    rport: u16,
    shared_db: bool,
    mirror: bool,
) -> Result<u32, String> {
    if !valid_version(version) {
        return Err(format!("Invalid version: {version}"));
    }
    if password.is_empty() || !password.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("serve password must be a non-empty alphanumeric random string".to_string());
    }
    let remote_cmd = serve_script(sys, version, password, rport, shared_db, mirror);
    let out = shell_exec(t, sys, &remote_cmd)?;
    out.lines()
        .rev()
        .find_map(|l| l.trim().parse::<u32>().ok())
        .ok_or_else(|| format!("remote serve start returned no valid PID: {out:?}"))
}

/// Build the remote script that starts the detached service and writes run.json, echoing its PID last.
/// Kept separate from execution so its exact shape is unit-testable.
fn serve_script(
    sys: &RemoteSystem,
    version: &str,
    password: &str,
    rport: u16,
    shared_db: bool,
    mirror: bool,
) -> String {
    let bin = sys.bin_path(version);
    let run = sys.run_json();
    let (data_dir_mk, data_dir_arg) = serve_data_dir_expr(&sys.os, sys.shell, shared_db);
    // Embed shared_db and mirror directly as JSON true/false literals.
    let shared_json = if shared_db { "true" } else { "false" };
    let mirror_json = if mirror { "true" } else { "false" };
    // The remote machine is headless and has no panel, so the mirror choice travels as a startup flag.
    let mirror_arg = if mirror { "1" } else { "0" };
    match sys.shell {
        // nohup, closed stdin, redirected logs, and `&` detach from SSH. Persist `$!` state under
        // ~/.velaterm regardless of the selected data directory, then echo the PID.
        RemoteShell::Posix => format!(
            "mkdir -p \"{data_dir_mk}\"; \
             nohup env VELA_SERVE_PASSWORD='{password}' \"{bin}\" --serve --local-http --port {rport} \
               --data-dir \"{data_dir_arg}\" --mirror {mirror_arg} </dev/null >\"$HOME/.velaterm/server.log\" 2>&1 & \
             P=$!; \
             printf '{{\"pid\":%d,\"port\":%d,\"password\":\"%s\",\"version\":\"%s\",\"shared_db\":{shared_json},\"mirror\":{mirror_json}}}\\n' \"$P\" {rport} '{password}' '{version}' > \"{run}\"; \
             echo \"$P\""
        ),
        // Windows starts the service in two stages, because `nohup … &` has two separate Windows problems
        // to solve and no single Start-Process call solves either one.
        //
        // 1. **The SSH session kills everything it spawned.** Microsoft's OpenSSH puts the session in a job
        //    object and terminates it on disconnect, and job membership is inherited, so every descendant
        //    dies with the session however it was launched. (Measured: the server wrote its startup log,
        //    then vanished the moment the SSH command returned.)
        // 2. **Redirection turns on handle inheritance.** `Start-Process` with any `-RedirectStandard*`
        //    creates the process with inheritance enabled, so the server receives every other inheritable
        //    handle the login shell holds — the SSH channel's pipes among them — and keeps that channel
        //    open for its whole life. (Measured: the client hung on a command that had already finished.)
        //
        // Stage one therefore does not spawn the service itself: it asks WMI to, through
        // `Win32_Process.Create`. The new process is created by the WMI service rather than by us, so it
        // belongs to no job of ours and inherits none of our handles, while still running under our own
        // token — `%USERPROFILE%` and `%APPDATA%` still resolve to this user's directories. It lands in
        // session 0 with no console, which is exactly right for a headless server.
        //
        // That WMI-created PowerShell is stage two: free of both problems, it starts the server with its
        // log redirections and writes run.json. Stage one waits for run.json to appear and reports the PID
        // recorded in it, so the returned PID is the server's own rather than any intermediate process's.
        //
        // The password is set on stage two's environment and inherited by the server, never placed in the
        // server's argv. Windows PowerShell 5.1 does not quote array elements of -ArgumentList, so the
        // server arguments travel as one prebuilt string; a user profile path containing spaces would
        // otherwise split the --data-dir value.
        RemoteShell::Powershell => {
            let inner = format!(
                "$ErrorActionPreference='Stop'; \
                 $b=\"$env:USERPROFILE\\.velaterm\"; \
                 $d=\"{data_dir_arg}\"; \
                 New-Item -ItemType Directory -Force -Path $d | Out-Null; \
                 $env:VELA_SERVE_PASSWORD='{password}'; \
                 $al='--serve --local-http --port {rport} --data-dir \"'+$d+'\" --mirror {mirror_arg}'; \
                 $p=Start-Process -FilePath \"{bin}\" -ArgumentList $al -WindowStyle Hidden -PassThru \
                     -RedirectStandardOutput \"$b\\server.log\" -RedirectStandardError \"$b\\server.err.log\"; \
                 $j='{{\"pid\":'+$p.Id+',\"port\":{rport},\"password\":\"{password}\",\"version\":\"{version}\",\"shared_db\":{shared_json},\"mirror\":{mirror_json}}}'; \
                 Set-Content -LiteralPath \"{run}\" -Value $j -Encoding ASCII"
            );
            let inner_b64 = ps_b64(&inner);
            format!(
                "$ErrorActionPreference='Stop'; \
                 $run=\"{run}\"; \
                 Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath $run; \
                 $r=Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{{ \
                     CommandLine = 'powershell -NoProfile -NonInteractive -EncodedCommand {inner_b64}' }}; \
                 if($r.ReturnValue -ne 0){{ throw ('could not spawn the service host (Win32_Process.Create returned ' + $r.ReturnValue + ')') }}; \
                 for($i=0;$i -lt 200;$i++){{ if(Test-Path -LiteralPath $run){{ break }}; Start-Sleep -Milliseconds 100 }}; \
                 $t=Get-Content -Raw -LiteralPath $run; \
                 if(-not $t){{ throw 'the remote service did not start (see .velaterm\\server.err.log)' }}; \
                 Write-Output ($t | ConvertFrom-Json).pid"
            )
        }
    }
}

/// Start persistent `ssh -N -L` forwarding independently from the remote service. Register the child,
/// return only after health passes, and let stop_serve terminate forwarding without the remote service.
fn openssh_open_forward(host: &str, session: &str, rport: u16) -> Result<u16, String> {
    let (target, port) = split_ssh_target(host);
    let sock = control_path(session);
    // A stable port preserves the remote window origin and its localStorage layout across reconnects.
    let lport = pick_local_port(host)?;

    let mut cmd = ssh_command("ssh");
    cmd.args(ssh_common_args(&sock, port, true));
    cmd.args(["-o", "BatchMode=yes"]);
    cmd.args(["-o", "ExitOnForwardFailure=yes"]);
    cmd.args(["-N", "-L", &format!("{lport}:127.0.0.1:{rport}")]);
    cmd.arg(target);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    let child = cmd
        .spawn()
        .map_err(|e| format!("failed to start port forward (ssh missing locally?): {e}"))?;
    SERVE_PROCS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .insert(session.to_string(), child);

    if let Err(e) = wait_port_healthy(lport, Duration::from_secs(20)) {
        stop_serve(session);
        return Err(e);
    }
    Ok(lport)
}

/// Read run.json and check the recorded PID to find a live remote service. Remove stale state and return
/// None otherwise.
fn detect_running(t: &dyn SshTransport, sys: &RemoteSystem) -> Option<RunState> {
    let run = sys.run_json();
    let out = shell_exec(t, sys, &read_file_cmd(sys, &run)).ok()?;
    if out.trim().is_empty() {
        return None;
    }
    let rs: RunState = serde_json::from_str(out.trim()).ok()?;
    // Defensively validate state fields before interpolating them into remote commands.
    if !valid_version(&rs.version) || !rs.password.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    // Windows reuses PIDs quickly, so there the process must actually be vela-server.
    let alive = shell_exec(t, sys, &pid_alive_cmd(sys, rs.pid, &["vela-server"]))
        .map(|o| o.trim() == "alive")
        .unwrap_or(false);
    if alive {
        Some(rs)
    } else {
        let _ = shell_exec(t, sys, &remove_file_cmd(sys, &run));
        None
    }
}

/// Shared remote command that extracts the PID from run.json, kills it, and removes the state file.
fn kill_service_cmd(sys: &RemoteSystem) -> String {
    let run = sys.run_json();
    match sys.shell {
        RemoteShell::Posix => format!(
            "P=$(sed -n 's/.*\"pid\":\\([0-9]\\{{1,\\}}\\).*/\\1/p' \"{run}\" 2>/dev/null); \
             [ -n \"$P\" ] && kill \"$P\" 2>/dev/null; rm -f \"{run}\""
        ),
        RemoteShell::Powershell => format!(
            "$ErrorActionPreference='SilentlyContinue'; \
             $t=Get-Content -Raw -LiteralPath \"{run}\"; \
             if($t){{ $o=$t | ConvertFrom-Json; \
               if($o.pid){{ Stop-Process -Id $o.pid -Force -ErrorAction SilentlyContinue }} }}; \
             Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath \"{run}\""
        ),
    }
}

/// Explicitly stop the persistent remote service after the user chooses close. This direct entry is
/// OpenSSH-only; active transports use the same command through exec.
pub fn kill_remote_service(host: &str, session: &str) {
    let sys = session_sys(session);
    let cmd = kill_service_cmd(&sys);
    let wrapped = match sys.shell {
        RemoteShell::Posix => cmd,
        RemoteShell::Powershell => ps_encoded(&cmd),
    };
    let _ = run_remote(host, session, &wrapped);
}

/// Poll the local forwarding port until the remote service actually responds over HTTP or times out.
///
/// TCP alone is insufficient because ssh -L listens before the remote service is ready. Send a real HTTP
/// request; any status, including 401/404, proves serving and prevents false-positive readiness.
pub(crate) fn wait_port_healthy(port: u16, timeout: Duration) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{port}/");
    let deadline = Instant::now() + timeout;
    loop {
        match ureq::get(&url).timeout(Duration::from_secs(3)).call() {
            Ok(_) => return Ok(()),
            // Any HTTP status means the remote service is serving.
            Err(ureq::Error::Status(_, _)) => return Ok(()),
            // Connection/transport failure means startup or forwarding is not ready; retry.
            Err(_) => {}
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "serve health check timed out (no HTTP response on local {port})"
            ));
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// Idempotently terminate only a session's local forwarding child; use kill_remote_service separately.
pub fn stop_serve(session: &str) {
    if let Some(map) = SERVE_PROCS.get() {
        if let Some(mut child) = map.lock().unwrap().remove(session) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

// ─────────────────────────── SSH transport abstraction for OpenSSH/russh ───────────────────────────

/// Authentication intent built by the command layer and implemented by the transport.
pub enum SshAuth {
    /// Agent/default-key authentication.
    Auto,
    /// Password authentication; OpenSSH uses a PTY and Windows OpenSSH does not support this flow.
    Password(String),
}

/// Established SSH transport encapsulating backend state. Orchestration depends only on this trait, so
/// switching a platform between OpenSSH and russh changes only connect_transport selection.
///
/// Send is required because the global registry crosses blocking threads; dropping russh state disconnects it.
pub trait SshTransport: Send {
    /// Run a remote command and return trimmed stdout.
    fn exec(&self, cmd: &str) -> Result<String, String>;
    /// Upload to a home-relative path with transfer progress.
    fn upload(&self, local: &Path, remote_rel: &str, progress: Progress) -> Result<(), String>;
    /// Start persistent local forwarding and return lport after health succeeds.
    fn open_forward(&self, rport: u16) -> Result<u16, String>;
    /// Disconnect forwarding and connection resources without stopping the remote service.
    fn close(&self);
    /// Session ID used for port derivation, ConnectResult, and registry keys.
    fn session(&self) -> &str;
}

/// OpenSSH transport using external ssh/scp. Unix reuses ControlMaster; Windows reconnects per operation.
struct OpensshTransport {
    host: String,
    session: String,
}

impl OpensshTransport {
    /// Authenticate/establish the connection after trust_host has populated known_hosts.
    fn connect(
        host: &str,
        session: &str,
        auth: SshAuth,
        _progress: Progress,
    ) -> Result<Self, String> {
        match auth {
            // Mark rejected key auth so the frontend can prompt for password retry.
            SshAuth::Auto => {
                if let Err(e) = open_master(host, session, true) {
                    if e.contains("Permission denied") {
                        return Err(format!("{AUTH_REQUIRED_TAG}{e}"));
                    }
                    return Err(e);
                }
            }
            SshAuth::Password(pw) => {
                // Windows OpenSSH cannot complete password orchestration without a reusable master; fail
                // early with clear guidance rather than accepting a password and failing later in BatchMode.
                #[cfg(windows)]
                {
                    let _ = pw;
                    return Err(
                        "SSH password authentication is not supported on Windows yet. \
                        Connect with an SSH key or ssh-agent (public-key auth) instead, \
                        or open this remote from a macOS/Linux machine."
                            .to_string(),
                    );
                }
                #[cfg(not(windows))]
                open_master_pty(host, session, true, &pw)?;
            }
        }
        Ok(OpensshTransport {
            host: host.to_string(),
            session: session.to_string(),
        })
    }
}

impl SshTransport for OpensshTransport {
    fn exec(&self, cmd: &str) -> Result<String, String> {
        run_remote(&self.host, &self.session, cmd)
    }

    fn upload(&self, local: &Path, remote_rel: &str, progress: Progress) -> Result<(), String> {
        let (target, port) = split_ssh_target(&self.host);
        // The SCP path after `:` is home-relative and does not use shell expansion. The size probe below
        // runs through the login shell instead, so it needs that shell's own path syntax.
        let sys = session_sys(&self.session);
        let tmp_abs = match sys.shell {
            RemoteShell::Posix => format!("$HOME/{remote_rel}"),
            RemoteShell::Powershell => {
                format!("$env:USERPROFILE\\{}", remote_rel.replace('/', "\\"))
            }
        };
        let size_cmd = match sys.shell {
            // Use Linux stat syntax then macOS/BSD fallback; report zero if neither works.
            RemoteShell::Posix => format!(
                "stat -c %s \"{tmp_abs}\" 2>/dev/null || stat -f %z \"{tmp_abs}\" 2>/dev/null || echo 0"
            ),
            RemoteShell::Powershell => ps_encoded(&format!(
                "$ErrorActionPreference='SilentlyContinue'; \
                 $i=Get-Item -LiteralPath \"{tmp_abs}\"; \
                 if($i){{ Write-Output $i.Length }} else {{ Write-Output 0 }}"
            )),
        };
        let total = std::fs::metadata(local).map(|m| m.len()).unwrap_or(0);

        let mut scp = ssh_command("scp");
        // Unix reuses the authenticated master. Windows connects directly with trusted known_hosts and
        // BatchMode so key/agent failure returns immediately without prompting.
        #[cfg(not(windows))]
        scp.args([
            "-o",
            &format!("ControlPath={}", control_path(&self.session).display()),
        ]);
        #[cfg(windows)]
        scp.args([
            "-o",
            "StrictHostKeyChecking=yes",
            "-o",
            "ConnectTimeout=15",
            "-o",
            "BatchMode=yes",
        ]);
        if let Some(p) = port {
            scp.args(["-P", p]); // SCP uses uppercase -P for port, unlike SSH's lowercase -p.
        }
        scp.arg(local);
        scp.arg(format!("{target}:{remote_rel}"));
        scp.stdin(Stdio::null());
        scp.stdout(Stdio::null());
        scp.stderr(Stdio::null());
        let mut child = scp
            .spawn()
            .map_err(|e| format!("scp vela-server failed to run (ssh missing locally?): {e}"))?;

        // While SCP runs, poll remote temporary-file size and emit transfer percentage until exit.
        progress("transfer", Some(0));
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        return Err("scp vela-server failed".to_string());
                    }
                    break;
                }
                Ok(None) => {}
                Err(e) => return Err(format!("scp wait failed: {e}")),
            }
            if total > 0 {
                if let Ok(out) = run_remote(&self.host, &self.session, &size_cmd) {
                    if let Ok(cur) = out.trim().parse::<u64>() {
                        let pct = (cur.min(total) * 99 / total) as u8;
                        progress("transfer", Some(pct));
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(400));
        }
        progress("transfer", Some(100));
        Ok(())
    }

    fn open_forward(&self, rport: u16) -> Result<u16, String> {
        openssh_open_forward(&self.host, &self.session, rport)
    }

    fn close(&self) {
        stop_serve(&self.session);
        close_master(&self.host, &self.session);
    }

    fn session(&self) -> &str {
        &self.session
    }
}

/// Select and establish a transport by platform and VLX_SSH_BACKEND override.
fn connect_transport(
    host: &str,
    session: &str,
    auth: SshAuth,
    progress: Progress,
) -> Result<Box<dyn SshTransport>, String> {
    // Default Windows to russh because OpenSSH multiplexing/password flows fail there; other platforms
    // keep OpenSSH config/agent/ProxyJump integration. Environment override supports debugging/fallback.
    let use_russh = match std::env::var("VLX_SSH_BACKEND").ok().as_deref() {
        Some("russh") => true,
        Some("openssh") => false,
        _ => cfg!(windows),
    };
    if use_russh {
        Ok(Box::new(crate::ssh_russh::RusshTransport::connect(
            host, session, auth, progress,
        )?))
    } else {
        Ok(Box::new(OpensshTransport::connect(
            host, session, auth, progress,
        )?))
    }
}

/// Active transport registry keyed by session. russh **must** remain registered to keep its connection/
/// forwarding alive; OpenSSH is registered for a unified disconnect path.
static TRANSPORTS: OnceLock<Mutex<HashMap<String, Box<dyn SshTransport>>>> = OnceLock::new();

fn transports() -> &'static Mutex<HashMap<String, Box<dyn SshTransport>>> {
    TRANSPORTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Unified disconnect prefers the active registry; missing state falls back to reconstructed OpenSSH.
/// kill_remote stops the service/all sessions before forwarding and connection teardown.
pub fn disconnect(host: &str, session: &str, kill_remote: bool) {
    // Remove the monitor first so it cannot rebuild during disconnect.
    unwatch_tunnel(session);
    // The remote desktop app is never ours to stop: a session attached through its local link only
    // drops the tunnel, whatever the caller asked for.
    let kill_remote = kill_remote && session_target(session) == ServiceTarget::Headless;
    // Remove the object under lock, then perform network I/O without blocking the registry.
    let removed = transports().lock().unwrap().remove(session);
    if let Some(t) = removed {
        if kill_remote {
            let sys = session_sys(session);
            let _ = shell_exec(t.as_ref(), &sys, &kill_service_cmd(&sys));
        }
        t.close();
        clear_session_sys(session);
        clear_session_target(session);
        return;
    }
    // Fallback for missing active state, applicable only to OpenSSH.
    if kill_remote {
        kill_remote_service(host, session);
    }
    stop_serve(session);
    close_master(host, session);
    clear_session_sys(session);
    clear_session_target(session);
}

// ─────────────────────────── Tunnel monitoring and automatic rebuild ───────────────────────────
//
// WebSocket retries cannot recover after an `ssh -N -L` process truly dies. Each OpenSSH connection
// therefore monitors its tunnel child and local port. A dead child alone is insufficient because the
// master may still own a healthy forwarding listener; rebuild only when the port is also dead. Recreate
// authentication, verify the persistent remote service, and reopen the **same local port** so origin and
// WebSocket recovery remain transparent. Debounce warnings until the first failure, back off with a cap,
// and wait for a manual kick after giving up. russh uses in-process forwarding and has no registered child.

/// Tunnel-monitor control: wanted keeps it alive; kick requests a manual rebuild.
struct TunnelCtl {
    wanted: AtomicBool,
    kick: AtomicBool,
}

/// Session-to-control registry used by monitor threads, disconnect, and manual kicks.
static TUNNEL_CTLS: OnceLock<Mutex<HashMap<String, Arc<TunnelCtl>>>> = OnceLock::new();

fn tunnel_ctls() -> &'static Mutex<HashMap<String, Arc<TunnelCtl>>> {
    TUNNEL_CTLS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Request one rebuild. Healthy tunnels discard the kick; missing sessions are no-ops.
pub fn kick_tunnel(session: &str) {
    if let Some(ctl) = tunnel_ctls().lock().unwrap().get(session) {
        ctl.kick.store(true, Ordering::SeqCst);
    }
}

/// Idempotently stop/remove a tunnel monitor; its thread exits on the next tick.
fn unwatch_tunnel(session: &str) {
    if let Some(ctl) = tunnel_ctls().lock().unwrap().remove(session) {
        ctl.wanted.store(false, Ordering::SeqCst);
    }
}

/// Rebuild failure classification: retryable after backoff or unrecoverable/manual intervention.
enum RebuildErr {
    Retryable(String),
    Fatal(String),
}

/// Rebuild master, verify the service, and reopen forwarding on the original port.
///
/// Unrecoverable when password input is required without a remembered password, the remote service died
/// (new credentials/sessions prevent transparent recovery), or another process occupies the original port.
fn rebuild_tunnel(
    host: &str,
    session: &str,
    identifier: &str,
    want_port: u16,
) -> Result<(), RebuildErr> {
    // 1. Clear stale master sockets before rebuilding. Otherwise OpenSSH disables multiplexing and falls
    // back to a bare connection that cannot support later password-authenticated forwarding.
    close_master(host, session);
    // Fall back to the keyring password through PTY auth after key rejection.
    if let Err(e) = open_master(host, session, true) {
        if !e.contains("Permission denied") {
            return Err(RebuildErr::Retryable(e));
        }
        let Some(pw) = load_password(identifier, "ssh", host) else {
            return Err(RebuildErr::Fatal(format!(
                "auth required and no saved password: {e}"
            )));
        };
        // A rejected remembered password is stale; retrying cannot help.
        open_master_pty(host, session, true, &pw).map_err(RebuildErr::Fatal)?;
    }
    // 2. Verify the remote service is still there: run.json plus a liveness check for a headless
    // service, the local-link record for the remote desktop app.
    let t = OpensshTransport {
        host: host.to_string(),
        session: session.to_string(),
    };
    let sys = session_sys(session);
    let rport = match session_target(session) {
        ServiceTarget::Headless => detect_running(&t, &sys).map(|rs| rs.port).ok_or_else(|| {
            RebuildErr::Fatal("remote server is no longer running".to_string())
        })?,
        // The desktop may have restarted on a new port with a new password, which the window's stored
        // credentials cannot follow; only the same record is transparently recoverable.
        ServiceTarget::DesktopLink => read_local_link(&t, &sys).map(|l| l.port).ok_or_else(|| {
            RebuildErr::Fatal("remote desktop app is no longer running".to_string())
        })?,
    };
    // 3. Reap the dead tunnel child and rebuild on the original port.
    stop_serve(session);
    let lport = openssh_open_forward(host, session, rport).map_err(RebuildErr::Retryable)?;
    if lport != want_port {
        // If another process owns the stable port, destroy the useless new forwarding child.
        stop_serve(session);
        return Err(RebuildErr::Fatal(format!(
            "stable local port {want_port} is taken (forward landed on {lport})"
        )));
    }
    Ok(())
}

/// Probe whether the local forwarding port still has a listener. ControlMaster owns the listener, so an
/// exited `ssh -N -L` requester may leave a fully healthy tunnel; process exit alone would be a false alarm.
fn tunnel_port_alive(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_secs(2),
    )
    .is_ok()
}

/// Interruptible sleep in 200 ms steps, returning false immediately when no longer wanted.
fn sleep_wanted(ctl: &TunnelCtl, total: Duration) -> bool {
    let deadline = Instant::now() + total;
    while Instant::now() < deadline {
        if !ctl.wanted.load(Ordering::SeqCst) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    ctl.wanted.load(Ordering::SeqCst)
}

/// Start a monitor for the original local port. Notify reconnecting only after the first failed attempt,
/// up on recovery, and down after automatic retries give up.
pub fn watch_tunnel(
    host: String,
    session: String,
    identifier: String,
    local_port: u16,
    notify: impl Fn(&str) + Send + 'static,
) {
    let ctl = Arc::new(TunnelCtl {
        wanted: AtomicBool::new(true),
        kick: AtomicBool::new(false),
    });
    tunnel_ctls()
        .lock()
        .unwrap()
        .insert(session.clone(), Arc::clone(&ctl));

    std::thread::spawn(move || {
        // Rebuild immediately, then after 2/4/8/16 seconds: five attempts over roughly half a minute.
        const BACKOFF_SECS: [u64; 5] = [0, 2, 4, 8, 16];
        loop {
            // ── Healthy phase: check child and port every three seconds ──
            if !sleep_wanted(&ctl, Duration::from_secs(3)) {
                return;
            }
            let child_exited = {
                let Some(map) = SERVE_PROCS.get() else { return };
                let mut map = map.lock().unwrap();
                match map.get_mut(&session) {
                    // No child means russh forwarding or disconnected cleanup; stop monitoring.
                    None => return,
                    Some(child) => !matches!(child.try_wait(), Ok(None)),
                }
            };
            if !child_exited {
                // A live child implies a live master/tunnel. Discard accumulated manual kicks.
                ctl.kick.store(false, Ordering::SeqCst);
                continue;
            }
            // After child exit, keep a listening port untouched unless the user explicitly kicks; another
            // process could create a false-positive listener, and explicit user intent is stronger evidence.
            if tunnel_port_alive(local_port) && !ctl.kick.swap(false, Ordering::SeqCst) {
                continue;
            }

            // ── Dead child and port: bounded automatic rebuild with backoff ──
            eprintln!("[ssh] tunnel for {host} ({session}) died; rebuilding…");
            let mut recovered = false;
            let mut notified = false;
            for (i, secs) in BACKOFF_SECS.iter().enumerate() {
                if !sleep_wanted(&ctl, Duration::from_secs(*secs)) {
                    return;
                }
                match rebuild_tunnel(&host, &session, &identifier, local_port) {
                    Ok(()) => {
                        eprintln!("[ssh] tunnel for {host} rebuilt on port {local_port}");
                        recovered = true;
                        break;
                    }
                    Err(RebuildErr::Fatal(e)) => {
                        eprintln!("[ssh] tunnel rebuild unrecoverable: {e}");
                        break;
                    }
                    Err(RebuildErr::Retryable(e)) => {
                        eprintln!("[ssh] tunnel rebuild attempt {} failed: {e}", i + 1);
                        // Show the red banner only after the immediate attempt fails, avoiding brief flicker.
                        if !notified {
                            notify("reconnecting");
                            notified = true;
                        }
                    }
                }
            }
            if recovered {
                // Always emit up so the frontend replaces a potentially half-open old WebSocket.
                notify("up");
                continue;
            }

            // ── Automatic recovery exhausted: enter down state and await a manual kick ──
            notify("down");
            loop {
                if !sleep_wanted(&ctl, Duration::from_millis(500)) {
                    return;
                }
                if !ctl.kick.swap(false, Ordering::SeqCst) {
                    continue;
                }
                notify("reconnecting");
                match rebuild_tunnel(&host, &session, &identifier, local_port) {
                    Ok(()) => {
                        eprintln!("[ssh] tunnel for {host} rebuilt on port {local_port} (manual)");
                        notify("up");
                        break;
                    }
                    Err(RebuildErr::Fatal(e)) | Err(RebuildErr::Retryable(e)) => {
                        eprintln!("[ssh] manual tunnel rebuild failed: {e}");
                        notify("down");
                    }
                }
            }
        }
    });
}

// ─────────────────────────── Complete connection orchestration ───────────────────────────

/// Successful connection details: session ID, local forwarding port, and random auto-login password.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectResult {
    pub session: String,
    pub local_port: u16,
    pub password: String,
    /// Whether an existing persistent remote service and its sessions were reused.
    pub reused: bool,
    /// Whether the tunnel leads to the remote desktop app itself (its local link) rather than to a
    /// headless service. The window then closes like a URL connection: leave, never stop anything.
    pub desktop_link: bool,
    /// Data mode actually in effect: true when the remote desktop release database is in use, either
    /// through the desktop app or through a headless service opened on it.
    pub shared_db: bool,
}

/// Generate a one-time 64-hex-character password safe for the serve environment.
fn random_password() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// Complete connection except window creation: connect, detect, locally provision/verify, validate or
/// upload remotely, start serve and forwarding, then return ConnectResult.
///
/// Requires confirmed known_hosts trust. Auto uses agent/keys and marks rejection for a password prompt;
/// Password uses the transport's supported implementation.
///
/// Without mirror, shared_db selects isolated ~/.velaterm/data or the remote desktop release database for
/// the headless service this client starts. With mirror the target is chosen for the caller, in this
/// order: the remote desktop app itself when it is running (its local link), else a headless service on
/// the desktop database when that database exists, else a headless service on the isolated database; a
/// headless service then runs with mirror mode on, so its own clients still mirror one another.
pub fn connect(
    app_data_dir: &Path,
    host: &str,
    auth: SshAuth,
    shared_db: bool,
    mirror: bool,
    progress: Progress,
) -> Result<ConnectResult, String> {
    if !valid_ssh_target(host) {
        return Err(format!("Invalid SSH target: {host}"));
    }
    let session = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();

    progress("connect", None);
    // Authenticate and connect through the selected OpenSSH/russh backend.
    let transport = connect_transport(host, &session, auth, progress)?;

    // Register successful transport to keep it alive; on failure stop forwarding and disconnect cleanly.
    match connect_inner(app_data_dir, transport.as_ref(), shared_db, mirror, progress) {
        Ok(r) => {
            transports()
                .lock()
                .unwrap()
                .insert(session.clone(), transport);
            Ok(r)
        }
        Err(e) => {
            transport.close();
            Err(e)
        }
    }
}

fn connect_inner(
    app_data_dir: &Path,
    t: &dyn SshTransport,
    shared_db: bool,
    mirror: bool,
    progress: Progress,
) -> Result<ConnectResult, String> {
    let session = t.session();
    progress("probe", None);
    let sys = probe_system(t)?;
    // Linux, macOS, and Windows are supported; each shell's command differences are handled by the
    // builders above. Anything else (BSD, Solaris, …) has no published vela-server artifact.
    if sys.os != "linux" && sys.os != "macos" && sys.os != "windows" {
        return Err(format!(
            "currently supports Linux, macOS and Windows remotes; detected remote is {} ({})",
            sys.os, sys.raw
        ));
    }
    // Every later step needs the detected shell, including disconnect and tunnel rebuild, which only carry
    // the session key.
    set_session_sys(session, &sys);
    let version = crate::VERSION;

    // Mirror mode means following the remote desktop app. When that app is running, attach to its
    // local link (see `web::local_link`) and skip provisioning entirely: the tunnel then leads into the
    // desktop's own process, so sessions, layout, and presence are the desktop's. A headless service
    // that an earlier connection left behind is left alone; stopping it would end its sessions.
    let mut shared_db = shared_db;
    if mirror {
        progress("probe", None);
        if let Some(link) = read_local_link(t, &sys) {
            set_session_target(session, ServiceTarget::DesktopLink);
            progress("forward", None);
            let local_port = t.open_forward(link.port)?;
            return Ok(ConnectResult {
                session: session.to_string(),
                local_port,
                password: link.password,
                reused: true,
                desktop_link: true,
                shared_db: true,
            });
        }
        // No running desktop: the closest thing to following it is a headless service on its database,
        // which only exists when the desktop app has been used on that machine.
        shared_db = desktop_db_exists(t, &sys);
    }
    set_session_target(session, ServiceTarget::Headless);

    // Detect a persistent service retained from an earlier disconnect or abnormal exit.
    if let Some(rs) = detect_running(t, &sys) {
        // Reuse only when version, data mode, and mirror mode all match; otherwise restart, both to avoid
        // exposing the wrong DB and because mirror mode is fixed at startup for a service with no panel.
        if rs.version == version && rs.shared_db == shared_db && rs.mirror == mirror {
            // Reuse its port/password and add only forwarding, preserving remote sessions.
            progress("forward", None);
            let local_port = t.open_forward(rs.port)?;
            return Ok(ConnectResult {
                session: session.to_string(),
                local_port,
                password: rs.password,
                reused: true,
                desktop_link: false,
                shared_db,
            });
        }
        // Stop mismatched version/mode and restart below with current settings.
        let _ = shell_exec(t, &sys, &kill_service_cmd(&sys));
    }

    progress("supply", None);
    let platkey = crate::server_supply::platform_key(&sys.os, &sys.arch);
    // Use a verified local cache or download/verify from R2. VLX_DEV_SERVER_BIN bypasses publication for
    // development-only end-to-end testing.
    let local_bin = match std::env::var("VLX_DEV_SERVER_BIN") {
        Ok(p) if !p.is_empty() && Path::new(&p).is_file() => {
            eprintln!("[ssh] using dev-bypass vela-server: {p}");
            PathBuf::from(p)
        }
        _ => crate::server_supply::ensure_supplied(app_data_dir, version, &platkey, &|pct| {
            progress("supply", Some(pct))
        })?,
    };
    let sha = crate::server_supply::cached_sha256(&local_bin)?;

    // Skip upload for an integrity-valid remote binary; otherwise verify and atomically install it.
    if !remote_binary_ok(t, &sys, version, &sha) {
        push_binary(t, &sys, version, &local_bin, &sha, progress)?;
    }
    // Retry executable verification around transient ETXTBSY immediately after SCP.
    progress("start", None);
    wait_remote_executable(t, &sys, version)?;

    let password = random_password();
    let rport = remote_serve_port(session);
    start_detached_serve(t, &sys, version, &password, rport, shared_db, mirror)?;
    progress("forward", None);
    let local_port = t.open_forward(rport)?;
    Ok(ConnectResult {
        session: session.to_string(),
        local_port,
        password,
        reused: false,
        desktop_link: false,
        shared_db,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a system value for command-shape tests without touching the network.
    fn sys_of(os: &str, arch: &str, shell: RemoteShell) -> RemoteSystem {
        RemoteSystem {
            os: os.to_string(),
            arch: arch.to_string(),
            raw: String::new(),
            shell,
        }
    }

    #[test]
    fn parse_uname_linux_and_macos() {
        let l = parse_uname("Linux x86_64").unwrap();
        assert_eq!((l.os.as_str(), l.arch.as_str()), ("linux", "x86_64"));
        assert_eq!(l.shell, RemoteShell::Posix);
        let m = parse_uname("Darwin arm64").unwrap();
        assert_eq!((m.os.as_str(), m.arch.as_str()), ("macos", "aarch64"));
        let a = parse_uname("Linux amd64").unwrap();
        assert_eq!(a.arch, "x86_64");
        assert!(parse_uname("").is_none());
        assert!(parse_uname("Plan9").is_none()); // Missing architecture field.
    }

    #[test]
    fn parse_uname_cygwin_is_windows_over_a_posix_shell() {
        // A Cygwin/MSYS sshd answers uname, so Windows is reached with the whole POSIX toolbox available.
        let c = parse_uname("CYGWIN_NT-10.0-19045 x86_64").unwrap();
        assert_eq!((c.os.as_str(), c.arch.as_str()), ("windows", "x86_64"));
        assert_eq!(c.shell, RemoteShell::Posix);
        let m = parse_uname("MINGW64_NT-10.0 x86_64").unwrap();
        assert_eq!(m.os, "windows");
        assert_eq!(m.shell, RemoteShell::Posix);
    }

    #[test]
    fn parse_windows_probe_reads_processor_architecture() {
        let w = parse_windows_probe("Windows_NT AMD64").unwrap();
        assert_eq!((w.os.as_str(), w.arch.as_str()), ("windows", "x86_64"));
        assert_eq!(w.shell, RemoteShell::Powershell);
        let a = parse_windows_probe("Windows_NT ARM64").unwrap();
        assert_eq!(a.arch, "aarch64");
        // An unknown architecture is preserved so the supply step can name it in its error.
        assert_eq!(parse_windows_probe("Windows_NT x86").unwrap().arch, "x86");
        assert!(parse_windows_probe("Linux x86_64").is_none());
        assert!(parse_windows_probe("").is_none());
    }

    #[test]
    fn ps_encoded_is_pure_base64_utf16le() {
        let wrapped = ps_encoded("hi");
        let b64 = wrapped
            .strip_prefix("powershell -NoProfile -NonInteractive -EncodedCommand ")
            .expect("the wrapper should carry the standard PowerShell flags");
        use base64::Engine;
        let raw = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .expect("the argument should be valid base64");
        assert_eq!(raw, vec![b'h', 0, b'i', 0], "PowerShell requires UTF-16LE");
        // No quoting reaches cmd.exe, which is the entire point of -EncodedCommand.
        assert!(!b64.contains(' ') && !b64.contains('"') && !b64.contains('\''));
    }

    #[test]
    fn first_host_key_skips_comments() {
        let scanned = "# host:22 SSH-2.0\n10.0.0.1 ssh-ed25519 AAAAC3NzaC1lZDI1\n";
        let (kt, kb) = first_host_key(scanned).unwrap();
        assert_eq!(kt, "ssh-ed25519");
        assert_eq!(kb, "AAAAC3NzaC1lZDI1");
        assert!(first_host_key("# only comments\n\n").is_none());
    }

    #[test]
    fn sha256_fingerprint_matches_ssh_format() {
        // Use a deterministic small blob to verify the `SHA256:` plus unpadded-base64 shape.
        let fp = sha256_fingerprint("AAAA").unwrap();
        assert!(fp.starts_with("SHA256:"));
        assert!(!fp.ends_with('='), "the fingerprint should be unpadded base64");
    }

    #[test]
    fn valid_version_guards_injection() {
        assert!(valid_version("0.1.73"));
        assert!(valid_version("10.20.30"));
        assert!(!valid_version(""));
        assert!(!valid_version("1.0; rm -rf ~"), "a semicolon should be rejected");
        assert!(!valid_version("1.0/../x"), "a slash should be rejected");
        assert!(!valid_version("$(id)"), "command substitution should be rejected");
    }

    #[test]
    fn remote_bin_path_shape() {
        assert_eq!(
            sys_of("linux", "x86_64", RemoteShell::Posix).bin_path("0.1.73"),
            "$HOME/.velaterm/versions/0.1.73/vela-server"
        );
        // Windows carries the .exe suffix in both shells, with each shell's own path syntax.
        assert_eq!(
            sys_of("windows", "x86_64", RemoteShell::Posix).bin_path("0.1.73"),
            "$HOME/.velaterm/versions/0.1.73/vela-server.exe"
        );
        assert_eq!(
            sys_of("windows", "x86_64", RemoteShell::Powershell).bin_path("0.1.73"),
            "$env:USERPROFILE\\.velaterm\\versions\\0.1.73\\vela-server.exe"
        );
    }

    #[test]
    fn run_json_path_by_shell() {
        assert_eq!(
            sys_of("linux", "x86_64", RemoteShell::Posix).run_json(),
            "$HOME/.velaterm/run.json"
        );
        assert_eq!(
            sys_of("windows", "x86_64", RemoteShell::Powershell).run_json(),
            "$env:USERPROFILE\\.velaterm\\run.json"
        );
    }

    #[test]
    fn serve_data_dir_expr_by_os_and_mode() {
        let posix = RemoteShell::Posix;
        // Isolated mode is always ~/.velaterm/data on Unix regardless of OS.
        assert_eq!(
            serve_data_dir_expr("linux", posix, false),
            ("$HOME/.velaterm/data".into(), "$HOME/.velaterm/data".into())
        );
        assert_eq!(
            serve_data_dir_expr("macos", posix, false),
            ("$HOME/.velaterm/data".into(), "$HOME/.velaterm/data".into())
        );
        // Shared mode uses each OS's desktop release data directory.
        assert_eq!(
            serve_data_dir_expr("linux", posix, true).1,
            "${XDG_DATA_HOME:-$HOME/.local/share}/io.vlinx.vlxterm.release"
        );
        assert_eq!(
            serve_data_dir_expr("macos", posix, true).1,
            "$HOME/Library/Application Support/io.vlinx.vlxterm.release"
        );
        // Native Windows uses %USERPROFILE% / %APPDATA% and needs no conversion.
        let ps = RemoteShell::Powershell;
        assert_eq!(
            serve_data_dir_expr("windows", ps, false),
            (
                "$env:USERPROFILE\\.velaterm\\data".into(),
                "$env:USERPROFILE\\.velaterm\\data".into()
            )
        );
        assert_eq!(
            serve_data_dir_expr("windows", ps, true).1,
            "$env:APPDATA\\io.vlinx.vlxterm.release"
        );
        // Windows behind a POSIX shell creates the directory with a POSIX path but hands the native
        // vela-server.exe a Windows one.
        let (mk, arg) = serve_data_dir_expr("windows", posix, false);
        assert_eq!(mk, "$HOME/.velaterm/data");
        assert_eq!(arg, "$(cygpath -w \"$HOME/.velaterm/data\")");
        let (mk, arg) = serve_data_dir_expr("windows", posix, true);
        assert_eq!(mk, "$(cygpath -u \"$APPDATA\")/io.vlinx.vlxterm.release");
        assert_eq!(arg, "$APPDATA\\io.vlinx.vlxterm.release");
    }

    #[test]
    fn local_link_probe_targets_the_desktop_release_directory() {
        // The link and the database both live in the desktop release data directory, written in the
        // detected shell's own syntax so the shell itself can read them.
        let mac = sys_of("macos", "aarch64", RemoteShell::Posix);
        let link = shell_join(&mac, &desktop_data_dir_expr(&mac), crate::web::local_link::FILENAME);
        assert_eq!(
            link,
            "$HOME/Library/Application Support/io.vlinx.vlxterm.release/vlx-local-link.json"
        );
        assert_eq!(
            read_file_cmd(&mac, &link),
            "cat \"$HOME/Library/Application Support/io.vlinx.vlxterm.release/vlx-local-link.json\" 2>/dev/null"
        );
        assert_eq!(
            desktop_db_probe_cmd(&mac),
            "test -f \"$HOME/Library/Application Support/io.vlinx.vlxterm.release/vlx-term.db\" && echo yes"
        );
        let win = sys_of("windows", "x86_64", RemoteShell::Powershell);
        let db = desktop_db_probe_cmd(&win);
        assert!(db.contains("Test-Path -LiteralPath \"$env:APPDATA\\io.vlinx.vlxterm.release\\vlx-term.db\""));
        // Cygwin reaches the same directory through cygpath, with POSIX separators.
        let cyg = sys_of("windows", "x86_64", RemoteShell::Posix);
        assert_eq!(
            shell_join(&cyg, &desktop_data_dir_expr(&cyg), "vlx-term.db"),
            "$(cygpath -u \"$APPDATA\")/io.vlinx.vlxterm.release/vlx-term.db"
        );
    }

    #[test]
    fn pid_alive_cmd_checks_the_process_name_only_on_windows() {
        let posix = pid_alive_cmd(&sys_of("linux", "x86_64", RemoteShell::Posix), 77, &["velaterm"]);
        assert_eq!(posix, "kill -0 77 2>/dev/null && echo alive");
        // Windows reuses PIDs quickly, so the desktop's two possible names are both accepted and nothing else.
        let win = pid_alive_cmd(
            &sys_of("windows", "x86_64", RemoteShell::Powershell),
            77,
            &["velaterm", "vlx-term"],
        );
        assert!(win.contains("Get-Process -Id 77"));
        assert!(win.contains("$q.ProcessName -like 'velaterm*' -or $q.ProcessName -like 'vlx-term*'"));
        let server = pid_alive_cmd(&sys_of("windows", "x86_64", RemoteShell::Powershell), 9, &["vela-server"]);
        assert!(server.contains("($q.ProcessName -like 'vela-server*')"));
    }

    #[test]
    fn session_target_defaults_to_headless_and_is_cleared_on_disconnect() {
        let s = "target-test-session";
        assert_eq!(session_target(s), ServiceTarget::Headless);
        set_session_target(s, ServiceTarget::DesktopLink);
        assert_eq!(session_target(s), ServiceTarget::DesktopLink);
        clear_session_target(s);
        assert_eq!(session_target(s), ServiceTarget::Headless);
    }

    #[test]
    fn serve_script_posix_detaches_and_records_state() {
        let sys = sys_of("linux", "x86_64", RemoteShell::Posix);
        let s = serve_script(&sys, "0.1.73", "pw123", 42000, false, true);
        assert!(s.contains("nohup env VELA_SERVE_PASSWORD='pw123'"));
        // Detaching every stream is what lets the SSH command return while the server keeps running.
        assert!(s.contains("</dev/null"));
        assert!(s.contains("--mirror 1"));
        assert!(s.contains("\"$HOME/.velaterm/run.json\""));
    }

    #[test]
    fn serve_script_windows_launches_in_two_stages() {
        let sys = sys_of("windows", "x86_64", RemoteShell::Powershell);
        let s = serve_script(&sys, "0.1.73", "pw123", 42000, false, false);
        // Stage one must hand the launch to WMI. Spawning it ourselves would put the server in the SSH
        // session's job object (killed on disconnect) and, with redirection, leak the channel's handles
        // into it (the client would hang).
        assert!(
            s.contains("Invoke-CimMethod -ClassName Win32_Process -MethodName Create"),
            "the first stage must spawn through WMI, not directly"
        );
        assert!(!s.contains("Start-Process -FilePath 'powershell'"));
        assert!(s.contains("CommandLine = 'powershell -NoProfile -NonInteractive -EncodedCommand "));
        // Stage two carries the real launch, so the password and log redirection live inside its blob.
        let b64 = s
            .split("-EncodedCommand ")
            .nth(1)
            .and_then(|t| t.split('\'').next())
            .expect("the first stage should embed the second stage's encoded script");
        use base64::Engine;
        let raw = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .expect("the embedded stage should be valid base64");
        let inner: String = raw
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect::<Vec<_>>()
            .iter()
            .filter_map(|u| char::from_u32(*u as u32))
            .collect();
        assert!(inner.contains("$env:VELA_SERVE_PASSWORD='pw123'"));
        assert!(inner.contains("-RedirectStandardOutput \"$b\\server.log\""));
        assert!(inner.contains("vela-server.exe"));
        assert!(inner.contains("--mirror 0"));
        // The PID handed back must be the server's own, read out of run.json rather than any wrapper's.
        assert!(s.contains("Write-Output ($t | ConvertFrom-Json).pid"));
    }

    #[test]
    fn kill_service_cmd_targets_the_right_run_json() {
        let posix = kill_service_cmd(&sys_of("linux", "x86_64", RemoteShell::Posix));
        assert!(posix.contains("$HOME/.velaterm/run.json"));
        assert!(posix.contains("kill \"$P\""));
        let win = kill_service_cmd(&sys_of("windows", "x86_64", RemoteShell::Powershell));
        assert!(win.contains("$env:USERPROFILE\\.velaterm\\run.json"));
        assert!(win.contains("Stop-Process -Id $o.pid"));
    }

    /// Ignored real-host end-to-end test: connect, detect, provision, SCP, serve, forward, and verify the
    /// port. Requires VLX_TEST_SSH_HOST and a local Linux VLX_DEV_SERVER_BIN.
    #[test]
    #[ignore = "requires VLX_TEST_SSH_HOST and VLX_DEV_SERVER_BIN; run with cargo test -- --ignored"]
    fn integration_connect_against_real_host() {
        let host = match std::env::var("VLX_TEST_SSH_HOST") {
            Ok(h) if !h.is_empty() => h,
            _ => return,
        };
        let tmp = std::env::temp_dir().join("vlx-ssh-it");
        let _ = std::fs::create_dir_all(&tmp);
        let r = connect(
            &tmp,
            &host,
            SshAuth::Auto,
            false,
            false,
            &|_: &str, _: Option<u8>| {},
        )
        .expect("connect should succeed");
        let base = format!("http://127.0.0.1:{}", r.local_port);
        // The embedded frontend root should return 200 through the tunnel.
        let root_status = ureq::get(&base)
            .timeout(Duration::from_secs(8))
            .call()
            .map(|resp| resp.status())
            .unwrap_or(0);
        // POST the alphanumeric random password to the same /api/login path used by LoginGate.
        let login_body = format!("{{\"password\":\"{}\"}}", r.password);
        let login_status = ureq::post(&format!("{base}/api/login"))
            .timeout(Duration::from_secs(8))
            .set("Content-Type", "application/json")
            .send_string(&login_body)
            .map(|resp| resp.status())
            .unwrap_or_else(|e| match e {
                ureq::Error::Status(code, _) => code,
                _ => 0,
            });
        stop_serve(&r.session);
        close_master(&host, &r.session);
        assert_eq!(root_status, 200, "the root page on the forwarded port should return 200");
        assert_eq!(
            login_status, 200,
            "/api/login with the random password should return 200, logging into the backend automatically"
        );
    }

    /// Ignored real-host lifecycle: initial start, disconnect while preserving, reconnect/reuse with the
    /// same password, stop service, then reconnect and verify a fresh start.
    #[test]
    #[ignore = "requires VLX_TEST_SSH_HOST; run with cargo test -- --ignored"]
    fn integration_persist_reuse_kill() {
        let host = match std::env::var("VLX_TEST_SSH_HOST") {
            Ok(h) if !h.is_empty() => h,
            _ => return,
        };
        let tmp = std::env::temp_dir().join("vlx-ssh-it");
        let _ = std::fs::create_dir_all(&tmp);
        let root = |port: u16| -> u16 {
            ureq::get(&format!("http://127.0.0.1:{port}/"))
                .timeout(Duration::from_secs(8))
                .call()
                .map(|r| r.status())
                .unwrap_or_else(|e| match e {
                    ureq::Error::Status(c, _) => c,
                    _ => 0,
                })
        };

        // 1) First connection starts a new persistent service.
        let r1 = connect(
            &tmp,
            &host,
            SshAuth::Auto,
            false,
            false,
            &|_: &str, _: Option<u8>| {},
        )
        .expect("the first connection should succeed");
        assert!(!r1.reused, "the first connection should start a new server (reused=false)");
        // 2) Preserve-service disconnect stops only forwarding/master.
        stop_serve(&r1.session);
        close_master(&host, &r1.session);

        // 3) Reconnect reuses the service/password and still serves the root.
        let r2 = connect(
            &tmp,
            &host,
            SshAuth::Auto,
            false,
            false,
            &|_: &str, _: Option<u8>| {},
        )
        .expect("reconnecting should succeed");
        assert!(r2.reused, "reconnecting should reuse the resident server (reused=true)");
        assert_eq!(r2.password, r1.password, "reuse should keep the same server's password");
        assert_eq!(root(r2.local_port), 200, "after reuse the root page should return 200");

        // 4) Stop-service disconnect kills the persistent remote service.
        kill_remote_service(&host, &r2.session);
        stop_serve(&r2.session);
        close_master(&host, &r2.session);

        // 5) Reconnect after removed run.json starts a fresh service.
        let r3 = connect(
            &tmp,
            &host,
            SshAuth::Auto,
            false,
            false,
            &|_: &str, _: Option<u8>| {},
        )
        .expect("connecting again after shutdown should succeed");
        assert!(!r3.reused, "connecting after shutdown should start a new server (reused=false)");
        // Final shutdown and disconnect.
        kill_remote_service(&host, &r3.session);
        stop_serve(&r3.session);
        close_master(&host, &r3.session);
    }

    /// Ignored real-host end-to-end test against a Windows remote: probe, provision `vela-server.exe`,
    /// start the detached service, forward, and log in through the tunnel.
    ///
    /// Point VLX_TEST_SSH_WIN_HOST at a Windows machine. Both server flavors are covered by the same
    /// test because the probe decides which one runs: a Microsoft OpenSSH host takes the PowerShell path,
    /// a Cygwin/MSYS sshd the POSIX one.
    #[test]
    #[ignore = "requires VLX_TEST_SSH_WIN_HOST; run with cargo test -- --ignored"]
    fn integration_connect_against_windows_host() {
        let host = match std::env::var("VLX_TEST_SSH_WIN_HOST") {
            Ok(h) if !h.is_empty() => h,
            _ => return,
        };
        let tmp = std::env::temp_dir().join("vlx-ssh-it-win");
        let _ = std::fs::create_dir_all(&tmp);

        // Confirm the probe really found Windows before judging anything that follows.
        let session = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
        let probe_t = OpensshTransport::connect(&host, &session, SshAuth::Auto, &|_, _| {})
            .expect("connecting to the Windows host should succeed");
        let sys = probe_system(&probe_t).expect("probing the remote system should succeed");
        close_master(&host, &session);
        assert_eq!(sys.os, "windows", "the probe should report Windows: {sys:?}");

        let r = connect(
            &tmp,
            &host,
            SshAuth::Auto,
            false,
            false,
            &|_: &str, _: Option<u8>| {},
        )
        .expect("connect should succeed");
        let base = format!("http://127.0.0.1:{}", r.local_port);
        let root_status = ureq::get(&base)
            .timeout(Duration::from_secs(8))
            .call()
            .map(|resp| resp.status())
            .unwrap_or(0);
        let login_body = format!("{{\"password\":\"{}\"}}", r.password);
        let login_status = ureq::post(&format!("{base}/api/login"))
            .timeout(Duration::from_secs(8))
            .set("Content-Type", "application/json")
            .send_string(&login_body)
            .map(|resp| resp.status())
            .unwrap_or_else(|e| match e {
                ureq::Error::Status(code, _) => code,
                _ => 0,
            });
        // Leave the machine as it was found: stop the service, not just the tunnel.
        disconnect(&host, &r.session, true);
        assert_eq!(root_status, 200, "the root page on the forwarded port should return 200");
        assert_eq!(
            login_status, 200,
            "/api/login with the random password should return 200, logging into the backend automatically"
        );
    }

    /// Ignored real-host PTY password-auth regression test requiring host/password environment variables.
    #[test]
    #[ignore = "requires VLX_TEST_SSH_HOST and VLX_TEST_SSH_PASSWORD; run with cargo test -- --ignored"]
    fn integration_open_master_pty_password() {
        let host = match std::env::var("VLX_TEST_SSH_HOST") {
            Ok(h) if !h.is_empty() => h,
            _ => return,
        };
        let password = match std::env::var("VLX_TEST_SSH_PASSWORD") {
            Ok(p) if !p.is_empty() => p,
            _ => return,
        };
        let session = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();

        // Ensure known_hosts trust first so strict checking reaches password authentication.
        if let Ok(probe) = probe_host_key(&host) {
            if probe.status != HostKeyStatus::Known {
                trust_host(&host, probe.status == HostKeyStatus::Changed)
                    .expect("writing the trusted entry into known_hosts should succeed");
            }
        }

        // Establish the master through PTY password auth and verify its control socket.
        open_master_pty(&host, &session, true, &password).expect("PTY password authentication should succeed");

        // Run a command through the master to verify reuse without reauthentication.
        let who = run_remote(&host, &session, "whoami").expect("running whoami over the reused master should succeed");
        assert!(!who.trim().is_empty(), "whoami should return a non-empty username");

        // Close the master and remove its socket.
        close_master(&host, &session);
    }
}

