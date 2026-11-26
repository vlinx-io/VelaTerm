//! Antigravity's native step transcript.
//!
//! The conversation view now runs over `agy acp`; this module keeps the transcript parser used by
//! history replay, search and export. The retired print-mode envelope helpers remain for compatibility
//! with recordings written before the switch.

use std::collections::{HashMap, VecDeque};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{json, Value};
use super::engine::ChatRow;
use super::protocol::ChatImage;

pub const EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];

/// Internal continuation prompt sent after restarting `agy acp` to recover from a mid-session HTTP 401.
/// Hidden from user bubbles in live and replayed timelines so recovery remains seamless.
pub const AUTH_RECOVERY_PROMPT: &str = "Your previous step was interrupted by a transient OAuth token expiration (HTTP 401) that has now been refreshed. Continue from where you left off without repeating completed steps.";

/// Internal continuation prompt sent after `agy acp` aborts a turn on a transient upstream API or
/// transport error (such as HTTP 404/500/503/504 or mid-stream EOF/connection reset).
/// Hidden from user bubbles in live and replayed timelines so recovery remains seamless.
pub const TRANSIENT_RECOVERY_PROMPT: &str = "Your previous step was interrupted by a transient upstream API error. Continue from where you left off without repeating completed steps.";

/// Whether a user prompt string is one of VelaTerm's internal Antigravity recovery prompts.
pub fn is_internal_recovery_prompt(text: &str) -> bool {
    matches!(text, AUTH_RECOVERY_PROMPT | TRANSIENT_RECOVERY_PROMPT)
}

/// Whether an error string is Antigravity's Google OAuth 401 credential failure.
pub fn is_oauth_401_error(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    (lower.contains("unauthenticated")
        && (lower.contains("401") || lower.contains("invalid authentication credentials")))
        || (lower.contains("invalid authentication credentials") && lower.contains("oauth 2"))
        || lower.contains("authentication expired. please log back in")
}

/// Extract the error message from `agy acp`'s streamed `"\n⚠ <err>\n"` chunk when `<err>` is an OAuth 401.
pub fn strip_streamed_401_warning(chunk: &str) -> Option<String> {
    let trimmed = chunk.trim();
    let rest = trimmed
        .strip_prefix('\u{26a0}')
        .map(|s| s.trim_start_matches('\u{fe0f}').trim())?;
    is_oauth_401_error(rest).then(|| rest.to_string())
}

/// Whether an error string is a transient Antigravity upstream API or transport error that can be
/// safely retried on the same ACP session (for example Google CloudCode 404/500/502/503/504,
/// `There was a network issue connecting to the server`, or stream EOF / connection reset).
pub fn is_transient_upstream_error(text: &str) -> bool {
    if is_oauth_401_error(text) || is_invalid_thinking_signature(text) {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    if lower.contains("failed to construct executor")
        || lower.contains("resource_exhausted")
        || lower.contains("quota_exhausted")
        || lower.contains("code 429")
        || lower.contains("error 429,")
        || lower.contains("permission_denied")
        || lower.contains("invalid_argument")
    {
        return false;
    }
    lower.contains("there was a network issue connecting to the server")
        || (lower.contains("not_found (code 404)") && lower.contains("requested entity was not found"))
        || lower.contains("internal (code 500)")
        || lower.contains("bad gateway")
        || lower.contains("code 502")
        || lower.contains("error 500,")
        || lower.contains("error 502,")
        || lower.contains("error 503,")
        || lower.contains("error 504,")
        || lower.contains("unavailable (code 503)")
        || lower.contains("deadline_exceeded (code 504)")
        || lower.contains("failed to connect to backend api")
        || lower.contains("the model is currently unreachable")
        || (lower.contains("request failed:")
            && (lower.contains("eof")
                || lower.contains("connection reset by peer")
                || lower.contains("broken pipe")
                || lower.contains("tls handshake timeout")
                || lower.contains("i/o timeout")
                || lower.contains("network is unreachable")
                || lower.contains("temporary failure in name resolution")
                || lower.contains("context deadline exceeded")))
}

/// Whether an error indicates a corrupted Claude `thinking` block signature in Antigravity's
/// persisted history, which rejects every subsequent prompt sent to that native conversation.
pub fn is_invalid_thinking_signature(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("messages.")
        && lower.contains(".content.")
        && (lower.contains(".thinking.signature: field required")
            || lower.contains("invalid `signature` in `thinking` block"))
}

/// Whether an error string reports an Antigravity quota or rate-limit exhaustion.
pub fn is_usage_limit_error(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("reason:quota_exhausted")
        || lower.contains("resource_exhausted")
        || lower.contains("code 429")
        || lower.contains("error 429,")
        || lower.contains("you have reached your current quota for this period")
}

/// Extract a quota / rate-limit `Reset` timestamp from Antigravity error metadata
/// (`quotaResetTimeStamp:<iso>`, `quotaResetDelay:<go_duration>`, or `retryDelay:<go_duration>`).
pub fn parse_usage_limit_reset(text: &str, now_epoch_secs: Option<i64>) -> Option<super::auto_continue::Reset> {
    if let Some(raw_ts) = extract_metadata_token(text, "quotaResetTimeStamp") {
        if let Some(ms) = parsed_timestamp_ms(Some(raw_ts)) {
            let resets_at = (ms / 1_000) as i64;
            let limit_type = now_epoch_secs.and_then(|now| infer_limit_type_secs(resets_at.saturating_sub(now)));
            return Some(super::auto_continue::Reset { resets_at, limit_type });
        }
    }
    let now = now_epoch_secs?;
    let delay_ms = extract_metadata_token(text, "quotaResetDelay")
        .and_then(parse_go_duration_ms)
        .or_else(|| extract_metadata_token(text, "retryDelay").and_then(parse_go_duration_ms))?;
    if delay_ms == 0 {
        return None;
    }
    let delay_secs = ((delay_ms + 999) / 1_000) as i64;
    Some(super::auto_continue::Reset {
        resets_at: now.saturating_add(delay_secs),
        limit_type: infer_limit_type_secs(delay_secs),
    })
}

fn infer_limit_type_secs(delta_secs: i64) -> Option<String> {
    if delta_secs <= 0 {
        None
    } else if delta_secs <= 6 * 3_600 {
        Some("five_hour".into())
    } else if delta_secs >= 24 * 3_600 {
        Some("seven_day".into())
    } else {
        None
    }
}

fn extract_metadata_token<'a>(text: &'a str, field: &str) -> Option<&'a str> {
    let mut search_from = 0;
    while let Some(rel) = text[search_from..].find(field) {
        let after_field = search_from + rel + field.len();
        search_from = after_field;
        let rest = &text[after_field..];
        // Accept Go map (`field:value`), JSON (`"field": "value"`), and escaped JSON (`\"field\":\"value\"`).
        let rest = rest.strip_prefix("\\\"").or_else(|| rest.strip_prefix('"')).unwrap_or(rest);
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix(':') else {
            continue;
        };
        let rest = rest.trim_start();
        let rest = rest.strip_prefix("\\\"").or_else(|| rest.strip_prefix('"')).unwrap_or(rest);
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, ']' | '}' | ',' | '"' | '\'' | '\\'))
            .unwrap_or(rest.len());
        let token = &rest[..end];
        if !token.is_empty() {
            return Some(token);
        }
    }
    None
}

