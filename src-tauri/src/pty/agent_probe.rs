//! Detect an agent the user started by hand inside a plain terminal session.
//!
//! The old detector forked `ps` every second for every terminal and walked the whole process table, which
//! is why it was removed. This one asks the kernel which process group owns the terminal's foreground —
//! one `tcgetpgrp` per check — and reads a command line only when that group changes. While the shell
//! itself sits in the foreground, which is almost all of the time, nothing else is done.
//!
//! Only Unix has a foreground process group; Windows terminals report no agent.

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Executable names of the agents with a terminal interface, mapped to their session kind strings.
const AGENT_BINS: &[(&str, &str)] = &[
    ("claude", "claude"),
    ("codex", "codex"),
    ("opencode", "opencode"),
    ("copilot", "copilot"),
    ("cursor-agent", "cursor"),
    ("cline", "cline"),
    ("pi", "pi"),
    ("omp", "omp"),
    ("agy", "antigravity"),
    ("crush", "crush"),
    ("kimi", "kimi"),
    ("kiro-cli", "kiro"),
    ("grok", "grok"),
    ("roo", "zoo"),
];

/// Script runtimes whose first argument is the program actually being run, as with a `#!/usr/bin/env node`
/// launcher installed by npm.
const RUNTIMES: &[&str] = &["node", "bun", "deno", "python", "python3"];

/// The agent kind a command line names, if any.
pub fn classify(argv: &[String], exe: Option<&str>) -> Option<&'static str> {
    let base = |s: &str| {
        let name = s.rsplit(['/', '\\']).next().unwrap_or(s);
        name.strip_suffix(".exe").unwrap_or(name).to_string()
    };
    let lookup = |name: &str| AGENT_BINS.iter().find(|(bin, _)| *bin == name).map(|(_, kind)| *kind);
    let first = argv.first().map(|s| base(s)).or_else(|| exe.map(base))?;
    if let Some(kind) = lookup(&first) {
        return Some(kind);
    }
    let runtime = first.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    if RUNTIMES.contains(&first.as_str()) || RUNTIMES.contains(&runtime) {
        // Skip runtime flags such as `--no-warnings` to reach the script path.
        let script = argv.iter().skip(1).find(|a| !a.starts_with('-'))?;
        let name = base(script);
        let name = name.strip_suffix(".js").or_else(|| name.strip_suffix(".mjs")).unwrap_or(&name);
        return lookup(name);
    }
    None
}

/// The agent running as the leader of process group `pgid`.
pub fn agent_of(pgid: u32) -> Option<&'static str> {
    let pid = Pid::from_u32(pgid);
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        false,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always).with_exe(UpdateKind::Always),
    );
    let process = sys.process(pid)?;
    let argv: Vec<String> = process.cmd().iter().map(|a| a.to_string_lossy().into_owned()).collect();
    let exe = process.exe().and_then(|p| p.to_str());
    classify(&argv, exe).or_else(|| classify(&[], process.name().to_str()))
}

#[cfg(test)]
mod tests {
    use super::classify;

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn native_binaries_match_by_name() {
        assert_eq!(classify(&argv(&["claude"]), None), Some("claude"));
        assert_eq!(classify(&argv(&["/Users/u/.local/bin/claude", "--resume"]), None), Some("claude"));
        assert_eq!(classify(&argv(&["cursor-agent"]), None), Some("cursor"));
        assert_eq!(classify(&argv(&["C:\\bin\\codex.exe"]), None), Some("codex"));
    }

    #[test]
    fn npm_launchers_match_by_script() {
        assert_eq!(classify(&argv(&["node", "/opt/homebrew/bin/codex"]), None), Some("codex"));
        assert_eq!(classify(&argv(&["node", "--no-warnings", "/usr/local/bin/claude"]), None), Some("claude"));
        assert_eq!(classify(&argv(&["node22", "/x/opencode.js"]), None), Some("opencode"));
    }

    #[test]
    fn other_programs_do_not_match() {
        assert_eq!(classify(&argv(&["vim", "claude"]), None), None);
        assert_eq!(classify(&argv(&["node", "server.js"]), None), None);
        assert_eq!(classify(&[], None), None);
        assert_eq!(classify(&[], Some("/usr/bin/claude")), Some("claude"));
    }
}
