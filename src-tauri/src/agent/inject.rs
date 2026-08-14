//! Agent launch injection for Claude, Codex, and other agent sessions.
//! 1. Generates environment variables for the shell process, including in-memory JSON with the
//!    port, session ID, and token already embedded.
//! 2. Generates the launch command to write to the PTY.
//!
//! All injection parameters are embedded in the URL (`/hook/<sid>?t=<token>&e=<event>`), allowing
//! the server to identify the session and map its state without correlating environment variables
//! or reading the hook body.

use crate::agent::server::HookEndpoint;
use crate::models::SessionKind;

/// Holds the Claude settings JSON so the command line can reference it without echoing a large JSON value.
pub const CLAUDE_SETTINGS_ENV: &str = "VLX_CLAUDE_SETTINGS";
/// Holds the Codex notify array referenced by `-c notify=`.
pub const CODEX_NOTIFY_ENV: &str = "VLX_CODEX_NOTIFY";
/// Holds the Codex lifecycle-hooks table referenced by `-c hooks=`.
/// The value is static; hook subprocesses read the dynamic session ID, port, and token from the shared `VLX_*` variables.
pub const CODEX_HOOKS_ENV: &str = "VLX_CODEX_HOOKS";
/// Holds the callback URL read by the Codex notify program (`--notify-env`).
/// This lets a static `config.toml` work with a dynamic port and token without persisting the URL.
pub const CODEX_NOTIFY_URL_ENV: &str = "VLX_CODEX_NOTIFY_URL";
/// Stores OpenCode's inline configuration, including the local plugin's absolute path, in its native
/// `OPENCODE_CONFIG_CONTENT` environment variable. OpenCode loads the generated state-bridge plugin
/// (`resources/vlx-opencode-notify.js`), which reads the injected `VLX_SESSION_ID`, `VLX_TOKEN`, and
/// `VLX_SPAWN_URL` values and reports events to the hook service. Using the native environment variable
/// avoids modifying `~/.config/opencode` or writing temporary configuration to disk.
pub const OPENCODE_CONFIG_ENV: &str = "OPENCODE_CONFIG_CONTENT";
/// Holds the absolute path to Pi's state-bridge extension, loaded through `pi -e "$VLX_PI_EXT"`.
/// An environment variable plus a quoted reference safely handles spaces in macOS data directories
/// such as `~/Library/Application Support/…`. `pty/manager.rs` derives the path from the data directory
/// and adds it to the injected environment (see `agent/pi.rs`); the extension reads the injected `VLX_*`
/// values to report events.
pub const PI_EXT_ENV: &str = "VLX_PI_EXT";
/// Holds the initial task prompt for a spawned child session. The launch command references it as a
/// positional prompt (`claude "$VLX_INIT_PROMPT"` / `codex "$VLX_INIT_PROMPT"`), so the new session
/// starts with its task and does not depend on timing a later UI write.
///
/// Keeping the prompt in an environment variable is injection-safe: spaces, quotes, newlines, `$`, and
/// backticks cannot disrupt parsing or be evaluated a second time. Shells do not rescan variable-expansion
/// results for substitutions, so text such as `$(...)` and backticks remains literal.
pub const INIT_PROMPT_ENV: &str = "VLX_INIT_PROMPT";
/// Holds the URL used to report a missing agent. If the launch guard cannot find the agent on `PATH`, it
/// sends an HTTP GET to this URL, which already contains `e=notfound`. The backend then marks the exact
/// session as not installed and asks the frontend to show the installation card. This authoritative path
/// does not depend on terminal-screen parsing and works consistently for every agent.
pub const NOTFOUND_URL_ENV: &str = "VLX_NOTFOUND_URL";

/// Values injected when launching a typed session.
pub struct AgentSpawn {
    /// Environment variables to set on the shell process as key-value pairs.
    pub env: Vec<(String, String)>,
    /// Command to write to the PTY after startup. `None` means the session type does not auto-launch an agent.
    pub launch: Option<String>,
}

/// Shell family, which determines launch-command syntax.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShellKind {
    /// POSIX shells such as bash, zsh, sh, and dash.
    Posix,
    Fish,
    PowerShell,
    Cmd,
}

/// Infers the shell family from its executable path.
pub fn shell_kind(path: &str) -> ShellKind {
    let base = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_ascii_lowercase();
    // Ignore the `.exe` suffix on Windows.
    let base = base.strip_suffix(".exe").unwrap_or(&base);
    if base.contains("powershell") || base.contains("pwsh") {
        ShellKind::PowerShell
    } else if base.contains("fish") {
        ShellKind::Fish
    } else if base == "cmd" || base == "command" {
        ShellKind::Cmd
    } else {
        // Treat bash, zsh, sh, dash, and all other shells as POSIX-compatible.
        ShellKind::Posix
    }
}

/// Builds a hook URL with all parameters embedded: `http://127.0.0.1:<port>/hook/<sid>?t=<token>&e=<event>`.
fn hook_url(ep: &HookEndpoint, sid: &str, event: &str) -> String {
    hook_url_at(ep.port, &ep.token, sid, event)
}

/// Like `hook_url`, but accepts the port and token explicitly. Remote sessions use the tunnel port instead
/// of the local hook port: remote Claude posts to `remote:127.0.0.1:<tunnel-port>`, and `ssh -R` forwards it
/// to the local hook service.
fn hook_url_at(port: u16, token: &str, sid: &str, event: &str) -> String {
    format!("http://127.0.0.1:{port}/hook/{sid}?t={token}&e={event}")
}

/// Builds Claude's inline `--settings` JSON with HTTP-hook URLs containing the port, session ID, and token.
///
/// Event-to-state mapping:
/// - UserPromptSubmit / PreToolUse / PostToolUse → working
/// - Stop → waiting
/// - Notification(permission_prompt|elicitation_dialog) → asking
/// - Notification(idle_prompt) → idle (silently corrects the state to waiting without a notification)
pub fn build_claude_settings(ep: &HookEndpoint, sid: &str) -> String {
    build_claude_settings_at(ep.port, &ep.token, sid)
}

/// Like `build_claude_settings`, but accepts the port and token explicitly for remote tunnel endpoints.
fn build_claude_settings_at(port: u16, token: &str, sid: &str) -> String {
    let working = hook_url_at(port, token, sid, "working");
    let waiting = hook_url_at(port, token, sid, "waiting");
    let asking = hook_url_at(port, token, sid, "asking");
    // Claude's idle_prompt maps to a silent waiting state: correct the state without displaying a
    // "replied" notification, because the agent is idle and awaiting input rather than just finishing.
    let idle = hook_url_at(port, token, sid, "idle");
    serde_json::json!({
        "hooks": {
            // A submitted prompt starts processing.
            "UserPromptSubmit": [{ "hooks": [{ "type": "http", "url": working }] }],
            // Keep the session working around tool calls, including the gap after approval.
            "PreToolUse": [{ "matcher": "*", "hooks": [{ "type": "http", "url": working }] }],
            "PostToolUse": [{ "matcher": "*", "hooks": [{ "type": "http", "url": working }] }],
            // The response has ended.
            "Stop": [{ "hooks": [{ "type": "http", "url": waiting }] }],
            // Approval/input requests map to asking; idle reminders silently correct the idle state.
            "Notification": [
                { "matcher": "permission_prompt|elicitation_dialog",
                  "hooks": [{ "type": "http", "url": asking }] },
                { "matcher": "idle_prompt",
                  "hooks": [{ "type": "http", "url": idle }] }
            ]
        }
    })
    .to_string()
}

/// Builds the Codex `-c notify` JSON array: `[<exe>, "--notify-env"]`.
///
/// When Codex emits `agent-turn-complete`, it runs `<exe> --notify-env <event-json>`. The hidden
/// subcommand reads `VLX_CODEX_NOTIFY_URL` and posts the event to it with `e=waiting`. Keeping the URL
/// outside the array lets the same static `config.toml` target a dynamic port and token.
pub fn build_codex_notify(exe_path: &str) -> String {
    serde_json::json!([exe_path, "--notify-env"]).to_string()
}

/// Builds an inline configuration table for the official Codex lifecycle hooks.
///
/// Injected through `-c hooks="$VLX_CODEX_HOOKS"` as the highest-priority layer for this VelaTerm
/// session only; the user's `~/.codex/config.toml` and `hooks.json` remain untouched. Each hook invokes
/// the current VelaTerm executable's hidden `--codex-hook <state>` subcommand, which reads raw JSON from
/// stdin and reports to the local hook service using the session's injected `VLX_SPAWN_URL`,
/// `VLX_SESSION_ID`, and `VLX_TOKEN` values.
///
/// Event mapping:
/// - SessionStart → ready (hook-health handshake only; it is not a completed turn)
/// - UserPromptSubmit → working (a turn starts; this state is always accepted)
/// - PreToolUse → tool (mid-turn working; the hook service drops it just after a turn ends)
/// - PermissionRequest → asking
/// - Stop / SessionEnd → waiting
///
/// `PostToolUse` is intentionally omitted. It reports the same working state that PreToolUse already
/// reported, and it fires immediately before `Stop`. Because every hook runs in its own short-lived
/// process, those two callbacks race, and a `PostToolUse` that lands after `Stop` leaves a finished turn
/// stuck on working forever. Dropping the redundant event removes the race outright; `server.rs` still
/// guards the remaining mid-turn event with `CodexTurnGuard`.
///
/// `SubagentStop` is intentionally omitted because a child agent finishing does not mean the root turn
/// has ended; registering it would incorrectly mark an active root session as waiting. Every handler has
/// a five-second timeout, emits no stdout, and exits successfully. Hooks observe state but never make decisions.
pub fn build_codex_hooks() -> String {
    // Values in `-c key=value` are parsed as TOML. JSON arrays happen to be valid TOML, but a JSON
    // object's `:` is not the `=` required by a TOML inline table, so it becomes a plain string and
    // triggers `expected HooksToml`. Generate the inline table explicitly. `serde_json` string escaping
    // is compatible with the TOML basic-string subset used here, including quotes and backslashes.
    fn handler(state: &str) -> String {
        let command = serde_json::to_string(&format!("\"$VLX_EXE\" --codex-hook {state}"))
            .expect("serializing the static hook command should not fail");
        let command_windows = serde_json::to_string(&format!(
            "powershell.exe -NoProfile -NonInteractive -Command \"& $env:VLX_EXE --codex-hook {state}\""
        ))
        .expect("serializing the static Windows hook command should not fail");
        format!(
            "{{ type = \"command\", command = {command}, commandWindows = {command_windows}, timeout = 5 }}"
        )
    }
    let group = |state: &str| format!("[{{ hooks = [{}] }}]", handler(state));
    format!(
        "{{ SessionStart = {}, UserPromptSubmit = {}, PreToolUse = {}, PermissionRequest = {}, Stop = {}, SessionEnd = {} }}",
        group("ready"),
        group("working"),
        group("tool"),
        group("asking"),
        group("waiting"),
        group("waiting")
    )
}

/// Manual Codex configuration snippet for precise state reporting, intended for `~/.codex/config.toml`.
/// VelaTerm only displays this snippet and never writes the user's configuration automatically.
pub fn codex_config_snippet(exe_path: &str) -> String {
    format!("notify = [{:?}, \"--notify-env\"]", exe_path)
}

/// Builds OpenCode's inline configuration JSON: `{"plugin":["<absolute-local-plugin-path>"]}`.
/// OpenCode reads it from `OPENCODE_CONFIG_ENV` and loads the plugin by absolute path. Declaring only
/// the plugin preserves all settings merged from the user's global and project configuration. When
/// `permission_mode` is `Some("skip")`, `"permission":"allow"` bypasses confirmations because OpenCode
/// controls permissions through configuration rather than a command-line flag.
pub fn build_opencode_config_content(plugin_path: &str, permission_mode: Option<&str>) -> String {
    let mut cfg = serde_json::json!({ "plugin": [plugin_path] });
    if permission_mode == Some("skip") {
        cfg["permission"] = serde_json::Value::String("allow".to_string());
    }
    cfg.to_string()
}

/// Returns the shell-specific reference for an injected compact JSON value, passed as one argument.
///
/// PowerShell caveat: when a bare `$env:X` is passed to a native command, Windows argument parsing
/// (`CommandLineToArgvW`) consumes the value's double quotes as delimiters. That corrupts compact JSON
/// and makes Claude report `Invalid JSON provided to --settings`. Escaping every `"` as `\"` keeps the
/// result as one argument; `CommandLineToArgvW` restores the quotes before they reach `claude.exe`.
/// This is reliable only for targets that use `CommandLineToArgvW`, such as a native `claude.exe`.
/// An npm `.cmd` wrapper may still let cmd.exe reinterpret `&` in the hook URL and needs separate handling.
fn value_ref(shell: ShellKind, env: &str) -> String {
    match shell {
        ShellKind::Posix | ShellKind::Fish => format!("\"${env}\""),
        ShellKind::PowerShell => format!("$($env:{env} -replace '\"', '\\\"')"),
        ShellKind::Cmd => format!("\"%{env}%\""),
    }
}

/// References the initial-prompt environment variable as one argument. PowerShell splits a bare `$env:X`
/// on spaces, so the reference must be double-quoted. Unlike `value_ref`'s JSON handling, prompt quotes
/// are not currently escaped; prompts containing `"` may still be split on PowerShell. This known limitation
/// is independent of the `--settings` fix.
fn prompt_ref(shell: ShellKind, env: &str) -> String {
    match shell {
        ShellKind::PowerShell => format!("\"$env:{env}\""),
        _ => value_ref(shell, env),
    }
}