fn parse_go_duration_ms(raw: &str) -> Option<u64> {
    let mut cursor = 0;
    let bytes = raw.as_bytes();
    let mut total_ms = 0f64;
    let mut matched = false;
    while cursor < bytes.len() {
        let num_start = cursor;
        let mut saw_dot = false;
        while cursor < bytes.len() {
            let b = bytes[cursor];
            if b.is_ascii_digit() {
                cursor += 1;
            } else if b == b'.' && !saw_dot {
                saw_dot = true;
                cursor += 1;
            } else {
                break;
            }
        }
        if cursor == num_start {
            return None;
        }
        let amount: f64 = raw[num_start..cursor].parse().ok()?;
        let rest = &raw[cursor..];
        let (unit_ms, unit_len) = if rest.starts_with("ns") {
            (1e-6, 2)
        } else if rest.starts_with("us") {
            (1e-3, 2)
        } else if rest.starts_with("µs") {
            (1e-3, "µs".len())
        } else if rest.starts_with("μs") {
            (1e-3, "μs".len())
        } else if rest.starts_with("ms") {
            (1.0, 2)
        } else if rest.starts_with('s') {
            (1_000.0, 1)
        } else if rest.starts_with('m') {
            (60_000.0, 1)
        } else if rest.starts_with('h') {
            (3_600_000.0, 1)
        } else {
            return None;
        };
        total_ms += amount * unit_ms;
        cursor += unit_len;
        matched = true;
    }
    matched.then_some(total_ms.ceil() as u64)
}

/// Extract the error message from `agy acp`'s streamed `"\n⚠ <err>\n"` chunk when `<err>` is a
/// transient upstream API or transport error.
pub fn strip_streamed_transient_warning(chunk: &str) -> Option<String> {
    let trimmed = chunk.trim();
    let rest = trimmed
        .strip_prefix('\u{26a0}')
        .map(|s| s.trim_start_matches('\u{fe0f}').trim())?;
    let mut lines = rest.lines().map(str::trim).filter(|line| !line.is_empty());
    let first = lines.next()?;
    if !is_transient_upstream_error(first) {
        return None;
    }
    if lines.any(|line| !line.to_ascii_lowercase().starts_with("error id:")) {
        return None;
    }
    Some(first.to_string())
}

/// Map VelaTerm's Antigravity permission mode to the matching `agy acp` session mode id.
pub fn acp_mode_for_permission(permission_mode: &str) -> &'static str {
    match permission_mode.trim() {
        "plan" => "plan",
        "acceptEdits" | "accept-edits" => "accept-edits",
        "auto" => "auto",
        // Bypass lifts plan/edit restrictions in `agy acp` while VelaTerm auto-approves requests.
        "bypassPermissions" | "skip" => "auto",
        _ => "default",
    }
}

/// Map an `agy acp` session mode id back into VelaTerm's Antigravity permission mode.
pub fn permission_for_acp_mode(acp_mode: &str, current_permission: &str) -> &'static str {
    match acp_mode.trim() {
        "plan" => "plan",
        "accept-edits" | "acceptEdits" => "acceptEdits",
        "auto" => if current_permission == "bypassPermissions" { "bypassPermissions" } else { "auto" },
        _ => "default",
    }
}

pub fn launch_args(resume: Option<&str>, model: Option<&str>, effort: Option<&str>, mode: &str,
    extra: &[String]) -> Result<Vec<String>, String> {
    if effort.is_some_and(|value| !EFFORTS.contains(&value)) {
        return Err("Antigravity effort must be low, medium, high, xhigh, max, or automatic".into());
    }
    let mut args = vec!["--input-format".into(), "stream-json".into(),
        "--output-format".into(), "stream-json".into()];
    for (flag, value) in [("--conversation", resume), ("--model", model), ("--effort", effort)] {
        if let Some(value) = value { args.extend([flag.into(), value.into()]); }
    }
    if mode == "bypassPermissions" { args.push("--dangerously-skip-permissions".into()); }
    // A custom argument must not replace the native conversation or the framing this driver owns.
    for arg in extra {
        let flag = arg.split('=').next().unwrap_or(arg);
        if matches!(flag, "--input-format" | "--output-format" | "--conversation" | "--continue" |
            "-c" | "--print" | "--prompt" | "-p" | "--prompt-interactive" | "-i" | "--remote-control") {
            return Err(format!("Antigravity conversation view manages {flag}; remove it from custom arguments"));
        }
    }
    args.extend_from_slice(extra);
    Ok(args)
}

pub fn validate(text: &str, image_count: usize, behavior: &str) -> Result<(), String> {
    if image_count > 0 { return Err("Antigravity conversation view currently supports text messages only".into()); }
    if behavior == "steer" { return Err("Antigravity does not support steering; queue the message instead".into()); }
    // Native CLI commands bypass NDJSON and would terminate a persistent input stream.
    if text.trim_start().starts_with('/') {
        return Err("Antigravity slash commands are unavailable in conversation view; use the terminal view".into());
    }
    Ok(())
}

pub fn user_message(text: &str) -> Value { json!({"event":"user","message":{"content":text}}) }

/// Normalize Antigravity `ask_question` arguments into the shared `AskUserQuestion` card input shape.
pub fn normalize_ask_question_input(args: &Value) -> Value {
    let Some(raw_questions) = args.get("questions").and_then(Value::as_array).filter(|q| !q.is_empty()) else {
        return args.clone();
    };
    let questions: Vec<Value> = raw_questions.iter().enumerate().map(|(idx, q)| {
        let header = q.get("header").and_then(Value::as_str).filter(|s| !s.is_empty())
            .map(str::to_string).unwrap_or_else(|| format!("Q{}", idx + 1));
        let question = q.get("question").and_then(Value::as_str).unwrap_or("");
        let multi = q.get("multiSelect").or_else(|| q.get("is_multi_select"))
            .or_else(|| q.get("multi_select")).and_then(Value::as_bool) == Some(true);
        let allow_other = q.get("allowOther").or_else(|| q.get("allow_other"))
            .and_then(Value::as_bool) != Some(false);
        let allow_empty = q.get("allowEmpty").and_then(Value::as_bool) == Some(true);
        let existing_values = q.get("_optionValues").and_then(Value::as_array);
        let mut options = Vec::new();
        let mut option_values = Vec::new();
        let mut has_explicit_value = false;
        if let Some(raw_opts) = q.get("options").and_then(Value::as_array) {
            for (opt_idx, opt) in raw_opts.iter().enumerate() {
                if let Some(s) = opt.as_str() {
                    options.push(json!({"label": s}));
                    let val = existing_values.and_then(|arr| arr.get(opt_idx)).and_then(Value::as_str).unwrap_or(s);
                    option_values.push(json!(val));
                } else if let Some(obj) = opt.as_object() {
                    let Some(label) = obj.get("label").or_else(|| obj.get("text")).or_else(|| obj.get("id"))
                        .and_then(Value::as_str) else { continue };
                    let mut item = json!({"label": label});
                    if let Some(desc) = obj.get("description").and_then(Value::as_str).filter(|s| !s.is_empty()) {
                        item["description"] = json!(desc);
                    }
                    options.push(item);
                    let explicit = obj.get("id").or_else(|| obj.get("value")).and_then(Value::as_str);
                    if explicit.is_some() { has_explicit_value = true; }
                    let val = explicit
                        .or_else(|| existing_values.and_then(|arr| arr.get(opt_idx)).and_then(Value::as_str))
                        .unwrap_or(label);
                    option_values.push(json!(val));
                }
            }
        }
        let mut out = json!({
            "header": header,
            "question": question,
            "multiSelect": multi,
            "allowOther": allow_other,
            "allowEmpty": allow_empty,
            "options": options,
        });
        if has_explicit_value || existing_values.is_some() {
            out["_optionValues"] = Value::Array(option_values);
        }
        for key in ["placeholder", "_key", "_otherKey", "_freeTextOnly"] {
            if let Some(val) = q.get(key) { out[key] = val.clone(); }
        }
        out
    }).collect();
    let mut normalized = json!({
        "_acpAskQuestion": true,
        "questions": questions,
    });
    for key in ["prompt", "answers", "_answerList", "_selectedIndices", "_otherTexts", "_skippedIndices"] {
        if let Some(val) = args.get(key) { normalized[key] = val.clone(); }
    }
    normalized
}

