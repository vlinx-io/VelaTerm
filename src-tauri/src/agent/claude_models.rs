//! The model catalogue offered to a Claude chat session.
//!
//! Claude Code has no command that prints its models, so the versioned entries below are a curated
//! table rather than something the CLI reports. Two details make that table more than a list of
//! aliases. First, a `[1m]` suffix is a model identifier in its own right when a family offers both
//! standard and expanded contexts: passing `claude-opus-4-6[1m]` asks for the 1M-token window, so it
//! stays separate from `claude-opus-4-6`. A model that is natively 1M, such as Opus 5, has only one
//! selectable entry; retired suffixed spellings resolve to it. Second, newer models are rejected by
//! older CLIs, so entries carry the version that introduced them and are filtered accordingly.
//!
//! Custom identifiers the user configured under `env` in `settings.json` are appended afterwards. That
//! covers the case the curated table structurally cannot: a gateway identifier pointing at something
//! other than Anthropic's own models.
//!
//! Explicit 1M-context variants stay beside their standard model, matching Paseo's catalogue. Asking for
//! that expanded window can cost usage credits the account does not have, and the refusal is easy to miss:
//! the CLI answers the turn with an ordinary assistant message carrying `API Error: Usage credits required
//! for 1M context`, so nothing is actually run. Native 1M models do not carry the suffix and do not get a
//! second menu row.
//!
//! Everything here reads files and spawns `claude --version`, so callers must stay off the main
//! thread; dispatch already runs this inside a blocking worker.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

/// How long `claude --version` may run before the catalogue gives up and lists everything.
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// Effort levels a model accepts. The newer families add `xhigh` between `high` and `max`.
const EFFORT_STANDARD: &[&str] = &["low", "medium", "high", "max"];
const EFFORT_XHIGH: &[&str] = &["low", "medium", "high", "xhigh", "max"];

/// Settings keys under `env` that name a model. Claude Code reads each of these, so a model named in
/// any of them is one the user actually reaches.
const SETTINGS_ENV_KEYS: &[&str] = &[
    "ANTHROPIC_MODEL",
    "ANTHROPIC_SMALL_FAST_MODEL",
    "ANTHROPIC_DEFAULT_OPUS_MODEL",
    "ANTHROPIC_DEFAULT_SONNET_MODEL",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
];

/// Short names and retired spellings paired with the identifier Claude reports for them. An alias
/// and its canonical spelling are the same model, so resolving one to the other keeps the menu from
/// listing both and lets old saved preferences continue to work.
///
/// The right-hand side tracks what an alias currently resolves to, which moves when a new default
/// ships: update these rows alongside the manifest whenever a family's newest release changes.
const ALIASES: &[(&str, &str)] = &[
    ("opus", "claude-opus-5-5"),
    ("opus[1m]", "claude-opus-5-5"),
    ("claude-opus-5-5[1m]", "claude-opus-5-5"),
    ("claude-opus-5[1m]", "claude-opus-5"),
    ("claude-fable-5[1m]", "claude-fable-5"),
    ("claude-fable-5-1[1m]", "claude-fable-5-1"),
    ("sonnet", "claude-sonnet-5"),
    ("sonnet[1m]", "claude-sonnet-5[1m]"),
    ("haiku", "claude-haiku-4-5"),
];

/// One curated entry, before the installed CLI version is taken into account.
struct Entry {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    context_window: u64,
    effort: &'static [&'static str],
    /// CLI version that first accepted this identifier, or None when every supported version does.
    min_version: Option<(u32, u32, u32)>,
}

impl Entry {
    /// Whether this entry is an explicit request for the 1M context window.
    ///
    /// The `[1m]` suffix is what distinguishes an expanded-context variant, not the window size. Native
    /// 1M models have no suffixed selectable entry; only a suffix on a standard-context model asks for
    /// a different window that can be refused for want of credits.
    fn wants_large_context(&self) -> bool {
        self.id.ends_with("[1m]")
    }
}

