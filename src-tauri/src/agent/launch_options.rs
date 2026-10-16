//! Launch choices shared by the single-session and batch confirmation dialogs.
//! The backend owns CLI capabilities; clients hold only the user's unsubmitted selections.

use crate::models::SessionKind;
use serde::{Deserialize, Serialize};

/// Reusable backend-owned Agent/model/effort choice for any one-shot agent task.
///
/// Feature settings persist this neutral shape; [`apply_selection`] is the only place that converts it
/// into CLI-specific flags. Empty model or effort values deliberately mean the agent's own default.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSelection {
    pub agent: SessionKind,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub effort: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOption {
    pub supports_plan_execute: bool,
    /// 调度角色由 Harness 能力决定。
    pub plan_execute_roles: Vec<&'static str>,
    /// Whether this CLI has a verified one-shot adapter for reference summarization.
    pub supports_refer_summary: bool,
    pub id: SessionKind,
    pub label: &'static str,
    pub accepts_task: bool,
    pub effort_flag: Option<&'static str>,
    pub effort_levels: Vec<&'static str>,
}

/// Display name for one agent kind.
///
/// The catalogue is the single source of these names, so a feature that lists agents outside the spawn
/// dialog spells them the same way rather than keeping its own copy.
pub fn label(kind: SessionKind) -> String {
    catalog()
        .into_iter()
        .find(|option| option.id == kind)
        .map(|option| option.label.to_string())
        .unwrap_or_else(|| kind.as_str().to_string())
}

pub fn catalog() -> Vec<LaunchOption> {
    use SessionKind::*;
    let entries: &[(SessionKind, &str, Option<&str>, &[&str])] = &[
        (
            Claude,
            "Claude",
            Some("--effort"),
            &["low", "medium", "high", "xhigh", "max"],
        ),
        (Codex, "Codex", Some("model_reasoning_effort"), &[]),
        // 仅作启动选择的持久格式；启动原生 app-server 前转为 RPC 参数并剥离。
        (Zcode, "Zcode", Some("--effort"), &[]),
        (Opencode, "OpenCode", Some("--variant"), &[]),
        (
            Copilot,
            "Copilot",
            Some("--effort"),
            &["none", "low", "medium", "high"],
        ),
        (Cursor, "Cursor", None, &[]),
        (
            Antigravity,
            "Antigravity",
            Some("--effort"),
            super::chat::antigravity_protocol::EFFORTS,
        ),
        (
            Cline,
            "Cline",
            Some("--thinking"),
            &["none", "low", "medium", "high", "xhigh"],
        ),
        (
            Pi,
            "Pi",
            Some("--thinking"),
            &["off", "minimal", "low", "medium", "high", "xhigh", "max"],
        ),
        (
            Omp,
            "OMP",
            Some("--thinking"),
            &["off", "minimal", "low", "medium", "high", "xhigh", "max"],
        ),
        (Crush, "Crush", None, &[]),
        (Kimi, "Kimi Code", None, &[]),
        (
            Kiro,
            "Kiro",
            Some("--effort"),
            &["low", "medium", "high", "xhigh", "max"],
        ),
        (
            Grok,
            "Grok Build",
            Some("--reasoning-effort"),
            &["none", "minimal", "low", "medium", "high", "xhigh", "max"],
        ),
        (
            Zoo,
            "Zoo Code",
            Some("--reasoning-effort"),
            &[
                "unspecified",
                "disabled",
                "none",
                "minimal",
                "low",
                "medium",
                "high",
                "xhigh",
            ],
        ),
        (Terminal, "Terminal", None, &[]),
    ];
    entries
        .iter()
        .map(|(id, label, effort_flag, levels)| LaunchOption {
            supports_plan_execute: super::plan_execute::supported(*id),
            plan_execute_roles: if super::plan_execute::supported(*id) { vec!["plan", "exec", "review"] } else { vec![] },
            supports_refer_summary: super::headless::spec(*id).is_some(),
            id: *id,
            label,
            accepts_task: *id != Terminal,
            effort_flag: *effort_flag,
            effort_levels: levels.to_vec(),
        })
        .collect()
}

/// Source spans retain the exact spelling of unrelated custom arguments, including quoting and paths.
fn tokens(text: &str) -> Vec<&str> {
    let mut spans = Vec::new();
    let mut start = None;
    let mut quote = None;
    let mut escaped = false;
    for (i, c) in text.char_indices() {
        if start.is_none() && !c.is_whitespace() {
            start = Some(i);
        }
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' && quote != Some('\'') {
            escaped = true;
            continue;
        }
        if Some(c) == quote {
            quote = None;
        } else if quote.is_none() && (c == '\'' || c == '"') {
            quote = Some(c);
        } else if quote.is_none() && c.is_whitespace() {
            if let Some(begin) = start.take() {
                spans.push(&text[begin..i]);
            }
        }
    }
    if let Some(begin) = start {
        spans.push(&text[begin..]);
    }
    spans
}

