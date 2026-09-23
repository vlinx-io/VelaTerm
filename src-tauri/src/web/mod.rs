//! Browser remote-access service hosting frontend assets and bridging commands, events, and PTY streams over WebSocket.
//!
//! Runs axum on a Tokio runtime in a dedicated thread, isolated from Tauri's main event loop. A shared `AppCtx`
//! resolves PtyManager, Db, and HookServer and exchanges events, so browser and desktop/headless clients naturally
//! use the same PTY manager and SQLite database (see `host.rs`).
//!
//! - LAN mode binds `0.0.0.0` by default, or the single address chosen by the owner (see [`BindChoice`]), and
//!   authenticates through the password login flow (see `auth`).
//! - `rust-embed` bundles `../dist` for offline release use.
//! - `/ws` multiplexes invokes, events, and PTY traffic (see `ws`).

mod access_store;
mod auth;
// desktop_call also uses dispatch, so expose it within the crate rather than keeping it private to web transport.
pub(crate) mod dispatch;
// dispatch mints download tickets, so this is reachable from the crate rather than private to web transport.
pub(crate) mod download;
mod e2ee;
#[cfg(test)]
mod remote_audit_fixture;
// The desktop's always-on loopback link for SSH mirror connections (see local_link.rs).
pub(crate) mod local_link;
pub(crate) mod mirror;
pub(crate) mod presence;
mod rate_limit;
pub(crate) mod share_policy;
// Loopback Web server instance serving the real app to public share visitors through the account tunnel.
pub(crate) mod share_server;
// Outbound WebSocket tunnel dialing the relay and forwarding the share server's HTTP and WebSocket traffic.
pub(crate) mod share_tunnel;
pub(crate) mod public_relay;
mod sniff;
mod static_assets;
mod tls;
pub mod tunnel;
mod ws;

use std::net::Ipv4Addr;
use std::sync::Mutex;

use axum::extract::State;
use axum::http::{StatusCode, Uri};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;

use crate::host::AppCtx;

use auth::AuthState;
use base64::Engine;
use std::sync::Arc;

/// Default listening port, overridable by `web_server_start`.
const DEFAULT_PORT: u16 = 8799;

/// Frontend build output at `src-tauri/../dist`, read from disk in debug and embedded in release.
#[derive(rust_embed::Embed)]
#[folder = "../dist"]
struct Assets;

/// Context shared by axum handlers.
#[derive(Clone)]
pub(crate) struct Ctx {
    pub app: AppCtx,
    pub auth: Arc<AuthState>,
    /// Long-lived server E2EE key used for handshake ECDH; its public key is embedded in pairing URLs.
    pub e2ee_keys: Arc<e2ee::ServerKeys>,
    /// Serve mode used by WS to require paired encryption and reject plaintext in network-exposed LanTls mode.
    pub mode: ServeMode,
    /// Failed-login limiter, checked before any Argon2 work on `/api/login` and the WS handshake.
    /// Shared per data directory across in-process instances (see `LoginRateLimiter::shared`), so a
    /// dual-instance `--serve` setup cannot double the per-IP attempt budget.
    pub limiter: Arc<rate_limit::LoginRateLimiter>,
    /// Per-launch secret required on every request of a `ShareTunnel` instance. The tunnel client holds it
    /// in memory and adds it as a header; local processes cannot reach the share surface without it.
    pub tunnel_secret: Option<String>,
}

/// Public web-service status returned to the frontend in camelCase. Carries no secrets, so Debug is safe.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebServerStatus {
    pub running: bool,
    pub port: Option<u16>,
    /// Primary candidate URL, the first in `urls`, preferring a normal LAN interface.
    pub url: Option<String>,
    /// All candidate URLs across Wi-Fi, Ethernet, and VPN interfaces, with point-to-point tunnels ranked last.
    pub urls: Vec<String>,
    /// Colon-separated uppercase SHA-256 fingerprint of the self-signed certificate for host verification; None when stopped.
    pub fingerprint: Option<String>,
    /// Error message of the most recent failed auto-start (e.g. port in use); cleared by any successful start.
    pub autostart_error: Option<String>,
    /// Last persisted port from app settings, used by the panel to prefill the port field; filled in command_core.
    pub saved_port: Option<u16>,
    /// Whether the persisted enabled flag would auto-start the service on next launch; filled in command_core.
    pub auto_start: bool,
    /// URL scheme of the running service ("https" for LanTls, "http" for the plaintext modes); None when
    /// stopped. Explicit so the frontend can synthesize a URL for an interface that appeared after start
    /// (absent from the `urls` snapshot) without guessing the scheme from the snapshot's first entry.
    pub scheme: Option<String>,
    /// Canonical listen address of the running service (`all`, `loopback`, or an IPv4 address, see
    /// [`BindChoice::as_setting`]); None when stopped.
    pub bind: Option<String>,
    /// Persisted listen-address setting (`remoteAccess.bind`); filled in command_core.
    pub saved_bind: Option<String>,
    /// Persisted address for pairing links in loopback mode (`remoteAccess.pairingHost`); filled in command_core.
    pub saved_pairing_host: Option<String>,
}

impl WebServerStatus {
    fn stopped() -> Self {
        Self {
            running: false,
            port: None,
            url: None,
            urls: Vec::new(),
            fingerprint: None,
            autostart_error: None,
            saved_port: None,
            auto_start: false,
            scheme: None,
            bind: None,
            saved_bind: None,
            saved_pairing_host: None,
        }
    }
}

/// Web serving modes separating bind scope from encryption, unlike the former `local_http` boolean. This enables
/// the plaintext-LAN combination required by native mobile shells.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ServeMode {
    /// LAN with self-signed TLS, including HTTP-to-HTTPS sniffing redirect; desktop remote default. Binds
    /// `0.0.0.0` unless the owner chose one address or loopback (see [`BindChoice`]); the security model
    /// (TLS, pairing, password, E2EE, remote origin) is the same for every bind choice.
    LanTls,
    /// Plain HTTP/ws on `127.0.0.1` for Electron sidecar; loopback never leaves the host.
    LoopbackHttp,
    /// Plain HTTP/ws on LAN for native mobile shells whose RN WebView cannot bypass self-signed certificates,
    /// especially Android. This sacrifices LAN encryption (architecture section 20).
    LanHttp,
    /// Plain HTTP/ws on `127.0.0.1` for the public account share server. Reachable only through the outbound
    /// account tunnel, and every request must carry the per-launch tunnel secret; visitors are authorized by
    /// the relay (account and grant) and tagged with a share scope on the WebSocket.
    ShareTunnel,
}

impl ServeMode {
    /// Whether transport is plaintext; only `LanTls` uses TLS.
    fn plaintext(self) -> bool {
        !matches!(self, ServeMode::LanTls)
    }
    /// Whether this mode is one of the two LAN modes whose listen address the owner may choose.
    fn is_lan(self) -> bool {
        matches!(self, ServeMode::LanTls | ServeMode::LanHttp)
    }
}

/// Where a LAN-mode listener binds. Orthogonal to [`ServeMode`] on purpose: the trust class of a client
/// (`CallOrigin`) depends only on the mode, so a LanTls listener bound to loopback for a tunnel still treats
/// everyone as remote. The default keeps the historical behaviour (every interface).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum BindChoice {
    /// Every interface (`0.0.0.0`), the historical and default behaviour.
    #[default]
    All,
    /// `127.0.0.1` only, for tunnels and reverse proxies on this machine (tailscale serve, ssh -L).
    Loopback,
    /// One IPv4 address of this machine, e.g. a LAN or Tailscale (CGNAT) address.
    Address(Ipv4Addr),
}

impl BindChoice {
    /// Parse the setting or `--bind` value: `all` (also `0.0.0.0` or empty), `loopback` (also `127.0.0.1`),
    /// or a dotted IPv4 address. Surrounding whitespace is trimmed and keywords are case-insensitive.
    /// Loopback addresses other than 127.0.0.1, multicast, and broadcast addresses are rejected; whether an
    /// address is actually present on this machine is only checked when the server starts.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let value = raw.trim();
        match value.to_ascii_lowercase().as_str() {
            "" | "all" | "0.0.0.0" => return Ok(BindChoice::All),
            "loopback" | "127.0.0.1" => return Ok(BindChoice::Loopback),
            _ => {}
        }
        let invalid = || format!("remote_bind_invalid:{}", clip(value));
        let ip: Ipv4Addr = value.parse().map_err(|_| invalid())?;
        if ip.is_loopback() || ip.is_multicast() || ip.is_broadcast() || ip.is_unspecified() {
            return Err(invalid());
        }
        Ok(BindChoice::Address(ip))
    }

    /// Canonical stored form: `all`, `loopback`, or the dotted IPv4 address.
    pub fn as_setting(&self) -> String {
        match self {
            BindChoice::All => "all".to_string(),
            BindChoice::Loopback => "loopback".to_string(),
            BindChoice::Address(ip) => ip.to_string(),
        }
    }
}

