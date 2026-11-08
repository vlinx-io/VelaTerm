//! Project tokens for a ZCode plan signed in through OAuth.
//!
//! Since ZCode 3.15, `zcode login` saves no plan API key. It keeps the platform's OAuth access token, and
//! before each model call exchanges it for a short-lived project token of the account's "zcode-api-key"
//! key. `zcode app-server` leaves that exchange to its host (the desktop app, or VelaTerm), so this module
//! does it the way ZCode 3.17's own resolver does:
//!
//! 1. Z.ai only: trade the OAuth token for a business token (`POST https://api.z.ai/api/auth/z/login`).
//! 2. Find where the key lives: the location ZCode saved in its credential store
//!    (`project-key-location:v1:<…>`), otherwise the account's default organization and project
//!    (`GET <origin>/api/biz/customer/getCustomerInfo`) and the key named `zcode-api-key` there, created
//!    when missing (`<origin>/api/biz/v1/organization/<org>/projects/<project>/api_keys`).
//! 3. Issue a project token for that key (`POST <api_keys>/<key id>/access_tokens`).
//!
//! Tokens are kept in memory until shortly before they expire, shared by every conversation and the quota
//! query of this process. Nothing here logs or stores a token.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const ZAI_ORIGIN: &str = "https://api.z.ai";
const BIGMODEL_ORIGIN: &str = "https://bigmodel.cn";
/// The organization and project names BigModel gives an account by default.
const DEFAULT_ORGANIZATION: &str = "默认机构";
const DEFAULT_PROJECT: &str = "默认项目";
const KEY_NAME: &str = "zcode-api-key";
/// `API_KEY_USAGE_SCENE.CODING_PLAN`.
const CODING_PLAN: u64 = 1;
/// What ZCode's agent bundle reports as its client version when run outside the desktop shell.
const CLIENT_VERSION: &str = "0.0.0-dev";
const TIMEOUT: Duration = Duration::from_secs(15);
/// A token is renewed this long before it expires, so a model call never starts with one about to lapse.
const RENEW_BEFORE: Duration = Duration::from_secs(150);

/// An OAuth sign-in to one platform, as ZCode saved it.
pub(crate) struct Login<'a> {
    /// `zai` or `bigmodel`.
    pub family: &'a str,
    /// The plan's account identity (`account-provider:<plan>:identity`).
    pub identity: &'a str,
    pub access_token: &'a str,
}

/// A project token and the id of the key it was issued for.
pub(crate) struct Material {
    pub token: String,
    pub api_key_id: String,
}

/// Why no token could be had. `SignedOut` means the platform no longer accepts the sign-in.
#[derive(Debug, PartialEq)]
pub(crate) enum TokenError {
    SignedOut,
    Disabled,
    Failed(String),
}

#[derive(Clone)]
struct Location {
    organization_id: String,
    project_id: String,
    api_key_id: String,
}

struct Issued {
    token: String,
    renew_at: Instant,
    expires_at: Instant,
}

#[derive(Default)]
struct Entry {
    location: Option<Location>,
    issued: Option<Issued>,
}

