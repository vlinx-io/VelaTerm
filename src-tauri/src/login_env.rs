//! Load current platform and shell configuration for every new session process.
//!
//! Keep the environment inherited when VelaTerm started as the shell's baseline, before initial GUI
//! hydration imports user exports. Re-running startup files against that baseline updates new,
//! changed, and removed exports without feeding a stale hydrated value back into the next shell.
//! Each child receives its own snapshot; session identifiers and other runtime overrides are applied
//! afterwards. Session creation never changes the environment of the running application.

use std::ffi::{OsStr, OsString};
use std::sync::OnceLock;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;

type Entries = Vec<(OsString, OsString)>;
static BASE: OnceLock<Entries> = OnceLock::new();
#[cfg(windows)]
static INITIAL_SYSTEM: OnceLock<Entries> = OnceLock::new();
#[cfg(unix)]
const BEGIN: &str = "__VLX_ENV_BEGIN__";
#[cfg(unix)]
const END: &str = "__VLX_ENV_END__";
pub(crate) const CAPTURE_LIMIT: usize = 1024 * 1024;

fn baseline() -> &'static Entries {
    BASE.get_or_init(|| {
        let mut entries: Entries = std::env::vars_os().collect();
        let mut cleanup = std::process::Command::new("");
        crate::appimage::scrub_command(&mut cleanup);
        for (key, value) in cleanup.get_envs() {
            entries.retain(|(k, _)| !same_key(k, key));
            if let Some(value) = value { entries.push((key.into(), value.into())); }
        }
        entries
    })
}

/// Remember the launch baseline on every platform; GUI launches also hydrate Unix exports once.
pub fn hydrate() {
    let _ = baseline();
    #[cfg(windows)]
    let _ = INITIAL_SYSTEM.get_or_init(|| windows_environment().unwrap_or_default());
    #[cfg(unix)]
    {
        if std::env::var_os("TERM").is_some() { return; }
        let shell = crate::agent::install::probe_shell();
        let script = environment_capture(BEGIN, END);
        let entries = crate::agent::executable::shell_probe_output_limited(&shell, "-lic", &script, CAPTURE_LIMIT)
            .and_then(|out| captured_environment(&out, BEGIN, END));
        let Some(entries) = entries else {
            crate::diagnostic_warn!("failed to read the login shell environment (keeping the inherited one)");
            return;
        };
        for (key, value) in entries {
            if skip(&key) { continue; }
            if key == OsStr::new("PATH") && !valid_path(&value) { continue; }
            std::env::set_var(&key, &value);
        }
    }
}

fn same_key(left: &OsStr, right: &OsStr) -> bool {
    if cfg!(windows) { left.to_string_lossy().eq_ignore_ascii_case(&right.to_string_lossy()) }
    else { left == right }
}

/// Bookkeeping belongs to the actual session, never to the temporary shell used to read configuration.
fn skip(key: &OsStr) -> bool {
    let key = key.to_string_lossy();
    let normalized = if cfg!(windows) { key.to_ascii_uppercase() } else { key.into_owned() };
    normalized.starts_with("VLX_") || matches!(normalized.as_str(),
        "PWD" | "OLDPWD" | "SHLVL" | "_" | "TERM" | "TERM_PROGRAM" | "TERM_PROGRAM_VERSION"
            | "COLORTERM" | "COLORFGBG")
}

#[cfg(unix)]
fn valid_path(value: &OsStr) -> bool {
    let value = value.to_string_lossy();
    !value.is_empty() && value.split(':').any(|part| part == "/usr/bin")
}

fn value<'a>(entries: &'a Entries, key: &OsStr) -> Option<&'a OsStr> {
    entries.iter().find(|(k, _)| same_key(k, key)).map(|(_, v)| v.as_os_str())
}

#[derive(Clone)]
pub(crate) struct SessionEnvironment { entries: Entries, shell_baseline: Entries }

