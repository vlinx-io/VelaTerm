//! Token, throughput and context accounting for DeepSeek Harness conversations.
//!
//! dsh commits one `assistant/message` event per model step. The event carries the provider's usage
//! (`inputTokens`, `outputTokens`, `cacheReadTokens`, `cacheWriteTokens`) and the step's stream, whose
//! `*-chunks` entries list per-chunk delays in milliseconds. Summing those delays measures the time
//! the model spent emitting output, so the reported rate excludes the wait before the first chunk and
//! tool execution between steps. Session totals are the sum of every committed message, which matches
//! dsh's own projection cache exactly.
//!
//! After the process exits, the ACP `usage_update` facts are gone. The context window is then read
//! from dsh's projection cache (`$DSH_HOME/storages/session_projcache/sessions/<id>.json`), which
//! records the window for the model that produced the latest usage.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use serde_json::Value;

use super::deepseek_history;
use crate::agent::transcript::{AgentContextInfo, ChangedFile, TurnStats};

/// dsh built-in tools whose arguments name the file they change. dsh names its built-ins in lowercase.
const EDIT_TOOLS: &[&str] = &["write", "edit", "multiedit", "multi_edit"];

/// Log events that carry turn boundaries, prompts, model usage and tool outcomes.
const WANTED: &[&str] = &["turn/start", "user/message", "assistant/message", "tool/call", "tool/result"];

/// One provider-reported usage record. dsh writes all fields together, but a value missing from an
/// older or imported log stays unknown instead of turning into a zero count.
#[derive(Clone, Copy, Debug, Default)]
struct Usage {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
}

impl Usage {
    fn parse(value: &Value) -> Option<Self> {
        Some(Self {
            input: value.get("inputTokens")?.as_u64()?,
            output: value.get("outputTokens")?.as_u64()?,
            cache_read: value.get("cacheReadTokens").and_then(Value::as_u64).unwrap_or(0),
            cache_write: value.get("cacheWriteTokens").and_then(Value::as_u64).unwrap_or(0),
        })
    }
    /// Input as the panel defines it for every agent: uncached input plus cache reads and writes.
    fn input_total(self) -> u64 {
        self.input.saturating_add(self.cache_read).saturating_add(self.cache_write)
    }
    fn total(self) -> u64 {
        self.input_total().saturating_add(self.output)
    }
    fn add(&mut self, other: Self) {
        self.input = self.input.saturating_add(other.input);
        self.output = self.output.saturating_add(other.output);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
        self.cache_write = self.cache_write.saturating_add(other.cache_write);
    }
}

#[derive(Clone, Debug)]
struct Message {
    turn: u64,
    usage: Usage,
    /// Milliseconds spent emitting this step's chunks, from the stream's per-chunk delays.
    generated_ms: u64,
}

#[derive(Clone, Debug, Default)]
struct Call {
    turn: u64,
    /// Files this call edits, when the tool and its arguments say so.
    paths: Vec<String>,
    done: bool,
    ok: bool,
}

/// A file-editing call's targets. A write or edit names `file_path`; an apply_patch body lists them
/// under its patch markers. Anything else is not a recorded edit, even if it changed the tree.
fn call_paths(name: &str, arguments: Option<&Value>) -> Vec<String> {
    let Some(raw) = arguments else { return Vec::new() };
    let parsed = match raw {
        Value::String(text) => serde_json::from_str::<Value>(text).unwrap_or(Value::Null),
        value => value.clone(),
    };
    if name == "apply_patch" {
        let patch = parsed.get("patch").and_then(Value::as_str).unwrap_or("");
        return patch.lines()
            .filter_map(|line| ["*** Update File: ", "*** Add File: ", "*** Delete File: "].iter()
                .find_map(|prefix| line.strip_prefix(*prefix)))
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_string)
            .collect();
    }
    if !EDIT_TOOLS.contains(&name) {
        return Vec::new();
    }
    ["file_path", "path", "notebook_path"].iter()
        .find_map(|key| parsed.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(|path| vec![path.to_string()])
        .unwrap_or_default()
}

/// Elapsed stream time for one assistant message. Each `*-chunks` entry stores the delay before every
/// emitted chunk; summing them discounts the wait for the first token and the gaps between blocks.
fn generated_ms(stream: Option<&Value>) -> u64 {
    stream.and_then(Value::as_array).into_iter().flatten()
        .filter_map(|entry| entry.get("dt").and_then(Value::as_array))
        .flatten()
        .filter_map(Value::as_u64)
        .fold(0u64, u64::saturating_add)
}

