//! The model catalogue offered to a Claude chat session.
//!
//! The website catalogue is the base, and without it the versioned table below. The installed Claude CLI
//! is merged in: `cli_model_catalog` asks it for its models over the stream-json control protocol and
//! caches the answer per binary and version. The CLI's list is a shortlist of recommendations, not the set
//! of models it runs, so it never replaces the catalogue: a CLI row updates the capabilities of the entry
//! it names (effort levels, fast mode, the default mark), an identifier the catalogue does not list is
//! appended (a model released after the catalogue was last published), and a disabled row removes its
//! entry. Custom identifiers the user configured under `env` in `settings.json` are appended in every case;
//! that covers a gateway identifier pointing at something other than Anthropic's own models. Finally, when
//! Claude Code's `availableModels` setting restricts the models a user may pick, the list is filtered to
//! what it allows.
//!
//! Identifiers are folded onto the catalogue's spelling before anything is compared (`fold_id`): the CLI
//! reports a natively 1M model as `claude-opus-5-5[1m]` and Haiku with its date, while the catalogue lists
//! `claude-opus-5-5` and `claude-haiku-4-5`. A `[1m]` suffix on a family that offers both standard and
//! expanded contexts is a model identifier in its own right, though: passing `claude-opus-4-6[1m]` asks for
//! the 1M-token window, so it stays separate from `claude-opus-4-6`. Asking for that expanded window can
//! cost usage credits the account does not have, and the refusal is easy to miss: the CLI answers the turn
//! with an ordinary assistant message carrying `API Error: Usage credits required for 1M context`.
//! Newer models are rejected by older CLIs, so table entries carry the version that introduced them and are
//! filtered accordingly.
//!
//! Everything here reads files and spawns `claude --version` or the probe, so callers must stay off the
//! main thread; dispatch already runs this inside a blocking worker.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use super::cli_model_catalog::{self, CliModel, Probe};
use crate::host::AppCtx;

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
/// This table is the fallback: with CLI data present, `normalize_id` follows the CLI's own
/// `value -> resolvedModel` pairs and consults this table only when its target is a model the CLI
/// offers. The right-hand side tracks what an alias resolved to when the table was last updated.
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
    /// True for the model the CLI's `default` row currently resolves to, so the chip can name the default.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_default: bool,
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

/// Catalogue for one Claude executable, probing it when the cache is missing or stale.
///
/// A failure to probe, read the version or the settings file is not an error: the catalogue degrades to
/// the website catalogue or the curated table, which is still usable, rather than leaving the chip empty.
pub fn list_for_bin(app: &AppCtx, bin: &str) -> Vec<ClaudeModel> {
    list_for_bin_with(app, bin, Probe::Allow)
}

/// What a stored selection may name: the merged catalogue before the `availableModels` filter, the
/// curated table, and the stored identifier itself when it is a well-formed Claude model id. The CLI's
/// list moves with every release, so a job saved with a model the menu offered then must keep validating
/// instead of failing its retry; the CLI decides at start whether it still runs that model. Offering
/// stays narrow; only acceptance widens.
pub fn accepted_for_bin(app: &AppCtx, bin: &str, selected: Option<&str>) -> Vec<ClaudeModel> {
    accepted(unfiltered(app, bin, Probe::Allow, None), selected)
}

fn accepted(offered: Vec<ClaudeModel>, selected: Option<&str>) -> Vec<ClaudeModel> {
    let mut out = with_curated(offered);
    if let Some(id) = selected.map(str::trim).filter(|id| is_claude_model_id(id)) {
        if !out.iter().any(|m| m.id == id) {
            out.push(ClaudeModel {
                id: id.to_string(),
                label: label_for(id, ""),
                description: String::new(),
                // Unknown here, so every level a Claude model can take; the CLI rejects what does not fit.
                effort_levels: EFFORT_XHIGH.iter().map(|s| s.to_string()).collect(),
                context_window: None,
                curated: false,
                large_context: id.ends_with("[1m]"),
                supports_fast_mode: false,
                is_default: false,
            });
        }
    }
    out
}

/// A well-formed Anthropic model identifier as a stored selection carries it: `claude-` followed by
/// lowercase letters, digits, dots and hyphens, with an optional `[1m]` suffix, at most 64 characters. It
/// cannot start with a hyphen, so it is never read as a command-line flag.
fn is_claude_model_id(id: &str) -> bool {
    let base = id.strip_suffix("[1m]").unwrap_or(id);
    id.len() <= 64
        && base.len() > "claude-".len()
        && base.starts_with("claude-")
        && base.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
}

fn with_curated(mut offered: Vec<ClaudeModel>) -> Vec<ClaudeModel> {
    for entry in bundled(None) {
        if !offered.iter().any(|m| m.id == entry.id) {
            offered.push(entry);
        }
    }
    offered
}

/// Catalogue for one executable with an explicit probing policy; `CacheOnly` never spawns anything.
pub fn list_for_bin_with(app: &AppCtx, bin: &str, probe: Probe) -> Vec<ClaudeModel> {
    list_for_session(app, bin, probe, None)
}

/// Catalogue for one session. `live` is the running conversation's own `list_models` answer: it is merged
/// in place of the probe for this session's menu, since it may reflect that session's project settings,
/// and the probe's rows then only add identifiers. Without it the neutral probe's rows are merged, and
/// what running conversations of the same binary reported is appended as additions only.
pub fn list_for_session(app: &AppCtx, bin: &str, probe: Probe, live: Option<&[Value]>) -> Vec<ClaudeModel> {
    filter_available(unfiltered(app, bin, probe, live), available_models().as_deref())
}

