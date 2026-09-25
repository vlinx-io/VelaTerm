//! Recover the login shell's environment for launches that did not come from a shell.
//!
//! A macOS Dock/Finder launch (and a Linux desktop autostart) inherits only the launchd/desktop
//! environment: a minimal PATH and none of the variables a user exports in `.zprofile`, `.zshrc`, or
//! their equivalents. Session-view agents are spawned directly from this process, so every provider
//! that authenticates through environment variables — `DEEPSEEK_API_KEY`, `AWS_ACCESS_KEY_ID`, and the
//! like — disappears from their catalogues, while the same agent inside a terminal workspace sees it.
//!
//! Run `$SHELL -i -l -c` once, dump the resulting environment between sentinels with `env -0`, and
//! apply it to this process. Everything spawned afterwards (chat engines, model catalogues, git
//! probes) inherits the result. Nothing is persisted: the next launch takes a fresh copy, so edits to
//! shell startup files apply after a restart.
//!
//! Skip when `TERM` is set: a terminal launch already carries the shell environment. Windows does not
//! use this module: Explorer-launched processes receive the registry environment, user variables
//! included.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStringExt;
use std::time::Duration;

/// How long the login shell may take before the inherited environment is kept.
const TIMEOUT: Duration = Duration::from_secs(5);

const BEGIN: &str = "__VLX_ENV_BEGIN__";
const END: &str = "__VLX_ENV_END__";

/// Capture the login shell's environment and merge it into this process's own.
pub fn hydrate() {
    if std::env::var_os("TERM").is_some() {
        return;
    }

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let script = format!("printf '{BEGIN}'; env -0; printf '{END}'");

    // Startup files may contain interactive prompts such as `read` or `select`. Run the shell in a
    // worker with a timeout so such prompts cannot block application startup.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let out = probe_command(&shell).arg("-i").arg("-l").arg("-c").arg(&script).output();
        let _ = tx.send(out);
    });

    let Ok(Ok(out)) = rx.recv_timeout(TIMEOUT) else {
        crate::diagnostic_warn!("failed to read the login shell environment (keeping the inherited one)");
        return;
    };
    let Some(entries) = parse_dump(&out.stdout) else {
        crate::diagnostic_warn!("login shell printed no environment marker (keeping the inherited environment)");
        return;
    };
    if entries.is_empty() {
        crate::diagnostic_warn!("login shell reported an empty environment (keeping the inherited one)");
        return;
    }

    for (key, value) in entries {
        if skip(&key) {
            continue;
        }
        // The same guard as before: fish and other shells can render PATH as a space-separated array,
        // which would break every child that resolves executables.
        if key == OsStr::new("PATH") && !valid_path(&value) {
            crate::diagnostic_warn!("login shell PATH looks malformed (keeping the inherited PATH)");
            continue;
        }
        std::env::set_var(&key, &value);
    }
}

/// The login-shell probe without its arguments. It runs the user's startup files, and whatever they
/// launch may outlive the probe, so it gets the system environment (AppImage bundle directories would
/// otherwise ride along) and never the `--serve` access password. `run_serve` already removes the
/// password before calling hydrate; this is the second line of defence for any other caller order.
fn probe_command(shell: &str) -> std::process::Command {
    let mut probe = std::process::Command::new(shell);
    crate::appimage::scrub_command(&mut probe);
    for key in crate::SERVE_PASSWORD_ENV_KEYS {
        probe.env_remove(key);
    }
    probe
}

/// Variables that describe the probing shell session itself rather than the user's configuration, and
/// would mislead children or duplicate state this process already owns. `VLX_` belongs to whichever
/// session launched VelaTerm; each child gets its own via the spawn paths.
fn skip(key: &OsStr) -> bool {
    let key = key.to_string_lossy();
    key.starts_with("VLX_")
        || matches!(
            key.as_ref(),
            "PWD"
                | "OLDPWD"
                | "SHLVL"
                | "_"
                | "TERM"
                | "TERM_PROGRAM"
                | "TERM_PROGRAM_VERSION"
                | "COLORTERM"
                | "COLORFGBG"
        )
}

fn valid_path(value: &OsStr) -> bool {
    let value = value.to_string_lossy();
    !value.is_empty() && value.split(':').any(|part| part == "/usr/bin")
}

/// Extract `KEY=VALUE` entries from the NUL-separated dump between the sentinels. Values keep their
/// bytes untouched, so secrets are never reformatted through UTF-8.
fn parse_dump(stdout: &[u8]) -> Option<Vec<(OsString, OsString)>> {
    let start = stdout
        .windows(BEGIN.len())
        .position(|window| window == BEGIN.as_bytes())?
        + BEGIN.len();
    let rest = &stdout[start..];
    let end = rest
        .windows(END.len())
        .position(|window| window == END.as_bytes())?;

    let mut entries = Vec::new();
    for item in rest[..end].split(|byte| *byte == 0) {
        let Some(eq) = item.iter().position(|byte| *byte == b'=') else {
            continue;
        };
        if eq == 0 {
            continue;
        }
        entries.push((
            OsString::from_vec(item[..eq].to_vec()),
            OsString::from_vec(item[eq + 1..].to_vec()),
        ));
    }
    Some(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_between_sentinels() {
        let bytes =
            b"profile noise\n__VLX_ENV_BEGIN__PATH=/usr/bin:/bin\0DEEPSEEK_API_KEY=sk-x\0__VLX_ENV_END__tail";
        let entries = parse_dump(bytes).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], (OsString::from("PATH"), OsString::from("/usr/bin:/bin")));
        assert_eq!(entries[1].1, OsStr::new("sk-x"));
    }

    #[test]
    fn keeps_multiline_values_and_equals_signs() {
        let entries = parse_dump(b"__VLX_ENV_BEGIN__TOKEN=a=b\nc\0__VLX_ENV_END__").unwrap();
        assert_eq!(entries, vec![(OsString::from("TOKEN"), OsString::from("a=b\nc"))]);
    }

    #[test]
    fn requires_both_sentinels() {
        assert!(parse_dump(b"PATH=/usr/bin").is_none());
        assert!(parse_dump(b"__VLX_ENV_BEGIN__PATH=/usr/bin").is_none());
    }

    #[test]
    fn skips_shell_bookkeeping_and_terminal_markers() {
        assert!(skip(OsStr::new("PWD")));
        assert!(skip(OsStr::new("VLX_TOKEN")));
        assert!(skip(OsStr::new("TERM_PROGRAM")));
        assert!(!skip(OsStr::new("PATH")));
        assert!(!skip(OsStr::new("DEEPSEEK_API_KEY")));
    }

    #[test]
    fn probe_never_inherits_the_serve_password() {
        let _guard = crate::SERVE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved: Vec<_> = crate::SERVE_PASSWORD_ENV_KEYS
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect();
        for key in crate::SERVE_PASSWORD_ENV_KEYS {
            std::env::set_var(key, "probe-secret");
        }
        let out = probe_command("sh")
            .arg("-c")
            .arg("printf %s \"${VELA_SERVE_PASSWORD-unset}|${VLX_SERVE_PASSWORD-unset}\"")
            .output()
            .expect("sh must run");
        for (key, value) in saved {
            match value {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
        assert_eq!(String::from_utf8_lossy(&out.stdout), "unset|unset");
    }

    #[test]
    fn rejects_malformed_paths() {
        assert!(valid_path(OsStr::new("/usr/bin:/bin")));
        assert!(!valid_path(OsStr::new("")));
        assert!(!valid_path(OsStr::new("/usr/local/bin /opt/homebrew/bin")));
    }
}