/// Listen configuration of a LAN-mode instance: the bind choice plus the host name or IP that pairing links
/// carry in loopback mode, where the machine's own interface addresses are not what clients connect to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListenConfig {
    pub bind: BindChoice,
    /// Only used with [`BindChoice::Loopback`]; kept otherwise so switching modes does not lose it.
    pub pairing_host: Option<String>,
}

/// Bound the length of a user-supplied value echoed back inside an error code.
fn clip(value: &str) -> String {
    value.chars().take(64).collect()
}

/// Validate the address for pairing links: empty means none; otherwise an IPv4 literal (not `0.0.0.0`) or a
/// DNS name (labels of 1 to 63 letters, digits, or hyphens, not starting or ending with a hyphen, at most 253
/// characters, no trailing dot, not an all-numeric last label). No scheme, port, path, fragment, or IPv6: the
/// value becomes the host of a URL whose fragment carries the pairing secret, so nothing that could end or
/// redirect the host part may pass.
pub fn validate_pairing_host(raw: &str) -> Result<Option<String>, String> {
    let value = raw.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let invalid = || format!("remote_pairing_host_invalid:{}", clip(value));
    if let Ok(ip) = value.parse::<Ipv4Addr>() {
        if ip.is_unspecified() || ip.is_multicast() || ip.is_broadcast() {
            return Err(invalid());
        }
        return Ok(Some(value.to_string()));
    }
    if value.len() > 253 {
        return Err(invalid());
    }
    let labels: Vec<&str> = value.split('.').collect();
    let label_ok = |l: &&str| {
        !l.is_empty()
            && l.len() <= 63
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    };
    if !labels.iter().all(label_ok) {
        return Err(invalid());
    }
    // An all-numeric last label reads as a (malformed) IPv4 address, never as a host name.
    if labels
        .last()
        .is_some_and(|l| l.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(invalid());
    }
    Ok(Some(value.to_string()))
}

/// Pure: resolve the socket IP for a mode and bind choice against the machine's current IPv4 addresses.
/// The loopback modes always bind 127.0.0.1 and refuse a specific address (a `--local-http` listener
/// treats its clients as local, so it must never be reachable from a network). The LAN modes bind
/// `0.0.0.0`, `127.0.0.1`, or the chosen address, which must be present; there is no fallback.
fn resolve_bind_ip(mode: ServeMode, bind: &BindChoice, local_v4: &[Ipv4Addr]) -> Result<Ipv4Addr, String> {
    match (mode.is_lan(), bind) {
        (false, BindChoice::Address(ip)) => Err(format!("remote_bind_invalid:{ip}")),
        (false, _) => Ok(Ipv4Addr::LOCALHOST),
        (true, BindChoice::All) => Ok(Ipv4Addr::UNSPECIFIED),
        (true, BindChoice::Loopback) => Ok(Ipv4Addr::LOCALHOST),
        (true, BindChoice::Address(ip)) if local_v4.contains(ip) => Ok(*ip),
        (true, BindChoice::Address(ip)) => Err(format!("remote_bind_unavailable:{ip}")),
    }
}