#[derive(Clone, Debug, Default)]
struct Summary {
    model: Option<String>,
    /// Every committed assistant message in the log.
    session: Option<Usage>,
    /// Assistant messages of the turn the log ends in.
    turn: Option<Usage>,
    /// Input of the latest API call, which is what the model carried as context.
    context_tokens: Option<u64>,
    has_turn: bool,
    generated_ms: u64,
    generated_tokens: u64,
    tools: u32,
    /// Files edited successfully in the current turn, sorted and deduplicated.
    files: Vec<String>,
}

fn parse_log(path: &Path) -> Result<Summary, String> {
    let mut messages: HashMap<String, Message> = HashMap::new();
    let mut calls: HashMap<String, Call> = HashMap::new();
    let mut current_turn = 0u64;
    let mut turn_events = false;
    let mut model: Option<String> = None;
    let mut last_usage: Option<Usage> = None;
    deepseek_history::each_event_of(path, WANTED, |event| {
        let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
        let data = event.get("data").unwrap_or(&Value::Null);
        match kind {
            "turn/start" => {
                turn_events = true;
                if let Some(turn) = data.get("turn").and_then(Value::as_u64) {
                    current_turn = current_turn.max(turn);
                }
            }
            "user/message" => {
                // Logs written before turn numbers existed have no `turn/start`; each person's prompt
                // then begins a turn. Runtime context and instruction messages are not prompts.
                if !turn_events && data.pointer("/source/kind").and_then(Value::as_str) == Some("user") {
                    current_turn = current_turn.saturating_add(1);
                }
            }
            "assistant/message" => {
                let turn = data.get("turn").and_then(Value::as_u64).unwrap_or(current_turn);
                current_turn = current_turn.max(turn);
                if let Some(name) = data.pointer("/message/source/model").and_then(Value::as_str).filter(|name| !name.is_empty()) {
                    model = Some(name.to_string());
                }
                let Some(usage) = data.get("usage").and_then(Usage::parse) else { return true };
                last_usage = Some(usage);
                let id = data.pointer("/message/id").and_then(Value::as_str).map(str::to_string)
                    .or_else(|| event.get("seq").and_then(Value::as_u64).map(|seq| format!("seq-{seq}")))
                    .unwrap_or_else(|| format!("message-{}", messages.len()));
                messages.insert(id, Message { turn, usage, generated_ms: generated_ms(data.get("stream")) });
            }
            "tool/call" => {
                let turn = data.get("turn").and_then(Value::as_u64).unwrap_or(current_turn);
                current_turn = current_turn.max(turn);
                let Some(id) = data.get("callId").and_then(Value::as_str).filter(|id| !id.is_empty()) else { return true };
                let name = data.get("name").and_then(Value::as_str).unwrap_or("");
                calls.insert(id.to_string(), Call {
                    turn,
                    paths: call_paths(name, data.get("arguments")),
                    ..Call::default()
                });
            }
            "tool/result" => {
                let id = data.pointer("/message/toolCallId").and_then(Value::as_str)
                    .or_else(|| data.get("toolCallId").and_then(Value::as_str))
                    .unwrap_or("");
                if let Some(call) = calls.get_mut(id) {
                    call.done = true;
                    call.ok = data.pointer("/message/isError").and_then(Value::as_bool) != Some(true);
                }
            }
            _ => {}
        }
        true
    })?;

    let latest_turn = current_turn;
    let mut session: Option<Usage> = None;
    let mut turn: Option<Usage> = None;
    let mut generated_ms = 0u64;
    let mut generated_tokens = 0u64;
    for message in messages.values() {
        session.get_or_insert_with(Usage::default).add(message.usage);
        if message.turn == latest_turn {
            turn.get_or_insert_with(Usage::default).add(message.usage);
            // Only a step whose stream recorded timing contributes to the rate; otherwise its tokens
            // would be divided by a window it was not measured over.
            if message.generated_ms > 0 {
                generated_ms = generated_ms.saturating_add(message.generated_ms);
                generated_tokens = generated_tokens.saturating_add(message.usage.output);
            }
        }
    }
    let mut files: Vec<String> = calls.values()
        .filter(|call| call.turn == latest_turn && call.done && call.ok)
        .flat_map(|call| call.paths.iter().cloned())
        .collect();
    files.sort();
    files.dedup();
    Ok(Summary {
        model,
        session,
        turn,
        context_tokens: last_usage.map(Usage::input_total),
        has_turn: !messages.is_empty() || !calls.is_empty(),
        generated_ms,
        generated_tokens,
        tools: calls.values().filter(|call| call.turn == latest_turn).count() as u32,
        files,
    })
}

