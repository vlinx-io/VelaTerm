//! Minimal `~/.ssh/config` parser for the six directives the SSH GUI client actually consults:
//! `HostName`, `User`, `Port`, `IdentityFile`, `IdentitiesOnly`, `PreferredAuthentications`.
//!
//! The OS-native `ssh` binary honours dozens of options through its own resolver; this module is not a
//! substitute for it. It exists so that the in-app russh transport (see `ssh_russh.rs`, GUI-only) can
//! respect the same connection preferences a user already keeps in `~/.ssh/config` for their normal
//! terminal sessions, instead of ignoring the file and prompting for a password when the config says
//! only public keys are acceptable.
//!
//! Semantics follow `ssh_config(5)`:
//!
//! - The file is parsed in order. Blocks begin with a `Host <patterns>` line; a section applies when
//!   the target host matches any of its patterns.
//! - Patterns support `*` and `?` glob wildcards. Negation (`!pattern`) is intentionally left out; the
//!   directives we care about here rarely rely on it and adding it would broaden the surface without a
//!   concrete GUI-client use case.
//! - For scalar directives (`HostName`, `User`, `Port`, `IdentitiesOnly`, `PreferredAuthentications`),
//!   the first obtained value wins across the whole file, matching openssh.
//! - `IdentityFile` is additive: every matching value is appended in file order.
//! - Keys are case-insensitive. Values may be double-quoted; a leading `~/` is expanded to the caller's
//!   home directory.
//!
//! The parser does not touch the network and never spawns a subprocess, so it is safe to call inside a
//! Tokio blocking context. It also does not resolve `%d`/`%u`/`%h` tokens or `Include` directives; these
//! are noted as follow-ups if a real-world config demands them.

use std::path::{Path, PathBuf};

/// Effective SSH configuration for a target host, distilled from the file.
///
/// Every field is optional so the caller can layer it on top of its own defaults (target string,
/// hard-coded key list, etc.) rather than being forced to choose one source.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HostConfig {
    /// `HostName` override; use in place of the target's literal host when set.
    pub hostname: Option<String>,
    /// `User` override; used when the target string does not embed one.
    pub user: Option<String>,
    /// `Port` override; used when the target string does not include a port.
    pub port: Option<u16>,
    /// Every `IdentityFile` matched, in file order. Duplicates are preserved because openssh keeps
    /// them; callers may dedupe if they need to.
    pub identity_files: Vec<PathBuf>,
    /// `IdentitiesOnly yes` restricts key attempts to `identity_files` and disables any built-in
    /// default-key fallback. `None` means the directive was not set.
    pub identities_only: Option<bool>,
    /// `PreferredAuthentications`, split on commas and lowercased. `Some(list)` means the user has
    /// pinned an ordered set; anything not in the list must be skipped, including a password fallback.
    pub preferred_authentications: Option<Vec<String>>,
}

impl HostConfig {
    /// Whether interactive password / keyboard-interactive is allowed by `PreferredAuthentications`.
    /// When the directive is absent, both are allowed (matching openssh's default of trying every
    /// available method). When it is set, only the exact names listed are.
    pub fn allows_password(&self) -> bool {
        match &self.preferred_authentications {
            None => true,
            Some(list) => list.iter().any(|m| m == "password" || m == "keyboard-interactive"),
        }
    }
}

/// Read the standard SSH config path (`~/.ssh/config`) and return the effective `HostConfig` for
/// `target`. A missing file yields `HostConfig::default()`, which is indistinguishable from a file that
/// simply has no matching block; callers should treat both the same way.
pub fn lookup(target: &str, home: Option<&Path>) -> HostConfig {
    let Some(home) = home else {
        return HostConfig::default();
    };
    let path = home.join(".ssh").join("config");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return HostConfig::default();
    };
    parse_lookup(&text, target, Some(home))
}

