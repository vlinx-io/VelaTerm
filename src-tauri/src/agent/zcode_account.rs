//! The ZCode account as its protocol server expects to hear it from the client.
//!
//! `zcode app-server` does not read the account on its own. The desktop app tells it which plans the signed-in
//! account holds (`provider/updateAccountConfig`) and answers its request for the plan's API key before each
//! model call (`interaction/requestProviderRuntimeHeaders`). Until it hears, every plan counts as not
//! entitled, the model catalogue is empty and a message is refused, however recently `zcode login` saved the
//! account. VelaTerm plays that part from the store `zcode login` writes, read the way ZCode's own prompt
//! mode reads it: `~/.zcode/v2/credentials.json`, each value sealed with AES-256-GCM under the SHA-256 of
//! `ZCODE_CREDENTIAL_SECRET` or, by default, of the platform, home directory and user name. Keys go only to
//! the agent's stdin; nothing here logs or saves them.
//!
//! A plan signed in to with an API key keeps the key in the store. One signed in to through OAuth (every
//! `zcode login` since ZCode 3.15) keeps only the platform's access token, which `agent::zcode_token`
//! exchanges for a short-lived project token before each model call.
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
pub(crate) fn env_of(cmd: &Command, key: &str) -> Option<OsString> {
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

/// How a plan was signed in to: `manual` (an API key) or `oauth`.
fn auth_source_key(provider: &str) -> String {
    format!("account-provider:{provider}:auth-source")
}

/// The OAuth access token of a platform's sign-in.
fn access_token_key(family: &str) -> String {
    format!("oauth:{family}:access_token")
}

/// What a plan's model calls authenticate with.
enum Credential {
    /// The plan's API key, used as it is.
    Key(String),
    /// An OAuth sign-in, exchanged for a project token before each call.
    OAuth { family: String, identity: String, access_token: String },
}

/// JavaScript's `encodeURIComponent`, which ZCode applies to the identity inside a store key.
pub(crate) fn encode_component(value: &str) -> String {
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

    /// What a plan on `family` authenticates with, while that is the platform signed in to. As in ZCode, a
    /// `manual` plan needs its saved key and any other needs the platform's OAuth token. A store written
    /// before ZCode recorded the source (3.14 and earlier) saved the key itself, which is used when present.
    fn credential(&self, store: &HashMap<String, String>, provider: &str, family: &str) -> Option<Credential> {
        if self.platform(store).is_some_and(|platform| platform != family) {
            return None;
        }
        let identity = self.read(store, &identity_key(provider))?;
        let key = || self.read(store, &api_key_key(provider, &identity)).map(Credential::Key);
        let oauth = || self.read(store, &access_token_key(family)).map(|access_token| Credential::OAuth {
            family: family.to_string(), identity: identity.clone(), access_token,
        });
        match self.read(store, &auth_source_key(provider)).as_deref() {
            Some("manual") => key(),
            Some(_) => oauth(),
            None => key().or_else(oauth),
        }
    }

    /// The key a model call or quota query sends: the saved key, or a project token for an OAuth sign-in.
    fn request_key(&self, store: &HashMap<String, String>, credential: Credential, rejected: Option<&str>)
        -> Result<(String, Option<String>), crate::agent::zcode_token::TokenError>
    {
        match credential {
            Credential::Key(key) => Ok((key, None)),
            Credential::OAuth { family, identity, access_token } => {
                let login = crate::agent::zcode_token::Login { family: &family, identity: &identity, access_token: &access_token };
                let material = crate::agent::zcode_token::resolve(&login, |key| self.read(store, key), rejected)?;
                Ok((material.token, Some(material.api_key_id)))
            }
        }
    }

    /// The platform the account is signed in to (`zai` or `bigmodel`), or `None` when signed out.
    pub fn signed_in(&self) -> Option<String> {
        self.platform(&self.store())
    }

    /// The signed-in platform and the key of a plan the account holds there, for the account's quota query.
    /// `None` when signed out, when no plan on that platform is signed in to, or when no project token could
    /// be had for it. May call the platform; never call it on the UI thread.
    pub fn plan_api_key(&self) -> Option<(String, String)> {
        let config: Value = serde_json::from_slice(&std::fs::read(&self.builtin).ok()?).ok()?;
        let store = self.store();
        let platform = self.platform(&store)?;
        let credential = plans(&config).iter()
            .filter(|(_, family)| *family == platform)
            .find_map(|(id, family)| self.credential(&store, id, family))?;
        let (key, _) = self.request_key(&store, credential, None).ok()?;
        Some((platform, key))
    }

    /// The `provider/updateAccountConfig` parameters for the account as saved now, or `None` when the
    /// provider config cannot be read. A signed-out account still goes out, with no plan entitled.
    pub fn config(&self) -> Option<Value> {
        let config: Value = serde_json::from_slice(&std::fs::read(&self.builtin).ok()?).ok()?;
        let based_on = builtin_revision(&config, &self.builtin)?;
        let plans = plans(&config);
        let store = self.store();
        let entitled: Vec<String> = plans.iter()
            .filter(|(id, family)| self.credential(&store, id, family).is_some())
            .map(|(id, _)| id.clone())
            .collect();
        Some(account_params(&based_on, &plans, &entitled))
    }

    /// Whether answering `params` may call the platform, so the answer has to wait off the reader thread.
    pub fn runtime_headers_need_network(&self, params: &Value) -> bool {
        let provider = params.get("providerId").and_then(Value::as_str).unwrap_or("").trim();
        let family = params.pointer("/accountAccess/accountType").and_then(Value::as_str).unwrap_or("");
        matches!(self.credential(&self.store(), provider, family), Some(Credential::OAuth { .. }))
    }

    /// The answer to `interaction/requestProviderRuntimeHeaders`, as ZCode's prompt mode gives it: the plan's
    /// API key, or for an OAuth sign-in a project token with its key id and the account scope the server
    /// checks a retry against; otherwise a refusal the server reports on the turn. May call the platform.
    pub fn runtime_headers(&self, params: &Value) -> Value {
        const SIGN_IN: &str = "Sign in to ZCode again from the ZCode account menu to use this plan.";
        let refuse = |message: &str| json!({"headersApplied": false, "errorMessage": message});
        let provider = params.get("providerId").and_then(Value::as_str).unwrap_or("").trim();
        let mode = params.pointer("/accountAccess/mode").and_then(Value::as_str);
        let family = params.pointer("/accountAccess/accountType").and_then(Value::as_str).unwrap_or("");
        let store = self.store();
        let Some(credential) = (mode == Some(PLAN_MODE)).then(|| self.credential(&store, provider, family)).flatten() else {
            return refuse(SIGN_IN);
        };
        let scope = match &credential {
            Credential::OAuth { identity, access_token, .. } => Some(crate::agent::zcode_token::fingerprint(
                &json!([provider, family, identity, access_token]).to_string())),
            Credential::Key(_) => None,
        };
        let expected = params.get("expectedAccountScope").and_then(Value::as_str);
        let rejected = params.get("rejectedProjectTokenFingerprint").and_then(Value::as_str);
        // A retry after a refused token must stay on the same sign-in; a changed one starts a new turn.
        if (expected.is_some() || rejected.is_some()) && (expected.is_none() || expected != scope.as_deref()) {
            return refuse(SIGN_IN);
        }
        match self.request_key(&store, credential, rejected) {
            Ok((key, key_id)) => {
                let mut auth = json!({"apiKey": key});
                if let Some(id) = key_id {
                    auth["apiKeyId"] = json!(id);
                }
                if let Some(scope) = scope {
                    auth["accountScope"] = json!(scope);
                }
                json!({"headersApplied": true, "requestAuth": auth})
            }
            Err(error) => {
                use crate::agent::zcode_token::TokenError;
                crate::diagnostics::record("WARN", "zcode_project_token", json!({"family": family, "error": format!("{error:?}")}));
                refuse(match error {
                    TokenError::SignedOut => SIGN_IN,
                    TokenError::Disabled => "The ZCode API key of this account is disabled. Enable it on the platform, then try again.",
                    TokenError::Failed(_) => "ZCode could not authorize this plan with the platform. Check the network and try again.",
                })
            }
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

        // ZCode 3.15+ signs in through OAuth: the identity names the project, no key is saved, and the plan
        // counts while the platform's access token is there. A manual plan still needs its own key.
        let oauth = |source: &str, token: bool| {
            let mut store = json!({
                "oauth:active_provider": seal("bigmodel", &key, [5; 12]),
                identity_key(other): seal("key-9e90dd9c36eeef0ae48a6ef0", &key, [3; 12]),
                auth_source_key(other): seal(source, &key, [6; 12]),
                api_key_key(other, "44081764743722744"): seal("stale-key", &key, [4; 12]),
            });
            if token {
                store[access_token_key("bigmodel")] = json!(seal("oauth-token", &key, [7; 12]));
            }
            std::fs::write(dir.join(".zcode/v2/credentials.json"), store.to_string()).unwrap();
            account.config().unwrap()["providers"][other]["access"]["entitled"].clone()
        };
        assert_eq!(oauth("oauth", true), true);
        assert_eq!(oauth("oauth", false), false);
        assert_eq!(oauth("manual", true), false, "a manual plan without its key is not entitled");
        // A retry after a refused token must come from the same sign-in.
        oauth("oauth", true);
        let retry = account.runtime_headers(&json!({"providerId": other, "rejectedProjectTokenFingerprint": "0".repeat(64),
            "accountAccess": {"mode": PLAN_MODE, "accountType": "bigmodel"}}));
        assert_eq!(retry["headersApplied"], false);
        assert!(account.runtime_headers_need_network(&json!({"providerId": other,
            "accountAccess": {"mode": PLAN_MODE, "accountType": "bigmodel"}})));

        // Without both config paths the server reads its own refreshed copy, whose revision is not known.
        let mut partial = Command::new("zcode");
        partial.env(BUILTIN_CONFIG, &builtin).env(PERSONAL_CONFIG, "");
        assert!(Account::for_command(&partial).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
