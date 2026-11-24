//! Gitea integration: a worktree landing provider for Gitea pull requests and token storage.
//!
//! - **Token storage**: prefer macOS Keychain or Windows Credential Manager through keyring, falling back
//!   to plaintext `app_settings` when unavailable. Service names follow development/release identifiers
//!   so credentials remain isolated. Tauri and Electron's Rust sidecar share this implementation.
//! - **Base URL storage**: nonsensitive plaintext `gitea.base_url` in `app_settings`.
//! - **Provider detection**: compare the worktree origin host with the configured base URL, then verify
//!   through `/api/v1/version`.
//! - **HTTP**: use the existing ureq dependency.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::db::{repo, Db};
use crate::git;

/// Plaintext app_settings key for the Gitea base URL.
const KEY_BASE_URL: &str = "gitea.base_url";
/// Plaintext fallback key used when keyring storage is unavailable.
const KEY_TOKEN_FALLBACK: &str = "gitea.token";
/// Fixed keyring account name; the service name carries the application identifier.
const KEYRING_ACCOUNT: &str = "token";

/// Identifier-scoped keyring service name separating development and release credentials.
fn keyring_service(identifier: &str) -> String {
    format!("{identifier}.gitea")
}

// ─────────────────────────── Token storage: keyring first, plaintext fallback ───────────────────────────

/// Store a token in the keyring or plaintext app_settings fallback. An empty token deletes both. After a
/// successful keyring write, clear the fallback to avoid stale or inconsistent plaintext credentials.
pub fn store_token(db: &Db, identifier: &str, token: &str) -> Result<(), String> {
    let service = keyring_service(identifier);
    let token = token.trim();
    let keyring_ok = keyring::Entry::new(&service, KEYRING_ACCOUNT)
        .and_then(|e| {
            if token.is_empty() {
                // Deletion tolerates an entry that is already absent.
                let _ = e.delete_credential();
                Ok(())
            } else {
                e.set_password(token)
            }
        })
        .is_ok();

    let conn = db.conn.lock().unwrap();
    if keyring_ok {
        // The keyring is authoritative, so remove the plaintext fallback.
        repo::delete_app_setting(&conn, KEY_TOKEN_FALLBACK)?;
        Ok(())
    } else if token.is_empty() {
        repo::delete_app_setting(&conn, KEY_TOKEN_FALLBACK)
    } else {
        let mut m = std::collections::HashMap::new();
        m.insert(KEY_TOKEN_FALLBACK.to_string(), token.to_string());
        repo::set_app_settings(&conn, &m)
    }
}