/// For a new session with an initial task prompt, stores the prompt safely in `env` and returns a
/// positional-argument reference, including its leading space, to append to the launch command.
///
/// The prompt is injected only when `resume` is `None`; resumed sessions already received it at first launch.
fn init_prompt_suffix(
    shell: ShellKind,
    resume: Option<&str>,
    init_prompt: Option<&str>,
    env: &mut Vec<(String, String)>,
) -> String {
    if resume.is_some() {
        return String::new();
    }
    match init_prompt.map(str::trim).filter(|s| !s.is_empty()) {
        Some(p) => {
            env.push((INIT_PROMPT_ENV.to_string(), p.to_string()));
            format!(" {}", prompt_ref(shell, INIT_PROMPT_ENV))
        }
        None => String::new(),
    }
}

/// Normalizes custom launch arguments by splitting on all whitespace and rejoining words with one space.
/// This folds multiline input into the single line required by the launch guard; otherwise, later lines
/// would execute as independent commands. An empty result is treated as unset.
///
/// The normalized text remains unquoted and is trusted like `init_cmd`: it is user-authored shell input,
/// so the shell performs final tokenization. Consecutive whitespace, including inside quoted input, is
/// collapsed; that tradeoff is acceptable for launch flags.
fn normalize_extra_args(extra: Option<&str>) -> Option<String> {
    let joined = extra?.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.is_empty() {
        return None;
    }
    Some(joined)
}

/// Builds a command-line fragment for custom launch arguments, including a leading space when nonempty.
///
/// The fragment is intentionally unquoted. These are user-supplied local shell flags, such as
/// `--model opus`, and normal shell tokenization must preserve them as two arguments.
fn extra_args_fragment(extra: Option<&str>) -> String {
    match normalize_extra_args(extra) {
        Some(s) => format!(" {s}"),
        None => String::new(),
    }
}

/// Maps the two-level permission mode to the appropriate command-line flag for each agent.
///
/// `None`, `"default"`, and unknown values request approval incrementally; `"skip"` bypasses all confirmations.
///
/// Most agents return a flag only for `skip`, preserving their native incremental approval by default:
/// - Claude: `--dangerously-skip-permissions`
/// - Codex: `--dangerously-bypass-approvals-and-sandbox` (also disables the sandbox)
/// - Copilot: `--allow-all-tools`
/// - Cursor: `--force`
/// - Antigravity: `--dangerously-skip-permissions`
/// - Grok: `--always-approve`
/// - OpenCode: no CLI flag; permissions are configured in its settings
/// - Pi: no permission-confirmation mechanism by design
/// - Terminal and Browser: not applicable
///
/// **Cline is reversed**: it natively approves everything, unlike the other agents. Both modes therefore
/// receive an explicit flag so the session UI remains consistent and future CLI defaults cannot change its
/// meaning: default uses `--auto-approve false`, while skip uses `--auto-approve true`.
pub fn permission_flag(kind: SessionKind, mode: Option<&str>) -> Option<&'static str> {
    let skip = mode.map(str::trim) == Some("skip");
    match kind {
        // Cline needs an explicit flag in both modes because its native default is full auto-approval.
        SessionKind::Cline => Some(if skip {
            "--auto-approve true"
        } else {
            "--auto-approve false"
        }),
        // Zoo Code defaults to automatic approval. Require approval explicitly in the default mode;
        // skip needs no flag, preserving VelaTerm's consistent permission semantics.
        SessionKind::Zoo => {
            if skip {
                None
            } else {
                Some("--require-approval")
            }
        }
        // Other agents receive a flag only in skip mode and retain native incremental approval by default.
        _ if !skip => None,
        SessionKind::Claude => Some("--dangerously-skip-permissions"),
        SessionKind::Codex => Some("--dangerously-bypass-approvals-and-sandbox"),
        SessionKind::Copilot => Some("--allow-all-tools"),
        SessionKind::Cursor => Some("--force"),
        // Antigravity (`agy`) uses Claude's flag in skip mode.
        SessionKind::Antigravity => Some("--dangerously-skip-permissions"),
        // Crush uses `--yolo` to bypass all tool confirmations.
        SessionKind::Crush => Some("--yolo"),
        // Kimi Code uses `--yolo` to bypass all tool confirmations.
        SessionKind::Kimi => Some("--yolo"),
        // Kiro trusts every tool for the session. Whether its terminal UI still asks for one confirmation
        // up front is unverified on a real machine.
        SessionKind::Kiro => Some("--trust-all-tools"),
        // Grok Build's documented always-approve mode bypasses tool confirmations.
        SessionKind::Grok => Some("--always-approve"),
        // OpenCode has no equivalent flag, Pi has no confirmation mechanism, and Terminal/Browser do not apply.
        SessionKind::Opencode | SessionKind::Pi | SessionKind::Terminal | SessionKind::Browser => {
            None
        }
    }
}

/// Merges the permission flag with custom launch arguments, placing the flag first. Spawn call sites pass
/// the result to `prepare_with_args` as `extra_args`, after built-in flags and before the initial prompt.
/// Returns `None` when both inputs are empty.
pub fn merge_permission_flag(
    kind: SessionKind,
    mode: Option<&str>,
    extra_args: Option<&str>,
) -> Option<String> {
    let flag = permission_flag(kind, mode);
    let user = extra_args.map(str::trim).filter(|s| !s.is_empty());
    match (flag, user) {
        (Some(f), Some(a)) => Some(format!("{f} {a}")),
        (Some(f), None) => Some(f.to_string()),
        (None, Some(a)) => Some(a.to_string()),
        (None, None) => None,
    }
}

/// Model values accepted by each agent's launch flags.
pub fn known_models(kind: SessionKind) -> Option<&'static [&'static str]> {
    match kind {
        SessionKind::Claude => Some(&["fable", "opus", "sonnet", "haiku"]),
        SessionKind::Codex => Some(&[
            "gpt-5.6-sol",
            "gpt-5.6-terra",
            "gpt-5.6-luna",
        ]),
        _ => None,
    }
}

/// Effort values accepted by each agent's launch flags.
pub fn known_efforts(kind: SessionKind) -> Option<&'static [&'static str]> {
    match kind {
        SessionKind::Claude => Some(&["low", "medium", "high"]),
        SessionKind::Codex => Some(&["low", "medium", "high", "xhigh", "max"]),
        _ => None,
    }
}

/// Validates a model/effort value for unquoted command-line insertion: nonempty after trimming,
/// only alphanumerics plus `- _ . :`. Invalid values are dropped rather than quoted so a bad
/// setting can never alter the launch command.
fn valid_launch_value(value: &str) -> Option<&str> {
    let value = value.trim();
    if !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
    {
        Some(value)
    } else {
        None
    }
}

/// Maps model/effort to agent flags: Claude `--model <m> --effort <e>`, Codex
/// `-m <m> -c model_reasoning_effort=<e>`. Other agents get none rather than guessed syntax.
/// Either setting may appear alone; invalid values are dropped (see `valid_launch_value`).
pub fn model_effort_flags(
    kind: SessionKind,
    model: Option<&str>,
    effort: Option<&str>,
) -> Option<String> {
    let model = model.and_then(valid_launch_value);
    let effort = effort.and_then(valid_launch_value);
    let (model_flag, effort_flag) = match kind {
        SessionKind::Claude => (
            model.map(|m| format!("--model {m}")),
            effort.map(|e| format!("--effort {e}")),
        ),
        SessionKind::Codex => (
            model.map(|m| format!("-m {m}")),
            effort.map(|e| format!("-c model_reasoning_effort={e}")),
        ),
        _ => (None, None),
    };
    match (model_flag, effort_flag) {
        (Some(m), Some(e)) => Some(format!("{m} {e}")),
        (Some(m), None) => Some(m),
        (None, Some(e)) => Some(e),
        (None, None) => None,
    }
}

/// Prepends translated model/effort flags to custom arguments so an explicit user flag wins as the
/// later occurrence. Applied before `merge_permission_flag`. Returns `None` when both are empty.
pub fn merge_model_effort_flags(
    kind: SessionKind,
    model: Option<&str>,
    effort: Option<&str>,
    extra_args: Option<&str>,
) -> Option<String> {
    let flags = model_effort_flags(kind, model, effort);
    let user = extra_args.map(str::trim).filter(|s| !s.is_empty());
    match (flags, user) {
        (Some(f), Some(a)) => Some(format!("{f} {a}")),
        (Some(f), None) => Some(f),
        (None, Some(a)) => Some(a.to_string()),
        (None, None) => None,
    }
}

/// Validates a nonempty resume session ID containing only `[0-9a-zA-Z_-]`, which includes UUIDs.
///
/// The value is inserted unquoted into commands such as `claude --resume <id>`, so rejecting spaces,
/// quotes, and shell metacharacters prevents command injection. Invalid IDs disable resume.
fn valid_resume_id(id: &str) -> Option<&str> {
    let id = id.trim();
    if !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Some(id)
    } else {
        None
    }
}

/// User-facing fallback shown when the agent binary is absent from `PATH`; it is not a frontend detection anchor.
///
/// Missing-agent detection uses the authoritative hook: the guard's `else` branch first GETs
/// `$VLX_NOTFOUND_URL` (see `report_not_found`), and the backend marks the session as not installed and
/// displays the installation card. This text is only a readable fallback before the hook is ready or if
/// reporting fails, so detection does not depend on its wording.
///
/// The usual cause is an interactive shell profile that did not load, leaving `PATH` incomplete. Common
/// examples include a Restricted PowerShell execution policy blocking `profile.ps1`, or uninitialized
/// fnm/nvm. The PowerShell message includes the command that enables profile execution.
///
/// Messages deliberately use only ASCII alphanumerics, spaces, and `.,:[]/-`, avoiding quotes and shell
/// metacharacters so they can be embedded safely in quoted strings and cmd.exe blocks.
fn not_found_message(shell: ShellKind, bin: &str) -> String {
    match shell {
        ShellKind::PowerShell => format!(
            "[VelaTerm] {bin} not found on PATH. Your shell profile may be blocked - run: \
             Set-ExecutionPolicy -Scope CurrentUser RemoteSigned, then reopen the session."
        ),
        _ => format!(
            "[VelaTerm] {bin} not found on PATH. Check that it is installed and your shell \
             profile loaded, then re-run."
        ),
    }
}

/// Guard `else` action: report the missing agent through the hook, then print a readable message.
///
/// 1. Report with an HTTP GET to the local hook service via `$VLX_NOTFOUND_URL`, which already contains
///    the session ID, token, and `e=notfound`. This is the authoritative, screen-independent source.
///    Reporting is best-effort: fall back from curl to wget with a two-second timeout; failure only prevents
///    the card from appearing and does not affect the shell.
/// 2. Print `not_found_message` for the user.
///
/// The cmd.exe branch only prints because `%VLX_NOTFOUND_URL%` contains `&`, which cmd.exe would reinterpret
/// after expansion. Windows agent sessions normally use PowerShell, so the rare cmd.exe path avoids brittle escaping.
fn report_not_found(shell: ShellKind, bin: &str) -> String {
    report_not_found_with(shell, &not_found_message(shell, bin))
}

/// General form of `report_not_found` with a caller-provided message. `launch_cmd` uses
/// `not_found_message`, while `launch_cmd_at` uses `path_missing_message`; reporting and printing remain identical.
fn report_not_found_with(shell: ShellKind, msg: &str) -> String {
    match shell {
        ShellKind::Posix => format!(
            "{{ curl -fsS -m 2 \"${NOTFOUND_URL_ENV}\" >/dev/null 2>&1 || \
             wget -qO- -T 2 \"${NOTFOUND_URL_ENV}\" >/dev/null 2>&1; }}; printf '%s\\n' '{msg}'"
        ),
        ShellKind::Fish => format!(
            "curl -fsS -m 2 \"${NOTFOUND_URL_ENV}\" >/dev/null 2>&1; \
             or wget -qO- -T 2 \"${NOTFOUND_URL_ENV}\" >/dev/null 2>&1; echo '{msg}'"
        ),
        ShellKind::PowerShell => format!(
            "try {{ Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 $env:{NOTFOUND_URL_ENV} | Out-Null }} \
             catch {{}}; Write-Host '{msg}'"
        ),
        ShellKind::Cmd => format!("echo {msg}"),
    }
}

/// Shell-specific prefix that clears the screen and scrollback and returns the cursor home.
///
/// The frontend writes the launch command into the PTY, so the shell echoes the entire line. Agents such
/// as Claude and Codex usually scroll inline on the primary screen and retain scrollback instead of taking
/// over the alternate screen. The echoed command would therefore remain permanently above the agent UI;
/// a timed startup overlay cannot remove scrollback content.
///
/// The command's first operation is the only point after shell echo and before the agent draws its first
/// screen. Clear outside the `if/else` guard so both success and failure start on a clean screen. Emit
/// `ESC[3J` for scrollback, `ESC[2J` for the screen, and `ESC[H` to home the cursor. A new session has no
/// useful prior content, so this is safe.
fn clear_prefix(shell: ShellKind) -> &'static str {
    match shell {
        // `printf` interprets \033 inside single quotes as ESC in bash, zsh, dash, and fish.
        ShellKind::Posix | ShellKind::Fish => "printf '\\033[3J\\033[2J\\033[H'; ",
        // Build ESC from `[char]27` for compatibility with Windows PowerShell 5.1 and PowerShell 7+.
        ShellKind::PowerShell => {
            "[Console]::Write([char]27 + '[3J' + [char]27 + '[2J' + [char]27 + '[H'); "
        }
        // cmd.exe's built-in `cls` becomes a clear-screen sequence through ConPTY.
        ShellKind::Cmd => "cls & ",
    }
}

