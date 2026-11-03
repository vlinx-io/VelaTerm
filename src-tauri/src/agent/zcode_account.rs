//! The ZCode account as its protocol server expects to hear it from the client.
//!
//! `zcode app-server` does not read the account on its own. The desktop app tells it which plans the signed-in
//! account holds (`provider/updateAccountConfig`) and answers its request for the plan's API key before each
//! model call (`interaction/requestProviderRuntimeHeaders`). Until it hears, every plan counts as not
//! entitled, the model catalogue is empty and a message is refused, however recently `zcode login` saved the
//! account. VelaTerm plays that part from the store `zcode login` writes, read the way ZCode's own prompt mode
//! reads it: `~/.zcode/v2/credentials.json`, each value sealed with AES-256-GCM under the SHA-256 of
//! `ZCODE_CREDENTIAL_SECRET` or, by default, of the platform, home directory and user name. Keys go only to
//! the agent's stdin; nothing here logs or keeps them.
//!
//! The server applies an account only when it names the provider config revision the server loaded. That
//! revision is `zcode-builtin:<revision in the file>:<SHA-256 of the file's path>`, so it is known only when
//! the agent was started with its config paths set (`agent::zcode::command` does so); otherwise ZCode reads
//! a refreshed copy of its own and no account is sent.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use base64::Engine as _;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

/// The provider config file the server reads its built-in providers from.
pub(crate) const BUILTIN_CONFIG: &str = "ZCODE_BUILTIN_PROVIDER_CONFIG_FILE";
/// The user's own provider settings; given together with the built-in file, ZCode reads both as they are.
pub(crate) const PERSONAL_CONFIG: &str = "ZCODE_PERSONAL_PROVIDER_CONFIG_FILE";
/// The plan access `zcode login` signs in to; team and off-peak plans are granted through the desktop app.
const PLAN_MODE: &str = "individual-coding-plan";
const SEALED: &str = "enc:v1:";

/// Where one agent process finds the account, and the provider config it was started with.
pub struct Account {
    credentials: PathBuf,
    /// Candidate keys for the store; a wrong one fails authentication, so trying each is safe.
    keys: Vec<[u8; 32]>,
    builtin: PathBuf,
}

/// A variable as the command will pass it: its own setting, otherwise what this process inherited.
fn env_of(cmd: &Command, key: &str) -> Option<OsString> {
    match cmd.get_envs().find(|(k, _)| *k == key) {
        Some((_, value)) => value.map(|v| v.to_os_string()),
        None => std::env::var_os(key),
    }
    .filter(|v| !v.to_string_lossy().trim().is_empty())
}

/// Node's `os.platform()`, which the default key includes.
fn node_platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

/// The names Node's `os.userInfo().username` may report: the account database's entry first, then the
/// environment's, which differ only on an unusual setup.
fn user_names() -> Vec<String> {
    let mut names = Vec::new();
    #[cfg(unix)]
    unsafe {
        let mut entry: libc::passwd = std::mem::zeroed();
        let mut found: *mut libc::passwd = std::ptr::null_mut();
        let mut buf = vec![0 as libc::c_char; 4096];
        if libc::getpwuid_r(libc::geteuid(), &mut entry, buf.as_mut_ptr(), buf.len(), &mut found) == 0
            && !found.is_null()
            && !entry.pw_name.is_null()
        {
            names.push(std::ffi::CStr::from_ptr(entry.pw_name).to_string_lossy().into_owned());
        }
    }
    for key in ["USER", "LOGNAME", "USERNAME"] {
        if let Some(name) = std::env::var(key).ok().filter(|n| !n.is_empty()) {
            names.push(name);
        }
    }
    names.dedup();
    if names.is_empty() {
        names.push("unknown".into());
    }
    names
}

fn key_of(secret: &str) -> [u8; 32] {
    Sha256::digest(secret.as_bytes()).into()
}

/// Open one sealed value: `enc:v1:<iv>.<tag>.<ciphertext>`, each part base64url without padding. A value that
/// is not sealed is stored as it is.
fn open(value: &str, keys: &[[u8; 32]]) -> Option<String> {
    use aes_gcm::aead::{Aead, KeyInit};
    let Some(sealed) = value.strip_prefix(SEALED) else { return Some(value.to_string()) };
    let decode = |part: &str| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(part.trim_end_matches('=')).ok();
    let mut parts = sealed.split('.');
    let (iv, tag, data) = (decode(parts.next()?)?, decode(parts.next()?)?, decode(parts.next()?)?);
    if parts.next().is_some() || iv.len() != 12 || tag.len() != 16 {
        return None;
    }
    let nonce = aes_gcm::Nonce::try_from(iv.as_slice()).ok()?;
    let mut payload = data;
    payload.extend_from_slice(&tag);
    keys.iter().find_map(|key| {
        let cipher = aes_gcm::Aes256Gcm::new_from_slice(key).ok()?;
        cipher.decrypt(&nonce, payload.as_slice()).ok()
    })
    .and_then(|plain| String::from_utf8(plain).ok())
}

