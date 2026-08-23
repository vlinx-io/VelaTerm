//! Inspect repository state through the system Git CLI without depending on libgit2.

use serde::Serialize;
use std::collections::HashMap;
use std::io::Read;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Git clone progress event, delivered directly through Tauri or forwarded globally over WebSocket.
pub const CLONE_PROGRESS_EVENT: &str = "git://clone-progress";
/// Stable cancellation code that lets the frontend distinguish user cancellation from failure.
pub const CLONE_CANCELLED_ERROR: &str = "CLONE_CANCELLED";

/// Stable phase parsed from `git clone --progress`; the frontend localizes it instead of showing Git output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneProgress {
    pub operation_id: String,
    pub stage: String,
    pub percent: Option<u8>,
}

struct CloneControl {
    cancelled: Arc<AtomicBool>,
    /// Origin (desktop/ws-N/system); only the same origin may cancel, isolating logged-in devices.
    owner: String,
    /// false means cancellation arrived before worker registration; reuse it to close that race.
    registered: bool,
    touched_at: Instant,
}

static CLONE_CONTROLS: OnceLock<Mutex<HashMap<String, CloneControl>>> = OnceLock::new();

fn clone_controls() -> &'static Mutex<HashMap<String, CloneControl>> {
    CLONE_CONTROLS.get_or_init(|| Mutex::new(HashMap::new()))
}

struct CloneControlGuard {
    operation_id: String,
}

impl Drop for CloneControlGuard {
    fn drop(&mut self) {
        if let Ok(mut controls) = clone_controls().lock() {
            controls.remove(&self.operation_id);
        }
    }
}

fn register_clone(
    operation_id: &str,
    owner: &str,
) -> Result<(Arc<AtomicBool>, CloneControlGuard), String> {
    Uuid::parse_str(operation_id).map_err(|_| "Invalid clone operation id".to_string())?;
    let mut controls = clone_controls()
        .lock()
        .map_err(|_| "Clone control registry is unavailable".to_string())?;
    // Early cancellation briefly leaves an unregistered race marker; expired markers are safe to remove.
    controls.retain(|_, entry| {
        entry.registered || entry.touched_at.elapsed() < Duration::from_secs(300)
    });
    let cancelled = match controls.get_mut(operation_id) {
        Some(entry) if entry.owner != owner => {
            return Err("Clone operation belongs to another client".to_string());
        }
        Some(entry) if entry.registered => {
            return Err("A clone with this operation id is already running".to_string());
        }
        Some(entry) => {
            entry.registered = true;
            entry.touched_at = Instant::now();
            Arc::clone(&entry.cancelled)
        }
        None => {
            let cancelled = Arc::new(AtomicBool::new(false));
            controls.insert(
                operation_id.to_string(),
                CloneControl {
                    cancelled: Arc::clone(&cancelled),
                    owner: owner.to_string(),
                    registered: true,
                    touched_at: Instant::now(),
                },
            );
            cancelled
        }
    };
    Ok((
        cancelled,
        CloneControlGuard {
            operation_id: operation_id.to_string(),
        },
    ))
}

/// Request clone cancellation, leaving a short-lived marker if the worker has not registered yet.
pub fn cancel_clone(operation_id: &str, owner: &str) -> Result<bool, String> {
    Uuid::parse_str(operation_id).map_err(|_| "Invalid clone operation id".to_string())?;
    let mut controls = clone_controls()
        .lock()
        .map_err(|_| "Clone control registry is unavailable".to_string())?;
    controls.retain(|_, entry| {
        entry.registered || entry.touched_at.elapsed() < Duration::from_secs(300)
    });
    if let Some(entry) = controls.get_mut(operation_id) {
        if entry.owner != owner {
            return Err("Clone operation belongs to another client".to_string());
        }
        entry.cancelled.store(true, Ordering::Release);
        entry.touched_at = Instant::now();
        return Ok(true);
    }
    controls.insert(
        operation_id.to_string(),
        CloneControl {
            cancelled: Arc::new(AtomicBool::new(true)),
            owner: owner.to_string(),
            registered: false,
            touched_at: Instant::now(),
        },
    );
    Ok(true)
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    /// Whether this is a linked worktree created by `git worktree add`, rather than the main tree.
    pub is_worktree: bool,
    /// Top-level worktree directory for frontend display; present only for linked worktrees.
    pub worktree_path: Option<String>,
}

impl GitStatus {
    fn not_repo() -> Self {
        Self {
            is_repo: false,
            branch: None,
            ahead: 0,
            behind: 0,
            staged: 0,
            unstaged: 0,
            untracked: 0,
            is_worktree: false,
            worktree_path: None,
        }
    }
}

/// Run Git in a directory and return stdout with only the trailing newline removed.
///
/// Use this for column-sensitive output. `git status --porcelain` encodes the index state in
/// column 0, so an unstaged-only change starts with a space (` M README.md`); trimming the whole
/// output eats that space on the first line, shifting every column after it.
fn run_git_raw(path: &str, args: &[&str]) -> Option<String> {
    let output = crate::host::command("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Some(text.trim_end_matches(['\n', '\r']).to_string())
}

/// Run Git in a directory and return trimmed stdout on success.
fn run_git(path: &str, args: &[&str]) -> Option<String> {
    run_git_raw(path, args).map(|s| s.trim().to_string())
}

/// Inspect Git state; return is_repo=false for non-repositories or unavailable Git.
pub fn status(path: &str) -> GitStatus {
    let is_repo = run_git(path, &["rev-parse", "--is-inside-work-tree"])
        .map(|s| s == "true")
        .unwrap_or(false);
    if !is_repo {
        return GitStatus::not_repo();
    }

    // A detached HEAD makes `--abbrev-ref` answer with the literal "HEAD", which is not a branch name.
    // Reporting None lets the UI say "detached" instead of showing a branch called HEAD.
    let branch = run_git(path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .filter(|s| !s.is_empty() && s != "HEAD");

    // ahead/behind are relative to upstream; use 0/0 when no upstream exists.
    let (mut ahead, mut behind) = (0, 0);
    if let Some(counts) = run_git(
        path,
        &["rev-list", "--left-right", "--count", "HEAD...@{u}"],
    ) {
        let mut it = counts.split_whitespace();
        ahead = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        behind = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    }

    // Parse porcelain output into staged, unstaged, and untracked counts.
    let (mut staged, mut unstaged, mut untracked) = (0, 0, 0);
    if let Some(porcelain) = run_git_raw(path, &["status", "--porcelain"]) {
        for line in porcelain.lines() {
            let bytes = line.as_bytes();
            if bytes.len() < 2 {
                continue;
            }
            let x = bytes[0] as char;
            let y = bytes[1] as char;
            if x == '?' && y == '?' {
                untracked += 1;
                continue;
            }
            if x != ' ' && x != '?' {
                staged += 1;
            }
            if y != ' ' && y != '?' {
                unstaged += 1;
            }
        }
    }

    // A linked worktree's git-dir differs from its common git-dir; the main worktree's paths match.
    //
    // Do not switch back to `--path-format=absolute`, introduced only in Git 2.31. Older Git echoes
    // the unknown option as a revision, shifting parsed lines and misclassifying ordinary repos as
    // worktrees. Use Git 2.13+'s `--absolute-git-dir`, resolve a relative common-dir against cwd,
    // canonicalize both paths, and compare them.
    let (mut is_worktree, mut worktree_path) = (false, None);
    // Normalize Git-returned paths into canonical absolute paths for exact comparison.
    let abs_git_path = |p: &str| -> Option<std::path::PathBuf> {
        let pb = std::path::Path::new(p);
        let joined = if pb.is_absolute() {
            pb.to_path_buf()
        } else {
            std::path::Path::new(path).join(pb)
        };
        std::fs::canonicalize(joined).ok()
    };
    if let (Some(git_dir), Some(common_dir)) = (
        run_git(path, &["rev-parse", "--absolute-git-dir"]),
        run_git(path, &["rev-parse", "--git-common-dir"]),
    ) {
        if let (Some(gd), Some(cd)) = (abs_git_path(&git_dir), abs_git_path(&common_dir)) {
            if gd != cd {
                is_worktree = true;
                // Obtain the worktree root for frontend directory-name display.
                worktree_path =
                    run_git(path, &["rev-parse", "--show-toplevel"]).filter(|s| !s.is_empty());
            }
        }
    }

    GitStatus {
        is_repo: true,
        branch,
        ahead,
        behind,
        staged,
        unstaged,
        untracked,
        is_worktree,
        worktree_path,
    }
}

/// Information about a Git worktree created for a session.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: String,
    /// Full base ref from the repository root at creation time; landing targets it. Detached uses SHA.
    pub base_ref: String,
}

/// Resolve HEAD to a full branch ref or detached commit SHA. Worktree creation records this base
/// independently from session parentage.
fn resolve_head_ref(repo: &str) -> Option<String> {
    if let Some(r) = run_git(repo, &["symbolic-ref", "-q", "HEAD"]).filter(|s| !s.is_empty()) {
        return Some(r); // refs/heads/<branch>
    }
    run_git(repo, &["rev-parse", "HEAD"]).filter(|s| !s.is_empty())
}

/// Convert a name into a branch/path slug: ASCII alphanumerics, hyphen, underscore; max 40 chars.
fn slugify(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let trimmed = s.trim_matches('-');
    let base = if trimmed.is_empty() { "task" } else { trimmed };
    base.chars().take(40).collect()
}

/// Create a session worktree at `.vlx-worktrees/<slug>-<short>` on branch `vlx/<slug>-<short>`.
/// Return Err for non-repositories or failure so callers can fall back without a worktree.
pub fn worktree_add(repo_root: &str, name: &str) -> Result<WorktreeInfo, String> {
    let is_repo = run_git(repo_root, &["rev-parse", "--is-inside-work-tree"])
        .map(|s| s == "true")
        .unwrap_or(false);
    if !is_repo {
        return Err(format!("Not a git repository: {repo_root}"));
    }
    // Place .vlx-worktrees at the repository root, not beneath an arbitrary cwd.
    let top = run_git(repo_root, &["rev-parse", "--show-toplevel"])
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| repo_root.to_string());

    // Record the root's current branch before adding the worktree. Detached HEAD uses its SHA; if
    // unresolved, store empty so landing can fall back to the main worktree's current branch.
    let base_ref = resolve_head_ref(&top).unwrap_or_default();

    let slug = slugify(name);
    let uuid = Uuid::new_v4().to_string();
    let leaf = format!("{slug}-{}", &uuid[..6]);
    let branch = format!("vlx/{leaf}");
    let path = std::path::Path::new(&top)
        .join(".vlx-worktrees")
        .join(&leaf);
    let path_str = path.to_string_lossy().to_string();

    let out = crate::host::command("git")
        .arg("-C")
        .arg(&top)
        .args(["worktree", "add", path_str.as_str(), "-b", branch.as_str()])
        .output()
        .map_err(|e| format!("Failed to run git worktree add: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("Failed to create worktree: {err}"));
    }
    // Add .vlx-worktrees/ to repository-local info/exclude; failure is nonfatal.
    ensure_worktrees_ignored(&top);
    Ok(WorktreeInfo {
        path: path_str,
        branch,
        base_ref,
    })
}

/// Derive a clone directory from the last URL/SCP-style path segment, removing `.git`; return None
/// when no valid name can be parsed.
pub fn derive_clone_dir_name(url: &str) -> Option<String> {
    let trimmed = url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let last = trimmed
        .rsplit(|c| c == '/' || c == ':')
        .next()
        .unwrap_or(trimmed);
    let name = last.strip_suffix(".git").unwrap_or(last).trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn clone_percent(line: &str) -> Option<u8> {
    let percent_at = line.find('%')?;
    let bytes = line.as_bytes();
    let mut start = percent_at;
    while start > 0 && bytes[start - 1].is_ascii_digit() {
        start -= 1;
    }
    if start == percent_at {
        return None;
    }
    line[start..percent_at]
        .parse::<u8>()
        .ok()
        .filter(|p| *p <= 100)
}

/// Normalize C-locale Git progress into stable phases; ignore unrelated lines.
fn parse_clone_progress(line: &str) -> Option<(&'static str, Option<u8>)> {
    let line = line.trim();
    if line.starts_with("Cloning into ") {
        return Some(("connecting", None));
    }
    let line = line.strip_prefix("remote: ").unwrap_or(line);
    if line.starts_with("Enumerating objects:")
        || line.starts_with("Counting objects:")
        || line.starts_with("Compressing objects:")
    {
        return Some(("preparing", clone_percent(line)));
    }
    if line.starts_with("Receiving objects:") {
        return Some(("receiving", clone_percent(line)));
    }
    if line.starts_with("Resolving deltas:") {
        return Some(("resolving", clone_percent(line)));
    }
    if line.starts_with("Updating files:") || line.starts_with("Filtering content:") {
        return Some(("checkout", clone_percent(line)));
    }
    None
}

/// Split Git's in-place progress on both carriage returns and newlines for immediate delivery.
fn read_clone_stderr(mut stderr: impl Read, tx: std::sync::mpsc::Sender<String>) {
    let mut chunk = [0_u8; 4096];
    let mut pending = Vec::new();
    loop {
        match stderr.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                for byte in &chunk[..n] {
                    if *byte == b'\r' || *byte == b'\n' {
                        if !pending.is_empty() {
                            let line = String::from_utf8_lossy(&pending).trim().to_string();
                            pending.clear();
                            if !line.is_empty() && tx.send(line).is_err() {
                                return;
                            }
                        }
                    } else {
                        pending.push(*byte);
                    }
                }
            }
            Err(_) => break,
        }
    }
    if !pending.is_empty() {
        let line = String::from_utf8_lossy(&pending).trim().to_string();
        if !line.is_empty() {
            let _ = tx.send(line);
        }
    }
}