impl SessionEnvironment {
    fn from_entries(mut entries: Entries) -> Self {
        entries.retain(|(key, _)| !skip(key));
        entries.extend(std::env::vars_os().filter(|(key, _)| skip(key)));
        let mut shell_baseline = baseline().clone();
        shell_baseline.retain(|(key, _)| !skip(key));
        shell_baseline.extend(std::env::vars_os().filter(|(key, _)| skip(key)));
        Self { entries, shell_baseline }
    }

    pub(crate) fn path(&self) -> Option<OsString> {
        value(&self.entries, OsStr::new("PATH")).map(OsStr::to_os_string)
    }

    pub(crate) fn apply_command(&self, command: &mut std::process::Command) {
        // Preserve explicit provider/session overrides, excluding the inherited AppImage cleanup that
        // host::command already applied. The snapshot contains its own freshly scrubbed configuration.
        let mut cleanup = std::process::Command::new("");
        crate::appimage::scrub_command(&mut cleanup);
        let overrides = command.get_envs().filter(|(key, val)|
            !cleanup.get_envs().any(|(k, v)| same_key(k, key) && v == *val))
            .map(|(key, val)| (key.to_os_string(), val.map(OsStr::to_os_string))).collect::<Vec<_>>();
        command.env_clear().envs(self.entries.iter().map(|(k, v)| (k, v)));
        for (key, val) in overrides {
            match val { Some(val) => { command.env(key, val); }, None => { command.env_remove(key); } }
        }
    }

    pub(crate) fn apply_pty(&self, command: &mut portable_pty::CommandBuilder) {
        // The real terminal shell loads its profiles itself. Seed it from the original launch baseline
        // so composed exports (for example FOO="$FOO:suffix") are evaluated once in the actual session.
        let overrides = command.iter_extra_env_as_str().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>();
        command.env_clear();
        for (key, val) in &self.shell_baseline { command.env(key, val); }
        if let Some(path) = self.path() { command.env("PATH", path); }
        for (key, val) in overrides { command.env(key, val); }
    }
}

/// The launch baseline with nothing read from startup files. A real terminal shell loads its own profiles, so
/// starting it from this snapshot gives the same result as a fresh probe without running a hidden shell first.
#[cfg(unix)]
pub(crate) fn launch_baseline() -> SessionEnvironment {
    SessionEnvironment::from_entries(Vec::new())
}

#[cfg(test)]
fn latest_environment(shell: Option<&str>) -> Option<SessionEnvironment> {
    latest_environment_in(shell, None)
}

pub(crate) fn latest_environment_in(shell: Option<&str>, cwd: Option<&std::path::Path>) -> Option<SessionEnvironment> {
    latest_environment_with_policy(shell, cwd, true)
}

#[cfg(windows)]
pub(crate) fn latest_terminal_environment(shell: &str, cwd: Option<&std::path::Path>, agent: bool) -> Option<SessionEnvironment> {
    latest_environment_with_policy(Some(shell), cwd, agent)
}

fn latest_environment_with_policy(shell: Option<&str>, cwd: Option<&std::path::Path>, agent: bool) -> Option<SessionEnvironment> {
    let _ = agent;
    #[cfg(unix)]
    {
        let shell = shell.filter(|s| !s.trim().is_empty()).map(str::to_string)
            .unwrap_or_else(crate::agent::install::probe_shell);
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        let begin = format!("VLX_ENV_{nonce}");
        let end = format!("VLX_ENV_END_{nonce}");
        let script = environment_capture(&begin, &end);
        let entries = crate::agent::executable::shell_probe_modes(&shell).iter().filter_map(|mode| {
            let output = crate::agent::executable::shell_probe_output_in(&shell, mode, &script, CAPTURE_LIMIT, cwd)?;
            captured_environment(&output, &begin, &end)
        }).collect::<Vec<_>>();
        combine_environments(&entries)
    }
    #[cfg(windows)]
    {
        let initial = INITIAL_SYSTEM.get_or_init(|| windows_environment().unwrap_or_default());
        let current = windows_environment()?;
        let entries = merge_system_environment(baseline(), initial, &current);
        // Windows uses the rebuilt system settings only; no hidden shell is started to read profiles.
        let _ = (shell, cwd);
        let mut environment = SessionEnvironment::from_entries(entries);
        environment.shell_baseline = environment.entries.clone();
        Some(environment)
    }
}