/// Parse `config` and return the effective `HostConfig` for `target`. `home` is used to expand a
/// leading `~/` in `IdentityFile` values; passing `None` leaves the path as-is.
pub fn parse_lookup(config: &str, target: &str, home: Option<&Path>) -> HostConfig {
    let mut out = HostConfig::default();
    // Every `Host` block delimits a section. A file may start with directives before any Host line;
    // openssh applies those unconditionally, so treat the implicit initial section as matching `*`.
    let mut matches_current = true;
    for raw in config.lines() {
        let line = strip_comment(raw);
        if line.is_empty() {
            continue;
        }
        let (key, value) = match split_key_value(line) {
            Some(kv) => kv,
            None => continue, // Malformed line; openssh warns and skips.
        };
        let key_lc = key.to_ascii_lowercase();
        if key_lc == "host" {
            matches_current = value.split_whitespace().any(|pat| glob_match(pat, target));
            continue;
        }
        if !matches_current {
            continue;
        }
        match key_lc.as_str() {
            "hostname" if out.hostname.is_none() => {
                out.hostname = Some(unquote(value).to_string());
            }
            "user" if out.user.is_none() => {
                out.user = Some(unquote(value).to_string());
            }
            "port" if out.port.is_none() => {
                if let Ok(p) = unquote(value).parse() {
                    out.port = Some(p);
                }
            }
            "identityfile" => {
                out.identity_files.push(expand_home(unquote(value), home));
            }
            "identitiesonly" if out.identities_only.is_none() => {
                out.identities_only = Some(matches!(
                    unquote(value).to_ascii_lowercase().as_str(),
                    "yes" | "true"
                ));
            }
            "preferredauthentications" if out.preferred_authentications.is_none() => {
                out.preferred_authentications = Some(
                    unquote(value)
                        .split(',')
                        .map(|s| s.trim().to_ascii_lowercase())
                        .filter(|s| !s.is_empty())
                        .collect(),
                );
            }
            _ => {}
        }
    }
    out
}

/// Strip an `# ...` trailing comment and surrounding whitespace.
fn strip_comment(line: &str) -> &str {
    let cut = line.find('#').unwrap_or(line.len());
    line[..cut].trim()
}

/// Split the first whitespace or `=` boundary. `Key = value` and `Key value` are both valid in openssh
/// configuration files.
fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let idx = line
        .char_indices()
        .find(|(_, c)| c.is_whitespace() || *c == '=')?
        .0;
    let key = &line[..idx];
    // Skip any run of whitespace or a single `=` between the key and value.
    let mut rest = line[idx..].trim_start();
    if let Some(after_eq) = rest.strip_prefix('=') {
        rest = after_eq.trim_start();
    }
    if key.is_empty() || rest.is_empty() {
        return None;
    }
    Some((key, rest))
}

/// Remove surrounding double quotes. openssh treats single quotes as literal characters, so this
/// deliberately handles only the `"..."` form.
fn unquote(s: &str) -> &str {
    let trimmed = s.trim();
    trimmed
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(trimmed)
}

/// Expand a leading `~/` to `home/...`. Anything else (an absolute path, a bare filename, a `%d`
/// token) is returned unchanged; the caller decides whether to reject it.
fn expand_home(value: &str, home: Option<&Path>) -> PathBuf {
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(h) = home {
            return h.join(rest);
        }
    }
    PathBuf::from(value)
}