/// Sanitize credentials/query tokens from URLs before returning user-visible errors.
fn redact_clone_error(line: &str) -> String {
    let mut out = line.to_string();
    let mut search_from = 0;
    while let Some(scheme_rel) = out[search_from..].find("://") {
        let authority_start = search_from + scheme_rel + 3;
        let authority_end = out[authority_start..]
            .find(|c: char| c == '/' || c.is_whitespace() || c == '\'' || c == '"')
            .map(|i| authority_start + i)
            .unwrap_or(out.len());
        if let Some(at_rel) = out[authority_start..authority_end].rfind('@') {
            let at = authority_start + at_rel;
            out.replace_range(authority_start..at, "***");
            search_from = authority_start + 4;
        } else {
            search_from = authority_end.max(authority_start + 1);
        }
        if search_from >= out.len() {
            break;
        }
    }
    // Query parameters often contain access tokens; retain only the diagnostic address body.
    if let Some(query) = out.find('?') {
        let end = out[query..]
            .find(|c: char| c.is_whitespace() || c == '\'' || c == '"')
            .map(|i| query + i)
            .unwrap_or(out.len());
        out.replace_range(query..end, "?<redacted>");
    }
    out
}

fn remove_partial_clone(path: &std::path::Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    std::fs::remove_dir_all(path)
        .err()
        .map(|e| format!("; partial clone remains at {}: {e}", path.to_string_lossy()))
}

/// Clone audit logs stable operation ID, phase, result, and duration—never URL, branch, path, or raw Git output.
fn clone_audit(level: &str, operation_id: &str, status: &str, duration_ms: u128) {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    eprintln!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} [{:<5}] [{}] event=git_clone step=clone method=git_cli inputCount=1 outputCount={} jobId={} status={} durationMs={}",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
        level,
        operation_id,
        if status == "success" { 1 } else { 0 },
        operation_id,
        status,
        duration_ms,
    );
}

fn terminate_clone_process(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // Signal the negative process-group ID to terminate Git helpers and SSH together on Unix.
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
    }
    #[cfg(windows)]
    {
        // Windows Child::kill misses helpers; taskkill /T removes the full process tree.
        let pid = child.id().to_string();
        let _ = crate::host::command("taskkill")
            .args(["/PID", pid.as_str(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

/// Cancellable Git clone with progress reporting.
///
/// Clone into an operation-private hidden directory and atomically rename it on success, allowing safe
/// cleanup/retry after failure or cancellation. Disable stdin/terminal prompts while still allowing
/// configured credential helpers and SSH agents, preventing invisible waits in a headless service.
pub fn clone_to_with_progress(
    url: &str,
    parent_dir: &str,
    folder_name: Option<&str>,
    branch: Option<&str>,
    operation_id: &str,
    owner: &str,
    on_progress: impl FnMut(CloneProgress),
) -> Result<String, String> {
    Uuid::parse_str(operation_id).map_err(|_| "Invalid clone operation id".to_string())?;
    let started = Instant::now();
    clone_audit("INFO", operation_id, "started", 0);
    let result = clone_to_with_progress_inner(
        url,
        parent_dir,
        folder_name,
        branch,
        operation_id,
        owner,
        on_progress,
    );
    let duration_ms = started.elapsed().as_millis();
    match &result {
        Ok(_) => clone_audit("INFO", operation_id, "success", duration_ms),
        Err(error) if error == CLONE_CANCELLED_ERROR => {
            clone_audit("INFO", operation_id, "cancelled", duration_ms)
        }
        Err(_) => clone_audit("ERROR", operation_id, "failed", duration_ms),
    }
    result
}

fn clone_to_with_progress_inner(
    url: &str,
    parent_dir: &str,
    folder_name: Option<&str>,
    branch: Option<&str>,
    operation_id: &str,
    owner: &str,
    mut on_progress: impl FnMut(CloneProgress),
) -> Result<String, String> {
    let (cancelled, _control_guard) = register_clone(operation_id, owner)?;
    let url = url.trim();
    if url.is_empty() {
        return Err("Repository URL is empty".to_string());
    }
    let parent = std::path::Path::new(parent_dir);
    if !parent.is_dir() {
        return Err(format!("Parent directory does not exist: {parent_dir}"));
    }
    let folder = folder_name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| derive_clone_dir_name(url))
        .ok_or_else(|| "Could not determine a target folder name from the URL".to_string())?;
    if folder.contains('/') || folder.contains('\\') {
        return Err(format!("Invalid folder name: {folder}"));
    }
    let target = parent.join(&folder);
    if target.exists() {
        return Err(format!(
            "Target already exists: {}",
            target.to_string_lossy()
        ));
    }
    let target_str = target.to_string_lossy().to_string();
    let temporary = parent.join(format!(".vlx-clone-{operation_id}.tmp"));
    if temporary.exists() {
        return Err(format!(
            "Temporary clone directory already exists: {}",
            temporary.to_string_lossy()
        ));
    }
    let temporary_str = temporary.to_string_lossy().to_string();
    let branch = branch.map(str::trim).filter(|s| !s.is_empty());

    let mut git_args: Vec<&str> = vec!["clone", "--progress"];
    if let Some(b) = branch {
        git_args.push("--branch");
        git_args.push(b);
    }
    git_args.push("--");
    git_args.push(url);
    git_args.push(temporary_str.as_str());

    on_progress(CloneProgress {
        operation_id: operation_id.to_string(),
        stage: "starting".to_string(),
        percent: None,
    });

    let mut command = crate::host::command("git");
    command
        .args(&git_args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .env("SSH_ASKPASS_REQUIRE", "never")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to run git clone: {e}"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture git clone progress".to_string())?;
    let (line_tx, line_rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || read_clone_stderr(stderr, line_tx));
    let mut last_progress: Option<(&'static str, Option<u8>)> = None;
    let mut error_lines: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    let mut was_cancelled = false;

    let status = loop {
        while let Ok(line) = line_rx.try_recv() {
            if let Some(progress) = parse_clone_progress(&line) {
                if last_progress != Some(progress) {
                    on_progress(CloneProgress {
                        operation_id: operation_id.to_string(),
                        stage: progress.0.to_string(),
                        percent: progress.1,
                    });
                    last_progress = Some(progress);
                }
            } else {
                error_lines.push_back(redact_clone_error(&line));
                if error_lines.len() > 24 {
                    error_lines.pop_front();
                }
            }
        }

        if cancelled.load(Ordering::Acquire) && !was_cancelled {
            was_cancelled = true;
            terminate_clone_process(&mut child);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                terminate_clone_process(&mut child);
                let suffix = remove_partial_clone(&temporary).unwrap_or_default();
                return Err(format!("Failed while waiting for git clone: {e}{suffix}"));
            }
        }
    };

    let _ = reader.join();
    while let Ok(line) = line_rx.try_recv() {
        if let Some(progress) = parse_clone_progress(&line) {
            if last_progress != Some(progress) {
                on_progress(CloneProgress {
                    operation_id: operation_id.to_string(),
                    stage: progress.0.to_string(),
                    percent: progress.1,
                });
                last_progress = Some(progress);
            }
        } else {
            error_lines.push_back(redact_clone_error(&line));
            if error_lines.len() > 24 {
                error_lines.pop_front();
            }
        }
    }

    if was_cancelled || cancelled.load(Ordering::Acquire) {
        return match remove_partial_clone(&temporary) {
            Some(cleanup_error) => Err(format!("Clone cancelled{cleanup_error}")),
            None => Err(CLONE_CANCELLED_ERROR.to_string()),
        };
    }
    if !status.success() {
        let suffix = remove_partial_clone(&temporary).unwrap_or_default();
        let detail = error_lines
            .iter()
            .rev()
            .find(|line| line.starts_with("fatal:") || line.starts_with("error:"))
            .or_else(|| error_lines.back())
            .cloned()
            .unwrap_or_else(|| "git clone failed".to_string());
        return Err(format!("{detail}{suffix}"));
    }

    on_progress(CloneProgress {
        operation_id: operation_id.to_string(),
        stage: "finalizing".to_string(),
        percent: None,
    });
    if target.exists() {
        let suffix = remove_partial_clone(&temporary).unwrap_or_default();
        return Err(format!(
            "Target appeared while cloning: {target_str}{suffix}"
        ));
    }
    if let Err(e) = std::fs::rename(&temporary, &target) {
        let suffix = remove_partial_clone(&temporary).unwrap_or_default();
        return Err(format!(
            "Failed to finalize cloned repository at {target_str}: {e}{suffix}"
        ));
    }
    Ok(target_str)
}

/// Clone into `parent_dir/<folder>` and return the full path. Derive an omitted folder from the URL,
/// reject path separators/traversal, optionally select a branch/tag, and never overwrite an existing
/// target. Use system credentials/SSH agent with interactive prompts disabled.
#[allow(dead_code)] // Retained for headless internal calls and tests; UI uses the progress-aware version.
pub fn clone_to(
    url: &str,
    parent_dir: &str,
    folder_name: Option<&str>,
    branch: Option<&str>,
) -> Result<String, String> {
    let operation_id = Uuid::new_v4().to_string();
    clone_to_with_progress(
        url,
        parent_dir,
        folder_name,
        branch,
        &operation_id,
        "system",
        |_| {},
    )
}

/// Existing `git worktree list` entry available for mounting into a new session.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeEntry {
    /// Top-level worktree directory.
    pub path: String,
    /// Short checked-out branch name, or None for detached HEAD.
    pub branch: Option<String>,
    /// Current HEAD SHA, always supplied by porcelain output.
    pub head: Option<String>,
    /// Whether this is the first-listed main worktree, which the selection UI excludes.
    pub is_main: bool,
    /// Whether this is a bare entry without a working tree, also excluded from selection.
    pub is_bare: bool,
}

/// Parse all porcelain worktrees for repo_root, including the first/main entry; return Err when unavailable.
pub fn worktree_list(repo_root: &str) -> Result<Vec<WorktreeEntry>, String> {
    let is_repo = run_git(repo_root, &["rev-parse", "--is-inside-work-tree"])
        .map(|s| s == "true")
        .unwrap_or(false);
    if !is_repo {
        return Err(format!("Not a git repository: {repo_root}"));
    }
    let out = run_git(repo_root, &["worktree", "list", "--porcelain"])
        .ok_or_else(|| "Failed to run git worktree list".to_string())?;

    // Porcelain consists of blank-separated records with worktree, HEAD, branch, detached, or bare lines.
    let mut entries: Vec<WorktreeEntry> = Vec::new();
    let mut cur: Option<WorktreeEntry> = None;
    for line in out.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            if let Some(e) = cur.take() {
                entries.push(e);
            }
            cur = Some(WorktreeEntry {
                path: p.to_string(),
                branch: None,
                head: None,
                is_main: false,
                is_bare: false,
            });
        } else if let Some(h) = line.strip_prefix("HEAD ") {
            if let Some(e) = cur.as_mut() {
                e.head = Some(h.to_string());
            }
        } else if let Some(b) = line.strip_prefix("branch ") {
            if let Some(e) = cur.as_mut() {
                e.branch = Some(b.strip_prefix("refs/heads/").unwrap_or(b).to_string());
            }
        } else if line == "bare" {
            if let Some(e) = cur.as_mut() {
                e.is_bare = true;
            }
        }
        // Other lines such as detached/locked/prunable need no handling; branch remains None.
    }
    if let Some(e) = cur.take() {
        entries.push(e);
    }
    // Git always lists the main worktree first.
    if let Some(first) = entries.first_mut() {
        first.is_main = true;
    }
    Ok(entries)
}