/// Temporary agent/catalogue processes have no session identity, but need the same fresh exports.
pub(crate) fn refresh_command(command: &mut std::process::Command) {
    if let Some(environment) = latest_environment_in(None, command.get_current_dir()) { environment.apply_command(command); }
}

#[cfg(windows)]
pub(crate) fn latest_path() -> Option<OsString> {
    // Executable discovery needs the persisted PATH, without starting another shell.
    windows_environment().and_then(|entries| value(&entries, OsStr::new("PATH")).map(OsStr::to_os_string))
}

#[cfg(test)]
fn latest_path_for(shell: Option<&str>) -> Option<OsString> {
    latest_environment(shell).and_then(|env| env.path())
}

#[cfg(unix)]
pub(crate) fn probe_environment(command: &mut std::process::Command) {
    command.env_clear().envs(baseline().iter().map(|(k, v)| (k, v)));
}

#[cfg(unix)]
pub(crate) fn environment_capture(begin: &str, end: &str) -> String {
    format!("printf '{begin}'; /usr/bin/env -0; printf '{end}'")
}

#[cfg(unix)]
pub(crate) fn captured_environment(stdout: &[u8], begin: &str, end: &str) -> Option<Entries> {
    let dump = capture_body(stdout, begin, end)?;
    if dump.is_empty() || !dump.ends_with(&[0]) { return None; }
    let mut entries = Vec::new();
    for item in dump[..dump.len() - 1].split(|byte| *byte == 0) {
        let eq = item.iter().position(|byte| *byte == b'=')?;
        if eq == 0 { return None; }
        entries.push((OsString::from_vec(item[..eq].to_vec()), OsString::from_vec(item[eq + 1..].to_vec())));
    }
    Some(entries)
}

#[cfg(unix)]
fn capture_body<'a>(stdout: &'a [u8], begin: &str, end: &str) -> Option<&'a [u8]> {
    let start = stdout.windows(begin.len()).position(|part| part == begin.as_bytes())? + begin.len();
    let rest = &stdout[start..];
    let stop = rest.windows(end.len()).position(|part| part == end.as_bytes())?;
    Some(&rest[..stop])
}

#[cfg(all(unix, test))]
fn parse_dump(stdout: &[u8]) -> Option<Entries> { captured_environment(stdout, BEGIN, END) }

/// Login configuration wins; Bash's interactive configuration supplies changes the login shell omitted.
#[cfg(unix)]
pub(crate) fn combine_environments(snapshots: &[Entries]) -> Option<SessionEnvironment> {
    let mut entries = snapshots.first()?.clone();
    for extra in snapshots.iter().skip(1) {
        let mut keys = extra.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
        keys.extend(baseline().iter().map(|(k, _)| k.clone()));
        for key in keys {
            if key == OsStr::new("PATH") || skip(&key) { continue; }
            let old = value(baseline(), &key);
            let fresh = value(extra, &key);
            if fresh == old || value(&entries, &key) != old { continue; }
            entries.retain(|(k, _)| k != &key);
            if let Some(fresh) = fresh { entries.push((key, fresh.into())); }
        }
    }
    let paths = snapshots.iter().filter_map(|e| value(e, OsStr::new("PATH")).map(OsStr::to_os_string)).collect::<Vec<_>>();
    if let Some(path) = combine_paths(&paths) {
        entries.retain(|(k, _)| k != OsStr::new("PATH"));
        entries.push(("PATH".into(), path));
    }
    Some(SessionEnvironment::from_entries(entries))
}

#[cfg(unix)]
pub(crate) fn combine_paths(values: &[OsString]) -> Option<OsString> {
    let mut paths = Vec::new();
    for value in values {
        for path in std::env::split_paths(value) {
            if !paths.contains(&path) { paths.push(path); }
        }
    }
    if paths.is_empty() { return None; }
    std::env::join_paths(paths).ok()
}

