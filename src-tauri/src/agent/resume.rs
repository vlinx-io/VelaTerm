//! Agent auto-resume support: capture native agent session IDs and verify their existence before resuming.
//!
//! - **Claude** IDs arrive directly in hook bodies (see `server.rs`).
//! - **Codex** notify is ineffective here, so scan `~/.codex/sessions/**/rollout-*.jsonl`. Its first
//!   `session_meta` line contains payload ID and cwd; match files created after launch with the session cwd.
//! - **Pi** normally reports through an extension, with a disk fallback scanning session headers under
//!   `~/.pi/agent/sessions` and matching cwd plus launch time.
//! - **Existence checks** verify conversation files before resume and fall back to a fresh launch only when absence
//!   is certain, avoiding hangs from invalid resume IDs.

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::db::repo;
use crate::host::AppCtx;
use crate::models::SessionKind;

// Home-directory resolution.

/// Cross-platform user home resolved through `crate::host::home_dir`.
fn home_dir() -> Option<PathBuf> {
    crate::host::home_dir()
}

/// Codex root: `CODEX_HOME` when set, otherwise `~/.codex`.
pub(crate) fn codex_home() -> Option<PathBuf> {
    if let Some(h) = std::env::var_os("CODEX_HOME") {
        return Some(PathBuf::from(h));
    }
    home_dir().map(|h| h.join(".codex"))
}

/// Claude root at `~/.claude`.
pub(crate) fn claude_home() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".claude"))
}

/// Cursor root at `~/.cursor`.
fn cursor_home() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".cursor"))
}

/// Pi root at `~/.pi/agent`, with sessions beneath `sessions/`.
fn pi_home() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".pi").join("agent"))
}

// Shared utilities.

/// Whether two directories identify the same location. Canonicalize both to resolve symlinks, falling back to raw
/// path equality if either cannot be canonicalized.
fn same_dir(a: &str, b: &str) -> bool {
    match (std::fs::canonicalize(a).ok(), std::fs::canonicalize(b).ok()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// Collects every `*.jsonl` below `root` using an explicit stack, skipping unreadable directories.
fn walk_jsonl(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                out.push(p);
            }
        }
    }
    out
}

// Codex capture.

/// Parses a rollout's first `session_meta` line into `(session_id, cwd?)`; returns None for another type or missing ID.
fn parse_rollout_meta(line: &str) -> Option<(String, Option<String>)> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    if v.get("type").and_then(|t| t.as_str()) != Some("session_meta") {
        return None;
    }
    let payload = v.get("payload")?;
    let id = payload.get("id")?.as_str()?.to_string();
    let cwd = payload
        .get("cwd")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string());
    Some((id, cwd))
}

/// Reads and parses the first line of a rollout file.
fn read_rollout_meta(path: &Path) -> Option<(String, Option<String>)> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    parse_rollout_meta(&line)
}

/// Parses a Pi session-file header into `(session_id, cwd?)`.
///
/// Current Pi files have this shape:
/// `{"type":"session","version":3,"id":"<uuid>","timestamp":"...","cwd":"..."}`
fn parse_pi_meta(line: &str) -> Option<(String, Option<String>)> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    if v.get("type").and_then(|t| t.as_str()) != Some("session") {
        return None;
    }
    let id = v.get("id")?.as_str()?.trim();
    if id.is_empty() {
        return None;
    }
    let cwd = v.get("cwd").and_then(|c| c.as_str()).map(|s| s.to_string());
    Some((id.to_string(), cwd))
}

/// Reads and parses the first line of a Pi session file.
fn read_pi_meta(path: &Path) -> Option<(String, Option<String>)> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    parse_pi_meta(&line)
}

/// Extracts the first genuine user message from a Codex rollout.
///
/// Accepts only `event_msg.payload.type == "user_message"`; the first user-role response_item is usually injected
/// environment context and cannot be a title. Codex writes user_message at submission, making this earlier and more
/// reliable than waiting for agent-turn-complete notification.
#[cfg(test)]
fn read_first_codex_user_prompt(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("event_msg")
            || v.pointer("/payload/type").and_then(|t| t.as_str()) != Some("user_message")
        {
            continue;
        }
        let Some(message) = v
            .pointer("/payload/message")
            .and_then(|message| message.as_str())
            .map(str::trim)
        else {
            continue;
        };
        if !message.is_empty() {
            return Some(message.to_string());
        }
    }
    None
}

