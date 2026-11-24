//! Pull and merge request integration for GitHub (`gh`) and GitLab (`glab`), paired with Gitea in
//! `crate::gitea`.
//!
//! Hosted repositories authenticate through the developer's existing `gh` or `glab` CLI login rather
//! than storing duplicate OAuth credentials in VelaTerm. Every CLI invocation runs headless with
//! prompts disabled and a bounded timeout so a missing credential or unreachable host fails cleanly
//! instead of blocking a worker thread.

use super::*;
use serde_json::Value;
use std::path::{Path, PathBuf};

const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const ACTION_TIMEOUT: Duration = Duration::from_secs(60);

/// Availability of pull/merge request operations for a repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrProviderStatus {
    /// `"github"`, `"gitlab"`, `"gitea"`, or `None` when the remote is unsupported.
    pub provider: Option<String>,
    pub available: bool,
    /// Stable reason code localized by the UI: `ok`, `not_gitea`, `no_token`, `unreachable`,
    /// `no_cli`, or `no_auth`.
    pub reason: String,
    /// Canonical remote host when identified.
    pub host: Option<String>,
}

/// Linked Git worktree metadata when `cwd` is not the main repository checkout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedWorktree {
    pub path: String,
    pub main_repo: String,
}

/// Current pull or merge request state for the checked-out branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrStatusInfo {
    pub provider: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    /// `"open"`, `"draft"`, `"merged"`, or `"closed"`.
    pub state: String,
    pub base: String,
    /// `"passing"`, `"failing"`, `"pending"`, or `"none"`.
    pub checks: String,
    /// Populated when `cwd` sits inside a linked Git worktree that can be cleaned up after merge.
    pub worktree: Option<LinkedWorktree>,
}

/// CLI overrides used exclusively by unit tests so parallel tests never mutate process `PATH`.
#[derive(Default, Clone, Copy)]
pub(crate) struct CliOverrides<'a> {
    pub gh: Option<&'a Path>,
    pub glab: Option<&'a Path>,
}

/// Identify whether `cwd` is a linked Git worktree distinct from its main repository checkout.
pub fn linked_worktree(cwd: &str) -> Option<LinkedWorktree> {
    require_repo(cwd).ok()?;
    let top = repo_top(cwd);
    let main = main_repo_of(&top)?;
    let same = match (std::fs::canonicalize(&top), std::fs::canonicalize(&main)) {
        (Ok(a), Ok(b)) => a == b,
        _ => Path::new(&top) == Path::new(&main),
    };
    if same {
        None
    } else {
        Some(LinkedWorktree { path: top, main_repo: main })
    }
}

fn origin_url_for_host(cwd: &str) -> Option<String> {
    remote_origin_url(cwd)
        .filter(|u| crate::gitea::url_host(u).is_some())
        .or_else(|| run_git(cwd, &["config", "--get", "remote.origin.url"]).filter(|s| !s.is_empty()))
}

/// Normalize SSH config host aliases like `github.com-work` to `github.com`.
fn canonical_host(raw: &str) -> String {
    let h = raw.trim().to_lowercase();
    if h == "ssh.github.com" || h.starts_with("github.com-") || h.starts_with("github.com_") {
        return "github.com".into();
    }
    if h.starts_with("gitlab.com-") || h.starts_with("gitlab.com_") {
        return "gitlab.com".into();
    }
    h
}

fn is_github_host(host: &str) -> bool {
    host == "github.com" || host.ends_with(".github.com")
}

fn is_gitlab_host(host: &str) -> bool {
    host == "gitlab.com" || host.ends_with(".gitlab.com") || host.contains("gitlab")
}

/// Locate `gh` or `glab` on `PATH` or in standard platform installation directories.
fn resolve_cli(name: &str, override_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = override_path {
        return p.is_file().then(|| p.to_path_buf());
    }
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            if let Some(found) = candidate_in_dir(&dir, name) {
                return Some(found);
            }
        }
    }
    for dir in fallback_cli_dirs(name) {
        if let Some(found) = candidate_in_dir(&dir, name) {
            return Some(found);
        }
    }
    None
}