/// Idempotently ignore `.vlx-worktrees/` through repository-local `.git/info/exclude`, keeping
/// vlx-term-specific configuration out of shared `.gitignore`. Silently ignore lookup/write failures.
fn ensure_worktrees_ignored(repo_dir: &str) {
    // Respect any existing ignore source and avoid duplicate entries.
    if run_git(repo_dir, &["check-ignore", ".vlx-worktrees"])
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        return;
    }
    let common = match run_git(
        repo_dir,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    ) {
        Some(d) if !d.is_empty() => d,
        _ => return,
    };
    let exclude = std::path::Path::new(&common).join("info").join("exclude");
    let existing = std::fs::read_to_string(&exclude).unwrap_or_default();
    // Treat entries with or without a trailing slash as equivalent.
    if existing
        .lines()
        .any(|l| matches!(l.trim(), ".vlx-worktrees" | ".vlx-worktrees/"))
    {
        return;
    }
    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str("# vlx-term worktrees (auto-added)\n.vlx-worktrees/\n");
    let _ = std::fs::write(&exclude, content);
}

/// Remove a worktree; force permits removal despite local changes.
pub fn worktree_remove(path: &str, force: bool) -> Result<(), String> {
    let mut args: Vec<&str> = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(path);
    let out = crate::host::command("git")
        .arg("-C")
        .arg(path)
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run git worktree remove: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("Failed to remove worktree: {err}"));
    }
    Ok(())
}

/// Merge result: success or preserved conflict state with affected files.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeOutcome {
    /// Whether the merge completed successfully.
    pub merged: bool,
    /// Whether conflicts remain for manual resolution; then merged is false.
    pub conflict: bool,
    /// Conflicting files, nonempty when conflict is true.
    pub conflicts: Vec<String>,
    /// User-facing English explanation.
    pub message: String,
}

/// Return the current branch for a worktree/repository, falling back to short HEAD when detached.
pub fn worktree_branch(path: &str) -> Option<String> {
    if let Some(b) = run_git(path, &["branch", "--show-current"]).filter(|s| !s.is_empty()) {
        return Some(b);
    }
    // Display short HEAD for detached state.
    run_git(path, &["rev-parse", "--short", "HEAD"]).filter(|s| !s.is_empty())
}

/// Return a repository's origin URL for Gitea detection/push, or None when absent/not a repository.
pub fn remote_origin_url(path: &str) -> Option<String> {
    run_git(path, &["remote", "get-url", "origin"]).filter(|s| !s.is_empty())
}

/// Push the current worktree branch to origin with upstream tracking before opening a PR.
pub fn push_branch(wt_path: &str, branch: &str) -> Result<(), String> {
    let out = crate::host::command("git")
        .arg("-C")
        .arg(wt_path)
        .args(["push", "-u", "origin", branch])
        .output()
        .map_err(|e| format!("Failed to run git push: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("git push failed: {err}"));
    }
    Ok(())
}

/// Whether a directory has uncommitted changes.
pub fn worktree_has_changes(path: &str) -> bool {
    run_git(path, &["status", "--porcelain"])
        .map(|s| !s.is_empty())
        .unwrap_or(false)
}

/// Commit all changes in a directory so a merge can include a child worktree's uncommitted work.
pub fn commit_all(path: &str, message: &str) -> Result<(), String> {
    let add = crate::host::command("git")
        .arg("-C")
        .arg(path)
        .args(["add", "-A"])
        .output()
        .map_err(|e| format!("Failed to run git add: {e}"))?;
    if !add.status.success() {
        let err = String::from_utf8_lossy(&add.stderr).trim().to_string();
        return Err(format!("git add failed: {err}"));
    }
    let out = crate::host::command("git")
        .arg("-C")
        .arg(path)
        .args(["commit", "-m", message])
        .output()
        .map_err(|e| format!("Failed to run git commit: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("git commit failed: {err}"));
    }
    Ok(())
}

/// Force-delete a local branch when cleaning up a merged child session.
pub fn branch_delete(repo: &str, branch: &str) -> Result<(), String> {
    let out = crate::host::command("git")
        .arg("-C")
        .arg(repo)
        .args(["branch", "-D", branch])
        .output()
        .map_err(|e| format!("Failed to run git branch -D: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("Failed to delete branch: {err}"));
    }
    Ok(())
}

// ─────────────────────────── Worktree landing ───────────────────────────
// Each worktree records a baseRef at creation. Landing merges its work back into that baseline.
// Providers are extensible: local merge first, with Gitea PR support added separately.

/// Landing-provider availability for frontend actions.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandProvider {
    /// Provider ID: `local` merge or later `gitea` PR.
    pub id: String,
    /// Whether it is currently available; otherwise the frontend disables it and explains reason.
    pub available: bool,
    /// Stable availability reason localized by the frontend: `ok`, `base_not_checked_out`, etc.
    /// `"base_remote"` / `"no_base"`。
    pub reason: String,
}

/// Landing preflight: target baseline, diff summary, dirty states, and available providers.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandTargets {
    /// Full baseline ref; legacy worktrees fall back to the main worktree's current branch.
    pub base_ref: String,
    /// Short baseline name for display.
    pub base_branch: String,
    /// Current worktree branch.
    pub branch: String,
    /// Summary of all committed, uncommitted, and untracked content landing would introduce.
    pub diff_stat: String,
    /// Whether the worktree needs a commit before local merge.
    pub has_uncommitted: bool,
    /// Worktree directory where the baseline is checked out, or None.
    pub base_dir: Option<String>,
    /// Whether the baseline worktree is dirty and may block merge.
    pub base_dir_dirty: bool,
    /// Available providers; initially only local.
    pub providers: Vec<LandProvider>,
}

/// Shorten branch refs; preserve other values such as raw SHAs.
fn short_ref(full: &str) -> String {
    full.strip_prefix("refs/heads/")
        .or_else(|| full.strip_prefix("refs/remotes/"))
        .unwrap_or(full)
        .to_string()
}

/// Worktree-list record containing directory and full checked-out ref, or None when detached.
struct WtEntry {
    path: String,
    branch: Option<String>,
}

/// Parse porcelain worktrees, main first; return an empty list on failure.
fn list_worktrees(any_path: &str) -> Vec<WtEntry> {
    let out = match run_git(any_path, &["worktree", "list", "--porcelain"]) {
        Some(s) if !s.is_empty() => s,
        _ => return Vec::new(),
    };
    let mut res: Vec<WtEntry> = Vec::new();
    let mut cur_path: Option<String> = None;
    let mut cur_branch: Option<String> = None;
    for line in out.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            if let Some(path) = cur_path.take() {
                res.push(WtEntry {
                    path,
                    branch: cur_branch.take(),
                });
            }
            cur_path = Some(p.trim().to_string());
            cur_branch = None;
        } else if let Some(b) = line.strip_prefix("branch ") {
            cur_branch = Some(b.trim().to_string());
        }
        // A detached line leaves cur_branch as None.
    }
    if let Some(path) = cur_path.take() {
        res.push(WtEntry {
            path,
            branch: cur_branch.take(),
        });
    }
    res
}

/// Find the worktree where a full baseline ref is currently checked out.
fn find_base_checkout(any_path: &str, base_full: &str) -> Option<String> {
    list_worktrees(any_path)
        .into_iter()
        .find(|e| e.branch.as_deref() == Some(base_full))
        .map(|e| e.path)
}

/// Resolved landing target.
struct LandResolve {
    base_ref: String,
    base_branch: String,
    base_dir: Option<String>,
    local_available: bool,
    reason: String,
}

/// Resolve a worktree's landing baseline and local-merge availability. A recorded local ref requires
/// its checked-out worktree; remote refs/raw SHAs cannot merge locally. Legacy entries use the main
/// worktree's current branch.
fn resolve_land(base_ref: Option<&str>, wt_path: &str) -> LandResolve {
    let explicit = base_ref.map(str::trim).filter(|s| !s.is_empty());
    if let Some(full) = explicit {
        let base_branch = short_ref(full);
        if full.starts_with("refs/heads/") {
            let dir = find_base_checkout(wt_path, full);
            let (available, reason) = match &dir {
                Some(_) => (true, "ok"),
                None => (false, "base_not_checked_out"),
            };
            return LandResolve {
                base_ref: full.to_string(),
                base_branch,
                base_dir: dir,
                local_available: available,
                reason: reason.to_string(),
            };
        }
        // Remote refs/raw SHAs have no local checked-out merge target; leave them for PR providers.
        let reason = if full.starts_with("refs/remotes/") {
            "base_remote"
        } else {
            "base_not_checked_out"
        };
        return LandResolve {
            base_ref: full.to_string(),
            base_branch,
            base_dir: None,
            local_available: false,
            reason: reason.to_string(),
        };
    }
    // Legacy worktrees without baseRef fall back to the main worktree's current branch.
    match list_worktrees(wt_path).into_iter().next() {
        Some(WtEntry {
            path,
            branch: Some(full),
        }) if full.starts_with("refs/heads/") => LandResolve {
            base_branch: short_ref(&full),
            base_ref: full,
            base_dir: Some(path),
            local_available: true,
            reason: "ok".to_string(),
        },
        _ => LandResolve {
            base_ref: String::new(),
            base_branch: String::new(),
            base_dir: None,
            local_available: false,
            reason: "no_base".to_string(),
        },
    }
}

/// Landing diff from the baseline fork point, including committed, uncommitted, and untracked work.
/// It follows `diff_stat_for_merge` but runs entirely inside the worktree against an explicit baseline.
fn land_diff_stat(wt_path: &str, base: &str, branch: &str) -> String {
    let committed =
        || run_git(wt_path, &["diff", "--stat", &format!("{base}...{branch}")]).unwrap_or_default();
    let mergebase = match run_git(wt_path, &["merge-base", base, "HEAD"]) {
        Some(b) if !b.is_empty() => b,
        _ => return committed(),
    };
    let index_path = std::env::temp_dir().join(format!("vlx-land-index-{}", Uuid::new_v4()));
    let run_with_index = |args: &[&str]| -> Option<String> {
        let out = crate::host::command("git")
            .arg("-C")
            .arg(wt_path)
            .env("GIT_INDEX_FILE", &index_path)
            .args(args)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let stat = (|| {
        run_with_index(&["read-tree", &mergebase])?;
        run_with_index(&["add", "-A"])?;
        run_with_index(&["diff", "--cached", "--stat", &mergebase])
    })();
    let _ = std::fs::remove_file(&index_path);
    stat.unwrap_or_else(committed)
}

/// Run landing preflight; return Err for non-Git worktrees or unresolved branches.
pub fn land_targets(base_ref: Option<&str>, wt_path: &str) -> Result<LandTargets, String> {
    if wt_path.trim().is_empty() {
        return Err("Session has no worktree to land".into());
    }
    let branch = worktree_branch(wt_path)
        .ok_or_else(|| format!("Cannot resolve worktree branch: {wt_path}"))?;
    let r = resolve_land(base_ref, wt_path);
    let diff_stat = if r.base_ref.is_empty() {
        String::new()
    } else {
        land_diff_stat(wt_path, &r.base_ref, &branch)
    };
    let base_dir_dirty = r
        .base_dir
        .as_deref()
        .map(worktree_has_changes)
        .unwrap_or(false);
    let providers = vec![LandProvider {
        id: "local".to_string(),
        available: r.local_available,
        reason: r.reason.clone(),
    }];
    Ok(LandTargets {
        base_ref: r.base_ref,
        base_branch: r.base_branch,
        branch,
        diff_stat,
        has_uncommitted: worktree_has_changes(wt_path),
        base_dir: r.base_dir,
        base_dir_dirty,
        providers,
    })
}

// ─────────────────────────── Unified branch merge ───────────────────────────
// The user selects source and target in either direction; merging no longer relies on baseRef,
// although creation still records it. Git merges only within a worktree where the target is checked
// out, so local merge remains unavailable otherwise and never switches branches implicitly.

/// Local branch with short name and its checked-out worktree directory, if any.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchEntry {
    pub name: String,
    pub checkout_dir: Option<String>,
}