/// Finds a rollout created after launch with a matching cwd under `codex_home/sessions` and returns its session ID.
///
/// `since` allows a two-second mtime margin because Codex writes session_meta at startup. A provided cwd must
/// match through `same_dir`; without it, choose the newest candidate. Candidates are searched newest-first.
fn capture_codex_candidates(
    codex_home: &Path,
    cwd: Option<&str>,
    since: SystemTime,
) -> Vec<String> {
    let sessions = codex_home.join("sessions");
    let threshold = since.checked_sub(Duration::from_secs(2)).unwrap_or(since);

    // Collect (mtime, path) candidates named rollout-* at or after the threshold.
    let mut cands: Vec<(SystemTime, PathBuf)> = walk_jsonl(&sessions)
        .into_iter()
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.starts_with("rollout-"))
        })
        .filter_map(|p| {
            let mtime = std::fs::metadata(&p).ok()?.modified().ok()?;
            (mtime >= threshold).then_some((mtime, p))
        })
        .collect();
    cands.sort_by_key(|c| std::cmp::Reverse(c.0)); // Newest mtime first.

    let mut matches = Vec::new();
    for (_, path) in cands {
        let Some((id, meta_cwd)) = read_rollout_meta(&path) else {
            continue;
        };
        match cwd {
            Some(want) => {
                if meta_cwd.as_deref().is_some_and(|c| same_dir(want, c)) {
                    matches.push(id);
                }
                // Skip a candidate whose cwd does not match the requested directory.
            }
            None => matches.push(id),
        }
    }
    matches
}

/// Newest matching Codex session captured since `since`, or None. Test-only: production capture goes
/// through `spawn_codex_capture`, which polls candidates while the agent starts up.
#[cfg(test)]
fn capture_codex_session(
    codex_home: &Path,
    cwd: Option<&str>,
    since: SystemTime,
) -> Option<String> {
    capture_codex_candidates(codex_home, cwd, since)
        .into_iter()
        .next()
}

// Pi capture.

fn capture_pi_candidates(
    pi_home: &Path,
    cwd: Option<&str>,
    since: SystemTime,
    newest_first: bool,
) -> Vec<String> {
    let sessions = pi_home.join("sessions");
    let threshold = since.checked_sub(Duration::from_secs(2)).unwrap_or(since);

    let mut cands: Vec<(SystemTime, PathBuf)> = walk_jsonl(&sessions)
        .into_iter()
        .filter_map(|p| {
            let mtime = std::fs::metadata(&p).ok()?.modified().ok()?;
            (mtime >= threshold).then_some((mtime, p))
        })
        .collect();
    if newest_first {
        cands.sort_by_key(|c| std::cmp::Reverse(c.0));
    } else {
        cands.sort_by_key(|c| c.0);
    }

    let mut matches = Vec::new();
    for (_, path) in cands {
        let Some((id, meta_cwd)) = read_pi_meta(&path) else {
            continue;
        };
        match cwd {
            Some(want) => {
                if meta_cwd.as_deref().is_some_and(|c| same_dir(want, c)) {
                    matches.push(id);
                }
            }
            None => matches.push(id),
        }
    }
    matches
}

/// Repairs legacy Pi sessions before launch by preferring the oldest candidate, avoiding a newer empty reopen.
pub fn capture_pi_session_candidates_oldest_first_since(
    cwd: Option<&str>,
    since: SystemTime,
) -> Vec<String> {
    let Some(home) = pi_home() else {
        return Vec::new();
    };
    capture_pi_candidates(&home, cwd, since, false)
}

/// Lists Codex rollouts newest-first for account usage fallback when a session ID is missing or invalid.
pub fn codex_rollout_paths_newest_first() -> Vec<PathBuf> {
    let Some(sessions) = codex_home().map(|h| h.join("sessions")) else {
        return Vec::new();
    };
    let mut files: Vec<(SystemTime, PathBuf)> = walk_jsonl(&sessions)
        .into_iter()
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.starts_with("rollout-"))
        })
        .filter_map(|p| {
            let mtime = std::fs::metadata(&p).ok()?.modified().ok()?;
            Some((mtime, p))
        })
        .collect();
    files.sort_by_key(|c| std::cmp::Reverse(c.0));
    files.into_iter().map(|(_, p)| p).collect()
}

