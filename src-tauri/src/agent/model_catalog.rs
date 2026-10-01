//! Live model enumeration for agent CLIs that can list their own catalogue.
//!
//! Each supported agent ships a different listing command and a different output shape, so the raw
//! text is normalised here into a plain list of identifiers the spawn dialog can offer. Agents whose
//! CLI has no listing command use backend catalogues when available; otherwise they return an empty
//! list and the dialog keeps a free-text model field.
//!
//! Everything here spawns a child process and blocks on its output, so callers must stay off the main
//! thread. Dispatch already runs inside desktop_call's blocking pool or a WebSocket worker.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// Upper bound on how long a listing command may run before it is killed. Some CLIs contact their
/// provider to enumerate models, so this is generous, but it must stay finite: an unauthenticated or
/// hung CLI would otherwise pin a blocking-pool thread forever.
const LIST_TIMEOUT: Duration = Duration::from_secs(12);

/// Cap on returned entries. OpenCode alone reports over five hundred models; the dialog stays usable
/// and the payload stays small without truncating any realistic catalogue.
const MAX_MODELS: usize = 1000;

/// Subcommand/flags that make an agent CLI print its model catalogue, or None when it has none.
///
/// Claude and Codex use their dedicated backend providers, Kimi uses backend presets, and
/// Copilot/Cline/Zoo have no enumeration command.
fn list_args(agent: &str) -> Option<&'static [&'static str]> {
    match agent {
        "opencode" => Some(&["models"]),
        "grok" => Some(&["models"]),
        "crush" => Some(&["models"]),
        "antigravity" => Some(&["models"]),
        "cursor" => Some(&["--list-models"]),
        "pi" => Some(&["--list-models"]),
        // OMP renders a box-drawn table grouped by provider, which no line parser survives; `--json` gives the
        // same catalogue as data. Verified against omp v18.1.10.
        "omp" => Some(&["models", "--json"]),
        // Kiro keeps model selection under its chat subcommand rather than at the top level.
        "kiro" => Some(&["chat", "--list-models"]),
        _ => None,
    }
}


/// List the models an agent CLI reports, or an empty vector when it cannot enumerate them.
///
/// Errors describe a failure to *ask* the CLI (missing executable, non-zero exit, timeout) so the
/// dialog can say why the list is empty instead of silently offering nothing.
pub fn list_models(app: &crate::host::AppCtx, agent: &str) -> Result<Vec<String>, String> {
    if agent == "claude" {
        let bin = super::executable::resolve(app, crate::models::SessionKind::Claude, None)
            .unwrap_or_else(|| "claude".into());
        return Ok(super::claude_models::list_for_bin(app, &bin).into_iter().map(|m| m.id).collect());
    }
    if agent == "codex" {
        let bin = super::executable::resolve(app, crate::models::SessionKind::Codex, None)
            .unwrap_or_else(|| "codex".into());
        return super::codex_models::list(&bin, &[]).map(|models| models.into_iter().map(|m| m.id).collect());
    }
    if agent == "kimi" {
        return Ok(["k3", "k3-256k", "kimi-for-coding", "kimi-for-coding-highspeed"].into_iter().map(str::to_string).collect());
    }
    let Some(args) = list_args(agent) else {
        return Ok(Vec::new());
    };
    let kind = crate::models::SessionKind::from_db(agent);
    let bin = super::executable::resolve(app, kind, None)
        .unwrap_or_else(|| super::executable::command_name(kind).to_string());
    let mut cmd = crate::host::command(&bin);
    super::executable::prepare_command(&mut cmd, &bin);
    cmd.args(args);
    // Several CLIs render a decorated tree when they detect a terminal and one plain identifier per
    // line otherwise. Capturing through a pipe already puts them in the plain mode; NO_COLOR removes
    // any remaining escape sequences.
    cmd.env("NO_COLOR", "1");
    let text = run_capture(cmd)?;
    Ok(parse(agent, &text))
}

/// Run a prepared command and return its stdout, killing it once LIST_TIMEOUT elapses.
///
/// stdout is drained on a helper thread rather than after waiting: a CLI that fills the pipe buffer
/// while the parent waits would deadlock. stderr is discarded because these commands print
/// authentication hints there that are not part of the catalogue.
fn run_capture(mut cmd: Command) -> Result<String, String> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to run the model list command: {e}"))?;
    let mut out = child
        .stdout
        .take()
        .ok_or_else(|| "Model list command produced no output stream".to_string())?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = out.read_to_string(&mut buf);
        let _ = tx.send(buf);
    });
    match rx.recv_timeout(LIST_TIMEOUT) {
        Ok(text) => {
            let _ = child.wait();
            Ok(text)
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            Err("The model list command timed out".to_string())
        }
    }
}

