//! Outbound-only public conversation sharing. The relay receives ciphertext and routing metadata.

use super::{
    e2ee::{Cipher, ServerKeys},
    share_policy::ShareScope,
};
use crate::host::AppCtx;
use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalShare {
    #[serde(flatten)]
    scope: ShareScope,
    secret: String,
    password_hash: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    origin: String,
    device_id: String,
    token: String,
    shares: HashMap<String, LocalShare>,
}
struct Linking {
    origin: String,
    code: String,
    poll_token: String,
}
static LINKING: OnceLock<Mutex<HashMap<PathBuf, Linking>>> = OnceLock::new();
static RUNNING: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
static FILE_LOCK: Mutex<()> = Mutex::new(());
static AUTH_BUDGET: OnceLock<Mutex<HashMap<String, (Instant, u32)>>> = OnceLock::new();
const CONFIG: &str = "vlx-public-sharing.json";

fn random() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
fn load(app: &AppCtx) -> Result<Config, String> {
    let bytes = std::fs::read(app.data_dir()?.join(CONFIG))
        .map_err(|_| "Link this device to your account first")?;
    serde_json::from_slice(&bytes).map_err(|_| "Invalid public sharing configuration".into())
}
fn save(app: &AppCtx, config: &Config) -> Result<(), String> {
    let path = app.data_dir()?.join(CONFIG);
    let pending = path.with_extension("new");
    super::write_owner_only(
        &pending,
        &serde_json::to_vec(config).map_err(|_| "Cannot encode sharing configuration")?,
    )
    .map_err(|_| "Cannot save sharing configuration")?;
    std::fs::rename(pending, path).map_err(|_| "Cannot save sharing configuration".into())
}
fn request(
    origin: &str,
    path: &str,
    token: Option<&str>,
    body: Option<&Value>,
) -> Result<Value, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .redirects(0)
        .build();
    let mut req = agent.request(
        if body.is_some() { "POST" } else { "GET" },
        &format!("{origin}{path}"),
    );
    if let Some(token) = token {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }
    let response = if let Some(body) = body {
        req.set("Content-Type", "application/json")
            .send_string(&body.to_string())
    } else {
        req.call()
    }
    .map_err(|_| "Public account service is unavailable or rejected the request")?;
    use std::io::Read;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(3 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read relay response")?;
    if bytes.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(&bytes).map_err(|_| "Invalid public account service response".into())
}