/// The store keys `zcode login` writes for one plan.
fn identity_key(provider: &str) -> String {
    format!("account-provider:{provider}:identity")
}

fn api_key_key(provider: &str, identity: &str) -> String {
    format!("account-provider:coding-plan:{provider}:account:{}:api-key", encode_component(identity))
}

/// JavaScript's `encodeURIComponent`, which ZCode applies to the identity inside a store key.
fn encode_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The plans a `zcode login` can hold, as the built-in config declares them, with the platform each is on
/// (`zai` or `bigmodel`).
fn plans(config: &Value) -> Vec<(String, String)> {
    let rules = config.pointer("/config/providerConfigRules/providerRules").and_then(Value::as_array);
    rules.into_iter().flatten().filter_map(|rule| {
        let id = rule.get("providerId").and_then(Value::as_str)?.trim();
        let access = rule.pointer("/config/access")?;
        let has_model = rule.pointer("/config/builtinModelIds").and_then(Value::as_array)
            .is_some_and(|ids| ids.iter().filter_map(Value::as_str).any(|m| !m.trim().is_empty()));
        let family = access.get("accountType").and_then(Value::as_str).filter(|t| !t.is_empty())?;
        let plan = access.get("type").and_then(Value::as_str) == Some("zhipu-account")
            && access.get("mode").and_then(Value::as_str) == Some(PLAN_MODE);
        (plan && has_model && !id.is_empty()).then(|| (id.to_string(), family.to_string()))
    })
    .collect()
}