/// One model as the composer's model chip shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeModel {
    /// Passed verbatim as the `--model` value and in `set_model` requests.
    pub id: String,
    /// Chip and menu text.
    pub label: String,
    /// Dimmed line beside the label, saying what the model is for or where it came from.
    pub description: String,
    /// Effort levels the effort chip should offer while this model is selected; empty when unknown.
    pub effort_levels: Vec<String>,
    /// Context window in tokens, or None for a configured model whose size is not known here.
    pub context_window: Option<u64>,
    /// False for an entry read from settings, which the curated descriptions do not cover.
    pub curated: bool,
    /// True for an entry whose id asks for the 1M window with `[1m]`, which some accounts must buy
    /// usage credits to run.
    pub large_context: bool,
    /// True when the running agent said this model honours fast mode. Never set for the curated table,
    /// which does not know.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub supports_fast_mode: bool,
}

/// Merge live model capabilities into the complete curated catalogue.
///
/// The live picker is a shortlist, not an exhaustive availability list. Omitted models remain selectable.
/// Its `default` row is omitted because the menu already offers the agent default.
/// The context window is not part of the answer, so it is borrowed from the curated table where the id
/// matches, and left unknown otherwise.
pub fn from_live(models: &[serde_json::Value]) -> Vec<ClaudeModel> {
    let curated = list();
    let live: Vec<ClaudeModel> = models
        .iter()
        .filter_map(|m| {
            let value = m.get("value").and_then(serde_json::Value::as_str)?;
            if value == "default"
                || m.get("disabled").and_then(serde_json::Value::as_bool) == Some(true)
            {
                return None;
            }
            let resolved = m.get("resolvedModel").and_then(Value::as_str).unwrap_or(value);
            let id = normalize_id(resolved).to_string();
            let known = curated.iter().find(|c| c.id == id);
            let label = known
                .map(|c| c.label.clone())
                .or_else(|| {
                    m.get("displayName")
                        .and_then(serde_json::Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| id.clone());
            let description = m
                .get("description")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .or_else(|| known.map(|c| c.description.clone()))
                .unwrap_or_default();
            let effort_levels: Vec<String> = m
                .get("supportedEffortLevels")
                .and_then(serde_json::Value::as_array)
                .map(|levels| {
                    levels
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .filter(|levels: &Vec<String>| !levels.is_empty())
                .or_else(|| known.map(|c| c.effort_levels.clone()))
                .unwrap_or_default();
            Some(ClaudeModel {
                large_context: id.contains("[1m]") || known.is_some_and(|c| c.large_context),
                context_window: known.and_then(|c| c.context_window),
                curated: known.is_some(),
                supports_fast_mode: m
                    .get("supportsFastMode")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                id,
                label,
                description,
                effort_levels,
            })
        })
        .collect();
    let mut merged = curated;
    for model in live {
        if let Some(existing) = merged.iter_mut().find(|entry| entry.id == model.id) {
            *existing = model;
        } else {
            merged.push(model);
        }
    }
    // An explicit refusal takes precedence; omission from the shortlist does not.
    merged.retain(|entry| {
        !models.iter().any(|model| {
            model.get("disabled").and_then(Value::as_bool) == Some(true)
                && model
                    .get("resolvedModel").or_else(|| model.get("value"))
                    .and_then(Value::as_str)
                    .is_some_and(|id| normalize_id(id) == entry.id)
        })
    });
    merged
}

/// The curated table, in the order the menu shows it: newest first, each family's 1M variant beside it.
const MANIFEST: &[Entry] = &[
    Entry {
        id: "claude-opus-5-5",
        label: "Opus 5.5",
        description: "Opus 5.5 · Latest release",
        context_window: 1_000_000,
        effort: EFFORT_XHIGH,
        min_version: Some((2, 1, 280)),
    },
    Entry {
        id: "claude-opus-5",
        label: "Opus 5",
        description: "Opus 5 · Previous release",
        context_window: 1_000_000,
        effort: EFFORT_XHIGH,
        min_version: Some((2, 1, 219)),
    },
    Entry {
        id: "claude-fable-5-1",
        label: "Fable 5.1",
        description: "Fable 5.1 · Most powerful model",
        context_window: 1_000_000,
        effort: EFFORT_XHIGH,
        min_version: None,
    },
    Entry {
        id: "claude-fable-5",
        label: "Fable 5",
        description: "Fable 5 · Previous release",
        context_window: 1_000_000,
        effort: EFFORT_XHIGH,
        min_version: Some((2, 1, 169)),
    },
    Entry {
        id: "claude-opus-4-8[1m]",
        label: "Opus 4.8 1M",
        description: "Opus 4.8 with 1M context window",
        context_window: 1_000_000,
        effort: EFFORT_XHIGH,
        min_version: None,
    },
    Entry {
        id: "claude-opus-4-8",
        label: "Opus 4.8",
        description: "Opus 4.8 · Previous release",
        context_window: 200_000,
        effort: EFFORT_XHIGH,
        min_version: None,
    },
    Entry {
        id: "claude-sonnet-5",
        label: "Sonnet 5",
        description: "Sonnet 5 · Best for everyday tasks",
        context_window: 200_000,
        effort: EFFORT_XHIGH,
        min_version: None,
    },
    Entry {
        id: "claude-sonnet-5[1m]",
        label: "Sonnet 5 1M",
        description: "Sonnet 5 with 1M context window",
        context_window: 1_000_000,
        effort: EFFORT_XHIGH,
        min_version: None,
    },
    Entry {
        id: "claude-opus-4-7[1m]",
        label: "Opus 4.7 1M",
        description: "Opus 4.7 with 1M context window",
        context_window: 1_000_000,
        effort: EFFORT_XHIGH,
        min_version: None,
    },
    Entry {
        id: "claude-opus-4-7",
        label: "Opus 4.7",
        description: "Opus 4.7 · Previous release",
        context_window: 200_000,
        effort: EFFORT_XHIGH,
        min_version: None,
    },
    Entry {
        id: "claude-opus-4-6[1m]",
        label: "Opus 4.6 1M",
        description: "Opus 4.6 with 1M context window",
        context_window: 1_000_000,
        effort: EFFORT_STANDARD,
        min_version: None,
    },
    Entry {
        id: "claude-opus-4-6",
        label: "Opus 4.6",
        description: "Opus 4.6 · Most capable for complex work",
        context_window: 200_000,
        effort: EFFORT_STANDARD,
        min_version: None,
    },
    Entry {
        id: "claude-sonnet-4-6[1m]",
        label: "Sonnet 4.6 1M",
        description: "Sonnet 4.6 with 1M context window",
        context_window: 1_000_000,
        effort: EFFORT_STANDARD,
        min_version: None,
    },
    Entry {
        id: "claude-sonnet-4-6",
        label: "Sonnet 4.6",
        description: "Sonnet 4.6 · Best for everyday tasks",
        context_window: 200_000,
        effort: EFFORT_STANDARD,
        min_version: None,
    },
    Entry {
        id: "claude-haiku-4-5",
        label: "Haiku 4.5",
        description: "Haiku 4.5 · Fastest for quick answers",
        context_window: 200_000,
        effort: EFFORT_STANDARD,
        min_version: None,
    },
];

/// The model list a running Claude process last reported, kept per executable.
///
/// The installed CLI is the only source that knows a model released after the table below was written.
/// A conversation that already answered leaves its list here, so a resting session and the new-session
/// dialog offer the same models instead of falling back to the table alone. Nothing is spawned to fill
/// this: it holds what a session reported on its own.
static REPORTED: Mutex<Option<HashMap<String, Vec<ClaudeModel>>>> = Mutex::new(None);

/// Record what the CLI behind `bin` reported, for the menus that have no process of their own.
pub fn remember_reported(bin: &str, models: &[ClaudeModel]) {
    if models.is_empty() {
        return;
    }
    let mut guard = REPORTED.lock().unwrap();
    guard
        .get_or_insert_with(HashMap::new)
        .insert(bin.to_string(), models.to_vec());
}

fn reported(bin: &str) -> Option<Vec<ClaudeModel>> {
    REPORTED.lock().unwrap().as_ref()?.get(bin).cloned()
}

/// Context windows a running Claude process reported at the end of a turn, keyed by model id.
///
/// The CLI is the only source that accounts for the `[1m]` request and the account's entitlement, but it
/// reports the window only to a live process and never writes it to the transcript. Keeping it here lets
/// a terminal session or a resting conversation on the same model show the same limit.
static REPORTED_WINDOWS: Mutex<Option<HashMap<String, u64>>> = Mutex::new(None);

/// Record the window the CLI reported for `model`.
pub fn remember_context_window(model: &str, window: u64) {
    if model.is_empty() || window == 0 {
        return;
    }
    let mut guard = REPORTED_WINDOWS.lock().unwrap();
    guard
        .get_or_insert_with(HashMap::new)
        .insert(model.to_string(), window);
}

/// Context window for a model id as a transcript records it, or None when no source knows the model.
///
/// A window the CLI reported wins, then the catalogue Claude Code itself uses, then the bundled table.
/// Nothing blocks on the network, so the Info panel can call this on every refresh.
pub fn context_window(model: &str) -> Option<u64> {
    let reported = REPORTED_WINDOWS.lock().unwrap().as_ref().and_then(|windows| {
        windows
            .iter()
            .find(|(id, _)| same_model(id, model))
            .map(|(_, window)| *window)
    });
    reported
        .or_else(|| super::official_claude_catalog::context_window(model, same_model))
        .or_else(|| {
            MANIFEST
                .iter()
                .find(|e| same_model(e.id, model))
                .map(|e| e.context_window)
        })
}

/// Whether two model ids name the same model, allowing either one to carry a dated snapshot suffix such
/// as `claude-haiku-4-5-20251001`.
fn same_model(a: &str, b: &str) -> bool {
    let dated = |long: &str, short: &str| {
        long.strip_prefix(short)
            .and_then(|rest| rest.strip_prefix('-'))
            .is_some_and(|date| date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()))
    };
    a == b || dated(a, b) || dated(b, a)
}

/// The path the installed Claude Code is launched from, or the bare name when no install is found.
fn installed_bin() -> String {
    crate::agent::install::locate_installed_bin("claude").unwrap_or_else(|| "claude".to_string())
}

/// The models a Claude chat session can be switched to.
///
/// A failure to read the version or the settings file is not an error: the catalogue degrades to the
/// full curated table, which is still usable, rather than leaving the chip empty.
///
/// The version is read on every call rather than once per run. Claude Code updates itself in the
/// background, so a version remembered from startup keeps hiding the models a newer binary accepts
/// until the whole application restarts.
pub fn list() -> Vec<ClaudeModel> {
    list_for_bin(&installed_bin())
}

/// Catalogue for the executable selected in application settings.
///
/// What this binary reported earlier is appended rather than substituted: the curated order stays as
/// written, and a model only the CLI knows still reaches the menu instead of being hidden until the
/// table catches up.
pub fn list_for_bin(bin: &str) -> Vec<ClaudeModel> {
    let mut out = list_with_version(read_version_for_bin(bin));
    for model in reported(bin).unwrap_or_default() {
        if !out.iter().any(|m| m.id == model.id) {
            out.push(model);
        }
    }
    out
}

fn list_with_version(version: Option<(u32, u32, u32)>) -> Vec<ClaudeModel> {
    let usable: Vec<&Entry> = MANIFEST.iter().filter(|e| accepts(e, version)).collect();
    let mut out: Vec<ClaudeModel> = usable
        .iter()
        .map(|e| ClaudeModel {
            id: e.id.to_string(),
            label: e.label.to_string(),
            description: e.description.to_string(),
            effort_levels: e.effort.iter().map(|s| s.to_string()).collect(),
            context_window: Some(e.context_window),
            curated: true,
            large_context: e.wants_large_context(),
            supports_fast_mode: false,
        })
        .collect();
    if let Some(remote) = super::remote_model_catalog::models(version) { out = merge_remote(out, remote); }
    for (id, origin) in settings_models() {
        let id = normalize_id(&id).to_string();
        if out.iter().any(|m| m.id == id) {
            continue;
        }
        out.push(ClaudeModel {
            label: id.clone(),
            id,
            description: format!("From Claude settings.json {origin}"),
            // A configured identifier may be a gateway model with its own effort rules, so offer the
            // set every Claude model has rather than guessing at the newer levels.
            effort_levels: EFFORT_STANDARD.iter().map(|s| s.to_string()).collect(),
            context_window: None,
            curated: false,
            // A configured identifier may or may not ask for the large window; nothing here can tell.
            large_context: false,
            supports_fast_mode: false,
        });
    }
    out
}

/// Lay the published catalogue over the bundled table without letting it hide what the table knows.
///
/// The published catalogue can lag behind a release: a model added to this build's table, which the
/// installed CLI accepts, would otherwise vanish from every launch form while the conversation view, which
/// asks the running CLI, still offers it. Published entries keep their order and wording; a table entry the
/// catalogue lacks is placed before the first later table entry the catalogue does list, so a new model
/// still leads its family. Both inputs have already been filtered by the installed CLI's version.
fn merge_remote(curated: Vec<ClaudeModel>, remote: Vec<ClaudeModel>) -> Vec<ClaudeModel> {
    let mut out = remote;
    for (index, entry) in curated.iter().enumerate() {
        if out.iter().any(|m| m.id == entry.id) {
            continue;
        }
        let anchor = curated[index + 1..]
            .iter()
            .find_map(|later| out.iter().position(|m| m.id == later.id));
        out.insert(anchor.unwrap_or(out.len()), entry.clone());
    }
    out
}

/// Resolve a short or retired alias to the identifier the CLI reports, leaving every other name untouched.
pub(crate) fn normalize_id(id: &str) -> &str {
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == id)
        .map(|(_, full)| *full)
        .unwrap_or(id)
}

/// Whether an entry is old enough for the installed CLI. An unknown version lists everything, on the
/// grounds that hiding a model the user has is worse than offering one they do not.
fn accepts(entry: &Entry, version: Option<(u32, u32, u32)>) -> bool {
    match (entry.min_version, version) {
        (Some(min), Some(have)) => have >= min,
        _ => true,
    }
}

fn read_version_for_bin(bin: &str) -> Option<(u32, u32, u32)> {
    let mut cmd = crate::host::command(bin);
    cmd.arg("--version");
    cmd.env("NO_COLOR", "1");
    let text = capture(cmd)?;
    parse_version(&text)
}

/// Read a short command's stdout, killing it once VERSION_TIMEOUT elapses.
fn capture(mut cmd: Command) -> Option<String> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = cmd.spawn().ok()?;
    let out = child.stdout.take();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut o) = out {
            use std::io::Read;
            let _ = o.read_to_string(&mut buf);
        }
        let _ = tx.send(buf);
    });
    let text = rx.recv_timeout(VERSION_TIMEOUT).ok();
    if text.is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
    text
}

