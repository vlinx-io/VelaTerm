//! End-to-end encryption (E2EE) for browser-based remote access.
//!
//! **Algorithm**: NaCl box (X25519 ECDH + XSalsa20-Poly1305), wire-compatible with tweetnacl-js.
//! The server persists a long-term key pair and sends its public key to the client in the URL fragment
//! of a QR-code or pairing link. Each side derives the same shared key from its private key and the
//! peer's public key, then encrypts every subsequent frame with that key.
//!
//! **Wire format (must match tweetnacl)**: each message is `24-byte random nonce ++ ciphertext`, with
//! the Poly1305 authentication tag included in the ciphertext. Text frames are then base64-encoded;
//! binary PTY frames are sent raw. This matches `encryptBytes` in Orca's `e2ee-crypto.ts`.
//!
//! **Handshake**, modeled after Orca's `e2ee-channel.ts` and driven synchronously by `ws.rs`:
//! 1. The client sends plaintext `e2ee_hello{publicKeyB64}`.
//! 2. The server derives a [`Cipher`] with [`ServerKeys::derive`] and replies with plaintext [`MSG_READY`].
//! 3. The client sends encrypted `e2ee_auth{deviceToken[,password]}`, parsed by [`parse_auth`].
//! 4. The server verifies it and replies with encrypted [`MSG_AUTHENTICATED`], or [`err_msg`] on failure.
//! Once ready, `ws.rs` uses [`Cipher`] to transparently encrypt and decrypt all application frames.

use std::path::Path;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use crypto_box::aead::{Aead, AeadCore, OsRng};
use crypto_box::{PublicKey, SalsaBox, SecretKey};
use serde::Deserialize;

/// Server key filename in the data directory, containing a base64-encoded 32-byte private key.
const KEYFILE: &str = "vlx-e2ee-key.b64";

/// Plaintext handshake frame indicating that the key is ready for encrypted authentication.
pub const MSG_READY: &str = "{\"type\":\"e2ee_ready\"}";
/// Encrypted handshake frame indicating successful authentication.
pub const MSG_AUTHENTICATED: &str = "{\"type\":\"e2ee_authenticated\"}";

/// Encrypted error frame for authentication failures.
pub fn err_msg(code: &str) -> String {
    format!("{{\"type\":\"e2ee_error\",\"code\":\"{code}\"}}")
}

/// Long-term server key pair and base64 public key embedded in pairing links.
pub struct ServerKeys {
    secret: SecretKey,
    public_b64: String,
}

impl ServerKeys {
    /// Load the server key or create and persist a random private key on first use.
    pub fn load_or_create(data_dir: &Path) -> Result<Self, String> {
        let path = data_dir.join(KEYFILE);
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(bytes) = B64.decode(text.trim()) {
                if let Ok(arr) = <[u8; 32]>::try_from(bytes.as_slice()) {
                    let secret = SecretKey::from(arr);
                    let public_b64 = B64.encode(secret.public_key().as_bytes());
                    return Ok(Self { secret, public_b64 });
                }
            }
            // Regenerate below when the key file is corrupt.
        }
        let secret = SecretKey::generate(&mut OsRng);
        let public_b64 = B64.encode(secret.public_key().as_bytes());
        // Create the key file owner-only from the start; chmod-after-write would leave a window in which
        // the private key exists with default umask permissions.
        super::write_owner_only(&path, B64.encode(secret.to_bytes()).as_bytes())
            .map_err(|e| format!("failed to write E2EE key: {e}"))?;
        Ok(Self { secret, public_b64 })
    }

    /// Base64 server public key for pairing links and QR codes.
    pub fn public_key_b64(&self) -> &str {
        &self.public_b64
    }

    /// Derive an ECDH shared key from the client's base64 public key and return a [`Cipher`].
    pub fn derive(&self, client_public_b64: &str) -> Result<Cipher, String> {
        let bytes = B64
            .decode(client_public_b64.trim())
            .map_err(|_| "client public key is invalid base64".to_string())?;
        let arr = <[u8; 32]>::try_from(bytes.as_slice())
            .map_err(|_| "client public key must be 32 bytes".to_string())?;
        let client_public = PublicKey::from(arr);
        Ok(Cipher {
            boxed: Arc::new(SalsaBox::new(&client_public, &self.secret)),
        })
    }
}

/// Symmetric cipher for one connection. Cloneable so the writer and reader can each own one.
#[derive(Clone)]
pub struct Cipher {
    boxed: Arc<SalsaBox>,
}

impl Cipher {
    /// Encrypt outbound text and return a base64 frame.
    pub fn encrypt_text(&self, plaintext: &str) -> Option<String> {
        self.seal(plaintext.as_bytes()).map(|b| B64.encode(b))
    }

    /// Encrypt outbound binary PTY output and return a raw frame.
    pub fn encrypt_bytes(&self, data: &[u8]) -> Option<Vec<u8>> {
        self.seal(data)
    }

    /// Decrypt an inbound text frame from base64 to plaintext bytes.
    pub fn decrypt_text(&self, b64: &str) -> Option<Vec<u8>> {
        let bundle = B64.decode(b64.trim()).ok()?;
        self.open(&bundle)
    }

    /// Decrypt an inbound binary frame. Kept for API symmetry and tests; clients currently send PTY
    /// input as text through `invoke pty_write`, so production code does not receive binary frames.
    #[allow(dead_code)]
    pub fn decrypt_bytes(&self, bundle: &[u8]) -> Option<Vec<u8>> {
        self.open(bundle)
    }

