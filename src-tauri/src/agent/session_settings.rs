//! Model and effort belong to a conversation, independently of its current interface.

use std::io::BufRead;

use crate::{
    db::repo,
    host::AppCtx,
    models::{Session, SessionKind},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Selection {
    pub model: Option<String>,
    pub effort: Option<String>,
}

pub fn clean(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

/// Missing fields are unknown; JSON null is an explicit return to the native default.
fn apply_native(selection: &mut Selection, native: &Value) {
    if let Some(model) = native.get("model") {
        selection.model = clean(model.as_str());
    }
    if let Some(effort) = native.get("effort") {
        selection.effort = clean(effort.as_str());
    }
}

pub fn stored(conn: &Connection, id: &str) -> Result<Option<(Selection, Option<String>)>, String> {
    conn.query_row(
        "SELECT model, effort, native_state FROM session_model_settings WHERE session_id = ?1",
        [id],
        |row| {
            Ok((
                Selection {
                    model: row.get(0)?,
                    effort: row.get(1)?,
                },
                row.get(2)?,
            ))
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn save(
    conn: &Connection,
    id: &str,
    selection: &Selection,
    native: &Value,
) -> Result<(), String> {
    conn.execute("INSERT INTO session_model_settings(session_id, model, effort, native_state) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(session_id) DO UPDATE SET model=excluded.model, effort=excluded.effort, native_state=excluded.native_state",
        params![id, selection.model, selection.effort, native.to_string()]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn copy(conn: &Connection, source: &str, target: &str) -> Result<(), String> {
    conn.execute("INSERT INTO session_model_settings(session_id, model, effort, native_state) SELECT ?2, model, effort, native_state FROM session_model_settings WHERE session_id = ?1",
        params![source, target]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn session(ctx: &AppCtx, id: &str) -> Result<Session, String> {
    let session =
        repo::get_session(&ctx.db().conn.lock().unwrap(), id)?.ok_or("Session not found")?;
    if !matches!(
        session.kind,
        SessionKind::Claude | SessionKind::Codex | SessionKind::Opencode
    ) {
        return Err(
            "Model transfer is available only for Claude, Codex, and OpenCode sessions".into(),
        );
    }
    Ok(session)
}

pub fn resolve(ctx: &AppCtx, session: &Session) -> Result<Selection, String> {
    let native = native_state(session)?;
    let conn = ctx.db().conn.lock().unwrap();
    if let Some((mut selection, previous)) = stored(&conn, &session.id)? {
        // A native record sampled when a chat choice was saved must not undo that newer choice.
        if previous.as_deref() != Some(native.to_string().as_str()) {
            apply_native(&mut selection, &native);
        }
        return Ok(selection);
    }
    let mut selection = from_args(session.kind, session.agent_args.as_deref());
    apply_native(&mut selection, &native);
    // Existing conversations retain native defaults rather than borrowing another conversation's pair.
    if session.agent_session_id.is_none() && selection == Selection::default() {
        let raw: Option<String> = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key='vlx-settings'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(settings) = raw.and_then(|s| serde_json::from_str::<Value>(&s).ok()) {
            let kind = session.kind.as_str();
            selection.model = clean(settings["chatModelByKind"][kind].as_str().or_else(|| {
                (session.kind == SessionKind::Claude)
                    .then(|| settings["chatModel"].as_str())
                    .flatten()
            }));
            let model = selection.model.as_deref().unwrap_or("");
            selection.effort = clean(
                settings["chatEffortByModel"][format!("{kind}:{model}")]
                    .as_str()
                    .or_else(|| {
                        (session.kind == SessionKind::Claude)
                            .then(|| settings["chatEffortByModel"][model].as_str())
                            .flatten()
                    }),
            );
        }
    }
    Ok(selection)
}

pub fn persist(ctx: &AppCtx, session: &Session, selection: &Selection) -> Result<(), String> {
    save(
        &ctx.db().conn.lock().unwrap(),
        &session.id,
        selection,
        &native_state(session)?,
    )
}

pub fn keeps_automatic_effort(ctx: &AppCtx, id: &str) -> bool {
    stored(&ctx.db().conn.lock().unwrap(), id)
        .ok()
        .flatten()
        .is_some_and(|(selection, _)| selection.effort.is_none())
}

fn plain_command_output(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for c in chars.by_ref() {
                if ('@'..='~').contains(&c) {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn native_state(session: &Session) -> Result<Value, String> {
    let Some(id) = session.agent_session_id.as_deref() else {
        return Ok(json!({}));
    };
    if session.kind == SessionKind::Codex {
        if let Some(state) = codex_thread_settings(id)? {
            return Ok(state);
        }
    }
    if session.kind == SessionKind::Opencode {
        let Some(model) = super::opencode_store::session_model(id)? else {
            return Ok(json!({}));
        };
        let Some(provider) = model["providerID"].as_str() else {
            return Ok(json!({}));
        };
        let Some(name) = model["id"].as_str().or_else(|| model["modelID"].as_str()) else {
            return Ok(json!({}));
        };
        let effort = model["variant"].as_str().filter(|v| *v != "default");
        return Ok(json!({"model":format!("{provider}/{name}"),"effort":effort}));
    }
    let Some(path) = super::transcript::source_path(session.kind, id) else {
        return Ok(json!({}));
    };
    let file =
        std::fs::File::open(path).map_err(|e| format!("Failed to read session settings: {e}"))?;
    parse_recording_lines(session.kind, std::io::BufReader::new(file).lines())
}

/// Recent Codex releases also keep the thread's settings in their local state database.
fn codex_thread_settings(id: &str) -> Result<Option<Value>, String> {
    let Some(home) = super::resume::codex_home() else {
        return Ok(None);
    };
    let Ok(entries) = std::fs::read_dir(home) else {
        return Ok(None);
    };
    let latest = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let version = name
                .strip_prefix("state_")?
                .strip_suffix(".sqlite")?
                .parse::<u32>()
                .ok()?;
            Some((version, entry.path()))
        })
        .max_by_key(|(version, _)| *version);
    let Some((_, path)) = latest else {
        return Ok(None);
    };
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| format!("Failed to read Codex thread settings: {e}"))?;
    conn.busy_timeout(std::time::Duration::from_secs(2))
        .map_err(|e| e.to_string())?;
    // Older releases have no model columns; their rollout remains the supported fallback.
    let Ok(mut query) = conn.prepare("SELECT model, reasoning_effort FROM threads WHERE id = ?1")
    else {
        return Ok(None);
    };
    let pair: Option<(Option<String>, Option<String>)> = query
        .query_row([id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(pair.and_then(|(model, effort)| model.map(|model| json!({"model":model,"effort":effort}))))
}

fn parse_recording_lines(
    kind: SessionKind,
    lines: impl IntoIterator<Item = std::io::Result<String>>,
) -> Result<Value, String> {
    let mut state = json!({});
    for line in lines {
        let line = line.map_err(|e| format!("Failed to read native session settings: {e}"))?;
        let Ok(row) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if row["isSidechain"] == true || row.get("parent_tool_use_id").is_some_and(|v| !v.is_null())
        {
            continue;
        }
        if kind == SessionKind::Codex && row["type"] == "turn_context" {
            let p = &row["payload"];
            if p.get("model").is_some() {
                state["model"] = p["model"].clone();
            }
            if let Some(effort) = p.get("effort").or_else(|| p.get("reasoning_effort")) {
                state["effort"] = effort.clone();
            }
        }
        if kind != SessionKind::Claude {
            continue;
        }
        if row["type"] == "assistant" {
            if let Some(model) = row
                .pointer("/message/model")
                .and_then(Value::as_str)
                .filter(|m| *m != "<synthetic>")
            {
                state["model"] = json!(model);
            }
        }
        if row.pointer("/attachment/type").and_then(Value::as_str) == Some("model") {
            if let Some(model) = row
                .pointer("/attachment/identity/modelId")
                .and_then(Value::as_str)
            {
                state["model"] = json!(model);
            }
        }
        if let Some(effort) = row.pointer("/thinkingMetadata/effortLevel") {
            state["effort"] = effort.clone();
        }
        // Local command output is authored by the CLI, unlike ordinary assistant/user prose.
        let content = row.pointer("/message/content");
        let content = content
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                content.and_then(Value::as_array).map(|parts| {
                    parts
                        .iter()
                        .filter_map(|p| p["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
            });
        if let Some(content) = content {
            if let Some(output) = content
                .split("<local-command-stdout>")
                .nth(1)
                .and_then(|s| s.split("</local-command-stdout>").next())
            {
                let output = plain_command_output(output);
                let output = output.trim();
                if let Some(level) = output
                    .strip_prefix("Set effort level to ")
                    .and_then(|s| s.split(|c: char| !c.is_ascii_alphanumeric()).next())
                {
                    state["effort"] = json!(level.to_lowercase());
                } else if output.starts_with("Effort level set to auto") {
                    state["effort"] = Value::Null;
                }
                if let Some(model) = output.strip_prefix("Set model to ") {
                    // Full model IDs survive display formatting; aliases are used by older CLI versions.
                    if let Some(id) = model
                        .split(|c: char| {
                            !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '[' | ']'))
                        })
                        .find(|v| {
                            v.starts_with("claude-")
                                || matches!(*v, "opus" | "sonnet" | "haiku" | "fable" | "opusplan")
                        })
                    {
                        state["model"] = json!(id);
                    }
                }
            }
        }
    }
    Ok(state)
}

#[cfg(test)]
fn parse_recording(kind: SessionKind, text: &str) -> Value {
    parse_recording_lines(kind, text.lines().map(|line| Ok(line.to_string()))).unwrap()
}

/// Keep untouched argument fragments intact, including their shell quoting and environment references.
fn tokens(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
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
        } else if quote.is_none() && matches!(c, '\'' | '"') {
            quote = Some(c);
        } else if quote.is_none() && c.is_whitespace() {
            if let Some(s) = start.take() {
                out.push(&text[s..i]);
            }
        }
    }
    if let Some(s) = start {
        out.push(&text[s..]);
    }
    out
}

fn config_choice(value: &str) -> Option<(&str, &str)> {
    let (key, value) = value.split_once('=')?;
    matches!(key.trim(), "model" | "model_reasoning_effort").then_some((key.trim(), value.trim()))
}

pub fn from_args(kind: SessionKind, text: Option<&str>) -> Selection {
    let args = super::inject::split_extra_args(text);
    let mut state = Selection::default();
    let mut i = 0;
    while i < args.len() {
        let (flag, inline) = args[i]
            .split_once('=')
            .map_or((args[i].as_str(), None), |(f, v)| (f, Some(v)));
        let value = inline.or_else(|| args.get(i + 1).map(String::as_str));
        let known = match flag {
            "--model" | "-m" => {
                state.model = clean(value);
                true
            }
            "--effort" if kind == SessionKind::Claude => {
                state.effort = clean(value).filter(|v| v != "auto");
                true
            }
            "--variant" if kind == SessionKind::Opencode => {
                state.effort = clean(value).filter(|v| v != "default");
                true
            }
            "-c" | "--config" if kind == SessionKind::Codex => {
                if let Some((key, value)) = value.and_then(config_choice) {
                    let value = serde_json::from_str::<String>(value)
                        .unwrap_or_else(|_| value.trim_matches('\'').into());
                    if key == "model" {
                        state.model = clean(Some(&value));
                    } else {
                        state.effort = clean(Some(&value));
                    }
                }
                true
            }
            _ => false,
        };
        i += if known && inline.is_none() { 2 } else { 1 };
    }
    state
}

pub fn without_selection_args(kind: SessionKind, text: Option<&str>) -> String {
    let raw = tokens(text.unwrap_or(""));
    let mut out = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let parsed = super::inject::split_extra_args(Some(raw[i]));
        let arg = parsed.first().map(String::as_str).unwrap_or("");
        let (flag, inline) = arg
            .split_once('=')
            .map_or((arg, None), |(f, v)| (f, Some(v)));
        let next = raw
            .get(i + 1)
            .map(|s| super::inject::split_extra_args(Some(s)));
        let value = inline.or_else(|| next.as_ref().and_then(|v| v.first()).map(String::as_str));
        let remove = matches!(flag, "--model" | "-m")
            || (kind == SessionKind::Claude && flag == "--effort")
            || (kind == SessionKind::Opencode && flag == "--variant")
            || (kind == SessionKind::Codex
                && matches!(flag, "-c" | "--config")
                && value.and_then(config_choice).is_some());
        if remove {
            i += if inline.is_none() { 2 } else { 1 };
        } else {
            out.push(raw[i]);
            i += 1;
        }
    }
    out.join(" ")
}

/// Values enter the shell through environment variables, never executable command text.
pub fn terminal_args(
    kind: SessionKind,
    shell: super::inject::ShellKind,
    text: Option<&str>,
    choice: &Selection,
) -> (String, Vec<(String, String)>) {
    let mut args = without_selection_args(kind, text);
    let mut env = Vec::new();
    let mut add = |flag: &str, key: &str, value: String| {
        args.push_str(&format!(" {flag} {}", super::inject::value_ref(shell, key)));
        env.push((key.into(), value));
    };
    if let Some(model) = &choice.model {
        add("--model", "VLX_SESSION_MODEL", model.clone());
    }
    match kind {
        SessionKind::Claude => {
            // The launcher merges "off" into its existing settings JSON so hooks remain installed.
            if let Some(effort) = choice.effort.as_ref().filter(|v| *v != "off") {
                add("--effort", "VLX_SESSION_EFFORT", effort.clone());
            }
        }
        SessionKind::Codex => {
            if let Some(effort) = &choice.effort {
                add(
                    "-c",
                    "VLX_SESSION_EFFORT",
                    format!("model_reasoning_effort={}", json!(effort)),
                );
            }
        }
        _ => {} // OpenCode stores the variant through its native session/model interface before handoff.
    }
    (args.trim().into(), env)
}

#[cfg(test)]
mod tests {
    use super::super::inject::ShellKind;
    use super::*;

    fn pair(model: &str, effort: &str) -> Selection {
        Selection {
            model: clean(Some(model)),
            effort: clean(Some(effort)),
        }
    }

    #[test]
    fn parses_each_engines_launch_selection() {
        assert_eq!(
            from_args(
                SessionKind::Claude,
                Some("--add-dir 'a b' --model opus --effort high")
            ),
            pair("opus", "high")
        );
        assert_eq!(
            from_args(
                SessionKind::Codex,
                Some(
                    "--model=gpt-5.5 -c 'model_reasoning_effort=\"low\"' -c sandbox_mode=read-only"
                )
            ),
            pair("gpt-5.5", "low")
        );
        assert_eq!(
            from_args(
                SessionKind::Codex,
                Some("--config=model=\"gpt-5.5\" --config=model_reasoning_effort='high'")
            ),
            pair("gpt-5.5", "high")
        );
        assert_eq!(
            from_args(
                SessionKind::Opencode,
                Some("-m openai/gpt-5.5 --variant high")
            ),
            pair("openai/gpt-5.5", "high")
        );
    }

    #[test]
    fn removes_only_overridden_flags_and_preserves_shell_fragments() {
        assert_eq!(without_selection_args(SessionKind::Codex, Some("--model=old -c 'model_reasoning_effort=\"high\"' --config 'sandbox_mode=\"read-only\"' --enable foo --add-dir \"$PROJECT/a b\"")),
            "--config 'sandbox_mode=\"read-only\"' --enable foo --add-dir \"$PROJECT/a b\"");
        assert_eq!(without_selection_args(SessionKind::Claude, Some("--model old --effort=high --add-dir 'a b' --settings '{\"env\":{\"A\":\"B\"}}'")),
            "--add-dir 'a b' --settings '{\"env\":{\"A\":\"B\"}}'");
    }

    #[test]
    fn terminal_launch_uses_environment_values_and_native_effort_spelling() {
        for kind in [
            SessionKind::Claude,
            SessionKind::Codex,
            SessionKind::Opencode,
        ] {
            for shell in [
                ShellKind::Posix,
                ShellKind::Fish,
                ShellKind::PowerShell,
                ShellKind::Pwsh,
                ShellKind::Cmd,
            ] {
                let selection = pair("custom/$(touch should-not-run)", "high");
                let (args, env) =
                    terminal_args(kind, shell, Some("--model old --verbose"), &selection);
                assert!(!args.contains("touch"));
                assert!(!args.contains(" old"));
                assert!(args.contains("--verbose"));
                assert_eq!(env[0].1, selection.model.clone().unwrap());
                match kind {
                    SessionKind::Claude => assert!(args.contains("--effort")),
                    SessionKind::Codex => assert_eq!(env[1].1, "model_reasoning_effort=\"high\""),
                    SessionKind::Opencode => {
                        assert_eq!(env.len(), 1);
                        assert!(!args.contains("--variant"));
                    }
                    _ => unreachable!(),
                }
            }
        }
        let (args, env) = terminal_args(
            SessionKind::Codex,
            ShellKind::Posix,
            Some("-c model_reasoning_effort=high"),
            &Selection::default(),
        );
        assert!(args.is_empty());
        assert!(env.is_empty());
        let (args, _) = terminal_args(
            SessionKind::Claude,
            ShellKind::Posix,
            None,
            &pair("opus", "off"),
        );
        assert!(!args.contains("--effort"));
        assert!(
            !args.contains("--settings"),
            "thinking is merged into the existing hook settings"
        );
    }

    #[test]
    fn native_recordings_restore_latest_root_settings_and_auto() {
        let state = parse_recording(
            SessionKind::Codex,
            &[
                json!({"type":"turn_context","payload":{"model":"old","effort":"high"}}),
                json!({"type":"turn_context","payload":{"model":"gpt-5.6-luna","effort":"low"}}),
            ]
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
        );
        assert_eq!(state, json!({"model":"gpt-5.6-luna","effort":"low"}));
        let state = parse_recording(SessionKind::Claude, &[
            json!({"type":"assistant","message":{"model":"claude-sonnet-4-6"}}),
            json!({"type":"assistant","isSidechain":true,"message":{"model":"claude-haiku-4-5"}}),
            json!({"type":"user","message":{"content":"<local-command-stdout>Set model to claude-opus-4-8</local-command-stdout>"}}),
            json!({"type":"user","message":{"content":"<local-command-stdout>Set effort level to high: More reasoning</local-command-stdout>"}}),
            json!({"type":"assistant","message":{"model":"<synthetic>"}}),
        ].iter().map(Value::to_string).collect::<Vec<_>>().join("\n"));
        assert_eq!(state, json!({"model":"claude-opus-4-8","effort":"high"}));
        let auto = parse_recording(SessionKind::Claude, &json!({"type":"user","message":{"content":"<local-command-stdout>Effort level set to auto</local-command-stdout>"}}).to_string());
        assert_eq!(auto, json!({"effort":null}));
        assert_eq!(
            parse_recording(
                SessionKind::Claude,
                &json!({"type":"user","message":{"content":"Set effort level to high"}})
                    .to_string()
            ),
            json!({})
        );
    }

    #[test]
    fn backend_choices_survive_idle_views_auto_clear_fork_and_reopen() {
        let dir = std::env::temp_dir().join(format!("vlx-selection-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let ctx = AppCtx::Headless(std::sync::Arc::new(crate::host::HeadlessHost::new(
            dir.clone(),
            db,
        )));
        for kind in [
            SessionKind::Claude,
            SessionKind::Codex,
            SessionKind::Opencode,
        ] {
            let source = {
                let conn = ctx.db().conn.lock().unwrap();
                let project = repo::import_project(&conn, dir.to_str().unwrap()).unwrap();
                let source = repo::create_session(
                    &conn,
                    &project.id,
                    None,
                    kind.as_str(),
                    kind,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap();
                repo::set_session_engine(&conn, &source.id, "chat").unwrap();
                repo::get_session(&conn, &source.id).unwrap().unwrap()
            };
            crate::command_core::chat_set_model(&ctx, &source.id, Some("chosen-model")).unwrap();
            crate::command_core::chat_set_effort(&ctx, &source.id, Some("high")).unwrap();
            assert_eq!(
                crate::command_core::chat_snapshot(&ctx, &source.id)
                    .unwrap()
                    .selection,
                Some(pair("chosen-model", "high"))
            );
            crate::command_core::chat_set_effort(&ctx, &source.id, None).unwrap();
            assert_eq!(resolve(&ctx, &source).unwrap(), pair("chosen-model", ""));
            // This transition has no live terminal in the fixture and must retain the exact pair.
            repo::set_session_engine(&ctx.db().conn.lock().unwrap(), &source.id, "tui").unwrap();
            crate::command_core::set_session_engine(&ctx, &source.id, "chat").unwrap();
            assert_eq!(
                crate::command_core::chat_snapshot(&ctx, &source.id)
                    .unwrap()
                    .selection,
                Some(pair("chosen-model", ""))
            );
            let conn = ctx.db().conn.lock().unwrap();
            let fresh = repo::create_fresh_chat_session(&conn, &source, "fresh").unwrap();
            assert_eq!(
                stored(&conn, &fresh.id).unwrap().unwrap().0,
                pair("chosen-model", "")
            );
            repo::set_agent_session_id(&conn, &source.id, "native-id", kind).unwrap();
            if kind != SessionKind::Opencode {
                let fork = repo::fork_session(&conn, &source.id).unwrap();
                assert_eq!(
                    stored(&conn, &fork.id).unwrap().unwrap().0,
                    pair("chosen-model", "")
                );
            }
            let reopened = crate::db::Db::open(&dir.join("test.db")).unwrap();
            assert_eq!(
                stored(&reopened.conn.lock().unwrap(), &fresh.id)
                    .unwrap()
                    .unwrap()
                    .0,
                pair("chosen-model", "")
            );
        }
        drop(ctx);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;

    /// Run with dedicated XDG directories and VLX_HANDOFF_TEST_ROOT; never uses a personal native store.
    #[test]
    #[ignore = "requires an installed OpenCode and isolated native data directories"]
    fn native_opencode_handoff_keeps_model_and_variant_without_a_prompt() {
        let root = std::path::PathBuf::from(
            std::env::var("VLX_HANDOFF_TEST_ROOT").expect("isolated test root required"),
        );
        assert!(std::env::var("XDG_DATA_HOME")
            .unwrap()
            .starts_with(root.to_str().unwrap()));
        let dir = root.join("backend-handoff");
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::Db::open(&dir.join("test.db")).unwrap();
        let host = std::sync::Arc::new(crate::host::HeadlessHost::new(dir.clone(), db));
        // OpenCode's status plugin is not installed in this fixture; no callback listener is needed.
        host.set_hooks(crate::agent::server::HookServer {
            port: 19191,
            token: "unused-test-token".into(),
        });
        let ctx = AppCtx::Headless(host);
        let source = {
            let conn = ctx.db().conn.lock().unwrap();
            let project = repo::import_project(&conn, dir.to_str().unwrap()).unwrap();
            let source = repo::create_session(
                &conn,
                &project.id,
                None,
                "native handoff",
                SessionKind::Opencode,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            repo::set_session_engine(&conn, &source.id, "chat").unwrap();
            let port: Value =
                serde_json::from_str(&std::fs::read_to_string(root.join("ports.json")).unwrap())
                    .unwrap();
            repo::set_app_settings(
                &conn,
                &std::collections::HashMap::from([(
                    format!("opencode.port.{}", source.id),
                    port["opencode"].to_string(),
                )]),
            )
            .unwrap();
            source
        };
        let result = (|| -> Result<(), String> {
            crate::command_core::chat_set_model(&ctx, &source.id, Some("openai/gpt-5.5"))?;
            crate::command_core::chat_set_effort(&ctx, &source.id, Some("high"))?;
            crate::command_core::set_session_engine(&ctx, &source.id, "tui")?;
            let session = session(&ctx, &source.id)?;
            let expected = json!({"model":"openai/gpt-5.5","effort":"high"});
            if native_state(&session)? != expected {
                return Err("native model or variant was lost".into());
            }
            crate::command_core::set_session_engine(&ctx, &source.id, "chat")?;
            let snapshot = crate::command_core::chat_snapshot(&ctx, &source.id)?;
            if snapshot.effort.as_deref() != Some("high") {
                return Err("chat did not restore high effort".into());
            }
            crate::command_core::chat_set_effort(&ctx, &source.id, None)?;
            crate::command_core::set_session_engine(&ctx, &source.id, "tui")?;
            if native_state(&session)?["effort"] != Value::Null {
                return Err("native automatic effort did not clear the old variant".into());
            }
            crate::command_core::set_session_engine(&ctx, &source.id, "chat")?;
            if crate::command_core::chat_snapshot(&ctx, &source.id)?
                .effort
                .is_some()
            {
                return Err("chat restored a stale variant".into());
            }
            if !super::super::opencode_store::messages(
                session.agent_session_id.as_deref().unwrap(),
            )?
            .is_empty()
            {
                return Err("settings transfer inserted a message".into());
            }
            Ok(())
        })();
        let _ = ctx.chat().stop(&ctx, &source.id);
        result.unwrap();
    }
}
