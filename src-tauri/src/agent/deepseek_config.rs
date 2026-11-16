//! DeepSeek's API endpoint and key, as configured in the conversation.
//!
//! DeepSeek conversations are driven by OpenCode's local server. OpenCode can find credentials on its
//! own (`opencode auth login` or `DEEPSEEK_API_KEY`), but the conversation view offers its own settings
//! instead, so a key can be entered once without leaving the conversation. The endpoint may also be
//! pointed at a compatible proxy; leaving it empty keeps the provider's official address.
//!
//! The key prefers the system keychain, falling back to plaintext `app_settings` when no keyring is
//! available, the same storage policy as the Gitea token. It is never returned to the UI: status carries
//! only whether one is stored.

use serde::Serialize;

use crate::db::{repo, Db};

/// Plaintext `app_settings` key for the endpoint override.
pub const KEY_BASE_URL: &str = "deepseek.base_url";
/// Plaintext fallback key used when keyring storage is unavailable.
pub const KEY_API_KEY_FALLBACK: &str = "deepseek.api_key";
/// Fixed keyring account name; the service name carries the application identifier.
const KEYRING_ACCOUNT: &str = "api_key";

/// Identifier-scoped keyring service name separating development and release credentials.
fn keyring_service(identifier: &str) -> String {
    format!("{identifier}.deepseek")
}

/// Configuration state for the API panel: the endpoint and whether a key exists, never the key.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Configured endpoint without a trailing slash; None uses the provider's official address.
    pub base_url: Option<String>,
    /// Whether a key is stored.
    pub has_key: bool,
}

/// Trim an endpoint and reject anything that is not a plain http(s) URL.
///
/// Credentials in the URL, query strings, and fragments are refused because the endpoint is written
/// into the agent's configuration as-is and would otherwise carry a second, hidden secret.
fn normalize_base_url(raw: &str) -> Result<Option<String>, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let url = url::Url::parse(raw).map_err(|_| "deepseek_invalid_url".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("deepseek_invalid_url".into());
    }
    Ok(Some(url.as_str().trim_end_matches('/').to_string()))
}

/// Trim a key and reject values that cannot travel in a single configuration line.
fn normalize_key(raw: &str) -> Result<Option<String>, String> {
    let key = raw.trim();
    if key.is_empty() {
        return Ok(None);
    }
    if key.chars().any(|c| c == '\n' || c == '\r') || key.len() > 8192 {
        return Err("deepseek_invalid_key".into());
    }
    Ok(Some(key.to_string()))
}

/// Store or delete the key, clearing the plaintext fallback after a successful keyring write.
fn store_key(db: &Db, identifier: &str, key: Option<&str>) -> Result<(), String> {
    let service = keyring_service(identifier);
    let keyring_ok = keyring::Entry::new(&service, KEYRING_ACCOUNT)
        .and_then(|entry| match key {
            Some(value) => entry.set_password(value),
            None => {
                // Deletion tolerates an entry that is already absent.
                let _ = entry.delete_credential();
                Ok(())
            }
        })
        .is_ok();

    let conn = db.conn.lock().unwrap();
    if keyring_ok {
        repo::delete_app_setting(&conn, KEY_API_KEY_FALLBACK)
    } else {
        match key {
            Some(value) => {
                let mut entries = std::collections::HashMap::new();
                entries.insert(KEY_API_KEY_FALLBACK.to_string(), value.to_string());
                repo::set_app_settings(&conn, &entries)
            }
            None => repo::delete_app_setting(&conn, KEY_API_KEY_FALLBACK),
        }
    }
}

/// Read the key from the keyring, then the plaintext fallback.
fn load_key(db: &Db, identifier: &str) -> Option<String> {
    let service = keyring_service(identifier);
    if let Ok(entry) = keyring::Entry::new(&service, KEYRING_ACCOUNT) {
        if let Ok(key) = entry.get_password() {
            let key = key.trim().to_string();
            if !key.is_empty() {
                return Some(key);
            }
        }
    }
    let conn = db.conn.lock().unwrap();
    repo::get_app_settings(&conn)
        .ok()
        .and_then(|settings| settings.get(KEY_API_KEY_FALLBACK).cloned())
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

/// The configured endpoint, without a trailing slash.
fn get_base_url(db: &Db) -> Option<String> {
    let conn = db.conn.lock().unwrap();
    repo::get_app_settings(&conn)
        .ok()
        .and_then(|settings| settings.get(KEY_BASE_URL).cloned())
        .map(|url| url.trim().trim_end_matches('/').to_string())
        .filter(|url| !url.is_empty())
}

/// Current configuration for the API panel.
pub fn status(db: &Db, identifier: &str) -> Status {
    Status { base_url: get_base_url(db), has_key: load_key(db, identifier).is_some() }
}

/// Everything a DeepSeek OpenCode server needs to reach the API, for one launch.
pub fn load(db: &Db, identifier: &str) -> (Option<String>, Option<String>) {
    (get_base_url(db), load_key(db, identifier))
}

/// Save the endpoint and key from the API panel.
///
/// An empty key preserves the existing one because stored keys are never echoed into the form; passing
/// `clear_key` removes it instead. An empty endpoint restores the provider's official address.
pub fn set_config(
    db: &Db,
    identifier: &str,
    base_url: &str,
    api_key: &str,
    clear_key: bool,
) -> Result<Status, String> {
    let base_url = normalize_base_url(base_url)?;
    let api_key = normalize_key(api_key)?;
    {
        let conn = db.conn.lock().unwrap();
        match &base_url {
            Some(url) => {
                let mut entries = std::collections::HashMap::new();
                entries.insert(KEY_BASE_URL.to_string(), url.clone());
                repo::set_app_settings(&conn, &entries)?;
            }
            None => repo::delete_app_setting(&conn, KEY_BASE_URL)?,
        }
    }
    if clear_key {
        store_key(db, identifier, None)?;
    } else if let Some(key) = api_key.as_deref() {
        store_key(db, identifier, Some(key))?;
    }
    Ok(status(db, identifier))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_are_plain_http_urls_without_trailing_slashes() {
        assert_eq!(normalize_base_url("").unwrap(), None);
        assert_eq!(normalize_base_url("  ").unwrap(), None);
        assert_eq!(
            normalize_base_url(" https://api.deepseek.com/ ").unwrap(),
            Some("https://api.deepseek.com".into())
        );
        assert_eq!(
            normalize_base_url("http://127.0.0.1:18789/v1/").unwrap(),
            Some("http://127.0.0.1:18789/v1".into())
        );
        for bad in [
            "api.deepseek.com",
            "ftp://api.deepseek.com",
            "https://user:secret@api.deepseek.com",
            "https://api.deepseek.com/?key=secret",
            "https://api.deepseek.com/#fragment",
        ] {
            assert_eq!(normalize_base_url(bad), Err("deepseek_invalid_url".into()), "{bad}");
        }
    }

    #[test]
    fn keys_must_be_single_line() {
        assert_eq!(normalize_key("").unwrap(), None);
        assert_eq!(normalize_key(" sk-test ").unwrap(), Some("sk-test".into()));
        assert!(normalize_key("sk-a\nsk-b").is_err());
        assert!(normalize_key(&"x".repeat(8193)).is_err());
    }
}
