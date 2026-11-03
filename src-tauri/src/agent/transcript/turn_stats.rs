//! Provider-reported turn accounting. Read complete histories once, then consume only appended lines.
//! Token usage is deduplicated by message/response identity; absent usage is never represented as zero.
//! Claude, Codex and OpenCode are read from their own recordings; Pi and OMP keep one JSONL tree each,
//! whose active branch supplies the same numbers. ZCode keeps its own usage tables; see `agent::zcode_info`.

use super::{resume, AgentContextInfo};
use crate::agent::opencode_store::{self, OpencodeMessage};
use crate::models::SessionKind;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TurnStats {
    /// Model the latest context reading came from, when the provider names one.
    pub model: Option<String>,
    pub tokens: Option<u64>,
    pub input_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cached_tokens: Option<u64>,
    pub cache_hit_percent: Option<f64>,
    pub session_tokens: Option<u64>,
    pub tools_used: Option<u32>,
    /// Successful, explicitly recorded file edits, not the working tree's pre-existing changes.
    pub files_touched: Option<u32>,
    /// The files behind `files_touched`, sorted by path.
    pub changed_files: Option<Vec<ChangedFile>>,
    pub context_tokens: Option<u64>,
    pub context_limit: Option<u64>,
    pub context_percent: Option<f64>,
    /// Estimated output throughput over measured response time, including reported reasoning.
    pub generation_tokens_per_second: Option<f64>,
}
/// One file the turn edited, with the lines its recorded edits added and removed. The counts are summed
/// over every edit to the file, and both are absent when any of those edits recorded nothing countable.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    /// As the agent wrote it: absolute, or relative to the session folder.
    pub path: String,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
}

/// Lines added and removed by one edit, or by every edit to one file; `None` when not countable.
type Lines = Option<(u32, u32)>;

/// Records one successful edit, keeping a file's counts unknown once any of its edits is.
fn add_file(files: &mut HashMap<String, Lines>, path: String, lines: Lines) {
    files
        .entry(path)
        .and_modify(|sum| *sum = sum.zip(lines).map(|(a, b)| (a.0 + b.0, a.1 + b.1)))
        .or_insert(lines);
}

fn changed_list(files: &HashMap<String, Lines>) -> Vec<ChangedFile> {
    let mut list: Vec<ChangedFile> = files
        .iter()
        .map(|(path, lines)| ChangedFile {
            path: path.clone(),
            additions: lines.map(|l| l.0),
            deletions: lines.map(|l| l.1),
        })
        .collect();
    list.sort_by(|a, b| a.path.cmp(&b.path));
    list
}

/// Counts `+` and `-` lines of a diff body, skipping unified-diff file headers.
fn diff_lines<'a>(lines: impl Iterator<Item = &'a str>) -> (u32, u32) {
    lines.fold((0, 0), |(added, removed), line| {
        if line.starts_with('+') && !line.starts_with("+++") {
            (added + 1, removed)
        } else if line.starts_with('-') && !line.starts_with("---") {
            (added, removed + 1)
        } else {
            (added, removed)
        }
    })
}

/// Claude's own record of an Edit/Write result: hunks for a change, full content for a new file.
fn claude_result_lines(result: Option<&Value>) -> Lines {
    let result = result?;
    let hunks = result.get("structuredPatch")?.as_array()?;
    if result.get("type").and_then(Value::as_str) == Some("create") {
        let content = result.get("content")?.as_str()?;
        return Some((content.lines().count() as u32, 0));
    }
    Some(diff_lines(
        hunks
            .iter()
            .filter_map(|h| h.get("lines").and_then(Value::as_array))
            .flatten()
            .filter_map(Value::as_str),
    ))
}

impl TurnStats {
    pub fn with_context(&mut self, context: &AgentContextInfo) {
        self.model = context.model.clone();
        self.context_tokens = context.context_tokens;
        self.context_limit = (context.context_limit > 0).then_some(context.context_limit);
        self.context_percent = self
            .context_tokens
            .zip(self.context_limit)
            .map(|(used, limit)| used as f64 * 100.0 / limit as f64);
    }
}

#[derive(Clone, Copy, Debug)]
struct Usage {
    input: u64,
    output: u64,
    cached: u64,
    cache_known: bool,
}
impl Default for Usage {
    fn default() -> Self {
        Self {
            input: 0,
            output: 0,
            cached: 0,
            cache_known: true,
        }
    }
}
impl Usage {
    fn parse(value: &Value, claude: bool) -> Option<Self> {
        let input = value.get("input_tokens")?.as_u64()?;
        let output = value.get("output_tokens")?.as_u64()?;
        let cache_value = value
            .get(if claude {
                "cache_read_input_tokens"
            } else {
                "cached_input_tokens"
            })
            .and_then(Value::as_u64);
        let cached = cache_value.unwrap_or(0);
        let created = value
            .get("cache_creation_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        Some(Self {
            input: if claude {
                input.saturating_add(cached).saturating_add(created)
            } else {
                input
            },
            output,
            cached,
            cache_known: cache_value.is_some(),
        })
    }
    fn total(self) -> u64 {
        self.input.saturating_add(self.output)
    }
    fn add(&mut self, other: Self) {
        self.input = self.input.saturating_add(other.input);
        self.output = self.output.saturating_add(other.output);
        self.cached = self.cached.saturating_add(other.cached);
        self.cache_known &= other.cache_known;
    }
    fn difference(self, base: Self) -> Self {
        Self {
            input: self.input.saturating_sub(base.input),
            output: self.output.saturating_sub(base.output),
            cached: self.cached.saturating_sub(base.cached),
            cache_known: self.cache_known && base.cache_known,
        }
    }
}