#[derive(Clone)]
struct Cached {
    path: PathBuf,
    modified: Option<SystemTime>,
    len: u64,
    summary: Summary,
}
static CACHE: OnceLock<Mutex<HashMap<String, Cached>>> = OnceLock::new();

fn summary(id: &str) -> Result<Summary, String> {
    let path = deepseek_history::find(id)
        .ok_or("source_missing: The DeepSeek conversation log could not be found")?;
    let metadata = std::fs::metadata(&path).map_err(|error| error.to_string())?;
    let modified = metadata.modified().ok();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap();
    // Bound retained accounting when many archived sessions are inspected.
    if cache.len() >= 16 && !cache.contains_key(id) {
        cache.clear();
    }
    if let Some(entry) = cache.get(id) {
        if entry.path == path && entry.modified == modified && entry.len == metadata.len() {
            return Ok(entry.summary.clone());
        }
    }
    let summary = parse_log(&path)?;
    cache.insert(id.to_string(), Cached { path, modified, len: metadata.len(), summary: summary.clone() });
    Ok(summary)
}

/// The context window of the model behind the latest usage, from dsh's projection cache.
///
/// It is a cache, so a missing, unreadable or half-written file leaves the limit unknown (0) rather
/// than failing the panel.
fn context_window(id: &str) -> u64 {
    if !deepseek_history::valid_id(id) {
        return 0;
    }
    let Some(home) = deepseek_history::home() else { return 0 };
    let path = home.join("storages").join("session_projcache").join("sessions").join(format!("{id}.json"));
    let Ok(bytes) = std::fs::read(path) else { return 0 };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else { return 0 };
    context_window_from(&value)
}

fn context_window_from(value: &Value) -> u64 {
    value.pointer("/record/rows/contextPressure/val/contextWindow")
        .and_then(Value::as_u64)
        .filter(|window| *window > 0)
        .unwrap_or(0)
}

fn stats_from(summary: &Summary) -> TurnStats {
    let turn = summary.turn;
    TurnStats {
        model: summary.model.clone(),
        tokens: turn.map(|usage| usage.output),
        input_tokens: turn.map(Usage::input_total),
        total_tokens: turn.map(Usage::total),
        cached_tokens: turn.filter(|usage| usage.cache_read > 0).map(|usage| usage.cache_read),
        cache_hit_percent: turn.filter(|usage| usage.input_total() > 0)
            .map(|usage| usage.cache_read as f64 * 100.0 / usage.input_total() as f64),
        session_tokens: summary.session.map(Usage::total),
        tools_used: summary.has_turn.then_some(summary.tools),
        files_touched: summary.has_turn.then_some(summary.files.len() as u32),
        changed_files: summary.has_turn.then(|| summary.files.iter()
            .map(|path| ChangedFile { path: path.clone(), additions: None, deletions: None })
            .collect()),
        // Below 100 ms the timing is mostly scheduling noise and would overstate the rate.
        generation_tokens_per_second: (summary.generated_ms >= 100 && summary.generated_tokens > 0)
            .then(|| summary.generated_tokens as f64 * 1000.0 / summary.generated_ms as f64),
        ..TurnStats::default()
    }
}

/// Provider-reported usage and recorded activity for a DeepSeek conversation's current turn.
pub fn turn_stats(id: &str) -> Result<TurnStats, String> {
    Ok(stats_from(&summary(id)?))
}