/// Builds `<bin> <args>` and wraps it in a guard that reports a friendly error when the binary is missing.
///
/// The guard runs inside the interactive shell after its profile loads, so it sees the actual `PATH` and
/// aliases. A backend preflight cannot see profile changes and would produce false negatives. If the binary
/// resolves, invoke it directly; otherwise print `not_found_message` instead of the shell's raw error.
///
/// Do not define a same-name wrapper function. Users often alias `claude` or `codex`, and defining
/// `claude(){...}` can conflict with an alias and trigger a zsh parse error. A conditional guard leaves the
/// command in command position, so aliases still apply. The tradeoff is that manually rerunning the agent
/// after it exits does not retain injection; only the initial launch guarantees it.
///
/// Use `if/else` rather than `A && B || C`, which would run C when the real agent exits nonzero and
/// misreport it as missing. The cmd.exe branch avoids the same problem with an explicit `if errorlevel`.
fn launch_cmd(shell: ShellKind, bin: &str, args: &str) -> String {
    let inner = if args.is_empty() {
        bin.to_string()
    } else {
        format!("{bin} {args}")
    };
    // The `else` branch reports through the hook and prints a readable message.
    let report = report_not_found(shell, bin);
    // Clear the screen and scrollback before the guard to remove the shell's echoed command.
    let clear = clear_prefix(shell);
    match shell {
        ShellKind::Posix => format!(
            "{clear}if command -v {bin} >/dev/null 2>&1; then {inner}; else {report}; fi"
        ),
        ShellKind::Fish => format!("{clear}if type -q {bin}; {inner}; else; {report}; end"),
        ShellKind::PowerShell => format!(
            "{clear}if (Get-Command {bin} -ErrorAction SilentlyContinue) {{ {inner} }} else {{ {report} }}"
        ),
        ShellKind::Cmd => format!("{clear}where {bin} >nul 2>nul & if errorlevel 1 ({report}) else ({inner})"),
    }
}

/// Message shown when a configured agent executable path is missing or not executable. Unlike
/// `not_found_message`, it points to Settings because the configured path itself is invalid, not `PATH`.
/// Apart from the path, the text uses a shell-safe ASCII subset; callers escape the path for each shell.
fn path_missing_message(path: &str) -> String {
    format!(
        "[VelaTerm] {path} does not exist or is not executable. \
         Fix the agent executable path in Settings, Agents tab, or clear it to launch from PATH."
    )
}

/// Embeds arbitrary text in POSIX/fish single quotes by replacing `'` with `'\''`.
fn sq_posix(s: &str) -> String {
    s.replace('\'', "'\\''")
}

/// Embeds arbitrary text in PowerShell single quotes by replacing `'` with `''`.
fn sq_pwsh(s: &str) -> String {
    s.replace('\'', "''")
}

/// Escapes bare text used as an `echo` argument inside a cmd.exe parenthesized block. Prefixing block-level
/// metacharacters with `^` prevents paths such as `(x86)` from closing the `if ... else (...)` block early.
fn caret_cmd(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '&' | '|' | '<' | '>' | '(' | ')' | '^') {
            out.push('^');
        }
        out.push(c);
    }
    out
}

/// Like `launch_cmd`, but launches an agent through its configured absolute executable path.
///
/// It differs from `launch_cmd` in two ways:
/// - The guard checks whether the file exists and is executable (`[ -x ]`, `Test-Path`, or `if exist`)
///   rather than resolving it on `PATH`.
/// - The executable path is quoted to support spaces; PowerShell invokes it with the `&` operator.
///
/// Failure still uses `report_not_found_with` so the frontend shows the installation card, but the message
/// is `path_missing_message` and points to Settings.
fn launch_cmd_at(shell: ShellKind, path: &str, args: &str) -> String {
    let clear = clear_prefix(shell);
    match shell {
        ShellKind::Posix => {
            let p = sq_posix(path);
            let inner = if args.is_empty() {
                format!("'{p}'")
            } else {
                format!("'{p}' {args}")
            };
            let report = report_not_found_with(shell, &sq_posix(&path_missing_message(path)));
            format!("{clear}if [ -x '{p}' ]; then {inner}; else {report}; fi")
        }
        ShellKind::Fish => {
            let p = sq_posix(path);
            let inner = if args.is_empty() {
                format!("'{p}'")
            } else {
                format!("'{p}' {args}")
            };
            let report = report_not_found_with(shell, &sq_posix(&path_missing_message(path)));
            format!("{clear}if test -x '{p}'; {inner}; else; {report}; end")
        }
        ShellKind::PowerShell => {
            let p = sq_pwsh(path);
            let inner = if args.is_empty() {
                format!("& '{p}'")
            } else {
                format!("& '{p}' {args}")
            };
            let report = report_not_found_with(shell, &sq_pwsh(&path_missing_message(path)));
            format!(
                "{clear}if (Test-Path -LiteralPath '{p}' -PathType Leaf) {{ {inner} }} else {{ {report} }}"
            )
        }
        ShellKind::Cmd => {
            // Parentheses inside double quotes are safe in cmd.exe blocks; only bare `echo` text needs carets.
            let inner = if args.is_empty() {
                format!("\"{path}\"")
            } else {
                format!("\"{path}\" {args}")
            };
            let report = report_not_found_with(shell, &caret_cmd(&path_missing_message(path)));
            format!("{clear}if exist \"{path}\" ({inner}) else ({report})")
        }
    }
}

/// Unified launch entry point: use `launch_cmd_at` for a configured path, otherwise resolve the command on `PATH`.
fn launch(shell: ShellKind, bin: &str, bin_path: Option<&str>, args: &str) -> String {
    match bin_path {
        Some(p) => launch_cmd_at(shell, p, args),
        None => launch_cmd(shell, bin, args),
    }
}

/// Generates injected values for a session type and shell. Terminal sessions receive no injection.
///
/// If `resume_id` contains a remembered agent session ID, the launch command resumes it in place using
/// Claude's `--resume <id>` or Codex's `resume <id>`. `None` or an invalid ID starts a new session.
///
/// `fork` marks the first launch of a forked conversation. With a valid `resume_id`, it forks instead of
/// resuming in place: Claude uses `--resume <source-id> --fork-session`, while Codex uses `fork <source-id>`.
/// The source remains unchanged. Without a resume ID it starts fresh. Only Claude and Codex support forks.
///
/// `init_prompt` is the initial task for a spawned child session. On a new launch, it is injected through
/// `INIT_PROMPT_ENV` as a positional prompt so the session starts with its task.
///
/// Convenience wrapper without custom agent arguments (`extra_args = None`), equivalent to
/// `prepare_with_args(..., None)`. Production uses `prepare_with_args_capabilities` to pass arguments and
/// capability-probe results; this wrapper retains the original signature for unit tests only.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub fn prepare(
    kind: SessionKind,
    shell_path: &str,
    exe_path: &str,
    ep: &HookEndpoint,
    sid: &str,
    resume_id: Option<&str>,
    fork: bool,
    init_prompt: Option<&str>,
) -> AgentSpawn {
    prepare_with_args(
        kind,
        shell_path,
        exe_path,
        ep,
        sid,
        resume_id,
        fork,
        init_prompt,
        None,
        None,
        None,
    )
}

/// Like `prepare`, but also accepts custom agent launch arguments such as `--model opus`. They are split
/// into words and inserted verbatim after built-in flags and before the initial positional prompt. Every
/// local agent receives them on both new and resumed launches so reopening a session preserves its options.
///
/// `bin_path` is the executable path configured globally for this agent type. The manager reads it from
/// `app_settings` at launch time (see `pty/manager.rs::agent_bin_path`). A nonempty value launches the
/// absolute path without consulting `PATH`; otherwise the command name is resolved normally.
///
/// `pi_ext_path` is the absolute path to Pi's state-bridge extension, derived by the manager from the data
/// directory (see `agent/pi.rs`). `Some` stores the path in `env` and loads it with `-e "$VLX_PI_EXT"`;
/// `None` omits the extension, allowing Pi to run without authoritative state reporting.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub fn prepare_with_args(
    kind: SessionKind,
    shell_path: &str,
    exe_path: &str,
    ep: &HookEndpoint,
    sid: &str,
    resume_id: Option<&str>,
    fork: bool,
    init_prompt: Option<&str>,
    extra_args: Option<&str>,
    bin_path: Option<&str>,
    pi_ext_path: Option<&str>,
) -> AgentSpawn {
    prepare_with_args_capabilities(
        kind,
        shell_path,
        exe_path,
        ep,
        sid,
        resume_id,
        fork,
        init_prompt,
        extra_args,
        bin_path,
        pi_ext_path,
        // Only the manager installs the Kiro shadow agent, so every other caller launches without `--agent`.
        None,
        true,
    )
}