pub fn dispatch(app: &AppCtx, cmd: &str, args: &Value) -> Result<Value, String> {
    match cmd {
        "public_account_status" => {
            if let Ok(config) = load(app) {
                let status = request(
                    &config.origin,
                    "/api/device-link/host/status",
                    Some(&config.token),
                    None,
                );
                if status.as_ref().is_ok_and(|value| value["linked"] == false) {
                    return Ok(json!({"linked":false,"origin":config.origin}));
                }
                start(app)?;
                Ok(json!({"linked":true,"deviceId":config.device_id,"origin":config.origin}))
            } else {
                Ok(json!({"linked":false,"origin":"https://velaterm.com"}))
            }
        }
        "public_account_link" => {
            let origin = "https://velaterm.com".to_string();
            let keys = ServerKeys::load_or_create(&app.data_dir()?)?;
            let name = args
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("VelaTerm");
            let result = request(
                &origin,
                "/api/device-link",
                None,
                Some(&json!({"name":name,"publicKey":keys.public_key_b64()})),
            )?;
            let get = |key: &str| {
                result
                    .get(key)
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .ok_or("Invalid device linking response")
            };
            let url = get("url")?;
            LINKING
                .get_or_init(Default::default)
                .lock()
                .map_err(|_| "Device linking unavailable")?
                .insert(
                    app.data_dir()?,
                    Linking {
                        origin,
                        code: get("code")?,
                        poll_token: get("pollToken")?,
                    },
                );
            Ok(json!({"url":url,"publicKey":keys.public_key_b64()}))
        }
        "public_account_poll" => {
            let mut attempts = LINKING
                .get_or_init(Default::default)
                .lock()
                .map_err(|_| "Device linking unavailable")?;
            let dir = app.data_dir()?;
            let attempt = attempts.get(&dir).ok_or("Start account linking first")?;
            let result = request(
                &attempt.origin,
                &format!("/api/device-link/{}/poll", attempt.code),
                Some(&attempt.poll_token),
                Some(&json!({})),
            )?;
            if result.is_null() {
                return Ok(json!({"linked":false}));
            }
            let config = Config {
                origin: attempt.origin.clone(),
                device_id: result["device"]["id"]
                    .as_str()
                    .ok_or("Invalid device ID")?
                    .into(),
                token: result["token"]
                    .as_str()
                    .ok_or("Invalid device credential")?
                    .into(),
                shares: HashMap::new(),
            };
            let _lock = FILE_LOCK
                .lock()
                .map_err(|_| "Sharing configuration unavailable")?;
            save(app, &config)?;
            attempts.remove(&dir);
            start(app)?;
            Ok(json!({"linked":true}))
        }
        "public_share_create" => {
            let _lock = FILE_LOCK
                .lock()
                .map_err(|_| "Sharing configuration unavailable")?;
            let mut config = load(app)?;
            let scope: ShareScope =
                serde_json::from_value(args.clone()).map_err(|_| "Invalid sharing scope")?;
            // Validate against host-owned records before asking the public service to create the share.
            if scope
                .dispatch(app, "shared_sessions", &json!({}))?
                .as_array()
                .is_none_or(|s| s.is_empty())
            {
                return Err("The selected scope contains no agent conversations".into());
            }
            let password = args
                .get("password")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty());
            if password.is_some_and(|p| p.len() < 8 || p.len() > 256) {
                return Err("Use a password between 8 and 256 characters".into());
            }
            let mut input = args.clone();
            let map = input.as_object_mut().ok_or("Invalid share")?;
            map.remove("password");
            map.insert("deviceId".into(), json!(config.device_id));
            map.insert("passwordRequired".into(), json!(password.is_some()));
            let result = request(
                &config.origin,
                "/api/device-link/host/shares",
                Some(&config.token),
                Some(&input),
            )?;
            let id = result["id"].as_str().ok_or("Invalid share response")?;
            let secret = random();
            let password_hash = password
                .map(|p| {
                    Argon2::default()
                        .hash_password(p.as_bytes(), &SaltString::generate(&mut OsRng))
                        .map(|h| h.to_string())
                        .map_err(|_| "Cannot protect share password")
                })
                .transpose()?;
            config.shares.insert(
                id.into(),
                LocalShare {
                    scope,
                    secret: secret.clone(),
                    password_hash,
                },
            );
            save(app, &config)?;
            start(app)?;
            let keys = ServerKeys::load_or_create(&app.data_dir()?)?;
            let fragment =
                URL_SAFE_NO_PAD.encode(json!({"k":keys.public_key_b64(),"s":secret}).to_string());
            Ok(json!({"url":format!("{}/s/{}#key={}",config.origin,id,fragment),"id":id}))
        }
        "public_share_links" => {
            let config = load(app)?;
            let current = request(
                &config.origin,
                "/api/device-link/host/shares",
                Some(&config.token),
                None,
            )?;
            let keys = ServerKeys::load_or_create(&app.data_dir()?)?;
            let links = current.as_array().ok_or("Invalid share list")?.iter().filter_map(|item| {
                let id = item["id"].as_str()?;
                let share = config.shares.get(id)?;
                let fragment = URL_SAFE_NO_PAD.encode(json!({"k":keys.public_key_b64(),"s":share.secret}).to_string());
                Some(json!({"id":id,"scope":share.scope.scope,"url":format!("{}/s/{}#key={}",config.origin,id,fragment)}))
            }).collect::<Vec<_>>();
            Ok(json!(links))
        }
        "public_share_options" => {
            let config = load(app)?;
            let options = request(&config.origin, "/api/account/sharing-options", None, None)?;
            let tree = crate::command_core::list_tree(app)?;
            let sessions = ShareScope {
                scope: "machine".into(),
                target_id: None,
            }
            .dispatch(app, "shared_sessions", &json!({}))?;
            Ok(
                json!({"options":options,"sessions":sessions,"projects":tree.projects.iter().map(|p|json!({"id":p.id,"name":p.name})).collect::<Vec<_>>()}),
            )
        }
        _ => Err("Unknown public sharing command".into()),
    }
}