/// Waits for a Codex rollout, returning its ID on match or None when the PTY ends.
///
/// Codex creates sessions lazily only when the first message is submitted, so a short fixed timeout would miss users
/// who pause at the prompt. Tie polling to PTY lifetime instead. Each pass offers matching new candidates newest-first
/// to atomic `try_claim`, which succeeds only if the target remains unbound and another session has not claimed the ID.
/// If the newest is taken, older candidates are tried to prevent cross-wiring. Polling ends with the PTY.
fn wait_for_codex_session(
    codex_home: &Path,
    cwd: Option<&str>,
    since: SystemTime,
    mut is_running: impl FnMut() -> bool,
    mut sleep: impl FnMut(Duration),
    mut try_claim: impl FnMut(&str) -> bool,
) -> bool {
    loop {
        // Scan before checking liveness so an ID written just before PTY shutdown can still be captured.
        for id in capture_codex_candidates(codex_home, cwd, since) {
            if try_claim(&id) {
                return true;
            }
        }
        if !is_running() {
            return false;
        }
        sleep(Duration::from_millis(300));
    }
}

/// Waits for a Pi session file, returning its ID on match or None when the PTY ends.
fn wait_for_pi_session(
    pi_home: &Path,
    cwd: Option<&str>,
    since: SystemTime,
    mut is_running: impl FnMut() -> bool,
    mut sleep: impl FnMut(Duration),
    mut try_claim: impl FnMut(&str) -> bool,
) -> bool {
    loop {
        for id in capture_pi_candidates(pi_home, cwd, since, true) {
            if try_claim(&id) {
                return true;
            }
        }
        if !is_running() {
            return false;
        }
        sleep(Duration::from_millis(300));
    }
}

/// Captures and stores an ID in the background for legacy Codex without lifecycle hooks, enabling later as-is resume.
///
/// This compatibility path captures IDs only and **never renames from a scanned rollout**. With concurrent Codex
/// sessions in one cwd, mtime/cwd cannot prove ownership; the old rename path could assign a neighboring title even
/// after hooks later corrected the ID. Modern Codex reports ID and prompt with an exact VelaTerm SID and skips this scan.
pub fn spawn_codex_capture(app: AppCtx, vlx_sid: String, cwd: Option<String>, pid: u32) {
    let since = SystemTime::now();
    std::thread::spawn(move || {
        // Normal resume retains its thread ID and must not be overwritten by cwd scanning. Capture only first launch
        // or fork_pending, where the current ID is merely a source anchor.
        let should_capture = app
            .db()
            .conn
            .lock()
            .ok()
            .and_then(|conn| {
                let existing = repo::get_agent_session_id(&conn, &vlx_sid).ok()?;
                let fork_pending = repo::get_fork_pending(&conn, &vlx_sid).ok()?;
                Some(existing.is_none() || fork_pending)
            })
            .unwrap_or(false);
        if !should_capture {
            return;
        }
        let Some(home) = codex_home() else {
            return;
        };
        // Without an explicit cwd, use the shell process's actual cwd where Codex launched.
        let effective_cwd = cwd.or_else(|| crate::pty::manager::process_cwd(pid));

        let claimed = wait_for_codex_session(
            &home,
            effective_cwd.as_deref(),
            since,
            || app.pty().is_running(&vlx_sid),
            std::thread::sleep,
            |id| {
                let db = app.db();
                let did_claim = match db.conn.lock() {
                    Ok(conn) => repo::claim_codex_session_id(&conn, &vlx_sid, id).unwrap_or(false),
                    Err(_) => false,
                };
                did_claim
            },
        );
        // Broadcast tree reload only after atomic claim because frontend Fork/Export availability depends on
        // agentSessionId. Newest-first claiming falls back to older candidates already not owned by another session.
        if claimed {
            app.emit(crate::host::TREE_CHANGED, ());
        }
    });
}

/// Recovers a Pi session ID from disk when the extension fails to report it.
pub fn spawn_pi_capture(
    app: AppCtx,
    vlx_sid: String,
    cwd: Option<String>,
    pid: u32,
    should_capture: bool,
) {
    let since = SystemTime::now();
    std::thread::spawn(move || {
        if !should_capture {
            return;
        }
        let Some(home) = pi_home() else {
            return;
        };
        let effective_cwd = cwd.or_else(|| crate::pty::manager::process_cwd(pid));

        let claimed = wait_for_pi_session(
            &home,
            effective_cwd.as_deref(),
            since,
            || app.pty().is_running(&vlx_sid),
            std::thread::sleep,
            |id| {
                let db = app.db();
                match db.conn.lock() {
                    Ok(conn) => repo::claim_agent_session_id(&conn, &vlx_sid, id, SessionKind::Pi)
                        .unwrap_or(false),
                    Err(_) => false,
                }
            },
        );
        if claimed {
            app.emit(crate::host::TREE_CHANGED, ());
        }
    });
}