fn value(raw: &str) -> String {
    super::inject::split_extra_args(Some(raw))
        .into_iter()
        .next()
        .unwrap_or_default()
}

#[derive(Serialize)]
pub struct Selection {
    pub model: String,
    pub effort: String,
}

/// Read inherited choices for display using the same flags accepted by the launch path.
pub fn selection(kind: SessionKind, args: Option<&str>) -> Selection {
    let spec = catalog().into_iter().find(|s| s.id == kind);
    let words = super::inject::split_extra_args(args);
    let mut result = Selection {
        model: String::new(),
        effort: String::new(),
    };
    for (i, word) in words.iter().enumerate() {
        if kind == SessionKind::Codex {
            if let Some(effort) = word.strip_prefix("--config=model_reasoning_effort=") {
                result.effort = effort.trim_matches('"').into();
            }
        }
        if word == "--model" || word == "-m" {
            result.model = words.get(i + 1).cloned().unwrap_or_default();
        }
        if let Some(model) = word.strip_prefix("--model=") {
            result.model = model.into();
        }
        if let Some(flag) = spec.as_ref().and_then(|s| s.effort_flag) {
            if word == flag {
                result.effort = words.get(i + 1).cloned().unwrap_or_default();
            }
            if let Some(effort) = word.strip_prefix(&format!("{flag}=")) {
                result.effort = effort.trim_matches('"').into();
            }
        }
    }
    result
}

/// Replace explicitly selected settings. None inherits; Some("") clears the corresponding override.
pub fn apply(
    kind: SessionKind,
    args: Option<&str>,
    model: Option<&str>,
    effort: Option<&str>,
) -> Result<Option<String>, String> {
    let spec = catalog()
        .into_iter()
        .find(|s| s.id == kind)
        .ok_or("This session type cannot run an agent task")?;
    if !spec.accepts_task {
        if [model, effort].into_iter().flatten().any(|s| !s.is_empty()) {
            return Err("A terminal has no model or reasoning effort setting".into());
        }
        return Ok(args
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string));
    }
    for choice in [model, effort].into_iter().flatten() {
        if !choice
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.:/@+[]=,".contains(c))
        {
            return Err(
                "Model and effort must be identifiers without spaces or shell operators".into(),
            );
        }
    }
    let raw = tokens(args.unwrap_or(""));
    let mut kept = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let word = value(raw[i]);
        if model.is_some() && (word == "--model" || word == "-m") {
            i += 2;
            continue;
        }
        if model.is_some() && word.starts_with("--model=") {
            i += 1;
            continue;
        }
        if let Some(flag) = spec.effort_flag.filter(|_| effort.is_some()) {
            if word == flag {
                i += 2;
                continue;
            }
            if word.starts_with(&format!("{flag}=")) {
                i += 1;
                continue;
            }
            if kind == SessionKind::Codex && word.starts_with("--config=model_reasoning_effort=") {
                i += 1;
                continue;
            }
            if kind == SessionKind::Codex
                && (word == "-c" || word == "--config")
                && raw
                    .get(i + 1)
                    .is_some_and(|v| value(v).starts_with("model_reasoning_effort="))
            {
                i += 2;
                continue;
            }
        }
        kept.push(raw[i].to_string());
        i += 1;
    }
    if let Some(model) = model.filter(|s| !s.is_empty()) {
        kept.push(format!("--model '{model}'"));
    }
    if let Some(effort) = effort.filter(|s| !s.is_empty()) {
        let flag = spec
            .effort_flag
            .ok_or("This agent does not expose a reasoning effort option")?;
        kept.push(if kind == SessionKind::Codex {
            format!("-c model_reasoning_effort={effort}")
        } else {
            format!("{flag} {effort}")
        });
    }
    let result = kept.join(" ");
    Ok((!result.is_empty()).then_some(result))
}

