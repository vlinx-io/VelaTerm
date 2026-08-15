//! Local TLS-termination tunnel for remote-connection windows.
//!
//! Remote vlx-term services use self-signed HTTPS, while wry 0.55 WKWebView cannot handle certificate
//! challenges on macOS. Direct remote HTTPS navigation therefore produces a blank window.
//!
//! A byte-level loopback tunnel accepts plaintext TCP and connects to the remote endpoint over TLS
//! with a permissive certificate verifier. The window loads `http://127.0.0.1:<port>`; HTTP and WebSocket
//! pass through unchanged and the remote service still receives HTTPS/WSS. Frontend protocol selection
//! automatically uses ws:// on loopback.
//!
//! Security boundary: bind only to 127.0.0.1 and accept arbitrary certificates only for explicit user-
//! initiated remote connections, equivalent to proceeding past a self-signed warning. A process-wide
//! registry reuses one tunnel per host/port.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{timeout, Duration};
use tokio_rustls::rustls::{self, pki_types::ServerName};
use tokio_rustls::TlsConnector;

/// Timeout for individual remote TCP/TLS operations. Without it, macOS connect may block for about
/// 75 seconds during outages. Eight seconds closes the connection so browser WebSocket onclose can
/// promptly enter its next backoff attempt.
const TUNNEL_CONNECT_TIMEOUT: Duration = Duration::from_secs(8);

/// Process-lifetime registry mapping remote host/port pairs to local tunnel ports.
static TUNNELS: OnceLock<Mutex<HashMap<(String, u16), u16>>> = OnceLock::new();

/// Ensure a local tunnel to the remote endpoint and return its reused or newly allocated port.
pub fn ensure(remote_host: String, remote_port: u16) -> Result<u16, String> {
    let registry = TUNNELS.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (remote_host.clone(), remote_port);
    let mut map = registry.lock().unwrap();
    if let Some(port) = map.get(&key) {
        return Ok(*port);
    }
    let port = start(remote_host, remote_port)?;
    map.insert(key, port);
    Ok(port)
}

/// Connect once with permissive TLS, retrieve the server certificate, and compute an uppercase,
/// colon-delimited SHA-256 fingerprint for user review. The pairing link's public key remains the
/// primary man-in-the-middle trust anchor.
pub fn probe_fingerprint(remote_host: &str, remote_port: u16) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let server_name = ServerName::try_from(remote_host.to_string())
        .map_err(|e| format!("Remote hostname is not usable for TLS: {e}"))?;
    let mut conn = rustls::ClientConnection::new(client_config_accept_any()?, server_name)
        .map_err(|e| format!("TLS client init failed: {e}"))?;
    let mut sock = std::net::TcpStream::connect((remote_host, remote_port))
        .map_err(|e| format!("Failed to connect to remote: {e}"))?;
    let _ = sock.set_read_timeout(Some(std::time::Duration::from_secs(8)));
    let _ = sock.set_write_timeout(Some(std::time::Duration::from_secs(8)));
    // Drive the TLS handshake synchronously only to obtain the peer certificate; send no application data.
    while conn.is_handshaking() {
        conn.complete_io(&mut sock)
            .map_err(|e| format!("TLS handshake failed: {e}"))?;
    }
    let certs = conn
        .peer_certificates()
        .ok_or("Remote did not present a certificate")?;
    let leaf = certs.first().ok_or("Empty certificate chain")?;
    let digest = Sha256::digest(leaf.as_ref());
    Ok(digest
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":"))
}

/// Deterministically derive a local port in 20000..=39999 from host/remote_port with FNV-1a. Stable
/// loopback origins preserve localStorage layout across reconnects.
fn stable_tunnel_port(host: &str, remote_port: u16) -> u16 {
    let mut h: u32 = 2166136261;
    for b in host.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    for b in remote_port.to_le_bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    20000 + (h % 20000) as u16
}