/// Pull `2.1.252` out of `2.1.252 (Claude Code)`, or out of any line that starts with three numbers.
fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    for token in text.split(|c: char| c.is_whitespace() || c == '(' || c == ')') {
        let mut parts = token.split('.');
        let (a, b, c) = (parts.next()?, parts.next(), parts.next());
        let (Some(b), Some(c)) = (b, c) else { continue };
        if parts.next().is_some() {
            continue;
        }
        if let (Ok(a), Ok(b), Ok(c)) = (a.parse(), b.parse(), c.parse()) {
            return Some((a, b, c));
        }
    }
    None
}

/// Models named under `env` in `settings.json`, each paired with the key it came from.
///
/// The top-level `model` key is deliberately skipped. It records which model is currently selected,
/// not a model the user defined, and Claude Code rewrites it on every `/model` switch, so reading it
/// would append a duplicate of a curated entry under whatever alias happened to be stored.
///
/// Only the user-level file is read. A project-level `.claude/settings.json` is scoped to one
/// directory, and a session's model chip is not, so listing those would offer models that do not
/// apply where the session actually runs.
fn settings_models() -> Vec<(String, String)> {
    let Some(root) = config_dir() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(root.join("settings.json")) else {
        return Vec::new();
    };
    let Ok(json) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let mut out: Vec<(String, String)> = Vec::new();
    let mut push = |value: Option<&Value>, origin: &str| {
        if let Some(id) = value.and_then(Value::as_str) {
            let id = id.trim();
            if !id.is_empty() && !out.iter().any(|(seen, _)| seen == id) {
                out.push((id.to_string(), origin.to_string()));
            }
        }
    };
    for key in SETTINGS_ENV_KEYS {
        push(json.get("env").and_then(|e| e.get(key)), &format!("env.{key}"));
    }
    out
}