/// Branch inventory for the merge dialog: repository status, cwd branch, and all local branches.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchList {
    pub is_repo: bool,
    /// Short cwd branch, or None when detached/not a repository.
    pub current: Option<String>,
    pub branches: Vec<BranchEntry>,
}

/// List all local branches and checkout locations; non-repositories return an empty non-error result.
pub fn branch_list(cwd: &str) -> Result<BranchList, String> {
    if cwd.trim().is_empty() {
        return Err("No working directory".into());
    }
    let is_repo = run_git(cwd, &["rev-parse", "--is-inside-work-tree"])
        .map(|s| s == "true")
        .unwrap_or(false);
    if !is_repo {
        return Ok(BranchList {
            is_repo: false,
            current: None,
            branches: vec![],
        });
    }
    let current = run_git(cwd, &["branch", "--show-current"]).filter(|s| !s.is_empty());
    // Map every full local branch ref to its checked-out worktree directory.
    let checkouts = list_worktrees(cwd);
    let refs =
        run_git(cwd, &["for-each-ref", "refs/heads", "--format=%(refname)"]).unwrap_or_default();
    let branches = refs
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|full| BranchEntry {
            name: short_ref(full),
            checkout_dir: checkouts
                .iter()
                .find(|e| e.branch.as_deref() == Some(full))
                .map(|e| e.path.clone()),
        })
        .collect();
    Ok(BranchList {
        is_repo: true,
        current,
        branches,
    })
}

/// Test whether one commit is already an ancestor of another.
fn is_ancestor(dir: &str, ancestor: &str, descendant: &str) -> bool {
    crate::host::command("git")
        .arg("-C")
        .arg(dir)
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Unified merge preflight: direction, diff, both dirty states, and availability.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeBranchesPreview {
    pub source: String,
    pub target: String,
    /// Summary of committed and uncommitted source content that would enter the target.
    pub diff_stat: String,
    /// Source branch worktree, if checked out.
    pub source_dir: Option<String>,
    /// Whether source changes require a commit and frontend commit message before merging.
    pub source_dirty: bool,
    /// Target worktree where merge executes; None makes local merge unavailable.
    pub target_dir: Option<String>,
    /// Whether the target is dirty; warn that it may block merge without preemptively rejecting.
    pub target_dirty: bool,
    /// Whether the clean source is already fully merged into the target.
    pub up_to_date: bool,
    /// Whether local merge is available because the target is checked out.
    pub available: bool,
    /// Stable reason code: ok, same_branch, branch_not_found, or target_not_checked_out.
    pub reason: String,
}

/// Resolve source/target short names to full refs and checkout directories.
struct MergeResolve {
    source_full: String,
    target_full: String,
    source_dir: Option<String>,
    target_dir: Option<String>,
    /// Whether both branches still exist after potentially stale UI selection.
    found: bool,
}

fn resolve_merge(cwd: &str, source: &str, target: &str) -> MergeResolve {
    let checkouts = list_worktrees(cwd);
    let find_dir = |full: &str| {
        checkouts
            .iter()
            .find(|e| e.branch.as_deref() == Some(full))
            .map(|e| e.path.clone())
    };
    let exists = |full: &str| run_git(cwd, &["rev-parse", "--verify", "--quiet", full]).is_some();
    let source_full = format!("refs/heads/{source}");
    let target_full = format!("refs/heads/{target}");
    let found = exists(&source_full) && exists(&target_full);
    MergeResolve {
        source_dir: find_dir(&source_full),
        target_dir: find_dir(&target_full),
        source_full,
        target_full,
        found,
    }
}

/// Unified merge preflight returns available=false plus a reason for identical/missing branches or an
/// unchecked-out target, allowing the frontend to disable the action without treating it as an error.
pub fn merge_branches_preview(
    cwd: &str,
    source: &str,
    target: &str,
) -> Result<MergeBranchesPreview, String> {
    if cwd.trim().is_empty() {
        return Err("No working directory".into());
    }
    let empty = |reason: &str| MergeBranchesPreview {
        source: source.to_string(),
        target: target.to_string(),
        diff_stat: String::new(),
        source_dir: None,
        source_dirty: false,
        target_dir: None,
        target_dirty: false,
        up_to_date: false,
        available: false,
        reason: reason.to_string(),
    };
    if source == target {
        return Ok(empty("same_branch"));
    }
    let r = resolve_merge(cwd, source, target);
    if !r.found {
        return Ok(empty("branch_not_found"));
    }
    let source_dirty = r
        .source_dir
        .as_deref()
        .map(worktree_has_changes)
        .unwrap_or(false);
    let target_dirty = r
        .target_dir
        .as_deref()
        .map(worktree_has_changes)
        .unwrap_or(false);
    // Up to date means all source commits are merged and no source worktree changes remain.
    let up_to_date = !source_dirty && is_ancestor(cwd, &r.source_full, &r.target_full);
    // For a dirty checked-out source, use the temporary-index algorithm to include uncommitted and
    // untracked files; otherwise summarize committed changes with a three-dot diff.
    let diff_stat = if up_to_date {
        String::new()
    } else if source_dirty {
        land_diff_stat(
            r.source_dir.as_deref().unwrap_or(cwd),
            &r.target_full,
            &r.source_full,
        )
    } else {
        run_git(
            cwd,
            &[
                "diff",
                "--stat",
                &format!("{}...{}", r.target_full, r.source_full),
            ],
        )
        .unwrap_or_default()
    };
    let (available, reason) = if r.target_dir.is_some() {
        (true, "ok")
    } else {
        (false, "target_not_checked_out")
    };
    Ok(MergeBranchesPreview {
        source: source.to_string(),
        target: target.to_string(),
        diff_stat,
        source_dir: r.source_dir,
        source_dirty,
        target_dir: r.target_dir,
        target_dirty,
        up_to_date,
        available,
        reason: reason.to_string(),
    })
}

/// Apply a unified merge: commit a dirty source with `commit_message`, then run no-ff/no-edit merge
/// in the target worktree. Preserve conflicts and list files for manual resolution. Missing branches
/// or an unchecked-out target return Err.
pub fn merge_branches_apply(
    cwd: &str,
    source: &str,
    target: &str,
    commit_message: Option<&str>,
) -> Result<MergeOutcome, String> {
    if cwd.trim().is_empty() {
        return Err("No working directory".into());
    }
    if source == target {
        return Err("Source and target are the same branch.".into());
    }
    let r = resolve_merge(cwd, source, target);
    if !r.found {
        return Err(format!("Branch '{source}' or '{target}' no longer exists."));
    }
    let target_dir = r.target_dir.ok_or_else(|| {
        format!(
            "Target branch '{target}' isn't checked out in any worktree. Check it out first, then retry."
        )
    })?;
    // Commit dirty source worktree changes first so the merge can include them.
    if let Some(src_dir) = r.source_dir.as_deref() {
        if worktree_has_changes(src_dir) {
            let msg = commit_message
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("vlx-term: commit before merge");
            commit_all(src_dir, msg)?;
        }
    }
    let out = crate::host::command("git")
        .arg("-C")
        .arg(&target_dir)
        .args(["merge", "--no-ff", "--no-edit", source])
        .output()
        .map_err(|e| format!("Failed to run git merge: {e}"))?;
    if out.status.success() {
        return Ok(MergeOutcome {
            merged: true,
            conflict: false,
            conflicts: vec![],
            message: format!("Merged {source} into {target}."),
        });
    }
    // Distinguish preserved merge conflicts from other failures.
    let conflicts: Vec<String> = run_git(&target_dir, &["diff", "--name-only", "--diff-filter=U"])
        .map(|s| s.lines().map(|l| l.to_string()).collect())
        .unwrap_or_default();
    if !conflicts.is_empty() {
        return Ok(MergeOutcome {
            merged: false,
            conflict: true,
            conflicts,
            message: "Merge has conflicts. Resolve them in the target branch's worktree terminal, then commit."
                .into(),
        });
    }
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    Err(format!("Merge failed: {err}"))
}

// ─────────────────────────── Change viewer (diff) ───────────────────────────
// Show staged, unstaged, and untracked worktree changes against HEAD. The frontend lists files and
// renders HEAD versus worktree content in CodeMirror MergeView.

/// Changed-file entry relative to HEAD.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    /// Repository-relative path using forward slashes.
    pub path: String,
    /// Status: modified, added, deleted, untracked, or renamed.
    pub status: String,
    /// Added lines from numstat; zero for binary/unavailable values.
    pub additions: u32,
    /// Deleted lines.
    pub deletions: u32,
    /// Whether numstat marks the file as binary.
    pub binary: bool,
    /// Index-side state from porcelain column X: empty, modified, added, deleted, or renamed.
    /// A non-empty value means the file has something staged.
    pub index: String,
    /// Worktree-side state from porcelain column Y: empty, modified, added, deleted, renamed, or
    /// untracked. A non-empty value means the file has something not yet staged.
    pub worktree: String,
    /// Staged line counts against HEAD (`git diff --numstat --cached`); zero when nothing is staged.
    pub staged_additions: u32,
    pub staged_deletions: u32,
    pub staged_binary: bool,
    /// Unstaged line counts against the index (`git diff --numstat`); zero when the worktree matches.
    pub unstaged_additions: u32,
    pub unstaged_deletions: u32,
    pub unstaged_binary: bool,
}

/// Both sides of a single-file diff: HEAD versus worktree.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    /// HEAD content, empty for added/untracked files.
    pub original: String,
    /// Worktree content, empty for deleted files.
    pub modified: String,
    /// Binary/oversized marker; both sides remain empty for frontend messaging.
    pub binary: bool,
}

/// Commit record shown in the group's recent-commits panel.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitInfo {
    /// Short hash (%h).
    pub hash: String,
    /// First-line subject (%s).
    pub subject: String,
    /// Author name (%an).
    pub author: String,
    /// Relative time (%cr), such as "2 days ago".
    pub relative: String,
}

/// Return up to `limit` recent commits; non-repositories/empty histories return an empty vector.
pub fn recent_commits(cwd: &str, limit: usize) -> Vec<CommitInfo> {
    log_page(cwd, limit.clamp(1, 50), 0)
}

/// Resolve repository root, falling back to the supplied directory.
fn repo_top(cwd: &str) -> String {
    run_git(cwd, &["rev-parse", "--show-toplevel"])
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| cwd.to_string())
}

/// Map one porcelain column to a display status. Column X is the index side and column Y the
/// worktree side; a space (or the `?` of an untracked pair, handled by the caller) means "nothing
/// on this side". Keeping the sides apart is what lets the Git panel show Staged and Changes as
/// separate groups, and one file in both at once when it is partly staged.
fn porcelain_side(c: char) -> &'static str {
    match c {
        'M' | 'T' => "modified",
        'A' => "added",
        'D' => "deleted",
        'R' => "renamed",
        'C' => "added",
        'U' => "modified", // unmerged: treat as a content conflict the user still has to resolve
        _ => "",
    }
}

/// Collect a `git diff --numstat` map of path -> (additions, deletions, binary).
/// Binary files report `-` for both counts.
fn numstat_map(cwd: &str, args: &[&str]) -> std::collections::HashMap<String, (u32, u32, bool)> {
    let mut map = std::collections::HashMap::new();
    let Some(out) = run_git(cwd, args) else {
        return map;
    };
    for line in out.lines() {
        let mut it = line.split('\t');
        let a = it.next().unwrap_or("");
        let d = it.next().unwrap_or("");
        let p = it.next().unwrap_or("").trim();
        if p.is_empty() {
            continue;
        }
        let binary = a == "-" || d == "-";
        map.insert(
            p.to_string(),
            (a.parse().unwrap_or(0), d.parse().unwrap_or(0), binary),
        );
    }
    map
}