/// Start a tunnel and return its local listening port. Bind synchronously for immediate caller use,
/// then run the accept loop on a background Tokio runtime.
fn start(remote_host: String, remote_port: u16) -> Result<u16, String> {
    let server_name = ServerName::try_from(remote_host.clone())
        .map_err(|e| format!("Remote hostname is not usable for TLS: {e}"))?;

    // Prefer the stable derived port so the loopback origin and localStorage layout survive reconnects.
    // If occupied, fall back to a random port and accept losing layout persistence for that run.
    let stable = stable_tunnel_port(&remote_host, remote_port);
    let std_listener = std::net::TcpListener::bind(("127.0.0.1", stable))
        .or_else(|_| std::net::TcpListener::bind(("127.0.0.1", 0)))
        .map_err(|e| format!("Failed to bind local tunnel port: {e}"))?;
    std_listener
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to set tunnel listener non-blocking: {e}"))?;
    let port = std_listener
        .local_addr()
        .map_err(|e| format!("Failed to get local tunnel port: {e}"))?
        .port();

    let connector = TlsConnector::from(client_config_accept_any()?);
    std::thread::Builder::new()
        .name(format!("vlx-tunnel-{remote_port}"))
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("failed to start tunnel runtime: {e}");
                    return;
                }
            };
            rt.block_on(async move {
                let listener = match TcpListener::from_std(std_listener) {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("failed to take over tunnel listener: {e}");
                        return;
                    }
                };
                loop {
                    let Ok((mut local, _)) = listener.accept().await else {
                        break;
                    };
                    let connector = connector.clone();
                    let server_name = server_name.clone();
                    let remote = (remote_host.clone(), remote_port);
                    tokio::spawn(async move {
                        // Both steps use timeout, yielding nested Result values: outer timeout and inner
                        // operation. Only Ok(Ok(_)) succeeds; timeout/connection failure closes this client.
                        let Ok(Ok(upstream)) =
                            timeout(TUNNEL_CONNECT_TIMEOUT, TcpStream::connect(remote)).await
                        else {
                            return;
                        };
                        let Ok(Ok(mut tls)) = timeout(
                            TUNNEL_CONNECT_TIMEOUT,
                            connector.connect(server_name, upstream),
                        )
                        .await
                        else {
                            return;
                        };
                        // Copy bytes bidirectionally until either side closes, without stopping the tunnel.
                        let _ = copy_bidirectional(&mut local, &mut tls).await;
                    });
                }
            });
        })
        .map_err(|e| format!("Failed to start tunnel thread: {e}"))?;

    Ok(port)
}

/// Build a rustls client accepting any server certificate. Select aws-lc-rs explicitly to match the
/// dependency tree without relying on process-global provider installation order.
fn client_config_accept_any() -> Result<Arc<rustls::ClientConfig>, String> {
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .map_err(|e| format!("Failed to configure TLS client: {e}"))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyServerCert { provider }))
        .with_no_client_auth();
    Ok(Arc::new(config))
}

/// Verifier that skips certificate-chain trust while retaining provider signature verification. Use
/// only for explicit connections to the user's self-signed vlx-term service.
#[derive(Debug)]
struct AcceptAnyServerCert {
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl rustls::client::danger::ServerCertVerifier for AcceptAnyServerCert {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// Find a free port by binding zero then releasing it; the small race is acceptable in tests.
    fn free_port() -> u16 {
        let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        l.local_addr().unwrap().port()
    }

    /// End-to-end test with a real self-signed headless WebServer: plaintext HTTP through the tunnel
    /// reaches `/api/me` as 401, and direct plaintext to the server receives a 301 HTTPS redirect.
    #[test]
    fn tunnel_and_sniff_end_to_end() {
        let tmp = std::env::temp_dir().join(format!("vlx-tunnel-e2e-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let db = crate::db::Db::open(&tmp.join("t.db")).unwrap();
        // `/api/me` does not access HookServer, so the host needs no hook initialization.
        let host = std::sync::Arc::new(crate::host::HeadlessHost::new(tmp.clone(), db));
        let ctx = crate::host::AppCtx::Headless(host);

        let web = crate::web::WebServer::new();
        let port = free_port();
        web.start(
            ctx,
            crate::web::StartAuth::Password("pw".into()),
            Some(port),
            crate::web::ServeMode::LanTls,
        )
        .expect("the web service should start");

        // Wait for the background service thread to finish binding after start's preflight.
        for _ in 0..50 {
            if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // 1. Tunnel: plaintext in, TLS out.
        let local = start("127.0.0.1".into(), port).expect("the tunnel should start");
        let mut s = std::net::TcpStream::connect(("127.0.0.1", local)).unwrap();
        s.write_all(b"GET /api/me HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        assert!(
            resp.starts_with("HTTP/1.1 401"),
            "the tunnel should return 401, got: {resp:.120}"
        );

        // 2. Direct plaintext to the HTTPS port redirects with 301 while preserving Host/path.
        let mut s2 = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        s2.write_all(b"GET /abc HTTP/1.1\r\nHost: 10.0.0.1:8799\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut resp2 = String::new();
        let _ = s2.read_to_string(&mut resp2);
        assert!(resp2.starts_with("HTTP/1.1 301"), "got: {resp2:.120}");
        assert!(
            resp2.contains("Location: https://10.0.0.1:8799/abc"),
            "got: {resp2:.300}"
        );

        web.stop();
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