/// Model, context occupancy and window for a DeepSeek conversation whose process is not running.
pub fn context_info(id: &str) -> Result<AgentContextInfo, String> {
    let summary = summary(id)?;
    Ok(AgentContextInfo {
        model: summary.model,
        context_tokens: summary.context_tokens,
        context_limit: context_window(id),
        current_tool: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn usage(input: u64, output: u64, read: u64, write: u64) -> Value {
        json!({"inputTokens":input,"outputTokens":output,"cacheReadTokens":read,"cacheWriteTokens":write,
            "totalTokens":input + output + read + write})
    }

    fn log(name: &str, events: &[Value]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vlx-dsh-stats-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.jsonl");
        let text = events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
        std::fs::write(&path, text + "\n").unwrap();
        path
    }

    fn turn_events() -> Vec<Value> {
        vec![
            json!({"type":"turn/start","seq":1,"time":1_000,"data":{"turn":1}}),
            json!({"type":"user/message","seq":2,"time":1_001,"data":{"content":[{"type":"text","text":"go"}],"source":{"kind":"user"},"role":"user","id":"u1"}}),
            json!({"type":"assistant/message","seq":3,"time":1_500,"data":{"turn":1,"step":1,
                "message":{"role":"assistant","id":"m1","source":{"kind":"model","provider":"deepseek-official","model":"deepseek-v4-flash"}},
                "usage":usage(100, 50, 900, 10),
                "stream":[{"type":"reasoning-chunks","time0":1_200,"dt":[10,20,30]},
                    {"type":"chunk","time":1_500,"chunk":{"type":"usage"}}]}}),
            json!({"type":"tool/call","seq":4,"time":1_501,"data":{"turn":1,"callId":"c1","name":"write",
                "arguments":"{\"file_path\":\"/w/a.txt\",\"content\":\"hi\"}"}}),
            json!({"type":"tool/result","seq":5,"time":1_520,"data":{"turn":1,
                "message":{"role":"tool","toolCallId":"c1","isError":false,"content":[{"type":"text","text":"ok"}]}}}),
            json!({"type":"turn/start","seq":6,"time":2_000,"data":{"turn":2}}),
            json!({"type":"user/message","seq":7,"time":2_001,"data":{"content":[{"type":"text","text":"again"}],"source":{"kind":"user"},"role":"user","id":"u2"}}),
            json!({"type":"assistant/message","seq":8,"time":2_500,"data":{"turn":2,"step":1,
                "message":{"role":"assistant","id":"m2","source":{"kind":"model","provider":"deepseek-official","model":"deepseek-v4-pro"}},
                "usage":usage(200, 80, 1_000, 20),
                "stream":[{"type":"text-chunks","time0":2_350,"dt":[100,50]},
                    {"type":"chunk","time":2_500,"chunk":{"type":"usage"}}]}}),
            json!({"type":"tool/call","seq":9,"time":2_501,"data":{"turn":2,"callId":"c2","name":"write",
                "arguments":"{\"file_path\":\"/w/b.txt\",\"content\":\"hi\"}"}}),
            json!({"type":"tool/result","seq":10,"time":2_520,"data":{"turn":2,
                "message":{"role":"tool","toolCallId":"c2","isError":false,"content":[{"type":"text","text":"ok"}]}}}),
            json!({"type":"tool/call","seq":11,"time":2_530,"data":{"turn":2,"callId":"c3","name":"bash",
                "arguments":"{\"command\":\"ls\"}"}}),
            json!({"type":"tool/result","seq":12,"time":2_540,"data":{"turn":2,
                "message":{"role":"tool","toolCallId":"c3","isError":true,"content":[{"type":"text","text":"no"}]}}}),
        ]
    }

    #[test]
    fn turn_usage_speed_and_files_come_from_committed_events() {
        let summary = parse_log(&log("turn", &turn_events())).unwrap();
        assert_eq!(summary.model.as_deref(), Some("deepseek-v4-pro"));
        assert_eq!(summary.session.unwrap().total(), (100 + 50 + 900 + 10) + (200 + 80 + 1_000 + 20));
        let turn = summary.turn.unwrap();
        assert_eq!((turn.input_total(), turn.output, turn.cache_read), (1_220, 80, 1_000));
        assert_eq!(summary.context_tokens, Some(1_220));
        assert_eq!(summary.tools, 2);
        assert_eq!(summary.files, ["/w/b.txt"]);
        assert_eq!(summary.generated_ms, 150);
        assert_eq!(summary.generated_tokens, 80);
    }

    #[test]
    fn stats_match_the_panel_units_and_rate() {
        let stats = stats_from(&parse_log(&log("panel", &turn_events())).unwrap());
        assert_eq!(stats.total_tokens, Some(1_300));
        assert_eq!(stats.input_tokens, Some(1_220));
        assert_eq!(stats.tokens, Some(80));
        assert_eq!(stats.cached_tokens, Some(1_000));
        assert_eq!(stats.cache_hit_percent, Some(1_000.0 * 100.0 / 1_220.0));
        assert_eq!(stats.session_tokens, Some(2_360));
        assert_eq!(stats.tools_used, Some(2));
        assert_eq!(stats.files_touched, Some(1));
        assert_eq!(stats.changed_files.unwrap()[0].path, "/w/b.txt");
        assert_eq!(stats.generation_tokens_per_second, Some(80.0 * 1000.0 / 150.0));
    }

    #[test]
    fn untimed_messages_do_not_dilute_the_measured_rate() {
        let events = vec![
            json!({"type":"turn/start","seq":1,"time":1,"data":{"turn":1}}),
            json!({"type":"assistant/message","seq":2,"time":2,"data":{"turn":1,"message":{"role":"assistant","id":"m1"},
                "usage":usage(10, 100, 0, 0),"stream":[{"type":"text-chunks","time0":1,"dt":[100]}]}}),
            json!({"type":"assistant/message","seq":3,"time":3,"data":{"turn":1,"message":{"role":"assistant","id":"m2"},
                "usage":usage(10, 900, 0, 0)}}),
        ];
        let stats = stats_from(&parse_log(&log("untimed", &events)).unwrap());
        assert_eq!(stats.tokens, Some(1_000));
        assert_eq!(stats.generation_tokens_per_second, Some(100.0 * 1000.0 / 100.0));
    }

    #[test]
    fn logs_without_turn_numbers_use_the_last_prompt_as_the_turn() {
        let events = vec![
            json!({"type":"user/message","seq":1,"time":1,"data":{"content":[],"source":{"kind":"user"},"role":"user","id":"u1"}}),
            json!({"type":"assistant/message","seq":2,"time":2,"data":{"message":{"role":"assistant","id":"m1"},"usage":usage(10, 5, 0, 0)}}),
            json!({"type":"user/message","seq":3,"time":3,"data":{"content":[],"source":{"kind":"user"},"role":"user","id":"u2"}}),
            json!({"type":"assistant/message","seq":4,"time":4,"data":{"message":{"role":"assistant","id":"m2"},"usage":usage(20, 7, 0, 0)}}),
        ];
        let stats = stats_from(&parse_log(&log("legacy", &events)).unwrap());
        assert_eq!(stats.input_tokens, Some(20));
        assert_eq!(stats.tokens, Some(7));
        assert_eq!(stats.session_tokens, Some(42));
        assert_eq!(stats.generation_tokens_per_second, None);
    }

    #[test]
    fn a_duplicate_message_replaces_its_usage_instead_of_counting_twice() {        let events = vec![
            json!({"type":"turn/start","seq":1,"time":1,"data":{"turn":1}}),
            json!({"type":"assistant/message","seq":2,"time":2,"data":{"turn":1,"message":{"role":"assistant","id":"m1"},"usage":usage(10, 5, 0, 0)}}),
            json!({"type":"assistant/message","seq":3,"time":3,"data":{"turn":1,"message":{"role":"assistant","id":"m1"},"usage":usage(10, 9, 0, 0)}}),
        ];
        let stats = stats_from(&parse_log(&log("duplicate", &events)).unwrap());
        assert_eq!(stats.session_tokens, Some(19));
        assert_eq!(stats.tokens, Some(9));
    }

    #[test]
    fn an_apply_patch_reports_every_file_it_names() {
        let paths = call_paths("apply_patch", Some(&json!({"patch":"*** Begin Patch\n*** Update File: /w/a.ts\n@@\n-x\n+y\n*** Add File: /w/b.ts\n+hi\n*** End Patch"})));
        assert_eq!(paths, ["/w/a.ts", "/w/b.ts"]);
    }

    #[test]
    fn the_context_window_comes_from_the_projection_cache() {
        let value = json!({"record":{"rows":{"contextPressure":{"val":{"contextWindow":1_048_576}}}}});
        assert_eq!(context_window_from(&value), 1_048_576);
        assert_eq!(context_window_from(&json!({"record":{"rows":{}}})), 0);
    }
}