struct Peer {
    cipher: Cipher,
    challenge: String,
    next: u64,
    authed: bool,
    created: Instant,
    share_id: String,
    transfer: Option<Transfer>,
}

const MAX_RESPONSE: usize = 16 * 1024 * 1024;
const CHUNK_SIZE: usize = 256 * 1024;
struct Transfer {
    id: String,
    parts: Vec<String>,
    next: usize,
    created: Instant,
}
impl Transfer {
    fn take(&mut self, args: &Value) -> Result<Value, String> {
        if self.created.elapsed() >= Duration::from_secs(120)
            || args["id"].as_str() != Some(self.id.as_str())
            || args["index"].as_u64() != Some(self.next as u64)
            || self.next >= self.parts.len()
        {
            return Err("Invalid or expired response chunk".into());
        }
        let part = std::mem::take(&mut self.parts[self.next]);
        self.next += 1;
        Ok(json!(part))
    }
}

fn response_value(peer: &mut Peer, value: Value) -> Result<Value, String> {
    let bytes = serde_json::to_vec(&value).map_err(|_| "Cannot encode shared response")?;
    if bytes.len() > MAX_RESPONSE {
        return Err("Shared response exceeds the 16 MiB limit".into());
    }
    if bytes.len() <= 1024 * 1024 {
        return Ok(json!({"result":value}));
    }
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let parts: Vec<String> = encoded
        .as_bytes()
        .chunks(CHUNK_SIZE)
        .map(|part| String::from_utf8(part.to_vec()).expect("Base64 is ASCII"))
        .collect();
    let transfer = Transfer {
        id: random(),
        parts,
        next: 0,
        created: Instant::now(),
    };
    let result = json!({"chunked":{"id":transfer.id,"parts":transfer.parts.len()}});
    peer.transfer = Some(transfer);
    Ok(result)
}

pub fn start(app: &AppCtx) -> Result<(), String> {
    load(app)?;
    let dir = app.data_dir()?;
    let mut running = RUNNING
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "Relay unavailable")?;
    if !running.insert(dir.clone()) {
        return Ok(());
    }
    let app = app.clone();
    std::thread::Builder::new()
        .name("public-relay".into())
        .spawn(move || {
            run(&app);
            if let Ok(mut running) = RUNNING.get_or_init(Default::default).lock() {
                running.remove(&dir);
            }
        })
        .map_err(|_| "Cannot start public relay")?;
    Ok(())
}