/// Apply a neutral selection through the shared CLI mapping.
pub fn apply_selection(
    args: Option<&str>,
    selection: &AgentSelection,
) -> Result<Option<String>, String> {
    apply(
        selection.agent,
        args,
        Some(selection.model.as_str()),
        Some(selection.effort.as_str()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zcode_spawn_choices_include_native_reasoning_effort() {
        let spec = catalog().into_iter().find(|option| option.id == SessionKind::Zcode).unwrap();
        assert_eq!(spec.effort_flag, Some("--effort"));
        let args = apply(SessionKind::Zcode, None, Some("deepseek/deepseek-flash"), Some("max")).unwrap();
        let chosen = selection(SessionKind::Zcode, args.as_deref());
        assert_eq!(chosen.model, "deepseek/deepseek-flash");
        assert_eq!(chosen.effort, "max");
    }

    #[test]
    fn codex_effort_choices_round_trip_and_default_clears_the_override() {
        for effort in ["low", "medium", "high", "xhigh", "max", "ultra"] {
            let args = apply(SessionKind::Codex, None, Some("gpt-5.6-sol"), Some(effort)).unwrap();
            let chosen = selection(SessionKind::Codex, args.as_deref());
            assert_eq!(chosen.model, "gpt-5.6-sol");
            assert_eq!(chosen.effort, effort);
            let cleared = apply(SessionKind::Codex, args.as_deref(), None, Some("")).unwrap();
            assert_eq!(cleared.as_deref(), Some("--model 'gpt-5.6-sol'"));
        }
    }

    #[test]
    fn neutral_agent_selection_uses_the_shared_cli_mapping() {
        let selection = AgentSelection {
            agent: SessionKind::Opencode,
            model: "provider/model".to_string(),
            effort: "high".to_string(),
        };
        assert_eq!(
            apply_selection(None, &selection).unwrap().as_deref(),
            Some("--model 'provider/model' --variant high")
        );
    }

    #[test]
    fn catalog_distinguishes_agents_from_plain_terminals() {
        let options = catalog();
        let mut workflow: Vec<_> = options.iter().filter(|option| option.supports_plan_execute).map(|option| option.id.as_str()).collect();
        workflow.sort_unstable();
        assert_eq!(workflow, ["claude", "codex", "omp", "opencode", "pi", "zcode"]);
        assert_eq!(options.iter().filter(|s| s.accepts_task).count(), 15);
        assert!(
            !options
                .iter()
                .find(|s| s.id == SessionKind::Terminal)
                .unwrap()
                .accepts_task
        );
        assert!(!options.iter().any(|s| s.id == SessionKind::Browser));
        assert!(
            options
                .iter()
                .find(|s| s.id == SessionKind::Opencode)
                .unwrap()
                .supports_refer_summary
        );
        assert!(
            !options
                .iter()
                .find(|s| s.id == SessionKind::Cline)
                .unwrap()
                .supports_refer_summary
        );
        assert_eq!(
            options
                .iter()
                .find(|s| s.id == SessionKind::Cline)
                .unwrap()
                .effort_flag,
            Some("--thinking")
        );
        assert!(options
            .iter()
            .find(|s| s.id == SessionKind::Omp)
            .unwrap()
            .effort_levels
            .contains(&"xhigh"));
    }

    #[test]
    fn overrides_replace_short_and_long_flags_without_changing_unrelated_arguments() {
        let args = "--add-dir '/My Notes' -m old --model=older --effort low --verbose";
        let applied = apply(
            SessionKind::Claude,
            Some(args),
            Some("opus[1m]"),
            Some("high"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            applied,
            "--add-dir '/My Notes' --verbose --model 'opus[1m]' --effort high"
        );
        let actual = selection(SessionKind::Claude, Some(&applied));
        assert_eq!(actual.model, "opus[1m]");
        assert_eq!(actual.effort, "high");
    }

    #[test]
    fn clearing_one_setting_preserves_the_other_and_codex_config() {
        let inline = "--config=model_reasoning_effort=high --model=gpt-5.5";
        assert_eq!(selection(SessionKind::Codex, Some(inline)).effort, "high");
        assert_eq!(
            apply(SessionKind::Codex, Some(inline), None, Some(""))
                .unwrap()
                .as_deref(),
            Some("--model=gpt-5.5")
        );
        let args =
            "--model 'gpt-5.5' -c 'model_reasoning_effort=low' -c sandbox_mode=workspace-write";
        assert_eq!(selection(SessionKind::Codex, Some(args)).effort, "low");
        let actual = apply(SessionKind::Codex, Some(args), Some(""), Some("xhigh"))
            .unwrap()
            .unwrap();
        assert_eq!(
            actual,
            "-c sandbox_mode=workspace-write -c model_reasoning_effort=xhigh"
        );
        assert_eq!(
            apply(
                SessionKind::Claude,
                Some("--model opus --effort high"),
                None,
                Some("")
            )
            .unwrap()
            .as_deref(),
            Some("--model opus")
        );
    }

    #[test]
    fn cli_specific_effort_and_identifiers_are_validated_before_launch() {
        assert_eq!(
            apply(
                SessionKind::Cline,
                None,
                Some("provider/model"),
                Some("high")
            )
            .unwrap()
            .as_deref(),
            Some("--model 'provider/model' --thinking high")
        );
        assert_eq!(
            apply(SessionKind::Pi, None, None, Some("high"))
                .unwrap()
                .as_deref(),
            Some("--thinking high")
        );
        assert!(apply(SessionKind::Cursor, None, None, Some("high")).is_err());
        assert!(apply(
            SessionKind::Claude,
            None,
            Some("$(touch /tmp/unwanted)"),
            None
        )
        .is_err());
        assert!(apply(SessionKind::Terminal, None, None, None)
            .unwrap()
            .is_none());
        assert!(apply(SessionKind::Terminal, None, Some("model"), None).is_err());
        assert!(apply(SessionKind::Claude, None, None, None)
            .unwrap()
            .is_none());
    }
}