/// Preserve launch-only variables, while deleted system/user settings disappear from new children.
#[cfg(any(windows, test))]
fn merge_system_environment(inherited: &Entries, initial: &Entries, current: &Entries) -> Entries {
    let mut entries = inherited.iter().filter(|(key, _)|
        !initial.iter().any(|(k, _)| k.to_string_lossy().eq_ignore_ascii_case(&key.to_string_lossy())))
        .cloned().collect::<Entries>();
    for (key, val) in current {
        entries.retain(|(k, _)| !k.to_string_lossy().eq_ignore_ascii_case(&key.to_string_lossy()));
        entries.push((key.clone(), val.clone()));
    }
    entries
}

#[cfg(windows)]
fn windows_environment() -> Option<Entries> {
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{TOKEN_DUPLICATE, TOKEN_QUERY};
    use windows::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY | TOKEN_DUPLICATE, &mut token).ok()?;
        let mut block = std::ptr::null_mut();
        let created = CreateEnvironmentBlock(&mut block, Some(token), false);
        let _ = CloseHandle(token);
        created.ok()?;
        let wide = block.cast::<u16>();
        let mut len = 0;
        while !(*wide.add(len) == 0 && *wide.add(len + 1) == 0) { len += 1; }
        let entries = windows_entries(std::slice::from_raw_parts(wide, len + 2)).into_iter()
            .map(|(k, v)| (OsString::from_wide(k), OsString::from_wide(v))).collect();
        let _ = DestroyEnvironmentBlock(block);
        Some(entries)
    }
}

#[cfg(any(windows, test))]
fn windows_entries(block: &[u16]) -> Vec<(&[u16], &[u16])> {
    block.split(|ch| *ch == 0).filter_map(|entry| {
        // Windows' =C: drive bookkeeping is not a valid key for Command::env.
        let eq = entry.iter().position(|ch| *ch == b'=' as u16)?;
        if eq == 0 { return None; }
        Some((&entry[..eq], &entry[eq + 1..]))
    }).collect()
}

