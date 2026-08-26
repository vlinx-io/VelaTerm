//! Client-side SSH remote orchestration for GUI builds.
//!
//! Connect by SSH, detect the remote system, provision `vela-server`, start `--serve --local-http`,
//! establish `ssh -L` forwarding, and open an auto-login client.
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

/// Detected remote system information.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSystem {
    /// Normalized OS: linux, macos, or windows.
    pub os: String,
    /// Normalized architecture: x86_64 or aarch64.
    pub arch: String,
    /// Raw uname output for diagnostics.
    pub raw: String,
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
    // Run only `true`; ControlPersist keeps the resulting master in the background.
    cmd.arg("true");
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
    // Run only `true`; ControlPersist retains the master.
    cmd.arg("true");

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

/// Detect/normalize Unix OS and architecture with `uname -sm`; remote Windows needs separate commands.
pub fn probe_system(t: &dyn SshTransport) -> Result<RemoteSystem, String> {
    let raw = t.exec("uname -sm")?;
    parse_uname(&raw).ok_or_else(|| format!("cannot parse remote uname output: {raw:?}"))
}

/// Parse uname output into normalized OS/architecture.
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

/// Calculate a lowercase SHA-256 remotely using Linux sha256sum or macOS shasum; missing file/tool is Err.
pub fn remote_sha256(t: &dyn SshTransport, remote_path: &str) -> Result<String, String> {
    // Double-quote paths to allow $HOME expansion without word splitting; single-quote awk's `$1`.
    let cmd = format!(
        "sha256sum \"{p}\" 2>/dev/null | awk '{{print $1}}' || shasum -a 256 \"{p}\" | awk '{{print $1}}'",
        p = remote_path
    );
    let out = t.exec(&cmd)?;
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

/// Remote binary path: `$HOME/.velaterm/versions/<version>/vela-server`.
fn remote_bin_path(version: &str) -> String {
    format!("$HOME/.velaterm/versions/{version}/vela-server")
}

/// Create the remote `~/.velaterm/{versions/<version>,data,bin}` layout.
pub fn ensure_remote_layout(t: &dyn SshTransport, version: &str) -> Result<(), String> {
    if !valid_version(version) {
        return Err(format!("Invalid version: {version}"));
    }
    let cmd = format!("mkdir -p \"$HOME/.velaterm/versions/{version}\" \"$HOME/.velaterm/data\" \"$HOME/.velaterm/bin\"");
    t.exec(&cmd).map(|_| ())
}

/// Whether the remote version has an integrity-valid binary with the expected SHA-256.
pub fn remote_binary_ok(t: &dyn SshTransport, version: &str, expected_sha256: &str) -> bool {
    if !valid_version(version) {
        return false;
    }
    match remote_sha256(t, &remote_bin_path(version)) {
        Ok(h) => h.eq_ignore_ascii_case(expected_sha256),
        Err(_) => false,
    }
}

/// Upload through the transport, verify remote SHA-256, atomically rename, and chmod executable. Delete
/// mismatched temporary files so partial data never appears installed. This integrity flow is transport-neutral.
pub fn push_binary(
    t: &dyn SshTransport,
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
    ensure_remote_layout(t, version)?;

    // Upload paths are home-relative and bypass shell expansion.
    let tmp_rel = format!(".velaterm/versions/{version}/.vela-server.tmp");
    let tmp_abs = format!("$HOME/{tmp_rel}");

    // The transport owns byte transfer and transfer progress.
    t.upload(local_bin, &tmp_rel, progress)?;

    // Verify the temporary file remotely.
    let got = remote_sha256(t, &tmp_abs)?;
    if !got.eq_ignore_ascii_case(expected_sha256) {
        let _ = t.exec(&format!("rm -f \"{tmp_abs}\""));
        return Err(format!(
            "remote hash mismatch (refusing to place): expected {expected_sha256}, got {got}"
        ));
    }

    // Atomically install and make executable.
    let final_abs = remote_bin_path(version);
    t.exec(&format!(
        "mv -f \"{tmp_abs}\" \"{final_abs}\" && chmod +x \"{final_abs}\""
    ))?;
    Ok(())
}

/// Verify the remote binary with --version, briefly retrying ETXTBSY after upload handles close.
fn wait_remote_executable(t: &dyn SshTransport, version: &str) -> Result<(), String> {
    let cmd = format!("\"{}\" --version", remote_bin_path(version));
    let mut last = String::new();
    for _ in 0..8 {
        match t.exec(&cmd) {
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

/// Remote service state path: `$HOME/.velaterm/run.json`.
fn remote_run_json() -> &'static str {
    "$HOME/.velaterm/run.json"
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

/// Return the POSIX-shell data-directory expression by mode/OS. Isolated mode uses
/// `$HOME/.velaterm/data`; shared mode explicitly targets the remote desktop release directory:
///   - Linux：`${XDG_DATA_HOME:-$HOME/.local/share}/io.vlinx.vlxterm.release`
///   - macOS: `$HOME/Library/Application Support/io.vlinx.vlxterm.release`.
/// Explicit paths are required because the server's independent identifier would not resolve to desktop data.
fn serve_data_dir_expr(os: &str, shared_db: bool) -> String {
    if !shared_db {
        return "$HOME/.velaterm/data".to_string();
    }
    match os {
        "macos" => "$HOME/Library/Application Support/io.vlinx.vlxterm.release".to_string(),
        // Linux and other Unix fallback.
        _ => "${XDG_DATA_HOME:-$HOME/.local/share}/io.vlinx.vlxterm.release".to_string(),
    }
}

/// Start detached remote `vela-server --serve --local-http` and persist pid/port/password/version/shared_db/
/// mirror in run.json so the service survives client disconnect and can be reused consistently.
///
/// Pass the alphanumeric random password through VELA_SERVE_PASSWORD, never remote process argv.
fn start_detached_serve(
    t: &dyn SshTransport,
    version: &str,
    password: &str,
    rport: u16,
    os: &str,
    shared_db: bool,
    mirror: bool,
) -> Result<u32, String> {
    if !valid_version(version) {
        return Err(format!("Invalid version: {version}"));
    }
    if password.is_empty() || !password.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("serve password must be a non-empty alphanumeric random string".to_string());
    }
    let bin = remote_bin_path(version);
    let run = remote_run_json();
    let data_dir_expr = serve_data_dir_expr(os, shared_db);
    // Embed shared_db and mirror directly as JSON true/false literals.
    let shared_json = if shared_db { "true" } else { "false" };
    let mirror_json = if mirror { "true" } else { "false" };
    // The remote machine is headless and has no panel, so the mirror choice travels as a startup flag.
    let mirror_arg = if mirror { "1" } else { "0" };
    // nohup, closed stdin, redirected logs, and `&` detach from SSH. Persist `$!` state under
    // ~/.velaterm regardless of the selected data directory, then echo the PID.
    let remote_cmd = format!(
        "mkdir -p \"{data_dir_expr}\"; \
         nohup env VELA_SERVE_PASSWORD='{password}' \"{bin}\" --serve --local-http --port {rport} \
           --data-dir \"{data_dir_expr}\" --mirror {mirror_arg} </dev/null >\"$HOME/.velaterm/server.log\" 2>&1 & \
         P=$!; \
         printf '{{\"pid\":%d,\"port\":%d,\"password\":\"%s\",\"version\":\"%s\",\"shared_db\":{shared_json},\"mirror\":{mirror_json}}}\\n' \"$P\" {rport} '{password}' '{version}' > \"{run}\"; \
         echo \"$P\""
    );
    let out = t.exec(&remote_cmd)?;
    out.lines()
        .rev()
        .find_map(|l| l.trim().parse::<u32>().ok())
        .ok_or_else(|| format!("remote serve start returned no valid PID: {out:?}"))
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

/// Read run.json and `kill -0` to find a live remote service. Remove stale state and return None otherwise.
fn detect_running(t: &dyn SshTransport) -> Option<RunState> {
    let run = remote_run_json();
    let out = t.exec(&format!("cat \"{run}\" 2>/dev/null")).ok()?;
    if out.trim().is_empty() {
        return None;
    }
    let rs: RunState = serde_json::from_str(out.trim()).ok()?;
    // Defensively validate state fields before interpolating them into remote commands.
    if !valid_version(&rs.version) || !rs.password.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let alive = t
        .exec(&format!("kill -0 {} 2>/dev/null && echo alive", rs.pid))
        .map(|o| o.trim() == "alive")
        .unwrap_or(false);
    if alive {
        Some(rs)
    } else {
        let _ = t.exec(&format!("rm -f \"{run}\""));
        None
    }
}

/// Shared remote command that extracts a numeric PID from run.json, kills it, and removes the state file.
fn kill_service_cmd() -> String {
    let run = remote_run_json();
    format!(
        "P=$(sed -n 's/.*\"pid\":\\([0-9]\\{{1,\\}}\\).*/\\1/p' \"{run}\" 2>/dev/null); \
         [ -n \"$P\" ] && kill \"$P\" 2>/dev/null; rm -f \"{run}\""
    )
}

/// Explicitly stop the persistent remote service after the user chooses close. This direct entry is
/// OpenSSH-only; active transports use the same command through exec.
pub fn kill_remote_service(host: &str, session: &str) {
    let _ = run_remote(host, session, &kill_service_cmd());
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
        // The SCP path after `:` is home-relative and does not use shell expansion.
        let tmp_abs = format!("$HOME/{remote_rel}");
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
                // Use Linux stat syntax then macOS/BSD fallback; report zero if neither works.
                if let Ok(out) = run_remote(
                    &self.host,
                    &self.session,
                    &format!("stat -c %s \"{tmp_abs}\" 2>/dev/null || stat -f %z \"{tmp_abs}\" 2>/dev/null || echo 0"),
                ) {
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
    // Remove the object under lock, then perform network I/O without blocking the registry.
    let removed = transports().lock().unwrap().remove(session);
    if let Some(t) = removed {
        if kill_remote {
            let _ = t.exec(&kill_service_cmd());
        }
        t.close();
        return;
    }
    // Fallback for missing active state, applicable only to OpenSSH.
    if kill_remote {
        kill_remote_service(host, session);
    }
    stop_serve(session);
    close_master(host, session);
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
    // 2. Verify the remote service through run.json and kill -0.
    let t = OpensshTransport {
        host: host.to_string(),
        session: session.to_string(),
    };
    let Some(rs) = detect_running(&t) else {
        return Err(RebuildErr::Fatal(
            "remote server is no longer running".to_string(),
        ));
    };
    // 3. Reap the dead tunnel child and rebuild on the original port.
    stop_serve(session);
    let lport = openssh_open_forward(host, session, rs.port).map_err(RebuildErr::Retryable)?;
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
/// shared_db selects isolated ~/.velaterm/data or the remote desktop release database, and mirror decides
/// whether the started service mirrors its UI layout across every client connected to it.
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
    // Linux/macOS share POSIX orchestration with hash/stat differences handled internally. Reject remote
    // Windows until its cmd/PowerShell path is implemented.
    if sys.os != "linux" && sys.os != "macos" {
        return Err(format!(
            "currently supports Linux and macOS remotes; detected remote is {} ({})",
            sys.os, sys.raw
        ));
    }
    let version = crate::VERSION;

    // Detect a persistent service retained from an earlier disconnect or abnormal exit.
    if let Some(rs) = detect_running(t) {
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
            });
        }
        // Stop mismatched version/mode and restart below with current settings.
        let _ = t.exec(&kill_service_cmd());
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
    if !remote_binary_ok(t, version, &sha) {
        push_binary(t, version, &local_bin, &sha, progress)?;
    }
    // Retry executable verification around transient ETXTBSY immediately after SCP.
    progress("start", None);
    wait_remote_executable(t, version)?;

    let password = random_password();
    let rport = remote_serve_port(session);
    start_detached_serve(t, version, &password, rport, &sys.os, shared_db, mirror)?;
    progress("forward", None);
    let local_port = t.open_forward(rport)?;
    Ok(ConnectResult {
        session: session.to_string(),
        local_port,
        password,
        reused: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_uname_linux_and_macos() {
        let l = parse_uname("Linux x86_64").unwrap();
        assert_eq!((l.os.as_str(), l.arch.as_str()), ("linux", "x86_64"));
        let m = parse_uname("Darwin arm64").unwrap();
        assert_eq!((m.os.as_str(), m.arch.as_str()), ("macos", "aarch64"));
        let a = parse_uname("Linux amd64").unwrap();
        assert_eq!(a.arch, "x86_64");
        assert!(parse_uname("").is_none());
        assert!(parse_uname("Plan9").is_none()); // Missing architecture field.
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
            remote_bin_path("0.1.73"),
            "$HOME/.velaterm/versions/0.1.73/vela-server"
        );
    }

    #[test]
    fn serve_data_dir_expr_by_os_and_mode() {
        // Isolated mode is always ~/.velaterm/data regardless of OS.
        assert_eq!(serve_data_dir_expr("linux", false), "$HOME/.velaterm/data");
        assert_eq!(serve_data_dir_expr("macos", false), "$HOME/.velaterm/data");
        // Shared mode uses each OS's desktop release data directory.
        assert_eq!(
            serve_data_dir_expr("linux", true),
            "${XDG_DATA_HOME:-$HOME/.local/share}/io.vlinx.vlxterm.release"
        );
        assert_eq!(
            serve_data_dir_expr("macos", true),
            "$HOME/Library/Application Support/io.vlinx.vlxterm.release"
        );
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
