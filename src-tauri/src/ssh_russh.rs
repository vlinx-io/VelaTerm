//! russh transport implementation using a pure-Rust SSH client in GUI builds.
//!
//! Implements [`crate::ssh_remote::SshTransport`] with one authenticated in-process connection.
//! Exec, SFTP upload, and local forwarding open channels on that connection, avoiding external
//! OpenSSH ControlMaster, ConPTY, and askpass issues. Password and key authentication are supported.
//! It currently serves Windows but is written for possible cross-platform migration.
//!
//! Each connection owns a multithreaded Tokio Runtime that continuously drives russh I/O and forwarding
//! accept tasks. Synchronous trait methods use `rt.block_on` for request/response. Command handlers run
//! under spawn_blocking, so this does not nest a runtime inside an async runtime.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::client;
use russh::keys::{load_secret_key, PrivateKeyWithHashAlg};
use russh::{ChannelMsg, Disconnect};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

use crate::agent::inject::split_ssh_target;
use crate::ssh_remote::{
    pick_local_port, wait_port_healthy, Progress, SshAuth, SshTransport, AUTH_REQUIRED_TAG,
};

/// russh callback that validates host keys against `~/.ssh/known_hosts`, sharing OpenSSH's trust source.
/// The frontend has already populated it through probe and trust_host.
struct ClientHandler {
    hostname: String,
    port: u16,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        // Accept an exact known_hosts match; reject unknown, changed, or unreadable keys.
        Ok(
            russh::keys::check_known_hosts(&self.hostname, self.port, server_public_key)
                .unwrap_or(false),
        )
    }
}

/// russh transport owning a runtime, authenticated Handle, original host, session ID, and forwarding task.
pub struct RusshTransport {
    rt: Runtime,
    /// Share through Arc because russh 0.62 Handle is not Clone, while channel/disconnect methods take `&self`.
    handle: Arc<client::Handle<ClientHandler>>,
    orig_host: String,
    session_id: String,
    forwards: Mutex<Vec<JoinHandle<()>>>,
}

impl RusshTransport {
    /// Connect and authenticate after trust_host has stored the host key. `Auto` tries unencrypted
    /// default keys; `Password` uses password plus keyboard-interactive fallback. Rejected public-key
    /// auth returns [`AUTH_REQUIRED_TAG`] so the frontend can request a password.
    ///
    /// `~/.ssh/config` is consulted for the target host: `IdentityFile` entries are tried before the
    /// built-in default keys, `IdentitiesOnly yes` disables the default-key fallback, and
    /// `PreferredAuthentications` without `password`/`keyboard-interactive` skips the password prompt
    /// entirely so a publickey-only config is not silently downgraded to a password request.
    pub fn connect(
        host: &str,
        session: &str,
        auth: SshAuth,
        _progress: Progress,
    ) -> Result<Self, String> {
        let (target, port_opt) = split_ssh_target(host);
        let port_from_target: Option<u16> = port_opt.and_then(|p| p.parse().ok());
        let (user_from_target, host_from_target) = match target.rsplit_once('@') {
            Some((u, h)) => (Some(u.to_string()), h.to_string()),
            None => (None, target.to_string()),
        };

        // Read ~/.ssh/config for the literal target host; missing file yields defaults so the previous
        // hard-coded behaviour is preserved when no config exists.
        let host_key = host_from_target.as_str();
        let ssh_cfg = crate::ssh_config::lookup(host_key, crate::host::home_dir().as_deref());
        let hostname = ssh_cfg.hostname.clone().unwrap_or_else(|| host_from_target.clone());
        let user = match user_from_target {
            Some(u) => u,
            None => match ssh_cfg.user.clone() {
                Some(u) => u,
                None => {
                    return Err(format!(
                        "SSH target must be user@host or set User in ~/.ssh/config: {host}"
                    ))
                }
            },
        };
        let port: u16 = port_from_target.or(ssh_cfg.port).unwrap_or(22);

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| format!("failed to build ssh runtime: {e}"))?;

        let config = Arc::new(client::Config {
            inactivity_timeout: Some(Duration::from_secs(3600)),
            keepalive_interval: Some(Duration::from_secs(30)),
            ..Default::default()
        });

        let handler = ClientHandler {
            hostname: hostname.clone(),
            port,
        };