/// Production launch entry point. In addition to launch arguments, it explicitly receives whether the
/// installed Codex supports lifecycle hooks and `--dangerously-bypass-hook-trust`. Older versions receive
/// only the existing notify integration and retain screen/busy fallbacks, avoiding unsupported CLI options.
#[allow(clippy::too_many_arguments)]
pub fn prepare_with_args_capabilities(
    kind: SessionKind,
    shell_path: &str,
    exe_path: &str,
    ep: &HookEndpoint,
    sid: &str,
    resume_id: Option<&str>,
    fork: bool,
    init_prompt: Option<&str>,
    extra_args: Option<&str>,
    bin_path: Option<&str>,
    pi_ext_path: Option<&str>,
    kiro_agent: Option<&str>,
    codex_hooks_supported: bool,
) -> AgentSpawn {
    let shell = shell_kind(shell_path);
    let resume = resume_id.and_then(valid_resume_id);
    let mut spawn = match kind {
        // Browser nodes should never reach PTY launch; handle them defensively like Terminal sessions.
        SessionKind::Terminal | SessionKind::Browser => AgentSpawn {
            env: Vec::new(),
            launch: None,
        },
        SessionKind::Claude => {
            // Claude: `claude [--resume <id>] --settings <value> ["$VLX_INIT_PROMPT"]`.
            // The initial prompt is the final positional argument.
            let settings = value_ref(shell, CLAUDE_SETTINGS_ENV);
            let base = match resume {
                // Fork the source conversation's current history into a new session ID.
                Some(rid) if fork => {
                    format!("--resume {rid} --fork-session --settings {settings}")
                }
                Some(rid) => format!("--resume {rid} --settings {settings}"),
                None => format!("--settings {settings}"),
            };
            let mut env = vec![(
                CLAUDE_SETTINGS_ENV.to_string(),
                build_claude_settings(ep, sid),
            )];
            let prompt = init_prompt_suffix(shell, resume, init_prompt, &mut env);
            // Custom arguments follow built-in flags and precede the positional prompt.
            let extra = extra_args_fragment(extra_args);
            AgentSpawn {
                env,
                launch: Some(launch(
                    shell,
                    "claude",
                    bin_path,
                    &format!("{base}{extra}{prompt}"),
                )),
            }
        }
        SessionKind::Codex => {
            // Codex command shape:
            // `codex [resume <id>] -c notify=<value> -c hooks=<value>
            //        --dangerously-bypass-hook-trust --no-alt-screen ["$VLX_INIT_PROMPT"]`。
            // `--no-alt-screen` keeps Codex in normal inline terminal history so interrupting or backing
            // out with Esc does not make VelaTerm's xterm viewport jump to the top during an alternate-screen
            // transition. It affects only Codex processes launched by VelaTerm and does not modify the user's
            // `~/.codex/config.toml`. Notify and hook values directly follow their configuration keys and
            // reference environment variables. The resume subcommand precedes global `-c` options, and the
            // callback URL is supplied separately for `--notify-env`.
            let notify = value_ref(shell, CODEX_NOTIFY_ENV);
            let mut env = vec![(CODEX_NOTIFY_ENV.to_string(), build_codex_notify(exe_path))];
            let status_args = if codex_hooks_supported {
                let hooks = value_ref(shell, CODEX_HOOKS_ENV);
                env.push((CODEX_HOOKS_ENV.to_string(), build_codex_hooks()));
                // VelaTerm constructs these trusted hooks and injects them only into this process. Bypass
                // Codex's interactive hook-trust prompt so every managed session does not require a manual
                // `/hooks` confirmation. This flag affects hook trust only, not approval policy or sandboxing.
                format!(
                    "-c notify={notify} -c features.hooks=true -c hooks={hooks} \
                     --dangerously-bypass-hook-trust --no-alt-screen"
                )
            } else {
                // Older Codex versions do not recognize lifecycle hooks or the trust flag. Preserve the
                // legacy launch options: notify can still report completion, while unavailable phases remain
                // unclassified instead of being inferred from terminal output.
                format!("-c notify={notify} --no-alt-screen")
            };
            let base = match resume {
                // `fork <id>` mirrors `resume <id>` but creates a new rollout without changing the source.
                Some(rid) if fork => format!("fork {rid} {status_args}"),
                Some(rid) => format!("resume {rid} {status_args}"),
                None => status_args,
            };
            env.push((
                CODEX_NOTIFY_URL_ENV.to_string(),
                hook_url(ep, sid, "waiting"),
            ));
            let prompt = init_prompt_suffix(shell, resume, init_prompt, &mut env);
            // Custom arguments follow built-in flags and precede the positional prompt.
            let extra = extra_args_fragment(extra_args);
            AgentSpawn {
                env,
                launch: Some(launch(
                    shell,
                    "codex",
                    bin_path,
                    &format!("{base}{extra}{prompt}"),
                )),
            }
        }
        SessionKind::Opencode => {
            // OpenCode: `opencode [--session <id>] [--prompt "$VLX_INIT_PROMPT"]`. `--session` resumes
            // the same ID without forking, while `--prompt` is injected only for new sessions. The manager
            // appends `OPENCODE_CONFIG_CONTENT` with the plugin's absolute path; the plugin reads injected
            // `VLX_*` values to report events, so this branch only handles the prompt and launch command.
            let mut env = Vec::new();
            let mut args: Vec<String> = Vec::new();
            if let Some(rid) = resume {
                args.push(format!("--session {rid}"));
            }
            // Custom arguments follow built-in flags and precede the initial prompt.
            if let Some(extra) = normalize_extra_args(extra_args) {
                args.push(extra);
            }
            if resume.is_none() {
                if let Some(p) = init_prompt.map(str::trim).filter(|s| !s.is_empty()) {
                    env.push((INIT_PROMPT_ENV.to_string(), p.to_string()));
                    args.push(format!("--prompt {}", prompt_ref(shell, INIT_PROMPT_ENV)));
                }
            }
            let launch = launch(shell, "opencode", bin_path, &args.join(" "));
            AgentSpawn {
                env,
                launch: Some(launch),
            }
        }
        SessionKind::Copilot => {
            // GitHub Copilot CLI: `copilot [--resume=<id>] [--interactive "$VLX_INIT_PROMPT"]`.
            // Hooks require file configuration, so `agent/copilot.rs` installs static command hooks at
            // `~/.copilot/hooks/vlx-term.json`; they read injected `VLX_*` values and no-op outside VelaTerm.
            // Resume must use `--resume=<id>` because the optional-value form does not consume a space-separated
            // ID. A missing ID starts a new session with that UUID, so no existence fallback is needed.
            // `--interactive <prompt>` starts the TUI and executes the initial prompt only for new sessions.
            // Copilot cannot fork; the repository layer guards that operation.
            let mut env = Vec::new();
            let mut args: Vec<String> = Vec::new();
            if let Some(rid) = resume {
                args.push(format!("--resume={rid}"));
            }
            // Custom arguments follow built-in flags and precede the initial prompt.
            if let Some(extra) = normalize_extra_args(extra_args) {
                args.push(extra);
            }
            if resume.is_none() {
                if let Some(p) = init_prompt.map(str::trim).filter(|s| !s.is_empty()) {
                    env.push((INIT_PROMPT_ENV.to_string(), p.to_string()));
                    args.push(format!(
                        "--interactive {}",
                        prompt_ref(shell, INIT_PROMPT_ENV)
                    ));
                }
            }
            let launch = launch(shell, "copilot", bin_path, &args.join(" "));
            AgentSpawn {
                env,
                launch: Some(launch),
            }
        }
        SessionKind::Cursor => {
            // Cursor CLI: `cursor-agent [--resume=<id>] ["$VLX_INIT_PROMPT"]`. Cursor hooks live only in
            // fixed configuration files, so `agent/cursor.rs` installs and merges scripts into
            // `~/.cursor/hooks.json`; they report through injected `VLX_*` values and no-op elsewhere.
            // `--resume=<id>` avoids ambiguity between its optional value and the trailing positional prompt.
            // The ID is the hook payload's UUID `conversation_id`; `resume::confirmed_missing` checks
            // `~/.cursor/chats/*/<id>/` and falls back to a new session if it was deleted. The prompt is the
            // final positional argument for new sessions. Cursor does not support forks.
            let mut env = Vec::new();
            let mut args: Vec<String> = Vec::new();
            if let Some(rid) = resume {
                args.push(format!("--resume={rid}"));
            }
            // Custom arguments must precede the positional prompt or Cursor will treat them as prompt text.
            if let Some(extra) = normalize_extra_args(extra_args) {
                args.push(extra);
            }
            if resume.is_none() {
                if let Some(p) = init_prompt.map(str::trim).filter(|s| !s.is_empty()) {
                    env.push((INIT_PROMPT_ENV.to_string(), p.to_string()));
                    args.push(prompt_ref(shell, INIT_PROMPT_ENV));
                }
            }
            let launch = launch(shell, "cursor-agent", bin_path, &args.join(" "));
            AgentSpawn {
                env,
                launch: Some(launch),
            }
        }
        SessionKind::Antigravity => {
            // Google Antigravity CLI (`agy`):
            // `agy [--conversation=<id>] [permission flag + custom args] ["$VLX_INIT_PROMPT"]`.
            // Hooks require fixed file configuration, installed and merged by `agent/antigravity.rs`; they
            // report through injected `VLX_*` values and no-op outside VelaTerm. `--conversation=<id>` avoids
            // ambiguity with the trailing positional prompt, and the ID comes from the hook payload's
            // `session_id`. Existence is not yet checked. The prompt is injected only for new sessions, and
            // Antigravity does not support forks.
            let mut env = Vec::new();
            let mut args: Vec<String> = Vec::new();
            if let Some(rid) = resume {
                args.push(format!("--conversation={rid}"));
            }
            // Custom arguments, including permission flags, precede the positional prompt.
            if let Some(extra) = normalize_extra_args(extra_args) {
                args.push(extra);
            }
            if resume.is_none() {
                if let Some(p) = init_prompt.map(str::trim).filter(|s| !s.is_empty()) {
                    env.push((INIT_PROMPT_ENV.to_string(), p.to_string()));
                    args.push(prompt_ref(shell, INIT_PROMPT_ENV));
                }
            }
            let launch = launch(shell, "agy", bin_path, &args.join(" "));
            AgentSpawn {
                env,
                launch: Some(launch),
            }
        }
        SessionKind::Cline => {
            // Cline CLI: `cline -i [--id <id>] [permission flag + custom args] ["$VLX_INIT_PROMPT"]`.
            // Always pass `-i`; a positional prompt without it starts a headless one-shot task rather than
            // the interactive TUI. `agent/cline.rs` installs hook scripts in `<data_dir>/cline/hooks/`, and
            // the manager exposes that directory as `CLINE_HOOKS_DIR` only to VelaTerm sessions. The scripts
            // report through injected `VLX_*` values. Resume uses the documented `--id <id>` form after ID
            // validation. The final positional prompt applies only to new sessions. Cline cannot fork.
            let mut env = Vec::new();
            // Keep `-i` first so resume options, custom arguments, and the prompt all enter the interactive TUI.
            let mut args: Vec<String> = vec!["-i".to_string()];
            if let Some(rid) = resume {
                args.push(format!("--id {rid}"));
            }
            // Custom arguments, including permission flags, precede the positional prompt.
            if let Some(extra) = normalize_extra_args(extra_args) {
                args.push(extra);
            }
            if resume.is_none() {
                if let Some(p) = init_prompt.map(str::trim).filter(|s| !s.is_empty()) {
                    env.push((INIT_PROMPT_ENV.to_string(), p.to_string()));
                    args.push(prompt_ref(shell, INIT_PROMPT_ENV));
                }
            }
            let launch = launch(shell, "cline", bin_path, &args.join(" "));
            AgentSpawn {
                env,
                launch: Some(launch),
            }
        }
        SessionKind::Pi => {
            // Pi: `pi [--session <id> | --fork <id>] -e "$VLX_PI_EXT" [custom args] ["$VLX_INIT_PROMPT"]`.
            // Pi's official `-e` option loads a local extension for this launch only; built-in jiti transpiles
            // `.ts` without compilation, and the user's `~/.pi` configuration remains untouched. The extension
            // reports via injected `VLX_*` values. Its path comes from `pi_ext_path` and is referenced through
            // `VLX_PI_EXT` to handle spaces safely; without it, Pi runs with screen detection as the fallback.
            // `--session <id>` resumes in place, while `--fork <source-id>` uses Pi's native fork support and
            // takes precedence. The prompt is the final positional argument for new sessions. Pi intentionally
            // has no permission prompts, asking state, or bypass flag.
            let mut env = Vec::new();
            let mut args: Vec<String> = Vec::new();
            match resume {
                Some(rid) if fork => args.push(format!("--fork {rid}")),
                Some(rid) => args.push(format!("--session {rid}")),
                None => {}
            }
            if let Some(ext) = pi_ext_path {
                env.push((PI_EXT_ENV.to_string(), ext.to_string()));
                args.push(format!("-e {}", prompt_ref(shell, PI_EXT_ENV)));
            }
            // Custom arguments follow built-in flags and precede the initial prompt.
            if let Some(extra) = normalize_extra_args(extra_args) {
                args.push(extra);
            }
            if resume.is_none() {
                if let Some(p) = init_prompt.map(str::trim).filter(|s| !s.is_empty()) {
                    env.push((INIT_PROMPT_ENV.to_string(), p.to_string()));
                    args.push(prompt_ref(shell, INIT_PROMPT_ENV));
                }
            }
            let launch = launch(shell, "pi", bin_path, &args.join(" "));
            AgentSpawn {
                env,
                launch: Some(launch),
            }
        }
        SessionKind::Crush => {
            // Crush: `crush [--session <id>] [--yolo + custom args]`. Because hooks can only live in
            // `crush.json`, `agent/crush.rs` creates a shadow configuration under `<data_dir>/crush/` by
            // cloning the user's global configuration and merging VelaTerm's `PreToolUse` hook. The manager
            // points `CRUSH_GLOBAL_CONFIG` to it without modifying user files. The hook reports `e=working`
            // and captures `session_id`, but Crush has no authoritative waiting hook; idle state relies on
            // `screenDetect.ts::detectCrush`. `--session <id>` resumes in place, with existence checking not
            // yet implemented. Do not inject a positional prompt: the root command is an interactive TUI,
            // while prompts belong to the noninteractive `crush run <prompt>` mode. Crush cannot fork.
            let _ = init_prompt; // Crush v1 does not inject an initial prompt; see the rationale above.
            let mut args: Vec<String> = Vec::new();
            if let Some(rid) = resume {
                args.push(format!("--session {rid}"));
            }
            // Custom arguments, including the `--yolo` permission flag, follow built-in flags.
            if let Some(extra) = normalize_extra_args(extra_args) {
                args.push(extra);
            }
            let launch = launch(shell, "crush", bin_path, &args.join(" "));
            AgentSpawn {
                env: Vec::new(),
                launch: Some(launch),
            }
        }
        SessionKind::Kimi => {
            // Kimi Code CLI: `kimi [--session <id>] [--yolo + custom args]`. K3 is the current model, but
            // do not force `--model`; preserve the user's selection. The interactive TUI has no initial-prompt
            // argument (`-p` switches to a noninteractive run-and-exit mode), so spawned tasks are not submitted automatically.
            let _ = init_prompt;
            let mut args: Vec<String> = Vec::new();
            if let Some(rid) = resume {
                args.push(format!("--session {rid}"));
            }
            if let Some(extra) = normalize_extra_args(extra_args) {
                args.push(extra);
            }
            let launch = launch(shell, "kimi", bin_path, &args.join(" "));
            AgentSpawn {
                env: Vec::new(),
                launch: Some(launch),
            }
        }
        SessionKind::Kiro => {
            // Kiro CLI:
            // `kiro-cli chat [--resume-id <id>] [permission flag + custom args] ["$VLX_INIT_PROMPT"]`.
            // Kiro reads lifecycle hooks only from an agent config, so `agent/kiro.rs` clones the user's
            // default agent into `~/.kiro/agents/vlx-term.json` with our hooks merged in and selects it with
            // `--agent`; the hooks report through injected `VLX_*` values and no-op elsewhere.
            // The ID is the hook payload's `session_id`. Sessions live in an internal SQLite database, so
            // `resume::confirmed_missing` cannot check existence and never downgrades to a new session.
            // The prompt is the final positional argument for new sessions. Kiro does not support forks.
            let mut env = Vec::new();
            let mut args: Vec<String> = vec!["chat".to_string()];
            // The shadow agent carries our hooks. Omit the flag when installation failed so Kiro still starts
            // with the user's own agent instead of erroring on an agent that does not exist.
            if let Some(agent) = kiro_agent.map(str::trim).filter(|s| !s.is_empty()) {
                args.push(format!("--agent {agent}"));
            }
            if let Some(rid) = resume {
                args.push(format!("--resume-id {rid}"));
            }
            // Custom arguments, including the permission flag, precede the positional prompt.
            if let Some(extra) = normalize_extra_args(extra_args) {
                args.push(extra);
            }
            if resume.is_none() {
                if let Some(p) = init_prompt.map(str::trim).filter(|s| !s.is_empty()) {
                    env.push((INIT_PROMPT_ENV.to_string(), p.to_string()));
                    args.push(prompt_ref(shell, INIT_PROMPT_ENV));
                }
            }
            let launch = launch(shell, "kiro-cli", bin_path, &args.join(" "));
            AgentSpawn {
                env,
                launch: Some(launch),
            }
        }
        SessionKind::Grok => {
            // Grok Build: `grok [--resume <id> | --session-id <sid>] --no-alt-screen
            // [--always-approve + custom args] ["$VLX_INIT_PROMPT"]`. A stable VelaTerm UUID prevents
            // parallel sessions in the same directory from sharing Grok's "most recent" conversation.
            // The positional PROMPT starts an interactive session; only `-p/--single` exits after one turn.
            let mut args: Vec<String> = Vec::new();
            if let Some(rid) = resume {
                args.push(format!("--resume {rid}"));
            } else if uuid::Uuid::parse_str(sid).is_ok() {
                args.push(format!("--session-id {sid}"));
            }
            args.push("--no-alt-screen".to_string());
            if let Some(extra) = normalize_extra_args(extra_args) {
                args.push(extra);
            }
            let mut env = Vec::new();
            let prompt = init_prompt_suffix(shell, resume, init_prompt, &mut env);
            let launch = launch(
                shell,
                "grok",
                bin_path,
                &format!("{}{prompt}", args.join(" ")),
            );
            AgentSpawn {
                env,
                launch: Some(launch),
            }
        }
        SessionKind::Zoo => {
            // Zoo Code still uses the `roo` command. Reuse VelaTerm's UUID session ID as the Zoo task ID for
            // a stable one-to-one mapping: create it on first launch, then resume after validation in `resume.rs`.
            let mut env = Vec::new();
            let mut args: Vec<String> = Vec::new();
            if let Some(rid) = resume {
                args.push(format!("--session-id {rid}"));
            } else if uuid::Uuid::parse_str(sid).is_ok() {
                args.push(format!("--create-with-session-id {sid}"));
            }
            if let Some(extra) = normalize_extra_args(extra_args) {
                args.push(extra);
            }
            if resume.is_none() {
                if let Some(p) = init_prompt.map(str::trim).filter(|s| !s.is_empty()) {
                    env.push((INIT_PROMPT_ENV.to_string(), p.to_string()));
                    args.push(prompt_ref(shell, INIT_PROMPT_ENV));
                }
            }
            let launch = launch(shell, "roo", bin_path, &args.join(" "));
            AgentSpawn {
                env,
                launch: Some(launch),
            }
        }
    };
    // Add the missing-agent report URL to every local agent. The guard's `else` branch GETs it; Terminal
    // and Browser sessions have no launch guard and do not need it.
    if matches!(
        kind,
        SessionKind::Claude
            | SessionKind::Codex
            | SessionKind::Opencode
            | SessionKind::Copilot
            | SessionKind::Cursor
            | SessionKind::Antigravity
            | SessionKind::Cline
            | SessionKind::Pi
            | SessionKind::Crush
            | SessionKind::Kimi
            | SessionKind::Kiro
            | SessionKind::Grok
            | SessionKind::Zoo
    ) {
        spawn
            .env
            .push((NOTFOUND_URL_ENV.to_string(), hook_url(ep, sid, "notfound")));
    }
    spawn
}

