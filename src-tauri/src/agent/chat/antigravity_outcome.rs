//! Native SQLite trajectory outcome inspection for Google Antigravity (`agy`).
//!
//! `agy acp` frequently reports upstream API, quota, or history-validation failures as ordinary
//! assistant text followed by `stopReason: "end_turn"`, hiding the failure from JSON-RPC. Meanwhile
//! Antigravity's own conversation database (`conversations/<id>.db`) appends a typed step row to
//! `steps` (`step_type = 17` for `CORTEX_STEP_TYPE_ERROR_MESSAGE`, `15` for `PLANNER_RESPONSE`).
//!
//! Recording `max(idx)` before dispatching `session/prompt` and inspecting only steps added by that
//! prompt (read-only, on an unlinked database file) exposes the real failure class, HTTP/gRPC code,
//! and quota reset timestamps without relying solely on fragile chat text matching.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use rusqlite::{params, Connection, OpenFlags};

use super::acp_protocol;
use super::antigravity_protocol;
use super::auto_continue::Reset;

/// `CORTEX_STEP_TYPE_PLANNER_RESPONSE`: the model produced a response within this turn.
const PLANNER_RESPONSE_STEP_TYPE: i64 = 15;
/// `CORTEX_STEP_TYPE_ERROR_MESSAGE`: the runtime terminated the turn on an error.
const ERROR_STEP_TYPE: i64 = 17;
/// Error text leads `step_payload`; a Go stack trace follows it.
const MAX_ERROR_PAYLOAD_BYTES: i64 = 16_384;

/// Failure uncovered from Antigravity's native trajectory log or `stopReason` after a prompt ends.
#[derive(Clone, Debug, PartialEq)]
pub enum HiddenTurnFailure {
    /// Cold-start model map not ready or plan model unset (`failed to construct executor: ...`).
    ColdModelNotReady {
        message: String,
        plan_model_missing: bool,
    },
    /// Google OAuth 401 credential expiration/rejection.
    Auth401 {
        message: String,
    },
    /// Transient upstream API or transport error safe to continue automatically.
    TransientUpstream {
        message: String,
    },
    /// Account quota or rate-limit exhaustion, with parsed reset timestamp if reported.
    UsageLimit {
        message: String,
        reset: Option<Reset>,
    },
    /// Saved Claude `thinking` block lost its signature; this native conversation cannot continue.
    InvalidThinkingSignature {
        message: String,
    },
    /// Non-retryable or unrecognized upstream error hidden behind `end_turn`.
    ProviderError {
        message: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
    #[cfg(not(unix))]
    created_or_len: u64,
}

fn unlinked_file_identity(path: &Path) -> Option<FileIdentity> {
    let meta = fs::symlink_metadata(path).ok()?;
    if meta.file_type().is_symlink() || !meta.file_type().is_file() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some(FileIdentity { dev: meta.dev(), ino: meta.ino() })
    }
    #[cfg(not(unix))]
    {
        Some(FileIdentity { created_or_len: meta.len() })
    }
}