/// Map porcelain XY codes to display status.
fn porcelain_status(xy: &str) -> &'static str {
    if xy == "??" {
        "untracked"
    } else if xy.contains('R') {
        "renamed"
    } else if xy.contains('D') {
        "deleted"
    } else if xy.contains('A') {
        "added"
    } else {
        "modified"
    }
}

/// List staged, unstaged, and untracked changes with per-file line counts; reject non-repositories.
pub fn changed_files(cwd: &str) -> Result<Vec<ChangedFile>, String> {
    if cwd.trim().is_empty() {
        return Err("No working directory".into());
    }
    let is_repo = run_git(cwd, &["rev-parse", "--is-inside-work-tree"])
        .map(|s| s == "true")
        .unwrap_or(false);
    if !is_repo {
        return Err(format!("Not a git repository: {cwd}"));
    }
    // Three numstat views: everything against HEAD (what the change viewer shows), the staged part
    // alone, and the not-yet-staged part alone. The Git panel needs the last two to label its
    // Staged and Changes groups with their own line counts.
    let stat = numstat_map(cwd, &["diff", "--numstat", "HEAD"]);
    let staged = numstat_map(cwd, &["diff", "--numstat", "--cached", "HEAD"]);
    let unstaged = numstat_map(cwd, &["diff", "--numstat"]);
    // Porcelain supplies XY in columns 0-1 and path from column 3, including untracked files.
    let porcelain = run_git_raw(cwd, &["status", "--porcelain"]).unwrap_or_default();
    let mut files: Vec<ChangedFile> = Vec::new();
    for line in porcelain.lines() {
        if line.len() < 4 {
            continue;
        }
        let xy = &line[..2];
        let rest = line[3..].trim();
        // For `old -> new` rename output, use the new path and remove Git's path quoting.
        let path = rest
            .rsplit(" -> ")
            .next()
            .unwrap_or(rest)
            .trim()
            .trim_matches('"')
            .to_string();
        if path.is_empty() {
            continue;
        }
        let status = porcelain_status(xy);
        let (additions, deletions, binary) = stat.get(&path).copied().unwrap_or((0, 0, false));
        let (sa, sd, sb) = staged.get(&path).copied().unwrap_or((0, 0, false));
        let (ua, ud, ub) = unstaged.get(&path).copied().unwrap_or((0, 0, false));
        // Untracked files report `??`, which is neither an index nor a worktree code: place them
        // entirely on the worktree side so the panel can list them under Untracked files.
        let mut chars = xy.chars();
        let (x, y) = (chars.next().unwrap_or(' '), chars.next().unwrap_or(' '));
        let (index_side, worktree_side) = if xy == "??" {
            (String::new(), "untracked".to_string())
        } else {
            (
                porcelain_side(x).to_string(),
                porcelain_side(y).to_string(),
            )
        };
        files.push(ChangedFile {
            path,
            status: status.to_string(),
            additions,
            deletions,
            binary,
            index: index_side,
            worktree: worktree_side,
            staged_additions: sa,
            staged_deletions: sd,
            staged_binary: sb,
            unstaged_additions: ua,
            unstaged_deletions: ud,
            unstaged_binary: ub,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// Treat content as binary when it contains NUL or exceeds 2 MB.
fn is_binary_bytes(bytes: &[u8]) -> bool {
    bytes.len() > 2 * 1024 * 1024 || bytes.contains(&0)
}

/// Read raw bytes with `git show <spec>`; avoid run_git's trimming/lossy conversion.
fn git_show_bytes(cwd: &str, spec: &str) -> Option<Vec<u8>> {
    let out = crate::host::command("git")
        .arg("-C")
        .arg(cwd)
        .args(["show", spec])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(out.stdout)
}

/// The filesystem path `file_diff` reads for the worktree side: `repo_top(cwd)` joined with `path`.
/// Callers that enforce access control (the remote data-dir ACL in web dispatch) must gate THIS path,
/// not the raw arguments: `Path::join` replaces the base entirely for an absolute `path`, and
/// `repo_top` falls back to `cwd` outside a repository, so the effective target is caller-chosen.
pub fn file_diff_worktree_path(cwd: &str, path: &str) -> std::path::PathBuf {
    std::path::Path::new(&repo_top(cwd)).join(path)
}

/// Return HEAD/worktree text for one file. Added/untracked has no original; deleted has no modified.
/// If either side is binary/oversized, set binary and leave both strings empty.
pub fn file_diff(cwd: &str, path: &str) -> Result<FileDiff, String> {
    if cwd.trim().is_empty() || path.trim().is_empty() {
        return Err("Missing working directory or path".into());
    }
    file_diff_at(cwd, path, &file_diff_worktree_path(cwd, path))
}

/// [`file_diff`] with a caller-supplied effective worktree path. Callers that enforce access control
/// (the remote data-dir ACL in web dispatch) compute the path ONCE via [`file_diff_worktree_path`],
/// gate it, and pass the SAME value here — so the checked path and the read path cannot diverge, and
/// the `repo_top` git subprocess runs once instead of twice.
pub fn file_diff_at(cwd: &str, path: &str, work_path: &std::path::Path) -> Result<FileDiff, String> {
    if cwd.trim().is_empty() || path.trim().is_empty() {
        return Err("Missing working directory or path".into());
    }
    // HEAD side via `git show HEAD:<path>`; added/untracked files have no object.
    let original_bytes = git_show_bytes(cwd, &format!("HEAD:{path}"));
    // Worktree side reads the caller-resolved repository file; deleted files are absent.
    let modified_bytes = std::fs::read(work_path).ok();

    let orig_bin = original_bytes
        .as_deref()
        .map(is_binary_bytes)
        .unwrap_or(false);
    let mod_bin = modified_bytes
        .as_deref()
        .map(is_binary_bytes)
        .unwrap_or(false);
    if orig_bin || mod_bin {
        return Ok(FileDiff {
            path: path.to_string(),
            original: String::new(),
            modified: String::new(),
            binary: true,
        });
    }
    Ok(FileDiff {
        path: path.to_string(),
        original: original_bytes
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default(),
        modified: modified_bytes
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default(),
        binary: false,
    })
}

// ─────────────────────────── Git panel actions ───────────────────────────
// Staging, committing, discarding, and history browsing for the right panel's Git tab. Everything
// here goes through the git binary; the one exception is deleting an untracked file, which no git
// command does and which therefore carries its own containment check.

/// Upper bound on paths handed to a single git invocation. A repository with thousands of changes
/// would otherwise exceed the platform's command-line length limit, so callers batch instead.
const MAX_PATHS_PER_CALL: usize = 200;

/// Reject a repository-relative path that could escape the repository or be read as an option.
/// Paths reaching these commands come from a client, not from our own listing, so treat them as
/// untrusted: absolute paths, `..` components, and empty strings are refused outright. Everything
/// that passes is still handed to git after a `--` separator.
fn check_rel_path(path: &str) -> Result<(), String> {
    let p = path.trim();
    if p.is_empty() {
        return Err("Empty path".into());
    }
    let candidate = std::path::Path::new(p);
    if candidate.is_absolute() || p.starts_with('/') || p.starts_with('\\') {
        return Err(format!("Path must be repository-relative: {p}"));
    }
    // Windows drive-letter forms such as `C:file` are relative to a drive, not to the repository.
    if p.chars().nth(1) == Some(':') {
        return Err(format!("Path must be repository-relative: {p}"));
    }
    for part in candidate.components() {
        if matches!(part, std::path::Component::ParentDir) {
            return Err(format!("Path escapes the repository: {p}"));
        }
    }
    Ok(())
}

/// Validate a non-empty batch of repository-relative paths.
fn check_rel_paths(paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("No paths given".into());
    }
    for p in paths {
        check_rel_path(p)?;
    }
    Ok(())
}

/// Confirm `cwd` is inside a work tree before any mutation runs.
fn require_repo(cwd: &str) -> Result<(), String> {
    if cwd.trim().is_empty() {
        return Err("No working directory".into());
    }
    let is_repo = run_git(cwd, &["rev-parse", "--is-inside-work-tree"])
        .map(|s| s == "true")
        .unwrap_or(false);
    if is_repo {
        Ok(())
    } else {
        Err(format!("Not a git repository: {cwd}"))
    }
}

/// Whether the repository has a commit yet. A freshly initialized repository has no HEAD, which
/// changes how unstaging and history have to work.
fn has_head(cwd: &str) -> bool {
    crate::host::command("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--verify", "--quiet", "HEAD"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run git and return its trimmed stderr as the error when it fails. The message goes straight to
/// the UI, so it stays in English like every other backend string.
fn run_git_checked(cwd: &str, args: &[&str]) -> Result<String, String> {
    let out = crate::host::command("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run git: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let err = if err.is_empty() {
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        } else {
            err
        };
        return Err(if err.is_empty() {
            format!("git {} failed", args.first().copied().unwrap_or("command"))
        } else {
            err
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Run `git <prefix> -- <paths>` in batches so long file lists cannot overflow the command line.
fn run_git_on_paths(cwd: &str, prefix: &[&str], paths: &[String]) -> Result<(), String> {
    for chunk in paths.chunks(MAX_PATHS_PER_CALL) {
        let mut args: Vec<&str> = prefix.to_vec();
        args.push("--");
        for p in chunk {
            args.push(p.as_str());
        }
        run_git_checked(cwd, &args)?;
    }
    Ok(())
}

/// Stage the given paths, including untracked files and deletions.
///
/// Runs from the repository root: `changed_files` reports root-relative paths, while git resolves a
/// pathspec against the directory it runs in. A session opened in a subdirectory would otherwise make
/// git look for `sub/a.txt` inside `sub/`, and `add` fails with "pathspec did not match any files".
pub fn stage(cwd: &str, paths: &[String]) -> Result<(), String> {
    require_repo(cwd)?;
    check_rel_paths(paths)?;
    run_git_on_paths(&repo_top(cwd), &["add", "-A"], paths)
}

/// Unstage the given paths, leaving the worktree untouched. Before the first commit there is no
/// HEAD to restore from, so drop the entries from the index instead.
///
/// Runs from the repository root for the same reason as `stage`, and here the mismatch is silent:
/// `reset` matching nothing still exits 0, so the panel would report success and leave the file staged.
pub fn unstage(cwd: &str, paths: &[String]) -> Result<(), String> {
    require_repo(cwd)?;
    check_rel_paths(paths)?;
    let root = repo_top(cwd);
    if has_head(&root) {
        run_git_on_paths(&root, &["reset", "-q", "HEAD"], paths)
    } else {
        run_git_on_paths(
            &root,
            &["rm", "--cached", "-r", "-q", "--ignore-unmatch"],
            paths,
        )
    }
}

/// Which of `paths` git currently tracks. Untracked files need deleting rather than restoring.
///
/// Callers pass the repository root: both the pathspec and, thanks to `--full-name`, the output are
/// then root-relative, matching the paths `changed_files` produced.
fn tracked_paths(cwd: &str, paths: &[String]) -> std::collections::HashSet<String> {
    let mut tracked = std::collections::HashSet::new();
    for chunk in paths.chunks(MAX_PATHS_PER_CALL) {
        let mut args: Vec<&str> = vec!["ls-files", "--full-name", "--"];
        for p in chunk {
            args.push(p.as_str());
        }
        if let Some(out) = run_git(cwd, &args) {
            for line in out.lines() {
                let name = line.trim().trim_matches('"');
                if !name.is_empty() {
                    tracked.insert(name.to_string());
                }
            }
        }
    }
    tracked
}

/// Delete one untracked entry, refusing anything that does not resolve inside the repository.
/// This is the only place the Git panel writes to disk without git mediating, so the containment
/// check is the safeguard: resolve the repository root and the target's parent directory to their
/// real locations and require the second to sit under the first. Resolving the parent rather than
/// the target itself means a symlinked file is removed as a link, never followed.
fn delete_untracked(repo_root: &std::path::Path, rel: &str) -> Result<(), String> {
    let target = repo_root.join(rel);
    let parent = target
        .parent()
        .ok_or_else(|| format!("Cannot resolve parent of {rel}"))?;
    let root_real = repo_root
        .canonicalize()
        .map_err(|e| format!("Cannot resolve repository root: {e}"))?;
    let parent_real = parent
        .canonicalize()
        .map_err(|e| format!("Cannot resolve {rel}: {e}"))?;
    if !parent_real.starts_with(&root_real) {
        return Err(format!("Path escapes the repository: {rel}"));
    }
    let name = target
        .file_name()
        .ok_or_else(|| format!("Cannot resolve name of {rel}"))?;
    let real_target = parent_real.join(name);
    // symlink_metadata does not follow links: a symlinked directory is unlinked, not walked into.
    let meta = match std::fs::symlink_metadata(&real_target) {
        Ok(m) => m,
        // Already gone is the outcome the caller wanted.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("Cannot inspect {rel}: {e}")),
    };
    let result = if meta.is_dir() {
        std::fs::remove_dir_all(&real_target)
    } else {
        std::fs::remove_file(&real_target)
    };
    result.map_err(|e| format!("Cannot delete {rel}: {e}"))
}

/// Discard worktree changes for the given paths: tracked files are restored from the index, and
/// untracked files are deleted. Staged content is deliberately left alone — unstage first to get
/// rid of that — so a single discard can never destroy work the user has already staged.
///
/// Everything here runs from the repository root. The paths come from `changed_files` and are
/// root-relative; resolving them against a subdirectory session would make `ls-files` match nothing,
/// which this function reads as "untracked" and answers by deleting the file from disk — the one
/// outcome a discard must never produce for a tracked file.
pub fn discard(cwd: &str, paths: &[String]) -> Result<(), String> {
    require_repo(cwd)?;
    check_rel_paths(paths)?;
    let root = repo_top(cwd);
    let tracked = tracked_paths(&root, paths);
    let (restore, remove): (Vec<String>, Vec<String>) = paths
        .iter()
        .cloned()
        .partition(|p| tracked.contains(p.as_str()));
    if !restore.is_empty() {
        run_git_on_paths(&root, &["checkout", "-q"], &restore)?;
    }
    if !remove.is_empty() {
        let root_path = std::path::PathBuf::from(&root);
        for rel in &remove {
            delete_untracked(&root_path, rel)?;
        }
    }
    Ok(())
}

/// Commit what is staged and return the new commit. `amend` rewrites the previous commit instead,
/// which git refuses before the first commit.
pub fn commit(cwd: &str, message: &str, amend: bool) -> Result<CommitInfo, String> {
    require_repo(cwd)?;
    let msg = message.trim();
    if msg.is_empty() {
        return Err("Commit message is empty".into());
    }
    if amend && !has_head(cwd) {
        return Err("Nothing to amend: this repository has no commits yet".into());
    }
    let mut args: Vec<&str> = vec!["commit", "-q"];
    if amend {
        args.push("--amend");
    }
    args.push("-m");
    args.push(msg);
    run_git_checked(cwd, &args)?;
    log_page(cwd, 1, 0)
        .into_iter()
        .next()
        .ok_or_else(|| "Commit succeeded but could not be read back".to_string())
}

/// Total commits reachable from HEAD; zero outside a repository or before the first commit.
pub fn commit_count(cwd: &str) -> u32 {
    run_git(cwd, &["rev-list", "--count", "HEAD"])
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// Field separator for commit pretty-printing: a one-line subject cannot contain a unit separator.
const COMMIT_PRETTY: &str = "--pretty=format:%h\x1f%s\x1f%an\x1f%cr";

/// Parse the unit-separated commit lines produced by [`COMMIT_PRETTY`].
fn parse_commit_lines(out: &str) -> Vec<CommitInfo> {
    out.lines()
        .filter_map(|line| {
            let mut f = line.split('\x1f');
            let hash = f.next()?.to_string();
            if hash.is_empty() {
                return None;
            }
            Some(CommitInfo {
                subject: f.next().unwrap_or("").to_string(),
                author: f.next().unwrap_or("").to_string(),
                relative: f.next().unwrap_or("").to_string(),
                hash,
            })
        })
        .collect()
}

/// One page of history, newest first. `offset` skips that many commits, so the Git panel can keep
/// loading older pages; non-repositories and empty histories return an empty vector.
pub fn log_page(cwd: &str, limit: usize, offset: usize) -> Vec<CommitInfo> {
    if run_git(cwd, &["rev-parse", "--is-inside-work-tree"]).as_deref() != Some("true") {
        return Vec::new();
    }
    let n = limit.clamp(1, 200).to_string();
    let skip = format!("--skip={}", offset.min(1_000_000));
    match run_git(cwd, &["log", "-n", &n, &skip, COMMIT_PRETTY]) {
        Some(s) if !s.is_empty() => parse_commit_lines(&s),
        _ => Vec::new(),
    }
}

/// Reject anything that is not a plain object name. History arguments come from a client, and git
/// accepts a whole revision language plus `--option`-shaped arguments in the same position, so the
/// only safe rule is to allow short and full hexadecimal hashes and nothing else.
fn check_commit_hash(hash: &str) -> Result<(), String> {
    let h = hash.trim();
    let ok = (4..=40).contains(&h.len()) && h.chars().all(|c| c.is_ascii_hexdigit());
    if ok {
        Ok(())
    } else {
        Err(format!("Not a commit hash: {hash}"))
    }
}

/// Files changed by one commit, with that commit's own line counts. Merge commits produce no
/// diff against a single parent and therefore return an empty list.
pub fn commit_files(cwd: &str, hash: &str) -> Result<Vec<ChangedFile>, String> {
    require_repo(cwd)?;
    check_commit_hash(hash)?;
    let stat = numstat_map(cwd, &["diff-tree", "--no-commit-id", "--numstat", "-r", hash]);
    let names = run_git(
        cwd,
        &["diff-tree", "--no-commit-id", "--name-status", "-r", hash],
    )
    .unwrap_or_default();
    let mut files: Vec<ChangedFile> = Vec::new();
    for line in names.lines() {
        let mut it = line.split('\t');
        let code = it.next().unwrap_or("").trim();
        let first = it.next().unwrap_or("").trim();
        // Rename and copy rows carry a similarity score and both paths; the new path is what the
        // commit produced, so report that one.
        let path = it.next().unwrap_or(first).trim().trim_matches('"').to_string();
        if path.is_empty() {
            continue;
        }
        let status = porcelain_side(code.chars().next().unwrap_or('M'));
        let status = if status.is_empty() { "modified" } else { status };
        let (additions, deletions, binary) = stat.get(&path).copied().unwrap_or((0, 0, false));
        files.push(ChangedFile {
            path,
            status: status.to_string(),
            additions,
            deletions,
            binary,
            index: status.to_string(),
            worktree: String::new(),
            staged_additions: additions,
            staged_deletions: deletions,
            staged_binary: binary,
            unstaged_additions: 0,
            unstaged_deletions: 0,
            unstaged_binary: false,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// Both sides of one file's diff inside a commit: the parent's content versus the commit's.
/// A root commit has no parent, so the original side is empty — the same shape an added file has.
pub fn commit_file_diff(cwd: &str, hash: &str, path: &str) -> Result<FileDiff, String> {
    require_repo(cwd)?;
    check_commit_hash(hash)?;
    check_rel_path(path)?;
    let original_bytes = git_show_bytes(cwd, &format!("{hash}^:{path}"));
    let modified_bytes = git_show_bytes(cwd, &format!("{hash}:{path}"));
    let binary = original_bytes.as_deref().map(is_binary_bytes).unwrap_or(false)
        || modified_bytes.as_deref().map(is_binary_bytes).unwrap_or(false);
    if binary {
        return Ok(FileDiff {
            path: path.to_string(),
            original: String::new(),
            modified: String::new(),
            binary: true,
        });
    }
    Ok(FileDiff {
        path: path.to_string(),
        original: original_bytes
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default(),
        modified: modified_bytes
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default(),
        binary: false,
    })
}

#[cfg(test)]
mod merge_tests {
    use super::*;
    use std::path::PathBuf;

    /// Run a Git command in a directory and assert success.
    fn git(dir: &std::path::Path, args: &[&str]) {
        let out = crate::host::command("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Create a temporary main-branch repository with committed a.txt.
    fn init_repo() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vlx-merge-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q"]);
        git(&dir, &["config", "user.email", "t@example.com"]);
        git(&dir, &["config", "user.name", "tester"]);
        git(&dir, &["config", "commit.gpgsign", "false"]);
        std::fs::write(dir.join("a.txt"), "hello\n").unwrap();
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", "init"]);
        git(&dir, &["branch", "-M", "main"]);
        dir
    }

    /// file_diff_at with the precomputed effective path returns exactly what file_diff returns —
    /// the pass-through contract the web dispatch remote lane relies on after gating that path.
    #[test]
    fn file_diff_at_matches_file_diff_on_the_precomputed_path() {
        let dir = init_repo();
        std::fs::write(dir.join("a.txt"), "hello\nworld\n").unwrap();
        let cwd = dir.to_string_lossy().to_string();
        let work_path = file_diff_worktree_path(&cwd, "a.txt");
        let direct = file_diff(&cwd, "a.txt").unwrap();
        let via_path = file_diff_at(&cwd, "a.txt", &work_path).unwrap();
        assert_eq!(direct.original, via_path.original);
        assert_eq!(direct.modified, via_path.modified);
        assert_eq!(via_path.original, "hello\n");
        assert_eq!(via_path.modified, "hello\nworld\n");
        // Discrimination check: file_diff_at must read the CALLER-supplied work_path, not
        // recompute it internally — pass a decoy file and expect its content back. This is the
        // check/use property the remote ACL relies on (gate the path once, read the same path).
        std::fs::write(dir.join("b.txt"), "decoy\n").unwrap();
        let via_decoy = file_diff_at(&cwd, "a.txt", &dir.join("b.txt")).unwrap();
        assert_eq!(
            via_decoy.modified, "decoy\n",
            "file_diff_at must read the caller-provided worktree path"
        );
        // Empty arguments stay rejected on both entry points.
        assert!(file_diff_at("", "a.txt", &work_path).is_err());
        assert!(file_diff_at(&cwd, " ", &work_path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Sessions often sit in a subdirectory of the repository. `changed_files` reports paths relative to
    /// the repository root, so every write has to run from the root: resolved against the subdirectory,
    /// `stage` fails outright, `unstage` matches nothing while still exiting 0, and `discard` sees a
    /// tracked file as untracked and deletes it from disk instead of restoring it.
    #[test]
    fn writes_from_a_subdirectory_session_use_root_relative_paths() {
        let dir = init_repo();
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/nested.txt"), "one\n").unwrap();
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", "nested"]);
        std::fs::write(dir.join("sub/nested.txt"), "one\ntwo\n").unwrap();
        // The panel's own working directory: the session opened inside sub/.
        let cwd = dir.join("sub").to_string_lossy().to_string();
        let path = vec!["sub/nested.txt".to_string()];

        let listed = changed_files(&cwd).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].path, "sub/nested.txt", "paths are root-relative");

        stage(&cwd, &path).unwrap();
        let staged = changed_files(&cwd).unwrap();
        assert_eq!(staged[0].index, "modified", "stage must reach the index");

        unstage(&cwd, &path).unwrap();
        let unstaged = changed_files(&cwd).unwrap();
        assert_eq!(unstaged[0].index, "", "unstage must clear the index side");

        discard(&cwd, &path).unwrap();
        assert!(
            dir.join("sub/nested.txt").exists(),
            "discarding a tracked file must restore it, never delete it"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("sub/nested.txt")).unwrap(),
            "one\n",
            "discard must restore the committed content"
        );
        assert!(changed_files(&cwd).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Staging one file must move it to the index side alone, leaving a second changed file
    /// entirely on the worktree side. This is the split the Git panel's Staged and Changes groups
    /// are built on, so a regression here silently mislabels every row.
    #[test]
    fn changed_files_separates_index_from_worktree() {
        let dir = init_repo();
        let cwd = dir.to_string_lossy().to_string();
        std::fs::write(dir.join("a.txt"), "hello\nstaged\n").unwrap();
        std::fs::write(dir.join("new.txt"), "brand new\n").unwrap();
        stage(&cwd, &["a.txt".to_string()]).unwrap();

        let files = changed_files(&cwd).unwrap();
        let a = files.iter().find(|f| f.path == "a.txt").unwrap();
        assert_eq!(a.index, "modified", "staged edit belongs to the index side");
        assert_eq!(a.worktree, "", "nothing is left unstaged for a.txt");
        assert_eq!((a.staged_additions, a.staged_deletions), (1, 0));
        assert_eq!((a.unstaged_additions, a.unstaged_deletions), (0, 0));

        let new = files.iter().find(|f| f.path == "new.txt").unwrap();
        assert_eq!(new.index, "");
        assert_eq!(new.worktree, "untracked");

        // Editing the staged file again puts it on BOTH sides at once, which is what git means and
        // what the panel must show: part of a.txt is going into the next commit, part is not.
        std::fs::write(dir.join("a.txt"), "hello\nstaged\nunstaged\n").unwrap();
        let files = changed_files(&cwd).unwrap();
        let a = files.iter().find(|f| f.path == "a.txt").unwrap();
        assert_eq!(a.index, "modified");
        assert_eq!(a.worktree, "modified");
        assert_eq!(a.staged_additions, 1);
        assert_eq!(a.unstaged_additions, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Stage, unstage, and commit round-trip, including the untracked file that only `git add`
    /// picks up and the commit count the history section labels itself with.
    #[test]
    fn stage_unstage_and_commit_round_trip() {
        let dir = init_repo();
        let cwd = dir.to_string_lossy().to_string();
        std::fs::write(dir.join("new.txt"), "brand new\n").unwrap();
        assert_eq!(commit_count(&cwd), 1);

        stage(&cwd, &["new.txt".to_string()]).unwrap();
        let staged = changed_files(&cwd).unwrap();
        assert_eq!(staged.iter().find(|f| f.path == "new.txt").unwrap().index, "added");

        unstage(&cwd, &["new.txt".to_string()]).unwrap();
        let back = changed_files(&cwd).unwrap();
        let f = back.iter().find(|f| f.path == "new.txt").unwrap();
        assert_eq!(f.index, "", "unstaging returns the file to the worktree side");
        assert_eq!(f.worktree, "untracked");

        stage(&cwd, &["new.txt".to_string()]).unwrap();
        let made = commit(&cwd, "add new.txt", false).unwrap();
        assert_eq!(made.subject, "add new.txt");
        assert_eq!(commit_count(&cwd), 2);
        assert!(changed_files(&cwd).unwrap().is_empty(), "committing clears the working tree");

        // An empty message is refused before git runs, so no half-made commit can result.
        assert!(commit(&cwd, "   ", false).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Discard restores a tracked file from the index and deletes an untracked one — but never
    /// touches what is already staged, which is the property that keeps it from destroying work.
    #[test]
    fn discard_restores_tracked_and_deletes_untracked_but_spares_staged() {
        let dir = init_repo();
        let cwd = dir.to_string_lossy().to_string();
        std::fs::write(dir.join("staged.txt"), "keep me\n").unwrap();
        stage(&cwd, &["staged.txt".to_string()]).unwrap();
        std::fs::write(dir.join("a.txt"), "hello\nedited\n").unwrap();
        std::fs::write(dir.join("junk.txt"), "throwaway\n").unwrap();

        discard(
            &cwd,
            &["a.txt".to_string(), "junk.txt".to_string(), "staged.txt".to_string()],
        )
        .unwrap();

        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "hello\n");
        assert!(!dir.join("junk.txt").exists(), "untracked file is deleted");
        assert!(
            dir.join("staged.txt").exists(),
            "a staged file survives discard: its content lives in the index, not only on disk"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Path and hash arguments arrive from a client, so the validators are the boundary. Absolute
    /// paths, parent-directory escapes, and anything that is not a plain object name are refused
    /// before a git command or a filesystem delete is ever built.
    #[test]
    fn path_and_hash_validators_reject_hostile_input() {
        assert!(check_rel_path("src/main.rs").is_ok());
        assert!(check_rel_path("").is_err());
        assert!(check_rel_path("   ").is_err());
        assert!(check_rel_path("/etc/passwd").is_err());
        assert!(check_rel_path("../outside.txt").is_err());
        assert!(check_rel_path("src/../../outside.txt").is_err());
        assert!(check_rel_path("C:secret.txt").is_err());
        assert!(check_rel_paths(&[]).is_err(), "an empty batch must not mean everything");

        assert!(check_commit_hash("11ac20c").is_ok());
        assert!(check_commit_hash("11ac20cdeadbeef11ac20cdeadbeef11ac20cdea").is_ok());
        assert!(check_commit_hash("HEAD").is_err());
        assert!(check_commit_hash("HEAD~3").is_err());
        assert!(check_commit_hash("--upload-pack=evil").is_err());
        assert!(check_commit_hash("abc").is_err(), "too short to be an object name");
    }

    /// Deleting an untracked entry is the one raw filesystem write here, so its containment check
    /// gets its own test: a path resolving outside the repository is refused, and a symlink is
    /// unlinked rather than followed into its target.
    #[test]
    fn delete_untracked_stays_inside_the_repository() {
        let dir = init_repo();
        let outside = std::env::temp_dir().join(format!("vlx-outside-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&outside).unwrap();
        let treasure = outside.join("treasure.txt");
        std::fs::write(&treasure, "do not delete\n").unwrap();

        // A path that climbs out never reaches the filesystem: the validator rejects it first.
        assert!(discard(&dir.to_string_lossy(), &["../treasure.txt".to_string()]).is_err());

        // A symlink pointing outside is removed as a link; its target survives.
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&treasure, dir.join("link.txt")).unwrap();
            delete_untracked(&dir, "link.txt").unwrap();
            assert!(!dir.join("link.txt").exists(), "the link itself is gone");
            assert!(treasure.exists(), "the link target outside the repository is untouched");
        }

        // Deleting something that is already gone is the outcome the caller wanted, not an error.
        delete_untracked(&dir, "never-existed.txt").unwrap();

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
    }

    /// History paging and per-commit file listing: what the Git panel's Committed changes section
    /// reads. Paging must not repeat a commit, and a commit's file list carries its own counts.
    #[test]
    fn history_paging_and_commit_files() {
        let dir = init_repo();
        let cwd = dir.to_string_lossy().to_string();
        for i in 0..3 {
            std::fs::write(dir.join(format!("f{i}.txt")), format!("line {i}\n")).unwrap();
            stage(&cwd, &[format!("f{i}.txt")]).unwrap();
            commit(&cwd, &format!("commit {i}"), false).unwrap();
        }
        assert_eq!(commit_count(&cwd), 4);

        let first = log_page(&cwd, 2, 0);
        let second = log_page(&cwd, 2, 2);
        assert_eq!(first.len(), 2);
        assert_eq!(second.len(), 2);
        assert_eq!(first[0].subject, "commit 2", "history is newest first");
        assert!(
            first.iter().all(|c| second.iter().all(|o| o.hash != c.hash)),
            "an offset page must not repeat the previous page"
        );

        let files = commit_files(&cwd, &first[0].hash).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "f2.txt");
        assert_eq!(files[0].status, "added");
        assert_eq!(files[0].additions, 1);

        let diff = commit_file_diff(&cwd, &first[0].hash, "f2.txt").unwrap();
        assert_eq!(diff.original, "", "a file added by the commit has no parent side");
        assert_eq!(diff.modified, "line 2\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Derive default clone directory names from common repository URL forms.
    #[test]
    fn derive_clone_dir_name_variants() {
        let cases = [
            ("https://github.com/vlinx/vlx-term.git", "vlx-term"),
            ("https://github.com/vlinx/vlx-term", "vlx-term"),
            ("git@github.com:vlinx/vlx-term.git", "vlx-term"),
            ("ssh://git@host.org:2222/group/sub/repo.git", "repo"),
            ("https://host/group/repo/", "repo"),
            ("  https://host/a/b.git  ", "b"),
        ];
        for (url, want) in cases {
            assert_eq!(
                derive_clone_dir_name(url).as_deref(),
                Some(want),
                "url={url}"
            );
        }
        assert_eq!(derive_clone_dir_name(""), None);
        assert_eq!(derive_clone_dir_name("   "), None);
        assert_eq!(derive_clone_dir_name(".git"), None);
    }

    /// Normalize carriage-return Git progress into stable phases/percentages independent of host text.
    #[test]
    fn clone_progress_lines_are_parsed() {
        assert_eq!(
            parse_clone_progress("Cloning into 'repo'..."),
            Some(("connecting", None))
        );
        assert_eq!(
            parse_clone_progress("remote: Enumerating objects: 123, done."),
            Some(("preparing", None))
        );
        assert_eq!(
            parse_clone_progress("Receiving objects:  42% (42/100), 1.00 MiB | 1.00 MiB/s"),
            Some(("receiving", Some(42)))
        );
        assert_eq!(
            parse_clone_progress("Resolving deltas: 100% (12/12), done."),
            Some(("resolving", Some(100)))
        );
        assert_eq!(parse_clone_progress("fatal: network unavailable"), None);
    }

    /// A nonresponsive local HTTP endpoint simulates a hung network; cancellation must kill Git and clean up.
    #[test]
    fn clone_can_be_cancelled_without_leaving_target() {
        use std::net::TcpListener;
        use std::sync::mpsc;

        let listener = TcpListener::bind("127.0.0.1:0").expect("binding a loopback port should work");
        let addr = listener.local_addr().unwrap();
        let (accepted_tx, accepted_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (_socket, _) = listener.accept().expect("git should connect to the test HTTP endpoint");
            accepted_tx.send(()).ok();
            let _ = release_rx.recv_timeout(Duration::from_secs(10));
        });

        let parent = std::env::temp_dir().join(format!("vlx-clone-cancel-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&parent).unwrap();
        let parent_str = parent.to_string_lossy().to_string();
        let operation_id = Uuid::new_v4().to_string();
        let url = format!("http://{addr}/repo.git");
        let operation_for_thread = operation_id.clone();
        let (result_tx, result_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = clone_to_with_progress(
                &url,
                &parent_str,
                Some("cancelled-repo"),
                None,
                &operation_for_thread,
                "test",
                |_| {},
            );
            result_tx.send(result).ok();
        });

        accepted_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("git should reach the test endpoint before the timeout");
        assert!(
            cancel_clone(&operation_id, "other-client").is_err(),
            "another logged-in client must not be able to cancel this clone"
        );
        assert!(cancel_clone(&operation_id, "test").expect("the cancel request should be accepted"));
        release_tx.send(()).ok();
        let result = result_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the clone worker thread should end promptly after cancellation");
        assert_eq!(result.unwrap_err(), CLONE_CANCELLED_ERROR);
        assert!(!parent.join("cancelled-repo").exists());
        assert!(
            std::fs::read_dir(&parent).unwrap().next().is_none(),
            "no hidden temporary clone directory should be left behind after cancelling"
        );

        server.join().unwrap();
        std::fs::remove_dir_all(parent).unwrap();
    }

    /// Real clone from a local source into a temporary parent produces an existing repository.
    #[test]
    fn clone_to_local_repo() {
        let src = init_repo();
        let src_str = src.to_string_lossy().to_string();
        let parent = std::env::temp_dir().join(format!("vlx-clone-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&parent).unwrap();
        let parent_str = parent.to_string_lossy().to_string();

        // Clone the default branch under a custom directory name.
        let cloned = clone_to(&src_str, &parent_str, Some("myrepo"), None).expect("the clone should succeed");
        assert!(std::path::Path::new(&cloned).join("a.txt").is_file());
        assert!(std::path::Path::new(&cloned).join(".git").exists());

        // Existing targets must fail without overwrite.
        let again = clone_to(&src_str, &parent_str, Some("myrepo"), None);
        assert!(again.is_err(), "an existing destination should be an error");

        // Reject folder names containing path separators.
        assert!(clone_to(&src_str, &parent_str, Some("a/b"), None).is_err());

        // An existing explicit branch succeeds; a blank branch means default.
        assert!(
            clone_to(&src_str, &parent_str, Some("on-main"), Some("main")).is_ok(),
            "cloning the named branch main should succeed"
        );
        assert!(
            clone_to(&src_str, &parent_str, Some("blank-branch"), Some("   ")).is_ok(),
            "a blank branch should fall back to the default branch and succeed"
        );

        // A nonexistent branch must fail.
        assert!(
            clone_to(&src_str, &parent_str, Some("nope"), Some("no-such-branch")).is_err(),
            "cloning a branch that does not exist should be an error"
        );
    }

    /// Branch inventory includes main/worktree branches with correct checkout paths and cwd branch.
    #[test]
    fn branch_list_maps_checkouts() {
        let repo = init_repo();
        let repo_str = repo.to_string_lossy().to_string();

        let wt = worktree_add(&repo_str, "list x").expect("creating the worktree should succeed");
        let short = wt.branch.clone();

        let list = branch_list(&repo_str).expect("listing branches should succeed");
        assert!(list.is_repo);
        assert_eq!(list.current.as_deref(), Some("main"));
        let main = list.branches.iter().find(|b| b.name == "main").unwrap();
        assert!(main.checkout_dir.is_some(), "main should have a checkout directory");
        let feat = list.branches.iter().find(|b| b.name == short).unwrap();
        // Compare canonical paths to account for macOS /var -> /private/var.
        let want = std::fs::canonicalize(&wt.path).unwrap();
        let got = std::fs::canonicalize(feat.checkout_dir.as_deref().unwrap()).unwrap();
        assert_eq!(got, want, "a worktree branch's checkout directory is the worktree path");

        // A non-repository returns is_repo=false and an empty list without error.
        let plain = std::env::temp_dir().join(format!("vlx-plain-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&plain).unwrap();
        let none = branch_list(&plain.to_string_lossy()).expect("a non-repository should still return normally");
        assert!(!none.is_repo && none.branches.is_empty());

        let _ = worktree_remove(&wt.path, true);
        let _ = branch_delete(&repo_str, &wt.branch);
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_dir_all(&plain);
    }

    /// Forward unified merge (old “merge into parent”): worktree branch -> main. Preflight includes
    /// dirty source files; apply commits then merges while preserving the worktree/branch.
    #[test]
    fn merge_branches_into_main_flow() {
        let repo = init_repo();
        let repo_str = repo.to_string_lossy().to_string();

        let wt = worktree_add(&repo_str, "feature x").expect("creating the worktree should succeed");
        // worktree_add still records the creation baseline even though unified merge does not read it.
        assert_eq!(
            wt.base_ref, "refs/heads/main",
            "the base ref is the current branch at the repository root"
        );

        // Preflight must mark an uncommitted b.txt dirty and include it through the temporary index.
        std::fs::write(PathBuf::from(&wt.path).join("b.txt"), "world\n").unwrap();
        let pre = merge_branches_preview(&repo_str, &wt.branch, "main").expect("the preview should succeed");
        assert!(pre.available, "main is checked out in the main repository, so merging is possible");
        assert!(pre.source_dirty, "uncommitted changes in the source working tree should be detected");
        assert!(!pre.up_to_date, "uncommitted changes mean it is not up to date");
        assert!(pre.diff_stat.contains("b.txt"), "the diff summary should mention b.txt");
        let want_dir = std::fs::canonicalize(&repo).unwrap();
        let got_dir = std::fs::canonicalize(pre.target_dir.as_deref().unwrap()).unwrap();
        assert_eq!(got_dir, want_dir, "the merge runs in the main repository where main lives");

        // Apply commits then merges; main receives b.txt while the worktree/branch remain.
        let outcome =
            merge_branches_apply(&repo_str, &wt.branch, "main", Some("add b")).expect("the merge should succeed");
        assert!(outcome.merged && !outcome.conflict, "the merge should have succeeded");
        assert!(repo.join("b.txt").exists(), "after the merge the main repository should contain b.txt");
        assert!(PathBuf::from(&wt.path).exists(), "the new flow does not clean up the worktree");

        // A subsequent preflight reports up to date.
        let pre2 = merge_branches_preview(&repo_str, &wt.branch, "main").expect("the second preview should succeed");
        assert!(pre2.up_to_date, "after merging it should be reported as up to date");

        worktree_remove(&wt.path, true).expect("removing the worktree should succeed");
        branch_delete(&repo_str, &wt.branch).expect("deleting the branch should succeed");
        let _ = std::fs::remove_dir_all(&repo);
    }

    /// Reverse unified merge (old “merge parent”): main -> worktree. A new main commit appears in
    /// preflight, becomes visible after merge, and then reports up to date.
    #[test]
    fn merge_branches_reverse_direction_flow() {
        let repo = init_repo();
        let repo_str = repo.to_string_lossy().to_string();

        let wt = worktree_add(&repo_str, "feature y").expect("creating the worktree should succeed");

        // Add c.txt on main so the worktree falls behind.
        std::fs::write(repo.join("c.txt"), "parent-new\n").unwrap();
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "main adds c"]);

        let pre = merge_branches_preview(&repo_str, "main", &wt.branch).expect("the preview should succeed");
        assert!(pre.available, "the worktree branch is checked out, so merging is possible");
        assert!(!pre.up_to_date, "the worktree is behind, so it is not up to date");
        assert!(!pre.source_dirty && !pre.target_dirty, "both sides should be clean");
        assert!(pre.diff_stat.contains("c.txt"), "the diff summary should mention c.txt");

        let out = merge_branches_apply(&repo_str, "main", &wt.branch, None).expect("the merge should succeed");
        assert!(out.merged && !out.conflict, "the merge should have succeeded");
        assert!(
            PathBuf::from(&wt.path).join("c.txt").exists(),
            "after the merge the worktree should contain c.txt"
        );

        let pre2 = merge_branches_preview(&repo_str, "main", &wt.branch).expect("the second preview should succeed");
        assert!(pre2.up_to_date, "after merging it should be reported as up to date");

        worktree_remove(&wt.path, true).expect("removing the worktree should succeed");
        branch_delete(&repo_str, &wt.branch).expect("deleting the branch should succeed");
        let _ = std::fs::remove_dir_all(&repo);
    }

    /// Conflicting edits on both sides report/list the conflict and preserve target state.
    #[test]
    fn merge_branches_conflict_detected() {
        let repo = init_repo();
        let repo_str = repo.to_string_lossy().to_string();

        let wt = worktree_add(&repo_str, "conf").expect("creating the worktree should succeed");
        std::fs::write(PathBuf::from(&wt.path).join("a.txt"), "from-child\n").unwrap();
        commit_all(&wt.path, "child edit").expect("the child commit should succeed");

        // Modify the same file on main to create a divergent conflict.
        std::fs::write(repo.join("a.txt"), "from-parent\n").unwrap();
        git(&repo, &["commit", "-q", "-am", "parent edit"]);

        let outcome =
            merge_branches_apply(&repo_str, &wt.branch, "main", None).expect("the command itself should return");
        assert!(outcome.conflict && !outcome.merged, "a conflict should be detected");
        assert!(
            outcome.conflicts.iter().any(|f| f.contains("a.txt")),
            "the conflicting files should include a.txt"
        );

        // Abort the merge in the target repository before cleanup.
        let _ = crate::host::command("git")
            .arg("-C")
            .arg(&repo)
            .args(["merge", "--abort"])
            .output();
        let _ = worktree_remove(&wt.path, true);
        let _ = std::fs::remove_dir_all(&repo);
    }

    /// An unchecked-out target is unavailable and apply names it in rejection; identical/missing
    /// branches return stable reasons without accidental merges.
    #[test]
    fn merge_branches_rejects_bad_targets() {
        let repo = init_repo();
        let repo_str = repo.to_string_lossy().to_string();

        let wt = worktree_add(&repo_str, "no target").expect("creating the worktree should succeed");
        // Switch the main repository away so main is no longer checked out anywhere.
        git(&repo, &["checkout", "-q", "-b", "other"]);

        let pre = merge_branches_preview(&repo_str, &wt.branch, "main").expect("the preview should return");
        assert!(!pre.available, "the target is not checked out, so merging is impossible");
        assert_eq!(pre.reason, "target_not_checked_out");
        assert!(pre.target_dir.is_none());

        let err = merge_branches_apply(&repo_str, &wt.branch, "main", None)
            .expect_err("an unchecked-out target should be refused");
        assert!(err.contains("main"), "the message should name the target branch");

        // Identical branch.
        let same = merge_branches_preview(&repo_str, "main", "main").expect("the preview should return");
        assert!(!same.available);
        assert_eq!(same.reason, "same_branch");
        assert!(merge_branches_apply(&repo_str, "main", "main", None).is_err());

        // Missing branch.
        let gone = merge_branches_preview(&repo_str, "no-such", "main").expect("the preview should return");
        assert!(!gone.available);
        assert_eq!(gone.reason, "branch_not_found");

        let _ = worktree_remove(&wt.path, true);
        let _ = std::fs::remove_dir_all(&repo);
    }

    /// land_targets remains used by Gitea PR landing; baseline, diff, and local availability still work.
    #[test]
    fn land_targets_still_resolves_base() {
        let repo = init_repo();
        let repo_str = repo.to_string_lossy().to_string();

        let wt = worktree_add(&repo_str, "land x").expect("creating the worktree should succeed");
        std::fs::write(PathBuf::from(&wt.path).join("b.txt"), "world\n").unwrap();

        let targets = land_targets(Some(&wt.base_ref), &wt.path).expect("the landing preview should succeed");
        assert_eq!(targets.base_branch, "main");
        assert_eq!(targets.branch, wt.branch);
        assert!(targets.has_uncommitted, "uncommitted changes should be detected");
        assert!(targets.diff_stat.contains("b.txt"), "the diff summary should mention b.txt");
        let local = targets.providers.iter().find(|p| p.id == "local").unwrap();
        assert!(local.available, "main is checked out in the main repository, so it is available");

        let _ = worktree_remove(&wt.path, true);
        let _ = branch_delete(&repo_str, &wt.branch);
        let _ = std::fs::remove_dir_all(&repo);
    }
}

#[cfg(test)]
mod porcelain_tests {
    use super::*;
    use std::path::PathBuf;

    /// Run a Git command in a directory and assert success.
    fn git(dir: &std::path::Path, args: &[&str]) {
        let out = crate::host::command("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Temporary repository with a committed a.txt on main.
    fn init_repo() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vlx-porcelain-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init", "-q"]);
        git(&dir, &["config", "user.email", "t@example.com"]);
        git(&dir, &["config", "user.name", "tester"]);
        git(&dir, &["config", "commit.gpgsign", "false"]);
        std::fs::write(dir.join("a.txt"), "hello\n").unwrap();
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", "init"]);
        git(&dir, &["branch", "-M", "main"]);
        dir
    }

    /// An unstaged-only change puts a space in column 0 of the first porcelain line. Reading that
    /// output through a whole-output trim used to eat the space, so the first file lost the first
    /// character of its path and its line counts no longer matched.
    #[test]
    fn unstaged_first_line_keeps_its_full_path() {
        let repo = init_repo();
        let repo_str = repo.to_string_lossy().to_string();
        std::fs::write(repo.join("a.txt"), "hello\nworld\n").unwrap();

        let files = changed_files(&repo_str).expect("listing changes should succeed");
        assert_eq!(files.len(), 1, "only a.txt changed: {files:?}");
        assert_eq!(files[0].path, "a.txt");
        assert_eq!(files[0].status, "modified");
        assert_eq!((files[0].additions, files[0].deletions), (1, 0));

        let _ = std::fs::remove_dir_all(&repo);
    }

    /// The same column shift used to move the first unstaged file into the staged count.
    #[test]
    fn unstaged_first_line_counts_as_unstaged() {
        let repo = init_repo();
        let repo_str = repo.to_string_lossy().to_string();
        std::fs::write(repo.join("a.txt"), "hello\nworld\n").unwrap();

        let st = status(&repo_str);
        assert!(st.is_repo);
        assert_eq!(st.unstaged, 1, "the edit is unstaged: {st:?}");
        assert_eq!(st.staged, 0, "nothing was added to the index: {st:?}");

        let _ = std::fs::remove_dir_all(&repo);
    }

    /// Staged and untracked entries keep working alongside the unstaged first line.
    #[test]
    fn mixed_states_all_report_full_paths() {
        let repo = init_repo();
        let repo_str = repo.to_string_lossy().to_string();
        std::fs::write(repo.join("a.txt"), "hello\nworld\n").unwrap();
        std::fs::write(repo.join("b.txt"), "staged\n").unwrap();
        git(&repo, &["add", "b.txt"]);
        std::fs::write(repo.join("c.txt"), "untracked\n").unwrap();

        let files = changed_files(&repo_str).expect("listing changes should succeed");
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["a.txt", "b.txt", "c.txt"], "{files:?}");
        assert_eq!(files[1].status, "added");
        assert_eq!(files[2].status, "untracked");

        let _ = std::fs::remove_dir_all(&repo);
    }
}