fn parse_agy_answer_line(line: &str) -> Option<(usize, &str)> {
    let rest = line.strip_prefix('A')?;
    let (digits, after_colon) = rest.split_once(':')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let num = digits.parse::<usize>().ok().filter(|n| *n > 0)?;
    let value = after_colon.strip_prefix(' ').unwrap_or(after_colon);
    Some((num, value))
}

/// Strip Antigravity header lines and populate `answers` / `_answerList` from `A1: ...` output lines.
pub fn apply_ask_question_output(input: &mut Value, raw_output: &str) -> String {
    let normalized = raw_output.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut start = 0;
    while start < lines.len() && (lines[start].starts_with("Created At:")
        || lines[start].starts_with("Completed At:")
        || lines[start].trim().is_empty()) {
        start += 1;
    }
    let cleaned = lines[start..].join("\n").trim().to_string();
    if cleaned.is_empty() || cleaned == "Step was canceled" || cleaned == "Dismissed by user" {
        return cleaned;
    }
    let mut by_num: HashMap<usize, String> = HashMap::new();
    let mut current: Option<usize> = None;
    for &line in &lines[start..] {
        if let Some((num, val)) = parse_agy_answer_line(line) {
            current = Some(num);
            by_num.insert(num, val.to_string());
        } else if let Some(active) = current {
            if let Some(buf) = by_num.get_mut(&active) {
                buf.push('\n');
                buf.push_str(line);
            }
        }
    }
    if by_num.is_empty() {
        return cleaned;
    }
    let Some(questions) = input.get("questions").and_then(Value::as_array) else {
        return cleaned;
    };
    let mut answers_map = serde_json::Map::new();
    let mut answer_list = Vec::with_capacity(questions.len());
    for (idx, q) in questions.iter().enumerate() {
        let val = by_num.get(&(idx + 1)).map(|raw| {
            let trimmed = raw.trim();
            if trimmed == "User Skipped" { "" } else { trimmed }
        });
        if let Some(ans) = val {
            if let Some(q_text) = q.get("question").and_then(Value::as_str) {
                answers_map.insert(q_text.to_string(), json!(ans));
            }
            answer_list.push(json!(ans));
        } else {
            answer_list.push(Value::Null);
        }
    }
    if !answers_map.is_empty() || answer_list.iter().any(|v| !v.is_null()) {
        input["answers"] = Value::Object(answers_map);
        input["_answerList"] = Value::Array(answer_list);
    }
    cleaned
}

/// Preserve native parameters while adding the presentation fields used by existing tool cards.
pub fn tool_input(name: &str, parameters: &Value) -> (String, Value) {
    if matches!(name, "ask_question" | "AskUserQuestion") {
        return ("AskUserQuestion".into(), normalize_ask_question_input(parameters));
    }
    let mut input = parameters.clone();
    let aliases: &[(&str, &str)] = match name {
        "run_command" => &[("CommandLine", "command"), ("Cwd", "cwd")],
        "view_file" => &[("AbsolutePath", "file_path")],
        "write_to_file" => &[("TargetFile", "file_path"), ("CodeContent", "content")],
        "replace_file_content" => &[("TargetFile", "file_path"), ("TargetContent", "old_string"), ("ReplacementContent", "new_string")],
        "grep_search" => &[("SearchPath", "path"), ("Query", "pattern")],
        "find_by_name" => &[("SearchDirectory", "path"), ("Pattern", "pattern")],
        "invoke_subagent" => &[("Role", "description"), ("TypeName", "subagent_type"), ("Prompt", "prompt")],
        _ => &[],
    };
    if let Some(object) = input.as_object_mut() {
        for (source, target) in aliases {
            if let Some(value) = parameters.get(source) {
                let mapped = if name == "invoke_subagent" {
                    unwrapped_arg_value(value)
                } else {
                    value.clone()
                };
                object.insert((*target).into(), mapped);
            }
        }
    }
    let name = match name {
        "run_command" => "Bash", "view_file" => "Read", "write_to_file" => "Write",
        "replace_file_content" => "Edit", "grep_search" => "Grep", "find_by_name" => "Glob",
        "invoke_subagent" => "Agent",
        _ => name,
    };
    (name.into(), input)
}

fn unwrapped_arg_value(value: &Value) -> Value {
    if let Some(raw) = value.as_str() {
        if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
            if let Ok(decoded) = serde_json::from_str::<String>(raw) {
                return Value::String(decoded);
            }
        }
    }
    value.clone()
}

/// Extract the child `conversationId` from an `invoke_subagent` tool result.
pub fn subagent_conversation_id(text: &str) -> Option<String> {
    let needle = "\"conversationId\"";
    let mut offset = 0;
    while let Some(pos) = text[offset..].find(needle) {
        let rest = &text[offset + pos + needle.len()..];
        let trimmed = rest.trim_start_matches(|c: char| c.is_ascii_whitespace());
        if let Some(after_colon) = trimmed.strip_prefix(':') {
            let after_ws = after_colon.trim_start_matches(|c: char| c.is_ascii_whitespace());
            if let Some(after_quote) = after_ws.strip_prefix('"') {
                if let Some(end) = after_quote.find('"') {
                    let candidate = &after_quote[..end];
                    if crate::agent::antigravity::is_native_id(candidate) {
                        return Some(candidate.to_string());
                    }
                }
            }
        }
        offset += pos + needle.len();
    }
    None
}

pub fn tool_row(id: String, name: &str, input: &Value, output: Option<String>, error: bool, done: bool) -> ChatRow {
    let (name, mut input) = tool_input(name, input);
    let output = if !error && name == "AskUserQuestion" {
        output.map(|raw| apply_ask_question_output(&mut input, &raw))
    } else {
        output
    };
    ChatRow::Tool { id, name, input, output, is_error: error,
        status: if error { "failed" } else if done { "completed" } else { "running" },
        subagent: None, children: Vec::new() }
}

/// Extract a string tool parameter, unwrapping one layer of JSON string encoding if present.
pub(crate) fn arg_string(args: &Value, key: &str) -> Option<String> {
    let raw = args.get(key)?.as_str()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let decoded = if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        serde_json::from_str::<String>(trimmed).unwrap_or_else(|_| trimmed.to_string())
    } else {
        trimmed.to_string()
    };
    let clean = decoded.trim();
    (!clean.is_empty()).then(|| clean.to_string())
}

pub(crate) fn first_line_preview(text: &str, max_chars: usize) -> Option<String> {
    let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    let mut chars = line.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        Some(format!("{head}..."))
    } else {
        Some(head)
    }
}

pub(crate) fn valid_shell_task_id(id: &str) -> bool {
    id.strip_prefix("task-")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
}

pub(crate) fn shell_task_id(raw: &str) -> Option<String> {
    let candidate = raw.rsplit('/').next().unwrap_or(raw).trim();
    valid_shell_task_id(candidate).then(|| candidate.to_string())
}

pub(crate) fn parse_background_launch(content: &str) -> Option<(String, String)> {
    const MARKER: &str = "Tool is running as a background task with task id:";
    let mut task_id = None;
    let mut desc_lines: Option<Vec<&str>> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(MARKER) {
            task_id = shell_task_id(rest.trim());
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("Task Description:") {
            let mut lines = Vec::new();
            let head = rest.trim();
            if !head.is_empty() {
                lines.push(head);
            }
            desc_lines = Some(lines);
            continue;
        }
        if let Some(lines) = desc_lines.as_mut() {
            if trimmed.starts_with("Task logs are available at:") || trimmed.starts_with("Log output:") {
                break;
            }
            lines.push(line.trim_end_matches('\r'));
        }
    }

    let id = task_id?;
    let description = desc_lines
        .map(|lines| lines.join("\n").trim().to_string())
        .unwrap_or_default();
    Some((id, description))
}