fn candidate_in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        for ext in ["exe", "cmd", "bat"] {
            let p = dir.join(format!("{name}.{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
        let bare = dir.join(name);
        bare.is_file().then_some(bare)
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let p = dir.join(name);
        let meta = std::fs::metadata(&p).ok()?;
        (meta.is_file() && (meta.permissions().mode() & 0o111 != 0)).then_some(p)
    }
}

fn fallback_cli_dirs(_name: &str) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(not(windows))]
    {
        for d in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/snap/bin"] {
            dirs.push(PathBuf::from(d));
        }
        if let Some(home) = crate::host::home_dir() {
            dirs.push(home.join(".local/bin"));
        }
    }
    #[cfg(windows)]
    {
        if let Ok(pf) = std::env::var("ProgramFiles") {
            dirs.push(PathBuf::from(&pf).join("GitHub CLI"));
            dirs.push(PathBuf::from(&pf).join("glab"));
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            dirs.push(PathBuf::from(&local).join("Programs").join("GitHub CLI"));
            dirs.push(PathBuf::from(&local).join("Programs").join("glab"));
        }
        if let Some(home) = crate::host::home_dir() {
            dirs.push(home.join("scoop").join("shims"));
        }
    }
    dirs
}

struct CliFailure {
    stdout: String,
    stderr: String,
}

/// Run an external CLI without interactive prompts and kill its process tree if it exceeds `timeout`.
fn run_cli(bin: &Path, cwd: &str, args: &[&str], timeout: Duration) -> Result<String, CliFailure> {
    let mut cmd = crate::host::command(bin);
    cmd.current_dir(cwd)
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_PROMPT", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .env("SSH_ASKPASS_REQUIRE", "never")
        .env("GIT_EDITOR", "true")
        .env("GLAB_CHECK_UPDATE", "false")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().map_err(|e| CliFailure {
        stdout: String::new(),
        stderr: e.to_string(),
    })?;
    let mut stdout = child.stdout.take().ok_or_else(|| CliFailure {
        stdout: String::new(),
        stderr: "Failed to capture output".into(),
    })?;
    let mut stderr = child.stderr.take().ok_or_else(|| CliFailure {
        stdout: String::new(),
        stderr: "Failed to capture error output".into(),
    })?;
    let out_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        buf
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if started.elapsed() >= timeout => {
                terminate_clone_process(&mut child);
                let _ = child.wait();
                return Err(CliFailure {
                    stdout: String::new(),
                    stderr: "The request timed out.".into(),
                });
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(e) => {
                terminate_clone_process(&mut child);
                return Err(CliFailure {
                    stdout: String::new(),
                    stderr: e.to_string(),
                });
            }
        }
    };
    let out = String::from_utf8_lossy(&out_reader.join().unwrap_or_default()).trim().to_string();
    let err = String::from_utf8_lossy(&err_reader.join().unwrap_or_default()).trim().to_string();
    if status.success() {
        Ok(out)
    } else {
        Err(CliFailure { stdout: out, stderr: err })
    }
}

/// Detect the hosting provider and CLI/token readiness for `cwd`.
pub fn detect_provider(db: &crate::db::Db, identifier: &str, cwd: &str) -> PrProviderStatus {
    detect_provider_with(db, identifier, cwd, CliOverrides::default())
}

