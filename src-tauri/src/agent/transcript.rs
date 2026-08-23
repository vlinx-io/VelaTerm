//! Parses Claude transcripts and Codex rollouts (JSONL) into user/assistant messages for archive chat viewing.
//! This replaces terminal-byte replay, which is unreadable for full-screen TUIs because of alternate screens and
//! coordinate-based redraws.
//!
//! Verified formats:
//! - **Claude** stores one JSON object per line. User content may be text or blocks; all-tool_result lines are tool
//!   responses, not user speech. Assistant blocks include text/tool_use/thinking. Skip metadata, sidechains, and meta.
//! - **Codex** uses response_item message rows with user/assistant roles and input_text/output_text blocks; function_call
//!   records tools. Filter injected environment_context/user_instructions from user messages.

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::models::SessionKind;

use super::resume;

/// One conversation message after merging adjacent rows of the same role.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptMessage {
    /// "user" | "assistant"
    pub role: String,
    pub text: String,
    /// ISO timestamp from the message's first fragment, possibly empty.
    pub timestamp: Option<String>,
    /// Tool names used in this assistant turn, ordered and deduplicated when adjacent.
    pub tools: Vec<String>,
}

/// Info-panel snapshot of model, context usage, and current tool.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentContextInfo {
    /// Actual model ID from the latest assistant transcript row.
    pub model: Option<String>,
    /// Current context tokens: latest input + cache creation + cache read, representing all context sent.
    pub context_tokens: Option<u64>,
    /// Context limit. A `[1m]` settings suffix explicitly selects 1M; otherwise infer from the transcript model ID,
    /// falling back conservatively to 200k.
    pub context_limit: u64,
    /// Current tool inferred for Codex from the latest call without output. Claude uses live hook state instead.
    pub current_tool: Option<String>,
}

/// Default Claude context window and explicit `[1m]` window.
const CONTEXT_LIMIT_DEFAULT: u64 = 200_000;
const CONTEXT_LIMIT_1M: u64 = 1_000_000;

/// Bytes read from a transcript tail. 512 KB reliably covers a recent usage-bearing assistant row without rereading
/// multi-megabyte histories on every refresh.
const CONTEXT_TAIL_BYTES: u64 = 512 * 1024;

/// Default Grok Build context window when session signals omit an explicit limit.
const GROK_CONTEXT_LIMIT_DEFAULT: u64 = 500_000;

/// Queries model, context usage, and current tool for the Info panel.
pub fn context_info(kind: SessionKind, agent_session_id: &str) -> Result<AgentContextInfo, String> {
    if matches!(kind, SessionKind::Codex) {
        let path =
            resume::find_codex_rollout(agent_session_id).ok_or("Codex rollout file not found")?;
        let tail = read_tail(&path, CONTEXT_TAIL_BYTES)?;
        return Ok(last_codex_context_info(&tail));
    }
    if matches!(kind, SessionKind::Grok) {
        return grok_context_info(agent_session_id);
    }
    if !matches!(kind, SessionKind::Claude) {
        return Err("Only claude, codex, and grok sessions support context info".to_string());
    }
    let path = resume::find_claude_transcript(agent_session_id)
        .ok_or("Claude transcript file not found")?;
    let tail = read_tail(&path, CONTEXT_TAIL_BYTES)?;
    let (model, context_tokens) = last_claude_usage(&tail);
    // Respect explicit `[1m]` first, then infer from the real model ID, then fall back to 200k.
    let settings = claude_settings();
    let force_1m = settings
        .as_ref()
        .and_then(|s| s.get("model"))
        .and_then(Value::as_str)
        .is_some_and(|m| m.contains("[1m]"));
    let context_limit = if force_1m {
        CONTEXT_LIMIT_1M
    } else {
        model
            .as_deref()
            .map_or(CONTEXT_LIMIT_DEFAULT, context_limit_for_model)
    };
    Ok(AgentContextInfo {
        model,
        context_tokens,
        context_limit,
        current_tool: None,
    })
}