fn unlinked_ancestors(root: &Path, leaf: &Path) -> bool {
    let Ok(rel) = leaf.strip_prefix(root) else {
        return false;
    };
    let mut cur = root.to_path_buf();
    if let Ok(meta) = fs::symlink_metadata(&cur) {
        if meta.file_type().is_symlink() {
            return false;
        }
    }
    for comp in rel.components() {
        cur.push(comp);
        match fs::symlink_metadata(&cur) {
            Ok(meta) if meta.file_type().is_symlink() => return false,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    true
}

/// Resolve candidate `(root, data_dir)` roots honoring custom `--gemini-dir`, `--app-data-dir`,
/// and `--cli_dir_name` flags when present.
fn resolve_data_dirs(settings_args: &[String]) -> Option<Vec<(PathBuf, PathBuf)>> {
    let mut gemini_dir: Option<PathBuf> = None;
    let mut app_data_dir: Option<String> = None;
    let mut cli_dir_name: Option<String> = None;
    let mut iter = settings_args.iter();
    while let Some(arg) = iter.next() {
        let trimmed = arg.trim();
        if matches!(trimmed, "--gemini-dir" | "--gemini_dir") {
            if let Some(val) = iter.next() {
                let v = val.trim();
                if !v.is_empty() {
                    gemini_dir = Some(PathBuf::from(v));
                }
            }
        } else if matches!(trimmed, "--app-data-dir" | "--app_data_dir") {
            if let Some(val) = iter.next() {
                let v = val.trim();
                if !v.is_empty() {
                    app_data_dir = Some(v.to_string());
                }
            }
        } else if trimmed == "--cli_dir_name" {
            if let Some(val) = iter.next() {
                let v = val.trim();
                if !v.is_empty() {
                    cli_dir_name = Some(v.to_string());
                }
            }
        } else if let Some(v) = trimmed
            .strip_prefix("--gemini-dir=")
            .or_else(|| trimmed.strip_prefix("--gemini_dir="))
        {
            let v = v.trim();
            if !v.is_empty() {
                gemini_dir = Some(PathBuf::from(v));
            }
        } else if let Some(v) = trimmed
            .strip_prefix("--app-data-dir=")
            .or_else(|| trimmed.strip_prefix("--app_data_dir="))
        {
            let v = v.trim();
            if !v.is_empty() {
                app_data_dir = Some(v.to_string());
            }
        } else if let Some(v) = trimmed.strip_prefix("--cli_dir_name=") {
            let v = v.trim();
            if !v.is_empty() {
                cli_dir_name = Some(v.to_string());
            }
        }
    }

    let base_gemini = gemini_dir.or_else(|| crate::host::home_dir().map(|h| h.join(".gemini")))?;
    let mut dirs = Vec::new();
    if let Some(app_dir) = app_data_dir {
        let sub = app_dir.trim_start_matches(['/', '\\']);
        dirs.push((base_gemini.clone(), base_gemini.join(sub)));
        let direct = PathBuf::from(&app_dir);
        if direct.is_absolute() {
            dirs.push((direct.clone(), direct));
        }
    }
    if let Some(cli_name) = cli_dir_name {
        dirs.push((base_gemini.clone(), base_gemini.join(cli_name)));
    }
    dirs.push((base_gemini.clone(), base_gemini.join("antigravity-cli")));
    dirs.push((base_gemini.clone(), base_gemini.join("antigravity-acp")));
    Some(dirs)
}

/// Resolve the candidate conversation `.db` path for `native_id`, honoring custom `--gemini-dir`
/// and `--app-data-dir` / `--cli_dir_name` flags when present.
pub fn resolve_conversation_db_path(native_id: &str, settings_args: &[String]) -> Option<PathBuf> {
    if !crate::agent::antigravity::is_native_id(native_id) {
        return None;
    }
    let db_name = format!("{native_id}.db");
    let candidates: Vec<(PathBuf, PathBuf)> = resolve_data_dirs(settings_args)?
        .into_iter()
        .map(|(root, data_dir)| (root, data_dir.join("conversations").join(&db_name)))
        .collect();

    for (root, candidate) in &candidates {
        if candidate.exists() {
            return unlinked_ancestors(root, candidate)
                .then(|| candidate.clone())
                .filter(|p| unlinked_file_identity(p).is_some());
        }
    }
    let (root, primary) = candidates.into_iter().next()?;
    unlinked_ancestors(&root, &primary).then_some(primary)
}

/// Whether Antigravity's local native recording (`.db` or `transcript_full.jsonl`) still exists on
/// disk for `native_id`. Used to avoid overwriting a live conversation binding on transient errors.
pub fn has_native_recording(native_id: &str, settings_args: &[String]) -> bool {
    if !crate::agent::antigravity::is_native_id(native_id) {
        return false;
    }
    let Some(dirs) = resolve_data_dirs(settings_args) else {
        return false;
    };
    let db_name = format!("{native_id}.db");
    for (root, data_dir) in &dirs {
        let db = data_dir.join("conversations").join(&db_name);
        if db.exists() && unlinked_ancestors(root, &db) && unlinked_file_identity(&db).is_some() {
            return true;
        }
        let transcript = data_dir
            .join("brain")
            .join(native_id)
            .join(".system_generated")
            .join("logs")
            .join("transcript_full.jsonl");
        if transcript.exists()
            && unlinked_ancestors(root, &transcript)
            && unlinked_file_identity(&transcript).is_some()
        {
            return true;
        }
    }
    false
}

/// Snapshot of a session's `steps` cursor taken right before `session/prompt` goes out.
#[derive(Clone, Debug)]
pub struct PromptOutcomeWatch {
    db_path: Option<PathBuf>,
    before_identity: Option<FileIdentity>,
    start_index: i64,
    usable: bool,
}

impl PromptOutcomeWatch {
    /// Capture the pre-prompt step index for `native_id` (using `settings_args` to locate the DB).
    pub fn capture(native_id: &str, settings_args: &[String]) -> Self {
        Self::capture_at(resolve_conversation_db_path(native_id, settings_args))
    }

    /// Capture against an explicit `.db` path (used directly by unit tests).
    pub fn capture_at(db_path: Option<PathBuf>) -> Self {
        let Some(path) = db_path else {
            return Self { db_path: None, before_identity: None, start_index: -1, usable: false };
        };
        match fs::symlink_metadata(&path) {
            Ok(meta) => {
                if meta.file_type().is_symlink() || !meta.file_type().is_file() {
                    return Self { db_path: Some(path), before_identity: None, start_index: -1, usable: false };
                }
                let identity = unlinked_file_identity(&path);
                let Some(start_index) = read_last_step_index(&path) else {
                    return Self { db_path: Some(path), before_identity: identity, start_index: -1, usable: false };
                };
                Self {
                    db_path: Some(path),
                    before_identity: identity,
                    start_index,
                    usable: true,
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Self {
                db_path: Some(path),
                before_identity: None,
                start_index: -1,
                usable: true,
            },
            Err(_) => Self { db_path: Some(path), before_identity: None, start_index: -1, usable: false },
        }
    }

    /// Inspect steps added since `capture` and combine with `stop_reason` to detect hidden failures.
    pub fn check(&self, stop_reason: &str, now_epoch_secs: Option<i64>) -> Option<HiddenTurnFailure> {
        let step = self.usable.then(|| self.read_added_step()).flatten();
        classify_turn_outcome(stop_reason, step.as_ref(), now_epoch_secs)
    }

    /// Return the resolved conversation `.db` path when this watch is usable.
    pub fn db_path(&self) -> Option<&Path> {
        self.usable.then_some(self.db_path.as_deref()).flatten()
    }

    /// Sample Antigravity's native conversation DB and WAL metadata plus latest `steps` payload
    /// size so the prompt inactivity watchdog can detect ongoing generation while `agy acp` is
    /// silent on stdout.
    pub fn activity_fingerprint(&self) -> Option<DbActivityFingerprint> {
        if !self.usable {
            return None;
        }
        let path = self.db_path.as_ref()?;
        let after_identity = unlinked_file_identity(path)?;
        #[cfg(unix)]
        if let Some(before) = self.before_identity {
            if before != after_identity {
                return None;
            }
        }
        let _ = after_identity;
        let db_meta = fs::symlink_metadata(path).ok()?;
        let wal_path = sidecar_path(path, "-wal");
        let wal_meta = unlinked_file_identity(&wal_path)
            .and_then(|_| fs::symlink_metadata(&wal_path).ok());
        let latest_step = read_latest_step_activity(path);
        Some(DbActivityFingerprint {
            db_len: db_meta.len(),
            db_modified: db_meta.modified().ok(),
            wal_len: wal_meta.as_ref().map(|m| m.len()),
            wal_modified: wal_meta.and_then(|m| m.modified().ok()),
            latest_step,
        })
    }

    fn read_added_step(&self) -> Option<AddedStep> {
        let path = self.db_path.as_ref()?;
        let after_identity = unlinked_file_identity(path)?;
        #[cfg(unix)]
        if let Some(before) = self.before_identity {
            if before != after_identity {
                return None;
            }
        }
        let _ = after_identity;
        read_last_step_after(path, self.start_index)
    }
}

/// Snapshot of Antigravity's native SQLite conversation file and latest `steps` row size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbActivityFingerprint {
    db_len: u64,
    db_modified: Option<SystemTime>,
    wal_len: Option<u64>,
    wal_modified: Option<SystemTime>,
    latest_step: Option<(i64, i64, usize)>,
}

impl DbActivityFingerprint {
    /// Merge a newer sample into `self` and report whether genuine DB/WAL progress occurred.
    ///
    /// Transient `None` reads of `latest_step` (for example under SQLite write contention) preserve
    /// the previous step reading so a brief lock never looks like false progress on the next tick.
    pub fn advance_from(&mut self, next: Self) -> bool {
        let db_changed = next.db_len != self.db_len
            || match (self.db_modified, next.db_modified) {
                (Some(prev), Some(cur)) => cur > prev,
                (None, Some(_)) => true,
                _ => false,
            };
        let wal_changed = match (self.wal_len, next.wal_len) {
            (Some(prev), Some(cur)) => cur != prev,
            (None, Some(cur)) => cur > 0,
            _ => false,
        } || match (self.wal_modified, next.wal_modified) {
            (Some(prev), Some(cur)) => cur > prev,
            (None, Some(_)) => next.wal_len.unwrap_or(0) > 0,
            _ => false,
        };
        let step_changed = match (self.latest_step, next.latest_step) {
            (Some(prev), Some(cur)) => cur != prev,
            (None, Some(_)) => true,
            _ => false,
        };
        self.db_len = next.db_len;
        if next.db_modified.is_some() {
            self.db_modified = next.db_modified;
        }
        if next.wal_len.is_some() {
            self.wal_len = next.wal_len;
        }
        if next.wal_modified.is_some() {
            self.wal_modified = next.wal_modified;
        }
        if next.latest_step.is_some() {
            self.latest_step = next.latest_step;
        }
        db_changed || wal_changed || step_changed
    }
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut raw: OsString = path.as_os_str().to_os_string();
    raw.push(suffix);
    PathBuf::from(raw)
}

fn read_latest_step_activity(path: &Path) -> Option<(i64, i64, usize)> {
    let conn = open_readonly_steps_db(path)?;
    let (idx, step_type, payload_len): (i64, i64, i64) = conn
        .query_row(
            "SELECT idx, step_type, COALESCE(length(step_payload), 0) FROM steps ORDER BY idx DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok()?;
    Some((idx, step_type, payload_len.max(0) as usize))
}

#[derive(Debug)]
struct AddedStep {
    step_type: i64,
    payload: Vec<u8>,
    reached_model: bool,
}

fn open_readonly_steps_db(path: &Path) -> Option<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let _ = conn.busy_timeout(Duration::from_millis(100));
    let has_steps: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'steps' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();
    has_steps?;
    Some(conn)
}

fn read_last_step_index(path: &Path) -> Option<i64> {
    let conn = match open_readonly_steps_db(path) {
        Some(conn) => conn,
        None => {
            // An empty or freshly initialized SQLite file before `steps` is created is valid; a
            // corrupt non-SQLite file fails `sqlite_master` and must not count as usable.
            let conn = Connection::open_with_flags(
                path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .ok()?;
            let _ = conn.busy_timeout(Duration::from_millis(100));
            conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get::<_, i64>(0))
                .ok()?;
            return Some(-1);
        }
    };
    let max_idx: Option<i64> = conn
        .query_row("SELECT max(idx) FROM steps", [], |row| row.get(0))
        .ok()?;
    Some(max_idx.unwrap_or(-1))
}

fn read_last_step_after(path: &Path, start_index: i64) -> Option<AddedStep> {
    let conn = open_readonly_steps_db(path)?;
    let (step_type, payload): (i64, Option<Vec<u8>>) = conn
        .query_row(
            "SELECT step_type, substr(step_payload, 1, ?1) FROM steps WHERE idx > ?2 ORDER BY idx DESC LIMIT 1",
            params![MAX_ERROR_PAYLOAD_BYTES, start_index],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok()?;
    let reached_model = conn
        .query_row(
            "SELECT 1 FROM steps WHERE idx > ?1 AND step_type = ?2 LIMIT 1",
            params![start_index, PLANNER_RESPONSE_STEP_TYPE],
            |row| row.get::<_, i64>(0),
        )
        .is_ok();
    Some(AddedStep {
        step_type,
        payload: payload.unwrap_or_default(),
        reached_model,
    })
}

#[derive(Debug, PartialEq, Eq)]
struct ParsedApiError {
    http_code: u16,
    message: String,
    status: String,
}

/// Parse Go harness error text (`Error 429, Message: ..., Status: RESOURCE_EXHAUSTED`) or
/// Antigravity 1.5 Google-account status error (`NOT_FOUND (code 404): Requested entity was not found.`).
fn parse_api_error(text: &str) -> Option<ParsedApiError> {
    if let Some(err) = parse_go_harness_error(text) {
        return Some(err);
    }
    parse_status_code_error(text)
}

fn parse_go_harness_error(text: &str) -> Option<ParsedApiError> {
    let mut search_from = 0;
    while let Some(rel) = text[search_from..].find("Error ") {
        let start = search_from + rel + "Error ".len();
        search_from = start;
        if text.len() < start + 3 {
            break;
        }
        let code_str = &text[start..start + 3];
        let Ok(http_code) = code_str.parse::<u16>() else {
            continue;
        };
        let Some(after_code) = text[start + 3..].strip_prefix(", Message: ") else {
            continue;
        };
        let Some(status_sep) = after_code.find(", Status: ") else {
            continue;
        };
        let message = after_code[..status_sep].trim();
        if message.is_empty() || message.contains("Error ") && parse_go_harness_error(message).is_some() {
            // Prefer the innermost structured error if nested.
            if let Some(inner) = parse_go_harness_error(message) {
                return Some(inner);
            }
        }
        let after_status = &after_code[status_sep + ", Status: ".len()..];
        let status_end = after_status
            .find([',', '\n', '\r'])
            .unwrap_or(after_status.len());
        let status = after_status[..status_end].trim();
        if !message.is_empty() && !status.is_empty() {
            return Some(ParsedApiError {
                http_code,
                message: clean_user_message(message),
                status: status.to_string(),
            });
        }
    }
    None
}

fn parse_status_code_error(text: &str) -> Option<ParsedApiError> {
    for line in text.lines() {
        let mut cursor = 0;
        while let Some(rel) = line[cursor..].find(" (code ") {
            let status_end = cursor + rel;
            let after_open = status_end + " (code ".len();
            cursor = after_open;
            let status_start = line[..status_end]
                .rfind(|c: char| !(c.is_ascii_uppercase() || c == '_'))
                .map(|idx| idx + 1)
                .unwrap_or(0);
            let status = &line[status_start..status_end];
            if status.is_empty() || !status.bytes().all(|b| b.is_ascii_uppercase() || b == b'_') {
                continue;
            }
            if line.len() < after_open + 3 {
                continue;
            }
            let Ok(http_code) = line[after_open..after_open + 3].parse::<u16>() else {
                continue;
            };
            let Some(rest) = line[after_open + 3..].strip_prefix("): ") else {
                continue;
            };
            let end = rest
                .find(|c: char| c.is_control())
                .unwrap_or(rest.len());
            let message = rest[..end].trim();
            if !message.is_empty() {
                return Some(ParsedApiError {
                    http_code,
                    message: clean_user_message(message),
                    status: status.to_string(),
                });
            }
        }
    }
    None
}

/// Strip control bytes and Go stack traces so raw internal traces never reach user-facing errors.
fn clean_user_message(raw: &str) -> String {
    let without_stack = raw
        .split("\n(1) attached stack trace")
        .next()
        .unwrap_or(raw)
        .split("\n  -- stack trace:")
        .next()
        .unwrap_or(raw);
    let first_clean: String = without_stack
        .chars()
        .take_while(|&c| c == '\n' || !c.is_control())
        .collect();
    let trimmed = first_clean.trim();
    if trimmed.len() > 600 {
        let mut cut = 600;
        while !trimmed.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}...", &trimmed[..cut])
    } else {
        trimmed.to_string()
    }
}

fn classify_turn_outcome(
    stop_reason: &str,
    step: Option<&AddedStep>,
    now_epoch_secs: Option<i64>,
) -> Option<HiddenTurnFailure> {
    let error_text = step
        .filter(|s| s.step_type == ERROR_STEP_TYPE)
        .map(|s| String::from_utf8_lossy(&s.payload).into_owned());
    if error_text.is_none() && stop_reason != "refusal" {
        return None;
    }
    let text = error_text.as_deref().unwrap_or("");

    if acp_protocol::antigravity_model_not_ready_text(text) {
        let missing = acp_protocol::antigravity_plan_model_missing_text(text);
        return Some(HiddenTurnFailure::ColdModelNotReady {
            message: extract_executor_error_line(text)
                .unwrap_or_else(|| "Antigravity is still loading its model list.".to_string()),
            plan_model_missing: missing,
        });
    }

    if antigravity_protocol::is_oauth_401_error(text) {
        let api = parse_api_error(text);
        let message = api
            .map(|e| format!("{} (code {}): {}", e.status, e.http_code, e.message))
            .unwrap_or_else(|| {
                "UNAUTHENTICATED (code 401): Antigravity credentials expired or were rejected.".to_string()
            });
        return Some(HiddenTurnFailure::Auth401 { message });
    }

    if antigravity_protocol::is_invalid_thinking_signature(text) {
        return Some(HiddenTurnFailure::InvalidThinkingSignature {
            message: "Antigravity's saved conversation can no longer be sent to the model. Your next message will automatically start a fresh Antigravity session with this conversation's context.".to_string(),
        });
    }

    let api = parse_api_error(text);
    let quota_exhausted = stop_reason == "refusal" || text.contains("reason:QUOTA_EXHAUSTED");
    let rate_limited = api.as_ref().is_some_and(|e| {
        e.http_code == 429 || e.status == "RESOURCE_EXHAUSTED"
    }) || antigravity_protocol::is_usage_limit_error(text);

    if quota_exhausted || rate_limited {
        let reset = antigravity_protocol::parse_usage_limit_reset(text, now_epoch_secs);
        let message = if quota_exhausted {
            "You have reached your Antigravity quota for this period.".to_string()
        } else {
            api.as_ref()
                .map(|e| e.message.clone())
                .unwrap_or_else(|| "You have reached your Antigravity rate limit. Please try again shortly.".to_string())
        };
        return Some(HiddenTurnFailure::UsageLimit { message, reset });
    }

    let reached_model = step.is_some_and(|s| s.reached_model);
    if let Some(ref err) = api {
        if err.http_code == 401 || err.status == "UNAUTHENTICATED" {
            return Some(HiddenTurnFailure::Auth401 {
                message: format!("{} (code {}): {}", err.status, err.http_code, err.message),
            });
        }
        // A 404 after the model already answered in this turn is a transient Google edge error;
        // a 404 before any model response is an unentitled or unknown model.
        let transient_404 = err.http_code == 404 && reached_model;
        let transient_5xx = err.http_code >= 500
            || matches!(
                err.status.as_str(),
                "INTERNAL" | "UNAVAILABLE" | "DEADLINE_EXCEEDED"
            );
        if transient_404 || transient_5xx || antigravity_protocol::is_transient_upstream_error(text) {
            return Some(HiddenTurnFailure::TransientUpstream {
                message: format!("{} (code {}): {}", err.status, err.http_code, err.message),
            });
        }
        return Some(HiddenTurnFailure::ProviderError {
            message: err.message.clone(),
        });
    }

    if antigravity_protocol::is_transient_upstream_error(text) {
        return Some(HiddenTurnFailure::TransientUpstream {
            message: "There was a network issue connecting to the server, please try again.".to_string(),
        });
    }

    Some(HiddenTurnFailure::ProviderError {
        message: "Antigravity stopped this turn because of an upstream error.".to_string(),
    })
}

fn extract_executor_error_line(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let idx = lower.find("failed to construct executor")?;
    let tail = &text[idx..];
    let line = tail
        .lines()
        .next()?
        .chars()
        .take_while(|c| !c.is_control())
        .collect::<String>();
    let trimmed = line.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER_INPUT: i64 = 14;
    const PLANNER_RESPONSE: i64 = 15;
    const TOOL: i64 = 8;
    const ERROR_MESSAGE: i64 = 17;

    fn error_payload(error: &str) -> Vec<u8> {
        let mut out = vec![0x0a, 0x24];
        out.extend_from_slice(
            format!(
                "Agent execution terminated due to error.\u{0012}agent executor error: {error}: {error}\n(1) attached stack trace\n  -- stack trace: internal.go:42"
            )
            .as_bytes(),
        );
        out
    }

    fn setup_db(name: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("vlx-agy-outcome-{name}-{}", uuid::Uuid::new_v4()));
        let conv = dir.join("antigravity-cli").join("conversations");
        fs::create_dir_all(&conv).unwrap();
        (dir, conv.join("e6aba774-5472-4277-b6ee-27b2e05570a7.db"))
    }

    fn append_steps(db_path: &Path, next_idx: &mut i64, steps: &[(i64, Vec<u8>)]) {
        let conn = Connection::open(db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS steps (idx INTEGER PRIMARY KEY, step_type INTEGER, step_payload BLOB);",
        )
        .unwrap();
        let mut stmt = conn.prepare("INSERT INTO steps (idx, step_type, step_payload) VALUES (?1, ?2, ?3)").unwrap();
        for (ty, payload) in steps {
            stmt.execute(params![*next_idx, *ty, payload]).unwrap();
            *next_idx += 1;
        }
    }

    #[test]
    fn classifies_quota_rate_limit_gateway_404_signature_and_401_from_steps_db() {
        let (dir, db_path) = setup_db("classify");
        let mut idx = 0;

        // First prompt creates the DB cleanly with a normal model response.
        let watch = PromptOutcomeWatch::capture_at(Some(db_path.clone()));
        append_steps(
            &db_path,
            &mut idx,
            &[
                (USER_INPUT, b"hello".to_vec()),
                (PLANNER_RESPONSE, b"world".to_vec()),
            ],
        );
        assert_eq!(watch.check("end_turn", Some(1_000)), None);

        // Quota exhaustion with fallback from invalid timestamp to quotaResetDelay.
        let quota = error_payload(
            "Error 429, Message: Quota exhausted for test model., Status: RESOURCE_EXHAUSTED, Details: [map[@type:type.googleapis.com/google.rpc.RetryInfo retryDelay:42s] map[@type:type.googleapis.com/google.rpc.ErrorInfo domain:googleapis.com metadata:map[quotaResetDelay:3600s quotaResetTimeStamp:soon] reason:QUOTA_EXHAUSTED]]",
        );
        let watch = PromptOutcomeWatch::capture_at(Some(db_path.clone()));
        append_steps(&db_path, &mut idx, &[(USER_INPUT, b"hi".to_vec()), (ERROR_MESSAGE, quota)]);
        assert_eq!(
            watch.check("end_turn", Some(1_000)),
            Some(HiddenTurnFailure::UsageLimit {
                message: "You have reached your Antigravity quota for this period.".into(),
                reset: Some(Reset { resets_at: 4_600, limit_type: Some("five_hour".into()) }),
            })
        );

        // Earlier error step does not poison a subsequent healthy turn quoting quota text.
        let watch = PromptOutcomeWatch::capture_at(Some(db_path.clone()));
        append_steps(
            &db_path,
            &mut idx,
            &[
                (USER_INPUT, b"quote".to_vec()),
                (PLANNER_RESPONSE, b"You have reached your current quota for this period.".to_vec()),
            ],
        );
        assert_eq!(watch.check("end_turn", Some(2_000)), None);

        // Bad gateway 502 is classified as TransientUpstream and strips Go stack traces.
        let bad_gw = error_payload(
            "Error 502, Message: Bad Gateway: Failed to connect to backend API, Status: 502 Bad Gateway, Details: []",
        );
        let watch = PromptOutcomeWatch::capture_at(Some(db_path.clone()));
        append_steps(&db_path, &mut idx, &[(USER_INPUT, b"hi".to_vec()), (ERROR_MESSAGE, bad_gw)]);
        assert_eq!(
            watch.check("end_turn", Some(2_000)),
            Some(HiddenTurnFailure::TransientUpstream {
                message: "502 Bad Gateway (code 502): Bad Gateway: Failed to connect to backend API".into(),
            })
        );

        // Mid-turn 404 after PLANNER_RESPONSE is TransientUpstream; pre-model 404 is ProviderError.
        let mut status_404 = vec![0x12, 0x37];
        status_404.extend_from_slice(
            b"NOT_FOUND (code 404): Requested entity was not found.\x1a\x10HTTP 404",
        );
        let watch = PromptOutcomeWatch::capture_at(Some(db_path.clone()));
        append_steps(
            &db_path,
            &mut idx,
            &[
                (USER_INPUT, b"hi".to_vec()),
                (PLANNER_RESPONSE, b"step1".to_vec()),
                (TOOL, b"ok".to_vec()),
                (ERROR_MESSAGE, status_404.clone()),
            ],
        );
        assert!(matches!(
            watch.check("end_turn", Some(2_000)),
            Some(HiddenTurnFailure::TransientUpstream { .. })
        ));

        let unentitled_404 = error_payload(
            "Error 404, Message: Model gemini-secret is not found for API version., Status: NOT_FOUND, Details: []",
        );
        let watch = PromptOutcomeWatch::capture_at(Some(db_path.clone()));
        append_steps(&db_path, &mut idx, &[(USER_INPUT, b"hi".to_vec()), (ERROR_MESSAGE, unentitled_404)]);
        assert_eq!(
            watch.check("end_turn", Some(2_000)),
            Some(HiddenTurnFailure::ProviderError {
                message: "Model gemini-secret is not found for API version.".into(),
            })
        );

        // Invalid Claude thinking signature marks conversation broken.
        let sig_err = error_payload(
            r#"Error 400, Message: {"type":"error","error":{"type":"invalid_request_error","message":"messages.3.content.0: Invalid `signature` in `thinking` block"}}, Status: INVALID_ARGUMENT, Details: []"#,
        );
        let watch = PromptOutcomeWatch::capture_at(Some(db_path.clone()));
        append_steps(&db_path, &mut idx, &[(USER_INPUT, b"hi".to_vec()), (ERROR_MESSAGE, sig_err)]);
        assert!(matches!(
            watch.check("end_turn", Some(2_000)),
            Some(HiddenTurnFailure::InvalidThinkingSignature { .. })
        ));

        // Hidden 401 error step is caught as Auth401.
        let err_401 = b"\x12\x40UNAUTHENTICATED (code 401): Request had invalid authentication credentials.".to_vec();
        let watch = PromptOutcomeWatch::capture_at(Some(db_path.clone()));
        append_steps(&db_path, &mut idx, &[(USER_INPUT, b"hi".to_vec()), (ERROR_MESSAGE, err_401)]);
        assert!(matches!(
            watch.check("end_turn", Some(2_000)),
            Some(HiddenTurnFailure::Auth401 { .. })
        ));

        // Typed refusal without DB step still reports UsageLimit.
        let watch = PromptOutcomeWatch::capture_at(Some(db_path.clone()));
        assert!(matches!(
            watch.check("refusal", Some(2_000)),
            Some(HiddenTurnFailure::UsageLimit { .. })
        ));

        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_session_db_while_preserving_typed_refusal() {
        let (dir, db_path) = setup_db("symlink");
        let peer = db_path.with_file_name("peer.db");
        let mut idx = 0;
        append_steps(&peer, &mut idx, &[(USER_INPUT, b"peer".to_vec())]);
        std::os::unix::fs::symlink(&peer, &db_path).unwrap();

        let watch = PromptOutcomeWatch::capture_at(Some(db_path.clone()));
        append_steps(
            &peer,
            &mut idx,
            &[(ERROR_MESSAGE, error_payload("Error 502, Message: Bad Gateway, Status: 502 Bad Gateway, Details: []"))],
        );
        assert_eq!(watch.check("end_turn", Some(1_000)), None);
        assert!(matches!(
            watch.check("refusal", Some(1_000)),
            Some(HiddenTurnFailure::UsageLimit { .. })
        ));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn activity_fingerprint_tracks_step_payload_growth_and_stays_quiet_when_idle() {
        let (dir, db_path) = setup_db("activity");
        let mut idx = 0;
        append_steps(&db_path, &mut idx, &[(USER_INPUT, b"start".to_vec())]);

        let watch = PromptOutcomeWatch::capture_at(Some(db_path.clone()));
        let mut baseline = watch.activity_fingerprint().expect("initial DB fingerprint");

        // Sampling again without writes reports no progress.
        let idle = watch.activity_fingerprint().expect("idle DB fingerprint");
        assert!(!baseline.advance_from(idle));

        // Appending a new running PLANNER_RESPONSE step reports progress.
        append_steps(&db_path, &mut idx, &[(PLANNER_RESPONSE, b"chunk-1".to_vec())]);
        let after_insert = watch.activity_fingerprint().expect("post-insert fingerprint");
        assert!(baseline.advance_from(after_insert));

        // Updating `step_payload` in place on the running step (as `agy` flushes tool-call bytes)
        // reports progress even though `max(idx)` has not changed.
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute(
                "UPDATE steps SET step_payload = ?1 WHERE idx = ?2",
                params![vec![b'x'; 8192], idx - 1],
            )
            .unwrap();
        }
        let after_growth = watch.activity_fingerprint().expect("post-growth fingerprint");
        assert!(baseline.advance_from(after_growth));

        // Subsequent idle sample reports no progress again.
        let idle_again = watch.activity_fingerprint().expect("second idle fingerprint");
        assert!(!baseline.advance_from(idle_again));

        let _ = fs::remove_dir_all(dir);
    }
}