/// Every IPv4 address currently assigned to this machine, unfiltered: `--bind` may name any local address
/// (a public one on a VPS too), while the panel only offers the filtered [`network_interfaces_list`].
fn local_ipv4_addrs() -> Vec<Ipv4Addr> {
    if_addrs::get_if_addrs()
        .map(|ifaces| {
            ifaces
                .into_iter()
                .filter_map(|i| match i.addr {
                    if_addrs::IfAddr::V4(v4) => Some(v4.ip),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Pure: the hosts clients should use for this listen configuration, in preference order. Every interface
/// advertises the enumerated LAN addresses (historical behaviour); one address advertises only itself;
/// loopback advertises the pairing host, or 127.0.0.1 (correct for `ssh -L`) when none is set.
fn advertised_hosts(listen: &ListenConfig, lan_ips: Vec<String>) -> Vec<String> {
    match &listen.bind {
        BindChoice::All => lan_ips,
        BindChoice::Address(ip) => vec![ip.to_string()],
        BindChoice::Loopback => vec![listen
            .pairing_host
            .clone()
            .unwrap_or_else(|| Ipv4Addr::LOCALHOST.to_string())],
    }
}

/// Extra certificate SAN hosts for a listen configuration: the chosen address (which may be absent from the
/// filtered LAN list, e.g. a public address) or, in loopback mode, the pairing host.
fn extra_san_hosts(listen: &ListenConfig) -> Vec<String> {
    match &listen.bind {
        BindChoice::All => Vec::new(),
        BindChoice::Address(ip) => vec![ip.to_string()],
        BindChoice::Loopback => listen.pairing_host.clone().into_iter().collect(),
    }
}

/// Production identifiers end in `.release` for packaged desktop or `.server` for deployed headless SSH service;
/// development/test uses the default identifier. Production forbids plaintext `LanHttp` on 0.0.0.0, which exists
/// only for device testing. Production mobile uses HTTPS with certificate pinning. Loopback `--local-http` remains
/// valid for SSH server use; the `.server` check only blocks someone manually starting it with `--lan-http`.
pub fn is_production_identifier(identifier: &str) -> bool {
    identifier.ends_with(".release") || identifier.ends_with(".server")
}

/// Handle for a running service.
struct Running {
    port: u16,
    /// Hosts clients should use, derived once at start by [`advertised_hosts`].
    hosts: Vec<String>,
    /// Listen configuration this instance was started with.
    listen: ListenConfig,
    /// Argon2id verifier the instance runs with, so a listen-address change can restart it without the
    /// plaintext password (which no longer exists after start). AuthState holds the same value.
    verifier_phc: String,
    fingerprint: Option<String>,
    /// Startup mode, determining status URL scheme and host.
    mode: ServeMode,
    /// Authentication state and device registry used for pairing links, device listing, and revocation.
    auth: Arc<AuthState>,
    /// Server E2EE key whose public half is embedded in pairing links.
    e2ee_keys: Arc<e2ee::ServerKeys>,
    handle: axum_server::Handle<std::net::SocketAddr>,
    thread: std::thread::JoinHandle<()>,
}

/// Pairing result returned to the frontend and encoded as a QR code, serialized in camelCase.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingInfo {
    /// Browser URL: `scheme://host:port/#pair=<base64url pairing data>`.
    pub url: String,
    /// Issued device token, also embedded in the URL and exposed separately for display/copy.
    pub device_token: String,
}

/// Public device-entry alias used by commands.rs return types.
pub use auth::DeviceEntry;
/// Argon2id PHC hashing, re-exported so command_core can persist the verifier it passes back on auto-start.
pub use auth::hash_password;

/// Password material for [`WebServer::start`].
pub enum StartAuth {
    /// Plaintext password from an interactive caller or the `--serve` CLI; hashed internally.
    Password(String),
    /// Persisted Argon2id PHC verifier string used by auto-start, where no plaintext exists anymore.
    PasswordHash(String),
    /// Tunnel-only mode for the public share server: no password is disclosed, every request must carry the
    /// tunnel secret, and WebSocket visitors are authorized by the share scope on the request headers.
    Tunnel { secret: String },
}

/// Tauri-managed web-service state holding the running handle behind a Mutex for start/stop transitions.
/// The pairing token, device registry, and blocklist persist in the data directory (`access_store`), so a
/// restart no longer invalidates pairing links; only explicit rotation does.
pub struct WebServer {
    inner: Mutex<Option<Running>>,
    /// Message of the most recent failed auto-start, surfaced through status(); cleared by a successful start.
    autostart_error: Mutex<Option<String>>,
}

impl WebServer {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            autostart_error: Mutex::new(None),
        }
    }

    /// Record or clear the auto-start failure shown in the remote-access panel.
    pub fn set_autostart_error(&self, msg: Option<String>) {
        *self.autostart_error.lock().unwrap() = msg;
    }

    /// Starts the service on the default listen configuration (every interface for the LAN modes); see
    /// [`WebServer::start_with_listen`].
    pub fn start(
        &self,
        app: AppCtx,
        auth: StartAuth,
        port: Option<u16>,
        mode: ServeMode,
    ) -> Result<WebServerStatus, String> {
        self.start_with_listen(app, auth, port, mode, ListenConfig::default())
    }

    /// Starts the service, first stopping any running instance so password, port, mode, and listen changes
    /// apply.
    ///
    /// Modes (see [`ServeMode`]): LanTls is self-signed TLS with sniff redirect; LoopbackHttp is plaintext
    /// 127.0.0.1 for Electron without per-message TLS overhead; LanHttp is plaintext for native mobile shells
    /// that cannot bypass self-signed certificates. The LAN modes bind according to `listen.bind` (0.0.0.0 by
    /// default); a chosen address that is not present fails the start before any running instance is stopped,
    /// and never falls back to every interface.
    ///
    /// All modes retain authentication because other local/LAN clients can reach even plaintext ports.
    pub fn start_with_listen(
        &self,
        app: AppCtx,
        auth: StartAuth,
        port: Option<u16>,
        mode: ServeMode,
        listen: ListenConfig,
    ) -> Result<WebServerStatus, String> {
        // Resolve the socket address first: an absent or invalid address must fail without touching a
        // running instance. Interfaces are only enumerated when a specific address needs checking.
        let local_v4 = match listen.bind {
            BindChoice::Address(_) => local_ipv4_addrs(),
            _ => Vec::new(),
        };
        let bind_ip = resolve_bind_ip(mode, &listen.bind, &local_v4)?;
        // Normalize both password variants to an Argon2id PHC verifier; plaintext never outlives this
        // scope. Tunnel-only mode hashes a random throwaway password so the same AuthState plumbing
        // applies while no usable password ever exists.
        let (verifier_phc, tunnel_secret) = match auth {
            StartAuth::Password(pw) => {
                if pw.trim().is_empty() {
                    return Err("Please set an access password first".into());
                }
                (auth::hash_password(&pw)?, None)
            }
            StartAuth::PasswordHash(phc) => {
                if phc.trim().is_empty() {
                    return Err("Please set an access password first".into());
                }
                (phc, None)
            }
            StartAuth::Tunnel { secret } => {
                let throwaway = format!(
                    "{}{}",
                    uuid::Uuid::new_v4().simple(),
                    uuid::Uuid::new_v4().simple()
                );
                (auth::hash_password(&throwaway)?, Some(secret))
            }
        };
        let mut guard = self.inner.lock().unwrap();
        if let Some(running) = guard.take() {
            running.handle.shutdown();
            let _ = running.thread.join();
        }

        let port = port.unwrap_or(DEFAULT_PORT);
        let addr = std::net::SocketAddr::from((bind_ip, port));
        // Synchronously preflight port availability on the exact address the server will use, release it
        // immediately, then let axum-server bind. An address that vanished since resolution is reported as
        // unavailable rather than as a busy port.
        drop(std::net::TcpListener::bind(addr).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrNotAvailable {
                format!("remote_bind_unavailable:{bind_ip}")
            } else {
                format!("Port {port} is already in use: {e}")
            }
        })?);

        let lan_ips = lan_ips();
        let hosts = advertised_hosts(&listen, lan_ips.clone());
        // Persist the E2EE server key, device registry, and self-signed TLS certificate in the data directory.
        let data_dir = app.data_dir()?;
        let e2ee_keys = Arc::new(e2ee::ServerKeys::load_or_create(&data_dir)?);
        // Plaintext needs no certificate; TLS creates one and calculates its frontend fingerprint before moving into the thread.
        let tls_pem: Option<(Vec<u8>, Vec<u8>)>;
        let fingerprint: Option<String>;
        if mode.plaintext() {
            tls_pem = None;
            fingerprint = None;
        } else {
            let (cert_pem, key_pem) =
                tls::ensure_cert(&data_dir, &lan_ips, &extra_san_hosts(&listen))?;
            fingerprint = tls::fingerprint_sha256(&cert_pem);
            tls_pem = Some((cert_pem, key_pem));
        }

        // Remove the obsolete per-device-token registry, replaced by one rotation token and the persisted
        // pairing state in access_store.
        let _ = std::fs::remove_file(data_dir.join("vlx-devices.json"));
        let auth = Arc::new(AuthState::load_or_create(&verifier_phc, &data_dir)?);
        let status = status_from(port, hosts.clone(), fingerprint.clone(), mode, &listen);
        let ctx = Ctx {
            app,
            auth: auth.clone(),
            e2ee_keys: e2ee_keys.clone(),
            mode,
            limiter: rate_limit::LoginRateLimiter::shared(&data_dir),
            tunnel_secret,
        };
        let handle = axum_server::Handle::new();
        let handle_clone = handle.clone();

        let thread = std::thread::Builder::new()
            .name("vlx-web".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        crate::diagnostic_warn!("failed to start Web service tokio runtime: {e}");
                        return;
                    }
                };
                rt.block_on(async move {
                    let router = build_router(ctx);
                    match tls_pem {
                        Some((cert_pem, key_pem)) => {
                            // rustls 0.23 requires an explicit global CryptoProvider when both aws-lc-rs and ring
                            // are present. Match tunnel.rs with aws-lc-rs; installation is process-idempotent.
                            let _ = tokio_rustls::rustls::crypto::aws_lc_rs::default_provider()
                                .install_default();
                            let config = match axum_server::tls_rustls::RustlsConfig::from_pem(
                                cert_pem, key_pem,
                            )
                            .await
                            {
                                Ok(c) => c,
                                Err(e) => {
                                    crate::diagnostic_warn!("TLS configuration failed: {e}");
                                    return;
                                }
                            };
                            // The TLS acceptor sniffs plaintext HTTP and redirects to HTTPS, avoiding errors when users omit the scheme.
                            let acceptor = axum_server::tls_rustls::RustlsAcceptor::new(config)
                                .acceptor(sniff::HttpSniff);
                            // ConnectInfo exposes the peer SocketAddr to handlers for the per-IP login limiter.
                            if let Err(e) = axum_server::bind(addr)
                                .acceptor(acceptor)
                                .handle(handle_clone)
                                .serve(router.into_make_service_with_connect_info::<std::net::SocketAddr>())
                                .await
                            {
                                crate::diagnostic_warn!("Web service exited abnormally: {e}");
                            }
                        }
                        None => {
                            // Local unencrypted path: plain loopback HTTP without TLS acceptor or sniff redirect.
                            if let Err(e) = axum_server::bind(addr)
                                .handle(handle_clone)
                                .serve(router.into_make_service_with_connect_info::<std::net::SocketAddr>())
                                .await
                            {
                                crate::diagnostic_warn!("Web service exited abnormally: {e}");
                            }
                        }
                    }
                });
            })
            .map_err(|e| format!("Failed to start web server thread: {e}"))?;

        *guard = Some(Running {
            port,
            hosts,
            listen,
            verifier_phc,
            fingerprint,
            mode,
            auth,
            e2ee_keys,
            handle,
            thread,
        });

        // Any successful start supersedes a previous auto-start failure.
        self.set_autostart_error(None);

        Ok(status)
    }

    /// Listen configuration of the running instance; None while stopped.
    pub fn listen_config(&self) -> Option<ListenConfig> {
        self.inner.lock().unwrap().as_ref().map(|r| r.listen.clone())
    }

    /// Restarts the running LAN instance on a new listen configuration with the same port, mode, and password
    /// verifier, so the owner never retypes the password. Pairing token, device registry, E2EE key, and
    /// certificate live in the data directory and survive; clients reconnect as after any restart.
    ///
    /// The new address is resolved before the old instance stops, so an absent address leaves it running
    /// untouched. Should the restart still fail after the stop (a port race), the previous configuration is
    /// restarted once and the original error returned: that restores what ran, it never widens the bind.
    /// Accepted limit: a concurrent `web_server_start` between stop and rollback can interleave; the panel
    /// is single-user, so this is documented rather than locked.
    pub fn restart_listen(&self, app: AppCtx, listen: ListenConfig) -> Result<WebServerStatus, String> {
        let (port, mode, verifier_phc, previous) = {
            let guard = self.inner.lock().unwrap();
            let running = guard.as_ref().ok_or("Web server not started")?;
            (
                running.port,
                running.mode,
                running.verifier_phc.clone(),
                running.listen.clone(),
            )
        };
        if !mode.is_lan() {
            return Err(format!("remote_bind_invalid:{}", listen.bind.as_setting()));
        }
        match self.start_with_listen(
            app.clone(),
            StartAuth::PasswordHash(verifier_phc.clone()),
            Some(port),
            mode,
            listen,
        ) {
            Ok(status) => Ok(status),
            Err(e) => {
                if !self.status().running {
                    if let Err(rollback) = self.start_with_listen(
                        app,
                        StartAuth::PasswordHash(verifier_phc),
                        Some(port),
                        mode,
                        previous,
                    ) {
                        crate::diagnostic_warn!(
                            "remote access could not be restored after a failed listen change: {rollback}"
                        );
                    }
                }
                Err(e)
            }
        }
    }

    /// Stops the service. A manual stop also retires any stale auto-start error: the panel must show the
    /// current truth, not the failure of a boot that the user has since overridden.
    pub fn stop(&self) {
        if let Some(running) = self.inner.lock().unwrap().take() {
            running.handle.shutdown();
            let _ = running.thread.join();
        }
        self.set_autostart_error(None);
    }

    /// Returns current status, including the last auto-start failure for the panel.
    pub fn status(&self) -> WebServerStatus {
        let mut status = match &*self.inner.lock().unwrap() {
            Some(r) => status_from(r.port, r.hosts.clone(), r.fingerprint.clone(), r.mode, &r.listen),
            None => WebServerStatus::stopped(),
        };
        status.autostart_error = self.autostart_error.lock().unwrap().clone();
        status
    }

    /// Generates a browser pairing URL containing the current shared token and server public key. `address` chooses
    /// an interface IP; `rotate=true` replaces the token, invalidates old links, and clears all registrations.
    /// When the instance listens on one address or on loopback, `address` is only honoured if it is one of the
    /// advertised hosts, so a link never points at an address the server does not listen on.
    pub fn create_pairing(
        &self,
        address: Option<String>,
        rotate: bool,
    ) -> Result<PairingInfo, String> {
        let guard = self.inner.lock().unwrap();
        let running = guard.as_ref().ok_or("Web server not started")?;
        // Rotate the shared token and clear registrations when requested; otherwise reuse the current token.
        // A rotation that failed to persist is surfaced: silently succeeding would revive old links on restart.
        let token = if rotate {
            running.auth.rotate_pairing_token()?
        } else {
            running.auth.pairing_token()
        };
        let scheme = if running.mode == ServeMode::LanTls {
            "https"
        } else {
            "http"
        };
        let address = address.filter(|s| !s.trim().is_empty());
        let address = match running.listen.bind {
            // Every interface: any address is allowed; the panel synthesizes URLs for interfaces that
            // appeared after start.
            BindChoice::All => address,
            _ => address.filter(|a| running.hosts.contains(a)),
        };
        let host = address
            .or_else(|| running.hosts.first().cloned())
            .unwrap_or_else(|| "localhost".to_string());
        // Put token and server public key in the URL fragment so they never reach the server, proxy logs, or Referer.
        let offer = serde_json::json!({
            "t": token,
            "k": running.e2ee_keys.public_key_b64(),
        });
        let code = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(offer.to_string());
        let url = format!("{scheme}://{host}:{}/#pair={code}", running.port);
        Ok(PairingInfo {
            url,
            device_token: token,
        })
    }

    /// Lists paired devices that actually connected; returns empty while stopped.
    pub fn list_devices(&self) -> Vec<DeviceEntry> {
        match &*self.inner.lock().unwrap() {
            Some(r) => r.auth.list_devices(),
            None => Vec::new(),
        }
    }

    /// Revokes a device by blacklisting and deregistering it. E2EE rejects reconnects and an active connection is
    /// dropped within one heartbeat; other devices are unaffected. Returns false while stopped. Errors when the
    /// revocation cannot be persisted, because it would otherwise be undone by the next restart.
    pub fn revoke_device(&self, device_id: &str) -> Result<bool, String> {
        match &*self.inner.lock().unwrap() {
            Some(r) => r.auth.block_device(device_id),
            None => Ok(false),
        }
    }
}