/// The revision the server derives for a built-in config file it read at `path`.
fn builtin_revision(config: &Value, path: &Path) -> Option<String> {
    let revision = config.get("revision").and_then(Value::as_u64)?;
    let path = std::path::absolute(path).ok()?;
    let digest = Sha256::digest(path.to_string_lossy().as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    Some(format!("zcode-builtin:{revision}:{hex}"))
}

/// The `provider/updateAccountConfig` request for a set of plans and the ones the account holds.
fn account_params(based_on: &str, plans: &[(String, String)], entitled: &[String]) -> Value {
    let providers: Map<String, Value> = plans.iter().map(|(id, _)| {
        (id.clone(), json!({"access": {"type": "zhipu-account", "entitled": entitled.contains(id)}}))
    }).collect();
    let states: Map<String, Value> = entitled.iter().map(|id| {
        (id.clone(), json!({"availability": "available", "entitled": true, "current": true}))
    }).collect();
    let mut seen = entitled.to_vec();
    seen.sort();
    json!({
        "revision": format!("velaterm:{based_on}:{}", seen.join(",")),
        "basedOnZCodeBuiltinRevision": based_on,
        "providers": providers,
        "states": states,
    })
}

impl Account {
    /// The account for an agent `cmd` starts, once its environment is final. `None` when the command does not
    /// pin both provider config files, so the revision the server loads is not known.
    pub fn for_command(cmd: &Command) -> Option<Account> {
        let builtin = PathBuf::from(env_of(cmd, BUILTIN_CONFIG)?);
        env_of(cmd, PERSONAL_CONFIG)?;
        let base = env_of(cmd, "ZCODE_DATA_BASE_DIR").map(PathBuf::from).or_else(crate::host::home_dir)?;
        let keys = match env_of(cmd, "ZCODE_CREDENTIAL_SECRET") {
            Some(secret) => vec![key_of(secret.to_string_lossy().trim())],
            None => {
                let home = env_of(cmd, if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                    .map(PathBuf::from)
                    .or_else(crate::host::home_dir)?;
                user_names().iter().map(|user| {
                    key_of(&format!("zcode-credential-fallback:{}:{}:{user}", node_platform(), home.display()))
                }).collect()
            }
        };
        Some(Account { credentials: base.join(".zcode").join("v2").join("credentials.json"), keys, builtin })
    }

    /// The store as saved now: a sign-in elsewhere shows up on the next read.
    fn store(&self) -> HashMap<String, String> {
        std::fs::read(&self.credentials).ok()
            .and_then(|bytes| serde_json::from_slice::<HashMap<String, Value>>(&bytes).ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(key, value)| Some((key, value.as_str()?.to_string())))
            .collect()
    }

    fn read(&self, store: &HashMap<String, String>, key: &str) -> Option<String> {
        open(store.get(key)?, &self.keys).map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
    }

    /// The platform the latest sign-in used. Signing in to the other one switches it, and the plans of the
    /// platform left behind stay saved but no longer count; signing out clears it.
    fn platform(&self, store: &HashMap<String, String>) -> Option<String> {
        self.read(store, "oauth:active_provider").filter(|p| p == "zai" || p == "bigmodel")
    }

    /// The key of a plan on `family`, while that is the platform signed in to.
    fn plan_key(&self, store: &HashMap<String, String>, provider: &str, family: &str) -> Option<String> {
        if self.platform(store).is_some_and(|platform| platform != family) {
            return None;
        }
        let identity = self.read(store, &identity_key(provider))?;
        self.read(store, &api_key_key(provider, &identity))
    }

    /// The platform the account is signed in to (`zai` or `bigmodel`), or `None` when signed out.
    pub fn signed_in(&self) -> Option<String> {
        self.platform(&self.store())
    }

    /// The signed-in platform and the key of a plan the account holds there, for the account's quota query.
    /// `None` when signed out or when no plan on that platform has a saved key.
    pub fn plan_api_key(&self) -> Option<(String, String)> {
        let config: Value = serde_json::from_slice(&std::fs::read(&self.builtin).ok()?).ok()?;
        let store = self.store();
        let platform = self.platform(&store)?;
        plans(&config).iter()
            .filter(|(_, family)| *family == platform)
            .find_map(|(id, family)| self.plan_key(&store, id, family))
            .map(|key| (platform, key))
    }

    /// The `provider/updateAccountConfig` parameters for the account as saved now, or `None` when the
    /// provider config cannot be read. A signed-out account still goes out, with no plan entitled.
    pub fn config(&self) -> Option<Value> {
        let config: Value = serde_json::from_slice(&std::fs::read(&self.builtin).ok()?).ok()?;
        let based_on = builtin_revision(&config, &self.builtin)?;
        let plans = plans(&config);
        let store = self.store();
        let entitled: Vec<String> = plans.iter()
            .filter(|(id, family)| self.plan_key(&store, id, family).is_some())
            .map(|(id, _)| id.clone())
            .collect();
        Some(account_params(&based_on, &plans, &entitled))
    }

    /// The answer to `interaction/requestProviderRuntimeHeaders`: the plan's API key, as ZCode's prompt mode
    /// supplies it, or a refusal the server reports on the turn.
    pub fn runtime_headers(&self, params: &Value) -> Value {
        let provider = params.get("providerId").and_then(Value::as_str).unwrap_or("").trim();
        let mode = params.pointer("/accountAccess/mode").and_then(Value::as_str);
        let family = params.pointer("/accountAccess/accountType").and_then(Value::as_str).unwrap_or("");
        let key = (mode == Some(PLAN_MODE)).then(|| self.plan_key(&self.store(), provider, family)).flatten();
        match key {
            Some(key) => json!({"headersApplied": true, "requestAuth": {"apiKey": key}}),
            None => json!({"headersApplied": false, "errorMessage": "Sign in to ZCode to use this plan."}),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seal(plain: &str, key: &[u8; 32], iv: [u8; 12]) -> String {
        use aes_gcm::aead::{Aead, KeyInit};
        let cipher = aes_gcm::Aes256Gcm::new_from_slice(key).unwrap();
        let mut out = cipher.encrypt(&aes_gcm::Nonce::try_from(iv.as_slice()).unwrap(), plain.as_bytes()).unwrap();
        let tag = out.split_off(out.len() - 16);
        let b64 = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        format!("{SEALED}{}.{}.{}", b64(&iv), b64(&tag), b64(&out))
    }

    #[test]
    fn sealed_values_open_with_the_right_key_only() {
        let right = key_of("secret");
        let sealed = seal("197804ef-9598", &right, [7; 12]);
        assert_eq!(open(&sealed, &[key_of("other"), right]).as_deref(), Some("197804ef-9598"));
        assert_eq!(open(&sealed, &[key_of("other")]), None);
        assert_eq!(open("plain", &[]).as_deref(), Some("plain"));
        assert_eq!(open("enc:v1:AAAA.BBBB", &[right]), None);
    }

    #[test]
    fn identities_are_encoded_like_encode_uri_component() {
        assert_eq!(encode_component("197804ef-9598_x.y"), "197804ef-9598_x.y");
        assert_eq!(encode_component("a:b/c d"), "a%3Ab%2Fc%20d");
        assert_eq!(
            api_key_key("account:zai-individual-coding-plan", "u:1"),
            "account-provider:coding-plan:account:zai-individual-coding-plan:account:u%3A1:api-key"
        );
    }

    #[test]
    fn the_account_names_the_loaded_config_and_the_plans_it_holds() {
        let dir = std::env::temp_dir().join(format!("vlx-zcode-account-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".zcode").join("v2")).unwrap();
        let builtin = dir.join("zcode-builtin.json");
        let plan = |id: &str, mode: &str| json!({"providerId": id, "config": {
            "builtinModelIds": ["GLM-5.3"], "access": {"type": "zhipu-account", "mode": mode,
                "accountType": if id.contains("bigmodel") { "bigmodel" } else { "zai" }}}});
        std::fs::write(&builtin, json!({"schemaVersion": 1, "revision": 30, "config": {"providerConfigRules": {"providerRules": [
            plan("account:zai-individual-coding-plan", PLAN_MODE),
            plan("account:bigmodel-individual-coding-plan", PLAN_MODE),
            plan("account:zai-team-coding-plan", "team-coding-plan"),
        ]}}}).to_string()).unwrap();
        let secret = "test-secret";
        let key = key_of(secret);
        let id = "account:zai-individual-coding-plan";
        let other = "account:bigmodel-individual-coding-plan";
        let save = |platform: &str| std::fs::write(dir.join(".zcode/v2/credentials.json"), json!({
            "oauth:active_provider": seal(platform, &key, [5; 12]),
            identity_key(id): seal("user-1", &key, [1; 12]),
            api_key_key(id, "user-1"): seal("plan-key", &key, [2; 12]),
            identity_key(other): seal("user-2", &key, [3; 12]),
            api_key_key(other, "user-2"): seal("other-key", &key, [4; 12]),
        }).to_string()).unwrap();
        save("zai");

        let mut cmd = Command::new("zcode");
        cmd.env(BUILTIN_CONFIG, &builtin).env(PERSONAL_CONFIG, dir.join("provider_config.json"))
            .env("ZCODE_DATA_BASE_DIR", &dir).env("ZCODE_CREDENTIAL_SECRET", secret);
        let account = Account::for_command(&cmd).unwrap();
        let params = account.config().unwrap();
        let digest: String = Sha256::digest(builtin.to_string_lossy().as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(params["basedOnZCodeBuiltinRevision"], format!("zcode-builtin:30:{digest}"));
        assert_eq!(account.signed_in().as_deref(), Some("zai"));
        assert_eq!(params["providers"][id]["access"]["entitled"], true);
        // Saved from an earlier sign-in to the other platform; only the platform signed in to counts.
        assert_eq!(params["providers"][other]["access"]["entitled"], false);
        assert!(params["providers"].get("account:zai-team-coding-plan").is_none(), "only plans `zcode login` grants");
        assert_eq!(params["states"][id]["current"], true);
        assert!(params["states"].get(other).is_none());

        let ask = |provider: &str, mode: &str, family: &str| account.runtime_headers(
            &json!({"providerId": provider, "accountAccess": {"mode": mode, "accountType": family}}));
        assert_eq!(ask(id, PLAN_MODE, "zai"), json!({"headersApplied": true, "requestAuth": {"apiKey": "plan-key"}}));
        assert_eq!(ask(other, PLAN_MODE, "bigmodel")["headersApplied"], false);
        assert_eq!(ask(id, "off-peak", "zai")["headersApplied"], false);

        // Signing in to BigModel switches platforms without signing out of Z.ai first.
        save("bigmodel");
        let params = account.config().unwrap();
        assert_eq!(params["providers"][id]["access"]["entitled"], false);
        assert_eq!(params["providers"][other]["access"]["entitled"], true);
        assert_eq!(ask(other, PLAN_MODE, "bigmodel")["requestAuth"]["apiKey"], "other-key");
        assert_eq!(ask(id, PLAN_MODE, "zai")["headersApplied"], false);

        // Signed out: the platform marker is gone, and so is every plan.
        std::fs::write(dir.join(".zcode/v2/credentials.json"), "{}").unwrap();
        assert_eq!(account.signed_in(), None);
        assert_eq!(account.config().unwrap()["states"], json!({}));

        // Without both config paths the server reads its own refreshed copy, whose revision is not known.
        let mut partial = Command::new("zcode");
        partial.env(BUILTIN_CONFIG, &builtin).env(PERSONAL_CONFIG, "");
        assert!(Account::for_command(&partial).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