/// Keyed by origin, platform, identity and the OAuth token, so a new sign-in starts afresh.
static CACHE: Mutex<Option<HashMap<String, Entry>>> = Mutex::new(None);

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The fingerprint ZCode's server sends back for a token a model call rejected: its SHA-256 in hex.
pub(crate) fn fingerprint(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

/// The API origin a platform's keys live at. BigModel honors the override ZCode reads.
fn origin(family: &str) -> Result<String, TokenError> {
    match family {
        "zai" => Ok(ZAI_ORIGIN.to_string()),
        "bigmodel" => Ok(std::env::var("BIGMODEL_API_BASE_URL").ok()
            .map(|v| v.trim().trim_end_matches('/').to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| BIGMODEL_ORIGIN.to_string())),
        other => Err(TokenError::Failed(format!("unknown ZCode platform {other}"))),
    }
}

/// The credential-store key under which ZCode saves where a plan's key lives. 3.17 looks the default
/// project up; 3.15 left out team projects, which it named `non-team`.
pub(crate) fn location_keys(family: &str, identity: &str) -> Vec<String> {
    let Ok(origin) = origin(family) else { return Vec::new() };
    ["default", "non-team"].iter().map(|selection| {
        let parts = json!([origin, family, identity, null, CODING_PLAN, selection]).to_string();
        format!("project-key-location:v1:{}", crate::agent::zcode_account::encode_component(&parts))
    }).collect()
}

fn parse_location(saved: &str) -> Option<Location> {
    let v: Value = serde_json::from_str(saved).ok()?;
    let field = |name: &str| v.get(name).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    Some(Location {
        organization_id: field("organizationId")?,
        project_id: field("projectId")?,
        api_key_id: field("apiKeyId")?,
    })
}

enum Reply {
    Ok(Value),
    NotFound,
}

/// One request. Business endpoints wrap their answer as `{code: 0 | 200, data}`; `data` is returned.
fn request(method: &str, url: &str, auth: Option<&str>, body: Option<Value>) -> Result<Reply, TokenError> {
    let mut req = ureq::request(method, url)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .timeout(TIMEOUT);
    if let Some(auth) = auth {
        req = req.set("Authorization", auth);
    }
    let resp = match body {
        Some(body) => req.send_string(&body.to_string()),
        None => req.call(),
    };
    let text = match resp {
        Ok(resp) => resp.into_string().map_err(|e| TokenError::Failed(format!("{url}: {e}")))?,
        Err(ureq::Error::Status(404, _)) => return Ok(Reply::NotFound),
        Err(ureq::Error::Status(401, _)) => return Err(TokenError::SignedOut),
        Err(ureq::Error::Status(code, _)) => return Err(TokenError::Failed(format!("{url}: HTTP {code}"))),
        Err(e) => return Err(TokenError::Failed(format!("{url}: {e}"))),
    };
    let v: Value = serde_json::from_str(&text).map_err(|_| TokenError::Failed(format!("{url}: not JSON")))?;
    let ok = match v.get("code") {
        Some(Value::Number(n)) => matches!(n.as_u64(), Some(0 | 200)),
        Some(Value::String(s)) => s == "0" || s == "200",
        _ => false,
    };
    if !ok {
        return Err(TokenError::Failed(format!("{url}: refused ({})", v.get("code").map(Value::to_string).unwrap_or_default())));
    }
    Ok(Reply::Ok(v.get("data").cloned().unwrap_or(Value::Null)))
}

fn data(reply: Reply, url: &str) -> Result<Value, TokenError> {
    match reply {
        Reply::Ok(v) => Ok(v),
        Reply::NotFound => Err(TokenError::Failed(format!("{url}: HTTP 404"))),
    }
}

/// Z.ai's business token for an OAuth token.
fn business_login(access_token: &str) -> Result<String, TokenError> {
    let url = format!("{ZAI_ORIGIN}/api/auth/z/login");
    let v = data(request("POST", &url, None, Some(json!({"token": access_token})))?, &url)?;
    ["access_token", "accessToken"].iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty()))
        .map(str::to_string)
        .ok_or(TokenError::SignedOut)
}

fn keys_url(origin: &str, org: &str, project: &str) -> String {
    use crate::agent::zcode_account::encode_component as enc;
    format!("{origin}/api/biz/v1/organization/{}/projects/{}/api_keys", enc(org), enc(project))
}

/// The account's default project, and its `zcode-api-key` key, created when the project has none.
fn discover(origin: &str, family: &str, token: &str) -> Result<Location, TokenError> {
    let url = format!("{origin}/api/biz/customer/getCustomerInfo");
    let info = data(request("GET", &url, Some(token), None)?, &url)?;
    let orgs = info.get("organizations").and_then(Value::as_array).cloned().unwrap_or_default();
    let name = |v: &Value, key: &str, wanted: &str| v.get(key).and_then(Value::as_str).is_some_and(|n| n.contains(wanted));
    let org = orgs.iter().find(|o| name(o, "organizationName", DEFAULT_ORGANIZATION)).or(orgs.first());
    let projects = org.and_then(|o| o.get("projects")).and_then(Value::as_array).cloned().unwrap_or_default();
    let project = projects.iter().find(|p| name(p, "projectName", DEFAULT_PROJECT)).or(projects.first());
    let id = |v: Option<&Value>, key: &str| v.and_then(|v| v.get(key)).and_then(Value::as_str)
        .map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let (Some(organization_id), Some(project_id)) = (id(org, "organizationId"), id(project, "projectId")) else {
        return Err(TokenError::Failed("the account has no project to hold a ZCode key".into()));
    };
    let url = keys_url(origin, &organization_id, &project_id);
    let bearer = format!("Bearer {token}");
    let auth = if family == "zai" { bearer.as_str() } else { token };
    let keys = data(request("GET", &url, Some(auth), None)?, &url)?;
    let named = keys.as_array().into_iter().flatten()
        .find(|k| k.get("name").and_then(Value::as_str) == Some(KEY_NAME))
        .cloned();
    let key = match named {
        Some(key) => key,
        None => data(request("POST", &url, Some(auth), Some(json!({"name": KEY_NAME, "usageScene": CODING_PLAN})))?, &url)?,
    };
    let api_key_id = key.get("apiKey").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())
        .ok_or_else(|| TokenError::Failed("the ZCode key has no id".into()))?
        .to_string();
    Ok(Location { organization_id, project_id, api_key_id })
}