        // Captured by the async block; kept out of the move to avoid cloning during setup.
        let ssh_cfg_for_auth = ssh_cfg.clone();

        let handle = rt.block_on(async move {
            let mut h = client::connect(config, (hostname.as_str(), port), handler)
                .await
                .map_err(|e| format!("SSH connect failed: {e}"))?;

            match auth {
                SshAuth::Auto => {
                    // Try IdentityFile keys first, then the built-in defaults unless IdentitiesOnly
                    // says otherwise. This preserves the previous behaviour when no config is present.
                    let mut got = false;
                    for path in &ssh_cfg_for_auth.identity_files {
                        if try_publickey_at(&mut h, &user, path).await? {
                            got = true;
                            break;
                        }
                    }
                    if !got
                        && !ssh_cfg_for_auth.identities_only.unwrap_or(false)
                        && authenticate_default_keys(&mut h, &user).await?
                    {
                        got = true;
                    }
                    if !got {
                        // A user who has pinned PreferredAuthentications away from password (typically
                        // publickey-only) must not be silently downgraded to a password prompt; return a
                        // plain error instead of AUTH_REQUIRED_TAG so the frontend does not ask for one.
                        return Err(if ssh_cfg_for_auth.allows_password() {
                            format!("{AUTH_REQUIRED_TAG}public-key authentication failed")
                        } else {
                            "SSH public-key authentication failed and ~/.ssh/config forbids password fallback (PreferredAuthentications does not list password or keyboard-interactive)".to_string()
                        });
                    }
                }
                SshAuth::Password(pw) => {
                    let ok = h
                        .authenticate_password(&user, &pw)
                        .await
                        .map_err(|e| format!("SSH password auth error: {e}"))?;
                    if !ok.success() {
                        return Err(
                            "SSH password authentication failed (wrong password?)".to_string()
                        );
                    }
                }
            }
            Ok::<_, String>(h)
        })?;

        Ok(RusshTransport {
            rt,
            handle: Arc::new(handle),
            orig_host: host.to_string(),
            session_id: session.to_string(),
            forwards: Mutex::new(Vec::new()),
        })
    }
}