impl Default for WebServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds the running status. `hosts` are the advertised hosts from [`advertised_hosts`].
fn status_from(
    port: u16,
    hosts: Vec<String>,
    fingerprint: Option<String>,
    mode: ServeMode,
    listen: &ListenConfig,
) -> WebServerStatus {
    // The single scheme of this serve mode, reported explicitly in the status so the frontend never
    // has to infer it from the URL snapshot.
    let scheme = if matches!(mode, ServeMode::LanTls) {
        "https"
    } else {
        "http"
    };
    let urls: Vec<String> = match mode {
        // Loopback plaintext exposes only 127.0.0.1 for the Electron sidecar and the share tunnel server.
        ServeMode::LoopbackHttp | ServeMode::ShareTunnel => {
            vec![format!("http://127.0.0.1:{port}")]
        }
        // LAN modes list the advertised hosts and choose HTTP for mobile or HTTPS for browser remote access.
        ServeMode::LanHttp | ServeMode::LanTls => {
            // Fall back to localhost when no LAN IP is found, preserving local access.
            let hosts = if hosts.is_empty() {
                vec!["localhost".to_string()]
            } else {
                hosts
            };
            hosts
                .iter()
                .map(|h| format!("{scheme}://{h}:{port}"))
                .collect()
        }
    };
    WebServerStatus {
        running: true,
        port: Some(port),
        url: urls.first().cloned(),
        urls,
        fingerprint,
        autostart_error: None,
        saved_port: None,
        auto_start: false,
        scheme: Some(scheme.to_string()),
        // The loopback modes always bind 127.0.0.1, whatever the (default) listen configuration says.
        bind: Some(if mode.is_lan() {
            listen.bind.as_setting()
        } else {
            BindChoice::Loopback.as_setting()
        }),
        saved_bind: None,
        saved_pairing_host: None,
    }
}

/// Builds public login/static routes and protected `/api/*` plus `/ws` routes.
fn build_router(ctx: Ctx) -> Router {
    let tunnel_secret = ctx.tunnel_secret.clone();
    Router::new()
        .route("/api/login", post(auth::login))
        .route("/api/me", get(auth::me))
        .route("/api/mode", get(mode_info))
        .route("/api/logout", post(auth::logout))
        // Image upload now invokes save_pasted_image over authenticated WS with paired E2EE. The cookie-only
        // POST /api/upload path was removed because paired sessions had no cookie and received 401.
        //
        // File download is the one route that cannot ride the WebSocket: a browser's download manager needs a
        // real HTTP response to stream, resume and report speed, and it sends no Authorization header when it
        // fetches. Its credential is therefore a ticket in the URL, minted over the authenticated socket for
        // one path and valid for minutes — see `download`.
        .route("/api/download", get(download::handler))
        .route("/ws", get(ws::ws_handler))
        .fallback(static_handler)
        .layer(axum::middleware::from_fn(audit_http))
        .layer(axum::middleware::from_fn_with_state(
            tunnel_secret,
            tunnel_gate,
        ))
        .with_state(ctx)
}

/// Rejects any request on a `ShareTunnel` instance that lacks the per-launch tunnel secret. Only the
/// in-process tunnel client holds the secret, so other local processes cannot reach the share surface and
/// bypass the relay's account and grant authorization. Other serve modes carry `None` and are unaffected.
async fn tunnel_gate(
    State(secret): State<Option<String>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if let Some(expected) = secret {
        let provided = request
            .headers()
            .get("x-vlx-tunnel-secret")
            .and_then(|v| v.to_str().ok());
        if provided != Some(expected.as_str()) {
            return (StatusCode::FORBIDDEN, "Tunnel secret required").into_response();
        }
    }
    next.run(request).await
}

/// HTTP headers and payloads never enter diagnostics; query credentials are intentionally ignored.
async fn audit_http(mut request: axum::extract::Request, next: axum::middleware::Next) -> axum::response::Response {
    let started=std::time::Instant::now();
    let id=request.headers().get("X-Request-Id").and_then(|v|v.to_str().ok())
        .filter(|s|s.len()==36 && uuid::Uuid::parse_str(s).is_ok())
        .map(str::to_owned).unwrap_or_else(||uuid::Uuid::new_v4().to_string());
    let method=request.method().as_str().to_owned();
    let path=match request.uri().path() {"/api/login"=>"/api/login","/api/me"=>"/api/me","/api/mode"=>"/api/mode","/api/logout"=>"/api/logout","/api/download"=>"/api/download","/ws"=>"/ws",_=>"static"};
    // Diagnostic IDs are UUIDs; arbitrary header text is never copied to either log sink.
    let diagnostic_id=uuid::Uuid::parse_str(&id).map(|u|u.to_string()).unwrap_or_else(|_|uuid::Uuid::new_v4().to_string());
    request.headers_mut().insert("X-Request-Id",axum::http::HeaderValue::from_str(&id).expect("validated request id"));
    crate::diagnostics::record("INFO","http_request",serde_json::json!({"requestId":diagnostic_id,"method":method,"path":path,"status":"started"}));
    let mut response=next.run(request).await;
    response.headers_mut().insert("X-Request-Id",axum::http::HeaderValue::from_str(&id).expect("validated request id"));
    crate::diagnostics::record("INFO","http_response",serde_json::json!({"requestId":diagnostic_id,"method":method,"path":path,"statusCode":response.status().as_u16(),"step":"prepare","durationMs":started.elapsed().as_millis() as u64}));
    response
}

/// Tells the frontend whether pairing is mandatory (LanTls only). Without pairing data, mandatory mode asks for a
/// pairing link instead of showing password login; plaintext loopback/LAN modes retain password login.
/// The share tunnel instance additionally reports the server public key and share flag, because visitors have no
/// pairing fragment: their E2EE handshake is authorized by the tunnel-injected grant instead.
async fn mode_info(State(ctx): State<Ctx>) -> impl IntoResponse {
    let mut info = serde_json::json!({
        "requirePairing": ctx.mode == ServeMode::LanTls,
    });
    if ctx.mode == ServeMode::ShareTunnel {
        info["requirePairing"] = serde_json::json!(true);
        info["share"] = serde_json::json!(true);
        info["e2eeKey"] = serde_json::json!(ctx.e2ee_keys.public_key_b64());
    }
    axum::Json(info)
}