    /// Frame data as `nonce ++ ciphertext`.
    fn seal(&self, plaintext: &[u8]) -> Option<Vec<u8>> {
        let nonce = SalsaBox::generate_nonce(&mut OsRng);
        let ct = self.boxed.encrypt(&nonce, plaintext).ok()?;
        let mut out = Vec::with_capacity(nonce.len() + ct.len());
        out.extend_from_slice(nonce.as_slice());
        out.extend_from_slice(&ct);
        Some(out)
    }

    /// Split and decrypt `nonce ++ ciphertext`; return None for short input or failed authentication.
    fn open(&self, bundle: &[u8]) -> Option<Vec<u8>> {
        const NONCE: usize = 24;
        const TAG: usize = 16;
        if bundle.len() < NONCE + TAG {
            return None;
        }
        let (nonce_bytes, ct) = bundle.split_at(NONCE);
        let nonce = crypto_box::Nonce::from_slice(nonce_bytes);
        self.boxed.decrypt(nonce, ct).ok()
    }
}

/// Plaintext client hello carrying the client's public key.
#[derive(Deserialize)]
struct Hello {
    #[serde(rename = "publicKeyB64")]
    public_key_b64: String,
}

/// Extract the base64 client public key from an `e2ee_hello` text frame.
pub fn parse_hello(raw: &str) -> Option<String> {
    serde_json::from_str::<Hello>(raw)
        .ok()
        .map(|h| h.public_key_b64)
}

/// Decrypted client authentication: pairing token, optional second-factor password, and device metadata.
#[derive(Deserialize)]
struct Auth {
    #[serde(rename = "deviceToken")]
    device_token: String,
    #[serde(default)]
    password: Option<String>,
    /// Stable client-reported device ID for display and registration, not a security credential.
    #[serde(default, rename = "deviceId")]
    device_id: Option<String>,
    /// Client-reported device name used only for display and registration.
    #[serde(default, rename = "deviceName")]
    device_name: Option<String>,
}

/// Parse a decrypted `e2ee_auth` into its token, optional password, device ID, and device name.
pub fn parse_auth(
    plain: &[u8],
) -> Option<(String, Option<String>, Option<String>, Option<String>)> {
    serde_json::from_slice::<Auth>(plain)
        .ok()
        .map(|a| (a.device_token, a.password, a.device_id, a.device_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify a round trip in which client and server derive the same key, covering ECDH symmetry and
    /// the nonce/tag wire format.
    #[test]
    fn ecdh_roundtrip_text_and_bytes() {
        let tmp = std::env::temp_dir().join(format!("vlx-e2ee-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let server = ServerKeys::load_or_create(&tmp).unwrap();

        // Simulate a client with its own key pair and the server's public key.
        let client_secret = SecretKey::generate(&mut OsRng);
        let client_pub_b64 = B64.encode(client_secret.public_key().as_bytes());

        let server_cipher = server.derive(&client_pub_b64).unwrap();
        let server_pub_arr = {
            let bytes = B64.decode(server.public_key_b64()).unwrap();
            <[u8; 32]>::try_from(bytes.as_slice()).unwrap()
        };
        let client_cipher = Cipher {
            boxed: Arc::new(SalsaBox::new(
                &PublicKey::from(server_pub_arr),
                &client_secret,
            )),
        };

        // Server encrypts and client decrypts text.
        let frame = server_cipher.encrypt_text("hello-from-server").unwrap();
        let back = client_cipher.decrypt_text(&frame).unwrap();
        assert_eq!(back, b"hello-from-server");

        // Client encrypts and server decrypts binary data.
        let frame2 = client_cipher
            .encrypt_bytes(&[1u8, 2, 3, 255, 0, 42])
            .unwrap();
        let back2 = server_cipher.decrypt_bytes(&frame2).unwrap();
        assert_eq!(back2, vec![1u8, 2, 3, 255, 0, 42]);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Reloading from the same directory preserves the public key across restarts.
    #[test]
    fn keys_persist_across_loads() {
        let tmp = std::env::temp_dir().join(format!("vlx-e2ee-persist-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let a = ServerKeys::load_or_create(&tmp).unwrap();
        let b = ServerKeys::load_or_create(&tmp).unwrap();
        assert_eq!(a.public_key_b64(), b.public_key_b64());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Tampered ciphertext must fail decryption through Poly1305 authentication.
    #[test]
    fn tamper_is_rejected() {
        let tmp = std::env::temp_dir().join(format!("vlx-e2ee-tamper-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let server = ServerKeys::load_or_create(&tmp).unwrap();
        let client_secret = SecretKey::generate(&mut OsRng);
        let client_pub_b64 = B64.encode(client_secret.public_key().as_bytes());
        let server_cipher = server.derive(&client_pub_b64).unwrap();

        let mut frame = server_cipher.encrypt_bytes(b"secret").unwrap();
        let last = frame.len() - 1;
        frame[last] ^= 0xff; // Flip the final byte.
        assert!(server_cipher.decrypt_bytes(&frame).is_none());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Parse handshake messages.
    #[test]
    fn parse_handshake_messages() {
        let pk = parse_hello("{\"publicKeyB64\":\"abc\"}").unwrap();
        assert_eq!(pk, "abc");
        let (tok, pw, did, dn) = parse_auth(
            b"{\"deviceToken\":\"t1\",\"password\":\"p\",\"deviceId\":\"d1\",\"deviceName\":\"Mac\"}",
        )
        .unwrap();
        assert_eq!(tok, "t1");
        assert_eq!(pw.as_deref(), Some("p"));
        assert_eq!(did.as_deref(), Some("d1"));
        assert_eq!(dn.as_deref(), Some("Mac"));
        let (tok2, pw2, did2, _) = parse_auth(b"{\"deviceToken\":\"t2\"}").unwrap();
        assert_eq!(tok2, "t2");
        assert!(pw2.is_none());
        assert!(did2.is_none());
    }
}