/// Reads Grok model/context from the session directory beside `updates.jsonl`.
///
/// Prefer `signals.json` (`contextTokensUsed` / `contextWindowTokens` / `primaryModelId`); fall back to
/// `summary.json` for `current_model_id` when signals are missing. Tool name still comes from live hooks.
fn grok_context_info(agent_session_id: &str) -> Result<AgentContextInfo, String> {
    let updates =
        resume::find_grok_updates(agent_session_id).ok_or("Grok session directory not found")?;
    let dir = updates.parent().ok_or("Grok session directory not found")?;
    let signals = read_json_file(&dir.join("signals.json"));
    let summary = read_json_file(&dir.join("summary.json"));

    let model = signals
        .as_ref()
        .and_then(|v| {
            v.get("primaryModelId")
                .or_else(|| v.get("primary_model_id"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            summary.as_ref().and_then(|v| {
                v.get("current_model_id")
                    .or_else(|| v.get("currentModelId"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
        });

    let context_tokens = signals.as_ref().and_then(|v| {
        v.get("contextTokensUsed")
            .or_else(|| v.get("context_tokens_used"))
            .and_then(json_u64)
    });
    let context_limit = signals
        .as_ref()
        .and_then(|v| {
            v.get("contextWindowTokens")
                .or_else(|| v.get("context_window_tokens"))
                .and_then(json_u64)
        })
        .filter(|&n| n > 0)
        .unwrap_or(GROK_CONTEXT_LIMIT_DEFAULT);

    if model.is_none() && context_tokens.is_none() {
        return Err("Grok session has no model/context signals yet".to_string());
    }
    Ok(AgentContextInfo {
        model,
        context_tokens,
        context_limit,
        current_tool: None,
    })
}

fn read_json_file(path: &Path) -> Option<Value> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn json_u64(v: &Value) -> Option<u64> {
    v.as_u64()
        .or_else(|| v.as_f64().map(|n| n as u64))
        .or_else(|| v.as_i64().map(|n| n.max(0) as u64))
}

/// Extracts Codex model, latest-request input tokens, context window, and current tool from the rollout tail.
/// `total_token_usage` is cumulative and not context size; use `last_token_usage.input_tokens` instead.
fn last_codex_context_info(text: &str) -> AgentContextInfo {
    let mut model = None;
    let mut context_tokens = None;
    let mut context_limit = 0;
    let mut current_tool = None;

    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match v.get("type").and_then(Value::as_str) {
            Some("session_meta") => {
                if let Some(limit) = v
                    .get("payload")
                    .and_then(|p| p.get("context_window"))
                    .and_then(Value::as_u64)
                {
                    context_limit = limit;
                }
            }
            Some("turn_context") => {
                if let Some(m) = v
                    .get("payload")
                    .and_then(|p| p.get("model"))
                    .and_then(Value::as_str)
                {
                    model = Some(m.to_string());
                }
            }
            Some("event_msg") => {
                let Some(payload) = v.get("payload") else {
                    continue;
                };
                match payload.get("type").and_then(Value::as_str) {
                    Some("token_count") => {
                        if let Some(info) = payload.get("info") {
                            context_tokens = info
                                .get("last_token_usage")
                                .and_then(|u| u.get("input_tokens"))
                                .and_then(Value::as_u64)
                                .or(context_tokens);
                            context_limit = info
                                .get("model_context_window")
                                .and_then(Value::as_u64)
                                .unwrap_or(context_limit);
                        }
                    }
                    Some("task_complete") => current_tool = None,
                    _ => {}
                }
            }
            Some("response_item") => {
                let Some(payload) = v.get("payload") else {
                    continue;
                };
                match payload.get("type").and_then(Value::as_str) {
                    Some("custom_tool_call") | Some("function_call") => {
                        current_tool = payload
                            .get("name")
                            .and_then(Value::as_str)
                            .map(str::to_string);
                    }
                    Some("local_shell_call") => current_tool = Some("shell".to_string()),
                    Some("web_search_call") => current_tool = Some("web_search".to_string()),
                    Some("custom_tool_call_output")
                    | Some("function_call_output")
                    | Some("local_shell_call_output")
                    | Some("web_search_call_output") => {
                        current_tool = None;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    AgentContextInfo {
        model,
        context_tokens,
        context_limit,
        current_tool,
    }
}

/// Infers context tokens from a real model ID using official max_input_tokens. `contains` handles dated suffixes.
/// Unknown models conservatively use 200k rather than overstating capacity; extend the table for new models.
fn context_limit_for_model(model: &str) -> u64 {
    // Known 1M models; Haiku 4.5, older unspecified models, and others use the 200k fallback.
    const ONE_M_MODELS: &[&str] = &[
        "opus-4-8",
        "opus-4-7",
        "opus-4-6",
        "sonnet-4-6",
        "fable-5",
        "mythos-5",
    ];
    if ONE_M_MODELS.iter().any(|needle| model.contains(needle)) {
        CONTEXT_LIMIT_1M
    } else {
        CONTEXT_LIMIT_DEFAULT
    }
}

/// Claude-only current-turn Info stats after the final genuine user message: output tokens, tool calls, and distinct changed files.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TurnStats {
    /// Sum of assistant `output_tokens` generated during this turn.
    pub tokens: u64,
    /// Number of tool_use blocks in this turn.
    pub tools_used: u32,
    /// Distinct paths changed by file-writing tools during this turn.
    pub files_touched: u32,
}

/// File-writing tools counted toward changed files.
fn is_edit_tool(name: &str) -> bool {
    matches!(
        name,
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "Update"
    )
}

/// Extracts the changed file path from a file-writing tool_use input.
fn tool_file_path(input: &Value) -> Option<String> {
    for key in ["file_path", "notebook_path", "path"] {
        if let Some(p) = input.get(key).and_then(Value::as_str) {
            let p = p.trim();
            if !p.is_empty() {
                return Some(p.to_string());
            }
        }
    }
    None
}

/// Computes current-turn Claude stats after the final genuine user message. Missing transcripts error. Because only
/// the last 512 KB is read, an exceptionally large single turn may be counted from a truncated starting point.
pub fn current_turn_stats(kind: SessionKind, agent_session_id: &str) -> Result<TurnStats, String> {
    if !matches!(kind, SessionKind::Claude) {
        return Err("Only claude sessions support per-turn stats".to_string());
    }
    let path = resume::find_claude_transcript(agent_session_id)
        .ok_or("Claude transcript file not found")?;
    let tail = read_tail(&path, CONTEXT_TAIL_BYTES)?;
    Ok(turn_stats_from_text(&tail))
}

/// Computes current-turn stats from transcript text, isolated for direct unit testing.
fn turn_stats_from_text(text: &str) -> TurnStats {
    let lines: Vec<&str> = text.lines().collect();

    // Start after the final genuine user message; parser excludes tool responses, metadata, and sidechains.
    let mut start = 0usize;
    for (i, line) in lines.iter().enumerate() {
        if parse_claude_line(line)
            .map(|p| p.role == "user")
            .unwrap_or(false)
        {
            start = i + 1;
        }
    }

    let mut tokens = 0u64;
    let mut tools_used = 0u32;
    let mut files: HashSet<String> = HashSet::new();
    for line in &lines[start..] {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        if v.get("isSidechain")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let Some(msg) = v.get("message") else {
            continue;
        };
        if msg.get("model").and_then(Value::as_str) == Some("<synthetic>") {
            continue;
        }
        if let Some(usage) = msg.get("usage") {
            tokens += usage
                .get("output_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0);
        }
        if let Some(items) = msg.get("content").and_then(Value::as_array) {
            for it in items {
                if it.get("type").and_then(Value::as_str) == Some("tool_use") {
                    tools_used += 1;
                    let name = it.get("name").and_then(Value::as_str).unwrap_or("");
                    if is_edit_tool(name) {
                        if let Some(fp) = it.get("input").and_then(tool_file_path) {
                            files.insert(fp);
                        }
                    }
                }
            }
        }
    }

    TurnStats {
        tokens,
        tools_used,
        files_touched: files.len() as u32,
    }
}

/// Reads at most `cap` bytes from a file tail, discarding the leading partial line when starting mid-file.
fn read_tail(path: &Path, cap: u64) -> Result<String, String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).map_err(|e| format!("Failed to open transcript: {e}"))?;
    let len = f
        .metadata()
        .map_err(|e| format!("Failed to read transcript metadata: {e}"))?
        .len();
    let start = len.saturating_sub(cap);
    f.seek(SeekFrom::Start(start))
        .map_err(|e| format!("Failed to seek transcript: {e}"))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)
        .map_err(|e| format!("Failed to read transcript: {e}"))?;
    let mut s = String::from_utf8_lossy(&buf).into_owned();
    if start > 0 {
        match s.find('\n') {
            Some(i) => {
                s.drain(..=i);
            }
            None => s.clear(),
        }
    }
    Ok(s)
}

/// Extracts model and context tokens from the **last** main-chain assistant row, skipping sidechain usage and
/// synthetic API-error rows without real usage.
fn last_claude_usage(text: &str) -> (Option<String>, Option<u64>) {
    let mut model = None;
    let mut tokens = None;
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        if v.get("isSidechain")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            continue;
        }
        let Some(msg) = v.get("message") else {
            continue;
        };
        let m = msg.get("model").and_then(Value::as_str);
        if m == Some("<synthetic>") {
            continue;
        }
        let Some(usage) = msg.get("usage") else {
            continue;
        };
        let sum: u64 = [
            "input_tokens",
            "cache_creation_input_tokens",
            "cache_read_input_tokens",
        ]
        .iter()
        .filter_map(|k| usage.get(k).and_then(Value::as_u64))
        .sum();
        if let Some(m) = m {
            model = Some(m.to_string());
        }
        tokens = Some(sum);
    }
    (model, tokens)
}

/// Reads the model name from `~/.claude/settings.json` for context limits; returns None when unreadable.
fn claude_settings() -> Option<Value> {
    let home = crate::host::home_dir()?;
    let p = home.join(".claude").join("settings.json");
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

/// One Codex account rate-limit window for Info. `window_minutes` identifies duration for labels; `resets_at` is a
/// Unix timestamp, unlike Claude's ISO strings, and is formatted separately by the frontend.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexRateWindow {
    /// Used percentage from 0 to 100.
    pub used_percent: f64,
    /// Window duration in minutes.
    pub window_minutes: u64,
    /// Reset time in Unix seconds, or None.
    pub resets_at: Option<i64>,
}

/// Codex account rate-limit snapshot read from the latest token_count event in the rollout tail.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexUsage {
    /// Primary short window, usually five hours.
    pub primary: Option<CodexRateWindow>,
    /// Secondary long window, usually weekly and absent for some plans.
    pub secondary: Option<CodexRateWindow>,
    /// Plan type such as free/plus/pro, passed through for optional display.
    pub plan_type: Option<String>,
}

/// Queries account rate limits from the latest Codex token_count rollout event. This is local and unthrottled.
/// Missing rollout/snapshot errors so the frontend hides the section.
pub fn codex_rate_limits(kind: SessionKind, agent_session_id: &str) -> Result<CodexUsage, String> {
    if !matches!(kind, SessionKind::Codex) {
        return Err("Only codex sessions support rate limits".to_string());
    }
    let path =
        resume::find_codex_rollout(agent_session_id).ok_or("Codex rollout file not found")?;
    let tail = read_tail(&path, CONTEXT_TAIL_BYTES)?;
    last_codex_rate_limits(&tail)
        .ok_or_else(|| "No rate_limits snapshot in codex rollout".to_string())
}

/// Actively reads current account limits through the installed Codex CLI app-server.
///
/// Rollout rate limits are historical snapshots written only during model turns and cannot refresh by rereading.
/// Start a short-lived stdio app-server, initialize, query account/rateLimits/read, then close. Unsupported old CLIs,
/// logged-out state, or network failure return Err for fallback to rollout.
pub fn live_codex_rate_limits(bin_path: Option<&str>) -> Result<CodexUsage, String> {
    let mut server = super::codex_app_server::CodexAppServer::start(bin_path, &[])?;
    let response = server.request(
        "account/rateLimits/read",
        Value::Null,
        Duration::from_secs(12),
    )?;
    parse_live_codex_rate_limits(&response)
        .ok_or_else(|| "Codex app-server returned no primary rate-limit window".to_string())
}

/// Maps app-server camelCase and rollout snake_case into the existing Info shape. Selects the `codex` bucket from
/// multi-bucket responses to avoid displaying separate products such as Spark.
fn parse_live_codex_rate_limits(result: &Value) -> Option<CodexUsage> {
    let snapshot = result
        .get("rateLimitsByLimitId")
        .and_then(|buckets| buckets.get("codex"))
        .or_else(|| result.get("rateLimits"))?;
    let parse_window = |key: &str| {
        let w = snapshot.get(key)?;
        if w.is_null() {
            return None;
        }
        Some(CodexRateWindow {
            used_percent: w.get("usedPercent")?.as_f64()?,
            window_minutes: w
                .get("windowDurationMins")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            resets_at: w.get("resetsAt").and_then(Value::as_i64),
        })
    };
    let usage = CodexUsage {
        primary: parse_window("primary"),
        secondary: parse_window("secondary"),
        plan_type: snapshot
            .get("planType")
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    (usage.primary.is_some() || usage.secondary.is_some()).then_some(usage)
}

/// Reads the newest Codex rollout containing rate limits. Because usage is account-level, this keeps Info available
/// before a VelaTerm session captures its own Codex ID.
pub fn latest_codex_rate_limits() -> Result<CodexUsage, String> {
    for path in resume::codex_rollout_paths_newest_first() {
        let Ok(tail) = read_tail(&path, CONTEXT_TAIL_BYTES) else {
            continue;
        };
        if let Some(usage) = last_codex_rate_limits(&tail) {
            return Ok(usage);
        }
    }
    Err("No rate_limits snapshot in recent codex rollouts".to_string())
}

/// Parses one primary/secondary rate-limit window, returning None when null or missing required fields.
fn parse_codex_window(w: &Value) -> Option<CodexRateWindow> {
    if w.is_null() {
        return None;
    }
    Some(CodexRateWindow {
        used_percent: w.get("used_percent").and_then(Value::as_f64)?,
        window_minutes: w.get("window_minutes").and_then(Value::as_u64).unwrap_or(0),
        resets_at: w.get("resets_at").and_then(Value::as_i64),
    })
}

/// Extracts rate limits from the **last token_count event containing actual window data**.
///
/// Do not blindly take the last event: after primary quota is exhausted, Codex may write a `premium` snapshot with
/// both windows null, hiding the preceding real 100% snapshot. Update only when at least one window is non-null.
fn last_codex_rate_limits(text: &str) -> Option<CodexUsage> {
    let mut found = None;
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("type").and_then(Value::as_str) != Some("event_msg") {
            continue;
        }
        let Some(payload) = v.get("payload") else {
            continue;
        };
        if payload.get("type").and_then(Value::as_str) != Some("token_count") {
            continue;
        }
        let Some(rl) = payload
            .get("rate_limits")
            .or_else(|| payload.get("info").and_then(|i| i.get("rate_limits")))
        else {
            continue;
        };
        if rl.is_null() {
            continue;
        }
        let usage = CodexUsage {
            primary: rl.get("primary").and_then(parse_codex_window),
            secondary: rl.get("secondary").and_then(parse_codex_window),
            plan_type: rl
                .get("plan_type")
                .and_then(Value::as_str)
                .map(str::to_string),
        };
        // Accept only snapshots with window data; skip the post-exhaustion empty premium snapshot.
        if usage.primary.is_some() || usage.secondary.is_some() {
            found = Some(usage);
        }
    }
    found
}

/// One parsed row, the smallest unit before merging.
struct Piece {
    role: &'static str,
    text: String,
    timestamp: Option<String>,
    tools: Vec<String>,
}

/// Locates a session's Claude transcript or Codex rollout. Returns None when absent or unsupported. Global search
/// stats this source path by mtime/length to detect changes.
pub fn source_path(kind: SessionKind, agent_session_id: &str) -> Option<std::path::PathBuf> {
    match kind {
        SessionKind::Claude => resume::find_claude_transcript(agent_session_id),
        SessionKind::Codex => resume::find_codex_rollout(agent_session_id),
        SessionKind::Grok => resume::find_grok_updates(agent_session_id),
        // OpenCode/Copilot/Cursor lack flat parseable files; terminal/browser nodes have no conversation.
        _ => None,
    }
}

/// Reads and parses an agent transcript by kind. Missing/deleted/unsupported sources return Err so the frontend can
/// fall back to raw recording playback.
pub fn read(kind: SessionKind, agent_session_id: &str) -> Result<Vec<TranscriptMessage>, String> {
    match kind {
        SessionKind::Claude => {
            let path = resume::find_claude_transcript(agent_session_id)
                .ok_or("Claude transcript file not found")?;
            parse_file(&path, parse_claude_line)
        }
        SessionKind::Codex => {
            let path = resume::find_codex_rollout(agent_session_id)
                .ok_or("Codex rollout file not found")?;
            parse_file(&path, parse_codex_line)
        }
        SessionKind::Opencode => {
            Err("Transcript view is not supported for opencode sessions yet".to_string())
        }
        // Copilot stores internal state under ~/.copilot rather than a flat parseable transcript.
        SessionKind::Copilot => {
            Err("Transcript view is not supported for copilot sessions yet".to_string())
        }
        // Cursor stores chats in SQLite blobs at ~/.cursor/chats/*/<id>/store.db, not flat JSONL.
        SessionKind::Cursor => {
            Err("Transcript view is not supported for cursor sessions yet".to_string())
        }
        // Antigravity transcript_path JSON logs are not integrated yet.
        SessionKind::Antigravity => {
            Err("Transcript view is not supported for antigravity sessions yet".to_string())
        }
        // Cline stores session data in internal SQLite under ~/.cline/data/sessions.
        SessionKind::Cline => {
            Err("Transcript view is not supported for cline sessions yet".to_string())
        }
        // Pi uses flat JSONL with a header and tree entries, but its parser is not integrated; archives use recording playback.
        SessionKind::Pi => Err("Transcript view is not supported for pi sessions yet".to_string()),
        // Crush uses an internal database under ~/.local/share/crush; archives fall back to recording playback.
        SessionKind::Crush => {
            Err("Transcript view is not supported for crush sessions yet".to_string())
        }
        SessionKind::Kimi => {
            Err("Transcript view is not supported for kimi sessions yet".to_string())
        }
        SessionKind::Kiro => {
            Err("Transcript view is not supported for kiro sessions yet".to_string())
        }
        SessionKind::Grok => {
            let path = resume::find_grok_updates(agent_session_id)
                .ok_or("Grok transcript file not found")?;
            parse_grok_file(&path)
        }
        SessionKind::Zoo => {
            Err("Transcript view is not supported for Zoo Code sessions yet".to_string())
        }
        SessionKind::Terminal => Err("Terminal sessions have no agent transcript".to_string()),
        SessionKind::Browser => Err("Browser pages have no agent transcript".to_string()),
    }
}

/// Parses an entire JSONL file line-by-line and merges adjacent fragments of the same role.
fn parse_file(
    path: &Path,
    parse_line: impl Fn(&str) -> Option<Piece>,
) -> Result<Vec<TranscriptMessage>, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read transcript: {e}"))?;
    let pieces = content.lines().filter_map(|l| parse_line(l));
    Ok(merge(pieces))
}

/// Parse Grok's authoritative ACP `updates.jsonl` stream.
///
/// Message rows are streaming chunks, so concatenate them directly instead of using the blank-line
/// fragment merging used by Claude/Codex. Tool results and internal thought chunks are intentionally
/// omitted; tool-call names are attached to the surrounding assistant turn.
fn parse_grok_file(path: &Path) -> Result<Vec<TranscriptMessage>, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read transcript: {e}"))?;
    Ok(parse_grok_updates(&content))
}

fn parse_grok_updates(content: &str) -> Vec<TranscriptMessage> {
    let mut out: Vec<TranscriptMessage> = Vec::new();
    for line in content.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if value.get("method").and_then(Value::as_str) != Some("session/update") {
            continue;
        }
        let Some(update) = value.get("params").and_then(|p| p.get("update")) else {
            continue;
        };
        let timestamp = grok_timestamp(&value);
        match update.get("sessionUpdate").and_then(Value::as_str) {
            Some("user_message_chunk") => {
                let Some(text) = update
                    .get("content")
                    .and_then(|c| c.get("text"))
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                append_grok_text(&mut out, "user", text, timestamp);
            }
            Some("agent_message_chunk") => {
                let Some(text) = update
                    .get("content")
                    .and_then(|c| c.get("text"))
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                append_grok_text(&mut out, "assistant", text, timestamp);
            }
            Some("tool_call") => {
                let name = update
                    .get("_meta")
                    .and_then(|m| m.get("x.ai/tool"))
                    .and_then(|t| t.get("name"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .unwrap_or("tool")
                    .to_string();
                if out.last().is_none_or(|message| message.role != "assistant") {
                    out.push(TranscriptMessage {
                        role: "assistant".to_string(),
                        text: String::new(),
                        timestamp,
                        tools: Vec::new(),
                    });
                }
                let message = out.last_mut().expect("assistant row was just ensured");
                if message.tools.last() != Some(&name) {
                    message.tools.push(name);
                }
            }
            // agent_thought_chunk is private reasoning; tool_call_update is tool output/status.
            _ => {}
        }
    }
    out.retain(|message| !message.text.trim().is_empty() || !message.tools.is_empty());
    out
}

fn append_grok_text(
    out: &mut Vec<TranscriptMessage>,
    role: &str,
    text: &str,
    timestamp: Option<String>,
) {
    if text.is_empty() {
        return;
    }
    if let Some(message) = out.last_mut().filter(|message| message.role == role) {
        message.text.push_str(text);
        return;
    }
    out.push(TranscriptMessage {
        role: role.to_string(),
        text: text.to_string(),
        timestamp,
        tools: Vec::new(),
    });
}

/// Grok writes Unix seconds at the row root. Convert it to the ISO shape consumed by the viewer.
fn grok_timestamp(value: &Value) -> Option<String> {
    let seconds = value.get("timestamp")?.as_i64()?;
    let dt = time::OffsetDateTime::from_unix_timestamp(seconds).ok()?;
    Some(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        dt.year(),
        u8::from(dt.month()),
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second()
    ))
}

/// Merges adjacent same-role fragments into one message, joining text with blank lines and appending ordered tools
/// with adjacent deduplication. Claude commonly spreads one response across many text/tool_use rows.
fn merge(pieces: impl Iterator<Item = Piece>) -> Vec<TranscriptMessage> {
    let mut out: Vec<TranscriptMessage> = Vec::new();
    for p in pieces {
        if let Some(last) = out.last_mut() {
            if last.role == p.role {
                if !p.text.is_empty() {
                    if !last.text.is_empty() {
                        last.text.push_str("\n\n");
                    }
                    last.text.push_str(&p.text);
                }
                for t in p.tools {
                    if last.tools.last() != Some(&t) {
                        last.tools.push(t);
                    }
                }
                continue;
            }
        }
        out.push(TranscriptMessage {
            role: p.role.to_string(),
            text: p.text,
            timestamp: p.timestamp,
            tools: p.tools,
        });
    }
    // Preserve tool-only messages interrupted after function calls; the tool list remains meaningful.
    out
}

/// Claude-injected inter-turn context begins with known XML tags but may lack isMeta. It is not genuine user input
/// and must not establish a turn boundary; export.rs filters it too.
pub(super) fn is_injected_context(text: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "<command-name>",
        "<command-message>",
        "<command-args>",
        "<local-command-stdout>",
        "<local-command-caveat>",
        "<system-reminder>",
        "<user-prompt-submit-hook>",
        "<environment_details>",
        "<context>",
    ];
    PREFIXES.iter().any(|p| text.starts_with(p))
}

/// Parses one Claude transcript line, returning None for irrelevant rows.
fn parse_claude_line(line: &str) -> Option<Piece> {
    let v: Value = serde_json::from_str(line).ok()?;
    // Sidechain and injected-meta messages are not part of the main conversation.
    if v.get("isSidechain")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || v.get("isMeta").and_then(Value::as_bool).unwrap_or(false)
    {
        return None;
    }
    let ts = v
        .get("timestamp")
        .and_then(Value::as_str)
        .map(str::to_string);
    let msg = v.get("message")?;
    match v.get("type").and_then(Value::as_str)? {
        "user" => {
            let text = match msg.get("content")? {
                Value::String(s) => s.clone(),
                Value::Array(items) => {
                    let mut parts: Vec<String> = Vec::new();
                    for it in items {
                        match it.get("type").and_then(Value::as_str) {
                            Some("text") => {
                                if let Some(s) = it.get("text").and_then(Value::as_str) {
                                    parts.push(s.to_string());
                                }
                            }
                            Some("image") => parts.push("[image]".to_string()),
                            // A tool-result row is not user speech; skip it entirely.
                            Some("tool_result") => return None,
                            _ => {}
                        }
                    }
                    parts.join("\n")
                }
                _ => return None,
            };
            let text = text.trim().to_string();
            if text.is_empty() {
                return None;
            }
            // Inter-turn context such as local-command output or system reminders may lack isMeta but starts with
            // XML. Treating it as a genuine user message would incorrectly reset current-turn statistics.
            if is_injected_context(&text) {
                return None;
            }
            Some(Piece {
                role: "user",
                text,
                timestamp: ts,
                tools: Vec::new(),
            })
        }
        "assistant" => {
            let items = msg.get("content")?.as_array()?;
            let mut parts: Vec<String> = Vec::new();
            let mut tools: Vec<String> = Vec::new();
            for it in items {
                match it.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(s) = it.get("text").and_then(Value::as_str) {
                            parts.push(s.to_string());
                        }
                    }
                    Some("tool_use") => {
                        if let Some(name) = it.get("name").and_then(Value::as_str) {
                            tools.push(name.to_string());
                        }
                    }
                    // Omit internal thinking; the reading view shows only communicated content.
                    _ => {}
                }
            }
            let text = parts.join("\n\n").trim().to_string();
            if text.is_empty() && tools.is_empty() {
                return None;
            }
            Some(Piece {
                role: "assistant",
                text,
                timestamp: ts,
                tools,
            })
        }
        _ => None,
    }
}