fn unfiltered(app: &AppCtx, bin: &str, probe: Probe, live: Option<&[Value]>) -> Vec<ClaudeModel> {
    let live = live.map(cli_model_catalog::parse_rows).filter(|rows| !rows.is_empty());
    // A running conversation has already answered for itself; nothing needs to be spawned for it.
    let probe = if live.is_some() { Probe::CacheOnly } else { probe };
    let snapshot = cli_model_catalog::ensure(app, bin, probe);
    // The version filter for the bundled table: what this run read, else what the snapshot recorded.
    let version = cli_model_catalog::cached_version(bin)
        .or_else(|| snapshot.as_ref().and_then(|s| s.version.clone()))
        .and_then(|v| parse_version(&v));
    // Additions of another CLI version do not describe this binary; a cache-only read (share surface) or a
    // failed probe after an update must not show them.
    let current = cli_model_catalog::cached_version(bin);
    let (probed, remembered): (&[CliModel], &[CliModel]) = match &snapshot {
        Some(s) if current.is_none() || s.live_version == current => (&s.models, &s.live),
        Some(s) => (&s.models, &[]),
        None => (&[], &[]),
    };
    match &live {
        Some(rows) => {
            let additions: Vec<CliModel> = probed.iter().chain(remembered).cloned().collect();
            assemble(Some(rows), &additions, version)
        }
        None => assemble(Some(probed), remembered, version),
    }
}

/// The one place the sources are combined: website catalogue (else the bundled table), the CLI rows
/// merged in, the additions appended, then the identifiers from `settings.json`.
fn assemble(cli: Option<&[CliModel]>, additions: &[CliModel], version: Option<(u32, u32, u32)>) -> Vec<ClaudeModel> {
    assemble_with(
        cli,
        additions,
        version,
        super::remote_model_catalog::models(version),
        &settings_models(),
        &cli_model_catalog::alias_pairs(),
    )
}

/// `cli` rows are merged: a row updates the capabilities of the entry it names, an unknown identifier is
/// appended, a disabled row removes its entry. `additions` only ever append an identifier nothing else
/// lists; they never change or remove an entry.
fn assemble_with(
    cli: Option<&[CliModel]>,
    additions: &[CliModel],
    version: Option<(u32, u32, u32)>,
    website: Option<Vec<ClaudeModel>>,
    settings: &[(String, String)],
    pairs: &[(String, String)],
) -> Vec<ClaudeModel> {
    let mut out = website.filter(|models| !models.is_empty()).unwrap_or_else(|| bundled(version));
    // Folding also consults the whole bundled table, so a spelling folds here exactly as in
    // `normalize_id`, even when the website does not list that model yet or the version filter hid it.
    let known = with_curated(out.clone());
    let refused = merge_cli(&mut out, cli.unwrap_or(&[]), &known);
    for row in additions.iter().filter(|r| !r.disabled && r.value != "default") {
        let id = fold_id(&row_target(row), &known);
        if refused.contains(&id) || out.iter().any(|m| m.id == id) {
            continue;
        }
        out.push(cli_model(&id, row, false));
    }
    for (id, origin) in settings {
        let id = normalize_with(id, pairs, &known);
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
            is_default: false,
        });
    }
    out
}

/// The bundled table, filtered to what the installed version accepts.
fn bundled(version: Option<(u32, u32, u32)>) -> Vec<ClaudeModel> {
    MANIFEST
        .iter()
        .filter(|e| accepts(e, version))
        .map(|e| ClaudeModel {
            id: e.id.to_string(),
            label: e.label.to_string(),
            description: e.description.to_string(),
            effort_levels: e.effort.iter().map(|s| s.to_string()).collect(),
            context_window: Some(e.context_window),
            curated: true,
            large_context: e.wants_large_context(),
            supports_fast_mode: false,
            is_default: false,
        })
        .collect()
}

/// What a CLI row stands for: the identifier it resolves to, or its own value when the CLI does not say.
fn row_target(row: &CliModel) -> String {
    row.resolved_model.clone().unwrap_or_else(|| row.value.clone())
}

/// Merge the CLI's rows into `out` and return the folded identifiers its disabled rows refuse.
///
/// Rows are keyed by what they resolve to, folded onto the catalogue's spelling, so `opus[1m]` and
/// `default`, which both resolve to `claude-opus-5-5[1m]`, update the one `claude-opus-5-5` entry. The
/// `default` row itself is not listed; its target is marked `is_default`, and appended from the default row
/// when neither the catalogue nor another row offers it. A disabled row removes its model even when another
/// row resolves to it.
fn merge_cli(out: &mut Vec<ClaudeModel>, rows: &[CliModel], known: &[ClaudeModel]) -> Vec<String> {
    let target = |row: &CliModel| fold_id(&row_target(row), known);
    let refused: Vec<String> = rows.iter().filter(|r| r.disabled).map(target).collect();
    out.retain(|m| !refused.contains(&m.id));
    let default_row = rows.iter().find(|r| r.value == "default" && !r.disabled);
    let default_id = default_row.map(target).filter(|id| id != "default" && !refused.contains(id));
    let mut seen: Vec<String> = Vec::new();
    let ordered = rows.iter().filter(|r| !r.disabled && r.value != "default").chain(default_row);
    for row in ordered {
        let id = target(row);
        if id == "default" || refused.contains(&id) || seen.contains(&id) {
            continue;
        }
        let is_default = default_id.as_deref() == Some(id.as_str());
        match out.iter_mut().find(|m| m.id == id) {
            Some(entry) => update_from_row(entry, row, is_default),
            None => out.push(cli_model(&id, row, is_default)),
        }
        seen.push(id);
    }
    refused
}

/// A CLI row describes what the installed version does with a catalogue entry: its effort levels, fast
/// mode and the default mark. The catalogue keeps its identifier, label and context window; the CLI's
/// description and display name fill in only where the catalogue has none.
fn update_from_row(entry: &mut ClaudeModel, row: &CliModel, is_default: bool) {
    if !row.supported_effort_levels.is_empty() {
        entry.effort_levels = row.supported_effort_levels.clone();
    }
    entry.supports_fast_mode = row.supports_fast_mode;
    entry.is_default = is_default;
    if entry.description.trim().is_empty() && !row.description.trim().is_empty() {
        entry.description = row.description.clone();
    }
    if entry.label.trim().is_empty() {
        entry.label = label_for(&entry.id, &row.display_name);
    }
}