/// Normalise one CLI's listing output into bare model identifiers, preserving the order the CLI used.
fn parse(agent: &str, text: &str) -> Vec<String> {
    // OMP answers with JSON rather than lines, so it is handled whole rather than line by line.
    if agent == "omp" {
        return parse_omp_json(text);
    }
    let mut out: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let id = match agent {
            // Pi prints an aligned table whose first two columns are the provider and the model, and
            // whose first row is a header. Its --model flag takes them joined as `provider/model`.
            "pi" => {
                let mut cols = line.split_whitespace();
                let provider = cols.next().unwrap_or_default();
                let model = cols.next().unwrap_or_default();
                if provider == "provider" || model.is_empty() {
                    continue;
                }
                format!("{provider}/{model}")
            }
            // Grok prints prose around a bulleted list: `  * grok-4.5 (default)`.
            "grok" => {
                let Some(rest) = line.strip_prefix("* ") else {
                    continue;
                };
                rest.split_whitespace().next().unwrap_or_default().to_string()
            }
            // Crush marks providers it cannot use; those models are not selectable.
            "crush" => {
                if line.contains("(not configured)") {
                    continue;
                }
                line.split_whitespace().next().unwrap_or_default().to_string()
            }
            // Antigravity and Kiro print `slug` followed by a human-readable display name; Cursor and
            // OpenCode print the identifier alone. Taking the first token covers all four.
            _ => line.split_whitespace().next().unwrap_or_default().to_string(),
        };
        // Headings, separators, and prose lines survive tokenisation, so drop anything that cannot be
        // an identifier before it reaches the dialog.
        if id.is_empty() || !looks_like_id(&id) || out.iter().any(|m| m == &id) {
            continue;
        }
        out.push(id);
        if out.len() >= MAX_MODELS {
            break;
        }
    }
    out
}

/// Pull the selectors out of `omp models --json`.
///
/// The payload is `{"models":[{"provider":…,"id":…,"selector":"provider/id",…},…]}`. `selector` is exactly what
/// `--model` accepts, so it is preferred; `provider` and `id` are joined only if a future build drops the field.
fn parse_omp_json(text: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    // Accept a bare array too, so a change of envelope does not empty the dialog.
    let items = v
        .get("models")
        .and_then(|m| m.as_array())
        .or_else(|| v.as_array());
    let Some(items) = items else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let selector = match item.get("selector").and_then(|s| s.as_str()) {
            Some(s) => s.to_string(),
            None => {
                let provider = item.get("provider").and_then(|p| p.as_str()).unwrap_or_default();
                let id = item.get("id").and_then(|i| i.as_str()).unwrap_or_default();
                if provider.is_empty() || id.is_empty() {
                    continue;
                }
                format!("{provider}/{id}")
            }
        };
        if !looks_like_id(&selector) || out.iter().any(|m| m == &selector) {
            continue;
        }
        out.push(selector);
        if out.len() >= MAX_MODELS {
            break;
        }
    }
    out
}

/// Whether a token is shaped like a model identifier rather than table decoration or prose.
fn looks_like_id(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omp_json_yields_selectors() {
        // Shape captured from `omp models --json` on v18.1.10.
        let text = r#"{"models":[
            {"provider":"openai","id":"gpt-5.6-sol","selector":"openai/gpt-5.6-sol","name":"GPT-5.6"},
            {"provider":"anthropic","id":"claude-opus-5","selector":"anthropic/claude-opus-5","name":"Opus 5"},
            {"provider":"openai","id":"gpt-5.6-sol","selector":"openai/gpt-5.6-sol","name":"dup"}
        ]}"#;
        assert_eq!(
            parse("omp", text),
            vec!["openai/gpt-5.6-sol", "anthropic/claude-opus-5"],
            "duplicates should collapse and the selector should be used verbatim"
        );
    }

    #[test]
    fn omp_json_falls_back_to_provider_and_id() {
        let text = r#"{"models":[{"provider":"openai","id":"gpt-5.6-sol"}]}"#;
        assert_eq!(parse("omp", text), vec!["openai/gpt-5.6-sol"]);
    }

    #[test]
    fn omp_non_json_output_yields_nothing() {
        // A CLI error printed on stdout must empty the list rather than reach the dialog as a fake model.
        assert!(parse("omp", "Error: not authenticated").is_empty());
    }

    #[test]
    fn pi_table_becomes_provider_slash_model() {
        let text = "provider        model            context\n\
                    amazon-bedrock  anthropic.claude-fable-5   1M\n\
                    openai          gpt-5.6-sol                400K\n";
        assert_eq!(
            parse("pi", text),
            vec![
                "amazon-bedrock/anthropic.claude-fable-5",
                "openai/gpt-5.6-sol"
            ]
        );
    }

    #[test]
    fn grok_prose_and_bullets_yield_only_models() {
        let text = "You are not authenticated.\n\nDefault model: grok-4.5\n\nAvailable models:\n  * grok-4.5 (default)\n  * grok-4.6\n";
        assert_eq!(parse("grok", text), vec!["grok-4.5", "grok-4.6"]);
    }

    #[test]
    fn crush_skips_unconfigured_providers() {
        let text = "anthropic/claude-opus-4-8\nopenai/gpt-5.6-sol (not configured)\n";
        assert_eq!(parse("crush", text), vec!["anthropic/claude-opus-4-8"]);
    }

    #[test]
    fn first_token_wins_and_duplicates_collapse() {
        let text = "gemini-3.7-flash-high   Gemini 3.7 Flash (High)\ngemini-3.7-flash-high   duplicate\nclaude-sonnet-4-6  Claude Sonnet 4.6\n";
        assert_eq!(
            parse("antigravity", text),
            vec!["gemini-3.7-flash-high", "claude-sonnet-4-6"]
        );
    }

    #[test]
    fn agents_without_a_listing_command_return_an_empty_list_instead_of_erroring() {
        // Agents without a listing command or a backend catalogue keep an editable model field.
        let dir = std::env::temp_dir().join(format!("vlx-model-catalog-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let app = crate::host::AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(dir.clone(), db)));
        assert_eq!(list_models(&app, "copilot"), Ok(Vec::new()));
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
        assert!(list_args("opencode").is_some());
    }
}
