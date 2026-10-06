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

use std::collections::HashMap;
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
    /// User forwards by local port; aborting a task drops its listener and frees the port.
    user_forwards: Mutex<HashMap<u16, JoinHandle<()>>>,
}

impl RusshTransport {
    /// Connect and authenticate after trust_host has stored the host key. `Auto` tries unencrypted
    /// default keys; `Password` uses password plus keyboard-interactive fallback. Rejected public-key
    /// auth returns [`AUTH_REQUIRED_TAG`] so the frontend can request a password.
    pub fn connect(
        host: &str,
        session: &str,
        auth: SshAuth,
        _progress: Progress,
    ) -> Result<Self, String> {
        let (target, port_opt) = split_ssh_target(host);
        let port: u16 = port_opt.and_then(|p| p.parse().ok()).unwrap_or(22);
        let (user, hostname) = match target.rsplit_once('@') {
            Some((u, h)) => (u.to_string(), h.to_string()),
            None => return Err(format!("SSH target must be user@host: {host}")),
        };

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

        let handle = rt.block_on(async move {
            let mut h = client::connect(config, (hostname.as_str(), port), handler)
                .await
                .map_err(|e| format!("SSH connect failed: {e}"))?;

            match auth {
                SshAuth::Auto => {
                    if !authenticate_default_keys(&mut h, &user).await? {
                        // No usable passwordless key; ask the frontend for a password like the OpenSSH path.
                        return Err(format!(
                            "{AUTH_REQUIRED_TAG}public-key authentication failed"
                        ));
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
            user_forwards: Mutex::new(HashMap::new()),
        })
    }

    /// Local accept loop: one direct-tcpip channel per connection, copying both directions.
    fn spawn_forward(&self, listener: TcpListener, rport: u16) -> JoinHandle<()> {
        let handle = Arc::clone(&self.handle);
        self.rt.spawn(async move {
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
        })
    }
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

impl RusshTransport {
    /// Run `cmd` and, when `input` is given, write it to the command's stdin followed by EOF.
    fn exec_with(&self, cmd: &str, input: Option<&str>) -> Result<String, String> {
        self.rt.block_on(async {
            let mut ch = self
                .handle
                .channel_open_session()
                .await
                .map_err(|e| format!("open channel failed: {e}"))?;
            ch.exec(true, cmd)
                .await
                .map_err(|e| format!("exec failed: {e}"))?;
            if let Some(input) = input {
                ch.data(input.as_bytes())
                    .await
                    .map_err(|e| format!("write stdin failed: {e}"))?;
                ch.eof().await.map_err(|e| format!("close stdin failed: {e}"))?;
            }

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
}

impl SshTransport for RusshTransport {
    fn exec(&self, cmd: &str) -> Result<String, String> {
        self.exec_with(cmd, None)
    }

    fn exec_input(&self, cmd: &str, input: &str) -> Result<String, String> {
        self.exec_with(cmd, Some(input))
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
        let listener = self
            .rt
            .block_on(async { TcpListener::bind(("127.0.0.1", lport)).await })
            .map_err(|e| format!("failed to bind local forward port {lport}: {e}"))?;
        let task = self.spawn_forward(listener, rport);
        self.forwards.lock().unwrap().push(task);

        // Send a real HTTP health probe to avoid a TCP-only false positive, sharing the OpenSSH helper.
        if let Err(e) = wait_port_healthy(lport, Duration::from_secs(20)) {
            self.close();
            return Err(e);
        }
        Ok(lport)
    }

    fn open_user_forward(&self, rport: u16, preferred_lport: Option<u16>) -> Result<u16, String> {
        let listener = self
            .rt
            .block_on(async {
                if let Some(p) = preferred_lport.filter(|&p| crate::ssh_remote::loopback_v6_free(p)) {
                    if let Ok(l) = TcpListener::bind(("127.0.0.1", p)).await {
                        return Ok(l);
                    }
                }
                TcpListener::bind(("127.0.0.1", 0)).await
            })
            .map_err(|e| format!("failed to bind a local port for remote port {rport}: {e}"))?;
        let lport = listener
            .local_addr()
            .map_err(|e| format!("failed to read local forward port: {e}"))?
            .port();
        let task = self.spawn_forward(listener, rport);
        self.user_forwards
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(lport, task);
        Ok(lport)
    }

    fn close_forward(&self, lport: u16) -> Result<(), String> {
        if let Some(task) = self
            .user_forwards
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&lport)
        {
            task.abort();
        }
        Ok(())
    }

    fn close(&self) {
        // Abort the local forwarding accept tasks.
        if let Ok(mut v) = self.forwards.lock() {
            for t in v.drain(..) {
                t.abort();
            }
        }
        for (_, t) in self
            .user_forwards
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain()
        {
            t.abort();
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