/// Derive a short task label from a native Antigravity tool call for task lists and wake headers.
pub(crate) fn tool_description(name: &str, args: &Value) -> Option<String> {
    match name {
        "run_command" => arg_string(args, "toolSummary")
            .or_else(|| arg_string(args, "CommandLine").and_then(|c| first_line_preview(&c, 160))),
        "run_workflow" => arg_string(args, "Description").or_else(|| arg_string(args, "ScriptPath")),
        "schedule" => arg_string(args, "Prompt").and_then(|p| first_line_preview(&p, 160)),
        "invoke_subagent" => arg_string(args, "Role").or_else(|| arg_string(args, "TypeName")),
        _ => None,
    }
}

/// Classify a finished shell/workflow task notification, keeping `vrun` exit code 75 checkpoints non-error.
pub(crate) fn classify_task_exit(content: &str) -> &'static str {
    if content.contains("was canceled with result:") {
        return "canceled";
    }
    for line in content.lines().take(15) {
        let trimmed = line.trim();
        if matches!(trimmed, "Command output:" | "Log output:") {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Command exited with code ") {
            let digits: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '-')
                .collect();
            if let Ok(code) = digits.parse::<i64>() {
                if code == 0 {
                    return "completed";
                }
                // `vrun` uses EX_TEMPFAIL (75) for periodic progress checkpoints while the command keeps running.
                if code == 75
                    && (content.contains("vrun: progress report; the command keeps running")
                        || (content.contains("vrun: ") && content.contains(" is still running · ")))
                {
                    return "completed";
                }
                return "failed";
            }
        }
    }
    if content.contains("finished with result:")
        || (content.contains("Scheduled timer of ") && content.contains(" has expired."))
        || content.contains("Timer fired.")
    {
        "completed"
    } else {
        "ended"
    }
}

/// Parse `[Message] timestamp=... sender=... content=...` deliveries from a `SYSTEM_MESSAGE` step.
pub(crate) fn parse_system_deliveries(content: &str) -> Vec<(Option<String>, String, String)> {
    let mut out = Vec::new();
    for block in content.split("[Message]").skip(1) {
        let Some((meta, body)) = block.split_once("content=") else {
            continue;
        };
        let timestamp_raw = meta
            .split_whitespace()
            .find_map(|token| token.strip_prefix("timestamp="))
            .map(str::to_string);
        let Some(sender) = meta
            .split_whitespace()
            .find_map(|token| token.strip_prefix("sender="))
            .map(str::to_string) else {
            continue;
        };
        out.push((timestamp_raw, sender, body.to_string()));
    }
    out
}

/// Format a one-line summary for a `ChatRow::Wake` separator opened by a background message delivery.
pub(crate) fn wake_summary(sender: &str, body: &str, task_description: Option<&str>) -> Option<String> {
    if sender == "system" {
        return None;
    }
    let desc = task_description.map(str::trim).filter(|s| !s.is_empty());
    if let Some(task_id) = shell_task_id(sender) {
        let status = classify_task_exit(body);
        return Some(match desc {
            Some(desc) => format!("Background task \"{desc}\" {status}"),
            None => format!("Background task {task_id} {status}"),
        });
    }
    if crate::agent::antigravity::is_native_id(sender) {
        return Some(match desc {
            Some(desc) => format!("Subagent \"{desc}\" completed"),
            None => "Subagent completed".into(),
        });
    }
    Some(first_line_preview(body, 120).unwrap_or_default())
}

/// Parse an RFC3339 timestamp (`...Z` or `...±HH:MM`) into epoch milliseconds.
pub(crate) fn parsed_timestamp_ms(timestamp: Option<&str>) -> Option<u64> {
    let raw = timestamp?.trim();
    if raw.ends_with('Z') || raw.ends_with('z') {
        let normalized = if raw.ends_with('z') {
            format!("{}Z", &raw[..raw.len() - 1])
        } else {
            raw.to_string()
        };
        return super::history::parsed_at(Some(&normalized)).and_then(|ms| u64::try_from(ms).ok());
    }
    if raw.len() >= 6 {
        let (head, tz) = raw.split_at(raw.len() - 6);
        let bytes = tz.as_bytes();
        if matches!(bytes[0], b'+' | b'-') && bytes[3] == b':' {
            let sign = if bytes[0] == b'-' { -1i64 } else { 1i64 };
            let hours: i64 = tz[1..3].parse().ok()?;
            let mins: i64 = tz[4..6].parse().ok()?;
            let offset_ms = sign * (hours * 3_600 + mins * 60) * 1_000;
            let utc_like = format!("{head}Z");
            let base = super::history::parsed_at(Some(&utc_like))?;
            return u64::try_from(base - offset_ms).ok();
        }
    }
    super::history::parsed_at(Some(raw)).and_then(|ms| u64::try_from(ms).ok())
}

pub fn step_id(index: u64) -> String { format!("agy-step-{index}") }

fn segmented_step_id(segment: u64, index: u64) -> String {
    if segment == 0 {
        step_id(index)
    } else {
        format!("agy-step-s{segment}-{index}")
    }
}

#[derive(Default)]
pub struct StreamState {
    text: HashMap<String, String>,
    pub last_answer: Option<String>,
}

impl StreamState {
    pub fn begin_turn(&mut self) { self.text.clear(); self.last_answer = None; }

    pub fn step(&mut self, step: &Value, model: Option<String>) -> Option<ChatRow> {
        let id = step_id(step.get("step_index")?.as_u64()?);
        let done = step.get("state").and_then(Value::as_str) == Some("DONE");
        match step.get("step_type")?.as_str()? {
            "agent_response" => {
                let text = self.text.entry(id.clone()).or_default();
                if let Some(delta) = step.get("text_delta").and_then(Value::as_str) { text.push_str(delta); }
                if text.is_empty() { return None; }
                self.last_answer = Some(id.clone());
                Some(ChatRow::Assistant { id, text: text.clone(), streaming: !done, model, at: None, duration_ms: None })
            }
            "tool" => {
                let info = step.get("tool_info").or_else(|| step.get("subagent_info"));
                let name = step.get("tool_name").and_then(Value::as_str)
                    .or_else(|| info?.get("name")?.as_str()).unwrap_or("Antigravity tool");
                let error = info.and_then(|v| v.get("error")).filter(|v| !v.is_null());
                let output = info.and_then(|v| v.get("output")).filter(|v| !v.is_null())
                    .map(|v| v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string()))
                    .or_else(|| error.map(|v| v.get("message").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| v.to_string())))
                    .or_else(|| step.get("subagent_info").map(|v| v.to_string()));
                Some(tool_row(id, name, &info.and_then(|v| v.get("parameters")).cloned().unwrap_or_else(|| json!({})),
                    output, error.is_some(), done))
            }
            _ => None,
        }
    }

    /// Native duration includes previous turns and idle time; the engine times the active turn itself.
    pub fn result(&mut self, result: &Value, model: Option<String>) -> Option<ChatRow> {
        let response = result.get("response").and_then(Value::as_str).unwrap_or("");
        if self.last_answer.is_none() && !response.is_empty() {
            Some(ChatRow::Assistant { id: format!("agy-result-{}", result.get("num_turns").and_then(Value::as_u64).unwrap_or(0)),
                text: response.into(), streaming: false, model, at: None, duration_ms: None })
        } else { None }
    }
}

/// Read the genuine user request, excluding runtime metadata and setting-change annotations.
pub fn user_request(content: &str) -> Option<String> {
    let text = raw_user_request(content)?;
    (!is_internal_recovery_prompt(&text)).then_some(text)
}