pub(crate) fn detect_provider_with(
    db: &crate::db::Db,
    identifier: &str,
    cwd: &str,
    overrides: CliOverrides<'_>,
) -> PrProviderStatus {
    if crate::gitea::relevant(db, cwd) {
        let (available, reason) = crate::gitea::land_provider_check(db, identifier);
        let host = remote_origin_url(cwd).and_then(|u| crate::gitea::url_host(&u));
        return PrProviderStatus {
            provider: Some("gitea".into()),
            available,
            reason,
            host,
        };
    }
    let Some(origin) = origin_url_for_host(cwd) else {
        return PrProviderStatus {
            provider: None,
            available: false,
            reason: "not_gitea".into(),
            host: None,
        };
    };
    let Some(raw_host) = crate::gitea::url_host(&origin) else {
        return PrProviderStatus {
            provider: None,
            available: false,
            reason: "not_gitea".into(),
            host: None,
        };
    };
    let host = canonical_host(&raw_host);

    if is_github_host(&host) {
        return check_github_cli(cwd, &host, overrides.gh);
    }
    if is_gitlab_host(&host) {
        return check_gitlab_cli(cwd, &host, overrides.glab);
    }

    // Custom enterprise domains: recognize them when the local gh or glab CLI is already signed in
    // to that exact hostname.
    if let Some(bin) = resolve_cli("gh", overrides.gh) {
        if run_cli(&bin, cwd, &["auth", "status", "--hostname", &host], PROBE_TIMEOUT).is_ok() {
            return PrProviderStatus {
                provider: Some("github".into()),
                available: true,
                reason: "ok".into(),
                host: Some(host),
            };
        }
    }
    if let Some(bin) = resolve_cli("glab", overrides.glab) {
        if run_cli(&bin, cwd, &["auth", "status", "--hostname", &host], PROBE_TIMEOUT).is_ok() {
            return PrProviderStatus {
                provider: Some("gitlab".into()),
                available: true,
                reason: "ok".into(),
                host: Some(host),
            };
        }
    }

    PrProviderStatus {
        provider: None,
        available: false,
        reason: "not_gitea".into(),
        host: None,
    }
}

fn check_github_cli(cwd: &str, host: &str, override_bin: Option<&Path>) -> PrProviderStatus {
    let Some(bin) = resolve_cli("gh", override_bin) else {
        return PrProviderStatus {
            provider: Some("github".into()),
            available: false,
            reason: "no_cli".into(),
            host: Some(host.to_string()),
        };
    };
    let ok = run_cli(&bin, cwd, &["auth", "status", "--hostname", host], PROBE_TIMEOUT).is_ok();
    PrProviderStatus {
        provider: Some("github".into()),
        available: ok,
        reason: if ok { "ok".into() } else { "no_auth".into() },
        host: Some(host.to_string()),
    }
}

fn check_gitlab_cli(cwd: &str, host: &str, override_bin: Option<&Path>) -> PrProviderStatus {
    let Some(bin) = resolve_cli("glab", override_bin) else {
        return PrProviderStatus {
            provider: Some("gitlab".into()),
            available: false,
            reason: "no_cli".into(),
            host: Some(host.to_string()),
        };
    };
    let ok = run_cli(&bin, cwd, &["auth", "status", "--hostname", host], PROBE_TIMEOUT).is_ok();
    PrProviderStatus {
        provider: Some("gitlab".into()),
        available: ok,
        reason: if ok { "ok".into() } else { "no_auth".into() },
        host: Some(host.to_string()),
    }
}