fn now_secs() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// A project token for the key at `at`, or `None` when the key is gone and has to be looked up again.
fn issue(origin: &str, token: &str, at: &Location) -> Result<Option<Issued>, TokenError> {
    let url = format!("{}/{}/access_tokens", keys_url(origin, &at.organization_id, &at.project_id),
        crate::agent::zcode_account::encode_component(&at.api_key_id));
    let started = Instant::now();
    let reply = request("POST", &url, Some(&format!("Bearer {token}")),
        Some(json!({"clientType": "zcode", "clientVersion": CLIENT_VERSION})))?;
    let Reply::Ok(v) = reply else { return Ok(None) };
    if v.get("enable").and_then(Value::as_bool) == Some(false) {
        return Err(TokenError::Disabled);
    }
    let token = v.get("accessToken").and_then(Value::as_str).map(str::trim).filter(|t| !t.is_empty())
        .ok_or_else(|| TokenError::Failed("the project token response has no token".into()))?;
    let expires_in = v.get("expiresIn").and_then(Value::as_f64).filter(|s| *s > 0.0);
    let expires_at = v.get("expiresAt").and_then(Value::as_f64).map(|at| at - now_secs()).filter(|s| *s > 0.0);
    let lifetime = match (expires_in, expires_at) {
        (Some(a), Some(b)) => a.min(b),
        (Some(s), None) | (None, Some(s)) => s,
        (None, None) => return Err(TokenError::Failed("the project token response has no expiry".into())),
    };
    let expires_at = started + Duration::from_secs_f64(lifetime);
    Ok(Some(Issued { token: token.to_string(), renew_at: expires_at.checked_sub(RENEW_BEFORE).unwrap_or(started), expires_at }))
}

/// A project token for `login`. `saved` reads ZCode's credential store; `rejected` is the fingerprint of a
/// token a model call was just refused with, which is then not handed out again.
pub(crate) fn resolve(login: &Login, saved: impl Fn(&str) -> Option<String>, rejected: Option<&str>)
    -> Result<Material, TokenError>
{
    let origin = origin(login.family)?;
    let access_token = login.access_token.trim().trim_start_matches("Bearer ").trim();
    if access_token.is_empty() || login.identity.trim().is_empty() {
        return Err(TokenError::SignedOut);
    }
    let scope = json!([origin, login.family, login.identity, fingerprint(access_token)]).to_string();
    let mut location = {
        let mut cache = CACHE.lock().unwrap();
        let entry = cache.get_or_insert_with(HashMap::new).entry(scope.clone()).or_default();
        if let Some(issued) = &entry.issued {
            if rejected.is_some_and(|r| r == fingerprint(&issued.token)) {
                entry.issued = None;
            }
        }
        if let (Some(issued), Some(at)) = (&entry.issued, &entry.location) {
            if Instant::now() < issued.renew_at {
                return Ok(Material { token: issued.token.clone(), api_key_id: at.api_key_id.clone() });
            }
        }
        entry.location.clone()
    };
    let fresh = (|| {
        let token = if login.family == "zai" { business_login(access_token)? } else { access_token.to_string() };
        if location.is_none() {
            location = location_keys(login.family, login.identity).iter()
                .find_map(|key| saved(key).as_deref().and_then(parse_location));
        }
        for attempt in 0..2 {
            let at = match location.clone() {
                Some(at) => at,
                None => discover(&origin, login.family, &token)?,
            };
            if let Some(issued) = issue(&origin, &token, &at)? {
                return Ok((at, issued));
            }
            // The key or project is gone; find the default one again, once.
            location = None;
            if attempt == 1 {
                break;
            }
        }
        Err(TokenError::Failed("the ZCode key could not be found".into()))
    })();
    let mut cache = CACHE.lock().unwrap();
    let entry = cache.get_or_insert_with(HashMap::new).entry(scope).or_default();
    match fresh {
        Ok((at, issued)) => {
            let material = Material { token: issued.token.clone(), api_key_id: at.api_key_id.clone() };
            entry.location = Some(at);
            entry.issued = Some(issued);
            Ok(material)
        }
        // A token that is due for renewal but still valid outlasts a network failure, as in ZCode.
        Err(TokenError::Failed(reason)) => match (&entry.issued, &entry.location) {
            (Some(issued), Some(at)) if Instant::now() < issued.expires_at
                && rejected.is_none_or(|r| r != fingerprint(&issued.token)) =>
            {
                Ok(Material { token: issued.token.clone(), api_key_id: at.api_key_id.clone() })
            }
            _ => Err(TokenError::Failed(reason)),
        },
        Err(other) => {
            entry.issued = None;
            Err(other)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_keys_match_what_zcode_saves() {
        // As found in a ZCode 3.15 credential store for a BigModel sign-in.
        let keys = location_keys("bigmodel", "key-9e90dd9c36eeef0ae48a6ef0");
        assert_eq!(keys[1], "project-key-location:v1:%5B%22https%3A%2F%2Fbigmodel.cn%22%2C%22bigmodel%22%2C%22key-9e90dd9c36eeef0ae48a6ef0%22%2Cnull%2C1%2C%22non-team%22%5D");
        assert!(keys[0].ends_with("%2C%22default%22%5D"));
    }

    #[test]
    fn saved_locations_need_all_three_ids() {
        let at = parse_location(r#"{"organizationId":"o","projectId":"p","apiKeyId":"k"}"#).unwrap();
        assert_eq!((at.organization_id.as_str(), at.project_id.as_str(), at.api_key_id.as_str()), ("o", "p", "k"));
        assert!(parse_location(r#"{"organizationId":"o","projectId":"p"}"#).is_none());
        assert!(parse_location("not json").is_none());
    }
}