fn cli_model(id: &str, row: &CliModel, is_default: bool) -> ClaudeModel {
    let known = MANIFEST.iter().find(|e| e.id == id);
    let effort_levels = if !row.supported_effort_levels.is_empty() {
        row.supported_effort_levels.clone()
    } else {
        known.map(|e| e.effort).unwrap_or(EFFORT_STANDARD).iter().map(|s| s.to_string()).collect()
    };
    let description = if row.description.trim().is_empty() {
        known.map(|e| e.description.to_string()).unwrap_or_default()
    } else {
        row.description.clone()
    };
    let large_context = id.ends_with("[1m]");
    ClaudeModel {
        id: id.to_string(),
        label: label_for(id, &row.display_name),
        description,
        effort_levels,
        context_window: known.map(|e| e.context_window).or(large_context.then_some(1_000_000)),
        curated: true,
        large_context,
        supports_fast_mode: row.supports_fast_mode,
        is_default,
    }
}

/// Fold a spelling of a model onto the identifier `known` lists for it, leaving every other id untouched.
///
/// The one folding rule for the merge, `normalize_id` and so the chip. An identifier `known` lists is
/// kept. A dated identifier (`claude-haiku-4-5-20251001`) folds onto its undated entry when `known` lists
/// one. A `[1m]` suffix folds away when `known` lists the plain identifier as a natively 1M model and no
/// separate suffixed entry (`claude-opus-5-5[1m]` becomes `claude-opus-5-5`); on a standard-context model
/// the suffix asks for a different window, so `claude-opus-4-6[1m]` stays what it is.
pub(crate) fn fold_id(id: &str, known: &[ClaudeModel]) -> String {
    let listed = |candidate: &str| known.iter().any(|m| m.id == candidate);
    if listed(id) {
        return id.to_string();
    }
    let (base, large) = match id.strip_suffix("[1m]") {
        Some(base) => (base, true),
        None => (id, false),
    };
    let base = match base.rsplit_once('-') {
        Some((stem, date))
            if date.len() == 8
                && date.bytes().all(|b| b.is_ascii_digit())
                && (listed(stem) || listed(&format!("{stem}[1m]"))) =>
        {
            stem
        }
        _ => base,
    };
    if !large {
        return base.to_string();
    }
    let suffixed = format!("{base}[1m]");
    if !listed(&suffixed) && natively_large(base, known) {
        base.to_string()
    } else {
        suffixed
    }
}

/// Whether `known` lists `id` as a model whose standard window is already 1M. A website entry without a
/// context window falls back to what the bundled table says about the same identifier.
fn natively_large(id: &str, known: &[ClaudeModel]) -> bool {
    known.iter().find(|m| m.id == id).is_some_and(|m| {
        !m.large_context
            && match m.context_window {
                Some(window) => window >= 1_000_000,
                None => MANIFEST.iter().any(|e| e.id == id && e.context_window >= 1_000_000),
            }
    })
}

/// Chip and menu text for a CLI row. The CLI's display names are generic ("Opus (1M context)", "Fable"),
/// so when the identifier tells the generation the label is derived from it; a display name that
/// already names a version is kept.
pub fn label_for(id: &str, display_name: &str) -> String {
    let display_name = display_name.trim();
    let mut depth = 0usize;
    let outside_parens: String = display_name
        .chars()
        .filter(|c| {
            match c {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                _ => {}
            }
            depth == 0 && *c != ')'
        })
        .collect();
    let generic = !outside_parens.chars().any(|c| c.is_ascii_digit());
    match (family_label(id), generic) {
        (Some(label), true) => label,
        (Some(_), false) => {
            if id.ends_with("[1m]") && !display_name.to_ascii_lowercase().contains("1m") {
                format!("{display_name} 1M")
            } else {
                display_name.to_string()
            }
        }
        (None, _) if !display_name.is_empty() => display_name.to_string(),
        (None, _) => id.to_string(),
    }
}