/// Parses one Codex rollout line, returning None for irrelevant rows.
fn parse_codex_line(line: &str) -> Option<Piece> {
    let v: Value = serde_json::from_str(line).ok()?;
    if v.get("type").and_then(Value::as_str) != Some("response_item") {
        return None;
    }
    let ts = v
        .get("timestamp")
        .and_then(Value::as_str)
        .map(str::to_string);
    let payload = v.get("payload")?;
    match payload.get("type").and_then(Value::as_str)? {
        "message" => {
            let role = match payload.get("role").and_then(Value::as_str)? {
                "user" => "user",
                "assistant" => "assistant",
                _ => return None,
            };
            let items = payload.get("content")?.as_array()?;
            let mut parts: Vec<String> = Vec::new();
            for it in items {
                if matches!(
                    it.get("type").and_then(Value::as_str),
                    Some("input_text") | Some("output_text")
                ) {
                    if let Some(s) = it.get("text").and_then(Value::as_str) {
                        parts.push(s.to_string());
                    }
                }
            }
            let text = parts.join("\n").trim().to_string();
            // Codex-injected environment/instruction blocks are noise, not user speech.
            if text.is_empty()
                || text.starts_with("<environment_context>")
                || text.starts_with("<user_instructions>")
            {
                return None;
            }
            Some(Piece {
                role,
                text,
                timestamp: ts,
                tools: Vec::new(),
            })
        }
        // Record function-call names; message rows supply text content.
        "function_call" | "custom_tool_call" => {
            let name = payload.get("name").and_then(Value::as_str)?.to_string();
            Some(Piece {
                role: "assistant",
                text: String::new(),
                timestamp: ts,
                tools: vec![name],
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Claude merges user strings and multi-row assistant text/tool_use into messages while skipping tool results,
    /// sidechains, and attachments.
    #[test]
    fn claude_lines_parse_and_merge() {
        let lines = [
            r#"{"type":"mode","mode":"normal"}"#,
            r#"{"type":"user","message":{"role":"user","content":"help me fix a bug"},"timestamp":"2026-06-08T09:01:47.010Z"}"#,
            r#"{"type":"attachment","attachment":{}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Let me take a look."}]},"timestamp":"t2"}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash","input":{}}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"ok"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Fixed it."},{"type":"tool_use","name":"Edit","input":{}}]}}"#,
            r#"{"type":"user","isSidechain":true,"message":{"role":"user","content":"subagent internals"}}"#,
        ];
        let msgs = merge(lines.iter().filter_map(|l| parse_claude_line(l)));
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[0].text, "help me fix a bug");
        assert_eq!(
            msgs[0].timestamp.as_deref(),
            Some("2026-06-08T09:01:47.010Z")
        );
        // Merge three assistant rows and skip the intervening tool-result row.
        assert_eq!(msgs[1].role, "assistant");
        assert_eq!(msgs[1].text, "Let me take a look.\n\nFixed it.");
        assert_eq!(msgs[1].tools, vec!["Bash", "Edit"]);
    }

    /// Codex filters environment context and merges function-call tool names into the assistant turn.
    #[test]
    fn codex_lines_parse_and_merge() {
        let lines = [
            r#"{"timestamp":"t0","type":"session_meta","payload":{"id":"x","cwd":"/p"}}"#,
            r#"{"timestamp":"t1","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>\n  <cwd>/p</cwd>\n</environment_context>"}]}}"#,
            r#"{"timestamp":"t2","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"check the database configuration"}]}}"#,
            r#"{"timestamp":"t3","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Let me get a feel for the project layout first."}],"phase":"commentary"}}"#,
            r#"{"timestamp":"t4","type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{}"}}"#,
            r#"{"timestamp":"t5","type":"response_item","payload":{"type":"function_call_output","output":"..."}}"#,
            r#"{"timestamp":"t6","type":"response_item","payload":{"type":"reasoning","summary":[]}}"#,
            r#"{"timestamp":"t7","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"The configuration lives in application.yml."}]}}"#,
        ];
        let msgs = merge(lines.iter().filter_map(|l| parse_codex_line(l)));
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[0].text, "check the database configuration");
        assert_eq!(msgs[1].role, "assistant");
        assert_eq!(
            msgs[1].text,
            "Let me get a feel for the project layout first.\n\nThe configuration lives in application.yml."
        );
        assert_eq!(msgs[1].tools, vec!["shell"]);
    }

    #[test]
    fn grok_updates_join_streaming_chunks_and_attach_tools() {
        let lines = [
            r#"{"timestamp":1785204468,"method":"session/update","params":{"sessionId":"g1","update":{"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"fix the login"}}}}"#,
            r#"{"timestamp":1785204469,"method":"session/update","params":{"sessionId":"g1","update":{"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"private reasoning"}}}}"#,
            r#"{"timestamp":1785204470,"method":"session/update","params":{"sessionId":"g1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Let me "}}}}"#,
            r#"{"timestamp":1785204470,"method":"session/update","params":{"sessionId":"g1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"check."}}}}"#,
            r#"{"timestamp":1785204471,"method":"session/update","params":{"sessionId":"g1","update":{"sessionUpdate":"tool_call","toolCallId":"t1","_meta":{"x.ai/tool":{"name":"read_file"}}}}}"#,
            r#"{"timestamp":1785204472,"method":"session/update","params":{"sessionId":"g1","update":{"sessionUpdate":"tool_call_update","toolCallId":"t1","content":[]}}}"#,
            r#"{"timestamp":1785204473,"method":"session/update","params":{"sessionId":"g1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Already fixed."}}}}"#,
        ]
        .join("\n");
        let messages = parse_grok_updates(&lines);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].text, "fix the login");
        assert_eq!(
            messages[0].timestamp.as_deref(),
            Some("2026-07-28T02:07:48Z")
        );
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].text, "Let me check.Already fixed.");
        assert_eq!(messages[1].tools, vec!["read_file"]);
        assert!(!messages
            .iter()
            .any(|m| m.text.contains("private reasoning")));
    }

    #[test]
    fn grok_tool_only_turn_is_preserved() {
        let line = r#"{"timestamp":1785204471,"method":"session/update","params":{"update":{"sessionUpdate":"tool_call","_meta":{"x.ai/tool":{"name":"run_terminal_command"}}}}}"#;
        let messages = parse_grok_updates(line);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "assistant");
        assert!(messages[0].text.is_empty());
        assert_eq!(messages[0].tools, vec!["run_terminal_command"]);
    }

    #[test]
    fn codex_rate_limits_parse_payload_and_legacy_info_locations() {
        let text = [
            r#"{"type":"event_msg","payload":{"type":"token_count","info":{},"rate_limits":{"primary":{"used_percent":12.0,"window_minutes":300,"resets_at":111},"secondary":null,"plan_type":"plus"}}}"#,
            r#"{"type":"event_msg","payload":{"type":"token_count","info":{"rate_limits":{"primary":{"used_percent":34.0,"window_minutes":10080,"resets_at":222},"secondary":{"used_percent":5.0,"window_minutes":43200,"resets_at":333},"plan_type":"pro"}}}}"#,
        ]
        .join("\n");
        let usage = last_codex_rate_limits(&text).unwrap();
        assert_eq!(usage.primary.as_ref().unwrap().used_percent, 34.0);
        assert_eq!(usage.primary.as_ref().unwrap().window_minutes, 10080);
        assert_eq!(usage.primary.as_ref().unwrap().resets_at, Some(222));
        assert_eq!(usage.secondary.as_ref().unwrap().used_percent, 5.0);
        assert_eq!(usage.plan_type.as_deref(), Some("pro"));
    }

    /// Regression: after quota exhaustion Codex writes an empty premium snapshot. Skip it and preserve the preceding
    /// real 100% snapshot so the panel does not show only a dash.
    #[test]
    fn codex_rate_limits_skip_trailing_premium_empty_snapshot() {
        let text = [
            r#"{"type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex","primary":{"used_percent":94.0,"window_minutes":43200,"resets_at":1784444950},"secondary":null,"plan_type":"free"}}}"#,
            r#"{"type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"codex","primary":{"used_percent":100.0,"window_minutes":43200,"resets_at":1784444950},"secondary":null,"plan_type":"free"}}}"#,
            r#"{"type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"premium","limit_name":null,"primary":null,"secondary":null,"plan_type":"free"}}}"#,
        ]
        .join("\n");
        let usage = last_codex_rate_limits(&text).expect("the earlier real snapshot should be kept rather than the empty one at the tail");
        let primary = usage.primary.as_ref().expect("primary must not be cleared by an empty snapshot");
        assert_eq!(primary.used_percent, 100.0);
        assert_eq!(primary.window_minutes, 43200);
        assert_eq!(primary.resets_at, Some(1784444950));
    }

    /// All-empty snapshots return None so higher layers can scan older rollouts through latest_codex_rate_limits.
    #[test]
    fn codex_rate_limits_all_empty_returns_none() {
        let text = r#"{"type":"event_msg","payload":{"type":"token_count","rate_limits":{"limit_id":"premium","primary":null,"secondary":null,"plan_type":"free"}}}"#;
        assert!(last_codex_rate_limits(text).is_none());
    }

    #[test]
    fn codex_context_info_uses_latest_turn_and_last_request_tokens() {
        let text = [
            r#"{"type":"session_meta","payload":{"context_window":200000}}"#,
            r#"{"type":"turn_context","payload":{"model":"gpt-old"}}"#,
            r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":90000},"last_token_usage":{"input_tokens":12345},"model_context_window":258400}}}"#,
            r#"{"type":"turn_context","payload":{"model":"gpt-5.6"}}"#,
            r#"{"type":"response_item","payload":{"type":"custom_tool_call","name":"exec"}}"#,
        ]
        .join("\n");
        let info = last_codex_context_info(&text);
        assert_eq!(info.model.as_deref(), Some("gpt-5.6"));
        assert_eq!(info.context_tokens, Some(12345));
        assert_eq!(info.context_limit, 258400);
        assert_eq!(info.current_tool.as_deref(), Some("exec"));
    }

    #[test]
    fn codex_context_info_clears_tool_after_output() {
        let text = [
            r#"{"type":"response_item","payload":{"type":"function_call","name":"wait"}}"#,
            r#"{"type":"response_item","payload":{"type":"function_call_output","output":"done"}}"#,
        ]
        .join("\n");
        assert_eq!(last_codex_context_info(&text).current_tool, None);
    }

    /// Ignored read-only smoke test parsing the newest real local Claude transcript and printing sample messages.
    /// `cargo test --lib -- --ignored claude_transcript_real_file_smoke --nocapture`
    #[test]
    #[ignore = "depends on the real ~/.claude/projects history on this machine"]
    fn claude_transcript_real_file_smoke() {
        let projects = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .expect("HOME")
            .join(".claude/projects");
        // Select the most recently modified transcript.
        let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = Vec::new();
        for dir in std::fs::read_dir(&projects).expect("reading projects").flatten() {
            let Ok(entries) = std::fs::read_dir(dir.path()) else {
                continue;
            };
            for f in entries.flatten() {
                let p = f.path();
                if p.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                    if let Ok(m) = p.metadata().and_then(|m| m.modified()) {
                        files.push((m, p));
                    }
                }
            }
        }
        files.sort_by_key(|f| std::cmp::Reverse(f.0));
        let path = &files.first().expect("there should be at least one transcript").1;
        eprintln!("parsing: {}", path.display());
        let msgs = parse_file(path, parse_claude_line).expect("parsing failed");
        eprintln!("{} messages in total", msgs.len());
        for m in msgs.iter().take(6) {
            let text: String = m.text.chars().take(120).collect();
            eprintln!(
                "[{}] ({:?}) tool {:?}\n  {}",
                m.role, m.timestamp, m.tools, text
            );
        }
        assert!(!msgs.is_empty(), "a real transcript should parse into messages");
        assert!(msgs.iter().any(|m| m.role == "user"));
        assert!(msgs.iter().any(|m| m.role == "assistant"));
    }

    /// Uses the final main-chain assistant model and usage while skipping sidechain and synthetic rows.
    #[test]
    fn last_claude_usage_picks_last_main_chain() {
        let text = [
            r#"{"type":"user","message":{"content":"hi"}}"#,
            r#"{"type":"assistant","message":{"model":"claude-fable-5","usage":{"input_tokens":100,"cache_creation_input_tokens":200,"cache_read_input_tokens":300,"output_tokens":50},"content":[]}}"#,
            // Sidechain usage belongs to the subagent, not the main chain.
            r#"{"type":"assistant","isSidechain":true,"message":{"model":"claude-haiku-4-5","usage":{"input_tokens":9,"cache_read_input_tokens":1},"content":[]}}"#,
            // Skip synthetic API-error rows.
            r#"{"type":"assistant","message":{"model":"<synthetic>","content":[]}}"#,
            r#"{"type":"assistant","message":{"model":"claude-fable-5","usage":{"input_tokens":10,"cache_creation_input_tokens":20,"cache_read_input_tokens":1000},"content":[]}}"#,
            "not json",
        ]
        .join("\n");
        let (model, tokens) = last_claude_usage(&text);
        assert_eq!(model.as_deref(), Some("claude-fable-5"));
        assert_eq!(tokens, Some(1030));

        let (model, tokens) = last_claude_usage("{}\nnot json");
        assert!(model.is_none());
        assert!(tokens.is_none());
    }

    /// Maps real model IDs, including dated suffixes, to 1M windows and uses 200k for unknown models.
    #[test]
    fn context_limit_for_model_maps_window() {
        for m in [
            "claude-opus-4-8",
            "claude-opus-4-7",
            "claude-opus-4-6",
            "claude-sonnet-4-6",
            "claude-fable-5",
            "claude-mythos-5",
        ] {
            assert_eq!(
                context_limit_for_model(m),
                CONTEXT_LIMIT_1M,
                "should be classified as 1M: {m}"
            );
        }
        // Dated suffixes still match.
        assert_eq!(
            context_limit_for_model("claude-opus-4-8-20260101"),
            CONTEXT_LIMIT_1M
        );
        // 200k and unknown models use the fallback.
        for m in [
            "claude-haiku-4-5",
            "claude-haiku-4-5-20251001",
            "claude-opus-4-1",
            "weird-model",
        ] {
            assert_eq!(
                context_limit_for_model(m),
                CONTEXT_LIMIT_DEFAULT,
                "should be classified as 200k: {m}"
            );
        }
    }

    /// Current-turn stats begin after the final real user message; tool results are not boundaries. Sum output tokens,
    /// count tool_use, and deduplicate changed paths.
    #[test]
    fn turn_stats_counts_after_last_user_message() {
        let text = [
            // Previous turn, which must be ignored.
            r#"{"type":"user","message":{"role":"user","content":"old turn"}}"#,
            r#"{"type":"assistant","message":{"model":"claude-x","usage":{"output_tokens":5},"content":[{"type":"tool_use","name":"Read","input":{"file_path":"old.ts"}}]}}"#,
            // Current-turn boundary: the final genuine user message.
            r#"{"type":"user","message":{"role":"user","content":"new turn"}}"#,
            r#"{"type":"assistant","message":{"model":"claude-x","usage":{"output_tokens":100},"content":[{"type":"text","text":"ok"},{"type":"tool_use","name":"Edit","input":{"file_path":"a.ts"}}]}}"#,
            // A user-role all-tool_result row is not a new turn boundary.
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"done"}]}}"#,
            // Read does not change a file; Write repeats Edit's a.ts and is deduplicated.
            r#"{"type":"assistant","message":{"model":"claude-x","usage":{"output_tokens":40},"content":[{"type":"tool_use","name":"Read","input":{"file_path":"x.ts"}},{"type":"tool_use","name":"Write","input":{"file_path":"a.ts"}}]}}"#,
        ]
        .join("\n");
        let stats = turn_stats_from_text(&text);
        assert_eq!(stats.tokens, 140); // 100 + 40; exclude 5 from the previous turn.
        assert_eq!(stats.tools_used, 3); // Edit + Read + Write
        assert_eq!(stats.files_touched, 1); // Deduplicate Edit/Write of a.ts; exclude Read.
    }

    /// Tail reading returns the full text when cap covers it, otherwise discards the first partial line.
    #[test]
    fn read_tail_trims_partial_first_line() {
        let path = std::env::temp_dir().join(format!("vlx-read-tail-{}.jsonl", std::process::id()));
        std::fs::write(&path, "first line\nsecond line\nthird line\n").unwrap();
        // A large enough cap returns the complete text.
        let all = read_tail(&path, 4096).unwrap();
        assert_eq!(all, "first line\nsecond line\nthird line\n");
        // Starting inside the second line drops its fragment and begins at the third.
        let tail = read_tail(&path, "third line\n".len() as u64 + 3).unwrap();
        assert_eq!(tail, "third line\n");
        let _ = std::fs::remove_file(&path);
    }

    /// Ignored read-only smoke test running usage/settings parsing against the newest real local Claude transcript.
    /// `cargo test --lib -- --ignored context_info_real_file_smoke --nocapture`
    #[test]
    #[ignore = "depends on the real ~/.claude history on this machine"]
    fn context_info_real_file_smoke() {
        let projects = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .expect("HOME")
            .join(".claude/projects");
        let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = Vec::new();
        for dir in std::fs::read_dir(&projects).expect("reading projects").flatten() {
            let Ok(entries) = std::fs::read_dir(dir.path()) else {
                continue;
            };
            for f in entries.flatten() {
                let p = f.path();
                if p.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                    if let Ok(m) = p.metadata().and_then(|m| m.modified()) {
                        files.push((m, p));
                    }
                }
            }
        }
        files.sort_by_key(|f| std::cmp::Reverse(f.0));
        let path = &files.first().expect("there should be at least one transcript").1;
        eprintln!("parsing: {}", path.display());
        let tail = read_tail(path, CONTEXT_TAIL_BYTES).expect("reading the tail failed");
        let (model, tokens) = last_claude_usage(&tail);
        let settings = claude_settings();
        eprintln!(
            "model={model:?} tokens={tokens:?} settings_model={:?}",
            settings.as_ref().and_then(|s| s.get("model")),
        );
        assert!(model.is_some(), "a real transcript should yield a model");
        assert!(tokens.is_some(), "a real transcript should yield context tokens");
    }

    /// Deduplicates adjacent repeated tool names while preserving order across different tools.
    #[test]
    fn merge_dedups_consecutive_tools() {
        let pieces = vec![
            Piece {
                role: "assistant",
                text: String::new(),
                timestamp: None,
                tools: vec!["shell".into()],
            },
            Piece {
                role: "assistant",
                text: String::new(),
                timestamp: None,
                tools: vec!["shell".into()],
            },
            Piece {
                role: "assistant",
                text: "done".into(),
                timestamp: None,
                tools: vec!["apply_patch".into()],
            },
        ];
        let msgs = merge(pieces.into_iter());
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].tools, vec!["shell", "apply_patch"]);
        assert_eq!(msgs[0].text, "done");
    }
}