/// Read the token from keyring, then plaintext app_settings; return None when neither exists.
pub fn load_token(db: &Db, identifier: &str) -> Option<String> {
    let service = keyring_service(identifier);
    if let Ok(entry) = keyring::Entry::new(&service, KEYRING_ACCOUNT) {
        if let Ok(t) = entry.get_password() {
            let t = t.trim().to_string();
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    let conn = db.conn.lock().unwrap();
    repo::get_app_settings(&conn)
        .ok()
        .and_then(|m| m.get(KEY_TOKEN_FALLBACK).cloned())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// ─────────────────────────── Base URL configuration ───────────────────────────

/// Read the configured base URL without trailing slashes, or None when unset.
pub fn get_base_url(db: &Db) -> Option<String> {
    let conn = db.conn.lock().unwrap();
    repo::get_app_settings(&conn)
        .ok()
        .and_then(|m| m.get(KEY_BASE_URL).cloned())
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
}

/// Store a base URL without trailing slashes; an empty value deletes it.
fn set_base_url(db: &Db, url: &str) -> Result<(), String> {
    let url = url.trim().trim_end_matches('/');
    let conn = db.conn.lock().unwrap();
    if url.is_empty() {
        repo::delete_app_setting(&conn, KEY_BASE_URL)
    } else {
        let mut m = std::collections::HashMap::new();
        m.insert(KEY_BASE_URL.to_string(), url.to_string());
        repo::set_app_settings(&conn, &m)
    }
}

/// Save base URL and token from the settings card. An empty token preserves the existing secret because
/// stored tokens are never echoed into the form. Call [`store_token`] explicitly with empty to clear it.
pub fn set_config(db: &Db, identifier: &str, base_url: &str, token: &str) -> Result<(), String> {
    set_base_url(db, base_url)?;
    if !token.trim().is_empty() {
        store_token(db, identifier, token)?;
    }
    Ok(())
}

/// Current Gitea integration state for the settings card.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GiteaStatus {
    /// Whether both base URL and token are configured.
    pub configured: bool,
    /// Configured base URL, or None.
    pub base_url: Option<String>,
    /// Whether a token exists, without returning the secret itself.
    pub has_token: bool,
}

/// Read Gitea configuration status without exposing plaintext token data.
pub fn status(db: &Db, identifier: &str) -> GiteaStatus {
    let base_url = get_base_url(db);
    let has_token = load_token(db, identifier).is_some();
    GiteaStatus {
        configured: base_url.is_some() && has_token,
        base_url,
        has_token,
    }
}

// ─────────────────────────── Probing and detection ───────────────────────────

/// Test Connection result from probing `/version` for the platform and `/user` for the token.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GiteaProbe {
    /// Whether both platform and token checks passed.
    pub ok: bool,
    /// Detected Gitea version confirming the platform.
    pub version: Option<String>,
    /// Login name associated with a valid token.
    pub user: Option<String>,
    /// User-facing English result description.
    pub message: String,
}

/// GET a URL with an optional token, returning status and error text to distinguish connectivity from auth.
fn http_get(url: &str, token: Option<&str>) -> Result<String, (Option<u16>, String)> {
    let mut req = ureq::get(url).timeout(Duration::from_secs(10));
    if let Some(t) = token {
        req = req.set("Authorization", &format!("token {t}"));
    }
    match req.call() {
        Ok(resp) => resp.into_string().map_err(|e| (None, e.to_string())),
        Err(ureq::Error::Status(code, _)) => Err((Some(code), format!("HTTP {code}"))),
        Err(e) => Err((None, e.to_string())),
    }
}

/// Probe `/api/v1/version` and return its version to confirm the endpoint is Gitea.
fn probe_version(base_url: &str) -> Result<String, (Option<u16>, String)> {
    let url = format!("{}/api/v1/version", base_url.trim_end_matches('/'));
    let body = http_get(&url, None)?;
    let v: Value = serde_json::from_str(&body).map_err(|e| (None, e.to_string()))?;
    Ok(v.get("version")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string())
}

/// Test platform and token, returning readable failure descriptions rather than throwing into the UI.
pub fn probe(base_url: &str, token: &str) -> GiteaProbe {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return GiteaProbe {
            ok: false,
            version: None,
            user: None,
            message: "Enter a Gitea base URL first.".into(),
        };
    }
    let version = match probe_version(base) {
        Ok(v) => v,
        Err((_, e)) => {
            return GiteaProbe {
                ok: false,
                version: None,
                user: None,
                message: format!("Couldn't reach a Gitea server at this URL: {e}"),
            };
        }
    };
    if token.trim().is_empty() {
        return GiteaProbe {
            ok: false,
            version: Some(version),
            user: None,
            message: "Reached the Gitea server, but no token was provided.".into(),
        };
    }
    let user_url = format!("{base}/api/v1/user");
    match http_get(&user_url, Some(token.trim())) {
        Ok(body) => {
            let login = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| v.get("login").and_then(Value::as_str).map(str::to_string));
            match login {
                Some(name) => GiteaProbe {
                    ok: true,
                    version: Some(version),
                    user: Some(name.clone()),
                    message: format!("Connected as {name}."),
                },
                None => GiteaProbe {
                    ok: false,
                    version: Some(version),
                    user: None,
                    message: "Reached the server but couldn't read the token's user.".into(),
                },
            }
        }
        Err((code, e)) => GiteaProbe {
            ok: false,
            version: Some(version),
            user: None,
            message: match code {
                Some(401) | Some(403) => {
                    "Reached the Gitea server, but the token is invalid.".into()
                }
                _ => format!("Token check failed: {e}"),
            },
        },
    }
}

/// Extract a lowercase host from HTTPS, SSH, or SCP-style Git URLs.
pub(crate) fn url_host(url: &str) -> Option<String> {
    let mut s = url.trim();
    if let Some(i) = s.find("://") {
        s = &s[i + 3..];
    }
    if let Some(i) = s.rfind('@') {
        s = &s[i + 1..];
    }
    // Host ends at the first slash or colon, including the colon in SCP-style Git URLs.
    let end = s.find(['/', ':']).unwrap_or(s.len());
    let host = &s[..end];
    if host.is_empty() {
        None
    } else {
        Some(host.to_lowercase())
    }
}