/// "Opus 5.5 1M" for `claude-opus-5-5[1m]`, "Fable 5.1" for `claude-fable-5-1`; None for any other shape.
fn family_label(id: &str) -> Option<String> {
    let (base, large) = match id.strip_suffix("[1m]") {
        Some(base) => (base, true),
        None => (id, false),
    };
    let rest = base.strip_prefix("claude-")?;
    let mut segments = rest.split('-');
    let family = segments.next().filter(|f| !f.is_empty() && f.chars().all(|c| c.is_ascii_alphabetic()))?;
    let version: Vec<&str> = segments.collect();
    if version.is_empty() || version.len() > 2 || version.iter().any(|v| v.is_empty() || !v.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    let mut chars = family.chars();
    let family = chars.next().map(|c| c.to_ascii_uppercase().to_string() + chars.as_str()).unwrap_or_default();
    let mut label = format!("{family} {}", version.join("."));
    if large {
        label.push_str(" 1M");
    }
    Some(label)
}

/// Resolve a short or retired alias to the identifier the catalogue lists for it, leaving every other name
/// untouched.
///
/// The one chokepoint for the chat engine's launch, `set_model` and `system/init` paths. The stored selection
/// a resting conversation shows goes through `fold_stored` instead, which folds only the spelling. With the
/// probe's pairs the CLI decides; without them the static table does. Either way the result is folded onto
/// the catalogue's spelling (`fold_id`).
pub(crate) fn normalize_id(id: &str) -> String {
    normalize_with(id, &cli_model_catalog::alias_pairs(), &fold_catalogue())
}

/// Fold only the spelling of a stored selection (`claude-opus-5-5[1m]`, a dated id) onto the catalogue's
/// identifier, without resolving aliases. What a resting conversation shows is sent back and persisted on
/// the next send, so resolving `opus` here would silently pin today's Opus into the user's own choice.
pub(crate) fn fold_stored(id: &str) -> String {
    fold_id(id, &fold_catalogue())
}

/// What `normalize_id` folds against: the website catalogue plus the whole bundled table.
fn fold_catalogue() -> Vec<ClaudeModel> {
    with_curated(super::remote_model_catalog::models(None).unwrap_or_default())
}

/// `normalize_id` over explicit `(value, resolvedModel)` pairs and an explicit catalogue to fold onto.
///
/// With pairs: an identifier the CLI offers keeps its spelling up to folding; a value the CLI knows becomes
/// what it resolves to; a static alias applies when its target is a model the CLI offers, and a retired
/// full spelling from the static table (`claude-opus-5[1m]`) keeps the rewrite the table always applied,
/// because its target is a real model id the CLI accepts even when its shortlist no longer names it; any
/// other short name is left to the CLI, which resolves it and reports the result in `system/init`.
/// Without pairs the static table applies as it always did.
fn normalize_with(id: &str, pairs: &[(String, String)], known: &[ClaudeModel]) -> String {
    let fold = |candidate: &str| fold_id(candidate, known);
    let static_target = ALIASES.iter().find(|(alias, _)| *alias == id).map(|(_, full)| *full);
    if pairs.is_empty() {
        return fold(static_target.unwrap_or(id));
    }
    if pairs.iter().any(|(_, resolved)| resolved == id) {
        return fold(id);
    }
    if let Some((_, resolved)) = pairs.iter().find(|(value, _)| value == id) {
        return fold(resolved);
    }
    match static_target {
        Some(target) if pairs.iter().any(|(_, resolved)| fold(resolved) == fold(target)) => fold(target),
        Some(target) if id.starts_with("claude-") => fold(target),
        _ => fold(id),
    }
}

/// Whether an entry is old enough for the installed CLI. An unknown version lists everything, on the
/// grounds that hiding a model the user has is worse than offering one they do not.
fn accepts(entry: &Entry, version: Option<(u32, u32, u32)>) -> bool {
    match (entry.min_version, version) {
        (Some(min), Some(have)) => have >= min,
        _ => true,
    }
}

/// Run `claude --version` and return the `major.minor.patch` it prints, or None when it does not answer
/// with a version within VERSION_TIMEOUT.
pub(crate) fn read_version_for_bin(bin: &str) -> Option<String> {
    let mut cmd = crate::host::command(bin);
    cmd.arg("--version");
    cmd.env("NO_COLOR", "1");
    let text = capture(cmd)?;
    parse_version(&text).map(|(a, b, c)| format!("{a}.{b}.{c}"))
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

/// Claude Code's `availableModels` allowlist from the settings the CLI reads for every session, or None
/// when no such setting restricts the choice. Tests never read the machine's own settings.
#[cfg(not(test))]
fn available_models() -> Option<Vec<String>> {
    available_models_in(config_dir().as_deref(), Some(Path::new(MANAGED_SETTINGS)))
}

#[cfg(test)]
fn available_models() -> Option<Vec<String>> {
    None
}

/// The file-based managed settings Claude Code reads. The macOS configuration profile, the Windows
/// registry and server-managed settings are other delivery channels this does not read.
/// Read only by the real settings chain, which tests replace with a fixed answer (hence the test allow).
#[cfg(target_os = "macos")]
#[cfg_attr(test, allow(dead_code))]
const MANAGED_SETTINGS: &str = "/Library/Application Support/ClaudeCode/managed-settings.json";
#[cfg(windows)]
#[cfg_attr(test, allow(dead_code))]
const MANAGED_SETTINGS: &str = r"C:\Program Files\ClaudeCode\managed-settings.json";
#[cfg(not(any(target_os = "macos", windows)))]
#[cfg_attr(test, allow(dead_code))]
const MANAGED_SETTINGS: &str = "/etc/claude-code/managed-settings.json";

/// `available_models` over an explicit configuration directory and managed settings file. A managed list
/// applies alone, as in Claude Code; otherwise the user's `settings.json` and `settings.local.json` lists
/// are concatenated and deduplicated. Project settings are not read, for the reason `settings_models`
/// gives. An empty list restricts nothing here.
fn available_models_in(config: Option<&Path>, managed: Option<&Path>) -> Option<Vec<String>> {
    let read = |path: &Path| std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok());
    let list = |json: &Value| {
        json.get("availableModels").and_then(Value::as_array).map(|entries| {
            entries
                .iter()
                .filter_map(Value::as_str)
                .map(|entry| entry.trim().to_ascii_lowercase())
                .filter(|entry| !entry.is_empty())
                .collect::<Vec<_>>()
        })
    };
    if let Some(managed) = managed.and_then(read).as_ref().and_then(list) {
        return (!managed.is_empty()).then_some(managed);
    }
    let mut out: Vec<String> = Vec::new();
    for file in ["settings.json", "settings.local.json"] {
        let Some(json) = config.and_then(|dir| read(&dir.join(file))) else { continue };
        for entry in list(&json).unwrap_or_default() {
            if !out.contains(&entry) {
                out.push(entry);
            }
        }
    }
    (!out.is_empty()).then_some(out)
}

/// Keep the models an `availableModels` allowlist permits, matching the way Claude Code documents it: a
/// family alias such as `opus` allows every model of that family; any other entry is a version prefix or a
/// full identifier and allows itself and every identifier that extends it by a further segment
/// (`claude-fable-5` allows Fable 5 and Fable 5.1, `claude-haiku-4-5` its dated form); a specific entry of a
/// family switches that family's alias off; a `[1m]` suffix is ignored on both sides. Without a list
/// nothing is filtered.
fn filter_available(models: Vec<ClaudeModel>, allow: Option<&[String]>) -> Vec<ClaudeModel> {
    let Some(allow) = allow else { return models };
    let entries: Vec<&str> = allow.iter().map(|e| e.strip_suffix("[1m]").unwrap_or(e)).collect();
    let is_family = |entry: &str| !entry.is_empty() && entry.chars().all(|c| c.is_ascii_alphabetic());
    let specific: Vec<&str> = entries.iter().copied().filter(|e| !is_family(e)).collect();
    let families: Vec<&str> = entries
        .iter()
        .copied()
        .filter(|e| is_family(e) && !specific.iter().any(|s| family_of(s) == Some(*e)))
        .collect();
    models
        .into_iter()
        .filter(|m| {
            // The allowlist entries are lowercased when read, so the identifier is compared the same way.
            let lower = m.id.to_ascii_lowercase();
            let id = lower.strip_suffix("[1m]").unwrap_or(&lower);
            specific.iter().any(|s| id == *s || id.strip_prefix(s).is_some_and(|rest| rest.starts_with('-')))
                || family_of(id).is_some_and(|family| families.contains(&family))
        })
        .collect()
}

/// `opus` for `claude-opus-4-6`, `haiku` for `claude-3-5-haiku-20241022`; None for any other shape.
fn family_of(id: &str) -> Option<&str> {
    id.strip_prefix("claude-")?
        .split('-')
        .find(|segment| !segment.is_empty() && segment.chars().all(|c| c.is_ascii_alphabetic()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cli_model_catalog::testing::fixture_models;

    fn fixture_rows() -> Vec<CliModel> {
        cli_model_catalog::parse_rows(&fixture_models())
    }

    fn entry(id: &str, label: &str) -> ClaudeModel {
        ClaudeModel {
            id: id.into(),
            label: label.into(),
            description: String::new(),
            effort_levels: vec![],
            context_window: None,
            curated: true,
            large_context: id.ends_with("[1m]"),
            supports_fast_mode: false,
            is_default: false,
        }
    }

    fn website(ids: &[&str]) -> Vec<ClaudeModel> {
        ids.iter().map(|id| entry(id, "Website")).collect()
    }

    fn ids(models: &[ClaudeModel]) -> Vec<&str> {
        models.iter().map(|m| m.id.as_str()).collect()
    }

    fn known() -> Vec<ClaudeModel> {
        with_curated(Vec::new())
    }

    fn row(value: &str, resolved: &str) -> CliModel {
        CliModel { value: value.into(), resolved_model: Some(resolved.into()), ..Default::default() }
    }

    /// The merge over every source combination: the catalogue is the base, CLI rows update or append,
    /// disabled rows remove, the bundled table stands in for a missing website catalogue, and the
    /// settings identifiers are appended once in every case.
    #[test]
    fn merge_table() {
        let rows = fixture_rows();
        let disabled = vec![CliModel { value: "claude-opus-4-8".into(), disabled: true, ..Default::default() }];
        let settings = vec![("my-gateway/opus".to_string(), "env.ANTHROPIC_MODEL".to_string())];
        let site = || Some(website(&["claude-opus-4-8", "claude-fable-5-1"]));
        let bundled_ids: Vec<&str> = MANIFEST.iter().map(|e| e.id).collect();
        let cases: Vec<(Option<&[CliModel]>, Option<Vec<ClaudeModel>>, Vec<&str>, &str)> = vec![
            (None, site(), vec!["claude-opus-4-8", "claude-fable-5-1"], "catalogue only"),
            (
                Some(&rows),
                site(),
                vec!["claude-opus-4-8", "claude-fable-5-1", "claude-opus-5-5"],
                "catalogue + CLI: the CLI's new id is appended, folded onto the bundled spelling",
            ),
            (Some(&disabled), site(), vec!["claude-fable-5-1"], "a disabled row removes its entry"),
            (Some(&[]), site(), vec!["claude-opus-4-8", "claude-fable-5-1"], "an empty CLI list changes nothing"),
            (Some(&rows), None, bundled_ids.clone(), "website unavailable: bundled + CLI, nothing duplicated"),
            (None, None, bundled_ids.clone(), "bundled only"),
        ];
        for (cli, site, expected, why) in cases {
            let models = assemble_with(cli, &[], None, site, &settings, &[]);
            let mut expected = expected.clone();
            expected.push("my-gateway/opus");
            assert_eq!(ids(&models), expected, "{why}");
            assert!(!models.iter().any(|m| m.id == "default"), "{why}");
        }
        // The appended CLI model: the default mark, the CLI's capabilities, the catalogue's single native
        // 1M spelling and label.
        let models = assemble_with(Some(&rows), &[], None, site(), &[], &[]);
        let opus = models.iter().find(|m| m.id == "claude-opus-5-5").unwrap();
        assert!(opus.is_default, "the default row marks its target");
        assert_eq!(opus.label, "Opus 5.5");
        assert!(!opus.large_context, "a natively 1M model is not an expanded-context request");
        assert_eq!(opus.context_window, Some(1_000_000));
        assert!(opus.supports_fast_mode);
        assert_eq!(opus.effort_levels, ["low", "medium", "high", "xhigh", "max"]);
        assert_eq!(opus.description, "Opus 5.5 with 1M context · Best for everyday, complex tasks");
        // A CLI row updating a catalogue entry: capabilities from the CLI, id and label from the catalogue,
        // the description only because the catalogue had none.
        let fable = models.iter().find(|m| m.id == "claude-fable-5-1").unwrap();
        assert_eq!(fable.label, "Website");
        assert_eq!(fable.effort_levels, ["low", "medium", "high", "xhigh", "max"]);
        assert_eq!(fable.description, "Fable 5.1 · Most capable for your hardest and longest-running tasks");
        assert!(!fable.is_default && !fable.supports_fast_mode);
        let mut described = website(&["claude-fable-5-1"]);
        described[0].description = "From the website".into();
        let models = assemble_with(Some(&rows), &[], None, Some(described), &[], &[]);
        assert_eq!(models[0].description, "From the website");
        // Website unavailable: the maintainer's list survives the CLI's shortlist.
        let models = assemble_with(Some(&rows), &[], None, None, &[], &[]);
        for id in ["claude-opus-4-8", "claude-sonnet-5[1m]", "claude-opus-4-7", "claude-opus-4-6", "claude-fable-5"] {
            assert!(models.iter().any(|m| m.id == id), "{id} must stay selectable");
        }
        assert_eq!(models.iter().filter(|m| m.is_default).map(|m| m.id.as_str()).collect::<Vec<_>>(), ["claude-opus-5-5"]);
        // A version filter hiding the bundled entry does not change the spelling the CLI row folds onto.
        let models = assemble_with(Some(&rows), &[], Some((2, 1, 200)), None, &[], &[]);
        assert!(models.iter().any(|m| m.id == "claude-opus-5-5"));
        assert!(!models.iter().any(|m| m.id == "claude-opus-5-5[1m]" || m.id == "claude-opus-5"));
        // A settings identifier already listed is not duplicated.
        let settings = vec![("claude-fable-5-1".to_string(), "env.ANTHROPIC_MODEL".to_string())];
        assert_eq!(assemble_with(Some(&rows), &[], None, site(), &settings, &[]).len(), 3);
        // A disabled row removes its model even when another row resolves to it.
        let mut rows_disabled = fixture_rows();
        rows_disabled.push(CliModel { value: "fable".into(), resolved_model: Some("claude-fable-5-1".into()), disabled: true, ..Default::default() });
        assert!(!assemble_with(Some(&rows_disabled), &[], None, None, &[], &[]).iter().any(|m| m.id == "claude-fable-5-1"));
        // A default resolving to a model nothing else lists is appended once, as the default.
        let only_default = vec![CliModel { value: "default".into(), resolved_model: Some("claude-sonnet-9".into()), display_name: "Default (recommended)".into(), description: "Sonnet".into(), ..Default::default() }];
        let models = assemble_with(Some(&only_default), &[], None, site(), &[], &[]);
        assert_eq!(ids(&models), ["claude-opus-4-8", "claude-fable-5-1", "claude-sonnet-9"]);
        assert_eq!(models[2].label, "Sonnet 9");
        assert!(models[2].is_default);
    }

    /// Additions only ever append: they neither change an entry's capabilities nor bring back what a
    /// disabled row refused.
    #[test]
    fn additions_append_and_never_change_or_remove() {
        let mut fast = row("claude-fable-5-1", "claude-fable-5-1");
        fast.supports_fast_mode = true;
        fast.supported_effort_levels = vec!["low".into()];
        let additions = vec![fast, row("claude-opus-6", "claude-opus-6"), row("claude-opus-4-8", "claude-opus-4-8")];
        let refusing = vec![CliModel { value: "claude-opus-4-8".into(), disabled: true, ..Default::default() }];
        let models = assemble_with(Some(&refusing), &additions, None, Some(website(&["claude-opus-4-8", "claude-fable-5-1"])), &[], &[]);
        assert_eq!(ids(&models), ["claude-fable-5-1", "claude-opus-6"]);
        assert!(!models[0].supports_fast_mode && models[0].effort_levels.is_empty());
        assert_eq!(models[1].label, "Opus 6");
    }

    /// The wiring behind `assemble_with`: the website module is the base, the CLI rows are merged into it.
    #[test]
    fn website_catalogue_is_the_base_and_the_cli_is_merged_in() {
        let _serial = cli_model_catalog::TEST_LOCK.lock().unwrap();
        let catalog = serde_json::json!({"schemaVersion": 1, "revision": 7, "models": [{
            "id": "test-website-model", "label": "Website model", "description": "From the website",
            "contextWindow": 200000, "effortLevels": ["low", "high"]
        }]});
        super::super::remote_model_catalog::set_for_tests(Some(&serde_json::to_vec(&catalog).unwrap()));
        let models = assemble(None, &[], None);
        assert_eq!(models[0].id, "test-website-model");
        let with_cli = assemble(Some(&fixture_rows()), &[], None);
        assert_eq!(with_cli[0].id, "test-website-model");
        assert!(with_cli.iter().any(|m| m.id == "claude-opus-5-5" && m.is_default));
        super::super::remote_model_catalog::set_for_tests(None);
        assert_eq!(assemble(None, &[], None)[0].id, MANIFEST[0].id);
    }

    /// AC2: one folding rule for the merge, `normalize_id` and the chip.
    #[test]
    fn folds_native_1m_and_dated_spellings_but_keeps_expanded_variants() {
        let known = known();
        for (id, folded) in [
            ("claude-opus-5-5[1m]", "claude-opus-5-5"),
            ("claude-fable-5-1[1m]", "claude-fable-5-1"),
            ("claude-opus-5[1m]", "claude-opus-5"),
            ("claude-haiku-4-5-20251001", "claude-haiku-4-5"),
            // Explicit expanded-context variants of standard-context models stay separate.
            ("claude-opus-4-6[1m]", "claude-opus-4-6[1m]"),
            ("claude-sonnet-5[1m]", "claude-sonnet-5[1m]"),
            ("claude-opus-4-8", "claude-opus-4-8"),
            // Unknown models keep their spelling.
            ("claude-opus-6[1m]", "claude-opus-6[1m]"),
            ("claude-opus-9-20300101", "claude-opus-9-20300101"),
            ("my-gateway/opus", "my-gateway/opus"),
        ] {
            assert_eq!(fold_id(id, &known), folded, "{id}");
        }
        // A catalogue that lists a suffixed entry of its own keeps it.
        let mut listed = website(&["claude-opus-5-5", "claude-opus-5-5[1m]"]);
        listed[0].context_window = Some(1_000_000);
        assert_eq!(fold_id("claude-opus-5-5[1m]", &listed), "claude-opus-5-5[1m]");
        // A website entry without a window falls back to what the bundled table knows about it.
        assert_eq!(fold_id("claude-opus-5-5[1m]", &website(&["claude-opus-5-5"])), "claude-opus-5-5");
        assert_eq!(fold_id("claude-sonnet-5[1m]", &website(&["claude-sonnet-5"])), "claude-sonnet-5[1m]");
        // Stored selections with either spelling resolve to the same menu row, with and without CLI data.
        let pairs = cli_model_catalog::pairs_of(fixture_rows().iter());
        let haiku = vec![("haiku".to_string(), "claude-haiku-4-5-20251001".to_string())];
        for pairs in [&pairs[..], &haiku[..], &[]] {
            for (stored, row) in [
                ("claude-opus-5-5[1m]", "claude-opus-5-5"),
                ("claude-opus-5-5", "claude-opus-5-5"),
                ("claude-haiku-4-5-20251001", "claude-haiku-4-5"),
                ("claude-haiku-4-5", "claude-haiku-4-5"),
                ("claude-opus-4-6[1m]", "claude-opus-4-6[1m]"),
            ] {
                assert_eq!(normalize_with(stored, pairs, &known), row, "{stored} with {pairs:?}");
            }
        }
        let menu = assemble_with(Some(&fixture_rows()), &[], None, None, &[], &[]);
        assert!(menu.iter().any(|m| m.id == normalize_with("claude-opus-5-5[1m]", &pairs, &known)));
    }

    /// The CLI's own pairs decide before the static table, and the result is folded.
    #[test]
    fn normalize_id_prefers_cli_pairs() {
        let known = known();
        let pairs = cli_model_catalog::pairs_of(fixture_rows().iter());
        assert_eq!(normalize_with("opus[1m]", &pairs, &known), "claude-opus-5-5");
        assert_eq!(normalize_with("claude-fable-5-1", &pairs, &known), "claude-fable-5-1");
        // The static table resolves `opus` because its target is the model the CLI offers.
        assert_eq!(normalize_with("opus", &pairs, &known), "claude-opus-5-5");
        // Once the CLI offers another generation, a short name the CLI does not list is left to the CLI.
        let newer = vec![("opus[1m]".to_string(), "claude-opus-6".to_string())];
        assert_eq!(normalize_with("opus", &newer, &known), "opus");
        // A retired full spelling keeps the table's rewrite even when the CLI's shortlist no longer names
        // the target: claude-opus-5 is a model id the CLI accepts.
        assert_eq!(normalize_with("claude-opus-5[1m]", &pairs, &known), "claude-opus-5");
        // A row without resolvedModel still counts as CLI data and is never rewritten by the table.
        assert_eq!(normalize_with("opus", &[("opus".to_string(), "opus".to_string())], &known), "opus");
        assert!(!pairs.iter().any(|(value, _)| value == "default"));
        assert_eq!(normalize_with("my-gateway/opus", &pairs, &known), "my-gateway/opus");
        // Without CLI data the static table applies, folded.
        assert_eq!(normalize_with("opus", &[], &known), "claude-opus-5-5");
        assert_eq!(normalize_with("opus[1m]", &[], &known), "claude-opus-5-5");
        assert_eq!(normalize_with("claude-opus-5[1m]", &[], &known), "claude-opus-5");
        assert_eq!(normalize_with("my-gateway/opus", &[], &known), "my-gateway/opus");
    }

    /// Acceptance is at least as wide as the offer plus the curated table, without duplicates.
    #[test]
    fn accepted_selection_includes_the_curated_table() {
        let offered = assemble_with(Some(&fixture_rows()), &[], None, Some(website(&["test-website-model"])), &[], &[]);
        let accepted = accepted(offered.clone(), None);
        for m in &offered {
            assert!(accepted.iter().any(|a| a.id == m.id), "{} offered but not accepted", m.id);
        }
        for e in MANIFEST {
            assert!(accepted.iter().any(|a| a.id == e.id), "{} curated but not accepted", e.id);
        }
        let mut seen: Vec<&str> = ids(&accepted);
        let before = seen.len();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), before, "no identifier appears twice");
    }

    /// The class, not the instance: whatever the menu offered when a job was saved keeps validating after
    /// the CLI's shortlist moved on, as long as it is a well-formed Claude id; anything else stays out.
    #[test]
    fn accepted_selection_keeps_any_well_formed_stored_claude_id() {
        let offered = with_curated(Vec::new());
        assert!(!offered.iter().any(|m| m.id == "claude-opus-6-1[1m]"));
        let kept = accepted(offered.clone(), Some("claude-opus-6-1[1m]"));
        let row = kept.iter().find(|m| m.id == "claude-opus-6-1[1m]").expect("stored id accepted");
        assert!(row.effort_levels.iter().any(|l| l == "xhigh"));
        assert!(row.large_context && !row.curated);
        for bad in ["--dangerously-skip-permissions", "gpt-5", "claude-", "claude-opus 5", "claude-opus-5;rm", "Claude-Opus-5", "claude-opus-5[2m]"] {
            assert_eq!(accepted(offered.clone(), Some(bad)).len(), offered.len(), "{bad} must not be accepted");
        }
        // A curated id is not duplicated.
        assert_eq!(accepted(offered.clone(), Some("claude-opus-5")).len(), offered.len());
    }

    /// AC5: Claude Code's `availableModels` matching.
    #[test]
    fn available_models_filter_matches_claude_code() {
        let menu = || {
            let mut models = with_curated(Vec::new());
            models.push(entry("claude-haiku-4-5-20251001", "Haiku dated"));
            models.push(entry("my-gateway/opus", "Gateway"));
            models
        };
        let allow = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // Without the setting nothing is filtered.
        assert_eq!(filter_available(menu(), None).len(), menu().len());
        // Manuel's list: every Opus; Fable only as named, because a specific Fable entry switches the
        // `fable` alias off; `claude-fable-5` also allows Fable 5.1.
        let manuel = allow(&["fable", "claude-fable-5", "opus"]);
        let kept = filter_available(menu(), Some(&manuel));
        let kept = ids(&kept);
        for id in ["claude-opus-5-5", "claude-opus-5", "claude-opus-4-8[1m]", "claude-opus-4-8", "claude-opus-4-6", "claude-fable-5-1", "claude-fable-5"] {
            assert!(kept.contains(&id), "{id} must be allowed");
        }
        assert!(!kept.iter().any(|id| id.contains("sonnet") || id.contains("haiku") || id.starts_with("my-gateway")));
        // A full id allows itself and its dated form, not its siblings; `[1m]` is ignored on both sides.
        let haiku = allow(&["claude-haiku-4-5"]);
        assert_eq!(ids(&filter_available(menu(), Some(&haiku))), ["claude-haiku-4-5", "claude-haiku-4-5-20251001"]);
        let fable51 = allow(&["claude-fable-5-1"]);
        assert_eq!(ids(&filter_available(menu(), Some(&fable51))), ["claude-fable-5-1"]);
        let opus46 = allow(&["claude-opus-4-6[1m]"]);
        assert_eq!(ids(&filter_available(menu(), Some(&opus46))), ["claude-opus-4-6[1m]", "claude-opus-4-6"]);
        let sonnet = allow(&["sonnet"]);
        assert!(filter_available(menu(), Some(&sonnet)).iter().all(|m| m.id.contains("sonnet")));
        assert_eq!(ids(&filter_available(menu(), Some(&allow(&["my-gateway/opus"])))), ["my-gateway/opus"]);
        // Entries are lowercased when read; an identifier with capitals is compared the same way.
        let mut mixed = menu();
        mixed.push(entry("MyGW/Opus", "Gateway with capitals"));
        assert_eq!(ids(&filter_available(mixed, Some(&allow(&["mygw/opus"])))), ["MyGW/Opus"]);
    }

    /// What a resting conversation shows is persisted on the next send, so only the spelling is folded:
    /// aliases stay the user's choice instead of being pinned to today's model.
    #[test]
    fn stored_selection_folds_spelling_but_keeps_aliases() {
        // The website catalogue is global test state; hold the lock every writer of it holds.
        let _serial = cli_model_catalog::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        super::super::remote_model_catalog::set_for_tests(None);
        assert_eq!(fold_stored("claude-opus-5-5[1m]"), "claude-opus-5-5");
        assert_eq!(fold_stored("claude-haiku-4-5-20251001"), "claude-haiku-4-5");
        assert_eq!(fold_stored("opus"), "opus");
        assert_eq!(fold_stored("sonnet"), "sonnet");
        assert_eq!(fold_stored("claude-opus-4-6[1m]"), "claude-opus-4-6[1m]");
    }

    /// AC5: where the list comes from. A managed list applies alone; otherwise the user's two files are
    /// concatenated and deduplicated; no or an empty list restricts nothing.
    #[test]
    fn available_models_are_read_from_the_settings_chain() {
        let dir = std::env::temp_dir().join(format!("vlx-available-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let managed = dir.join("managed-settings.json");
        assert_eq!(available_models_in(Some(&dir), Some(&managed)), None);
        std::fs::write(dir.join("settings.json"), r#"{"availableModels": ["fable", "claude-fable-5", " Opus "]}"#).unwrap();
        std::fs::write(dir.join("settings.local.json"), r#"{"availableModels": ["opus", "sonnet"]}"#).unwrap();
        assert_eq!(available_models_in(Some(&dir), Some(&managed)).unwrap(), ["fable", "claude-fable-5", "opus", "sonnet"]);
        std::fs::write(&managed, r#"{"availableModels": ["haiku"]}"#).unwrap();
        assert_eq!(available_models_in(Some(&dir), Some(&managed)).unwrap(), ["haiku"]);
        std::fs::write(&managed, r#"{"availableModels": []}"#).unwrap();
        assert_eq!(available_models_in(Some(&dir), Some(&managed)), None);
        std::fs::write(&managed, r#"{"model": "opus"}"#).unwrap();
        std::fs::write(dir.join("settings.json"), "not json").unwrap();
        assert_eq!(available_models_in(Some(&dir), Some(&managed)).unwrap(), ["opus", "sonnet"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// AC5
    #[test]
    fn label_formatter() {
        assert_eq!(label_for("claude-opus-5-5[1m]", "Opus (1M context)"), "Opus 5.5 1M");
        assert_eq!(label_for("claude-fable-5-1", "Fable"), "Fable 5.1");
        assert_eq!(label_for("claude-sonnet-4-6", "Sonnet"), "Sonnet 4.6");
        assert_eq!(label_for("claude-haiku-4-5", "Haiku"), "Haiku 4.5");
        assert_eq!(label_for("claude-opus-5-5[1m]", "Default (recommended)"), "Opus 5.5 1M");
        assert_eq!(label_for("claude-opus-5", ""), "Opus 5");
        // A display name that names the generation is kept, with the window added when the id asks for it.
        assert_eq!(label_for("claude-opus-4-6", "Opus 4.6"), "Opus 4.6");
        assert_eq!(label_for("claude-opus-4-6[1m]", "Opus 4.6"), "Opus 4.6 1M");
        assert_eq!(label_for("claude-opus-4-6[1m]", "Opus 4.6 (1M)"), "Opus 4.6 (1M)");
        // Unknown shapes fall back to the display name, then the id.
        assert_eq!(label_for("claude-opus-4-8-20260101", "Opus"), "Opus");
        assert_eq!(label_for("my-gateway/opus", "Gateway Opus"), "Gateway Opus");
        assert_eq!(label_for("my-gateway/opus", ""), "my-gateway/opus");
        assert_eq!(label_for("claude-opus-4-6", "Opus (1M context)"), "Opus 4.6");
    }

    #[test]
    fn parses_the_version_line_claude_prints() {
        assert_eq!(parse_version("2.1.252 (Claude Code)"), Some((2, 1, 252)));
        assert_eq!(parse_version("nothing here"), None);
    }

    #[test]
    fn hides_entries_the_installed_cli_predates() {
        let opus5 = MANIFEST.iter().find(|e| e.id == "claude-opus-5").unwrap();
        assert!(!accepts(opus5, Some((2, 1, 200))));
        assert!(accepts(opus5, Some((2, 1, 219))));
        // An unknown version must not hide anything.
        assert!(accepts(opus5, None));
        assert!(!bundled(Some((2, 1, 200))).iter().any(|m| m.id == "claude-opus-5"));
        let opus55 = MANIFEST.iter().find(|e| e.id == "claude-opus-5-5").unwrap();
        assert!(!accepts(opus55, Some((2, 1, 279))));
        assert!(accepts(opus55, Some((2, 1, 280))));
    }

    #[test]
    fn keeps_large_context_variants_beside_their_standard_model() {
        let models = assemble_with(None, &[], None, None, &[], &[]);
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
            assert_eq!(normalize_with(alias, &[], &known()), *full);
            assert!(
                ids.contains(full),
                "{full} is aliased but missing from the manifest"
            );
        }
    }

    #[test]
    fn offers_one_native_one_million_entry_per_opus_5_generation() {
        for id in ["claude-opus-5", "claude-opus-5-5"] {
            let entries: Vec<&str> = MANIFEST
                .iter()
                .filter(|entry| normalize_with(entry.id, &[], &known()) == id)
                .map(|entry| entry.id)
                .collect();
            assert_eq!(entries, vec![id]);
        }
        let opus55 = MANIFEST.iter().find(|e| e.id == "claude-opus-5-5").unwrap();
        assert_eq!((opus55.label, opus55.context_window, opus55.min_version), ("Opus 5.5", 1_000_000, Some((2, 1, 280))));
        assert!(!opus55.wants_large_context());
    }

    #[test]
    fn keeps_the_one_million_variants_as_separate_ids() {
        let ids: Vec<&str> = MANIFEST.iter().map(|e| e.id).collect();
        assert!(ids.contains(&"claude-opus-4-6"));
        assert!(ids.contains(&"claude-opus-4-6[1m]"));
    }
}
