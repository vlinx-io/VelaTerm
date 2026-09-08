//! What settings the agent accepts, asked of the agent itself.
//!
//! `/config` with no arguments prints its own usage: one line per setting, as `key=value|value` for a fixed
//! choice or `key=<value>` for free text. That is a description of a form, so the composer's completion is
//! built from it rather than from a list written here — a list that would go stale the first time the agent
//! gained a setting.
//!
//! Two things make this cheap. The command is answered by the CLI locally, costing no tokens and no model
//! call, and the answer depends only on the installed binary, so one lookup serves every session.
//!
//! What it does not give is the value each setting currently holds; the CLI has no way to read one back.
//! So this drives completion — choosing a value — and not a settings panel, which would have to show a
//! switch without knowing which way it is set.

use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// One setting the agent accepts.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigKey {
    pub key: String,
    /// Allowed values. Empty when the setting takes free text.
    pub values: Vec<String>,
}

/// How long to wait for the agent to describe itself before giving up. Completion is a convenience; a slow
/// or wedged binary must never hold up starting a conversation.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(20);

/// Cache keyed by the agent executable, since the answer changes only when that binary does.
static CACHE: Mutex<Option<HashMap<String, Vec<ConfigKey>>>> = Mutex::new(None);

/// Ask the agent which settings it accepts, or return the previous answer for this binary.
///
/// Runs a throwaway process rather than asking on the live conversation, which would leave a wall of usage
/// text sitting in the middle of the user's transcript.
pub fn lookup(bin: &str, cwd: Option<&str>) -> Vec<ConfigKey> {
    if let Some(hit) = CACHE
        .lock()
        .ok()
        .and_then(|c| c.as_ref().and_then(|m| m.get(bin).cloned()))
    {
        return hit;
    }
    let keys = ask(bin, cwd).unwrap_or_default();
    if !keys.is_empty() {
        if let Ok(mut cache) = CACHE.lock() {
            cache
                .get_or_insert_with(HashMap::new)
                .insert(bin.to_string(), keys.clone());
        }
    }
    keys
}

fn ask(bin: &str, cwd: Option<&str>) -> Option<Vec<ConfigKey>> {
    let mut cmd = Command::new(bin);
    cmd.args(["-p", "--output-format", "text", "/config"]);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    for key in crate::pty::manager::AGENT_HARNESS_MARKERS {
        cmd.env_remove(key);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = cmd.spawn().ok()?;
    let deadline = Instant::now() + LOOKUP_TIMEOUT;
    // Poll rather than block: a binary that never answers must not strand this thread.
    let output = loop {
        match child.try_wait() {
            Ok(Some(_)) => break child.wait_with_output().ok()?,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100))
            }
            _ => {
                let _ = child.kill();
                return None;
            }
        }
    };
    Some(parse(&String::from_utf8_lossy(&output.stdout)))
}

/// Read the usage text into settings.
///
/// Lines that are not `key=…` — the heading, blank lines, anything else the agent prints — are skipped, so
/// a reworded preamble cannot break this.
pub fn parse(text: &str) -> Vec<ConfigKey> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some((key, rest)) = line.split_once('=') else {
            continue;
        };
        // A setting name, not prose: the key is a bare identifier with no spaces in it.
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            continue;
        }
        // `<value>` means free text rather than a choice.
        let values = if rest.starts_with('<') {
            Vec::new()
        } else {
            rest.split('|')
                .map(str::trim)
                .filter(|v| !v.is_empty() && !v.contains(' '))
                .map(str::to_string)
                .collect()
        };
        out.push(ConfigKey {
            key: key.to_string(),
            values,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const USAGE: &str = concat!(
        "Usage: /config key=value [key=value ...]\n",
        "  autoCompact=true|false\n",
        "  editor=normal|vim\n",
        "  language=<value>\n",
        "  outputStyle=default|Concise|Explanatory\n",
        "\n",
        "Run /config to see what's available.\n"
    );

    /// A choice becomes a list of values; free text becomes an empty list; prose is skipped.
    #[test]
    fn parses_choices_and_free_text() {
        let keys = parse(USAGE);
        assert_eq!(
            keys.iter().map(|k| k.key.as_str()).collect::<Vec<_>>(),
            ["autoCompact", "editor", "language", "outputStyle"]
        );
        assert_eq!(keys[0].values, ["true", "false"]);
        assert!(keys[2].values.is_empty(), "free text has no choices");
        assert_eq!(keys[3].values, ["default", "Concise", "Explanatory"]);
    }

    /// The heading contains an `=` too, and must not be mistaken for a setting.
    #[test]
    fn skips_the_usage_heading_and_anything_unparseable() {
        assert!(parse("Usage: /config key=value [key=value ...]").is_empty());
        assert!(parse("Expected key=value, got \"autoCompact\".").is_empty());
        assert!(parse("").is_empty());
    }
}