/// Parse `(owner, repo)` from HTTPS, SSH, or SCP-style origin URLs and remove a trailing `.git`.
fn parse_owner_repo(url: &str) -> Option<(String, String)> {
    let u = url.trim().trim_end_matches('/');
    let u = u.strip_suffix(".git").unwrap_or(u);
    // Extract the path following the host.
    let path = if let Some(i) = u.find("://") {
        // scheme://[user@]host[:port]/owner/repo
        let rest = &u[i + 3..];
        let slash = rest.find('/')?;
        &rest[slash + 1..]
    } else if let Some(colon) = u.rfind(':') {
        // SCP-style `git@host:owner/repo`.
        &u[colon + 1..]
    } else {
        u
    };
    let mut parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() < 2 {
        return None;
    }
    let repo = parts.pop().unwrap().to_string();
    let owner = parts.pop().unwrap().to_string();
    Some((owner, repo))
}

/// Convert a full baseline ref to the remote base branch name by stripping `refs/heads/` or
/// `refs/remotes/<remote>/`; return other forms unchanged.
pub(crate) fn pr_base_name(base_ref: &str) -> String {
    if let Some(rest) = base_ref.strip_prefix("refs/heads/") {
        return rest.to_string();
    }
    if let Some(rest) = base_ref.strip_prefix("refs/remotes/") {
        // Drop the first segment, the remote name, and keep the branch path.
        if let Some(idx) = rest.find('/') {
            return rest[idx + 1..].to_string();
        }
        return rest.to_string();
    }
    base_ref.to_string()
}

/// Whether this worktree matches configured Gitea by base URL and origin host without network access.
pub fn relevant(db: &Db, wt_path: &str) -> bool {
    let Some(base_url) = get_base_url(db) else {
        return false;
    };
    let Some(origin) = git::remote_origin_url(wt_path) else {
        return false;
    };
    matches!((url_host(&base_url), url_host(&origin)), (Some(a), Some(b)) if a == b)
}

/// Check provider availability after `relevant`: validate a token and probe `/version`. Return an
/// availability flag and localizable `ok`, `no_token`, or `unreachable` reason.
pub fn land_provider_check(db: &Db, identifier: &str) -> (bool, String) {
    if load_token(db, identifier).is_none() {
        return (false, "no_token".into());
    }
    let Some(base_url) = get_base_url(db) else {
        return (false, "no_token".into());
    };
    if probe_version(&base_url).is_ok() {
        (true, "ok".into())
    } else {
        (false, "unreachable".into())
    }
}

// ─────────────────────────── Pull request creation ───────────────────────────

/// Pull request creation result.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrOutcome {
    /// URL of the new or existing pull request, empty when unavailable.
    pub url: String,
    /// User-facing English description.
    pub message: String,
}

/// Land through Gitea by pushing the worktree branch to origin and opening a pull request whose head is
/// the worktree branch and base is the shortened baseline branch.
#[allow(clippy::too_many_arguments)]
pub fn open_pr(
    db: &Db,
    identifier: &str,
    wt_path: &str,
    head_branch: &str,
    base_ref: &str,
    title: &str,
    body: &str,
    draft: bool,
) -> Result<PrOutcome, String> {
    let base_url = get_base_url(db).ok_or("Gitea base URL not configured")?;
    let token = load_token(db, identifier).ok_or("Gitea token not configured")?;
    let origin = git::remote_origin_url(wt_path)
        .ok_or("This worktree has no 'origin' remote; can't open a PR")?;
    let (owner, repo) = parse_owner_repo(&origin)
        .ok_or_else(|| format!("Couldn't parse owner/repo from origin URL: {origin}"))?;
    let base_name = pr_base_name(base_ref);

    // Push the branch to origin before creating a pull request that references it.
    git::push_branch(wt_path, head_branch)?;

    let url = format!(
        "{}/api/v1/repos/{}/{}/pulls",
        base_url.trim_end_matches('/'),
        owner,
        repo
    );
    let raw_title = if title.trim().is_empty() {
        head_branch
    } else {
        title.trim()
    };
    let final_title = if draft
        && !raw_title.to_lowercase().starts_with("wip:")
        && !raw_title.to_lowercase().starts_with("draft:")
    {
        format!("WIP: {raw_title}")
    } else {
        raw_title.to_string()
    };
    let payload = serde_json::json!({
        "head": head_branch,
        "base": base_name,
        "title": final_title,
        "body": body,
    });
    // Send serialized JSON as a string because ureq's send_json requires an otherwise unused feature.
    let payload_str = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    let resp = ureq::post(&url)
        .set("Authorization", &format!("token {token}"))
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(20))
        .send_string(&payload_str);
    match resp {
        Ok(r) => {
            let body = r.into_string().map_err(|e| e.to_string())?;
            let html_url = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| {
                    v.get("html_url")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_default();
            Ok(PrOutcome {
                url: html_url,
                message: "Pull request opened.".into(),
            })
        }
        Err(ureq::Error::Status(code, r)) => {
            let raw = r.into_string().unwrap_or_default();
            // Extract Gitea's `{"message": "..."}` body for a more readable error.
            let detail = serde_json::from_str::<Value>(&raw)
                .ok()
                .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_string))
                .unwrap_or(raw);
            // A 409 usually means a pull request already exists for this head and base.
            Err(format!("Gitea returned HTTP {code}: {detail}"))
        }
        Err(e) => Err(format!("Gitea PR request failed: {e}")),
    }
}