fn run(app: &AppCtx) {
    let Ok(dir) = app.data_dir() else { return };
    let Ok(keys) = ServerKeys::load_or_create(&dir) else {
        return;
    };
    let mut peers: HashMap<String, Peer> = HashMap::new();
    loop {
        let Ok(config) = load(app) else { return };
        let frame = match request(
            &config.origin,
            "/api/relay/host/poll",
            Some(&config.token),
            None,
        ) {
            Ok(value) => value,
            Err(_) => {
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
        };
        for peer in peers.values_mut() {
            if peer
                .transfer
                .as_ref()
                .is_some_and(|t| t.created.elapsed() >= Duration::from_secs(120))
            {
                peer.transfer = None;
            }
        }
        if frame.is_null() {
            continue;
        }
        peers.retain(|_, p| p.created.elapsed() < Duration::from_secs(3600));
        let response = process(app, &config, &keys, &mut peers, &frame).unwrap_or_else(|_| {
            json!({"error":"Shared conversation request rejected"}).to_string()
        });
        let _ = request(
            &config.origin,
            "/api/relay/host/reply",
            Some(&config.token),
            Some(&json!({"requestId":frame["requestId"],"payload":response})),
        );
    }
}

fn process(
    app: &AppCtx,
    config: &Config,
    keys: &ServerKeys,
    peers: &mut HashMap<String, Peer>,
    frame: &Value,
) -> Result<String, String> {
    let channel = frame["channel"].as_str().ok_or("Missing channel")?;
    let id = frame["share"]["id"].as_str().ok_or("Missing share")?;
    let share = config.shares.get(id).ok_or("Unknown share")?;
    // Relay grants cannot broaden a scope registered by the host.
    if frame["share"]["scope"].as_str() != Some(&share.scope.scope)
        || frame["share"]["targetId"].as_str() != share.scope.target_id.as_deref()
    {
        return Err("Scope mismatch".into());
    }
    let payload = frame["payload"].as_str().ok_or("Missing payload")?;
    if let Ok(hello) = serde_json::from_str::<Value>(payload) {
        if hello["type"] == "hello" {
            if peers.len() >= 16 && !peers.contains_key(channel) {
                // Closed browser channels leave no host notification. Reclaim the oldest peer;
                // a still-open visitor can establish a fresh authenticated connection.
                if let Some(oldest) = peers
                    .iter()
                    .min_by_key(|(_, p)| p.created)
                    .map(|(id, _)| id.clone())
                {
                    peers.remove(&oldest);
                }
            }
            let cipher = keys.derive(hello["publicKey"].as_str().ok_or("Missing public key")?)?;
            let challenge = random();
            let response = cipher
                .encrypt_text(&json!({"challenge":challenge}).to_string())
                .ok_or("Encryption failed")?;
            peers.insert(
                channel.into(),
                Peer {
                    cipher,
                    challenge,
                    next: 0,
                    authed: false,
                    created: Instant::now(),
                    share_id: id.into(),
                    transfer: None,
                },
            );
            return Ok(response);
        }
    }
    let peer = peers.get_mut(channel).ok_or("Handshake required")?;
    if peer.share_id != id {
        return Err("Share mismatch".into());
    }
    let bytes = peer
        .cipher
        .decrypt_text(payload)
        .ok_or("Invalid encrypted frame")?;
    let message: Value = serde_json::from_slice(&bytes).map_err(|_| "Invalid message")?;
    let seq = message["seq"].as_u64().ok_or("Missing sequence")?;
    if message["challenge"].as_str() != Some(&peer.challenge) || seq != peer.next {
        return Err("Replayed frame".into());
    }
    peer.next = peer.next.checked_add(1).ok_or("Sequence exhausted")?;
    let result = (|| {
        if !peer.authed {
            {
                let mut budgets = AUTH_BUDGET
                    .get_or_init(Default::default)
                    .lock()
                    .map_err(|_| "Authentication unavailable")?;
                budgets.retain(|_, (created, _)| created.elapsed() < Duration::from_secs(60));
                let budget = budgets.entry(id.into()).or_insert((Instant::now(), 0));
                if budget.1 >= 20 {
                    return Err("Too many authentication attempts; try again later".into());
                }
                budget.1 += 1;
            }
            use sha2::{Digest, Sha256};
            let actual = message["secret"].as_str().unwrap_or("");
            if Sha256::digest(actual.as_bytes()) != Sha256::digest(share.secret.as_bytes()) {
                return Err("Invalid link secret".to_string());
            }
            if let Some(hash) = &share.password_hash {
                let parsed = PasswordHash::new(hash).map_err(|_| "Invalid password verifier")?;
                Argon2::default()
                    .verify_password(
                        message["password"].as_str().unwrap_or("").as_bytes(),
                        &parsed,
                    )
                    .map_err(|_| "Invalid share password")?;
            }
            peer.authed = true;
        }
        let cmd = message["cmd"].as_str().ok_or("Missing command")?;
        if cmd == "_relay_chunk" {
            let transfer = peer.transfer.as_mut().ok_or("No pending response")?;
            let value = transfer.take(&message["args"])?;
            if transfer.next == transfer.parts.len() {
                peer.transfer = None;
            }
            return Ok(json!({"result":value}));
        }
        peer.transfer = None;
        let value = share.scope.dispatch(app, cmd, &message["args"])?;
        response_value(peer, value)
    })();
    let output = match result {
        Ok(mut value) => {
            value["challenge"] = json!(peer.challenge);
            value["seq"] = json!(seq);
            value["ok"] = json!(true);
            value
        }
        Err(error) => json!({"challenge":peer.challenge,"seq":seq,"ok":false,"error":error}),
    };
    peer.cipher
        .encrypt_text(&output.to_string())
        .ok_or("Encryption failed".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::HeadlessHost;
    use std::sync::Arc;

    /// Opt-in fixture for testing the real browser/relay/host path without starting any AI process.
    #[test]
    #[ignore]
    fn browser_fixture() {
        let Some(dir) = std::env::var_os("VELATERM_RELAY_BROWSER_FIXTURE").map(PathBuf::from)
        else {
            return;
        };
        assert!(dir.is_absolute());
        let db = crate::db::Db::open(&dir.join("fixture.db")).unwrap();
        {
            let conn = db.conn.lock().unwrap();
            let project =
                crate::db::repo::create_virtual_project(&conn, "Shared test project").unwrap();
            for (name, kind) in [
                ("Conversation A", crate::models::SessionKind::Claude),
                ("Conversation B", crate::models::SessionKind::Codex),
                ("Private terminal", crate::models::SessionKind::Terminal),
            ] {
                let session = crate::db::repo::create_session(
                    &conn,
                    &project.id,
                    None,
                    name,
                    kind,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap();
                if kind != crate::models::SessionKind::Terminal {
                    crate::db::repo::set_session_engine(&conn, &session.id, "chat").unwrap();
                }
            }
        }
        let app = AppCtx::Headless(Arc::new(HeadlessHost::new(dir.clone(), db)));
        start(&app).unwrap();
        println!("Public relay browser fixture ready");
        for _ in 0..600 {
            if dir.join("stop").exists() {
                return;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    #[test]
    fn encrypted_channel_rejects_replay_tampering_terminal_and_scope_changes() {
        let dir = std::env::temp_dir().join(format!(
            "velaterm-public-relay-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(dir.join("client")).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let app = AppCtx::Headless(Arc::new(HeadlessHost::new(dir.clone(), db)));
        let host = ServerKeys::load_or_create(&dir).unwrap();
        let guest = ServerKeys::load_or_create(&dir.join("client")).unwrap();
        let cipher = guest.derive(host.public_key_b64()).unwrap();
        let scope = ShareScope {
            scope: "machine".into(),
            target_id: None,
        };
        let mut config = Config {
            origin: "https://velaterm.com".into(),
            device_id: "device".into(),
            token: "test".into(),
            shares: HashMap::from([(
                "share".into(),
                LocalShare {
                    scope,
                    secret: "secret".into(),
                    password_hash: None,
                },
            )]),
        };
        let mut peers = HashMap::new();
        let mut frame = json!({"channel":"channel","share":{"id":"share","scope":"machine","targetId":null},
            "payload":json!({"type":"hello","publicKey":guest.public_key_b64()}).to_string()});
        let response = process(&app, &config, &host, &mut peers, &frame).unwrap();
        let hello: Value =
            serde_json::from_slice(&cipher.decrypt_text(&response).unwrap()).unwrap();
        let challenge = hello["challenge"].as_str().unwrap();
        let encrypted=cipher.encrypt_text(&json!({"challenge":challenge,"seq":0,"secret":"secret","cmd":"shared_sessions","args":{}}).to_string()).unwrap();
        frame["payload"] = json!(encrypted);
        let response = process(&app, &config, &host, &mut peers, &frame).unwrap();
        let clear: Value =
            serde_json::from_slice(&cipher.decrypt_text(&response).unwrap()).unwrap();
        assert_eq!(clear["ok"], true);
        assert_eq!(clear["result"], json!([]));
        assert!(
            process(&app, &config, &host, &mut peers, &frame).is_err(),
            "Replayed messages must not execute again"
        );
        frame["payload"] = json!(format!("!{encrypted}"));
        assert!(
            process(&app, &config, &host, &mut peers, &frame).is_err(),
            "Tampered frames must fail authentication"
        );
        frame["payload"]=json!(cipher.encrypt_text(&json!({"challenge":challenge,"seq":1,"cmd":"pty_write","args":{"sessionId":"any","data":"touch /tmp/never"}}).to_string()).unwrap());
        let response = process(&app, &config, &host, &mut peers, &frame).unwrap();
        let clear: Value =
            serde_json::from_slice(&cipher.decrypt_text(&response).unwrap()).unwrap();
        assert_eq!(
            clear["ok"], false,
            "Direct terminal commands must be denied after valid E2EE authentication"
        );
        frame["share"]["scope"] = json!("project");
        assert!(
            process(&app, &config, &host, &mut peers, &frame).is_err(),
            "Relay metadata cannot broaden host grants"
        );
        let peer = peers.get_mut("channel").unwrap();
        let large = json!({"text":"长会话 🖼️\n".repeat(150_000)});
        let manifest = response_value(peer, large.clone()).unwrap();
        let transfer = peer.transfer.as_mut().unwrap();
        let id = manifest["chunked"]["id"].as_str().unwrap();
        assert!(transfer.take(&json!({"id":id,"index":1})).is_err());
        assert!(transfer.take(&json!({"id":"other","index":0})).is_err());
        let mut encoded = String::new();
        for index in 0..manifest["chunked"]["parts"].as_u64().unwrap() {
            let part = transfer.take(&json!({"id":id,"index":index})).unwrap();
            assert!(part.as_str().unwrap().len() <= CHUNK_SIZE);
            encoded.push_str(part.as_str().unwrap());
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), large);
        assert!(transfer.take(&json!({"id":id,"index":0})).is_err());
        transfer.next = 0;
        transfer.created = Instant::now() - Duration::from_secs(121);
        assert!(transfer.take(&json!({"id":id,"index":0})).is_err());
        assert!(response_value(peer, json!("x".repeat(MAX_RESPONSE))).is_err());
        config.shares.get_mut("share").unwrap().password_hash = Some(
            Argon2::default()
                .hash_password(b"fixture-password", &SaltString::generate(&mut OsRng))
                .unwrap()
                .to_string(),
        );
        frame["share"]["scope"] = json!("machine");
        frame["payload"] =
            json!(json!({"type":"hello","publicKey":guest.public_key_b64()}).to_string());
        let response = process(&app, &config, &host, &mut peers, &frame).unwrap();
        let hello: Value =
            serde_json::from_slice(&cipher.decrypt_text(&response).unwrap()).unwrap();
        for (seq, password, accepted) in
            [(0, "wrong-password", false), (1, "fixture-password", true)]
        {
            frame["payload"] = json!(cipher.encrypt_text(&json!({
                "challenge":hello["challenge"],"seq":seq,"secret":"secret","password":password,
                "cmd":"shared_sessions","args":{}
            }).to_string()).unwrap());
            let response = process(&app, &config, &host, &mut peers, &frame).unwrap();
            let clear: Value =
                serde_json::from_slice(&cipher.decrypt_text(&response).unwrap()).unwrap();
            assert_eq!(
                clear["ok"], accepted,
                "Password must be checked on the host"
            );
        }
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