#[cfg(test)]
fn windows_path(block: &[u16]) -> Option<&[u16]> {
    windows_entries(block).into_iter().find(|(key, val)|
        !val.is_empty() && String::from_utf16_lossy(key).eq_ignore_ascii_case("PATH")).map(|(_, val)| val)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_environment_path_is_case_insensitive_and_keeps_unicode() {
        let expected = r"C:\Windows\system32;C:\Users\user\新安装工具";
        let block = format!("=C:=C:\\folder\0TEMP=C:\\Temp\0Path={expected}\0OTHER=x\0\0")
            .encode_utf16().collect::<Vec<_>>();
        assert_eq!(String::from_utf16(windows_path(&block).unwrap()).unwrap(), expected);
        let empty = "PATH=\0\0".encode_utf16().collect::<Vec<_>>();
        assert_eq!(windows_path(&empty), None);
        let absent = "PATHEXT=.EXE;.CMD\0\0".encode_utf16().collect::<Vec<_>>();
        assert_eq!(windows_path(&absent), None);
    }

    #[test]
    fn windows_settings_refresh_added_changed_and_deleted_variables() {
        let entries = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| ((*k).into(), (*v).into())).collect::<Entries>();
        let inherited = entries(&[("PATH", "old"), ("API_KEY", "old-key"), ("HTTPS_PROXY", "old-proxy"), ("LAUNCH_ONLY", "keep")]);
        let initial = entries(&[("Path", "old"), ("API_KEY", "old-key"), ("HTTPS_PROXY", "old-proxy")]);
        let current = entries(&[("Path", "new"), ("api_key", "new-key"), ("NEW_SETTING", "新值\nsecond=part"), ("EMPTY", "")]);
        let refreshed = merge_system_environment(&inherited, &initial, &current);
        assert!(!refreshed.iter().any(|(k, _)| k == "HTTPS_PROXY" || k == "PATH" || k == "API_KEY"));
        for pair in current { assert!(refreshed.contains(&pair)); }
        assert!(refreshed.contains(&("LAUNCH_ONLY".into(), "keep".into())));
        let wide = "=C:=C:\\folder\0API_KEY=新值\0EMPTY=\0MULTI=one\ntwo=three\0\0".encode_utf16().collect::<Vec<_>>();
        let decoded = windows_entries(&wide).into_iter().map(|(k, v)| (String::from_utf16(k).unwrap(), String::from_utf16(v).unwrap())).collect::<Vec<_>>();
        assert_eq!(decoded, vec![("API_KEY".into(), "新值".into()), ("EMPTY".into(), "".into()), ("MULTI".into(), "one\ntwo=three".into())]);
    }

    #[cfg(unix)]
    #[test]
    fn deleted_exports_do_not_return_from_the_apps_initial_hydration() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("vlx-hydrated-environment-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let shell = root.join("bash");
        std::fs::write(&shell, format!("#!/bin/sh\nexport HOME='{}'\nexec /bin/bash \"$@\"\n", root.display())).unwrap();
        std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(root.join(".bash_profile"), "export PATH=/usr/bin:/bin\nsource \"$HOME/.bashrc\"\n").unwrap();
        std::fs::write(root.join(".bashrc"), "export FIXTURE_HYDRATED_KEY=first\nexport FIXTURE_HYDRATED_REMOVED=old\n").unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "login_env::tests::hydration_process_helper", "--ignored", "--nocapture"])
            .env("SHELL", &shell).env("HOME", &root).env("VLX_ENV_TEST_HOME", &root)
            .env_remove("TERM").env_remove("FIXTURE_HYDRATED_KEY").env_remove("FIXTURE_HYDRATED_REMOVED")
            .output().unwrap();
        assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        assert!(String::from_utf8_lossy(&output.stdout).contains("VLX_ENV_FIXTURE_VALIDATED"), "the isolated hydration assertions did not run");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "invoked in an isolated process by the hydration regression"]
    fn hydration_process_helper() {
        let home = std::path::PathBuf::from(std::env::var_os("VLX_ENV_TEST_HOME").unwrap());
        hydrate();
        assert_eq!(std::env::var("FIXTURE_HYDRATED_KEY").unwrap(), "first");
        assert_eq!(std::env::var("FIXTURE_HYDRATED_REMOVED").unwrap(), "old");
        std::fs::write(home.join(".bashrc"), "export FIXTURE_HYDRATED_KEY=second\n").unwrap();
        let environment = latest_environment(None).unwrap();
        let mut command = std::process::Command::new("/usr/bin/env");
        environment.apply_command(&mut command);
        let output = command.arg("-0").output().unwrap();
        assert!(output.status.success());
        let begin = format!("VLX_TEST_ENV_{}", uuid::Uuid::new_v4().simple());
        let end = format!("VLX_TEST_END_{}", uuid::Uuid::new_v4().simple());
        let framed = [begin.as_bytes(), &output.stdout, end.as_bytes()].concat();
        let entries = captured_environment(&framed, &begin, &end).unwrap();
        assert_eq!(value(&entries, OsStr::new("FIXTURE_HYDRATED_KEY")), Some(OsStr::new("second")));
        assert!(value(&entries, OsStr::new("FIXTURE_HYDRATED_REMOVED")).is_none());
        assert_eq!(std::env::var("FIXTURE_HYDRATED_KEY").unwrap(), "first");
        assert_eq!(std::env::var("FIXTURE_HYDRATED_REMOVED").unwrap(), "old");
        println!("VLX_ENV_FIXTURE_VALIDATED");
    }

    #[cfg(unix)]
    #[test]
    fn new_sessions_reload_full_shell_exports_and_preserve_runtime_overrides() {
        use std::os::unix::fs::PermissionsExt;
        let inherited = std::env::vars_os().collect::<Entries>();
        let root = std::env::temp_dir().join(format!("vlx-full-environment-{}", uuid::Uuid::new_v4()));
        for name in ["bash", "zsh"] {
            let native = format!("/bin/{name}");
            if !std::path::Path::new(&native).exists() { continue; }
            let home = root.join(name);
            std::fs::create_dir_all(&home).unwrap();
            let shell = home.join(name);
            std::fs::write(&shell, format!("#!/bin/sh\nexec /usr/bin/env -i HOME='{}' PATH=/usr/bin:/bin FIXTURE_COMPOSE=base {native} \"$@\"\n", home.display())).unwrap();
            std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).unwrap();
            let profile = if name == "bash" { ".bash_profile" } else { ".zprofile" };
            std::fs::write(home.join(profile), if name == "bash" { "export PATH=/usr/bin:/bin\nsource \"$HOME/.bashrc\"\n" } else { "export PATH=/usr/bin:/bin\n" }).unwrap();
            let rc = home.join(if name == "bash" { ".bashrc" } else { ".zshrc" });
            std::fs::write(home.join("profile.env"), "export FIXTURE_LOCAL_KEY=project-key\n").unwrap();
            std::fs::write(&rc, "export FIXTURE_COMPOSE=\"${FIXTURE_COMPOSE}:suffix\"\nexport FIXTURE_API_KEY=first\nexport HTTPS_PROXY=http://fixture.invalid:12345\nexport FIXTURE_REMOVED=old\nexport FIXTURE_MULTI='first\nsecond=value'\nexport FIXTURE_EMPTY=\nexport VLX_TOKEN=wrong-profile-token\nsource ./profile.env\n").unwrap();
            let first = latest_environment_in(shell.to_str(), Some(&home)).unwrap();
            let run = |environment: &SessionEnvironment| {
                let mut command = std::process::Command::new("/usr/bin/env");
                command.arg("-0").env("VLX_TOKEN", "session-token").env("PROVIDER_PASSWORD", "session-password");
                environment.apply_command(&mut command);
                let output = command.output().unwrap();
                assert!(output.status.success());
                // Session variables such as VLX_PATH_APPEND reach the child, so a bare "END" marker can match inside them.
                let framed = [BEGIN.as_bytes(), &output.stdout, END.as_bytes()].concat();
                captured_environment(&framed, BEGIN, END).unwrap()
            };
            let first_child = run(&first);
            assert_eq!(value(&first_child, OsStr::new("FIXTURE_API_KEY")), Some(OsStr::new("first")));
            assert_eq!(value(&first_child, OsStr::new("FIXTURE_LOCAL_KEY")), Some(OsStr::new("project-key")));
            assert_eq!(value(&first_child, OsStr::new("FIXTURE_COMPOSE")), Some(OsStr::new("base:suffix")));
            assert_eq!(value(&first_child, OsStr::new("FIXTURE_MULTI")), Some(OsStr::new("first\nsecond=value")));
            assert_eq!(value(&first_child, OsStr::new("FIXTURE_EMPTY")), Some(OsStr::new("")));
            assert_eq!(value(&first_child, OsStr::new("VLX_TOKEN")), Some(OsStr::new("session-token")));
            assert_eq!(value(&first_child, OsStr::new("PROVIDER_PASSWORD")), Some(OsStr::new("session-password")));
            std::fs::write(&rc, "export FIXTURE_API_KEY=second\nexport FIXTURE_ADDED=新值\nexport FIXTURE_MULTI='changed\nnew=value'\nexport FIXTURE_EMPTY=\n").unwrap();
            let second = latest_environment(shell.to_str()).unwrap();
            let second_child = run(&second);
            assert_eq!(value(&second_child, OsStr::new("FIXTURE_API_KEY")), Some(OsStr::new("second")));
            assert_eq!(value(&second_child, OsStr::new("FIXTURE_ADDED")), Some(OsStr::new("新值")));
            assert_eq!(value(&second_child, OsStr::new("FIXTURE_MULTI")), Some(OsStr::new("changed\nnew=value")));
            assert!(value(&second_child, OsStr::new("FIXTURE_REMOVED")).is_none());
            assert!(value(&second_child, OsStr::new("HTTPS_PROXY")).is_none());
            // Seed the actual terminal with the baseline, not an already composed profile export.
            std::fs::write(&rc, "export FIXTURE_COMPOSE=\"${FIXTURE_COMPOSE}:suffix\"\n").unwrap();
            let environment = latest_environment(shell.to_str()).unwrap();
            let mut pty = portable_pty::CommandBuilder::new(&native);
            environment.apply_pty(&mut pty);
            pty.env("HOME", &home);
            pty.env("FIXTURE_COMPOSE", "base");
            let mut command = std::process::Command::new(&native);
            command.env_clear().envs(pty.iter_full_env_as_str()).args(["-lic", "printf '%s' \"$FIXTURE_COMPOSE\""]);
            let output = command.output().unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, b"base:suffix");
        }
        assert!(std::env::vars_os().collect::<Entries>() == inherited, "the application's environment was never mutated");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn new_child_sees_bashrc_path_updates_for_commands_outside_the_agent_catalog() {
        use std::os::unix::fs::PermissionsExt;
        let inherited = std::env::var_os("PATH");
        let root = std::env::temp_dir().join(format!("vlx-current-path-{}", uuid::Uuid::new_v4()));
        let home = root.join("home");
        let bin = root.join("new-tools/bin");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(home.join(".bash_profile"), "export PATH=/usr/bin:/bin\n").unwrap();
        let shell = root.join("bash");
        std::fs::write(&shell, format!("#!/bin/sh\nexec /usr/bin/env -i HOME='{}' PATH=/usr/bin:/bin /bin/bash \"$@\"\n", home.display())).unwrap();
        std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).unwrap();
        let before = latest_path_for(shell.to_str()).unwrap();
        assert!(!std::env::split_paths(&before).any(|path| path == bin));

        std::fs::write(home.join(".bashrc"), format!("export PATH='{}:/usr/bin:/bin'\n", bin.display())).unwrap();
        let tool = bin.join("vlx-path-fixture");
        std::fs::write(&tool, "#!/bin/sh\nprintf CURRENT_PATH_OK\n").unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        let current = latest_path_for(shell.to_str()).unwrap();
        let output = std::process::Command::new("vlx-path-fixture").env("PATH", &current).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"CURRENT_PATH_OK");
        assert_eq!(std::env::var_os("PATH"), inherited, "the application environment was never mutated");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn captured_environment_ignores_banners_and_keeps_non_utf8_bytes() {
        assert_eq!(captured_environment(b"noiseBEGINPATH=/usr/bin:/tmp/\xff\0ENDtail", "BEGIN", "END").unwrap()[0].1.clone().into_vec(),
            b"/usr/bin:/tmp/\xff");
        assert!(captured_environment(b"BEGINPATH=/usr/bin\0", "BEGIN", "END").is_none());
        assert!(captured_environment(b"BEGINEND", "BEGIN", "END").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn parses_entries_between_sentinels() {
        let bytes =
            b"profile noise\n__VLX_ENV_BEGIN__PATH=/usr/bin:/bin\0DEEPSEEK_API_KEY=sk-x\0__VLX_ENV_END__tail";
        let entries = parse_dump(bytes).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], (OsString::from("PATH"), OsString::from("/usr/bin:/bin")));
        assert_eq!(entries[1].1, OsStr::new("sk-x"));
    }

    #[cfg(unix)]
    #[test]
    fn keeps_multiline_values_and_equals_signs() {
        let entries = parse_dump(b"__VLX_ENV_BEGIN__TOKEN=a=b\nc\0__VLX_ENV_END__").unwrap();
        assert_eq!(entries, vec![(OsString::from("TOKEN"), OsString::from("a=b\nc"))]);
    }

    #[cfg(unix)]
    #[test]
    fn requires_both_sentinels() {
        assert!(parse_dump(b"PATH=/usr/bin").is_none());
        assert!(parse_dump(b"__VLX_ENV_BEGIN__PATH=/usr/bin").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn skips_shell_bookkeeping_and_terminal_markers() {
        assert!(skip(OsStr::new("PWD")));
        assert!(skip(OsStr::new("VLX_TOKEN")));
        assert!(skip(OsStr::new("TERM_PROGRAM")));
        assert!(!skip(OsStr::new("PATH")));
        assert!(!skip(OsStr::new("DEEPSEEK_API_KEY")));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_malformed_paths() {
        assert!(valid_path(OsStr::new("/usr/bin:/bin")));
        assert!(!valid_path(OsStr::new("")));
        assert!(!valid_path(OsStr::new("/usr/local/bin /opt/homebrew/bin")));
    }
}