/// Inspect the newest Gitea pull request matching `head_branch`, preferring open PRs.
pub fn pr_status(
    db: &Db,
    identifier: &str,
    wt_path: &str,
    head_branch: &str,
) -> Result<Option<crate::git::pr::PrStatusInfo>, String> {
    let Some(base_url) = get_base_url(db) else {
        return Ok(None);
    };
    let Some(token) = load_token(db, identifier) else {
        return Ok(None);
    };
    let Some(origin) = git::remote_origin_url(wt_path) else {
        return Ok(None);
    };
    let Some((owner, repo)) = parse_owner_repo(&origin) else {
        return Ok(None);
    };
    let url = format!(
        "{}/api/v1/repos/{}/{}/pulls?state=all&limit=30",
        base_url.trim_end_matches('/'),
        owner,
        repo
    );
    let body = http_get(&url, Some(&token))
        .map_err(|_| "Could not load Gitea pull request status right now.".to_string())?;
    let items: Vec<Value> = serde_json::from_str(&body)
        .map_err(|_| "Received an invalid Gitea pull request response.".to_string())?;
    let matching: Vec<&Value> = items
        .iter()
        .filter(|item| {
            item.get("head")
                .and_then(|h| h.get("ref"))
                .and_then(Value::as_str)
                == Some(head_branch)
        })
        .collect();
    let chosen = matching
        .iter()
        .copied()
        .find(|item| item.get("state").and_then(Value::as_str) == Some("open"))
        .or_else(|| matching.first().copied());
    let Some(v) = chosen else {
        return Ok(None);
    };
    let number = v.get("number").and_then(Value::as_u64).unwrap_or(0);
    let title = v.get("title").and_then(Value::as_str).unwrap_or_default().to_string();
    let html_url = v.get("html_url").and_then(Value::as_str).unwrap_or_default().to_string();
    let base = v
        .get("base")
        .and_then(|b| b.get("ref"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let merged = v.get("merged").and_then(Value::as_bool).unwrap_or(false);
    let draft = v.get("draft").and_then(Value::as_bool).unwrap_or(false)
        || title.to_lowercase().starts_with("wip:")
        || title.to_lowercase().starts_with("[wip]");
    let raw_state = v.get("state").and_then(Value::as_str).unwrap_or("open");
    let state = if merged {
        "merged"
    } else if raw_state == "open" && draft {
        "draft"
    } else if raw_state == "open" {
        "open"
    } else {
        "closed"
    };
    Ok(Some(crate::git::pr::PrStatusInfo {
        provider: "gitea".into(),
        number,
        title,
        url: html_url,
        state: state.into(),
        base,
        checks: "none".into(),
        worktree: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_owner_repo_forms() {
        let cases = [
            ("https://git.example.com/alice/repo.git", ("alice", "repo")),
            ("https://git.example.com/alice/repo", ("alice", "repo")),
            ("git@git.example.com:alice/repo.git", ("alice", "repo")),
            (
                "ssh://git@git.example.com:2222/alice/repo.git",
                ("alice", "repo"),
            ),
        ];
        for (url, (o, r)) in cases {
            let got = parse_owner_repo(url).unwrap_or_else(|| panic!("failed to parse: {url}"));
            assert_eq!(got, (o.to_string(), r.to_string()), "url={url}");
        }
    }

    #[test]
    fn url_host_forms() {
        assert_eq!(
            url_host("https://git.example.com/a/b.git").as_deref(),
            Some("git.example.com")
        );
        assert_eq!(
            url_host("git@git.example.com:a/b.git").as_deref(),
            Some("git.example.com")
        );
        assert_eq!(
            url_host("ssh://git@git.example.com:2222/a/b").as_deref(),
            Some("git.example.com")
        );
    }

    #[test]
    fn pr_base_name_strips_ref_prefixes() {
        assert_eq!(pr_base_name("refs/heads/dev-electron"), "dev-electron");
        assert_eq!(pr_base_name("refs/remotes/origin/main"), "main");
        assert_eq!(pr_base_name("main"), "main");
    }
}