/// Load a single unencrypted key from `path` and try it as `user`. Returns `Ok(false)` for a missing,
/// encrypted, or malformed file so the caller can move on to the next `IdentityFile` or to defaults.
/// Auto is a passwordless flow; passphrase-protected keys need agent support (out of scope here).
async fn try_publickey_at(
    h: &mut client::Handle<ClientHandler>,
    user: &str,
    path: &Path,
) -> Result<bool, String> {
    if !path.is_file() {
        return Ok(false);
    }
    let key = match load_secret_key(path, None) {
        Ok(k) => Arc::new(k),
        Err(_) => return Ok(false),
    };
    let hash = h.best_supported_rsa_hash().await.ok().flatten().flatten();
    if let Ok(res) = h
        .authenticate_publickey(user, PrivateKeyWithHashAlg::new(key, hash))
        .await
    {
        if res.success() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Try unencrypted `~/.ssh/{id_ed25519,id_ecdsa,id_rsa}` keys in order. Skip encrypted or unparseable
/// keys because Auto is passwordless; an explicit Key path may support passphrases later.
async fn authenticate_default_keys(
    h: &mut client::Handle<ClientHandler>,
    user: &str,
) -> Result<bool, String> {
    let ssh_dir = match crate::host::home_dir() {
        Some(d) => d.join(".ssh"),
        None => return Ok(false),
    };
    for name in ["id_ed25519", "id_ecdsa", "id_rsa"] {
        let path = ssh_dir.join(name);
        if !path.is_file() {
            continue;
        }
        let key = match load_secret_key(&path, None) {
            Ok(k) => Arc::new(k),
            Err(_) => continue, // Skip encrypted or unparseable keys.
        };
        // RSA negotiates SHA-256/512 signature hashes; Ed25519/ECDSA use None.
        let hash = h.best_supported_rsa_hash().await.ok().flatten().flatten();
        if let Ok(res) = h
            .authenticate_publickey(user, PrivateKeyWithHashAlg::new(key, hash))
            .await
        {
            if res.success() {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

impl SshTransport for RusshTransport {
    fn exec(&self, cmd: &str) -> Result<String, String> {
        self.rt.block_on(async {
            let mut ch = self
                .handle
                .channel_open_session()
                .await
                .map_err(|e| format!("open channel failed: {e}"))?;
            ch.exec(true, cmd)
                .await
                .map_err(|e| format!("exec failed: {e}"))?;

            let mut out: Vec<u8> = Vec::new();
            let mut err: Vec<u8> = Vec::new();
            let mut code: Option<u32> = None;
            loop {
                match ch.wait().await {
                    Some(ChannelMsg::Data { ref data }) => out.extend_from_slice(data),
                    Some(ChannelMsg::ExtendedData { ref data, ext }) => {
                        if ext == 1 {
                            err.extend_from_slice(data);
                        }
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => code = Some(exit_status),
                    Some(_) => {}
                    None => break,
                }
            }
            let stdout = String::from_utf8_lossy(&out).trim().to_string();
            match code {
                Some(0) | None => Ok(stdout),
                Some(c) => {
                    let stderr = String::from_utf8_lossy(&err);
                    Err(format!(
                        "remote command failed (exit {c}): {}",
                        stderr.trim()
                    ))
                }
            }
        })
    }

    fn upload(&self, local: &Path, remote_rel: &str, progress: Progress) -> Result<(), String> {
        let bytes = std::fs::read(local).map_err(|e| format!("read local binary failed: {e}"))?;
        let remote_rel = remote_rel.to_string();
        self.rt.block_on(async {
            let ch = self
                .handle
                .channel_open_session()
                .await
                .map_err(|e| format!("open sftp channel failed: {e}"))?;
            ch.request_subsystem(true, "sftp")
                .await
                .map_err(|e| format!("request sftp subsystem failed: {e}"))?;
            let sftp = russh_sftp::client::SftpSession::new(ch.into_stream())
                .await
                .map_err(|e| format!("sftp init failed: {e}"))?;

            // remote_rel is home-relative because SFTP starts at home; ensure_remote_layout created parents.
            let mut file = sftp
                .create(&remote_rel)
                .await
                .map_err(|e| format!("sftp create failed: {e}"))?;

            let total = bytes.len();
            progress("transfer", Some(0));
            let mut written = 0usize;
            for chunk in bytes.chunks(64 * 1024) {
                file.write_all(chunk)
                    .await
                    .map_err(|e| format!("sftp write failed: {e}"))?;
                written += chunk.len();
                let pct = if total > 0 {
                    (written.min(total) * 99 / total) as u8
                } else {
                    0
                };
                progress("transfer", Some(pct));
            }
            file.flush().await.ok();
            file.shutdown().await.ok();
            progress("transfer", Some(100));
            Ok::<(), String>(())
        })
    }

    fn open_forward(&self, rport: u16) -> Result<u16, String> {
        let lport = pick_local_port(&self.orig_host)?;

        // A local TCP accept loop opens one direct-tcpip channel and copies both directions per connection.
        let listener = self
            .rt
            .block_on(async { TcpListener::bind(("127.0.0.1", lport)).await })
            .map_err(|e| format!("failed to bind local forward port {lport}: {e}"))?;
        let handle = Arc::clone(&self.handle);
        let task = self.rt.spawn(async move {
            loop {
                let (mut socket, _) = match listener.accept().await {
                    Ok(x) => x,
                    Err(_) => break,
                };
                let handle = Arc::clone(&handle);
                tokio::spawn(async move {
                    let channel = match handle
                        .channel_open_direct_tcpip("127.0.0.1", rport as u32, "127.0.0.1", 0)
                        .await
                    {
                        Ok(c) => c,
                        Err(_) => return,
                    };
                    let mut stream = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut socket, &mut stream).await;
                });
            }
        });
        self.forwards.lock().unwrap().push(task);

        // Send a real HTTP health probe to avoid a TCP-only false positive, sharing the OpenSSH helper.
        if let Err(e) = wait_port_healthy(lport, Duration::from_secs(20)) {
            self.close();
            return Err(e);
        }
        Ok(lport)
    }

    fn close(&self) {
        // Abort the local forwarding accept task.
        if let Ok(mut v) = self.forwards.lock() {
            for t in v.drain(..) {
                t.abort();
            }
        }
        // Disconnect SSH; dropping the Box also shuts down the runtime completely.
        let _ = self
            .rt
            .block_on(self.handle.disconnect(Disconnect::ByApplication, "", "en"));
    }

    fn session(&self) -> &str {
        &self.session_id
    }
}