/// Serves embedded SPA assets, falling back to index.html for frontend routing. Assets contain no secrets and are
/// public; actual data and terminal access remain protected through `/ws`.
///
/// Compression, ETag validators, and cache policy live in `static_assets`; this handler only resolves which
/// embedded file answers the request. See that module for why serving these raw was expensive.
async fn static_handler(
    State(_ctx): State<Ctx>,
    headers: axum::http::HeaderMap,
    uri: Uri,
) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(res) = static_assets::serve(path, path, mime_for(path), &headers).await {
        return res;
    }
    // SPA fallback sends unknown paths to frontend routing. index.html's own revalidate policy applies,
    // not the requested route's, because these bytes are index.html regardless of the URL that asked.
    if let Some(res) = static_assets::serve(
        "index.html",
        "index.html",
        "text/html; charset=utf-8",
        &headers,
    )
    .await
    {
        return res;
    }
    (
        StatusCode::NOT_FOUND,
        "Frontend assets not found (dist not built?)",
    )
        .into_response()
}

/// Write a secret file so it is owner-only (0600) from the moment it exists, instead of chmod-after-write,
/// which leaves a window where the file carries default umask permissions. On non-Unix platforms this is a
/// plain create-truncate write. An idempotent set_permissions afterwards also repairs a pre-existing file
/// that was created with wider permissions by an older build.
pub(crate) fn write_owner_only(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(path)?;
    f.write_all(bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // `mode` applies only at creation; tighten a pre-existing file too.
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Minimal extension-to-MIME mapping that avoids another dependency.
fn mime_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// Enumerates LAN IPv4 candidates that other devices can actually reach, for UI listing, pairing URLs, and TLS SANs.
/// Filtering is deliberately strict because the first result becomes the pairing-link host.
///
/// Rules:
/// 1. Keep only non-loopback private IPv4 ranges, plus the RFC 6598 CGNAT range (100.64.0.0/10) that VPN meshes
///    such as Tailscale assign; `is_private()` does not cover CGNAT, but peers on the same mesh can reach it.
/// 2. Exclude network and broadcast addresses; macOS internal interfaces often report unreachable `.0` noise.
/// 3. Exclude system-internal, VM, container, and bridge interfaces by name.
/// 4. Rank broadcast-capable Wi-Fi/Ethernet first and point-to-point VPN/tunnel interfaces last; CGNAT
///    addresses always rank behind every non-CGNAT entry, regardless of broadcast capability. Tunnels remain
///    as fallbacks because they can be the only reachable address for VPN users, but must not become the default.
fn lan_ips() -> Vec<String> {
    iface_candidates().into_iter().map(|c| c.ip).collect()
}

/// A reachable interface candidate exposed to the remote-access panel's IP selector.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInterface {
    /// Interface name as reported by the OS (e.g. `en0`, `utun3`, `tailscale0`), shown for recognition.
    pub name: String,
    /// IPv4 address in dotted-decimal form; feeds `web_pairing_create`'s `address` argument.
    pub ip: String,
    /// Point-to-point interface without a broadcast address, usually a VPN/tunnel; marked in the selector.
    pub vpn: bool,
}

/// Lists the selectable interface candidates with names and a VPN flag for the UI's IP selector.
/// Shares [`iface_candidates`] with [`lan_ips`] so selector options, URL list, and TLS SANs
/// are identically filtered by construction.
pub fn network_interfaces_list() -> Vec<NetworkInterface> {
    iface_candidates()
}

/// Single enumeration behind [`lan_ips`] and [`network_interfaces_list`]; applies the documented rules
/// and delegates ranking to the pure [`order_candidates`].
fn iface_candidates() -> Vec<NetworkInterface> {
    let mut candidates: Vec<NetworkInterface> = Vec::new();
    if let Ok(ifaces) = if_addrs::get_if_addrs() {
        for iface in ifaces {
            if iface.is_loopback() || is_virtual_iface(&iface.name) {
                continue;
            }
            let name = iface.name;
            if let if_addrs::IfAddr::V4(v4) = iface.addr {
                if !(v4.ip.is_private() || is_cgnat(v4.ip))
                    || is_network_or_broadcast(v4.ip, v4.netmask)
                {
                    continue;
                }
                // A broadcast address indicates normal LAN; its absence usually indicates a point-to-point VPN.
                let vpn = v4.broadcast.is_none();
                candidates.push(NetworkInterface {
                    name,
                    ip: v4.ip.to_string(),
                    vpn,
                });
            }
        }
    }
    order_candidates(candidates)
}

/// Pure ranking of accepted candidates: CGNAT (100.64.0.0/10, e.g. Tailscale) addresses always rank
/// behind every non-CGNAT entry regardless of broadcast capability, and within each of those groups
/// broadcast-capable LAN interfaces come before point-to-point tunnels. The sort is stable, so OS
/// enumeration order is preserved inside each bucket. Separated from [`iface_candidates`] so the
/// ordering invariant is unit-testable without depending on the machine's real interfaces.
fn order_candidates(mut candidates: Vec<NetworkInterface>) -> Vec<NetworkInterface> {
    let cgnat = |c: &NetworkInterface| {
        c.ip
            .parse::<std::net::Ipv4Addr>()
            .map(is_cgnat)
            .unwrap_or(false)
    };
    candidates.sort_by_key(|c| (cgnat(c), c.vpn));
    candidates
}

/// Whether an IPv4 lies in the RFC 6598 carrier-grade NAT range 100.64.0.0/10, used by Tailscale.
/// Not covered by `Ipv4Addr::is_private()`; a bitmask check avoids the unstable `is_shared()`.
fn is_cgnat(ip: std::net::Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 100 && (o[1] & 0b1100_0000) == 64
}

/// Whether an interface is system-internal, virtual-machine, container, or bridge traffic unreachable from LAN peers.
///
/// VPN tunnels (`utun`/`ppp`) are retained but ranked last by [`lan_ips`] because VPN access may depend on them.
/// Bridges are excluded because they are usually VM NAT gateways or macOS Internet Sharing interfaces unreachable
/// from the real LAN. This intentionally omits the rare direct-phone Internet Sharing case.
fn is_virtual_iface(name: &str) -> bool {
    // Prefixes covering common virtual interfaces across all three platforms:
    const VIRTUAL_PREFIXES: &[&str] = &[
        // macOS AirDrop, low-latency WLAN, Apple private, and software AP.
        "awdl", "llw", "anpi", "ap", // VMware, VirtualBox, and Parallels.
        "vmnet", "vboxnet", "vnic", // Containers and Linux virtual bridges.
        "docker", "veth", "virbr",
        // Bridges/Internet Sharing, usually host-side .1 NAT gateways unreachable externally.
        "bridge", // Overlay networks such as ZeroTier.
        "zt",
    ];
    let name = name.to_ascii_lowercase();
    VIRTUAL_PREFIXES.iter().any(|p| name.starts_with(p))
}

/// Whether an IPv4 is its subnet's network or broadcast address, neither of which is a connectable host.
///
/// For /24, `.0` is network and `.255` broadcast. /31 and /32 point-to-point masks have no conventional host
/// distinction and are allowed directly.
fn is_network_or_broadcast(ip: std::net::Ipv4Addr, netmask: std::net::Ipv4Addr) -> bool {
    let ip = u32::from(ip);
    let host_mask = !u32::from(netmask); // Host bits are 1; network bits are 0.
    if host_mask <= 1 {
        // Allow /32 and /31, which have no conventional network/broadcast distinction.
        return false;
    }
    let host_part = ip & host_mask;
    host_part == 0 || host_part == host_mask
}

#[cfg(test)]
mod tests {
    use super::{
        advertised_hosts, hash_password, is_cgnat, is_network_or_broadcast, is_production_identifier,
        is_virtual_iface, order_candidates, resolve_bind_ip, validate_pairing_host, BindChoice,
        ListenConfig, NetworkInterface, ServeMode, StartAuth, WebServer,
    };
    use std::net::Ipv4Addr;

    /// Find a free port by binding zero then releasing it; the small race is acceptable in tests.
    fn free_port() -> u16 {
        let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        l.local_addr().unwrap().port()
    }

    /// Restarting the service against the same data dir keeps the pairing token, and a persisted PHC
    /// verifier (the auto-start path) still accepts the original password. Loopback-only; the server is
    /// stopped immediately (never bind 0.0.0.0 in tests). Since the in-process PAIRING_STORES registry
    /// shares live pairing state, the first server AND its references are dropped and the registry is
    /// asserted empty before the restart — proving the token really reloads from the FILE.
    #[test]
    fn pairing_token_survives_restart_and_hash_start_verifies_password() {
        let tmp = std::env::temp_dir().join(format!(
            "vlx-web-restart-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let db = crate::db::Db::open(&tmp.join("t.db")).unwrap();
        let host = std::sync::Arc::new(crate::host::HeadlessHost::new(tmp.clone(), db));
        let ctx = crate::host::AppCtx::Headless(host);

        let port = free_port();
        let web = WebServer::new();
        web.start(
            ctx.clone(),
            StartAuth::Password("pw".into()),
            Some(port),
            ServeMode::LoopbackHttp,
        )
        .expect("first start should succeed");
        let token1 = web.create_pairing(None, false).unwrap().device_token;
        web.stop();
        // Drop every reference to the first server's pairing state. stop() joins the server thread,
        // so its Ctx clones are gone; dropping the WebServer releases the Running handle's AuthState.
        drop(web);
        assert!(
            !super::auth::pairing_store_alive(&tmp),
            "the shared in-process pairing store must be dead before the restart, otherwise this \
             test would prove shared-state reuse instead of the file reload it claims"
        );

        // "Restart": a fresh WebServer started from a persisted Argon2id hash, as auto-start does.
        let phc = hash_password("pw").unwrap();
        let web2 = WebServer::new();
        // A stale auto-start error must be cleared by a successful start (panel shows current truth).
        web2.set_autostart_error(Some("stale error".into()));
        web2.start(
            ctx,
            StartAuth::PasswordHash(phc),
            Some(port),
            ServeMode::LoopbackHttp,
        )
        .expect("restart from persisted hash should succeed");
        assert!(
            web2.status().autostart_error.is_none(),
            "a successful start must clear a stale autostart error"
        );
        let token2 = web2.create_pairing(None, false).unwrap().device_token;
        assert_eq!(token1, token2, "pairing token must survive a restart");

        // The persisted verifier still accepts the original password (second factor of the handshake).
        {
            let guard = web2.inner.lock().unwrap();
            let running = guard.as_ref().unwrap();
            assert!(running.auth.verify_password("pw"));
            assert!(!running.auth.verify_password("wrong"));
        }
        web2.stop();
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Raw loopback HTTP POST to /api/login with `Connection: close`, returning the full response text.
    /// Connecting retries briefly: start() returns once the thread is spawned, slightly before axum's
    /// async bind actually accepts connections.
    fn http_post_login(port: u16, password: &str) -> String {
        use std::io::{Read, Write};
        let mut stream = None;
        for _ in 0..100 {
            match std::net::TcpStream::connect(("127.0.0.1", port)) {
                Ok(s) => {
                    stream = Some(s);
                    break;
                }
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(20)),
            }
        }
        let mut stream = stream.expect("web server never started accepting connections");
        let body = format!("{{\"password\":\"{password}\"}}");
        let req = format!(
            "POST /api/login HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        let _ = stream.read_to_string(&mut resp);
        resp
    }

    /// Repeated failed logins are rate-limited per IP: five wrong passwords yield 401, the sixth attempt
    /// is rejected with 429 before any Argon2 work — even with the correct password — and a correct
    /// password within the limit returns 200 plus a token. Loopback-only; this also proves the
    /// ConnectInfo wiring works with axum-server for real connections.
    #[test]
    fn login_is_rate_limited_per_ip_before_argon2() {
        let tmp = std::env::temp_dir().join(format!(
            "vlx-web-ratelimit-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let db = crate::db::Db::open(&tmp.join("t.db")).unwrap();
        let host = std::sync::Arc::new(crate::host::HeadlessHost::new(tmp.clone(), db));
        let ctx = crate::host::AppCtx::Headless(host);

        // Ephemeral-port retry pattern shared with the dispatch tests: the probed port can be stolen.
        let web = WebServer::new();
        let mut port = 0;
        let mut started = Err("never attempted".to_string());
        for _ in 0..5 {
            port = free_port();
            started = web.start(
                ctx.clone(),
                StartAuth::Password("right-pw".into()),
                Some(port),
                ServeMode::LoopbackHttp,
            );
            if started.is_ok() {
                break;
            }
        }
        started.expect("failed to start the loopback web server after retries");

        // A correct password within the limit succeeds and returns a session token.
        let ok = http_post_login(port, "right-pw");
        assert!(ok.starts_with("HTTP/1.1 200"), "expected 200, got: {ok}");
        assert!(ok.contains("token"), "expected a token body, got: {ok}");

        // Five wrong passwords are individually rejected as 401 (success above cleared the counter).
        for i in 0..5 {
            let r = http_post_login(port, "wrong");
            assert!(r.starts_with("HTTP/1.1 401"), "attempt {i}: expected 401, got: {r}");
        }
        // The sixth attempt hits the limiter BEFORE Argon2: even the correct password now yields 429.
        let blocked = http_post_login(port, "right-pw");
        assert!(
            blocked.starts_with("HTTP/1.1 429"),
            "expected 429 after five failures, got: {blocked}"
        );

        web.stop();
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// write_owner_only creates secret files with 0600 at open time and repairs looser pre-existing modes.
    #[cfg(unix)]
    #[test]
    fn write_owner_only_creates_and_repairs_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!(
            "vlx-write-owner-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("secret.txt");
        super::write_owner_only(&path, b"s3cret").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(std::fs::read(&path).unwrap(), b"s3cret");

        // A pre-existing file with wide permissions is overwritten AND tightened.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        super::write_owner_only(&path, b"rotated").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(std::fs::read(&path).unwrap(), b"rotated");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The status carries the serve mode's scheme explicitly (FIX for the frontend's proxy inference
    /// from the first snapshot URL): https only for LanTls, http for both plaintext modes, None stopped.
    #[test]
    fn status_reports_the_explicit_scheme_per_mode() {
        use super::{status_from, WebServerStatus};
        let s = |mode| status_from(8799, vec![], None, mode, &super::ListenConfig::default());
        assert_eq!(s(ServeMode::LanTls).scheme.as_deref(), Some("https"));
        assert_eq!(s(ServeMode::LanHttp).scheme.as_deref(), Some("http"));
        assert_eq!(s(ServeMode::LoopbackHttp).scheme.as_deref(), Some("http"));
        assert!(WebServerStatus::stopped().scheme.is_none());
    }

    fn v4(s: &str) -> Ipv4Addr {
        s.parse().unwrap()
    }

    #[test]
    fn bind_choice_parse_accepts_and_canonicalizes() {
        for raw in ["all", "", "0.0.0.0", "ALL", "  all  "] {
            assert_eq!(BindChoice::parse(raw), Ok(BindChoice::All), "{raw:?}");
        }
        for raw in ["loopback", "127.0.0.1", "Loopback", " loopback "] {
            assert_eq!(BindChoice::parse(raw), Ok(BindChoice::Loopback), "{raw:?}");
        }
        for raw in ["100.100.83.2", "192.168.1.5", "10.0.0.5", "203.0.113.7"] {
            let parsed = BindChoice::parse(raw).unwrap();
            assert_eq!(parsed, BindChoice::Address(v4(raw)));
            // Round trip through the canonical stored form.
            assert_eq!(parsed.as_setting(), raw);
            assert_eq!(BindChoice::parse(&parsed.as_setting()), Ok(parsed));
        }
        assert_eq!(BindChoice::All.as_setting(), "all");
        assert_eq!(BindChoice::Loopback.as_setting(), "loopback");
        assert_eq!(BindChoice::default(), BindChoice::All, "the default must stay every interface");
    }

    #[test]
    fn bind_choice_parse_rejects_invalid() {
        for raw in [
            "::1",
            "fe80::1",
            "300.1.1.1",
            "224.0.0.1",
            "255.255.255.255",
            "127.0.0.2",
            "host.example",
            "10.0.0.1:80",
            "lo opback",
        ] {
            let err = BindChoice::parse(raw).unwrap_err();
            assert!(err.starts_with("remote_bind_invalid:"), "{raw:?}: {err}");
        }
    }

    /// The default bind is byte-identical to the historical behaviour: 0.0.0.0 for both LAN modes,
    /// 127.0.0.1 for the loopback modes.
    #[test]
    fn resolve_bind_ip_default_unchanged() {
        let none: &[Ipv4Addr] = &[];
        assert_eq!(resolve_bind_ip(ServeMode::LanTls, &BindChoice::All, none), Ok(Ipv4Addr::UNSPECIFIED));
        assert_eq!(resolve_bind_ip(ServeMode::LanHttp, &BindChoice::All, none), Ok(Ipv4Addr::UNSPECIFIED));
        assert_eq!(resolve_bind_ip(ServeMode::LoopbackHttp, &BindChoice::All, none), Ok(Ipv4Addr::LOCALHOST));
        assert_eq!(resolve_bind_ip(ServeMode::ShareTunnel, &BindChoice::All, none), Ok(Ipv4Addr::LOCALHOST));
    }

    #[test]
    fn resolve_bind_ip_loopback_and_specific() {
        let local = [v4("192.168.1.5"), v4("100.100.83.2")];
        assert_eq!(resolve_bind_ip(ServeMode::LanTls, &BindChoice::Loopback, &local), Ok(Ipv4Addr::LOCALHOST));
        assert_eq!(
            resolve_bind_ip(ServeMode::LanTls, &BindChoice::Address(v4("100.100.83.2")), &local),
            Ok(v4("100.100.83.2"))
        );
        assert_eq!(
            resolve_bind_ip(ServeMode::LanHttp, &BindChoice::Address(v4("192.168.1.5")), &local),
            Ok(v4("192.168.1.5"))
        );
    }

    /// An address that is not present fails with its own code; there is no fallback to 0.0.0.0.
    #[test]
    fn resolve_bind_ip_absent_address_fails() {
        let local = [v4("192.168.1.5")];
        assert_eq!(
            resolve_bind_ip(ServeMode::LanTls, &BindChoice::Address(v4("192.0.2.10")), &local),
            Err("remote_bind_unavailable:192.0.2.10".to_string())
        );
        assert_eq!(
            resolve_bind_ip(ServeMode::LanTls, &BindChoice::Address(v4("192.0.2.10")), &[]),
            Err("remote_bind_unavailable:192.0.2.10".to_string())
        );
    }

    /// A loopback mode (local trust for --local-http) never binds a network address, even if present.
    #[test]
    fn resolve_bind_ip_never_widens_loopback_modes() {
        let local = [v4("192.168.1.5")];
        for mode in [ServeMode::LoopbackHttp, ServeMode::ShareTunnel] {
            assert!(resolve_bind_ip(mode, &BindChoice::Address(v4("192.168.1.5")), &local)
                .unwrap_err()
                .starts_with("remote_bind_invalid:"));
            assert_eq!(resolve_bind_ip(mode, &BindChoice::Loopback, &local), Ok(Ipv4Addr::LOCALHOST));
        }
    }

    #[test]
    fn validate_pairing_host_accepts_names_and_ipv4() {
        assert_eq!(validate_pairing_host(""), Ok(None));
        assert_eq!(validate_pairing_host("   "), Ok(None));
        for ok in ["vela.tailnet-abc.ts.net", "my-mac", "100.100.83.2", "127.0.0.1", "a1.example"] {
            assert_eq!(validate_pairing_host(ok), Ok(Some(ok.to_string())), "{ok}");
        }
        assert_eq!(validate_pairing_host(" my-mac "), Ok(Some("my-mac".to_string())));
        // 63-character labels and a 253-character name are the limits, inclusive.
        let label63 = "a".repeat(63);
        assert!(validate_pairing_host(&format!("{label63}.example")).is_ok());
        let name253 = format!("{}.{}.{}.{}", "a".repeat(63), "b".repeat(63), "c".repeat(63), "d".repeat(61));
        assert_eq!(name253.len(), 253);
        assert!(validate_pairing_host(&name253).is_ok());
    }

    #[test]
    fn validate_pairing_host_rejects_injection_and_malformed() {
        let label64 = "a".repeat(64);
        let name254 = format!("{}.{}.{}.{}", "a".repeat(63), "b".repeat(63), "c".repeat(63), "d".repeat(62));
        for bad in [
            "https://x",
            "x:8799",
            "x/y",
            "a b",
            "-bad.example",
            "bad-.example",
            "evil.example/#",
            "evil.example#pair=x",
            "user@host",
            "0.0.0.0",
            "[::1]",
            "::1",
            "host.example.",
            "a..b",
            "300.1.1.1",
            "1.2.3",
            label64.as_str(),
            name254.as_str(),
        ] {
            let err = validate_pairing_host(bad).unwrap_err();
            assert!(err.starts_with("remote_pairing_host_invalid:"), "{bad:?}: {err}");
        }
    }

    #[test]
    fn advertised_hosts_follow_listen_config() {
        let lan = || vec!["192.168.1.5".to_string(), "100.100.83.2".to_string()];
        let cfg = |bind, host: Option<&str>| ListenConfig { bind, pairing_host: host.map(str::to_string) };
        assert_eq!(advertised_hosts(&cfg(BindChoice::All, Some("ignored.example")), lan()), lan());
        assert_eq!(
            advertised_hosts(&cfg(BindChoice::Address(v4("100.100.83.2")), Some("ignored.example")), lan()),
            vec!["100.100.83.2"]
        );
        assert_eq!(
            advertised_hosts(&cfg(BindChoice::Loopback, Some("vela.example.ts.net")), lan()),
            vec!["vela.example.ts.net"]
        );
        assert_eq!(advertised_hosts(&cfg(BindChoice::Loopback, None), lan()), vec!["127.0.0.1"]);
    }

    #[test]
    fn status_urls_reflect_listen_config() {
        use super::status_from;
        let cfg = |bind, host: Option<&str>| ListenConfig { bind, pairing_host: host.map(str::to_string) };
        let lan = vec!["192.168.1.5".to_string(), "100.100.83.2".to_string()];

        let all = cfg(BindChoice::All, None);
        let s = status_from(8799, advertised_hosts(&all, lan.clone()), None, ServeMode::LanTls, &all);
        assert_eq!(s.urls, ["https://192.168.1.5:8799", "https://100.100.83.2:8799"]);
        assert_eq!(s.bind.as_deref(), Some("all"));

        let one = cfg(BindChoice::Address(v4("100.100.83.2")), None);
        let s = status_from(8799, advertised_hosts(&one, lan.clone()), None, ServeMode::LanTls, &one);
        assert_eq!(s.urls, ["https://100.100.83.2:8799"]);
        assert_eq!(s.bind.as_deref(), Some("100.100.83.2"));

        let tunnel = cfg(BindChoice::Loopback, Some("vela.example.ts.net"));
        let s = status_from(8799, advertised_hosts(&tunnel, lan), None, ServeMode::LanTls, &tunnel);
        assert_eq!(s.urls, ["https://vela.example.ts.net:8799"]);
        assert_eq!(s.bind.as_deref(), Some("loopback"));

        // The loopback modes report their real bind regardless of the (default) listen configuration.
        let s = status_from(8799, vec![], None, ServeMode::LoopbackHttp, &ListenConfig::default());
        assert_eq!(s.bind.as_deref(), Some("loopback"));
    }

    fn temp_ctx(tag: &str) -> (std::path::PathBuf, crate::host::AppCtx) {
        let tmp = std::env::temp_dir().join(format!(
            "vlx-web-{tag}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let db = crate::db::Db::open(&tmp.join("t.db")).unwrap();
        let host = std::sync::Arc::new(crate::host::HeadlessHost::new(tmp.clone(), db));
        (tmp, crate::host::AppCtx::Headless(host))
    }

    /// Start a LanTls instance bound to loopback on a free 127.0.0.1 port, retrying against port theft.
    fn start_loopback_lan_tls(web: &WebServer, ctx: &crate::host::AppCtx, pairing_host: Option<&str>) -> u16 {
        let mut last = Err("never attempted".to_string());
        for _ in 0..5 {
            let port = free_port();
            last = web.start_with_listen(
                ctx.clone(),
                StartAuth::Password("pw".into()),
                Some(port),
                ServeMode::LanTls,
                ListenConfig { bind: BindChoice::Loopback, pairing_host: pairing_host.map(str::to_string) },
            );
            if last.is_ok() {
                return port;
            }
        }
        panic!("failed to start the loopback LanTls server: {last:?}");
    }

    /// Plaintext HTTP to the port returns the response head; retries until the listener accepts.
    fn plaintext_get(port: u16) -> String {
        use std::io::{Read, Write};
        for _ in 0..100 {
            if let Ok(mut s) = std::net::TcpStream::connect(("127.0.0.1", port)) {
                let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(5)));
                let req = format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
                s.write_all(req.as_bytes()).unwrap();
                let mut resp = String::new();
                let _ = s.read_to_string(&mut resp);
                return resp;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("server never accepted connections");
    }

    /// A LanTls listener bound to loopback keeps the LanTls security model: plaintext is answered by the
    /// HTTPS sniff redirect (TLS acceptor), clients are remote, and status and pairing links carry the
    /// pairing host instead of any interface address, even when another address is requested.
    #[test]
    fn loopback_bound_lan_tls_keeps_tls_and_pairing_host() {
        let (tmp, ctx) = temp_ctx("loopback-tls");
        let web = WebServer::new();
        let port = start_loopback_lan_tls(&web, &ctx, Some("vela.example.ts.net"));

        let resp = plaintext_get(port);
        assert!(resp.starts_with("HTTP/1.1 301"), "expected the TLS sniff redirect, got: {resp}");

        let status = web.status();
        assert_eq!(status.urls, [format!("https://vela.example.ts.net:{port}")]);
        assert_eq!(status.bind.as_deref(), Some("loopback"));
        assert!(status.fingerprint.is_some(), "a TLS instance reports its fingerprint");

        for requested in [None, Some("192.168.1.5".to_string())] {
            let url = web.create_pairing(requested, false).unwrap().url;
            assert!(
                url.starts_with(&format!("https://vela.example.ts.net:{port}/#pair=")),
                "unexpected pairing link: {url}"
            );
        }
        {
            let guard = web.inner.lock().unwrap();
            let mode = guard.as_ref().unwrap().mode;
            assert!(
                crate::web::dispatch::CallOrigin::for_serve_mode(mode)
                    == crate::web::dispatch::CallOrigin::Remote,
                "a loopback-bound LAN listener must never treat its clients as local"
            );
        }
        web.stop();
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// A listen change on a running instance keeps the pairing token and the password verifier and
    /// moves the advertised host; an absent address fails and leaves the old instance running untouched.
    /// Every step binds 127.0.0.1 only.
    #[test]
    fn restart_listen_keeps_credentials_and_rejects_absent_address() {
        let (tmp, ctx) = temp_ctx("restart-listen");
        let web = WebServer::new();
        let port = start_loopback_lan_tls(&web, &ctx, None);
        let token = web.create_pairing(None, false).unwrap().device_token;
        assert_eq!(web.status().urls, [format!("https://127.0.0.1:{port}")]);

        let status = web
            .restart_listen(
                ctx.clone(),
                ListenConfig { bind: BindChoice::Loopback, pairing_host: Some("vela.example.ts.net".into()) },
            )
            .expect("restart on loopback with a pairing host");
        assert_eq!(status.port, Some(port), "the port is kept");
        assert_eq!(status.urls, [format!("https://vela.example.ts.net:{port}")]);
        assert_eq!(web.create_pairing(None, false).unwrap().device_token, token, "pairing token survives");
        {
            let guard = web.inner.lock().unwrap();
            let running = guard.as_ref().unwrap();
            assert!(running.auth.verify_password("pw"), "the password verifier survives");
        }

        let err = web
            .restart_listen(
                ctx.clone(),
                ListenConfig { bind: BindChoice::Address(v4("192.0.2.10")), pairing_host: None },
            )
            .unwrap_err();
        assert_eq!(err, "remote_bind_unavailable:192.0.2.10");
        let after = web.status();
        assert!(after.running, "the previous instance keeps running");
        assert_eq!(after.urls, [format!("https://vela.example.ts.net:{port}")]);
        assert_eq!(after.bind.as_deref(), Some("loopback"));

        web.stop();
        assert!(web.restart_listen(ctx, ListenConfig::default()).is_err(), "nothing to restart");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// A start on an absent address fails before anything is bound and never falls back to 0.0.0.0.
    #[test]
    fn start_with_absent_address_fails_without_fallback() {
        let (tmp, ctx) = temp_ctx("absent");
        let web = WebServer::new();
        let err = web
            .start_with_listen(
                ctx,
                StartAuth::Password("pw".into()),
                Some(free_port()),
                ServeMode::LanTls,
                ListenConfig { bind: BindChoice::Address(v4("192.0.2.10")), pairing_host: None },
            )
            .unwrap_err();
        assert_eq!(err, "remote_bind_unavailable:192.0.2.10");
        assert!(!web.status().running);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn release_identifier_is_production() {
        // Release-script identifiers end in .release and forbid plaintext LAN mode.
        assert!(is_production_identifier("io.vlinx.vlxterm.release"));
    }

    #[test]
    fn dev_identifier_is_not_production() {
        // Default development/test identifier permits plaintext LAN for mobile-device testing.
        assert!(!is_production_identifier("io.vlinx.vlxterm"));
    }

    #[test]
    fn server_identifier_is_production() {
        // Headless SSH server identifiers end in .server and also forbid plaintext LAN defensively.
        assert!(is_production_identifier("io.vlinx.vlxterm.server"));
    }

    #[test]
    fn network_and_broadcast_addresses_are_filtered() {
        let mask24 = Ipv4Addr::new(255, 255, 255, 0);
        // Filter network addresses whose host bits are all zero.
        assert!(is_network_or_broadcast(
            Ipv4Addr::new(192, 168, 97, 0),
            mask24
        ));
        // Filter broadcast addresses whose host bits are all one.
        assert!(is_network_or_broadcast(
            Ipv4Addr::new(192, 168, 97, 255),
            mask24
        ));
        // Keep normal host addresses.
        assert!(!is_network_or_broadcast(
            Ipv4Addr::new(192, 168, 88, 205),
            mask24
        ));
        // Keep a VM gateway .1 here; interface-name filtering handles it separately.
        assert!(!is_network_or_broadcast(
            Ipv4Addr::new(172, 16, 68, 1),
            mask24
        ));
        // Allow /32 point-to-point addresses common to VPNs.
        assert!(!is_network_or_broadcast(
            Ipv4Addr::new(10, 10, 10, 134),
            Ipv4Addr::new(255, 255, 255, 255)
        ));
        // Under /16, 192.168.0.0 is network while 192.168.97.0 is a valid host.
        let mask16 = Ipv4Addr::new(255, 255, 0, 0);
        assert!(is_network_or_broadcast(
            Ipv4Addr::new(192, 168, 0, 0),
            mask16
        ));
        assert!(!is_network_or_broadcast(
            Ipv4Addr::new(192, 168, 97, 0),
            mask16
        ));
    }

    #[test]
    fn cgnat_range_boundaries_are_exact() {
        // 100.64.0.0/10 (RFC 6598, Tailscale) is inside; its immediate neighbours are outside.
        assert!(!is_cgnat(Ipv4Addr::new(100, 63, 255, 255)));
        assert!(is_cgnat(Ipv4Addr::new(100, 64, 0, 0)));
        assert!(is_cgnat(Ipv4Addr::new(100, 100, 83, 2)));
        assert!(is_cgnat(Ipv4Addr::new(100, 127, 255, 255)));
        assert!(!is_cgnat(Ipv4Addr::new(100, 128, 0, 0)));
        // Ordinary private ranges are not CGNAT; they stay accepted via is_private().
        assert!(!is_cgnat(Ipv4Addr::new(192, 168, 1, 5)));
        assert!(!is_cgnat(Ipv4Addr::new(10, 0, 0, 5)));
    }

    /// Shorthand fixture for [`order_candidates`] tests; deliberately independent of real interfaces.
    fn cand(name: &str, ip: &str, vpn: bool) -> NetworkInterface {
        NetworkInterface {
            name: name.into(),
            ip: ip.into(),
            vpn,
        }
    }

    #[test]
    fn cgnat_candidates_rank_behind_lan_regardless_of_broadcast_capability() {
        // Worst-case input order: a broadcast-capable CGNAT entry first (the case the old
        // broadcast-only heuristic misplaced), then LAN, a non-CGNAT tunnel, and a CGNAT tunnel.
        let ordered = order_candidates(vec![
            cand("feth0", "100.100.83.2", false), // CGNAT with broadcast: must not count as LAN.
            cand("en0", "192.168.1.5", false),
            cand("utun1", "10.8.0.2", true), // Non-CGNAT VPN tunnel.
            cand("utun3", "100.101.0.7", true), // CGNAT VPN tunnel (typical Tailscale).
        ]);
        let ips: Vec<&str> = ordered.iter().map(|c| c.ip.as_str()).collect();
        assert_eq!(
            ips,
            ["192.168.1.5", "10.8.0.2", "100.100.83.2", "100.101.0.7"],
            "CGNAT must rank behind every non-CGNAT entry; broadcast-capable before tunnels within each group"
        );
    }

    #[test]
    fn order_candidates_is_stable_within_buckets() {
        // OS enumeration order is meaningful inside a bucket and must survive the ranking.
        let ordered = order_candidates(vec![
            cand("en0", "192.168.1.5", false),
            cand("en1", "10.0.0.5", false),
            cand("utun3", "100.100.83.2", true),
            cand("utun4", "100.100.83.3", true),
        ]);
        let ips: Vec<&str> = ordered.iter().map(|c| c.ip.as_str()).collect();
        assert_eq!(
            ips,
            ["192.168.1.5", "10.0.0.5", "100.100.83.2", "100.100.83.3"]
        );
    }

    #[test]
    fn virtual_interfaces_are_flagged() {
        // Exclude system, VM, container, and bridge interfaces, including host-side .1 NAT/Internet Sharing gateways.
        for n in [
            "awdl0",
            "llw0",
            "anpi0",
            "anpi3",
            "ap1",
            "vmnet1",
            "vmnet8",
            "vboxnet0",
            "vnic0",
            "docker0",
            "veth1a2b3c",
            "virbr0",
            "zt5u4i",
            "bridge0",
            "bridge100",
        ] {
            assert!(is_virtual_iface(n), "{n} should be treated as a virtual interface");
        }
        // Keep real LAN and VPN tunnel interfaces; lan_ips merely ranks VPNs lower.
        for n in ["en0", "en1", "eth0", "wlan0", "utun0", "ppp0"] {
            assert!(!is_virtual_iface(n), "{n} should not be treated as a virtual interface");
        }
    }
}
