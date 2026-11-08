//! Session-scoped CLI versions and bounded checks against official release metadata.

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use crate::{host::AppCtx, models::SessionKind};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentVersion {
    pub version: Option<String>,
    pub running: bool,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub check_status: &'static str,
    pub docs_url: Option<String>,
    /// The self-update command the badge offers to run, present only while an update is available.
    pub update_command: Option<String>,
}

#[derive(Default)]
struct Latest {
    checked_at: Option<Instant>,
    version: Option<String>,
}

type ReleaseCache = Mutex<HashMap<String, Arc<Mutex<Latest>>>>;
static RELEASES: OnceLock<ReleaseCache> = OnceLock::new();

/// Called by the blocking dispatch worker. No executable path or command comes from the client.
pub fn for_session(app: &AppCtx, id: &str, check_updates: bool) -> Result<AgentVersion, String> {
    let session = {
        let conn = app.db().conn.lock().map_err(|_| "Database unavailable")?;
        crate::db::repo::get_session(&conn, id)?.ok_or("Session not found")?
    };
    if !session.kind.supports_chat() { return Err("This agent does not support conversation view".into()); }
    let live = app.chat().agent_version(id);
    let running = live.is_some();
    let version = live.unwrap_or_else(|| installed(&super::executable::for_session(app, &session)));
    let opencode_v2=session.kind==SessionKind::Opencode && super::opencode::is_v2_version(version.as_deref());
    let latest = if opencode_v2 {latest_opencode_v2(check_updates)} else {latest(session.kind, check_updates)};
    let update_available = if opencode_v2 {newer_opencode_v2(version.as_deref(),latest.version.as_deref())}
        else {newer(version.as_deref(), latest.version.as_deref())};
    let check_status = if latest.checked_at.is_none() { "unchecked" }
        else if latest.version.is_some() { "checked" } else { "failed" };
    let update_command = if update_available { super::update::command_for(app, &session, version.as_deref()) } else { None };
    Ok(AgentVersion {
        version, running, latest_version: latest.version, update_available, check_status,
        docs_url: super::install::install_recipe(session.kind.as_str()).map(|r|r.docs_url), update_command,
    })
}

/// Capture the selected executable before launch, retaining that version throughout the process lifetime.
pub fn installed(bin: &str) -> Option<String> {
    match super::health::run_version(bin, Duration::from_secs(3)) {
        super::health::Outcome::Exited { success: true, stdout } => parse(&stdout),
        _ => None,
    }
}

fn parse(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | ',' | '"' | '/'))
        .filter_map(|token| {
            // Copilot ends its sentence with the version ("1.0.63."), and Cursor numbers releases by date
            // with zero-padded parts ("2026.07.01-41b2de7"), which semver rejects; the date is kept as written.
            let token = token.trim_start_matches('v').trim_end_matches('.');
            if let Ok(version) = semver::Version::parse(token) { return Some(version.to_string()); }
            let (core, rest) = token.split_at(token.find(['-', '+']).unwrap_or(token.len()));
            let parts: Vec<&str> = core.split('.').collect();
            let numeric = parts.len() == 3 && parts.iter().all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
            if !numeric { return None; }
            let unpadded = parts.iter().map(|part| part.trim_start_matches('0').to_string()).map(|part| if part.is_empty() { "0".into() } else { part })
                .collect::<Vec<_>>().join(".");
            semver::Version::parse(&format!("{unpadded}{rest}")).ok().map(|_| token.to_string())
        })
        .next()
}

fn newer(current: Option<&str>, latest: Option<&str>) -> bool {
    match (current.and_then(|s|semver::Version::parse(s).ok()), latest.and_then(|s|semver::Version::parse(s).ok())) {
        (Some(current), Some(latest)) => latest.pre.is_empty() && latest.cmp_precedence(&current).is_gt(),
        _ => false,
    }
}