fn raw_user_request(content: &str) -> Option<String> {
    let text = if let Some((_, rest)) = content.split_once("<USER_REQUEST>") {
        rest.split_once("</USER_REQUEST>")?.0
    } else { content };
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Images the user attached to one step.
///
/// The transcript records a media entry's file URI, not its bytes, and the file lives under the
/// conversation's brain directory. Read it back within the same size and count limits a live prompt
/// obeys; an unreadable or oversized file simply leaves that attachment out of the replay.
fn step_images(step: &Value, budget: &mut usize) -> Vec<ChatImage> {
    let mut images = Vec::new();
    for media in step.get("media").and_then(Value::as_array).into_iter().flatten() {
        if images.len() >= crate::command_core::MAX_IMAGES_PER_MESSAGE { break; }
        let Some(mime) = media.get("mime_type").and_then(Value::as_str).filter(|mime| mime.starts_with("image/")) else { continue };
        let Some(uri) = media.get("uri").and_then(Value::as_str).filter(|uri| !uri.is_empty()) else { continue };
        let path = uri.strip_prefix("file://").unwrap_or(uri);
        let Ok(bytes) = std::fs::read(path) else { continue };
        if bytes.len() > crate::command_core::MAX_IMAGE_BYTES || bytes.len() > *budget { continue; }
        *budget -= bytes.len();
        images.push(ChatImage { mime_type: mime.into(), data: STANDARD.encode(bytes) });
    }
    images
}

pub fn history_rows(content: &str) -> Vec<ChatRow> {
    let mut rows = Vec::new();
    let mut pending: VecDeque<(usize, String, Option<String>)> = VecDeque::new();
    let mut task_descriptions: HashMap<String, String> = HashMap::new();
    let mut in_user_turn = false;
    let mut in_wake_turn = false;
    // A replay holds every image inline. Past this budget older attachments stay out of the restored
    // timeline; the native recording still has them for a later read.
    let mut image_budget = 64 * 1024 * 1024;
    let mut last_step: Option<u64> = None;
    let mut segment: u64 = 0;
    for line in content.lines() {
        let Ok(step) = serde_json::from_str::<Value>(line) else { continue };
        let Some(index) = step.get("step_index").and_then(Value::as_u64) else { continue };
        if last_step.is_some_and(|prev| index < prev) {
            segment += 1;
            pending.clear();
            in_user_turn = false;
            in_wake_turn = false;
        }
        last_step = Some(index);
        let id = segmented_step_id(segment, index);
        let at = super::history::parsed_at(step.get("created_at").and_then(Value::as_str))
            .or_else(|| parsed_timestamp_ms(step.get("created_at").and_then(Value::as_str)).and_then(|ms| i64::try_from(ms).ok()));
        let text = step.get("content").and_then(Value::as_str).unwrap_or("");
        let done = step.get("status").and_then(Value::as_str) == Some("DONE");
        match step.get("type").and_then(Value::as_str) {
            Some("USER_INPUT") => {
                pending.clear();
                in_user_turn = true;
                in_wake_turn = false;
                let raw_req = raw_user_request(text);
                if raw_req.as_deref() == Some(AUTH_RECOVERY_PROMPT) {
                    if rows.last().is_some_and(|r| matches!(r, ChatRow::Error { message, .. } if is_oauth_401_error(message))) {
                        rows.pop();
                    }
                } else if raw_req.as_deref() == Some(TRANSIENT_RECOVERY_PROMPT) {
                    if rows.last().is_some_and(|r| matches!(r, ChatRow::Error { message, .. } if is_transient_upstream_error(message))) {
                        rows.pop();
                    }
                } else if let Some(text) = user_request(text) {
                    rows.push(super::engine::user_row(id, &text, step_images(&step, &mut image_budget), at));
                }
            }
            Some("SYSTEM_MESSAGE") => {
                if !in_user_turn && !in_wake_turn {
                    for (ts_raw, sender, body) in parse_system_deliveries(text) {
                        let key = shell_task_id(&sender).unwrap_or_else(|| sender.clone());
                        let desc = task_descriptions.get(&key).map(String::as_str);
                        if let Some(summary) = wake_summary(&sender, &body, desc) {
                            let wake_at = at.or_else(|| {
                                parsed_timestamp_ms(ts_raw.as_deref()).and_then(|ms| i64::try_from(ms).ok())
                            });
                            in_wake_turn = true;
                            rows.push(ChatRow::Wake { id, summary, at: wake_at });
                            break;
                        }
                    }
                }
            }
            Some("PLANNER_RESPONSE") => {
                if let Some(thinking) = step.get("thinking").and_then(Value::as_str).filter(|v| !v.is_empty()) {
                    rows.push(ChatRow::Reasoning { id: format!("{id}-thinking"), text: thinking.into(), streaming: !done });
                }
                if !text.is_empty() {
                    rows.push(ChatRow::Assistant { id: id.clone(), text: text.into(), streaming: !done, model: None, at, duration_ms: None });
                }
                let tool_calls = step.get("tool_calls").and_then(Value::as_array);
                for (ordinal, call) in tool_calls.into_iter().flatten().enumerate() {
                    let Some(name) = call.get("name").and_then(Value::as_str) else { continue };
                    let args = call.get("args").cloned().unwrap_or_else(|| json!({}));
                    let position = rows.len();
                    let call_desc = tool_description(name, &args);
                    rows.push(tool_row(format!("{id}-call-{ordinal}"), name, &args, None, false, false));
                    pending.push_back((position, name.to_string(), call_desc));
                }
                if done && tool_calls.is_none_or(Vec::is_empty) {
                    in_user_turn = false;
                    in_wake_turn = false;
                }
            }
            Some("ERROR_MESSAGE") => {
                if let Some((position, _, _)) = pending.pop_front() {
                    if let ChatRow::Tool { output, is_error, status, .. } = &mut rows[position] {
                        *output = Some(text.into()); *is_error = true; *status = "failed";
                    }
                } else if !text.is_empty() { rows.push(ChatRow::Error { id, message: text.into() }); }
            }
            // Native tool results have several category names; only pair them with a recorded call.
            Some("GENERIC" | "LIST_DIRECTORY" | "VIEW_FILE" | "CODE_ACTION" | "ASK_QUESTION") => {
                if let Some((position, call_name, call_desc)) = pending.pop_front() {
                    if let Some((task_id, task_desc)) = parse_background_launch(text) {
                        if let Some(desc) = call_desc.or_else(|| first_line_preview(&task_desc, 160)) {
                            task_descriptions.insert(task_id, desc);
                        }
                    } else if call_name == "invoke_subagent" {
                        if let Some(child_id) = subagent_conversation_id(text) {
                            if let Some(desc) = call_desc {
                                task_descriptions.insert(child_id, desc);
                            }
                        }
                    }
                    if let ChatRow::Tool { id: tool_id, name, input, output, status, .. } = &mut rows[position] {
                        *tool_id = id;
                        *output = Some(if name == "AskUserQuestion" {
                            apply_ask_question_output(input, text)
                        } else {
                            text.into()
                        });
                        *status = if done { "completed" } else { "running" };
                    }
                }
            }
            _ => {},
        }
    }
    rows
}

/// The same parsed context feeds archive viewing, search and Markdown export.
pub(crate) fn events(content: &str) -> Vec<crate::agent::export::Event> {
    use crate::agent::export::Event;
    let mut timestamps = HashMap::new();
    let mut last_step: Option<u64> = None;
    let mut segment: u64 = 0;
    for line in content.lines() {
        let Ok(step) = serde_json::from_str::<Value>(line) else { continue };
        let Some(index) = step.get("step_index").and_then(Value::as_u64) else { continue };
        if last_step.is_some_and(|prev| index < prev) {
            segment += 1;
        }
        last_step = Some(index);
        if let Some(ts) = step.get("created_at").and_then(Value::as_str) {
            timestamps.insert(segmented_step_id(segment, index), ts.to_string());
        }
    }
    let mut events = Vec::new();
    for row in history_rows(content) {
        let id = match &row {
            ChatRow::User { id, .. } | ChatRow::Assistant { id, .. } | ChatRow::Reasoning { id, .. }
            | ChatRow::Tool { id, .. } | ChatRow::Error { id, .. } | ChatRow::Shell { id, .. } => id,
            _ => continue,
        };
        let base = id.split("-thinking").next().unwrap_or(id).split("-call-").next().unwrap_or(id);
        let ts = timestamps.get(base).cloned();
        match row {
            ChatRow::User { text, .. } => events.push(Event::User { text, ts: ts.clone() }),
            ChatRow::Assistant { text, .. } => events.push(Event::AssistantText { text, ts: ts.clone() }),
            ChatRow::Reasoning { text, .. } => events.push(Event::Thinking { text, ts: ts.clone() }),
            ChatRow::Tool { id, name, input, output, is_error, .. } => {
                events.push(Event::ToolUse { id: Some(id.clone()), name: name.clone(), input, ts: ts.clone() });
                if let Some(text) = output { events.push(Event::ToolResult { id: Some(id), name: Some(name), text, is_error }); }
            }
            ChatRow::Error { message, .. } => events.push(Event::Command { text: message, ts: ts.clone() }),
            ChatRow::Shell { id, command, stdout, exit_code, status, stdout_truncated, .. } => {
                events.push(Event::Shell { id: Some(id), command, output: stdout, exit_code,
                    cancelled: status == "cancelled", truncated: stdout_truncated, ts: ts.clone() });
            }
            _ => {},
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_deltas_and_result_fallback_do_not_duplicate_answers() {
        let mut state = StreamState::default();
        for (delta, done) in [("hel", false), ("lo", true)] {
            let row = state.step(&json!({"step_index":1,"step_type":"agent_response","state":if done{"DONE"}else{"ACTIVE"},"text_delta":delta}), None).unwrap();
            if let ChatRow::Assistant { text, streaming, .. } = row { assert_eq!(streaming, !done); assert!("hello".starts_with(&text)); } else { panic!(); }
        }
        assert!(state.result(&json!({"response":"hello","duration_seconds":3.0}), None).is_none());
        state.begin_turn();
        let row = state.result(&json!({"response":"again","num_turns":2,"duration_seconds":5.5}), None);
        assert!(matches!(row, Some(ChatRow::Assistant { text, .. }) if text == "again"));
    }

    #[test]
    fn tool_failure_and_native_parameters_are_preserved() {
        let mut state = StreamState::default();
        let row = state.step(&json!({"step_index":4,"step_type":"tool","state":"DONE","tool_info":{"name":"run_command","parameters":{"CommandLine":"echo hi"},"error":{"message":"Permission denied"}}}), None).unwrap();
        assert!(matches!(row, ChatRow::Tool { name, input, output, status:"failed", .. }
            if name == "Bash" && input["command"] == "echo hi" && input["CommandLine"] == "echo hi" && output.as_deref() == Some("Permission denied")));
    }

    #[test]
    fn replay_excludes_metadata_and_pairs_the_tool_result() {
        let lines = [json!({"step_index":0,"type":"USER_INPUT","content":"<USER_REQUEST>test</USER_REQUEST><ADDITIONAL_METADATA>private</ADDITIONAL_METADATA>"}),
            json!({"step_index":1,"type":"PLANNER_RESPONSE","tool_calls":[{"name":"view_file","args":{"AbsolutePath":"/tmp/test"}}]}),
            json!({"step_index":2,"type":"GENERIC","status":"DONE","content":"file contents"}),
            json!({"step_index":3,"type":"PLANNER_RESPONSE","status":"DONE","content":"answer","thinking":"reason"})];
        let rows = history_rows(&lines.iter().map(Value::to_string).collect::<Vec<_>>().join("\n"));
        assert_eq!(rows.len(), 4);
        assert!(matches!(&rows[0], ChatRow::User { text, .. } if text == "test"));
        assert!(matches!(&rows[1], ChatRow::Tool { id, name, output, status:"completed", .. }
            if id == "agy-step-2" && name == "Read" && output.as_deref() == Some("file contents")));
    }

    #[test]
    fn user_media_is_replayed_as_a_chat_image() {
        let dir = std::env::temp_dir().join(format!("vlx-agy-media-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("upload.png");
        let bytes = b"\x89PNG\r\n\x1a\nreplay";
        std::fs::write(&path, bytes).unwrap();
        let line = json!({"step_index": 0, "type": "USER_INPUT",
            "content": "<USER_REQUEST>see this</USER_REQUEST>",
            "media": [{"mime_type": "image/png", "uri": path.to_string_lossy()}]}).to_string();
        let rows = history_rows(&line);
        let ChatRow::User { text, images, .. } = &rows[0] else { panic!() };
        assert_eq!(text, "see this");
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].mime_type, "image/png");
        assert_eq!(STANDARD.decode(&images[0].data).unwrap(), bytes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unsupported_input_and_protocol_overrides_are_rejected() {
        assert!(validate("hello", 1, "queue").is_err());
        assert!(validate("hello", 0, "steer").is_err());
        assert!(validate("/model", 0, "queue").is_err());
        assert!(launch_args(None, None, None, "default", &["--output-format=text".into()]).is_err());
        let args = launch_args(Some("native"), Some("model"), Some("high"), "default", &[]).unwrap();
        assert!(!args.iter().any(|v| v == "--dangerously-skip-permissions"));
        assert!(args.windows(2).any(|v| v == ["--conversation", "native"]));
    }

    #[test]
    fn invoke_subagent_maps_to_agent_and_extracts_conversation_id() {
        let (name, input) = tool_input(
            "invoke_subagent",
            &json!({"Role": "\"Codebase Researcher\"", "TypeName": "research", "Prompt": "Inspect engine.rs"}),
        );
        assert_eq!(name, "Agent");
        assert_eq!(input["description"], "Codebase Researcher");
        assert_eq!(input["subagent_type"], "research");
        assert_eq!(input["prompt"], "Inspect engine.rs");

        let ok = "Created At: 2026-11-15T12:45:17+08:00\nSubagent started: {\"conversationId\": \"c542ee7b-24f3-4043-9644-9dd04bd3bc15\"}";
        assert_eq!(subagent_conversation_id(ok).as_deref(), Some("c542ee7b-24f3-4043-9644-9dd04bd3bc15"));
        assert_eq!(subagent_conversation_id("{\"conversationId\":\"../escape\"}"), None);
        assert_eq!(subagent_conversation_id("plain error output"), None);
    }

    #[test]
    fn replay_emits_wake_only_between_turns_and_classifies_vrun_checkpoint() {
        let vrun_msg = "Task id \"conv-1/task-2\" finished with result:\nCommand exited with code 75.\n\nCommand output:\nvrun: beauty is still running · 180s elapsed · PID 42\nvrun: progress report; the command keeps running. Continue with `vrun --wait beauty`, which reports again in 180s.";
        assert_eq!(classify_task_exit(vrun_msg), "completed");
        assert_eq!(classify_task_exit("Command exited with code 75.\n\nCommand output:\nreal tempfail"), "failed");
        assert_eq!(classify_task_exit("Command exited with code 2."), "failed");
        assert_eq!(classify_task_exit("Scheduled timer of 3m0s (task \"conv-1/task-489\") has expired."), "completed");

        let lines = [
            json!({"step_index":0,"type":"USER_INPUT","created_at":"2026-11-15T04:00:00Z","content":"<USER_REQUEST>start build</USER_REQUEST>"}),
            json!({"step_index":1,"type":"PLANNER_RESPONSE","created_at":"2026-11-15T04:00:01Z","tool_calls":[{"name":"run_command","args":{"CommandLine":"vrun beauty echo","toolSummary":"Render story"}}]}),
            // Mid-turn SYSTEM_MESSAGE should not emit a wake boundary.
            json!({"step_index":2,"type":"SYSTEM_MESSAGE","created_at":"2026-11-15T04:00:02Z","content":"[Message] timestamp=2026-11-15T04:00:02Z sender=conv-1/task-99 content=Command exited with code 0."}),
            json!({"step_index":3,"type":"GENERIC","status":"RUNNING","created_at":"2026-11-15T04:00:02Z","content":"Tool is running as a background task with task id: conv-1/task-2\nTask Description: Render story"}),
            json!({"step_index":4,"type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-11-15T04:00:03Z","content":"Started in background."}),
            // System restart notices do not emit a wake boundary.
            json!({"step_index":5,"type":"SYSTEM_MESSAGE","created_at":"2026-11-15T04:01:00Z","content":"[Message] timestamp=2026-11-15T04:01:00Z sender=system content=[Notice] All your subagents and background tasks have been stopped due to server restart"}),
            // Idle vrun checkpoint delivery opens a wake turn with resolved task description.
            json!({"step_index":6,"type":"SYSTEM_MESSAGE","created_at":"2026-11-15T04:03:00Z","content":format!("[Message] timestamp=2026-11-15T04:03:00Z sender=conv-1/task-2 content={vrun_msg}")}),
            json!({"step_index":7,"type":"PLANNER_RESPONSE","created_at":"2026-11-15T04:03:01Z","tool_calls":[{"name":"run_command","args":{"CommandLine":"vrun --wait beauty","toolSummary":"Continue wait"}}]}),
            json!({"step_index":8,"type":"GENERIC","status":"RUNNING","created_at":"2026-11-15T04:03:02Z","content":"Tool is running as a background task with task id: conv-1/task-3\nTask Description: Continue wait"}),
            json!({"step_index":9,"type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-11-15T04:03:05Z","content":"3/10 complete; next report in 3 minutes."}),
        ];
        let content = lines.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
        let rows = history_rows(&content);
        assert!(rows.iter().all(|r| !matches!(r, ChatRow::Wake { id, .. } if id == "agy-step-2" || id == "agy-step-5")));
        let wake_pos = rows.iter().position(|r| matches!(r, ChatRow::Wake { id, summary, at: Some(_) }
            if id == "agy-step-6" && summary == "Background task \"Render story\" completed")).expect("expected wake row");
        assert!(matches!(&rows[wake_pos + 1], ChatRow::Tool { id, name, status: "running", .. } if id == "agy-step-8" && name == "Bash"));
        assert!(matches!(&rows[wake_pos + 2], ChatRow::Assistant { id, text, .. } if id == "agy-step-9" && text.contains("3/10 complete")));
        // Export events intentionally exclude synthetic Wake separators.
        assert_eq!(events(&content).iter().filter(|e| matches!(e, crate::agent::export::Event::AssistantText { .. })).count(), 2);
    }

    #[test]
    fn antigravity_permission_and_acp_modes_map_bidirectionally() {
        assert_eq!(acp_mode_for_permission("plan"), "plan");
        assert_eq!(acp_mode_for_permission("default"), "default");
        assert_eq!(acp_mode_for_permission("acceptEdits"), "accept-edits");
        assert_eq!(acp_mode_for_permission("accept-edits"), "accept-edits");
        assert_eq!(acp_mode_for_permission("auto"), "auto");
        assert_eq!(acp_mode_for_permission("bypassPermissions"), "auto");
        assert_eq!(acp_mode_for_permission("skip"), "auto");

        assert_eq!(permission_for_acp_mode("plan", "default"), "plan");
        assert_eq!(permission_for_acp_mode("accept-edits", "default"), "acceptEdits");
        assert_eq!(permission_for_acp_mode("acceptEdits", "default"), "acceptEdits");
        assert_eq!(permission_for_acp_mode("default", "auto"), "default");
        assert_eq!(permission_for_acp_mode("auto", "default"), "auto");
        assert_eq!(permission_for_acp_mode("auto", "bypassPermissions"), "bypassPermissions");
    }

    #[test]
    fn ask_question_step_pairs_without_shifting_following_tools() {
        let lines = [
            json!({"step_index":0,"type":"USER_INPUT","content":"<USER_REQUEST>configure</USER_REQUEST>"}),
            json!({"step_index":1,"type":"PLANNER_RESPONSE","tool_calls":[
                {"name":"ask_question","args":{"questions":[{"question":"Which port?","is_multi_select":false,"options":[{"id":"1","text":"8080"}]}]}},
                {"name":"view_file","args":{"AbsolutePath":"/tmp/cfg"}}
            ]}),
            json!({"step_index":2,"type":"ASK_QUESTION","status":"DONE","content":"Created At: 2026-11-16T10:00:00+08:00\nCompleted At: 2026-11-16T10:00:05+08:00\nA1: 8080"}),
            json!({"step_index":3,"type":"VIEW_FILE","status":"DONE","content":"port=8080"}),
        ];
        let rows = history_rows(&lines.iter().map(Value::to_string).collect::<Vec<_>>().join("\n"));
        assert_eq!(rows.len(), 3);
        let ChatRow::Tool { id, name, input, output, status, .. } = &rows[1] else { panic!() };
        assert_eq!(id, "agy-step-2");
        assert_eq!(name, "AskUserQuestion");
        assert_eq!(*status, "completed");
        assert_eq!(output.as_deref(), Some("A1: 8080"));
        assert_eq!(input["questions"][0]["header"], "Q1");
        assert_eq!(input["answers"], json!({"Which port?": "8080"}));
        let ChatRow::Tool { id: next_id, name: next_name, output: next_out, .. } = &rows[2] else { panic!() };
        assert_eq!(next_id, "agy-step-3");
        assert_eq!(next_name, "Read");
        assert_eq!(next_out.as_deref(), Some("port=8080"));
    }

    #[test]
    fn oauth_401_helpers_and_history_replay_hide_recovered_401_prompts() {
        let chunk = "\n⚠ UNAUTHENTICATED (code 401): Request had invalid authentication credentials. Expected OAuth 2 access token, login cookie or other valid authentication credential.\n";
        let extracted = strip_streamed_401_warning(chunk).expect("401 warning should be recognized");
        assert!(extracted.starts_with("UNAUTHENTICATED (code 401):"));
        assert!(is_oauth_401_error(&extracted));
        assert!(strip_streamed_401_warning("\n⚠ Command exited with code 1\n").is_none());
        assert!(user_request(&format!("<USER_REQUEST>{AUTH_RECOVERY_PROMPT}</USER_REQUEST>")).is_none());

        let recovered = [
            json!({"step_index":0,"type":"USER_INPUT","content":"<USER_REQUEST>Submit 6 declarations</USER_REQUEST>"}),
            json!({"step_index":1,"type":"PLANNER_RESPONSE","status":"DONE","content":"Submitting 6 declarations..."}),
            json!({"step_index":2,"type":"ERROR_MESSAGE","status":"DONE","content":extracted}),
            json!({"step_index":3,"type":"USER_INPUT","content":format!("<USER_REQUEST>{AUTH_RECOVERY_PROMPT}</USER_REQUEST>")}),
            json!({"step_index":4,"type":"PLANNER_RESPONSE","status":"DONE","content":"All 6 declarations submitted."}),
        ];
        let rows = history_rows(&recovered.iter().map(Value::to_string).collect::<Vec<_>>().join("\n"));
        assert_eq!(rows.len(), 3, "recovered 401 error and internal continuation prompt must stay hidden: {rows:?}");
        assert!(matches!(&rows[0], ChatRow::User { text, .. } if text == "Submit 6 declarations"));
        assert!(matches!(&rows[1], ChatRow::Assistant { text, .. } if text == "Submitting 6 declarations..."));
        assert!(matches!(&rows[2], ChatRow::Assistant { text, .. } if text == "All 6 declarations submitted."));

        // An unrecovered trailing 401 remains visible as an Error row.
        let unrecovered = &recovered[..3];
        let err_rows = history_rows(&unrecovered.iter().map(Value::to_string).collect::<Vec<_>>().join("\n"));
        assert_eq!(err_rows.len(), 3);
        assert!(matches!(&err_rows[2], ChatRow::Error { message, .. } if is_oauth_401_error(message)));
    }

    #[test]
    fn transient_upstream_helpers_and_history_replay_hide_recovered_transient_prompts() {
        let not_found = "\n⚠ NOT_FOUND (code 404): Requested entity was not found.\n";
        let extracted = strip_streamed_transient_warning(not_found).expect("404 transient warning should be recognized");
        assert_eq!(extracted, "NOT_FOUND (code 404): Requested entity was not found.");
        assert!(is_transient_upstream_error(&extracted));

        assert_eq!(
            strip_streamed_transient_warning("\n⚠ INTERNAL (code 500): Internal error encountered.\nError ID: abc-123\n").as_deref(),
            Some("INTERNAL (code 500): Internal error encountered."),
        );
        assert_eq!(
            strip_streamed_transient_warning("\n⚠ UNAVAILABLE (code 503): Service unavailable.\n").as_deref(),
            Some("UNAVAILABLE (code 503): Service unavailable."),
        );
        assert_eq!(
            strip_streamed_transient_warning("\n⚠ DEADLINE_EXCEEDED (code 504): Deadline exceeded.\n").as_deref(),
            Some("DEADLINE_EXCEEDED (code 504): Deadline exceeded."),
        );
        assert_eq!(
            strip_streamed_transient_warning("\n⚠ request failed: Post \"https://daily-cloudcode-pa.googleapis.com/v1internal:streamGenerateContent?alt=sse\": EOF\n").as_deref(),
            Some("request failed: Post \"https://daily-cloudcode-pa.googleapis.com/v1internal:streamGenerateContent?alt=sse\": EOF"),
        );
        assert!(is_transient_upstream_error("request failed: Post \"https://example.com\": read tcp 10.0.0.1:1234->1.2.3.4:443: read: connection reset by peer"));

        // Non-transient, 401, 429, cold-model, and prose lines must not match.
        assert!(strip_streamed_transient_warning("\n⚠ UNAUTHENTICATED (code 401): Request had invalid authentication credentials.\n").is_none());
        assert!(strip_streamed_transient_warning("\n⚠ RESOURCE_EXHAUSTED (code 429): Resource has been exhausted.\n").is_none());
        assert!(strip_streamed_transient_warning("\n⚠ failed to construct executor: unknown model key MODEL_PLACEHOLDER_M532: model not found\n").is_none());
        assert!(strip_streamed_transient_warning("\n⚠ NOT_FOUND (code 404): Requested entity was not found.\nHere is why that happened.").is_none());
        assert!(strip_streamed_transient_warning("\n⚠ Command exited with code 1\n").is_none());
        assert!(user_request(&format!("<USER_REQUEST>{TRANSIENT_RECOVERY_PROMPT}</USER_REQUEST>")).is_none());

        let recovered = [
            json!({"step_index":0,"type":"USER_INPUT","content":"<USER_REQUEST>Check readiness</USER_REQUEST>"}),
            json!({"step_index":1,"type":"PLANNER_RESPONSE","status":"DONE","content":"Reading readiness.rs..."}),
            json!({"step_index":2,"type":"ERROR_MESSAGE","status":"DONE","content":extracted}),
            json!({"step_index":3,"type":"USER_INPUT","content":format!("<USER_REQUEST>{TRANSIENT_RECOVERY_PROMPT}</USER_REQUEST>")}),
            json!({"step_index":4,"type":"PLANNER_RESPONSE","status":"DONE","content":"Readiness check complete."}),
        ];
        let rows = history_rows(&recovered.iter().map(Value::to_string).collect::<Vec<_>>().join("\n"));
        assert_eq!(rows.len(), 3, "recovered transient error and internal continuation prompt must stay hidden: {rows:?}");
        assert!(matches!(&rows[0], ChatRow::User { text, .. } if text == "Check readiness"));
        assert!(matches!(&rows[1], ChatRow::Assistant { text, .. } if text == "Reading readiness.rs..."));
        assert!(matches!(&rows[2], ChatRow::Assistant { text, .. } if text == "Readiness check complete."));

        // An unrecovered trailing transient error remains visible as an Error row.
        let unrecovered = &recovered[..3];
        let err_rows = history_rows(&unrecovered.iter().map(Value::to_string).collect::<Vec<_>>().join("\n"));
        assert_eq!(err_rows.len(), 3);
        assert!(matches!(&err_rows[2], ChatRow::Error { message, .. } if is_transient_upstream_error(message)));
    }

    #[test]
    fn recognizes_generic_network_issue_502_thinking_signature_and_usage_reset() {
        let generic_net = "\n⚠ There was a network issue connecting to the server, please try again.\n";
        assert_eq!(
            strip_streamed_transient_warning(generic_net).as_deref(),
            Some("There was a network issue connecting to the server, please try again."),
        );
        assert!(is_transient_upstream_error(
            "Error 502, Message: Bad Gateway: Failed to connect to backend API, Status: 502 Bad Gateway"
        ));
        assert!(is_invalid_thinking_signature(
            r#"Error 400, Message: {"type":"error","error":{"message":"messages.1.content.0.thinking.signature: Field required"}}"#
        ));
        assert!(is_invalid_thinking_signature(
            r#"messages.3.content.0: Invalid `signature` in `thinking` block"#
        ));
        assert!(!is_transient_upstream_error(
            r#"messages.3.content.0: Invalid `signature` in `thinking` block"#
        ));

        let ts_reset = parse_usage_limit_reset(
            "metadata:map[quotaResetDelay:3600s quotaResetTimeStamp:2026-11-09T01:05:00Z] reason:QUOTA_EXHAUSTED]",
            Some(1_700_000_000),
        )
        .unwrap();
        assert_eq!(ts_reset.resets_at, 1_794_186_300);

        let delay_reset = parse_usage_limit_reset(
            "Details: [map[retryDelay:1m30s] map[quotaResetTimeStamp:invalid]]",
            Some(10_000),
        )
        .unwrap();
        assert_eq!(delay_reset.resets_at, 10_090);
        assert_eq!(delay_reset.limit_type.as_deref(), Some("five_hour"));
    }

    #[test]
    fn history_rows_and_events_preserve_both_segments_after_step_index_reset() {
        let lines = [
            json!({"step_index":0,"type":"USER_INPUT","created_at":"2026-11-23T10:00:00Z","content":"<USER_REQUEST>before reset</USER_REQUEST>"}),
            json!({"step_index":1,"type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-11-23T10:00:01Z","content":"first answer"}),
            json!({"step_index":0,"type":"USER_INPUT","created_at":"2026-11-23T11:00:00Z","content":"<USER_REQUEST>after reset</USER_REQUEST>\n<USER_SETTINGS_CHANGE>Model Selection</USER_SETTINGS_CHANGE>"}),
            json!({"step_index":1,"type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-11-23T11:00:01Z","content":"second answer"}),
        ];
        let raw = lines.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
        let rows = history_rows(&raw);
        assert_eq!(rows.len(), 4);
        assert!(matches!(&rows[0], ChatRow::User { id, text, .. } if id == "agy-step-0" && text == "before reset"));
        assert!(matches!(&rows[1], ChatRow::Assistant { id, text, .. } if id == "agy-step-1" && text == "first answer"));
        assert!(matches!(&rows[2], ChatRow::User { id, text, .. } if id == "agy-step-s1-0" && text == "after reset"));
        assert!(matches!(&rows[3], ChatRow::Assistant { id, text, .. } if id == "agy-step-s1-1" && text == "second answer"));

        let exported = events(&raw);
        assert_eq!(exported.len(), 4);
        assert!(matches!(&exported[0], crate::agent::export::Event::User { text, ts } if text == "before reset" && ts.as_deref() == Some("2026-11-23T10:00:00Z")));
        assert!(matches!(&exported[2], crate::agent::export::Event::User { text, ts } if text == "after reset" && ts.as_deref() == Some("2026-11-23T11:00:00Z")));
    }
}