// Pre-resume existence verification.

/// Whether a conversation is **confirmed absent**. Only true triggers fallback to a fresh launch.
///
/// Best-effort semantics: absence requires an accessible root with no matching file. An inaccessible root cannot be
/// verified and returns false so resume is still attempted.
pub fn confirmed_missing(kind: SessionKind, id: &str) -> bool {
    match kind {
        SessionKind::Claude => {
            let Some(projects) = claude_home().map(|h| h.join("projects")) else {
                return false;
            };
            if !projects.is_dir() {
                return false; // Cannot verify; do not claim absence.
            }
            !claude_transcript_exists(&projects, id)
        }
        SessionKind::Codex => {
            let Some(sessions) = codex_home().map(|h| h.join("sessions")) else {
                return false;
            };
            if !sessions.is_dir() {
                return false;
            }
            !codex_rollout_exists(&sessions, id)
        }
        // OpenCode stores sessions internally, so probe the ID directly with `opencode export <id>`.
        // Absence is claimed only on the explicit "Session not found" signal; anything else attempts resume.
        SessionKind::Opencode => opencode_session_missing(id),
        // Copilot never needs absence fallback: an unknown resume UUID starts a new session under that UUID.
        SessionKind::Copilot => false,
        // Cline stores sessions in internal SQLite. Do not verify or downgrade initially; if invalid IDs prove to
        // hang or loop, add `cline history` verification analogous to OpenCode.
        // Antigravity storage is undocumented, so do not verify or downgrade until invalid-ID behavior is tested.
        SessionKind::Antigravity => false,
        SessionKind::Cline => false,
        // Crush uses an internal database. Do not verify initially; if invalid IDs hang or repeatedly error, add
        // session-list verification analogous to OpenCode.
        SessionKind::Crush => false,
        // Kimi Code manages internal storage; let `--session` handle IDs when reliable verification is unavailable.
        SessionKind::Kimi => false,
        // Kiro keeps sessions in a SQLite database under ~/.kiro with no flat files to scan. Never downgrade;
        // whether a stale `--resume-id` hangs is still unverified on a real machine.
        SessionKind::Kiro => false,
        // Grok stores one session tree under ~/.grok/sessions. Confirm absence only when the root is readable;
        // a missing seeded UUID then launches with --session-id instead of attempting --resume.
        SessionKind::Grok => {
            let Some(home) = grok_home() else {
                return false;
            };
            let sessions = home.join("sessions");
            if !sessions.is_dir() {
                return true;
            }
            find_grok_updates_in(&sessions, id).is_none()
        }
        // Zoo creates one task directory per UUID and VelaTerm uses its own session UUID as task ID. Confirm absence
        // only when the tasks directory is readable and target missing, so launch uses --create-with-session-id.
        SessionKind::Zoo => {
            let Some(home) = crate::host::home_dir() else {
                return false;
            };
            let tasks = home
                .join(".vscode-mock")
                .join("global-storage")
                .join("tasks");
            if !tasks.is_dir() {
                return true;
            }
            !tasks.join(id).is_dir()
        }
        // Cursor stores chats at `~/.cursor/chats/<md5(cwd)>/<chatId>/`. Avoid depending on its cwd hash by scanning
        // one level for `chats/*/<id>`.
        SessionKind::Cursor => {
            let Some(chats) = cursor_home().map(|h| h.join("chats")) else {
                return false;
            };
            if !chats.is_dir() {
                return false; // Cannot verify; do not claim absence.
            }
            !cursor_chat_exists(&chats, id)
        }
        // Pi session filenames end with the resume UUID under cwd-specific directories. As with Codex, claim absence
        // only when the sessions root is readable and no JSONL filename contains the ID.
        SessionKind::Pi => {
            let Some(sessions) = pi_home().map(|h| h.join("sessions")) else {
                return false;
            };
            if !sessions.is_dir() {
                return false;
            }
            !pi_session_exists(&sessions, id)
        }
        SessionKind::Terminal => false,
        // Browser nodes have no agent conversation or resume semantics.
        SessionKind::Browser => false,
    }
}