fn source(kind: SessionKind) -> Option<(String, &'static str)> {
    let package = match kind {
        SessionKind::Claude => Some("@anthropic-ai/claude-code"),
        SessionKind::Codex => Some("@openai/codex"),
        SessionKind::Opencode => Some("opencode-ai"),
        SessionKind::Pi => Some("@earendil-works/pi-coding-agent"),
        SessionKind::Deepseek => Some("@deepseek-ai/dsh"),
        _ => None,
    };
    if let Some(package) = package { return Some((format!("https://registry.npmjs.org/{package}/latest"), "version")); }
    if kind == SessionKind::Omp {
        return Some(("https://api.github.com/repos/can1357/oh-my-pi/releases/latest".into(), "tag_name"));
    }
    if kind == SessionKind::Antigravity {
        let os = match std::env::consts::OS { "macos" => "darwin", "windows" => "windows", "linux" => "linux", _ => return None };
        let arch = match std::env::consts::ARCH { "aarch64" => "arm64", "x86_64" => "amd64", _ => return None };
        let suffix = if cfg!(target_env = "musl") { "_musl" } else { "" };
        return Some((format!("https://antigravity-cli-auto-updater-974169037036.us-central1.run.app/manifests/{os}_{arch}{suffix}.json"), "version"));
    }
    None
}

fn latest(kind: SessionKind, check: bool) -> Latest {
    let cache = RELEASES.get_or_init(||Mutex::new(HashMap::new()));
    let entry = cache.lock().unwrap().entry(kind.as_str().into()).or_default().clone();
    let mut entry = entry.lock().unwrap();
    let ttl = if entry.version.is_some() { Duration::from_secs(6 * 3600) } else { Duration::from_secs(300) };
    if check && entry.checked_at.is_none_or(|at|at.elapsed() >= ttl) {
        entry.version = fetch(kind);
        entry.checked_at = Some(Instant::now());
    }
    Latest { checked_at: entry.checked_at, version: entry.version.clone() }
}

fn fetch(kind: SessionKind) -> Option<String> {
    let (url, field) = source(kind)?;
    let response = ureq::AgentBuilder::new().timeout(Duration::from_secs(8)).build()
        .get(&url).set("User-Agent", "VelaTerm agent version check")
        .set("Accept", "application/json").call().ok()?;
    let mut bytes = Vec::new();
    response.into_reader().take(262_145).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 262_144 { return None; }
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let version = semver::Version::parse(value.get(field)?.as_str()?.trim_start_matches('v')).ok()?;
    if !version.pre.is_empty() { return None; }
    Some(version.to_string())
}

fn latest_opencode_v2(check: bool) -> Latest {
    let cache=RELEASES.get_or_init(||Mutex::new(HashMap::new()));
    let entry=cache.lock().unwrap().entry("opencode-v2".into()).or_default().clone();
    let mut entry=entry.lock().unwrap();
    let ttl=if entry.version.is_some(){Duration::from_secs(6*3600)}else{Duration::from_secs(300)};
    if check && entry.checked_at.is_none_or(|at|at.elapsed()>=ttl) {
        entry.version=(|| {
            let response=ureq::get("https://registry.npmjs.org/@opencode-ai/cli/beta")
                .set("User-Agent","VelaTerm agent version check").timeout(Duration::from_secs(8)).call().ok()?;
            let mut bytes=Vec::new();response.into_reader().take(262_145).read_to_end(&mut bytes).ok()?;
            if bytes.len()>262_144{return None;}
            let value:serde_json::Value=serde_json::from_slice(&bytes).ok()?;
            Some(semver::Version::parse(value["version"].as_str()?).ok()?.to_string())
        })();
        entry.checked_at=Some(Instant::now());
    }
    Latest{checked_at:entry.checked_at,version:entry.version.clone()}
}