#[derive(Default)]
struct Accounting {
    messages: HashMap<String, Usage>,
    turn_messages: HashSet<String>,
    tools: HashSet<String>,
    pending_files: HashMap<String, Vec<(String, Lines)>>,
    files: HashMap<String, Lines>,
    session: Option<Usage>,
    baseline: Option<Usage>,
    turn: Option<Usage>,
    native_turn: Option<String>,
    native_turn_usage: bool,
    has_turn: bool,
    line_number: u64,
}
impl Accounting {
    fn start(&mut self) {
        self.baseline = self.session;
        self.turn = None;
        self.native_turn_usage = false;
        self.turn_messages.clear();
        self.tools.clear();
        self.pending_files.clear();
        self.files.clear();
        self.has_turn = true;
    }
    fn consume(&mut self, line: &str, kind: SessionKind) {
        self.line_number += 1;
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            return;
        };
        if kind == SessionKind::Claude {
            self.claude(&v, line);
        } else {
            self.codex(&v, line);
        }
    }
    fn claude(&mut self, v: &Value, line: &str) {
        if v.get("isSidechain").and_then(Value::as_bool) == Some(true) {
            return;
        }
        if super::parse_claude_line(line).is_some_and(|p| p.role == "user") {
            self.start();
        }
        let Some(message) = v.get("message") else {
            return;
        };
        let blocks = message.get("content").and_then(Value::as_array);
        if v.get("type").and_then(Value::as_str) == Some("user") {
            let results: Vec<&Value> = blocks
                .into_iter()
                .flatten()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_result"))
                .collect();
            // The row-level `toolUseResult` belongs to the row's tool result, so it is only
            // attributable when the row carries exactly one.
            let recorded = (results.len() == 1).then(|| v.get("toolUseResult")).flatten();
            for block in results {
                let id = block
                    .get("tool_use_id")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if let Some(edits) = self.pending_files.remove(id) {
                    if block.get("is_error").and_then(Value::as_bool) != Some(true) {
                        let lines = claude_result_lines(recorded);
                        for (path, _) in edits {
                            add_file(&mut self.files, path, lines);
                        }
                    }
                }
            }
            return;
        }
        if v.get("type").and_then(Value::as_str) != Some("assistant")
            || message.get("model").and_then(Value::as_str) == Some("<synthetic>")
        {
            return;
        }
        let id = message
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("line-{}", self.line_number));
        if let Some(usage) = message.get("usage").and_then(|u| Usage::parse(u, true)) {
            let previous = self.messages.insert(id.clone(), usage).unwrap_or_default();
            let mut session = self.session.unwrap_or_default().difference(previous);
            session.add(usage);
            self.session = Some(session);
            self.turn_messages.insert(id.clone());
            let mut turn = Usage::default();
            for message_id in &self.turn_messages {
                if let Some(usage) = self.messages.get(message_id) {
                    turn.add(*usage);
                }
            }
            self.turn = Some(turn);
        }
        for (index, block) in blocks.into_iter().flatten().enumerate() {
            if block.get("type").and_then(Value::as_str) != Some("tool_use") {
                continue;
            }
            let call_id = block
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("{id}-{index}"));
            if !self.tools.insert(call_id.clone()) {
                continue;
            }
            let name = block.get("name").and_then(Value::as_str).unwrap_or("");
            if super::is_edit_tool(name) {
                if let Some(path) = block.get("input").and_then(super::tool_file_path) {
                    self.pending_files.insert(call_id, vec![(path, None)]);
                }
            }
        }
    }
    fn codex(&mut self, v: &Value, line: &str) {
        let Some(p) = v.get("payload") else { return };
        let row_type = v.get("type").and_then(Value::as_str).unwrap_or("");
        let kind = p.get("type").and_then(Value::as_str).unwrap_or("");
        if row_type == "event_msg" && kind == "task_started" {
            let id = p.get("turn_id").and_then(Value::as_str);
            if id.is_none() || self.native_turn.as_deref() != id {
                self.start();
                self.native_turn = Some(
                    id.map(str::to_string)
                        .unwrap_or_else(|| format!("legacy-{}", self.line_number)),
                );
            }
        } else if self.native_turn.is_none()
            && super::parse_codex_line(line).is_some_and(|p| p.role == "user")
        {
            self.start();
        }
        if row_type == "token_usage_record" {
            if let Some(usage) = p
                .get("thread_token_usage")
                .and_then(|u| Usage::parse(u, false))
            {
                self.session = Some(usage);
            }
            if let Some(usage) = p
                .get("turn_token_usage")
                .and_then(|u| Usage::parse(u, false))
            {
                self.turn = Some(usage);
                self.native_turn_usage = true;
            }
        }
        if row_type == "event_msg" && kind == "token_count" {
            if let Some(usage) = p
                .pointer("/info/total_token_usage")
                .and_then(|u| Usage::parse(u, false))
            {
                self.session = Some(usage);
                // Older CLIs expose only cumulative counters. A decreasing counter starts a new epoch.
                let base = self
                    .baseline
                    .filter(|b| b.input <= usage.input && b.output <= usage.output)
                    .unwrap_or_default();
                if !self.native_turn_usage {
                    self.turn = Some(usage.difference(base));
                }
            }
        }
        if row_type != "response_item" {
            return;
        }
        if matches!(
            kind,
            "function_call" | "custom_tool_call" | "local_shell_call" | "web_search_call"
        ) {
            let id = p
                .get("call_id")
                .or_else(|| p.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("line-{}", self.line_number));
            if !self.tools.insert(id.clone()) {
                return;
            }
            let name = p.get("name").and_then(Value::as_str).unwrap_or("");
            let raw = p
                .get("input")
                .or_else(|| p.get("arguments"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if name.rsplit('.').next() == Some("apply_patch") {
                let args = serde_json::from_str::<Value>(raw).ok();
                let patch = args
                    .as_ref()
                    .and_then(|a| a.get("patch").and_then(Value::as_str))
                    .unwrap_or(raw);
                self.pending_files.insert(id, patch_edits(patch));
            } else if name == "exec" && raw.contains("apply_patch") {
                let edits = patch_edits(&code_mode_patches(raw));
                if !edits.is_empty() {
                    self.pending_files.insert(id, edits);
                }
            }
        }
        if matches!(kind, "function_call_output" | "custom_tool_call_output") {
            let id = p.get("call_id").and_then(Value::as_str).unwrap_or("");
            if let Some(edits) = self.pending_files.remove(id) {
                let output = codex_output_text(p.get("output"));
                // A code-mode cell that throws reports "Script failed"; a completed cell applied
                // every patch it ran, since a failed patch throws.
                if output.starts_with("Script completed")
                    || !output.starts_with("Script ")
                        && (output.contains("Success. Updated the following files:")
                            || output.contains("\"exit_code\":0")
                            || output.contains("\"exit_code\": 0"))
                {
                    for (path, lines) in edits {
                        add_file(&mut self.files, path, lines);
                    }
                }
            }
        }
    }
    fn snapshot(&self) -> TurnStats {
        let turn = self.turn;
        TurnStats {
            tokens: turn.map(|u| u.output),
            input_tokens: turn.map(|u| u.input),
            total_tokens: turn.map(Usage::total),
            cached_tokens: turn.filter(|u| u.cache_known).map(|u| u.cached),
            cache_hit_percent: turn
                .filter(|u| u.cache_known && u.input > 0 && u.cached <= u.input)
                .map(|u| u.cached as f64 * 100.0 / u.input as f64),
            session_tokens: self.session.map(Usage::total),
            tools_used: self.has_turn.then_some(self.tools.len() as u32),
            files_touched: self.has_turn.then_some(self.files.len() as u32),
            changed_files: self.has_turn.then(|| changed_list(&self.files)),
            ..TurnStats::default()
        }
    }
}
/// Tool output as text: older CLIs store a string, code-mode cells a list of `input_text` items.
fn codex_output_text(output: Option<&Value>) -> String {
    match output {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// The files an apply_patch body touches, each with the `+`/`-` lines of its section. A deleted file's
/// removed lines are not in the patch, and a moved file's hunks belong to its destination, so the
/// deleted and the moved-away paths carry no counts.
fn patch_edits(patch: &str) -> Vec<(String, Lines)> {
    let mut edits: Vec<(String, Lines)> = Vec::new();
    for line in patch.lines() {
        let header = |prefix: &str| line.strip_prefix(prefix).map(str::trim).filter(|p| !p.is_empty());
        if let Some(path) = header("*** Update File: ").or_else(|| header("*** Add File: ")) {
            edits.push((path.to_string(), Some((0, 0))));
        } else if let Some(path) = header("*** Delete File: ") {
            edits.push((path.to_string(), None));
        } else if let Some(path) = header("*** Move to: ") {
            if let Some(source) = edits.last_mut() {
                let lines = source.1.take();
                edits.push((path.to_string(), lines));
            }
        } else if line.starts_with("***") {
            continue;
        } else if let Some((_, Some(lines))) = edits.last_mut() {
            if line.starts_with('+') {
                lines.0 += 1;
            } else if line.starts_with('-') {
                lines.1 += 1;
            }
        }
    }
    edits
}

/// Patch bodies from a code-mode cell, which runs apply_patch from JavaScript, so each patch is a string
/// literal in the script source, passed directly or through a variable. Double-quoted literals decode as
/// JSON strings and template literals are taken as written; anything else falls back to undoing `\n`
/// escapes across the whole source.
fn code_mode_patches(source: &str) -> String {
    let mut patches = Vec::new();
    for (at, _) in source.match_indices("*** Begin Patch") {
        let Some(quote) = source[..at].chars().next_back().filter(|c| matches!(c, '"' | '`')) else {
            continue;
        };
        let rest = &source[at - 1..];
        let mut escaped = false;
        let end = rest.char_indices().skip(1).find_map(|(i, c)| {
            let close = !escaped && c == quote;
            escaped = !escaped && c == '\\';
            close.then_some(i)
        });
        let Some(end) = end else { continue };
        if quote == '"' {
            if let Ok(text) = serde_json::from_str::<String>(&rest[..=end]) {
                patches.push(text);
            }
        } else {
            patches.push(rest[1..end].replace("\\`", "`"));
        }
    }
    if patches.is_empty() {
        source.replace("\\n", "\n")
    } else {
        patches.join("\n")
    }
}

#[derive(Default)]
struct Cached {
    path: PathBuf,
    modified: Option<SystemTime>,
    offset: u64,
    accounting: Accounting,
}
static CACHE: OnceLock<Mutex<HashMap<String, Cached>>> = OnceLock::new();

/// Rebuilds OpenCode's turn accounting from its own SQLite store.
///
/// OpenCode records one assistant message per model step, each carrying the step's token counts and a
/// created/completed window, so no stream interception is needed: the last user message marks the
/// turn, and each assistant message under it is one step of that turn.
fn opencode_stats_from_messages(messages: &[OpencodeMessage]) -> TurnStats {
    let last_user = messages
        .iter()
        .rposition(|m| m.info.get("role").and_then(Value::as_str) == Some("user"));
    let turn_id = last_user.and_then(|i| messages[i].info.get("id").and_then(Value::as_str));
    let mut session = Usage::default();
    let mut session_known = false;
    let mut turn = Usage::default();
    let mut turn_known = false;
    let mut tools: HashSet<String> = HashSet::new();
    // Edit and write calls carry exact line counts. Snapshot patches also catch shell edits but miss
    // files outside the snapshot repository or ignored by it, so they only add files the calls missed.
    let mut files: HashMap<String, Lines> = HashMap::new();
    let mut patched: HashSet<String> = HashSet::new();
    let mut generated_tokens = 0u64;
    let mut generated_ms = 0u64;
    for (index, message) in messages.iter().enumerate() {
        let info = &message.info;
        if info.get("role").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let Some(tokens) = info.get("tokens") else { continue };
        let usage = opencode_usage(tokens);
        if tokens.get("total").and_then(Value::as_u64).is_some() {
            session.add(usage);
            session_known = true;
        }
        // The legacy store links each step to its user message. The v2 store records no parent but
        // keeps messages in order, so there a step belongs to the turn when it follows the last user
        // message.
        let in_turn = match info.get("parentID").and_then(Value::as_str) {
            Some(parent) => turn_id.is_none_or(|id| parent == id),
            None => last_user.is_none_or(|i| index > i),
        };
        if !in_turn {
            continue;
        }
        turn.add(usage);
        turn_known = true;
        if let Some((ms, produced)) = opencode_step(info, tokens) {
            generated_ms = generated_ms.saturating_add(ms);
            generated_tokens = generated_tokens.saturating_add(produced);
        }
        for part in &message.parts {
            match part.get("type").and_then(Value::as_str) {
                Some("tool") => {
                    let id = part
                        .get("callID")
                        .or_else(|| part.get("id"))
                        .and_then(Value::as_str)
                        .unwrap_or("tool")
                        .to_string();
                    tools.insert(id);
                    if let Some((path, lines)) = opencode_edit(part) {
                        add_file(&mut files, path, lines);
                    }
                }
                Some("patch") => {
                    for file in part.get("files").and_then(Value::as_array).into_iter().flatten() {
                        if let Some(path) = file.as_str() {
                            patched.insert(path.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }
    for path in patched {
        files.entry(path).or_insert(None);
    }
    TurnStats {
        tokens: turn_known.then_some(turn.output),
        input_tokens: turn_known.then_some(turn.input),
        total_tokens: turn_known.then_some(turn.total()),
        cached_tokens: (turn.cache_known && turn.cached > 0).then_some(turn.cached),
        cache_hit_percent: (turn.cache_known && turn.input > 0 && turn.cached <= turn.input)
            .then(|| turn.cached as f64 * 100.0 / turn.input as f64),
        session_tokens: session_known.then_some(session.total()),
        tools_used: turn_known.then_some(tools.len() as u32),
        files_touched: turn_known.then_some(files.len() as u32),
        changed_files: turn_known.then(|| changed_list(&files)),
        generation_tokens_per_second: (generated_ms > 0 && generated_tokens > 0)
            .then(|| generated_tokens as f64 * 1000.0 / generated_ms as f64),
        ..TurnStats::default()
    }
}

/// A completed OpenCode edit or write call: its file, and the lines OpenCode measured for an edit or
/// counted for a write that created the file. Overwriting an existing file records no diff.
fn opencode_edit(part: &Value) -> Option<(String, Lines)> {
    let tool = part.get("tool").and_then(Value::as_str)?;
    let state = part.get("state")?;
    if !matches!(tool, "edit" | "write") || state.get("status").and_then(Value::as_str) != Some("completed") {
        return None;
    }
    let input = state.get("input")?;
    let path = input.get("filePath").and_then(Value::as_str)?.to_string();
    let metadata = state.get("metadata");
    let lines = if tool == "edit" {
        let diff = metadata.and_then(|m| m.get("filediff"));
        let count = |key: &str| diff.and_then(|d| d.get(key)).and_then(Value::as_u64).map(|n| n as u32);
        count("additions").zip(count("deletions"))
    } else if metadata.and_then(|m| m.get("exists")).and_then(Value::as_bool) == Some(false) {
        input.get("content").and_then(Value::as_str).map(|c| (c.lines().count() as u32, 0))
    } else {
        None
    };
    Some((path, lines))
}

/// OpenCode's `tokens` object: uncached input, output, reasoning, and the cache read/write pair. Input
/// is widened to include cache traffic, matching the Claude accounting the panel already shows.
fn opencode_usage(tokens: &Value) -> Usage {
    let input = tokens.get("input").and_then(Value::as_u64).unwrap_or(0);
    let output = tokens.get("output").and_then(Value::as_u64).unwrap_or(0);
    let reasoning = tokens.get("reasoning").and_then(Value::as_u64).unwrap_or(0);
    let cached = tokens.pointer("/cache/read").and_then(Value::as_u64).unwrap_or(0);
    let created = tokens.pointer("/cache/write").and_then(Value::as_u64).unwrap_or(0);
    Usage {
        input: input.saturating_add(cached).saturating_add(created),
        output: output.saturating_add(reasoning),
        cached,
        cache_known: tokens.pointer("/cache/read").is_some(),
    }
}

/// One assistant message is one model step, so its created/completed window measures generation rather
/// than the surrounding tool work. Returns the elapsed milliseconds and the tokens produced.
fn opencode_step(info: &Value, tokens: &Value) -> Option<(u64, u64)> {
    let created = info.pointer("/time/created").and_then(Value::as_u64)?;
    let completed = info.pointer("/time/completed").and_then(Value::as_u64)?;
    let produced = tokens.get("output").and_then(Value::as_u64).unwrap_or(0)
        .saturating_add(tokens.get("reasoning").and_then(Value::as_u64).unwrap_or(0));
    (completed > created && produced > 0).then_some((completed - created, produced))
}

/// Pi and OMP record one `usage` object per assistant message: uncached input, output with reasoning
/// already included, and the cache read/write pair. As with Claude, input is widened to include cache
/// traffic, so the panel's input, cache-hit and turn-token figures mean the same thing for every agent.
fn pi_usage(usage: &Value) -> Option<Usage> {
    let input = usage.get("input")?.as_u64()?;
    let output = usage.get("output")?.as_u64()?;
    let cache_read = usage.get("cacheRead").and_then(Value::as_u64);
    let cached = cache_read.unwrap_or(0);
    let created = usage.get("cacheWrite").and_then(Value::as_u64).unwrap_or(0);
    Some(Usage {
        input: input.saturating_add(cached).saturating_add(created),
        output,
        cached,
        cache_known: cache_read.is_some(),
    })
}

/// OMP stamps each assistant message with the wall-clock milliseconds its stream took; that is the same
/// window Claude's message_start/message_stop measures, so the two rates stay comparable. Upstream Pi
/// records no timing at all, which leaves the rate to the live engine while the process runs.
fn pi_step_ms(message: &Value) -> Option<u64> {
    let ms = message.get("duration").and_then(Value::as_f64)?;
    (ms > 0.0).then_some(ms as u64)
}

/// The file-editing tools Pi and OMP expose. Both write through lowercase `edit` and `write`; OMP adds the
/// AST rewrite. Matching is by exact name, so a model inventing a differently-cased tool is not counted.
fn is_pi_edit_tool(name: &str) -> bool {
    matches!(name, "edit" | "write" | "ast_edit" | "apply_patch")
}

/// Turn accounting for one Pi or OMP recording.
///
/// A turn begins at the last user message and consists of every assistant message after it, one per model
/// step. Session totals accumulate all assistant messages seen, including ones later abandoned by a fork,
/// because those tokens were still spent.
#[derive(Default)]
struct PiAccounting {
    messages: HashMap<String, Usage>,
    turn_messages: HashSet<String>,
    tools: HashSet<String>,
    pending_files: HashMap<String, Vec<(String, Lines)>>,
    files: HashMap<String, Lines>,
    session: Option<Usage>,
    turn: Option<Usage>,
    generated_ms: u64,
    generated_tokens: u64,
    has_turn: bool,
    /// Id of the last entry consumed, which the next appended entry has to name as its parent.
    tail: Option<String>,
}
impl PiAccounting {
    fn start(&mut self) {
        self.turn = None;
        self.turn_messages.clear();
        self.tools.clear();
        self.pending_files.clear();
        self.files.clear();
        self.generated_ms = 0;
        self.generated_tokens = 0;
        self.has_turn = true;
    }
    fn consume(&mut self, entry: &Value) {
        if let Some(id) = entry.get("id").and_then(Value::as_str) {
            self.tail = Some(id.to_string());
        }
        if entry.get("type").and_then(Value::as_str) != Some("message") {
            return;
        }
        let Some(id) = entry.get("id").and_then(Value::as_str) else {
            return;
        };
        let Some(message) = entry.get("message") else {
            return;
        };
        match message.get("role").and_then(Value::as_str) {
            Some("user") => self.start(),
            Some("assistant") => self.assistant(id, message),
            Some("toolResult") => self.tool_result(message),
            _ => {}
        }
    }
    fn assistant(&mut self, id: &str, message: &Value) {
        if let Some(usage) = message.get("usage").and_then(pi_usage) {
            let previous = self.messages.insert(id.to_string(), usage).unwrap_or_default();
            let mut session = self.session.unwrap_or_default().difference(previous);
            session.add(usage);
            self.session = Some(session);
            if self.has_turn {
                self.turn_messages.insert(id.to_string());
                let mut turn = Usage::default();
                for message_id in &self.turn_messages {
                    if let Some(usage) = self.messages.get(message_id) {
                        turn.add(*usage);
                    }
                }
                self.turn = Some(turn);
                if let Some(ms) = pi_step_ms(message).filter(|_| usage.output > 0) {
                    self.generated_ms = self.generated_ms.saturating_add(ms);
                    self.generated_tokens = self.generated_tokens.saturating_add(usage.output);
                }
            }
        }
        let Some(blocks) = message.get("content").and_then(Value::as_array) else {
            return;
        };
        for block in blocks {
            if block.get("type").and_then(Value::as_str) != Some("toolCall") {
                continue;
            }
            let Some(call_id) = block.get("id").and_then(Value::as_str) else {
                continue;
            };
            if !self.tools.insert(call_id.to_string()) {
                continue;
            }
            let name = block.get("name").and_then(Value::as_str).unwrap_or("");
            if is_pi_edit_tool(name) {
                if let Some(path) = block.get("arguments").and_then(super::tool_file_path) {
                    self.pending_files.insert(call_id.to_string(), vec![(path, None)]);
                }
            }
        }
    }
    fn tool_result(&mut self, message: &Value) {
        let call_id = message
            .get("toolCallId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some(edits) = self.pending_files.remove(call_id) else {
            return;
        };
        if message.get("isError").and_then(Value::as_bool) != Some(true) {
            // Pi records a unified patch, OMP a diff whose changed lines start `+12|` / `-12|`.
            // Writes record neither, since they may have replaced an existing file.
            let details = message.get("details");
            let lines = details
                .and_then(|d| d.get("patch").or_else(|| d.get("diff")))
                .and_then(Value::as_str)
                .map(|diff| diff_lines(diff.lines()));
            for (path, _) in edits {
                add_file(&mut self.files, path, lines);
            }
        }
    }
    fn snapshot(&self) -> TurnStats {
        let turn = self.turn;
        TurnStats {
            tokens: turn.map(|u| u.output),
            input_tokens: turn.map(|u| u.input),
            total_tokens: turn.map(Usage::total),
            cached_tokens: turn.filter(|u| u.cache_known && u.cached > 0).map(|u| u.cached),
            cache_hit_percent: turn
                .filter(|u| u.cache_known && u.input > 0 && u.cached <= u.input)
                .map(|u| u.cached as f64 * 100.0 / u.input as f64),
            session_tokens: self.session.map(Usage::total),
            tools_used: self.has_turn.then_some(self.tools.len() as u32),
            files_touched: self.has_turn.then_some(self.files.len() as u32),
            changed_files: self.has_turn.then(|| changed_list(&self.files)),
            generation_tokens_per_second: (self.generated_ms > 0 && self.generated_tokens > 0)
                .then(|| self.generated_tokens as f64 * 1000.0 / self.generated_ms as f64),
            ..TurnStats::default()
        }
    }
}

/// Whether appended lines leave the branch the accounting already followed.
///
/// A fork or `/tree` jump writes its new entries under an earlier parent, which makes every count derived
/// from file order wrong; detecting that early lets the caller rebuild from the branch instead.
fn pi_branch_broken(tail: Option<&str>, suffix: &str) -> bool {
    let mut expected = tail.map(str::to_string);
    for line in suffix.lines() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(id) = entry.get("id").and_then(Value::as_str) else {
            continue;
        };
        if entry.get("parentId").and_then(Value::as_str) != expected.as_deref() {
            return true;
        }
        expected = Some(id.to_string());
    }
    false
}

#[derive(Default)]
struct PiCached {
    path: PathBuf,
    modified: Option<SystemTime>,
    offset: u64,
    accounting: PiAccounting,
}
static PI_CACHE: OnceLock<Mutex<HashMap<String, PiCached>>> = OnceLock::new();

/// Reads a Pi or OMP recording and consumes only the lines appended since the last read. A fork forces a
/// rebuild, because entries written under an earlier parent are not the continuation of the current branch.
fn pi_stats_from_file(kind: SessionKind, agent_session_id: &str) -> Result<TurnStats, String> {
    let key = format!("{kind:?}:{agent_session_id}");
    let mut cache = PI_CACHE.get_or_init(Default::default).lock().unwrap();
    if cache.len() >= 16 && !cache.contains_key(&key) {
        cache.clear();
    }
    let entry = cache.entry(key).or_default();
    if entry.path.as_os_str().is_empty() || !entry.path.is_file() {
        entry.path = resume::find_pi_session(kind, agent_session_id)
            .ok_or("The agent's session recording could not be found")?;
        entry.offset = 0;
        entry.modified = None;
        entry.accounting = PiAccounting::default();
    }
    pi_read_cached(entry)
}

fn pi_read_cached(entry: &mut PiCached) -> Result<TurnStats, String> {
    let metadata = std::fs::metadata(&entry.path).map_err(|e| e.to_string())?;
    let modified = metadata.modified().ok();
    if entry.modified == modified && entry.offset == metadata.len() {
        return Ok(entry.accounting.snapshot());
    }
    if metadata.len() <= entry.offset {
        // The recording was replaced rather than appended to; every count has to start over.
        entry.offset = 0;
        entry.accounting = PiAccounting::default();
    }
    let mut file = BufReader::new(std::fs::File::open(&entry.path).map_err(|e| e.to_string())?);
    file.seek(SeekFrom::Start(entry.offset))
        .map_err(|e| e.to_string())?;
    let mut suffix = String::new();
    let mut complete_bytes = 0;
    loop {
        let mut line = String::new();
        let n = file.read_line(&mut line).map_err(|e| e.to_string())?;
        if n == 0 || !line.ends_with('\n') {
            break;
        }
        complete_bytes += n as u64;
        suffix.push_str(&line);
    }
    if pi_branch_broken(entry.accounting.tail.as_deref(), &suffix) {
        let content = std::fs::read_to_string(&entry.path).map_err(|e| e.to_string())?;
        entry.accounting = PiAccounting::default();
        for message in resume::pi_active_branch(&content) {
            entry.accounting.consume(&message);
        }
    } else {
        for line in suffix.lines() {
            if let Ok(entry_value) = serde_json::from_str::<Value>(line) {
                entry.accounting.consume(&entry_value);
            }
        }
    }
    entry.offset += complete_bytes;
    entry.modified = modified;
    Ok(entry.accounting.snapshot())
}

pub fn current_turn_stats(kind: SessionKind, agent_session_id: &str) -> Result<TurnStats, String> {
    if kind == SessionKind::Kiro { return crate::agent::kiro_info::turn_stats(agent_session_id); }
    if kind == SessionKind::Zcode { return crate::agent::zcode_info::turn_stats(agent_session_id); }
    if kind == SessionKind::Opencode {
        return Ok(opencode_stats_from_messages(&opencode_store::messages(agent_session_id)?));
    }
    if matches!(kind, SessionKind::Pi | SessionKind::Omp) {
        return pi_stats_from_file(kind, agent_session_id);
    }
    if !matches!(kind, SessionKind::Claude | SessionKind::Codex) {
        return Ok(TurnStats::default());
    }
    let key = format!("{kind:?}:{agent_session_id}");
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    // Bound retained accounting state when many archived sessions are inspected.
    if cache.len() >= 16 && !cache.contains_key(&key) {
        cache.clear();
    }
    let entry = cache.entry(key).or_default();
    if entry.path.as_os_str().is_empty() || !entry.path.is_file() {
        entry.path = if kind == SessionKind::Claude {
            resume::find_claude_transcript(agent_session_id)
        } else {
            resume::find_codex_rollout(agent_session_id)
        }
        .ok_or("Agent transcript not found")?;
        entry.offset = 0;
        entry.modified = None;
    }
    read_cached(entry, kind)
}

fn read_cached(entry: &mut Cached, kind: SessionKind) -> Result<TurnStats, String> {
    let metadata = std::fs::metadata(&entry.path).map_err(|e| e.to_string())?;
    let modified = metadata.modified().ok();
    if entry.modified == modified && entry.offset == metadata.len() {
        return Ok(entry.accounting.snapshot());
    }
    if metadata.len() <= entry.offset {
        entry.offset = 0;
    }
    let mut file = BufReader::new(std::fs::File::open(&entry.path).map_err(|e| e.to_string())?);
    file.seek(SeekFrom::Start(entry.offset))
        .map_err(|e| e.to_string())?;
    let mut suffix = String::new();
    let mut complete_bytes = 0;
    loop {
        let mut line = String::new();
        let n = file.read_line(&mut line).map_err(|e| e.to_string())?;
        if n == 0 || !line.ends_with('\n') {
            break;
        }
        complete_bytes += n as u64;
        suffix.push_str(&line);
    }
    let rewound = kind == SessionKind::Claude
        && suffix.lines().any(|line| {
            serde_json::from_str::<Value>(line)
                .ok()
                .is_some_and(|v| v.get("rewound").and_then(Value::as_bool) == Some(true))
        });
    if entry.offset == 0 || rewound {
        if rewound && entry.offset != 0 {
            suffix = std::fs::read_to_string(&entry.path).map_err(|e| e.to_string())?;
            complete_bytes = suffix.rfind('\n').map_or(0, |i| i + 1) as u64;
            suffix.truncate(complete_bytes as usize);
        }
        entry.offset = 0;
        entry.accounting = Accounting::default();
        if kind == SessionKind::Claude {
            // Rewinding changes the active turn, but does not refund tokens already consumed.
            let mut all = Accounting::default();
            for line in suffix.lines() {
                all.consume(line, kind);
            }
            let active = resume::claude_active_branch(&suffix);
            for line in active.lines() {
                entry.accounting.consume(line, kind);
            }
            entry.accounting.messages = all.messages;
            entry.accounting.session = all.session;
            suffix.clear();
        }
    }
    for line in suffix.lines() {
        entry.accounting.consume(line, kind);
    }
    entry.offset += complete_bytes;
    entry.modified = modified;
    Ok(entry.accounting.snapshot())
}

#[cfg(test)]
pub(super) fn turn_stats_from_text(text: &str) -> TurnStats {
    parse(text, SessionKind::Claude)
}
#[cfg(test)]
fn parse(text: &str, kind: SessionKind) -> TurnStats {
    let mut accounting = Accounting::default();
    for line in text.lines() {
        accounting.consume(line, kind);
    }
    accounting.snapshot()
}

#[cfg(test)]
fn changed_file(path: &str, lines: Lines) -> ChangedFile {
    ChangedFile { path: path.to_string(), additions: lines.map(|l| l.0), deletions: lines.map(|l| l.1) }
}

/// Mirrors the file reader for a Pi or OMP recording: only the active branch is accounted.
#[cfg(test)]
fn pi_parse(text: &str) -> TurnStats {
    let mut accounting = PiAccounting::default();
    for entry in resume::pi_active_branch(text) {
        accounting.consume(&entry);
    }
    accounting.snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn lines(values: Vec<Value>) -> String {
        values
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }
    #[test]
    fn claude_counts_usage_once_and_only_successful_file_edits() {
        let message = json!({"type":"assistant","message":{"id":"m","usage":{"input_tokens":10,"output_tokens":50,"cache_read_input_tokens":80,"cache_creation_input_tokens":10},"content":[{"type":"tool_use","id":"write","name":"Write","input":{"file_path":"a.ts"}}]}});
        let text = lines(vec![
            json!({"type":"user","message":{"content":"go"}}),
            message.clone(),
            message,
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"write","content":"done"}]}}),
            json!({"type":"assistant","isSidechain":true,"message":{"id":"child","usage":{"input_tokens":1000,"output_tokens":1000}}}),
        ]);
        let stats = parse(&text, SessionKind::Claude);
        assert_eq!(stats.total_tokens, Some(150));
        assert_eq!(stats.cache_hit_percent, Some(80.0));
        assert_eq!(stats.tools_used, Some(1));
        assert_eq!(stats.files_touched, Some(1));
    }
    #[test]
    fn claude_updated_usage_replaces_previous_frames_and_resets_turn_only() {
        let text = lines(vec![
            json!({"type":"user","message":{"content":"first"}}),
            json!({"type":"assistant","message":{"id":"a","usage":{"input_tokens":100,"output_tokens":20}}}),
            json!({"type":"assistant","message":{"id":"a","usage":{"input_tokens":100,"output_tokens":40}}}),
            json!({"type":"user","message":{"content":"second"}}),
            json!({"type":"assistant","message":{"id":"b","usage":{"input_tokens":200,"output_tokens":60},"content":[{"type":"tool_use","id":"edit","name":"Edit","input":{"file_path":"a.ts"}}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"edit","is_error":true,"content":"failed"}]}}),
        ]);
        let stats = parse(&text, SessionKind::Claude);
        assert_eq!(stats.tokens, Some(60));
        assert_eq!(stats.total_tokens, Some(260));
        assert_eq!(stats.session_tokens, Some(400));
        assert_eq!(stats.files_touched, Some(0));
    }
    #[test]
    fn codex_prefers_native_turn_usage_and_never_adds_repeated_snapshots() {
        let usage = json!({"input_tokens":1000,"output_tokens":100,"cached_input_tokens":800});
        let record = json!({"type":"token_usage_record","payload":{"turn_token_usage":usage,"thread_token_usage":{"input_tokens":9000,"output_tokens":900,"cached_input_tokens":5000}}});
        let stats = parse(
            &lines(vec![
                json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"a"}}),
                record.clone(),
                record,
                json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":9000,"output_tokens":900,"cached_input_tokens":5000}}}}),
            ]),
            SessionKind::Codex,
        );
        assert_eq!(stats.total_tokens, Some(1100));
        assert_eq!(stats.session_tokens, Some(9900));
        assert_eq!(stats.cache_hit_percent, Some(80.0));
    }
    #[test]
    fn codex_legacy_usage_uses_difference_across_turns() {
        let count = |input, output| json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":input,"output_tokens":output}}}});
        let stats = parse(
            &lines(vec![
                json!({"type":"event_msg","payload":{"type":"task_started"}}),
                count(100, 10),
                json!({"type":"event_msg","payload":{"type":"task_started"}}),
                count(400, 40),
                count(400, 40),
            ]),
            SessionKind::Codex,
        );
        assert_eq!(stats.total_tokens, Some(330));
        assert_eq!(stats.session_tokens, Some(440));
    }
    #[test]
    fn codex_counts_calls_once_and_waits_for_successful_patch() {
        let call = json!({"type":"response_item","payload":{"type":"custom_tool_call","call_id":"c","name":"apply_patch","input":"*** Update File: a.ts\n*** Add File: b.ts"}});
        let mut accounting = Accounting::default();
        for line in lines(vec![
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"a"}}),
            call.clone(),
            call,
        ])
        .lines()
        {
            accounting.consume(line, SessionKind::Codex);
        }
        assert_eq!(accounting.snapshot().tools_used, Some(1));
        assert_eq!(accounting.snapshot().files_touched, Some(0));
        accounting.consume(&json!({"type":"response_item","payload":{"type":"custom_tool_call_output","call_id":"c","output":"Success. Updated the following files:\nM a.ts\nA b.ts"}}).to_string(), SessionKind::Codex);
        assert_eq!(accounting.snapshot().files_touched, Some(2));
        assert_eq!(accounting.snapshot().changed_files, Some(vec![changed_file("a.ts", Some((0, 0))), changed_file("b.ts", Some((0, 0)))]));
    }
    #[test]
    fn codex_code_mode_patches_count_only_when_the_cell_completes() {
        // The cell source holds the patch as a JS string literal with `\n` escapes, as recorded.
        let source = "text(await tools.apply_patch(\"*** Begin Patch\\n*** Update File: /r/b.ts\\n@@\\n-x\\n+y\\n*** Add File: /r/a.ts\\n+z\\n*** End Patch\\n\"));\n";
        let call = |id: &str| json!({"type":"response_item","payload":{"type":"custom_tool_call","name":"exec","call_id":id,"input":source}});
        let output = |id: &str, head: &str| json!({"type":"response_item","payload":{"type":"custom_tool_call_output","call_id":id,"output":[{"type":"input_text","text":format!("{head}\nWall time 0.1 seconds\nOutput:\n")},{"type":"input_text","text":"{\"exit_code\":0}"}]}});
        let text = lines(vec![
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"t"}}),
            call("failed"),
            output("failed", "Script failed"),
            json!({"type":"response_item","payload":{"type":"custom_tool_call","name":"exec","call_id":"read","input":"text(await tools.exec_command({cmd:\"cat a.ts\"}));"}}),
            output("read", "Script completed"),
        ]);
        let mut stats = parse(&text, SessionKind::Codex);
        assert_eq!(stats.files_touched, Some(0)); // A failed cell, and a cell that patches nothing.
        stats = parse(&format!("{text}\n{}\n{}", call("ok"), output("ok", "Script completed")), SessionKind::Codex);
        assert_eq!(stats.files_touched, Some(2));
        assert_eq!(stats.changed_files, Some(vec![changed_file("/r/a.ts", Some((1, 0))), changed_file("/r/b.ts", Some((1, 1)))]));
    }
    #[test]
    fn codex_patch_lines_follow_each_section_and_decode_the_cell_literal() {
        // `\\n` inside a patched line is source text, not a line break, once the literal is decoded.
        let source = r#"await tools.apply_patch("*** Begin Patch\n*** Update File: a.ts\n@@\n-old\n+let s = \"x\\ny\";\n+next\n*** Update File: b.ts\n*** Move to: c.ts\n@@\n-1\n+2\n*** Delete File: d.ts\n*** End Patch");"#;
        let edits = patch_edits(&code_mode_patches(source));
        assert_eq!(edits, vec![
            ("a.ts".to_string(), Some((2, 1))),
            ("b.ts".to_string(), None),
            ("c.ts".to_string(), Some((1, 1))),
            ("d.ts".to_string(), None),
        ]);
        // A patch held in a variable is found the same way.
        assert_eq!(patch_edits(&code_mode_patches("const p = \"*** Begin Patch\\n*** Add File: e.ts\\n+1\\n+2\\n*** End Patch\";\nawait tools.apply_patch(p);")), vec![("e.ts".to_string(), Some((2, 0)))]);
    }
    #[test]
    fn claude_lines_come_from_the_recorded_patch_and_sum_per_file() {
        let edit = |id: &str, name: &str, path: &str| json!({"type":"assistant","message":{"id":id,"content":[{"type":"tool_use","id":id,"name":name,"input":{"file_path":path}}]}});
        let result = |id: &str, recorded: Value| json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":id,"content":"ok"}]},"toolUseResult":recorded});
        let hunk = |lines: Vec<&str>| json!({"structuredPatch":[{"oldStart":1,"lines":lines}]});
        let text = lines(vec![
            json!({"type":"user","message":{"content":"go"}}),
            edit("e1", "Edit", "/r/a.ts"),
            result("e1", hunk(vec![" keep", "-old", "+new", "+more"])),
            edit("e2", "Edit", "/r/a.ts"),
            result("e2", hunk(vec!["-gone"])),
            edit("w1", "Write", "/r/new.ts"),
            result("w1", json!({"type":"create","content":"1\n2\n3\n","structuredPatch":[]})),
            edit("n1", "NotebookEdit", "/r/book.ipynb"),
            result("n1", json!({"cell_id":"c"})),
        ]);
        let stats = parse(&text, SessionKind::Claude);
        assert_eq!(stats.changed_files, Some(vec![
            changed_file("/r/a.ts", Some((2, 2))),
            changed_file("/r/book.ipynb", None),
            changed_file("/r/new.ts", Some((3, 0))),
        ]));
    }
    #[test]
    fn missing_usage_and_zero_input_have_no_invented_ratios() {
        let mut stats = parse("{}\nnot json", SessionKind::Claude);
        assert_eq!(stats.total_tokens, None);
        stats.with_context(&AgentContextInfo {
            model: Some("claude-sonnet-4-5".to_string()),
            context_tokens: Some(73_000),
            context_limit: 258_400,
            current_tool: None,
        });
        assert_eq!(stats.model.as_deref(), Some("claude-sonnet-4-5"));
        assert!((stats.context_percent.unwrap() - 28.25077).abs() < 0.001);
        stats.with_context(&AgentContextInfo {
            model: None,
            context_tokens: Some(73_000),
            context_limit: 0,
            current_tool: None,
        });
        assert_eq!(stats.context_percent, None);
        let stats = parse(
            &lines(vec![
                json!({"type":"user","message":{"content":"go"}}),
                json!({"type":"assistant","message":{"usage":{"input_tokens":0,"output_tokens":0}}}),
            ]),
            SessionKind::Claude,
        );
        assert_eq!(stats.total_tokens, Some(0));
        assert_eq!(stats.cache_hit_percent, None);
    }
    #[test]
    fn pi_reads_the_last_turn_and_only_counts_successful_edits() {
        let text = lines(vec![
            json!({"type":"session","id":"s"}),
            json!({"type":"model_change","id":"m1","parentId":null,"provider":"deepseek","modelId":"deepseek-flash"}),
            json!({"type":"message","id":"u1","parentId":"m1","message":{"role":"user","content":"first"}}),
            json!({"type":"message","id":"a1","parentId":"u1","message":{"role":"assistant","model":"deepseek-flash","usage":{"input":100,"output":20,"cacheRead":800,"cacheWrite":100},"content":[]}}),
            json!({"type":"message","id":"u2","parentId":"a1","message":{"role":"user","content":"second"}}),
            json!({"type":"message","id":"a2","parentId":"u2","message":{"role":"assistant","model":"deepseek-flash","usage":{"input":50,"output":10,"cacheRead":100,"cacheWrite":0},"duration":2000.0,"content":[
                {"type":"toolCall","id":"c1","name":"edit","arguments":{"path":"a.ts"}},
                {"type":"toolCall","id":"c2","name":"write","arguments":{"path":"b.ts"}}]}}),
            json!({"type":"message","id":"r1","parentId":"a2","message":{"role":"toolResult","toolCallId":"c1","toolName":"edit","isError":false,
                "details":{"patch":"--- a.ts\n+++ a.ts\n@@ -1,2 +1,2 @@\n keep\n-old\n+new\n+more"}}}),
            json!({"type":"message","id":"r2","parentId":"r1","message":{"role":"toolResult","toolCallId":"c2","toolName":"write","isError":true}}),
        ]);
        let stats = pi_parse(&text);
        assert_eq!(stats.changed_files, Some(vec![changed_file("a.ts", Some((2, 1)))]));
        assert_eq!(stats.input_tokens, Some(150));
        assert_eq!(stats.tokens, Some(10));
        assert_eq!(stats.total_tokens, Some(160));
        assert_eq!(stats.session_tokens, Some(1180));
        assert_eq!(stats.tools_used, Some(2));
        assert_eq!(stats.files_touched, Some(1));
        assert!((stats.cache_hit_percent.unwrap() - 100.0 / 150.0 * 100.0).abs() < 0.001);
        assert!((stats.generation_tokens_per_second.unwrap() - 5.0).abs() < 0.001);
    }
    #[test]
    fn pi_ignores_an_abandoned_branch_and_user_only_sessions_report_nothing() {
        let text = lines(vec![
            json!({"type":"message","id":"u1","parentId":null,"message":{"role":"user","content":"go"}}),
            json!({"type":"message","id":"a1","parentId":"u1","message":{"role":"assistant","usage":{"input":100,"output":10},"content":[]}}),
            json!({"type":"message","id":"a2","parentId":"u1","message":{"role":"assistant","usage":{"input":900,"output":90},"content":[]}}),
            json!({"type":"message","id":"u2","parentId":"a2","message":{"role":"user","content":"branch"}}),
            json!({"type":"message","id":"a3","parentId":"u2","message":{"role":"assistant","usage":{"input":300,"output":30},"content":[]}}),
        ]);
        let stats = pi_parse(&text);
        assert_eq!(stats.total_tokens, Some(330));
        // The abandoned sibling `a1` is off the branch; `a2` is still an ancestor of the live leaf.
        assert_eq!(stats.session_tokens, Some(1320));
        let idle = pi_parse(&lines(vec![json!({"type":"message","id":"u1","parentId":null,"message":{"role":"user","content":"go"}})]));
        assert_eq!(idle.total_tokens, None);
        assert_eq!(idle.session_tokens, None);
        assert_eq!(idle.tools_used, Some(0));
        assert_eq!(idle.generation_tokens_per_second, None);
        let empty = pi_parse("");
        assert_eq!(empty.total_tokens, None);
        assert_eq!(empty.session_tokens, None);
        assert_eq!(empty.tools_used, None);
        assert_eq!(empty.files_touched, None);
    }
    #[test]
    fn pi_branch_check_detects_a_fork_but_not_a_non_message_entry() {
        assert!(!pi_branch_broken(None, "{\"id\":\"a1\",\"parentId\":null}\n"));
        assert!(!pi_branch_broken(Some("a1"), "{\"id\":\"a2\",\"parentId\":\"a1\"}\n"));
        assert!(pi_branch_broken(Some("a2"), "{\"id\":\"a3\",\"parentId\":\"a1\"}\n"));
        assert!(!pi_branch_broken(Some("a2"), "{\"type\":\"title\"}\n{\"id\":\"a3\",\"parentId\":\"a2\"}\n"));
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn appended_partial_lines_are_retried_and_truncation_resets_accounting() {
        let path = std::env::temp_dir().join(format!("turn-stats-{}.jsonl", uuid::Uuid::new_v4()));
        let mut cached = Cached {
            path: path.clone(),
            ..Cached::default()
        };
        let user = "{\"type\":\"user\",\"message\":{\"content\":\"go\"}}\n";
        let message = "{\"type\":\"assistant\",\"message\":{\"id\":\"a\",\"usage\":{\"input_tokens\":100,\"output_tokens\":50}}}\n";
        std::fs::write(&path, format!("{user}{}", &message[..40])).unwrap();
        assert_eq!(
            read_cached(&mut cached, SessionKind::Claude)
                .unwrap()
                .total_tokens,
            None
        );
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(message[40..].as_bytes())
            .unwrap();
        assert_eq!(
            read_cached(&mut cached, SessionKind::Claude)
                .unwrap()
                .total_tokens,
            Some(150)
        );
        assert_eq!(
            read_cached(&mut cached, SessionKind::Claude)
                .unwrap()
                .total_tokens,
            Some(150)
        );
        std::fs::write(&path, user).unwrap();
        assert_eq!(
            read_cached(&mut cached, SessionKind::Claude)
                .unwrap()
                .total_tokens,
            None
        );
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn a_turn_larger_than_the_old_tail_limit_remains_complete() {
        let path =
            std::env::temp_dir().join(format!("turn-stats-long-{}.jsonl", uuid::Uuid::new_v4()));
        let mut text = String::from("{\"type\":\"user\",\"message\":{\"content\":\"go\"}}\n{\"type\":\"assistant\",\"message\":{\"id\":\"a\",\"usage\":{\"input_tokens\":100,\"output_tokens\":50}}}\n");
        text.push_str(
            &serde_json::json!({"type":"attachment","content":"x".repeat(600_000)}).to_string(),
        );
        text.push('\n');
        std::fs::write(&path, text).unwrap();
        let mut cached = Cached {
            path: path.clone(),
            ..Cached::default()
        };
        assert_eq!(
            read_cached(&mut cached, SessionKind::Claude)
                .unwrap()
                .total_tokens,
            Some(150)
        );
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn rewind_changes_the_active_turn_without_refunding_session_usage() {
        let path =
            std::env::temp_dir().join(format!("turn-stats-rewind-{}.jsonl", uuid::Uuid::new_v4()));
        let rows = [
            serde_json::json!({"type":"user","uuid":"u1","parentUuid":null,"message":{"content":"first"}}),
            serde_json::json!({"type":"assistant","uuid":"a1","parentUuid":"u1","message":{"id":"m1","usage":{"input_tokens":100,"output_tokens":10}}}),
            serde_json::json!({"type":"user","uuid":"u2","parentUuid":"a1","message":{"content":"second"}}),
            serde_json::json!({"type":"assistant","uuid":"a2","parentUuid":"u2","message":{"id":"m2","usage":{"input_tokens":200,"output_tokens":20}}}),
        ];
        std::fs::write(
            &path,
            rows.iter().map(|r| format!("{r}\n")).collect::<String>(),
        )
        .unwrap();
        let mut cached = Cached {
            path: path.clone(),
            ..Cached::default()
        };
        assert_eq!(
            read_cached(&mut cached, SessionKind::Claude)
                .unwrap()
                .total_tokens,
            Some(220)
        );
        writeln!(
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap(),
            "{}",
            serde_json::json!({"type":"last-prompt","rewound":true,"leafUuid":"a1"})
        )
        .unwrap();
        let stats = read_cached(&mut cached, SessionKind::Claude).unwrap();
        assert_eq!(stats.total_tokens, Some(110));
        assert_eq!(stats.session_tokens, Some(330));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn pi_reader_follows_appended_entries_and_rebuilds_after_a_fork() {
        let path = std::env::temp_dir().join(format!("turn-stats-pi-{}.jsonl", uuid::Uuid::new_v4()));
        let user = "{\"type\":\"message\",\"id\":\"u1\",\"parentId\":null,\"message\":{\"role\":\"user\",\"content\":\"go\"}}\n";
        std::fs::write(&path, user).unwrap();
        let mut cached = PiCached {
            path: path.clone(),
            ..PiCached::default()
        };
        assert_eq!(pi_read_cached(&mut cached).unwrap().total_tokens, None);
        let append = |text: &str| {
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap()
                .write_all(text.as_bytes())
                .unwrap();
        };
        append("{\"type\":\"message\",\"id\":\"a1\",\"parentId\":\"u1\",\"message\":{\"role\":\"assistant\",\"usage\":{\"input\":100,\"output\":10},\"content\":[]}}\n");
        assert_eq!(pi_read_cached(&mut cached).unwrap().total_tokens, Some(110));
        // A fork names an earlier parent, so the incremental counts have to be rebuilt from the branch.
        append("{\"type\":\"message\",\"id\":\"a2\",\"parentId\":\"u1\",\"message\":{\"role\":\"assistant\",\"usage\":{\"input\":900,\"output\":90},\"content\":[]}}\n");
        let stats = pi_read_cached(&mut cached).unwrap();
        assert_eq!(stats.total_tokens, Some(990));
        assert_eq!(stats.session_tokens, Some(990));
        // A recording replaced in place must not keep counts from the file it used to be.
        std::fs::write(&path, user).unwrap();
        let stats = pi_read_cached(&mut cached).unwrap();
        assert_eq!(stats.total_tokens, None);
        assert_eq!(stats.session_tokens, None);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn opencode_counts_the_last_turn_and_measures_generation() {
        fn user(id: &str) -> OpencodeMessage {
            OpencodeMessage { info: serde_json::json!({"id": id, "role": "user"}), parts: vec![] }
        }
        fn assistant(id: &str, parent: &str, created: u64, completed: u64, out: u64, reas: u64, cache: u64) -> OpencodeMessage {
            OpencodeMessage {
                info: serde_json::json!({
                    "id": id, "role": "assistant", "parentID": parent,
                    "tokens": {
                        "total": 10 + out + reas + cache, "input": 10, "output": out,
                        "reasoning": reas, "cache": {"read": cache, "write": 0}
                    },
                    "time": {"created": created, "completed": completed}
                }),
                parts: vec![
                    serde_json::json!({"id": format!("{id}-tool"), "type": "tool", "callID": format!("call-{id}")}),
                    serde_json::json!({"id": format!("{id}-patch"), "type": "patch", "files": ["/a.ts"]}),
                ],
            }
        }
        let messages = vec![
            user("u1"),
            assistant("a1", "u1", 0, 1000, 50, 10, 0),
            user("u2"),
            assistant("a2", "u2", 2000, 3000, 100, 20, 40),
            assistant("a3", "u2", 4000, 5000, 60, 0, 50),
        ];
        let stats = opencode_stats_from_messages(&messages);
        assert_eq!(stats.total_tokens, Some(290));
        assert_eq!(stats.tokens, Some(180));
        assert_eq!(stats.input_tokens, Some(110));
        assert_eq!(stats.session_tokens, Some(360));
        assert_eq!(stats.tools_used, Some(2));
        assert_eq!(stats.files_touched, Some(1));
        assert!((stats.cache_hit_percent.unwrap() - 90.0 / 110.0 * 100.0).abs() < 0.001);
        assert!((stats.generation_tokens_per_second.unwrap() - 90.0).abs() < 0.001);
    }

    #[test]
    fn opencode_lines_come_from_edit_calls_and_patches_add_the_files_they_missed() {
        let tool = |tool: &str, path: &str, metadata: Value| serde_json::json!({"type": "tool", "callID": path, "tool": tool,
            "state": {"status": "completed", "input": {"filePath": path, "content": "1\n2\n"}, "metadata": metadata}});
        let messages = vec![
            OpencodeMessage { info: serde_json::json!({"id": "u", "role": "user"}), parts: vec![] },
            OpencodeMessage {
                info: serde_json::json!({"id": "a", "role": "assistant", "parentID": "u", "tokens": {"total": 2, "input": 1, "output": 1}}),
                parts: vec![
                    tool("edit", "/r/a.ts", serde_json::json!({"filediff": {"additions": 4, "deletions": 1}})),
                    tool("write", "/r/new.ts", serde_json::json!({"exists": false})),
                    tool("write", "/r/old.ts", serde_json::json!({"exists": true})),
                    serde_json::json!({"type": "patch", "files": ["/r/a.ts", "/r/shell.ts"]}),
                ],
            },
        ];
        let stats = opencode_stats_from_messages(&messages);
        assert_eq!(stats.changed_files, Some(vec![
            changed_file("/r/a.ts", Some((4, 1))),
            changed_file("/r/new.ts", Some((2, 0))),
            changed_file("/r/old.ts", None),
            changed_file("/r/shell.ts", None),
        ]));
    }

    #[test]
    fn opencode_v2_messages_without_parents_join_the_turn_after_the_last_user_message() {
        let user = |id: &str| OpencodeMessage { info: serde_json::json!({"id": id, "role": "user"}), parts: vec![] };
        let step = |id: &str, out: u64, path: &str| OpencodeMessage {
            info: serde_json::json!({"id": id, "role": "assistant", "tokens": {"total": 1 + out, "input": 1, "output": out}}),
            parts: vec![serde_json::json!({"type": "tool", "callID": id, "tool": "edit",
                "state": {"status": "completed", "input": {"filePath": path}, "metadata": {"filediff": {"additions": 1, "deletions": 0}}}})],
        };
        let stats = opencode_stats_from_messages(&[user("u1"), step("a1", 10, "/r/old.ts"), user("u2"), step("a2", 20, "/r/a.ts"), step("a3", 30, "/r/a.ts")]);
        assert_eq!(stats.tokens, Some(50));
        assert_eq!(stats.session_tokens, Some(63));
        assert_eq!(stats.changed_files, Some(vec![changed_file("/r/a.ts", Some((2, 0)))]));
    }

    /// Opt-in read-only validation against a locally selected provider transcript. Only numeric
    /// accounting leaves the parser; no message content or tool arguments are printed or exported.
    #[test]
    #[ignore]
    fn real_turn_stats_sample() {
        let id = std::env::var("VLX_TURN_STATS_SAMPLE_ID").unwrap();
        let kind = match std::env::var("VLX_TURN_STATS_SAMPLE_KIND").as_deref() {
            Ok("claude") => SessionKind::Claude,
            Ok("opencode") => SessionKind::Opencode,
            Ok("pi") => SessionKind::Pi,
            Ok("omp") => SessionKind::Omp,
            _ => SessionKind::Codex,
        };
        let mut stats = current_turn_stats(kind, &id).unwrap();
        stats.with_context(&super::super::context_info(kind, &id).unwrap());
        let output = std::env::var("VLX_TURN_STATS_SAMPLE_OUTPUT").unwrap();
        std::fs::write(output, serde_json::to_string_pretty(&stats).unwrap()).unwrap();
        assert!(stats.total_tokens.is_some());
    }
}