fn grok_home() -> Option<std::path::PathBuf> {
    if let Some(home) = std::env::var_os("GROK_HOME") {
        return Some(home.into());
    }
    crate::host::home_dir().map(|home| home.join(".grok"))
}

/// Locate Grok's authoritative ACP update stream at
/// `~/.grok/sessions/<encoded-cwd>/<session-id>/updates.jsonl`.
pub fn find_grok_updates(id: &str) -> Option<std::path::PathBuf> {
    let sessions = grok_home()?.join("sessions");
    find_grok_updates_in(&sessions, id)
}

fn find_grok_updates_in(sessions: &std::path::Path, id: &str) -> Option<std::path::PathBuf> {
    if id.is_empty() || id.contains(['/', '\\']) {
        return None;
    }
    for group in std::fs::read_dir(sessions).ok()?.flatten() {
        if !group.path().is_dir() {
            continue;
        }
        let updates = group.path().join(id).join("updates.jsonl");
        if updates.is_file() {
            return Some(updates);
        }
    }
    None
}

/// Whether OpenCode reports the session ID as absent.
///
/// `opencode session list` cannot answer this: it only lists sessions belonging to the current working directory,
/// and the desktop app runs with the launcher's directory (`/` on macOS), so every ID looked absent and every
/// resume silently started an empty session. `opencode export <id>` looks the ID up directly, independent of the
/// working directory: exit 0 when it exists, exit 1 with `Session not found` when it does not.
///
/// Absence is claimed only on that explicit message. A missing binary, an older build without `export`, a locked
/// database, or any other failure leaves the answer unknown and resume proceeds; a genuinely stale ID then fails
/// fast on OpenCode's own side. Stdout is discarded because export writes the whole conversation as JSON.
fn opencode_session_missing(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    let Ok(output) = crate::host::command("opencode")
        .args(["export", id])
        .env("NO_COLOR", "1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null())
        .output()
    else {
        return false;
    };
    if output.status.success() {
        return false;
    }
    opencode_reports_not_found(&String::from_utf8_lossy(&output.stderr))
}

/// Whether OpenCode's stderr carries its "Session not found" error, ignoring ANSI styling around it.
fn opencode_reports_not_found(stderr: &str) -> bool {
    stderr.to_ascii_lowercase().contains("session not found")
}

/// Whether `projects/*/<id>.jsonl` exists for a Claude transcript named by session ID.
fn claude_transcript_exists(projects: &Path, id: &str) -> bool {
    find_claude_transcript_in(projects, id).is_some()
}

/// Whether sessions contains a rollout filename with `<id>`, such as `rollout-<ts>-<id>.jsonl`.
fn codex_rollout_exists(sessions: &Path, id: &str) -> bool {
    find_codex_rollout_in(sessions, id).is_some()
}

/// Whether sessions contains a Pi JSONL filename with the resume UUID, across cwd-specific subdirectories.
fn pi_session_exists(sessions: &Path, id: &str) -> bool {
    find_codex_rollout_in(sessions, id).is_some()
}

/// Whether `chats/*/<id>` exists for Cursor's `<cwd-hash>/<chatId>/` layout.
fn cursor_chat_exists(chats: &Path, id: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(chats) else {
        return false;
    };
    entries.flatten().any(|e| e.path().join(id).is_dir())
}

/// Finds the first Claude transcript `*/<id>.jsonl` below a projects root.
fn find_claude_transcript_in(projects: &Path, id: &str) -> Option<PathBuf> {
    let target = format!("{id}.jsonl");
    for e in std::fs::read_dir(projects).ok()?.flatten() {
        let p = e.path().join(&target);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Finds the first rollout filename containing `<id>` below a sessions root.
fn find_codex_rollout_in(sessions: &Path, id: &str) -> Option<PathBuf> {
    walk_jsonl(sessions).into_iter().find(|p| {
        p.file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n.contains(id))
    })
}

/// Locates `~/.claude/projects/*/<id>.jsonl` for archive transcript rendering; returns None if deleted or remote.
pub fn find_claude_transcript(id: &str) -> Option<PathBuf> {
    let projects = claude_home()?.join("projects");
    find_claude_transcript_in(&projects, id)
}

/// Locates a Codex rollout by agent session ID under `~/.codex/sessions`.
pub fn find_codex_rollout(id: &str) -> Option<PathBuf> {
    let sessions = codex_home()?.join("sessions");
    find_codex_rollout_in(&sessions, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Newest candidate for a cwd. Only these tests want a single ID; production callers take the
    /// full oldest-first list from `capture_pi_session_candidates_oldest_first_since`.
    fn capture_pi_session(pi_home: &Path, cwd: Option<&str>, since: SystemTime) -> Option<String> {
        capture_pi_candidates(pi_home, cwd, since, true)
            .into_iter()
            .next()
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
    }

    #[test]
    fn parse_rollout_meta_extracts_id_and_cwd() {
        let line = r#"{"timestamp":"t","type":"session_meta","payload":{"id":"uuid-1","cwd":"/private/tmp","source":"exec"}}"#;
        assert_eq!(
            parse_rollout_meta(line),
            Some(("uuid-1".to_string(), Some("/private/tmp".to_string())))
        );
        // A non-session_meta line, such as a normal message, returns None.
        assert!(parse_rollout_meta(r#"{"type":"message","payload":{}}"#).is_none());
        // Missing ID returns None.
        assert!(parse_rollout_meta(r#"{"type":"session_meta","payload":{"cwd":"/x"}}"#).is_none());
        // Invalid JSON returns None.
        assert!(parse_rollout_meta("garbage").is_none());
    }

    #[test]
    fn pi_capture_reads_header_filters_cwd_and_orders_candidates() {
        let tmp = std::env::temp_dir().join(format!("vlx-pi-cap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let dir_a = tmp.join("projA");
        let dir_b = tmp.join("projB");
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();
        let home = tmp.join("pi");

        // Build with serde_json so the path is escaped: a raw Windows path such as C:\Users\... turns
        // \U into an invalid JSON escape, the line fails to parse, and the candidate is silently skipped.
        let meta = |id: &str, cwd: &Path| {
            serde_json::json!({
                "type": "session",
                "version": 3,
                "id": id,
                "timestamp": "2026-07-21T03:11:37.854Z",
                "cwd": cwd.to_string_lossy(),
            })
            .to_string()
        };
        write_file(
            &home.join("sessions/--projA--/2026-07-21T03-10-00-000Z_pi-old.jsonl"),
            &meta("pi-old", &dir_a),
        );
        std::thread::sleep(Duration::from_millis(10));
        write_file(
            &home.join("sessions/--projA--/2026-07-21T03-11-00-000Z_pi-new.jsonl"),
            &meta("pi-new", &dir_a),
        );
        write_file(
            &home.join("sessions/--projB--/2026-07-21T03-12-00-000Z_pi-b.jsonl"),
            &meta("pi-b", &dir_b),
        );

        let since = SystemTime::now() - Duration::from_secs(3600);
        assert_eq!(
            capture_pi_session(&home, Some(dir_a.to_str().unwrap()), since).as_deref(),
            Some("pi-new")
        );
        assert_eq!(
            capture_pi_candidates(&home, Some(dir_a.to_str().unwrap()), since, false),
            vec!["pi-old".to_string(), "pi-new".to_string()]
        );
        assert_eq!(
            capture_pi_session(&home, Some(dir_b.to_str().unwrap()), since).as_deref(),
            Some("pi-b")
        );

        let future = SystemTime::now() + Duration::from_secs(3600);
        assert!(capture_pi_session(&home, Some(dir_a.to_str().unwrap()), future).is_none());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn reads_first_real_codex_user_prompt() {
        let tmp = std::env::temp_dir().join(format!("vlx-prompt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let rollout = tmp.join("rollout.jsonl");
        write_file(
            &rollout,
            concat!(
                r#"{"type":"session_meta","payload":{"id":"x","cwd":"/p"}}"#,
                "\n",
                r#"{"type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"<environment_context>injected</environment_context>"}]}}"#,
                "\n",
                r#"{"type":"event_msg","payload":{"type":"user_message","message":"  Fix the Codex automatic title  ","images":[]}}"#,
                "\n",
                r#"{"type":"event_msg","payload":{"type":"user_message","message":"a later message"}}"#,
            ),
        );
        assert_eq!(
            read_first_codex_user_prompt(&rollout).as_deref(),
            Some("Fix the Codex automatic title")
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn codex_prompt_reader_ignores_injected_response_items_and_empty_messages() {
        let tmp = std::env::temp_dir().join(format!("vlx-empty-prompt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let rollout = tmp.join("rollout.jsonl");
        write_file(
            &rollout,
            concat!(
                r#"{"type":"response_item","payload":{"role":"user","content":[{"type":"input_text","text":"injected"}]}}"#,
                "\n",
                r#"{"type":"event_msg","payload":{"type":"user_message","message":"   "}}"#,
            ),
        );
        assert!(read_first_codex_user_prompt(&rollout).is_none());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn capture_picks_cwd_matching_newest() {
        let tmp = std::env::temp_dir().join(format!("vlx-cap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let dir_a = tmp.join("projA");
        let dir_b = tmp.join("projB");
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();
        let home = tmp.join("codex");

        // serde_json escapes the path; see the note in the Pi test above.
        let meta = |id: &str, cwd: &Path| {
            serde_json::json!({
                "type": "session_meta",
                "payload": { "id": id, "cwd": cwd.to_string_lossy() },
            })
            .to_string()
        };
        write_file(
            &home.join("sessions/2026/06/03/rollout-1-id-a.jsonl"),
            &meta("id-a", &dir_a),
        );
        write_file(
            &home.join("sessions/2026/06/03/rollout-2-id-b.jsonl"),
            &meta("id-b", &dir_b),
        );

        // Use an earlier since value so both files pass the mtime threshold.
        let since = SystemTime::now() - Duration::from_secs(3600);
        // cwd=dir_a selects id-a even when id-b is newer.
        let got = capture_codex_session(&home, Some(dir_a.to_str().unwrap()), since);
        assert_eq!(got.as_deref(), Some("id-a"));
        // cwd=dir_b selects id-b.
        let got_b = capture_codex_session(&home, Some(dir_b.to_str().unwrap()), since);
        assert_eq!(got_b.as_deref(), Some("id-b"));

        // A future since value leaves no eligible candidates.
        let future = SystemTime::now() + Duration::from_secs(3600);
        assert!(capture_codex_session(&home, Some(dir_a.to_str().unwrap()), future).is_none());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn wait_capture_does_not_expire_before_late_first_prompt() {
        let tmp = std::env::temp_dir().join(format!(
            "vlx-wait-cap-{}-{:?}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let home = tmp.join("codex");
        let cwd = tmp.join("project");
        std::fs::create_dir_all(&cwd).unwrap();
        let rollout = home.join("sessions/2026/07/11/rollout-late.jsonl");
        let since = SystemTime::now() - Duration::from_secs(1);
        let mut waits = 0;
        let mut claimed_id: Option<String> = None;

        let got = wait_for_codex_session(
            &home,
            Some(cwd.to_str().unwrap()),
            since,
            || true,
            |_| {
                waits += 1;
                // The old implementation stopped after about eight seconds; simulate a later first message.
                if waits == 30 {
                    // serde_json escapes the path. Written by hand, a Windows path made this line
                    // unparseable, the candidate never matched, and this test span forever.
                    write_file(
                        &rollout,
                        &serde_json::json!({
                            "type": "session_meta",
                            "payload": { "id": "late-id", "cwd": cwd.to_string_lossy() },
                        })
                        .to_string(),
                    );
                }
            },
            |id| {
                claimed_id = Some(id.to_string()); // Always claim and retain the ID for assertion.
                true
            },
        );

        assert_eq!(waits, 30);
        assert!(got, "a candidate that appears and is claimed successfully should return true");
        assert_eq!(claimed_id.as_deref(), Some("late-id"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn wait_capture_stops_when_pty_session_ends() {
        let tmp = std::env::temp_dir().join(format!("vlx-wait-stop-{}", std::process::id()));
        let mut slept = false;
        let got = wait_for_codex_session(
            &tmp,
            None,
            SystemTime::now(),
            || false,
            |_| slept = true,
            |_| false,
        );

        assert!(!got, "once the PTY has ended with nothing claimed it should return false");
        assert!(!slept, "polling should stop once the PTY has ended");
    }

    #[test]
    fn existence_checks_detect_presence_and_absence() {
        let tmp = std::env::temp_dir().join(format!("vlx-exist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);

        // claude：projects/<encoded>/<id>.jsonl
        let projects = tmp.join("claude/projects");
        write_file(&projects.join("-Users-x/sess-1.jsonl"), "{}");
        assert!(claude_transcript_exists(&projects, "sess-1"));
        assert!(!claude_transcript_exists(&projects, "sess-missing"));

        // codex：sessions/.../rollout-<ts>-<id>.jsonl
        let sessions = tmp.join("codex/sessions");
        write_file(&sessions.join("2026/06/03/rollout-1-uuid-9.jsonl"), "{}");
        assert!(codex_rollout_exists(&sessions, "uuid-9"));
        assert!(!codex_rollout_exists(&sessions, "uuid-absent"));

        // Cursor: verify directory existence for chats/<cwd-hash>/<chatId>/store.db.
        let chats = tmp.join("cursor/chats");
        write_file(&chats.join("b83a109e/chat-uuid-1/store.db"), "");
        assert!(cursor_chat_exists(&chats, "chat-uuid-1"));
        assert!(!cursor_chat_exists(&chats, "chat-absent"));

        // Pi: verify the UUID in sessions/--<cwd>--/<timestamp>_<UUID>.jsonl filenames.
        let pi_sessions = tmp.join("pi/sessions");
        write_file(
            &pi_sessions.join("--Users-x--/2026-07-03T10-00-00_pi-uuid-7.jsonl"),
            "{}",
        );
        assert!(pi_session_exists(&pi_sessions, "pi-uuid-7"));
        assert!(!pi_session_exists(&pi_sessions, "pi-uuid-absent"));

        // Grok: sessions/<encoded-cwd>/<session-id>/updates.jsonl.
        let grok_sessions = tmp.join("grok/sessions");
        write_file(
            &grok_sessions.join("%2FUsers%2Fx/grok-uuid-3/updates.jsonl"),
            "{}",
        );
        assert_eq!(
            find_grok_updates_in(&grok_sessions, "grok-uuid-3"),
            Some(grok_sessions.join("%2FUsers%2Fx/grok-uuid-3/updates.jsonl"))
        );
        assert!(find_grok_updates_in(&grok_sessions, "grok-absent").is_none());
        assert!(find_grok_updates_in(&grok_sessions, "../escape").is_none());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn terminal_never_confirmed_missing() {
        assert!(!confirmed_missing(SessionKind::Terminal, "whatever"));
    }

    /// Only OpenCode's own "Session not found" counts as absence. The real message arrives wrapped in ANSI
    /// styling; every other failure must stay unknown so resume is still attempted.
    #[test]
    fn opencode_absence_needs_the_not_found_message() {
        assert!(opencode_reports_not_found(
            "\u{1b}[91m\u{1b}[1mError: \u{1b}[0mSession not found: ses_abc\r\n"
        ));
        assert!(opencode_reports_not_found("Error: Session not found: ses_abc"));
        assert!(!opencode_reports_not_found(""));
        assert!(!opencode_reports_not_found("Unknown argument: export"));
        assert!(!opencode_reports_not_found("Error: database is locked"));
    }

    /// An empty ID never reaches the CLI, so it is never reported as absent.
    #[test]
    fn opencode_empty_id_is_not_missing() {
        assert!(!opencode_session_missing(""));
        assert!(!confirmed_missing(SessionKind::Opencode, ""));
    }

    /// Read-only check against the OpenCode installed on this machine: a fabricated ID must be reported absent
    /// regardless of the working directory, which is what `session list` got wrong.
    /// `cargo test --lib -- --ignored opencode_absence_real_cli_smoke --nocapture`
    #[test]
    #[ignore = "depends on the opencode CLI installed on this machine"]
    fn opencode_absence_real_cli_smoke() {
        assert!(opencode_session_missing("ses_definitelynotreal1234"));
        // Set VLX_TEST_OPENCODE_SESSION_ID to an ID that exists in any project to cover the other
        // direction: an existing session must survive the check from an unrelated working directory.
        if let Ok(real) = std::env::var("VLX_TEST_OPENCODE_SESSION_ID") {
            assert!(!opencode_session_missing(&real));
        }
    }

    /// Read-only smoke test against real `~/.codex/sessions`, validating nested traversal and first-line session_meta
    /// parsing with full payloads. Reads only ID/cwd and is ignored by default because local history is required.
    /// `cargo test --lib -- --ignored codex_capture_real_dir_smoke --nocapture`
    #[test]
    #[ignore = "depends on the real ~/.codex/sessions history on this machine"]
    fn codex_capture_real_dir_smoke() {
        let home = codex_home().expect("codex_home should resolve");
        // An old since value admits all history; cwd=None selects the newest rollout.
        let since = SystemTime::now() - Duration::from_secs(3650 * 24 * 3600);
        let got = capture_codex_session(&home, None, since);
        eprintln!("latest session id captured from the real ~/.codex/sessions = {got:?}");
        assert!(
            got.is_some(),
            "at least one session id should be parsed out of the real rollout files"
        );
    }
}