/// Match a target hostname against an openssh-style pattern with `*` and `?` wildcards. Case-sensitive
/// against the hostname, since DNS is case-insensitive but user config generally is not.
fn glob_match(pattern: &str, target: &str) -> bool {
    // Iterative dynamic-programming with backtracking: pt/tt walk pattern/target, back_pt/back_tt
    // remember the last `*` we can rewind to when a later fixed segment fails to match.
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = target.chars().collect();
    let mut pt = 0usize;
    let mut tt = 0usize;
    let mut back_pt: Option<usize> = None;
    let mut back_tt: usize = 0;
    while tt < t.len() {
        if pt < p.len() && (p[pt] == '?' || p[pt] == t[tt]) {
            pt += 1;
            tt += 1;
        } else if pt < p.len() && p[pt] == '*' {
            back_pt = Some(pt);
            back_tt = tt;
            pt += 1; // Skip the `*`; on mismatch we come back and consume one more target char.
        } else if let Some(bp) = back_pt {
            pt = bp + 1;
            back_tt += 1;
            tt = back_tt;
        } else {
            return false;
        }
    }
    while pt < p.len() && p[pt] == '*' {
        pt += 1;
    }
    pt == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn home() -> PathBuf {
        PathBuf::from("/home/tester")
    }

    /// The reporter's config from #107: publickey-only against a specific host, with an explicit
    /// IdentityFile and IdentitiesOnly. The parser must surface all four fields for that host.
    #[test]
    fn parses_issue_107_reporter_config() {
        let cfg = r#"
Host 192.168.31.3
  HostName 192.168.31.3
  User ccx
  PreferredAuthentications publickey
  IdentityFile ~/.ssh/id_rsa
  IdentitiesOnly yes
"#;
        let got = parse_lookup(cfg, "192.168.31.3", Some(&home()));
        assert_eq!(got.hostname.as_deref(), Some("192.168.31.3"));
        assert_eq!(got.user.as_deref(), Some("ccx"));
        assert_eq!(got.identity_files, vec![home().join(".ssh/id_rsa")]);
        assert_eq!(got.identities_only, Some(true));
        assert_eq!(
            got.preferred_authentications.as_deref(),
            Some(&["publickey".to_string()][..])
        );
        assert!(!got.allows_password(), "publickey-only must forbid password fallback");
    }

    /// Global directives before any `Host` line apply unconditionally, and later per-host directives
    /// do not override an earlier value for scalar keys.
    #[test]
    fn first_match_wins_across_blocks() {
        let cfg = r#"
User admin
Port 2200
Host prod.example.com
  User deploy
  Port 22
"#;
        let got = parse_lookup(cfg, "prod.example.com", Some(&home()));
        assert_eq!(got.user.as_deref(), Some("admin"));
        assert_eq!(got.port, Some(2200));
    }

    /// IdentityFile is additive: every matching value is appended in file order, including from
    /// wildcard blocks after a specific one.
    #[test]
    fn identity_file_is_additive_and_expanded() {
        let cfg = r#"
Host build.internal
  IdentityFile ~/.ssh/build_ed25519
Host *
  IdentityFile ~/.ssh/id_ed25519
  IdentityFile /etc/ssh/backup_key
"#;
        let got = parse_lookup(cfg, "build.internal", Some(&home()));
        assert_eq!(
            got.identity_files,
            vec![
                home().join(".ssh/build_ed25519"),
                home().join(".ssh/id_ed25519"),
                PathBuf::from("/etc/ssh/backup_key"),
            ]
        );
    }

    /// Wildcard patterns follow ssh_config semantics: `*` matches any run and `?` matches one char.
    #[test]
    fn host_patterns_support_star_and_question_mark() {
        let cfg = r#"
Host db-?.example.com
  User dba
Host *.example.com
  User app
"#;
        let db = parse_lookup(cfg, "db-1.example.com", Some(&home()));
        assert_eq!(db.user.as_deref(), Some("dba"));
        let app = parse_lookup(cfg, "web.example.com", Some(&home()));
        assert_eq!(app.user.as_deref(), Some("app"));
        let none = parse_lookup(cfg, "web.other.com", Some(&home()));
        assert_eq!(none.user, None);
    }

    /// `PreferredAuthentications` splits on commas and lowercases. `allows_password` returns true only
    /// when password or keyboard-interactive appears in the list.
    #[test]
    fn preferred_authentications_controls_password_fallback() {
        let publickey_only = parse_lookup(
            "Host x\n  PreferredAuthentications publickey,gssapi-with-mic\n",
            "x",
            Some(&home()),
        );
        assert!(!publickey_only.allows_password());
        let with_password = parse_lookup(
            "Host x\n  PreferredAuthentications publickey,password\n",
            "x",
            Some(&home()),
        );
        assert!(with_password.allows_password());
        let unset = parse_lookup("Host x\n  User u\n", "x", Some(&home()));
        assert!(unset.allows_password(), "absent directive keeps the openssh default");
    }

    /// Comments, blank lines, `Key = Value` form, and quoted values all parse cleanly, and unknown
    /// directives are ignored without failing the surrounding block.
    #[test]
    fn tolerates_comments_equals_quotes_and_unknown_keys() {
        let cfg = r#"
# global block
Host       spaced-out # trailing comment
   User = "ada"
   Port=2222
   KexAlgorithms curve25519-sha256   # unknown key
   HostName "spaced-out.example.com"
"#;
        let got = parse_lookup(cfg, "spaced-out", Some(&home()));
        assert_eq!(got.user.as_deref(), Some("ada"));
        assert_eq!(got.port, Some(2222));
        assert_eq!(got.hostname.as_deref(), Some("spaced-out.example.com"));
    }

    /// A missing ~/.ssh/config yields the default HostConfig; no field is set and password is allowed.
    #[test]
    fn missing_home_or_file_returns_default() {
        let empty: HostConfig = lookup("anywhere", Some(Path::new("/nonexistent-dir")));
        assert_eq!(empty, HostConfig::default());
        assert!(empty.allows_password());
    }
}