/// Claude's configuration directory: `CLAUDE_CONFIG_DIR` when set, otherwise `~/.claude`.
fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    crate::host::home_dir().map(|h| h.join(".claude"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn published_catalogue_keeps_table_models_it_lacks_in_table_order() {
        let model = |id: &str, label: &str| super::ClaudeModel {
            id: id.into(), label: label.into(), description: String::new(), effort_levels: vec![],
            context_window: None, curated: true, large_context: false, supports_fast_mode: false,
        };
        let curated = vec![model("new", "Table new"), model("current", "Table current"), model("old", "Table old"), model("last", "Table last")];
        let remote = vec![model("current", "Published current"), model("old", "Published old"), model("remote-only", "Published only")];
        let merged = super::merge_remote(curated, remote);
        let ids: Vec<_> = merged.iter().map(|m| m.id.as_str()).collect();
        // A model newer than the published catalogue leads; one listed after every published entry trails.
        assert_eq!(ids, ["new", "current", "old", "remote-only", "last"]);
        // Published wording wins for a model both list.
        assert_eq!(merged[1].label, "Published current");
    }

    use super::*;

    #[test]
    fn context_window_prefers_the_cli_and_matches_dated_snapshots() {
        assert_eq!(context_window("claude-opus-5-5"), Some(1_000_000));
        assert_eq!(context_window("claude-haiku-4-5-20251001"), Some(200_000));
        assert_eq!(context_window("claude-opus-5-5-preview"), None);
        assert_eq!(context_window("weird-model"), None);
        remember_context_window("test-reported-model", 400_000);
        assert_eq!(context_window("test-reported-model"), Some(400_000));
        assert_eq!(context_window("test-reported-model-20260101"), Some(400_000));
    }

    #[test]
    fn live_shortlist_preserves_complete_catalogue_and_updates_capabilities() {
        let live = vec![
            serde_json::json!({"value":"default","displayName":"Default (recommended)"}),
            serde_json::json!({"value":"claude-opus-5","displayName":"Opus (1M context)","description":"Best","supportedEffortLevels":["low","high"],"supportsFastMode":true}),
            serde_json::json!({"value":"gateway-x","displayName":"Gateway","disabled":true}),
            serde_json::json!({"value":"gateway-y","displayName":"Gateway Y"}),
        ];
        let models = from_live(&live);
        assert!(models.iter().any(|m| m.id == "claude-opus-4-6"));
        assert!(models.iter().any(|m| m.id == "claude-opus-4-6[1m]"));
        assert!(models.iter().any(|m| m.id == "claude-fable-5-1"));
        assert!(!models
            .iter()
            .any(|m| m.id == "default" || m.id == "gateway-x"));
        assert_eq!(models.iter().filter(|m| m.id == "claude-opus-5").count(), 1);
        let opus5 = models.iter().find(|m| m.id == "claude-opus-5").unwrap();
        assert_eq!(opus5.label, "Opus 5");
        assert_eq!(opus5.effort_levels, vec!["low", "high"]);
        assert!(opus5.supports_fast_mode);
        assert!(
            opus5.context_window.is_some(),
            "borrowed from the curated table"
        );
        let gateway = models.iter().find(|m| m.id == "gateway-y").unwrap();
        assert!(!gateway.curated);
        assert!(gateway.context_window.is_none());
        let disabled =
            from_live(&[serde_json::json!({"value":"claude-sonnet-4-6","disabled":true})]);
        assert!(!disabled.iter().any(|m| m.id == "claude-sonnet-4-6"));
        assert!(disabled.iter().any(|m| m.id == "claude-sonnet-4-6[1m]"));
        for entry in MANIFEST {
            if accepts(entry, read_version_for_bin(&installed_bin())) {
                assert!(
                    models.iter().any(|m| m.id == entry.id),
                    "missing {}",
                    entry.id
                );
            }
        }
    }

    #[test]
    fn parses_the_version_line_claude_prints() {
        assert_eq!(parse_version("2.1.252 (Claude Code)"), Some((2, 1, 252)));
        assert_eq!(parse_version("nothing here"), None);
    }

    #[test]
    fn hides_entries_the_installed_cli_predates() {
        let entry = |id: &str| MANIFEST.iter().find(|e| e.id == id).unwrap();
        let opus5 = entry("claude-opus-5");
        assert!(!accepts(opus5, Some((2, 1, 200))));
        assert!(accepts(opus5, Some((2, 1, 219))));
        // An unknown version must not hide anything.
        assert!(accepts(opus5, None));
        let opus55 = entry("claude-opus-5-5");
        assert!(!accepts(opus55, Some((2, 1, 278))));
        assert!(accepts(opus55, Some((2, 1, 280))));
    }

    #[test]
    fn keeps_large_context_variants_beside_their_standard_model() {
        let models = list();
        let expanded = models
            .iter()
            .position(|m| m.id == "claude-opus-4-8[1m]")
            .unwrap();
        let standard = models
            .iter()
            .position(|m| m.id == "claude-opus-4-8")
            .unwrap();
        assert_eq!(expanded + 1, standard);
    }

    #[test]
    fn resolves_short_and_retired_aliases_to_curated_identifiers() {
        let ids: Vec<&str> = MANIFEST.iter().map(|e| e.id).collect();
        for (alias, full) in ALIASES {
            assert_eq!(normalize_id(alias), *full);
            assert!(
                ids.contains(full),
                "{full} is aliased but missing from the manifest"
            );
        }
        // Anything that is not an alias passes through, gateway identifiers included.
        assert_eq!(normalize_id("claude-opus-5[1m]"), "claude-opus-5");
        assert_eq!(normalize_id("my-gateway/opus"), "my-gateway/opus");
    }

    #[test]
    fn offers_one_native_one_million_opus_5_entry() {
        let opus5: Vec<&str> = MANIFEST
            .iter()
            .filter(|entry| normalize_id(entry.id) == "claude-opus-5")
            .map(|entry| entry.id)
            .collect();
        assert_eq!(opus5, vec!["claude-opus-5"]);
    }

    #[test]
    fn folds_the_live_opus_5_5_one_million_spelling_into_one_entry() {
        // Claude Code 2.1.280 reports its Opus row as `opus[1m]` resolving to `claude-opus-5-5[1m]`.
        let models = from_live(&[serde_json::json!({
            "value": "opus[1m]", "resolvedModel": "claude-opus-5-5[1m]",
            "displayName": "Opus (1M context)", "supportsFastMode": true
        })]);
        assert_eq!(models.iter().filter(|m| m.id == "claude-opus-5-5").count(), 1);
        assert!(!models.iter().any(|m| m.id == "claude-opus-5-5[1m]"));
    }

    #[test]
    fn appends_reported_models_the_curated_table_does_not_list() {
        let bin = "/test/appends-reported/claude";
        let reported = from_live(&[serde_json::json!({
            "value": "test-unreleased", "displayName": "Unreleased",
            "supportedEffortLevels": ["low", "high"]
        })]);
        remember_reported(bin, &reported);
        let models = list_for_bin(bin);
        let curated = list_with_version(None);
        // The curated order is untouched and the model only the CLI named is offered after it.
        assert_eq!(
            models.iter().position(|m| m.id == "test-unreleased"),
            Some(models.len() - 1)
        );
        for entry in &curated {
            assert!(models.iter().any(|m| m.id == entry.id), "dropped {}", entry.id);
        }
        assert_eq!(models.iter().filter(|m| m.id == "claude-opus-5-5").count(), 1);
    }

    #[test]
    fn keeps_the_one_million_variants_as_separate_ids() {
        let ids: Vec<&str> = MANIFEST.iter().map(|e| e.id).collect();
        assert!(ids.contains(&"claude-opus-4-6"));
        assert!(ids.contains(&"claude-opus-4-6[1m]"));
    }

    #[test]
    fn live_alias_uses_the_resolved_version_without_removing_other_versions() {
        let models = from_live(&[serde_json::json!({
            "value": "opus", "resolvedModel": "test-future-opus",
            "displayName": "Future Opus", "supportsFastMode": true
        })]);
        assert!(models.iter().any(|m| m.id == "test-future-opus" && m.supports_fast_mode));
        assert!(models.iter().any(|m| m.id == "claude-opus-4-6"));
        let disabled = from_live(&[serde_json::json!({
            "value": "opus", "resolvedModel": "claude-opus-4-6", "disabled": true
        })]);
        assert!(!disabled.iter().any(|m| m.id == "claude-opus-4-6"));
    }
}