/// Validates an SSH target such as `user@host`. Because it is inserted unquoted into the local shell's SSH
/// command, only `[0-9a-zA-Z._@:-]` is accepted to prevent command injection. Invalid hosts do not launch.
/// An optional `:port` suffix is allowed and converted to `-p` by `split_ssh_target`.
pub(crate) fn valid_ssh_target(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '@' | ':'))
}

/// Splits `user@host[:port]` into an SSH target and optional port. SSH does not accept `host:port` as its
/// host operand, so a numeric final segment becomes a separate `-p` argument; otherwise the input remains intact.
pub(crate) fn split_ssh_target(host: &str) -> (&str, Option<&str>) {
    match host.rsplit_once(':') {
        Some((target, port)) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => {
            (target, Some(port))
        }
        _ => (host, None),
    }
}

/// Standard Base64 encoding with `=` padding and no line breaks. Used for shell-safe settings JSON and by
/// `files::read_file_base64` when encoding file chunks.
pub(crate) fn base64_encode(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep() -> HookEndpoint {
        HookEndpoint {
            port: 51234,
            token: "tok-abc".to_string(),
        }
    }

    /// Asserts that the guarded launch string contains the expected inner command. Variant tests focus on
    /// flag, resume, and prompt combinations; `launch_cmd_*_guards_missing_bin` verifies each guard exactly.
    /// Negative cases also assert missing environment values, so an inner-command substring is sufficient.
    fn assert_launch_inner(launch: &str, inner: &str) {
        assert!(
            launch.contains(inner),
            "the guarded launch string should contain the inner command\n  expected inner: {inner}\n  got: {launch}"
        );
    }

    #[test]
    fn launch_cmd_posix_guards_missing_bin() {
        // POSIX launches directly only when `command -v` succeeds, preserving aliases. Otherwise it
        // reports through curl or wget and prints a readable message. `if/else` avoids false reports on nonzero exits.
        let s = launch_cmd(
            ShellKind::Posix,
            "claude",
            "--settings \"$VLX_CLAUDE_SETTINGS\"",
        );
        assert_eq!(
            s,
            "printf '\\033[3J\\033[2J\\033[H'; \
             if command -v claude >/dev/null 2>&1; then claude --settings \"$VLX_CLAUDE_SETTINGS\"; \
             else { curl -fsS -m 2 \"$VLX_NOTFOUND_URL\" >/dev/null 2>&1 || \
             wget -qO- -T 2 \"$VLX_NOTFOUND_URL\" >/dev/null 2>&1; }; \
             printf '%s\\n' '[VelaTerm] claude not found on PATH. Check that it is installed \
             and your shell profile loaded, then re-run.'; fi"
        );
        // With no arguments, the inner command is the bare binary.
        let bare = launch_cmd(ShellKind::Posix, "opencode", "");
        assert!(bare.starts_with(
            "printf '\\033[3J\\033[2J\\033[H'; \
             if command -v opencode >/dev/null 2>&1; then opencode; else"
        ));
        assert!(bare.contains("\"$VLX_NOTFOUND_URL\""));
    }

    #[test]
    fn launch_cmd_powershell_guards_missing_bin() {
        // PowerShell probes with `Get-Command`; failure reports through `Invoke-WebRequest`, then displays
        // guidance with `Write-Host`, including the execution-policy fix.
        assert_eq!(
            launch_cmd(ShellKind::PowerShell, "claude", "--settings $env:VLX_CLAUDE_SETTINGS"),
            "[Console]::Write([char]27 + '[3J' + [char]27 + '[2J' + [char]27 + '[H'); \
             if (Get-Command claude -ErrorAction SilentlyContinue) { claude --settings $env:VLX_CLAUDE_SETTINGS } \
             else { try { Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 $env:VLX_NOTFOUND_URL | Out-Null } \
             catch {}; Write-Host '[VelaTerm] claude not found on PATH. Your shell profile may be blocked - run: \
             Set-ExecutionPolicy -Scope CurrentUser RemoteSigned, then reopen the session.' }"
        );
    }

    #[test]
    fn launch_cmd_fish_guards_missing_bin() {
        assert_eq!(
            launch_cmd(
                ShellKind::Fish,
                "claude",
                "--settings \"$VLX_CLAUDE_SETTINGS\""
            ),
            "printf '\\033[3J\\033[2J\\033[H'; \
             if type -q claude; claude --settings \"$VLX_CLAUDE_SETTINGS\"; \
             else; curl -fsS -m 2 \"$VLX_NOTFOUND_URL\" >/dev/null 2>&1; \
             or wget -qO- -T 2 \"$VLX_NOTFOUND_URL\" >/dev/null 2>&1; \
             echo '[VelaTerm] claude not found on PATH. Check that it is installed \
             and your shell profile loaded, then re-run.'; end"
        );
    }

    #[test]
    fn launch_cmd_cmd_guards_missing_bin() {
        // cmd.exe probes with `where` and branches explicitly on `errorlevel` to avoid false reports.
        // It only echoes the message because re-parsing `&` in `%URL%` would corrupt the hook URL.
        assert_eq!(
            launch_cmd(
                ShellKind::Cmd,
                "claude",
                "--settings \"%VLX_CLAUDE_SETTINGS%\""
            ),
            "cls & where claude >nul 2>nul & if errorlevel 1 \
             (echo [VelaTerm] claude not found on PATH. Check that it is installed \
             and your shell profile loaded, then re-run.) \
             else (claude --settings \"%VLX_CLAUDE_SETTINGS%\")"
        );
    }

    #[test]
    fn launch_cmd_at_posix_uses_file_test_and_quotes() {
        // Absolute paths use `[ -x ]` rather than `command -v`, are quoted to allow spaces, and still
        // report through the hook. The fallback message points to Settings instead of `PATH`.
        let s = launch_cmd_at(
            ShellKind::Posix,
            "/Users/me/my tools/claude",
            "--settings x",
        );
        assert!(s.contains("if [ -x '/Users/me/my tools/claude' ]; then '/Users/me/my tools/claude' --settings x; else"));
        assert!(
            s.contains("\"$VLX_NOTFOUND_URL\""),
            "a miss must still be reported through the hook"
        );
        assert!(s.contains("Settings, Agents tab"), "the hint should point at the settings page");
        assert!(!s.contains("command -v"), "a full path should no longer search PATH");
        // With no arguments, the inner command is only the quoted path.
        let bare = launch_cmd_at(ShellKind::Posix, "/opt/bin/codex", "");
        assert!(bare.contains("then '/opt/bin/codex'; else"));
    }

    #[test]
    fn launch_cmd_at_posix_escapes_single_quote() {
        // Escape a single quote in the path as `'\''` so the command remains valid.
        let s = launch_cmd_at(ShellKind::Posix, "/a/it's/claude", "");
        assert!(s.contains("if [ -x '/a/it'\\''s/claude' ]"));
    }

    #[test]
    fn launch_cmd_at_powershell_uses_testpath_and_call_operator() {
        // PowerShell probes with `Test-Path` and invokes the quoted path with `&`.
        let s = launch_cmd_at(
            ShellKind::PowerShell,
            r"C:\Users\me\.local\bin\claude.exe",
            "--settings $env:X",
        );
        assert!(s.contains(
            r"if (Test-Path -LiteralPath 'C:\Users\me\.local\bin\claude.exe' -PathType Leaf) { & 'C:\Users\me\.local\bin\claude.exe' --settings $env:X }"
        ));
        assert!(s.contains("$env:VLX_NOTFOUND_URL"), "a miss is still reported through the hook");
    }

    #[test]
    fn launch_cmd_at_fish_and_cmd() {
        let f = launch_cmd_at(ShellKind::Fish, "/opt/claude", "-x");
        assert!(f.contains("if test -x '/opt/claude'; '/opt/claude' -x; else;"));
        // cmd.exe probes with `if exist`; carets escape path parentheses in echoed text so the block stays intact.
        let c = launch_cmd_at(ShellKind::Cmd, r"C:\Program Files (x86)\codex.cmd", "-x");
        assert!(c.contains(r#"if exist "C:\Program Files (x86)\codex.cmd" ("C:\Program Files (x86)\codex.cmd" -x) else ("#));
        assert!(c.contains(r"C:\Program Files ^(x86^)\codex.cmd does not exist"));
    }

    #[test]
    fn prepare_with_bin_path_launches_via_full_path() {
        // End to end: a configured Claude path is invoked directly while preserving its flags.
        let a = prepare_with_args(
            SessionKind::Claude,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
            Some("--model opus"),
            Some("/Users/me/.local/bin/claude"),
            None,
        );
        let launch = a.launch.unwrap();
        assert!(launch.contains(
            "'/Users/me/.local/bin/claude' --settings \"$VLX_CLAUDE_SETTINGS\" --model opus"
        ));
        assert!(!launch.contains("command -v"));
    }

    #[test]
    fn prepare_local_agents_inject_notfound_url() {
        // Every local agent receives a missing-agent URL with `e=notfound` for its guard to GET.
        for kind in [
            SessionKind::Claude,
            SessionKind::Codex,
            SessionKind::Opencode,
            SessionKind::Copilot,
            SessionKind::Cursor,
            SessionKind::Antigravity,
            SessionKind::Cline,
            SessionKind::Pi,
        ] {
            let a = prepare(kind, "/bin/zsh", "/exe", &ep(), "sid-1", None, false, None);
            let url = a
                .env
                .iter()
                .find(|(k, _)| k == NOTFOUND_URL_ENV)
                .unwrap_or_else(|| panic!("{kind:?} should have {NOTFOUND_URL_ENV} injected"));
            assert_eq!(
                url.1,
                "http://127.0.0.1:51234/hook/sid-1?t=tok-abc&e=notfound"
            );
        }
        // Terminal sessions have no guard and receive no URL.
        let term = prepare(
            SessionKind::Terminal,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
        );
        assert!(!term.env.iter().any(|(k, _)| k == NOTFOUND_URL_ENV));
    }

    #[test]
    fn shell_kind_detects_families() {
        assert_eq!(shell_kind("/bin/zsh"), ShellKind::Posix);
        assert_eq!(shell_kind("/bin/bash"), ShellKind::Posix);
        assert_eq!(shell_kind("/usr/local/bin/fish"), ShellKind::Fish);
        assert_eq!(
            shell_kind("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"),
            ShellKind::PowerShell
        );
        assert_eq!(shell_kind("pwsh"), ShellKind::PowerShell);
        assert_eq!(shell_kind("C:\\Windows\\System32\\cmd.exe"), ShellKind::Cmd);
    }

    #[test]
    fn claude_settings_is_valid_json_with_baked_urls() {
        let s = build_claude_settings(&ep(), "sid-1");
        let v: serde_json::Value = serde_json::from_str(&s).expect("should be valid JSON");
        // UserPromptSubmit maps to working; the URL includes the port, session ID, token, and event.
        let up = &v["hooks"]["UserPromptSubmit"][0]["hooks"][0];
        assert_eq!(up["type"], "http");
        assert_eq!(
            up["url"],
            "http://127.0.0.1:51234/hook/sid-1?t=tok-abc&e=working"
        );
        // Stop maps to waiting.
        assert_eq!(
            v["hooks"]["Stop"][0]["hooks"][0]["url"],
            "http://127.0.0.1:51234/hook/sid-1?t=tok-abc&e=waiting"
        );
        // Permission and input notifications map to asking.
        assert_eq!(
            v["hooks"]["Notification"][0]["matcher"],
            "permission_prompt|elicitation_dialog"
        );
        assert_eq!(
            v["hooks"]["Notification"][0]["hooks"][0]["url"],
            "http://127.0.0.1:51234/hook/sid-1?t=tok-abc&e=asking"
        );
        // Idle reminders map to idle, which the server silently treats as waiting.
        assert_eq!(v["hooks"]["Notification"][1]["matcher"], "idle_prompt");
        assert_eq!(
            v["hooks"]["Notification"][1]["hooks"][0]["url"],
            "http://127.0.0.1:51234/hook/sid-1?t=tok-abc&e=idle"
        );
    }

    #[test]
    fn codex_notify_is_argv_array() {
        let s = build_codex_notify("/Apps/vlx-term");
        let v: serde_json::Value = serde_json::from_str(&s).expect("should be a valid JSON array");
        assert_eq!(v[0], "/Apps/vlx-term");
        assert_eq!(v[1], "--notify-env");
        // The URL lives in an environment variable, allowing static configuration to use a dynamic port.
        assert!(v.get(2).is_none());
    }

    #[test]
    fn codex_hooks_are_valid_and_cover_full_turn_lifecycle() {
        let hooks = build_codex_hooks();
        for event in [
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "PermissionRequest",
            "Stop",
            "SessionEnd",
        ] {
            assert!(
                hooks.contains(&format!("{event} = [{{ hooks = [")),
                "{event} should be configured"
            );
        }
        assert_eq!(
            hooks.matches("--codex-hook working").count(),
            2,
            "only UserPromptSubmit reports the working state that starts a turn, with one Unix and one Windows command"
        );
        assert_eq!(
            hooks.matches("--codex-hook tool").count(),
            2,
            "PreToolUse reports a mid-turn working that the out-of-order guard can intercept"
        );
        assert_eq!(
            hooks.matches("--codex-hook ready").count(),
            2,
            "SessionStart carries one Unix and one Windows health-handshake command"
        );
        assert_eq!(hooks.matches("--codex-hook asking").count(), 2);
        assert_eq!(
            hooks.matches("--codex-hook waiting").count(),
            4,
            "Stop and SessionEnd each carry one Unix and one Windows command"
        );
        assert!(
            !hooks.contains("PostToolUse"),
            "PostToolUse races Stop and would push a finished turn back into working"
        );
        assert!(
            !hooks.contains("SubagentStop"),
            "a subagent stopping must not end the root turn"
        );
        assert!(hooks.contains("type = \"command\""));
        assert!(hooks.contains("timeout = 5"));
        assert!(hooks.contains("$VLX_EXE"));
        assert!(hooks.contains("$env:VLX_EXE"));
    }

    #[test]
    fn codex_config_snippet_quotes_exe() {
        let s = codex_config_snippet("/Apps/vlx-term");
        assert_eq!(s, "notify = [\"/Apps/vlx-term\", \"--notify-env\"]");
    }

    #[test]
    fn permission_flag_maps_skip_per_agent() {
        // `skip` maps to each agent's bypass-all-confirmations flag.
        assert_eq!(
            permission_flag(SessionKind::Claude, Some("skip")),
            Some("--dangerously-skip-permissions")
        );
        assert_eq!(
            permission_flag(SessionKind::Codex, Some("skip")),
            Some("--dangerously-bypass-approvals-and-sandbox")
        );
        assert_eq!(
            permission_flag(SessionKind::Copilot, Some("skip")),
            Some("--allow-all-tools")
        );
        assert_eq!(
            permission_flag(SessionKind::Cursor, Some("skip")),
            Some("--force")
        );
        // Cline is inverted: skip maps to `--auto-approve true`.
        assert_eq!(
            permission_flag(SessionKind::Cline, Some("skip")),
            Some("--auto-approve true")
        );
        assert_eq!(
            permission_flag(SessionKind::Grok, Some("skip")),
            Some("--always-approve")
        );
        // OpenCode has no equivalent flag, Pi has no confirmation mechanism, and Terminal does not apply.
        assert_eq!(permission_flag(SessionKind::Opencode, Some("skip")), None);
        assert_eq!(permission_flag(SessionKind::Pi, Some("skip")), None);
        assert_eq!(permission_flag(SessionKind::Terminal, Some("skip")), None);
    }

    #[test]
    fn permission_flag_default_and_unknown_add_nothing() {
        // None, `default`, and unknown values add no flag and preserve incremental approval.
        assert_eq!(permission_flag(SessionKind::Claude, None), None);
        assert_eq!(permission_flag(SessionKind::Claude, Some("default")), None);
        assert_eq!(permission_flag(SessionKind::Claude, Some("yolo")), None);
        assert_eq!(permission_flag(SessionKind::Claude, Some("")), None);
    }

    #[test]
    fn permission_flag_cline_both_directions() {
        // Cline alone receives a flag in default mode because its native default is full auto-approval.
        assert_eq!(
            permission_flag(SessionKind::Cline, Some("skip")),
            Some("--auto-approve true")
        );
        assert_eq!(
            permission_flag(SessionKind::Cline, None),
            Some("--auto-approve false")
        );
        assert_eq!(
            permission_flag(SessionKind::Cline, Some("default")),
            Some("--auto-approve false")
        );
        // Unknown values also use the default incremental-approval mode.
        assert_eq!(
            permission_flag(SessionKind::Cline, Some("bogus")),
            Some("--auto-approve false")
        );
    }

    #[test]
    fn merge_permission_flag_combines_flag_and_user_args() {
        // The permission flag precedes user arguments.
        assert_eq!(
            merge_permission_flag(SessionKind::Claude, Some("skip"), Some("--model opus")),
            Some("--dangerously-skip-permissions --model opus".to_string())
        );
        // Permission flag only.
        assert_eq!(
            merge_permission_flag(SessionKind::Claude, Some("skip"), None),
            Some("--dangerously-skip-permissions".to_string())
        );
        // User arguments only, with default permissions.
        assert_eq!(
            merge_permission_flag(SessionKind::Claude, None, Some("--model opus")),
            Some("--model opus".to_string())
        );
        // Both empty yields `None`.
        assert_eq!(merge_permission_flag(SessionKind::Claude, None, None), None);
        // OpenCode has no skip flag, so only user arguments remain.
        assert_eq!(
            merge_permission_flag(SessionKind::Opencode, Some("skip"), Some("--foo")),
            Some("--foo".to_string())
        );
    }

    #[test]
    fn model_effort_flags_map_per_agent() {
        // Claude uses its native --model/--effort flags.
        assert_eq!(
            model_effort_flags(SessionKind::Claude, Some("fable"), Some("high")),
            Some("--model fable --effort high".to_string())
        );
        // Either setting works alone.
        assert_eq!(
            model_effort_flags(SessionKind::Claude, Some("opus-5"), None),
            Some("--model opus-5".to_string())
        );
        assert_eq!(
            model_effort_flags(SessionKind::Claude, None, Some("xhigh")),
            Some("--effort xhigh".to_string())
        );
        // Codex uses -m and the reasoning-effort config override.
        assert_eq!(
            model_effort_flags(SessionKind::Codex, Some("gpt-5.6-luna"), Some("xhigh")),
            Some("-m gpt-5.6-luna -c model_reasoning_effort=xhigh".to_string())
        );
        // Agents without a verified mapping receive no flags.
        assert_eq!(
            model_effort_flags(SessionKind::Copilot, Some("gpt-5"), Some("high")),
            None
        );
        // Values with shell metacharacters or whitespace are dropped, never quoted.
        assert_eq!(
            model_effort_flags(SessionKind::Claude, Some("opus; rm -rf /"), Some("high$(x)")),
            None
        );
        assert_eq!(model_effort_flags(SessionKind::Claude, None, None), None);
    }

    #[test]
    fn merge_model_effort_flags_precede_user_args() {
        // Translated flags come first so an explicit user --model wins as the later occurrence.
        assert_eq!(
            merge_model_effort_flags(
                SessionKind::Claude,
                Some("fable"),
                Some("high"),
                Some("--model sonnet")
            ),
            Some("--model fable --effort high --model sonnet".to_string())
        );
        // User arguments pass through untouched for unmapped agents.
        assert_eq!(
            merge_model_effort_flags(SessionKind::Copilot, Some("gpt-5"), None, Some("--foo")),
            Some("--foo".to_string())
        );
        assert_eq!(
            merge_model_effort_flags(SessionKind::Claude, None, None, None),
            None
        );
    }

    #[test]
    fn prepare_terminal_injects_nothing() {
        let a = prepare(
            SessionKind::Terminal,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
        );
        assert!(a.env.is_empty());
        assert!(a.launch.is_none());
    }

    #[test]
    fn prepare_claude_posix_direct_launch() {
        let a = prepare(
            SessionKind::Claude,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
        );
        assert_eq!(a.env[0].0, CLAUDE_SETTINGS_ENV);
        // The environment value is valid settings JSON.
        serde_json::from_str::<serde_json::Value>(&a.env[0].1).unwrap();
        // Launch directly through the environment reference without defining an alias-conflicting function.
        assert_launch_inner(
            &a.launch.unwrap(),
            "claude --settings \"$VLX_CLAUDE_SETTINGS\"",
        );
    }

    #[test]
    fn prepare_codex_posix_direct_launch() {
        let a = prepare(
            SessionKind::Codex,
            "/bin/bash",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
        );
        // Three variables: notify array, lifecycle hooks, and notify callback URL.
        assert_eq!(a.env[0].0, CODEX_NOTIFY_ENV);
        assert_eq!(a.env[1].0, CODEX_HOOKS_ENV);
        assert!(a.env[1].1.starts_with("{ SessionStart ="));
        assert_eq!(a.env[2].0, CODEX_NOTIFY_URL_ENV);
        assert_eq!(
            a.env[2].1,
            "http://127.0.0.1:51234/hook/s?t=tok-abc&e=waiting"
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "codex -c notify=\"$VLX_CODEX_NOTIFY\" -c features.hooks=true -c hooks=\"$VLX_CODEX_HOOKS\" --dangerously-bypass-hook-trust --no-alt-screen",
        );
    }

    #[test]
    fn prepare_codex_without_hook_support_falls_back_to_notify() {
        let a = prepare_with_args_capabilities(
            SessionKind::Codex,
            "/bin/bash",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
            None,
            None,
            None,
            None,
            false,
        );
        assert!(a.env.iter().any(|(key, _)| key == CODEX_NOTIFY_ENV));
        assert!(!a.env.iter().any(|(key, _)| key == CODEX_HOOKS_ENV));
        assert_launch_inner(
            &a.launch.unwrap(),
            "codex -c notify=\"$VLX_CODEX_NOTIFY\" --no-alt-screen",
        );
    }

    #[test]
    fn opencode_config_content_is_valid_json_with_plugin_path() {
        let s = build_opencode_config_content("/data/opencode/vlx-opencode-notify.js", None);
        let v: serde_json::Value = serde_json::from_str(&s).expect("should be valid JSON");
        assert_eq!(v["plugin"][0], "/data/opencode/vlx-opencode-notify.js");
        // Declare only the plugin so merging does not overwrite user settings.
        assert_eq!(v.as_object().unwrap().len(), 1);
    }

    #[test]
    fn prepare_opencode_fresh_launch_has_no_args() {
        // A fresh session without a prompt launches bare `opencode`; the manager injects the state plugin separately.
        let a = prepare(
            SessionKind::Opencode,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
        );
        assert_launch_inner(&a.launch.unwrap(), "opencode");
        // A fresh promptless session injects only the missing-agent URL, not `INIT_PROMPT`.
        assert!(
            !a.env.iter().any(|(k, _)| k == INIT_PROMPT_ENV),
            "a fresh session with no prompt should not set INIT_PROMPT"
        );
        assert!(
            a.env.iter().any(|(k, _)| k == NOTFOUND_URL_ENV),
            "a local agent should have the not-installed reporting URL injected"
        );
    }

    #[test]
    fn prepare_opencode_with_resume_uses_session_flag() {
        // Resume uses an unquoted, validated `--session <id>`; OpenCode IDs look like `ses_...`.
        let a = prepare(
            SessionKind::Opencode,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("ses_157f2033dffehEDVNHlPeHfXnc"),
            false,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "opencode --session ses_157f2033dffehEDVNHlPeHfXnc",
        );
    }

    #[test]
    fn prepare_opencode_appends_init_prompt() {
        // Pass the initial prompt as one environment-backed `--prompt` argument.
        let a = prepare(
            SessionKind::Opencode,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            Some("Refactor the foo module for me"),
        );
        assert_launch_inner(&a.launch.unwrap(), "opencode --prompt \"$VLX_INIT_PROMPT\"");
        assert!(a
            .env
            .iter()
            .any(|(k, v)| k == INIT_PROMPT_ENV && v == "Refactor the foo module for me"));
    }

    #[test]
    fn prepare_opencode_skips_prompt_on_resume() {
        // Resume does not reinject the prompt and carries only `--session`.
        let a = prepare(
            SessionKind::Opencode,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("ses_abc"),
            false,
            Some("a prompt that should not be pasted again"),
        );
        assert_launch_inner(&a.launch.unwrap(), "opencode --session ses_abc");
        assert!(!a.env.iter().any(|(k, _)| k == INIT_PROMPT_ENV));
    }

    #[test]
    fn prepare_pi_fresh_launch_is_bare_without_ext_path() {
        // Without a prompt or extension path, a fresh Pi session launches bare and lacks authoritative state.
        let a = prepare(
            SessionKind::Pi,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
        );
        assert_launch_inner(&a.launch.unwrap(), "pi");
        assert!(
            !a.env.iter().any(|(k, _)| k == INIT_PROMPT_ENV),
            "a fresh session with no prompt should not set INIT_PROMPT"
        );
        assert!(
            !a.env.iter().any(|(k, _)| k == PI_EXT_ENV),
            "VLX_PI_EXT is not set when no path is supplied"
        );
        assert!(
            a.env.iter().any(|(k, _)| k == NOTFOUND_URL_ENV),
            "a local agent should have the not-installed reporting URL injected"
        );
    }

    #[test]
    fn prepare_pi_loads_extension_via_env_ref() {
        // An extension path is stored safely in the environment and loaded through `-e "$VLX_PI_EXT"`.
        let a = prepare_with_args(
            SessionKind::Pi,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
            None,
            None,
            Some("/Users/me/Library/Application Support/io.vlinx.vlxterm/pi/vlx-pi-notify.ts"),
        );
        assert_launch_inner(&a.launch.unwrap(), "pi -e \"$VLX_PI_EXT\"");
        assert!(a.env.iter().any(|(k, v)| k == PI_EXT_ENV
            && v == "/Users/me/Library/Application Support/io.vlinx.vlxterm/pi/vlx-pi-notify.ts"));
    }

    #[test]
    fn prepare_pi_with_resume_uses_session_flag() {
        // Resume uses an unquoted, validated `--session <id>` before `-e`.
        let a = prepare_with_args(
            SessionKind::Pi,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("3f2504e0-4f89-41d3-9a0c-0305e82c3301"),
            false,
            None,
            None,
            None,
            Some("/data/pi/vlx-pi-notify.ts"),
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "pi --session 3f2504e0-4f89-41d3-9a0c-0305e82c3301 -e \"$VLX_PI_EXT\"",
        );
    }

    #[test]
    fn prepare_pi_fork_uses_fork_flag() {
        // A first fork launch uses `--fork <source-id>`, mutually exclusive with and preferred over `--session`.
        let a = prepare_with_args(
            SessionKind::Pi,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("3f2504e0-4f89-41d3-9a0c-0305e82c3301"),
            true,
            None,
            None,
            None,
            Some("/data/pi/vlx-pi-notify.ts"),
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "pi --fork 3f2504e0-4f89-41d3-9a0c-0305e82c3301 -e \"$VLX_PI_EXT\"",
        );
    }

    #[test]
    fn prepare_pi_appends_init_prompt_positional() {
        // The environment-backed initial prompt is the final positional argument after `-e`.
        let a = prepare_with_args(
            SessionKind::Pi,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            Some("Refactor the foo module for me"),
            None,
            None,
            Some("/data/pi/vlx-pi-notify.ts"),
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "pi -e \"$VLX_PI_EXT\" \"$VLX_INIT_PROMPT\"",
        );
        assert!(a
            .env
            .iter()
            .any(|(k, v)| k == INIT_PROMPT_ENV && v == "Refactor the foo module for me"));
    }

    #[test]
    fn prepare_pi_skips_prompt_on_resume() {
        // Resume does not reinject the prompt and carries only `--session` and `-e`.
        let a = prepare_with_args(
            SessionKind::Pi,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("uuid-abc"),
            false,
            Some("a prompt that should not be pasted again"),
            None,
            None,
            Some("/data/pi/vlx-pi-notify.ts"),
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "pi --session uuid-abc -e \"$VLX_PI_EXT\"",
        );
        assert!(!a.env.iter().any(|(k, _)| k == INIT_PROMPT_ENV));
    }

    #[test]
    fn prepare_copilot_fresh_launch_is_bare() {
        // A fresh session without a prompt launches bare `copilot`; `agent/copilot.rs` installs static hooks.
        let a = prepare(
            SessionKind::Copilot,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
        );
        assert_launch_inner(&a.launch.unwrap(), "copilot");
        // A fresh promptless session injects only the missing-agent URL, not `INIT_PROMPT`.
        assert!(
            !a.env.iter().any(|(k, _)| k == INIT_PROMPT_ENV),
            "a fresh session with no prompt should not set INIT_PROMPT"
        );
        assert!(
            a.env.iter().any(|(k, _)| k == NOTFOUND_URL_ENV),
            "a local agent should have the not-installed reporting URL injected"
        );
    }

    #[test]
    fn prepare_copilot_resume_uses_equals_form() {
        // Resume must use `--resume=<id>` because the optional-value flag does not consume a space-separated ID.
        let a = prepare(
            SessionKind::Copilot,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("0cb916db-26aa-40f2-86b5-1ba81b225fd2"),
            false,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "copilot --resume=0cb916db-26aa-40f2-86b5-1ba81b225fd2",
        );
    }

    #[test]
    fn prepare_copilot_appends_init_prompt_interactively() {
        // Pass the initial prompt as one environment-backed `--interactive` argument to start and run it in the TUI.
        let a = prepare(
            SessionKind::Copilot,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            Some("Refactor the foo module for me"),
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "copilot --interactive \"$VLX_INIT_PROMPT\"",
        );
        assert!(a
            .env
            .iter()
            .any(|(k, v)| k == INIT_PROMPT_ENV && v == "Refactor the foo module for me"));
    }

    #[test]
    fn prepare_copilot_skips_prompt_on_resume() {
        // Resume does not reinject the prompt and carries only `--resume=`.
        let a = prepare(
            SessionKind::Copilot,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("abc-123"),
            false,
            Some("a prompt that should not be pasted again"),
        );
        assert_launch_inner(&a.launch.unwrap(), "copilot --resume=abc-123");
        assert!(!a.env.iter().any(|(k, _)| k == INIT_PROMPT_ENV));
    }

    #[test]
    fn prepare_kiro_fresh_launch_uses_chat_subcommand() {
        // Kiro's interactive entry point is the `chat` subcommand; `agent/kiro.rs` installs the hooks file.
        let a = prepare(
            SessionKind::Kiro,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
        );
        assert_launch_inner(&a.launch.unwrap(), "kiro-cli chat");
        assert!(
            a.env.iter().any(|(k, _)| k == NOTFOUND_URL_ENV),
            "local agents must inject the missing-agent report URL"
        );
    }

    #[test]
    fn prepare_kiro_resume_uses_resume_id_flag() {
        let a = prepare(
            SessionKind::Kiro,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("f2946a26-3735-4b08-8d05-c928010302d5"),
            false,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "kiro-cli chat --resume-id f2946a26-3735-4b08-8d05-c928010302d5",
        );
    }

    #[test]
    fn prepare_kiro_appends_init_prompt_only_when_fresh() {
        let fresh = prepare(
            SessionKind::Kiro,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            Some("Refactor the foo module"),
        );
        assert_launch_inner(&fresh.launch.unwrap(), "kiro-cli chat \"$VLX_INIT_PROMPT\"");
        assert!(fresh
            .env
            .iter()
            .any(|(k, v)| k == INIT_PROMPT_ENV && v == "Refactor the foo module"));

        let resumed = prepare(
            SessionKind::Kiro,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("f2946a26-3735-4b08-8d05-c928010302d5"),
            false,
            Some("Refactor the foo module"),
        );
        assert_launch_inner(
            &resumed.launch.unwrap(),
            "kiro-cli chat --resume-id f2946a26-3735-4b08-8d05-c928010302d5",
        );
    }

    #[test]
    fn prepare_kiro_skip_permission_uses_trust_all_tools() {
        let skip = merge_permission_flag(SessionKind::Kiro, Some("skip"), None);
        let a = prepare_with_args_capabilities(
            SessionKind::Kiro,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
            skip.as_deref(),
            None,
            None,
            Some("vlx-term"),
            true,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "kiro-cli chat --agent vlx-term --trust-all-tools",
        );
        // The default mode adds no flag, leaving Kiro's own staged approval in place.
        assert!(merge_permission_flag(SessionKind::Kiro, None, None).is_none());
    }

    #[test]
    fn prepare_cursor_fresh_launch_is_bare() {
        // A fresh session without a prompt launches bare `cursor-agent`; `agent/cursor.rs` installs the static hooks.
        let a = prepare(
            SessionKind::Cursor,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
        );
        assert_launch_inner(&a.launch.unwrap(), "cursor-agent");
        // A fresh promptless session injects only the missing-agent URL, not `INIT_PROMPT`.
        assert!(
            !a.env.iter().any(|(k, _)| k == INIT_PROMPT_ENV),
            "a fresh session with no prompt should not set INIT_PROMPT"
        );
        assert!(
            a.env.iter().any(|(k, _)| k == NOTFOUND_URL_ENV),
            "a local agent should have the not-installed reporting URL injected"
        );
    }

    #[test]
    fn prepare_cursor_resume_uses_equals_form() {
        // `--resume=<id>` avoids ambiguity between the optional flag value and the positional prompt.
        let a = prepare(
            SessionKind::Cursor,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("672a3b4c-2f3e-4382-936f-9b991adf40db"),
            false,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "cursor-agent --resume=672a3b4c-2f3e-4382-936f-9b991adf40db",
        );
    }

    #[test]
    fn prepare_cursor_appends_init_prompt() {
        // The environment-backed initial prompt is the final positional argument.
        let a = prepare(
            SessionKind::Cursor,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            Some("Refactor the foo module for me"),
        );
        assert_launch_inner(&a.launch.unwrap(), "cursor-agent \"$VLX_INIT_PROMPT\"");
        assert!(a
            .env
            .iter()
            .any(|(k, v)| k == INIT_PROMPT_ENV && v == "Refactor the foo module for me"));
    }

    #[test]
    fn prepare_cursor_skips_prompt_on_resume() {
        // Resume does not reinject the prompt and carries only `--resume=`.
        let a = prepare(
            SessionKind::Cursor,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("abc-123"),
            false,
            Some("a prompt that should not be pasted again"),
        );
        assert_launch_inner(&a.launch.unwrap(), "cursor-agent --resume=abc-123");
        assert!(!a.env.iter().any(|(k, _)| k == INIT_PROMPT_ENV));
    }

    #[test]
    fn prepare_cline_fresh_launch_is_interactive() {
        // A fresh promptless Cline session launches as `cline -i` to guarantee interactive mode. Hook scripts
        // are installed by `agent/cline.rs` and exposed by the manager through `CLINE_HOOKS_DIR`.
        let a = prepare(
            SessionKind::Cline,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
        );
        assert_launch_inner(&a.launch.unwrap(), "cline -i");
        // A fresh promptless session injects only the missing-agent URL, not `INIT_PROMPT`.
        assert!(
            !a.env.iter().any(|(k, _)| k == INIT_PROMPT_ENV),
            "a fresh session with no prompt should not set INIT_PROMPT"
        );
        assert!(
            a.env.iter().any(|(k, _)| k == NOTFOUND_URL_ENV),
            "a local agent should have the not-installed reporting URL injected"
        );
    }

    #[test]
    fn prepare_cline_resume_uses_id_flag() {
        // Resume uses the documented `--id <id>` form after `-i`.
        let a = prepare(
            SessionKind::Cline,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("019e8662-62f8-7a01-a144-e26a07a5e5bb"),
            false,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "cline -i --id 019e8662-62f8-7a01-a144-e26a07a5e5bb",
        );
    }

    #[test]
    fn prepare_cline_appends_init_prompt() {
        // The environment-backed initial prompt is the final positional argument.
        let a = prepare(
            SessionKind::Cline,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            Some("Refactor the foo module for me"),
        );
        assert_launch_inner(&a.launch.unwrap(), "cline -i \"$VLX_INIT_PROMPT\"");
        assert!(a
            .env
            .iter()
            .any(|(k, v)| k == INIT_PROMPT_ENV && v == "Refactor the foo module for me"));
    }

    #[test]
    fn prepare_cline_skips_prompt_on_resume() {
        // Resume does not reinject the prompt and carries only `--id`.
        let a = prepare(
            SessionKind::Cline,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("abc-123"),
            false,
            Some("a prompt that should not be pasted again"),
        );
        assert_launch_inner(&a.launch.unwrap(), "cline -i --id abc-123");
        assert!(!a.env.iter().any(|(k, _)| k == INIT_PROMPT_ENV));
    }

    #[test]
    fn prepare_cline_fork_degrades_to_fresh() {
        // Cline has no fork support; the branch must not add a fork flag.
        let a = prepare(
            SessionKind::Cline,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("019e8662-62f8-7a01-a144-e26a07a5e5bb"),
            true,
            None,
        );
        // Cline ignores fork semantics and still resumes in place through `--id` without a fork flag.
        let launch = a.launch.unwrap();
        assert!(launch.contains("cline -i --id 019e8662-62f8-7a01-a144-e26a07a5e5bb"));
        assert!(!launch.contains("--fork"), "cline has no fork flag");
    }

    #[test]
    fn prepare_cline_permission_flags_both_directions() {
        // Cline's permission direction is reversed: default injects `--auto-approve false`, while skip injects
        // `--auto-approve true`. Production merges the flag into `extra_args`; verify it lands after `-i`
        // and before the prompt.
        let skip = merge_permission_flag(SessionKind::Cline, Some("skip"), None);
        let a = prepare_with_args(
            SessionKind::Cline,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
            skip.as_deref(),
            None,
            None,
        );
        assert_launch_inner(&a.launch.unwrap(), "cline -i --auto-approve true");

        let def = merge_permission_flag(SessionKind::Cline, None, None);
        let b = prepare_with_args(
            SessionKind::Cline,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
            def.as_deref(),
            None,
            None,
        );
        assert_launch_inner(&b.launch.unwrap(), "cline -i --auto-approve false");
    }

    #[test]
    fn prepare_claude_powershell_uses_env_form() {
        let a = prepare(
            SessionKind::Claude,
            "powershell.exe",
            "C:\\vlx.exe",
            &ep(),
            "s",
            None,
            false,
            None,
        );
        // Escape injected double quotes on PowerShell so `CommandLineToArgvW` does not consume them.
        assert_launch_inner(
            &a.launch.unwrap(),
            "claude --settings $($env:VLX_CLAUDE_SETTINGS -replace '\"', '\\\"')",
        );
    }

    #[test]
    fn prepare_claude_fish_direct_launch() {
        let a = prepare(
            SessionKind::Claude,
            "/usr/bin/fish",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "claude --settings \"$VLX_CLAUDE_SETTINGS\"",
        );
    }

    #[test]
    fn prepare_claude_with_resume_prepends_flag() {
        let a = prepare(
            SessionKind::Claude,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("019e8662-62f8-7a01-a144-e26a07a5e5bb"),
            false,
            None,
        );
        // `--resume <id>` precedes `--settings`; the validated ID is unquoted.
        assert_launch_inner(
            &a.launch.unwrap(),
            "claude --resume 019e8662-62f8-7a01-a144-e26a07a5e5bb --settings \"$VLX_CLAUDE_SETTINGS\"",
        );
    }

    #[test]
    fn prepare_codex_with_resume_uses_subcommand() {
        let a = prepare(
            SessionKind::Codex,
            "/bin/bash",
            "/exe",
            &ep(),
            "s",
            Some("019e8662-62f8-7a01-a144-e26a07a5e5bb"),
            false,
            None,
        );
        // The `resume <id>` subcommand precedes global `-c` options.
        assert_launch_inner(
            &a.launch.unwrap(),
            "codex resume 019e8662-62f8-7a01-a144-e26a07a5e5bb -c notify=\"$VLX_CODEX_NOTIFY\" -c features.hooks=true -c hooks=\"$VLX_CODEX_HOOKS\" --dangerously-bypass-hook-trust --no-alt-screen",
        );
    }

    #[test]
    fn prepare_claude_fork_appends_fork_session_flag() {
        // Claude forks with `--resume <source-id> --fork-session`, leaving the source unchanged.
        let a = prepare(
            SessionKind::Claude,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("019e8662-62f8-7a01-a144-e26a07a5e5bb"),
            true,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "claude --resume 019e8662-62f8-7a01-a144-e26a07a5e5bb --fork-session --settings \"$VLX_CLAUDE_SETTINGS\"",
        );
    }

    #[test]
    fn prepare_codex_fork_uses_fork_subcommand() {
        // Codex forks with the `fork <source-id>` subcommand, mirroring resume syntax.
        let a = prepare(
            SessionKind::Codex,
            "/bin/bash",
            "/exe",
            &ep(),
            "s",
            Some("019e8662-62f8-7a01-a144-e26a07a5e5bb"),
            true,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "codex fork 019e8662-62f8-7a01-a144-e26a07a5e5bb -c notify=\"$VLX_CODEX_NOTIFY\" -c features.hooks=true -c hooks=\"$VLX_CODEX_HOOKS\" --dangerously-bypass-hook-trust --no-alt-screen",
        );
    }

    #[test]
    fn prepare_fork_without_resume_id_degrades_to_fresh() {
        // A fork without a source conversation ID becomes a fresh launch with no fork or resume arguments.
        let a = prepare(
            SessionKind::Claude,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            true,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "claude --settings \"$VLX_CLAUDE_SETTINGS\"",
        );
    }

    #[test]
    fn prepare_rejects_unsafe_resume_id() {
        // Reject IDs containing shell metacharacters and fall back to a new session to prevent injection.
        let a = prepare(
            SessionKind::Claude,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("x; rm -rf ~"),
            false,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "claude --settings \"$VLX_CLAUDE_SETTINGS\"",
        );
    }

    #[test]
    fn prepare_claude_appends_init_prompt() {
        let a = prepare(
            SessionKind::Claude,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            Some("Refactor the foo module for me"),
        );
        // The environment-backed prompt is the final positional argument.
        assert_launch_inner(
            &a.launch.unwrap(),
            "claude --settings \"$VLX_CLAUDE_SETTINGS\" \"$VLX_INIT_PROMPT\"",
        );
        // Store the trimmed original in `VLX_INIT_PROMPT`, never in the command literal.
        let (_, v) = a
            .env
            .iter()
            .find(|(k, _)| k == INIT_PROMPT_ENV)
            .expect("VLX_INIT_PROMPT should be injected");
        assert_eq!(v, "Refactor the foo module for me");
    }

    #[test]
    fn prepare_codex_appends_init_prompt() {
        let a = prepare(
            SessionKind::Codex,
            "/bin/bash",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            Some("Fix the login timeout"),
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "codex -c notify=\"$VLX_CODEX_NOTIFY\" -c features.hooks=true -c hooks=\"$VLX_CODEX_HOOKS\" --dangerously-bypass-hook-trust --no-alt-screen \"$VLX_INIT_PROMPT\"",
        );
        assert!(a
            .env
            .iter()
            .any(|(k, v)| k == INIT_PROMPT_ENV && v == "Fix the login timeout"));
    }

    #[test]
    fn prepare_init_prompt_trimmed_and_binary_safe() {
        // Prompts containing spaces, quotes, newlines, `$`, or backticks remain intact in the environment
        // except for outer trimming. The command only references the variable, preventing parsing or reevaluation.
        let raw = "  line1\nwith spaces \"quotes\" $(whoami) `id`  ";
        let a = prepare(
            SessionKind::Claude,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            Some(raw),
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "claude --settings \"$VLX_CLAUDE_SETTINGS\" \"$VLX_INIT_PROMPT\"",
        );
        let (_, v) = a.env.iter().find(|(k, _)| k == INIT_PROMPT_ENV).unwrap();
        assert_eq!(v, "line1\nwith spaces \"quotes\" $(whoami) `id`");
    }

    #[test]
    fn prepare_powershell_quotes_init_prompt() {
        // PowerShell needs double quotes to keep a spaced prompt in one argument; a bare `$env:` would split it.
        let a = prepare(
            SessionKind::Claude,
            "powershell.exe",
            "C:\\vlx.exe",
            &ep(),
            "s",
            None,
            false,
            Some("do a thing"),
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "claude --settings $($env:VLX_CLAUDE_SETTINGS -replace '\"', '\\\"') \"$env:VLX_INIT_PROMPT\"",
        );
    }

    #[test]
    fn prepare_init_prompt_skipped_on_resume() {
        // Resume does not reinject a prompt that was already delivered at first launch.
        let a = prepare(
            SessionKind::Claude,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("019e8662-62f8-7a01-a144-e26a07a5e5bb"),
            false,
            Some("a prompt that should not be pasted again"),
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "claude --resume 019e8662-62f8-7a01-a144-e26a07a5e5bb --settings \"$VLX_CLAUDE_SETTINGS\"",
        );
        assert!(
            !a.env.iter().any(|(k, _)| k == INIT_PROMPT_ENV),
            "VLX_INIT_PROMPT must not be injected when resuming"
        );
    }

    #[test]
    fn prepare_blank_init_prompt_is_ignored() {
        // Treat a whitespace-only prompt as absent: add neither an argument nor an environment variable.
        let a = prepare(
            SessionKind::Claude,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            Some("   \n  "),
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "claude --settings \"$VLX_CLAUDE_SETTINGS\"",
        );
        assert!(!a.env.iter().any(|(k, _)| k == INIT_PROMPT_ENV));
    }

    #[test]
    fn valid_resume_id_filters() {
        assert_eq!(
            valid_resume_id("019e8662-62f8-7a01-a144-e26a07a5e5bb"),
            Some("019e8662-62f8-7a01-a144-e26a07a5e5bb")
        );
        assert_eq!(valid_resume_id("  abc_DEF-123  "), Some("abc_DEF-123"));
        assert!(valid_resume_id("").is_none());
        assert!(valid_resume_id("a b").is_none(), "a space should be rejected");
        assert!(valid_resume_id("a;b").is_none(), "a semicolon should be rejected");
        assert!(valid_resume_id("$(x)").is_none(), "command substitution should be rejected");
    }

    #[test]
    fn base64_encode_matches_known_vectors() {
        // RFC 4648 test vectors.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        // JSON quotes encode successfully, leaving no shell metacharacters.
        let b = base64_encode(br#"{"a":"b"}"#);
        assert!(b
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=')));
    }

    // ───────────────── Custom launch arguments (`extra_args`) ─────────────────

    #[test]
    fn prepare_with_args_claude_inserts_before_prompt() {
        // Custom arguments are tokenized without added quotes after `--settings` and before the prompt.
        let a = prepare_with_args(
            SessionKind::Claude,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            Some("Fix the login"),
            Some("--model opus"),
            None,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "claude --settings \"$VLX_CLAUDE_SETTINGS\" --model opus \"$VLX_INIT_PROMPT\"",
        );
    }

    #[test]
    fn prepare_with_args_applies_on_resume_too() {
        // Unlike the initial prompt, custom arguments remain on resume so a session keeps the same options.
        let a = prepare_with_args(
            SessionKind::Claude,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("019e8662-62f8-7a01-a144-e26a07a5e5bb"),
            false,
            None,
            Some("--model opus"),
            None,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "claude --resume 019e8662-62f8-7a01-a144-e26a07a5e5bb --settings \"$VLX_CLAUDE_SETTINGS\" --model opus",
        );
    }

    #[test]
    fn prepare_with_args_codex_inserts_args() {
        let a = prepare_with_args(
            SessionKind::Codex,
            "/bin/bash",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
            Some("-m gpt-5"),
            None,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "codex -c notify=\"$VLX_CODEX_NOTIFY\" -c features.hooks=true -c hooks=\"$VLX_CODEX_HOOKS\" --dangerously-bypass-hook-trust --no-alt-screen -m gpt-5",
        );
    }

    #[test]
    fn prepare_with_args_cursor_before_positional_prompt() {
        // Cursor's prompt is positional, so custom arguments must precede it.
        let a = prepare_with_args(
            SessionKind::Cursor,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            Some("Do something"),
            Some("--force"),
            None,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "cursor-agent --force \"$VLX_INIT_PROMPT\"",
        );
    }

    #[test]
    fn prepare_with_args_opencode_resume_appends() {
        let a = prepare_with_args(
            SessionKind::Opencode,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            Some("ses_abc"),
            false,
            None,
            Some("--model anthropic/claude"),
            None,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "opencode --session ses_abc --model anthropic/claude",
        );
    }

    #[test]
    fn prepare_with_args_blank_and_multiline_joined() {
        // Ignore whitespace-only input as if no custom arguments were supplied.
        let a = prepare_with_args(
            SessionKind::Claude,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
            Some("   "),
            None,
            None,
        );
        assert_launch_inner(
            &a.launch.unwrap(),
            "claude --settings \"$VLX_CLAUDE_SETTINGS\"",
        );
        // Fold multiline input into one argument line so later lines cannot execute as separate commands.
        let b = prepare_with_args(
            SessionKind::Claude,
            "/bin/zsh",
            "/exe",
            &ep(),
            "s",
            None,
            false,
            None,
            Some("--model opus\n--add-dir /foo"),
            None,
            None,
        );
        let launch = b.launch.unwrap();
        assert!(
            launch.contains("--model opus --add-dir /foo"),
            "multi-line arguments should be folded onto one line: {launch}"
        );
        assert!(!launch.contains('\n'), "the launch command must be one line, with newlines already folded into spaces");
    }

    #[test]
    fn prepare_grok_uses_stable_session_model_and_interactive_prompt() {
        let sid = "019e8662-62f8-7a01-a144-e26a07a5e5bb";
        let fresh = prepare_with_args(
            SessionKind::Grok,
            "/bin/zsh",
            "/exe",
            &ep(),
            sid,
            None,
            false,
            Some("fix the failing integration test"),
            Some("--model grok-4.5"),
            None,
            None,
        );
        assert_launch_inner(
            &fresh.launch.unwrap(),
            &format!(
                "grok --session-id {sid} --no-alt-screen --model grok-4.5 \"$VLX_INIT_PROMPT\""
            ),
        );
        assert_eq!(
            fresh
                .env
                .iter()
                .find(|(k, _)| k == INIT_PROMPT_ENV)
                .map(|(_, v)| v.as_str()),
            Some("fix the failing integration test")
        );

        let resumed = prepare(
            SessionKind::Grok,
            "/bin/zsh",
            "/exe",
            &ep(),
            sid,
            Some(sid),
            false,
            None,
        );
        assert_launch_inner(
            &resumed.launch.unwrap(),
            &format!("grok --resume {sid} --no-alt-screen"),
        );
    }

    #[test]
    fn normalize_extra_args_filters() {
        assert_eq!(
            normalize_extra_args(Some("--model opus")).as_deref(),
            Some("--model opus")
        );
        assert_eq!(
            normalize_extra_args(Some("  --x  ")).as_deref(),
            Some("--x")
        );
        assert!(normalize_extra_args(Some("")).is_none());
        assert!(normalize_extra_args(Some("   ")).is_none());
        assert_eq!(
            normalize_extra_args(Some("a\nb")).as_deref(),
            Some("a b"),
            "a newline is folded into a space"
        );
        assert_eq!(
            normalize_extra_args(Some("a\r\nb")).as_deref(),
            Some("a b"),
            "a CRLF is folded into a space"
        );
        assert_eq!(
            normalize_extra_args(Some("--model opus\n--verbose")).as_deref(),
            Some("--model opus --verbose"),
            "several lines are folded onto one",
        );
        assert!(
            normalize_extra_args(Some(" \n \r ")).is_none(),
            "whitespace containing only newlines counts as unset"
        );
        assert!(normalize_extra_args(None).is_none());
    }
}