fn newer_opencode_v2(current: Option<&str>, latest: Option<&str>) -> bool {
    let build=|value:&str|value.strip_prefix("0.0.0-beta-").and_then(|v|v.parse::<u64>().ok());
    if let Some((current,latest))=current.and_then(build).zip(latest.and_then(build)){return latest>current;}
    newer(current,latest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_version_lines_of_acp_agents() {
        assert_eq!(parse("GitHub Copilot CLI 1.0.63.\nRun 'copilot update' to check for updates.").as_deref(), Some("1.0.63"));
        assert_eq!(parse("2026.07.01-41b2de7").as_deref(), Some("2026.07.01-41b2de7"));
        assert_eq!(parse("grok 1.0.46 (2765805b9442)").as_deref(), Some("1.0.46"));
        assert_eq!(parse("2.1.1").as_deref(), Some("2.1.1"));
    }

    #[test]
    fn parses_cli_output_and_preserves_prereleases() {
        assert_eq!(parse("codex-cli 0.161.0"), Some("0.161.0".into()));
        assert_eq!(parse("2.1.293 (Claude Code)"), Some("2.1.293".into()));
        assert_eq!(parse("omp v18.8.4-beta.1+build"), Some("18.8.4-beta.1+build".into()));
        assert_eq!(parse("omp/18.1.18"), Some("18.1.18".into()));
        assert_eq!(parse("not installed"), None);
    }

    #[test]
    fn updates_require_a_newer_stable_version() {
        assert!(newer(Some("1.9.0"), Some("1.10.0")));
        assert!(newer(Some("1.10.0-beta.1"), Some("1.10.0")));
        assert!(!newer(Some("1.10.0+local"), Some("1.10.0+official")));
        assert!(!newer(Some("2.0.0"), Some("1.10.0")));
        assert!(!newer(Some("1.0.0"), Some("2.0.0-beta.1")));
        assert!(!newer(None, Some("2.0.0")));
        assert!(!newer(Some("1.0.0"), None));
    }

    #[test]
    fn opencode_previews_compare_their_own_build_sequence() {
        assert!(super::super::opencode::is_v2_version(Some("0.0.0-beta-19271")));
        assert!(super::super::opencode::is_v2_version(Some("2.0.0")));
        assert!(!super::super::opencode::is_v2_version(Some("1.18.31")));
        assert!(newer_opencode_v2(Some("0.0.0-beta-999"),Some("0.0.0-beta-1000")));
        assert!(!newer_opencode_v2(Some("0.0.0-beta-19271"),Some("0.0.0-beta-17823")));
        assert!(!newer_opencode_v2(Some("0.0.0-beta-19271"),Some("0.0.0-beta-19271")));
        assert!(newer_opencode_v2(Some("2.0.0"),Some("2.1.0")));
    }

    #[test]
    fn every_chat_agent_has_an_official_source() {
        for kind in [SessionKind::Claude, SessionKind::Codex, SessionKind::Opencode, SessionKind::Pi, SessionKind::Omp, SessionKind::Antigravity] {
            assert!(source(kind).is_some());
        }
        assert!(source(SessionKind::Terminal).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn reads_the_session_override_and_never_substitutes_a_missing_binary() {
        use std::os::unix::fs::PermissionsExt;
        use crate::{db::{Db, repo}, host::HeadlessHost};
        let dir = std::env::temp_dir().join(format!("vlx-agent-version-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("custom-agent");
        std::fs::write(&bin, "#!/bin/sh\nprintf 'codex-cli 0.100.0\\n'\n").unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o700)).unwrap();
        let db = Db::open(&dir.join("test.db")).unwrap();
        let session = {
            let conn = db.conn.lock().unwrap();
            let project = repo::import_project(&conn, dir.to_str().unwrap()).unwrap();
            repo::create_session_full(&conn, &project.id, None, "Custom", SessionKind::Codex,
                None, None, None, None, None, None, None, None, None, bin.to_str(), Some("chat")).unwrap()
        };
        let app = AppCtx::Headless(Arc::new(HeadlessHost::new(dir.clone(), db)));
        let result = for_session(&app, &session.id, false).unwrap();
        assert_eq!(result.version.as_deref(), Some("0.100.0"));
        assert!(!result.running);
        std::fs::remove_file(&bin).unwrap();
        assert!(for_session(&app, &session.id, false).unwrap().version.is_none());
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