/// Push `head_branch` and open a pull or merge request against `base_ref`.
pub fn open_pull_request(
    db: &crate::db::Db,
    identifier: &str,
    cwd: &str,
    head_branch: &str,
    base_ref: &str,
    title: &str,
    body: &str,
    draft: bool,
) -> Result<crate::gitea::PrOutcome, String> {
    open_pull_request_with(
        db,
        identifier,
        cwd,
        head_branch,
        base_ref,
        title,
        body,
        draft,
        CliOverrides::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn open_pull_request_with(
    db: &crate::db::Db,
    identifier: &str,
    cwd: &str,
    head_branch: &str,
    base_ref: &str,
    title: &str,
    body: &str,
    draft: bool,
    overrides: CliOverrides<'_>,
) -> Result<crate::gitea::PrOutcome, String> {
    let detected = detect_provider_with(db, identifier, cwd, overrides);
    let Some(provider) = detected.provider.as_deref() else {
        return Err("Pull requests are not supported for this repository's remote.".into());
    };
    if provider == "gitea" {
        return crate::gitea::open_pr(db, identifier, cwd, head_branch, base_ref, title, body, draft);
    }
    if !detected.available {
        return Err(unavailable_message(provider, &detected.reason));
    }

    let base_name = crate::gitea::pr_base_name(base_ref);
    let effective_title = if title.trim().is_empty() { head_branch } else { title.trim() };

    push_branch(cwd, head_branch)?;

    match provider {
        "github" => {
            let bin = resolve_cli("gh", overrides.gh)
                .ok_or_else(|| unavailable_message("github", "no_cli"))?;
            let mut args = vec![
                "pr",
                "create",
                "--head",
                head_branch,
                "--base",
                &base_name,
                "--title",
                effective_title,
                "--body",
                body,
            ];
            if draft {
                args.push("--draft");
            }
            match run_cli(&bin, cwd, &args, ACTION_TIMEOUT) {
                Ok(out) => {
                    let url = extract_http_url(&out)
                        .or_else(|| {
                            view_github_pr(&bin, cwd, head_branch)
                                .ok()
                                .flatten()
                                .map(|s| s.url)
                        })
                        .unwrap_or_default();
                    Ok(crate::gitea::PrOutcome {
                        url,
                        message: "Pull request opened.".into(),
                    })
                }
                Err(fail) => Err(friendly_create_error("github", &fail)),
            }
        }
        "gitlab" => {
            let bin = resolve_cli("glab", overrides.glab)
                .ok_or_else(|| unavailable_message("gitlab", "no_cli"))?;
            let mut args = vec![
                "mr",
                "create",
                "--source-branch",
                head_branch,
                "--target-branch",
                &base_name,
                "--title",
                effective_title,
                "--description",
                body,
                "--yes",
            ];
            if draft {
                args.push("--draft");
            }
            match run_cli(&bin, cwd, &args, ACTION_TIMEOUT) {
                Ok(out) => {
                    let url = extract_http_url(&out)
                        .or_else(|| {
                            view_gitlab_mr(&bin, cwd, head_branch)
                                .ok()
                                .flatten()
                                .map(|s| s.url)
                        })
                        .unwrap_or_default();
                    Ok(crate::gitea::PrOutcome {
                        url,
                        message: "Merge request opened.".into(),
                    })
                }
                Err(fail) => Err(friendly_create_error("gitlab", &fail)),
            }
        }
        _ => Err("Pull requests are not supported for this repository's remote.".into()),
    }
}

/// Inspect any existing pull or merge request associated with the checked-out branch.
pub fn current_pr_status(
    db: &crate::db::Db,
    identifier: &str,
    cwd: &str,
) -> Result<Option<PrStatusInfo>, String> {
    current_pr_status_with(db, identifier, cwd, CliOverrides::default())
}

pub(crate) fn current_pr_status_with(
    db: &crate::db::Db,
    identifier: &str,
    cwd: &str,
    overrides: CliOverrides<'_>,
) -> Result<Option<PrStatusInfo>, String> {
    let Some(branch) = status(cwd).branch else {
        return Ok(None);
    };
    let detected = detect_provider_with(db, identifier, cwd, overrides);
    if !detected.available {
        return Ok(None);
    }
    let worktree = linked_worktree(cwd);
    let mut info = match detected.provider.as_deref() {
        Some("github") => {
            let Some(bin) = resolve_cli("gh", overrides.gh) else {
                return Ok(None);
            };
            view_github_pr(&bin, cwd, &branch)?
        }
        Some("gitlab") => {
            let Some(bin) = resolve_cli("glab", overrides.glab) else {
                return Ok(None);
            };
            view_gitlab_mr(&bin, cwd, &branch)?
        }
        Some("gitea") => crate::gitea::pr_status(db, identifier, cwd, &branch)?,
        _ => None,
    };
    if let Some(ref mut pr) = info {
        pr.worktree = worktree;
    }
    Ok(info)
}

fn view_github_pr(bin: &Path, cwd: &str, branch: &str) -> Result<Option<PrStatusInfo>, String> {
    let args = [
        "pr",
        "view",
        branch,
        "--json",
        "number,title,url,state,isDraft,baseRefName,statusCheckRollup",
    ];
    match run_cli(bin, cwd, &args, PROBE_TIMEOUT) {
        Ok(out) => parse_github_pr_json(&out).map(Some),
        Err(fail) => {
            let combined = format!("{}\n{}", fail.stderr, fail.stdout).to_lowercase();
            if combined.contains("no pull requests found")
                || combined.contains("could not resolve to a pullrequest")
                || combined.contains("not found")
            {
                Ok(None)
            } else {
                Err("Could not load pull request status right now. Please try again later.".into())
            }
        }
    }
}

pub(crate) fn parse_github_pr_json(raw: &str) -> Result<PrStatusInfo, String> {
    let v: Value = serde_json::from_str(raw)
        .map_err(|_| "Received an invalid pull request status response.".to_string())?;
    let number = v.get("number").and_then(Value::as_u64).unwrap_or(0);
    let title = v.get("title").and_then(Value::as_str).unwrap_or_default().to_string();
    let url = v.get("url").and_then(Value::as_str).unwrap_or_default().to_string();
    let base = v.get("baseRefName").and_then(Value::as_str).unwrap_or_default().to_string();
    let is_draft = v.get("isDraft").and_then(Value::as_bool).unwrap_or(false);
    let raw_state = v.get("state").and_then(Value::as_str).unwrap_or("OPEN").to_uppercase();
    let state = match raw_state.as_str() {
        "MERGED" => "merged",
        "CLOSED" => "closed",
        _ if is_draft => "draft",
        _ => "open",
    };
    let checks = v
        .get("statusCheckRollup")
        .and_then(Value::as_array)
        .map(|items| rollup_github_checks(items))
        .unwrap_or("none");
    Ok(PrStatusInfo {
        provider: "github".into(),
        number,
        title,
        url,
        state: state.into(),
        base,
        checks: checks.into(),
        worktree: None,
    })
}

fn rollup_github_checks(items: &[Value]) -> &'static str {
    if items.is_empty() {
        return "none";
    }
    let mut has_pending = false;
    for item in items {
        // StatusContext entries carry `state`; CheckRun entries carry `status` + `conclusion`.
        if let Some(ctx_state) = item.get("state").and_then(Value::as_str) {
            match ctx_state.to_uppercase().as_str() {
                "FAILURE" | "ERROR" => return "failing",
                "PENDING" | "EXPECTED" => has_pending = true,
                _ => {}
            }
            continue;
        }
        let status = item.get("status").and_then(Value::as_str).unwrap_or("").to_uppercase();
        let conclusion = item.get("conclusion").and_then(Value::as_str).unwrap_or("").to_uppercase();
        if status != "COMPLETED" {
            has_pending = true;
            continue;
        }
        if matches!(
            conclusion.as_str(),
            "FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE"
        ) {
            return "failing";
        }
    }
    if has_pending { "pending" } else { "passing" }
}

fn view_gitlab_mr(bin: &Path, cwd: &str, branch: &str) -> Result<Option<PrStatusInfo>, String> {
    let args = ["mr", "view", branch, "--output", "json"];
    match run_cli(bin, cwd, &args, PROBE_TIMEOUT) {
        Ok(out) => parse_gitlab_mr_json(&out).map(Some),
        Err(fail) => {
            let combined = format!("{}\n{}", fail.stderr, fail.stdout).to_lowercase();
            if combined.contains("no merge request")
                || combined.contains("no open merge request")
                || combined.contains("404")
                || combined.contains("not found")
            {
                Ok(None)
            } else {
                Err("Could not load merge request status right now. Please try again later.".into())
            }
        }
    }
}

pub(crate) fn parse_gitlab_mr_json(raw: &str) -> Result<PrStatusInfo, String> {
    let v: Value = serde_json::from_str(raw)
        .map_err(|_| "Received an invalid merge request status response.".to_string())?;
    let number = v
        .get("iid")
        .or_else(|| v.get("id"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let title = v.get("title").and_then(Value::as_str).unwrap_or_default().to_string();
    let url = v.get("web_url").and_then(Value::as_str).unwrap_or_default().to_string();
    let base = v.get("target_branch").and_then(Value::as_str).unwrap_or_default().to_string();
    let is_draft = v.get("draft").and_then(Value::as_bool).unwrap_or(false)
        || v.get("work_in_progress").and_then(Value::as_bool).unwrap_or(false);
    let raw_state = v.get("state").and_then(Value::as_str).unwrap_or("opened").to_lowercase();
    let state = match raw_state.as_str() {
        "merged" => "merged",
        "closed" => "closed",
        _ if is_draft => "draft",
        _ => "open",
    };
    let checks = match v
        .get("head_pipeline")
        .and_then(|p| p.get("status"))
        .and_then(Value::as_str)
        .map(|s| s.to_lowercase())
        .as_deref()
    {
        Some("success" | "skipped") => "passing",
        Some("failed" | "canceled") => "failing",
        Some("running" | "pending" | "created" | "waiting_for_resource" | "preparing" | "manual") => "pending",
        _ => "none",
    };
    Ok(PrStatusInfo {
        provider: "gitlab".into(),
        number,
        title,
        url,
        state: state.into(),
        base,
        checks: checks.into(),
        worktree: None,
    })
}

fn extract_http_url(text: &str) -> Option<String> {
    text.split_whitespace()
        .rev()
        .map(|tok| tok.trim_matches(|c: char| matches!(c, '(' | ')' | '<' | '>' | '"' | '\'' | ',' | '.')))
        .find(|tok| tok.starts_with("https://") || tok.starts_with("http://"))
        .map(str::to_string)
}

fn unavailable_message(provider: &str, reason: &str) -> String {
    match (provider, reason) {
        ("github", "no_cli") => "Install GitHub CLI (gh) to open pull requests.".into(),
        ("github", _) => "Sign in with GitHub CLI (`gh auth login`) to open pull requests.".into(),
        ("gitlab", "no_cli") => "Install GitLab CLI (glab) to open merge requests.".into(),
        ("gitlab", _) => "Sign in with GitLab CLI (`glab auth login`) to open merge requests.".into(),
        _ => "Pull request integration is not configured.".into(),
    }
}

fn friendly_create_error(provider: &str, fail: &CliFailure) -> String {
    let combined = format!("{}\n{}", fail.stderr, fail.stdout);
    let lower = combined.to_lowercase();
    let kind = if provider == "gitlab" { "merge request" } else { "pull request" };
    if lower.contains("already exists") {
        return format!("A {kind} already exists for this branch.");
    }
    if lower.contains("auth login") || lower.contains("not logged into") || lower.contains("authentication") {
        return unavailable_message(provider, "no_auth");
    }
    if lower.contains("no commits between") {
        return format!("There are no new commits between this branch and the base branch to open a {kind}.");
    }
    if lower.contains("timed out") {
        return format!("Opening the {kind} timed out. Please check your network connection and try again.");
    }
    format!("Could not open the {kind}. Verify repository permissions and try again.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_cmd(dir: &Path, args: &[&str]) {
        run_git_checked(dir.to_str().unwrap(), args).unwrap();
    }

    #[cfg(unix)]
    fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    }

    #[test]
    fn parses_github_pr_status_and_checks_rollup() {
        let passing = parse_github_pr_json(
            r#"{
                "number": 42,
                "title": "Add inline review",
                "url": "https://github.com/acme/app/pull/42",
                "state": "OPEN",
                "isDraft": false,
                "baseRefName": "main",
                "statusCheckRollup": [
                    {"status": "COMPLETED", "conclusion": "SUCCESS"},
                    {"status": "COMPLETED", "conclusion": "SKIPPED"}
                ]
            }"#,
        )
        .unwrap();
        assert_eq!(passing.number, 42);
        assert_eq!(passing.state, "open");
        assert_eq!(passing.checks, "passing");

        let draft_pending = parse_github_pr_json(
            r#"{
                "number": 43,
                "title": "WIP",
                "url": "https://github.com/acme/app/pull/43",
                "state": "OPEN",
                "isDraft": true,
                "baseRefName": "main",
                "statusCheckRollup": [
                    {"status": "IN_PROGRESS", "conclusion": ""}
                ]
            }"#,
        )
        .unwrap();
        assert_eq!(draft_pending.state, "draft");
        assert_eq!(draft_pending.checks, "pending");

        let merged_failing = parse_github_pr_json(
            r#"{
                "number": 44,
                "title": "Land feature",
                "url": "https://github.com/acme/app/pull/44",
                "state": "MERGED",
                "isDraft": false,
                "baseRefName": "main",
                "statusCheckRollup": [
                    {"state": "FAILURE"}
                ]
            }"#,
        )
        .unwrap();
        assert_eq!(merged_failing.state, "merged");
        assert_eq!(merged_failing.checks, "failing");
    }

    #[test]
    fn parses_gitlab_mr_status_and_pipeline() {
        let mr = parse_gitlab_mr_json(
            r#"{
                "iid": 19,
                "title": "Support glab",
                "web_url": "https://gitlab.com/acme/app/-/merge_requests/19",
                "state": "opened",
                "draft": true,
                "target_branch": "main",
                "head_pipeline": {"status": "running"}
            }"#,
        )
        .unwrap();
        assert_eq!(mr.provider, "gitlab");
        assert_eq!(mr.number, 19);
        assert_eq!(mr.state, "draft");
        assert_eq!(mr.checks, "pending");
    }

    #[cfg(unix)]
    #[test]
    fn detects_provider_opens_pr_and_reads_worktree_status_via_fake_gh() {
        let base_dir = std::env::temp_dir().join(format!("vlx-pr-test-{}", Uuid::new_v4()));
        let bare = base_dir.join("origin.git");
        let main_repo = base_dir.join("repo");
        let bin_dir = base_dir.join("bin");
        std::fs::create_dir_all(&bare).unwrap();
        std::fs::create_dir_all(&main_repo).unwrap();
        std::fs::create_dir_all(&bin_dir).unwrap();

        git_cmd(&bare, &["init", "-q", "--bare"]);
        git_cmd(&main_repo, &["init", "-q", "-b", "main"]);
        git_cmd(&main_repo, &["config", "user.email", "t@example.com"]);
        git_cmd(&main_repo, &["config", "user.name", "T"]);
        git_cmd(&main_repo, &["config", "commit.gpgsign", "false"]);
        std::fs::write(main_repo.join("README.md"), "hello\n").unwrap();
        git_cmd(&main_repo, &["add", "-A"]);
        git_cmd(&main_repo, &["commit", "-qm", "init"]);
        // Point origin at bare for push while setting pushurl/url or testing github host detection:
        // configure a github URL with url.<bare>.insteadOf so git push stays local and offline!
        let bare_str = bare.to_string_lossy().to_string();
        git_cmd(
            &main_repo,
            &["config", &format!("url.{bare_str}.pushInsteadOf"), "https://github.com/acme/demo.git"],
        );
        git_cmd(&main_repo, &["remote", "add", "origin", "https://github.com/acme/demo.git"]);
        git_cmd(&main_repo, &["push", "-q", "-u", "origin", "main"]);

        let wt_info = worktree_add(main_repo.to_str().unwrap(), "feature").unwrap();
        std::fs::write(Path::new(&wt_info.path).join("README.md"), "hello\nworld\n").unwrap();
        git_cmd(Path::new(&wt_info.path), &["add", "-A"]);
        git_cmd(Path::new(&wt_info.path), &["commit", "-qm", "Add world"]);

        let args_log = base_dir.join("gh_args.txt");
        let fake_gh = write_script(
            &bin_dir,
            "gh",
            &format!(
                r#"if [ "$1" = "auth" ]; then exit 0; fi
if [ "$1" = "pr" ] && [ "$2" = "create" ]; then
  printf '%s\n' "$@" > "{}"
  echo "https://github.com/acme/demo/pull/99"
  exit 0
fi
if [ "$1" = "pr" ] && [ "$2" = "view" ]; then
  echo '{{"number":99,"title":"Add world","url":"https://github.com/acme/demo/pull/99","state":"MERGED","isDraft":false,"baseRefName":"main","statusCheckRollup":[{{"status":"COMPLETED","conclusion":"SUCCESS"}}]}}'
  exit 0
fi
exit 1"#,
                args_log.display()
            ),
        );

        let db = crate::db::Db::open(&base_dir.join("test.db")).unwrap();
        let overrides = CliOverrides { gh: Some(&fake_gh), glab: None };

        let detected = detect_provider_with(&db, "test", &wt_info.path, overrides);
        assert_eq!(detected.provider.as_deref(), Some("github"));
        assert!(detected.available);
        assert_eq!(detected.reason, "ok");

        let created = open_pull_request_with(
            &db,
            "test",
            &wt_info.path,
            &wt_info.branch,
            "refs/remotes/origin/main",
            "Add world",
            "Body notes",
            true,
            overrides,
        )
        .unwrap();
        assert_eq!(created.url, "https://github.com/acme/demo/pull/99");
        let logged = std::fs::read_to_string(&args_log).unwrap();
        assert!(logged.contains("--base\nmain\n"), "logged={logged}");
        assert!(logged.contains("--draft"), "logged={logged}");

        let status = current_pr_status_with(&db, "test", &wt_info.path, overrides)
            .unwrap()
            .unwrap();
        assert_eq!(status.number, 99);
        assert_eq!(status.state, "merged");
        assert_eq!(status.checks, "passing");
        assert!(status.worktree.is_some());

        // Main repository checkout reports worktree: None.
        let main_status = current_pr_status_with(&db, "test", main_repo.to_str().unwrap(), overrides)
            .unwrap()
            .unwrap();
        assert!(main_status.worktree.is_none());

        // Switch origin to GitLab and verify missing CLI vs authenticated fake glab.
        git_cmd(
            Path::new(&wt_info.path),
            &["remote", "set-url", "origin", "https://gitlab.com/acme/demo.git"],
        );
        git_cmd(
            &main_repo,
            &["config", &format!("url.{bare_str}.pushInsteadOf"), "https://gitlab.com/acme/demo.git"],
        );
        let no_glab = detect_provider_with(
            &db,
            "test",
            &wt_info.path,
            CliOverrides { gh: Some(&fake_gh), glab: Some(&base_dir.join("missing-glab")) },
        );
        assert_eq!(no_glab.provider.as_deref(), Some("gitlab"));
        assert!(!no_glab.available);
        assert_eq!(no_glab.reason, "no_cli");

        let glab_log = base_dir.join("glab_args.txt");
        let fake_glab = write_script(
            &bin_dir,
            "glab",
            &format!(
                r#"if [ "$1" = "auth" ]; then exit 0; fi
if [ "$1" = "mr" ] && [ "$2" = "create" ]; then
  printf '%s\n' "$@" > "{}"
  echo "https://gitlab.com/acme/demo/-/merge_requests/12"
  exit 0
fi
if [ "$1" = "mr" ] && [ "$2" = "view" ]; then
  echo '{{"iid":12,"title":"Add world","web_url":"https://gitlab.com/acme/demo/-/merge_requests/12","state":"opened","draft":false,"target_branch":"main","head_pipeline":{{"status":"success"}}}}'
  exit 0
fi
exit 1"#,
                glab_log.display()
            ),
        );
        let glab_overrides = CliOverrides { gh: None, glab: Some(&fake_glab) };
        let mr_created = open_pull_request_with(
            &db,
            "test",
            &wt_info.path,
            &wt_info.branch,
            "refs/remotes/origin/main",
            "Add world",
            "MR description",
            false,
            glab_overrides,
        )
        .unwrap();
        assert_eq!(mr_created.url, "https://gitlab.com/acme/demo/-/merge_requests/12");
        let mr_logged = std::fs::read_to_string(&glab_log).unwrap();
        assert!(mr_logged.contains("--target-branch\nmain\n"), "mr_logged={mr_logged}");
        assert!(mr_logged.contains("--yes"), "mr_logged={mr_logged}");

        let _ = std::fs::remove_dir_all(&base_dir);
    }
}
