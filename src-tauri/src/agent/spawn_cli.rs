//! Installation and discovery of built-in command shims and their companion skills (`vspawn`,
//! `vspawn-tree`, `vopen`, `vrefer`, `vsearch`, `vask`, `vself`, `vflow`, `vtell`, and `vkb`).
//!
//! At startup VelaTerm installs thin shims under the application data `bin/` directory and prepends it
//! to each session shell's PATH, allowing `vspawn "task"`, `vopen <file>`, `vrefer <session>`, and
//! `vsearch <words>` with no separate setup.
//!
//! Each shim invokes a hidden main-program subcommand such as `$VLX_EXE --spawn` or `--view`, implemented
//! in `agent/cli_client.rs`. Unix uses a tiny `#!/bin/sh` wrapper and Windows a `.cmd` file, keeping all
//! platform logic in Rust and working on Windows without Bash.

use std::path::{Path, PathBuf};

/// Built-in `(command name, main-program subcommand arguments)` shims. `vspawn` creates a child session
/// without a worktree, `vspawn-tree` forces `--worktree`, `vopen` opens a document or browser tab,
/// `vrefer` reads another session's conversation, `vsearch` searches across every session, `vstat` reports which sessions are busy, and `vkb` queries code
/// and the knowledge base. `vtell` sends session messages and execution reports, and
/// `vrun` starts a long-running command and waits for it, so its completion reaches whoever started it.
///
/// Unique `v`-prefixed names avoid shadowing system commands such as Vim's `/usr/bin/view`, so simply
/// prepending the bin directory is sufficient without ZDOTDIR/path_helper reordering.
const SHIMS: &[(&str, &str)] = &[
    ("vspawn", "--spawn"),
    ("vspawn-tree", "--spawn --worktree"),
    ("vopen", "--view"),
    ("vrefer", "--refer"),
    ("vsearch", "--search"),
    ("vstat", "--stat"),
    ("vself", "--self"),
    ("vkb", "--knowledge"),
    ("vflow", "--flow"),
    ("vtell", "--tell"),
    ("vrun", "--run"),
];

#[cfg(feature = "gui")]
const VELA_SHIM_MARKER: &str = "VelaTerm managed vela command";

/// Fixed destination of the managed `vela` launcher on macOS, the same location VS Code uses for its
/// `code` command. Writing there requires administrator authorization, so only explicit user actions
/// install it.
#[cfg(all(feature = "gui", target_os = "macos"))]
const MACOS_CLI_PATH: &str = "/usr/local/bin/vela";

/// Visibility of the `vela` command in the user's shell. `conflict` means an earlier PATH entry contains
/// an unmanaged command with the same name, which the installer never overwrites.
#[cfg(feature = "gui")]
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserCliStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub conflict: Option<String>,
}

/// Compile-time embedded agent skill definitions installed on demand into Claude and Codex user skill
/// directories. They install as one bundle controlled by a single setting; add future skills here.
const SKILLS: &[(&str, &str)] = &[
    ("vspawn", include_str!("../../../skills/vspawn/SKILL.md")),
    (
        "vspawn-tree",
        include_str!("../../../skills/vspawn-tree/SKILL.md"),
    ),
    ("vopen", include_str!("../../../skills/vopen/SKILL.md")),
    ("vrefer", include_str!("../../../skills/vrefer/SKILL.md")),
    ("vsearch", include_str!("../../../skills/vsearch/SKILL.md")),
    ("vask", include_str!("../../../skills/vask/SKILL.md")),
    ("vstat", include_str!("../../../skills/vstat/SKILL.md")),
    ("vself", include_str!("../../../skills/vself/SKILL.md")),
    ("vflow", include_str!("../../../skills/vflow/SKILL.md")),
    ("vtell", include_str!("../../../skills/vtell/SKILL.md")),
    ("vkb", include_str!("../../../skills/vkb/SKILL.md")),
];

/// Bin directory prepended to session PATH: `<data_dir>/bin`.
pub fn bin_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("bin")
}

/// Install built-in shims under `<data_dir>/bin/`, making Unix files executable.
///
/// Rewrite on every startup to track application updates and return the bin path.
pub fn install(data_dir: &Path) -> std::io::Result<PathBuf> {
    let bin = bin_dir(data_dir);
    std::fs::create_dir_all(&bin)?;
    for (name, subcmd) in SHIMS {
        write_shim(&bin, name, subcmd)?;
    }
    // Retire both wrappers, including the extensionless Git Bash variant on Windows.
    for name in ["vknowledge", "vknowledge.cmd", "vorch", "vorch.cmd"] {
        match std::fs::remove_file(bin.join(name)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(bin)
}

/// Resolve the `vela` command seen by the current login shell in PATH order. Checking only our target
/// would miss an earlier same-name command that shadows a successful installation.
#[cfg(feature = "gui")]
pub fn user_cli_status() -> UserCliStatus {
    for dir in user_path_dirs() {
        for dest in command_paths(&dir) {
            if !dest.exists() {
                continue;
            }
            let shown = dest.to_string_lossy().into_owned();
            if is_managed_cli(&dest) {
                return UserCliStatus {
                    installed: true,
                    path: Some(shown),
                    conflict: None,
                };
            }
            return UserCliStatus {
                installed: false,
                path: None,
                conflict: Some(shown),
            };
        }
    }
    UserCliStatus {
        installed: false,
        path: None,
        conflict: None,
    }
}

/// Install `vela` into the current login shell's PATH. On macOS this is `/usr/local/bin/vela` written
/// with administrator authorization, matching VS Code's `code` command; Windows and Linux releases pick
/// the first writable user bin directory already in PATH and may run this at startup. Never modify
/// shell profiles or overwrite a same-name file without the application marker.
#[cfg(feature = "gui")]
pub fn install_user_cli() -> std::io::Result<UserCliStatus> {
    // 独立实验构建复用真实界面时，保留用户现有的全局命令与技能。
    if std::env::var("VELA_ISOLATED_RUNTIME").as_deref() == Ok("1") {
        return Ok(user_cli_status());
    }
    if cfg!(debug_assertions) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "install the packaged VelaTerm app before adding its shell command to PATH",
        ));
    }
    let before = user_cli_status();
    if before.installed {
        return Ok(before);
    }
    if let Some(conflict) = before.conflict.as_deref() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("another 'vela' command already exists at {conflict}"),
        ));
    }
    let exe = std::env::current_exe()?;

    #[cfg(target_os = "macos")]
    {
        return install_user_cli_macos(&exe);
    }

    #[cfg(not(target_os = "macos"))]
    {
        for dir in user_path_dirs() {
            if !is_safe_user_bin_dir(&dir) {
                continue;
            }
            if !dir.exists() && std::fs::create_dir_all(&dir).is_err() {
                continue;
            }
            if !dir.is_dir() {
                continue;
            }
            let dest = managed_cli_path(&dir);
            match write_user_cli_at(&dest, &exe) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(e),
                Err(_) => continue,
            }
            return Ok(UserCliStatus {
                installed: true,
                path: Some(dest.to_string_lossy().into_owned()),
                conflict: None,
            });
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "no writable bin directory from your PATH is available; add a user-writable bin directory to PATH and try again",
        ))
    }
}

/// Remove every VelaTerm-managed `vela` launcher in PATH without touching user-owned commands. The
/// macOS copy under `/usr/local/bin` needs administrator authorization; legacy user PATH shims do not.
#[cfg(feature = "gui")]
pub fn uninstall_user_cli() -> std::io::Result<UserCliStatus> {
    #[cfg(target_os = "macos")]
    {
        return uninstall_user_cli_macos();
    }

    #[cfg(not(target_os = "macos"))]
    {
        for dir in user_path_dirs() {
            let dest = managed_cli_path(&dir);
            remove_user_cli_at(&dest)?;
        }
        Ok(user_cli_status())
    }
}

/// Install the macOS launcher at `/usr/local/bin/vela`. The launcher is staged as the current user and
/// the authorized shell only copies it into place, so the authorization prompt never runs
/// user-controlled script content.
#[cfg(all(feature = "gui", target_os = "macos"))]
fn install_user_cli_macos(exe: &Path) -> std::io::Result<UserCliStatus> {
    let dest = Path::new(MACOS_CLI_PATH);
    let staged = std::env::temp_dir().join(format!("vlx-vela-{}", std::process::id()));
    if let Err(e) = write_user_cli_at(&staged, exe) {
        let _ = std::fs::remove_file(&staged);
        return Err(e);
    }
    let command = format!(
        "mkdir -p /usr/local/bin && rm -f {dst} && cp {src} {dst} && chmod 755 {dst}",
        src = shell_quote(&staged),
        dst = shell_quote(dest)
    );
    let authorized = run_admin_shell(&command);
    let _ = std::fs::remove_file(&staged);
    match authorized {
        Ok(()) => {}
        // A dismissed authorization prompt is not a failure; report the unchanged state.
        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => return Ok(user_cli_status()),
        Err(e) => return Err(e),
    }
    let status = user_cli_status();
    if status.installed {
        Ok(status)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "the 'vela' command could not be installed in /usr/local/bin",
        ))
    }
}

/// Remove the macOS launcher, plus any managed shims older releases left in user PATH directories.
/// `/usr/local/bin` is root-owned, so deleting the managed file there asks for authorization; a
/// user-owned `vela` at that path is never touched.
#[cfg(all(feature = "gui", target_os = "macos"))]
fn uninstall_user_cli_macos() -> std::io::Result<UserCliStatus> {
    let fixed_dir = Path::new("/usr/local/bin");
    for dir in user_path_dirs() {
        if dir.as_path() == fixed_dir {
            continue;
        }
        remove_user_cli_at(&managed_cli_path(&dir))?;
    }
    let dest = Path::new(MACOS_CLI_PATH);
    if is_managed_cli(dest) {
        match std::fs::remove_file(dest) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                let command = format!("rm -f {}", shell_quote(dest));
                match run_admin_shell(&command) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            }
            Err(e) => return Err(e),
        }
    }
    Ok(user_cli_status())
}

/// Execute `command` as root through `osascript`, the mechanism VS Code uses to install its `code`
/// command. A dismissed authorization prompt maps to `Interrupted` so callers can treat it as a no-op.
#[cfg(all(feature = "gui", target_os = "macos"))]
fn run_admin_shell(command: &str) -> std::io::Result<()> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(render_admin_script(command))
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if is_admin_cancel(&stderr) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "administrator authorization was cancelled",
        ));
    }
    crate::diagnostic_warn!("osascript administrator command failed: {}", stderr.trim());
    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "administrator authorization failed",
    ))
}

#[cfg(all(feature = "gui", target_os = "macos"))]
fn render_admin_script(command: &str) -> String {
    let escaped = command.replace('\\', "\\\\").replace('"', "\\\"");
    format!("do shell script \"{escaped}\" with administrator privileges")
}

/// AppleScript reports a dismissed authorization prompt as error -128. Match the numeric code because
/// the surrounding message is localized.
#[cfg(all(feature = "gui", target_os = "macos"))]
fn is_admin_cancel(stderr: &str) -> bool {
    stderr.contains("-128") || stderr.contains("User canceled") || stderr.contains("User cancelled")
}

/// Single-quote a path for the `/bin/sh` command executed by `do shell script`.
#[cfg(all(feature = "gui", target_os = "macos"))]
fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

#[cfg(feature = "gui")]
fn user_path_dirs() -> Vec<PathBuf> {
    let Some(path_env) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = std::env::split_paths(&path_env).collect();
    // macOS always checks `/usr/local/bin` first: that is where the managed command lives, and a
    // Finder-launched app inherits a minimal PATH without the login shell's additions.
    #[cfg(target_os = "macos")]
    {
        let preferred = PathBuf::from("/usr/local/bin");
        dirs.retain(|dir| dir != &preferred);
        dirs.insert(0, preferred);
    }
    dirs.dedup();
    dirs
}

#[cfg(feature = "gui")]
fn managed_cli_path(dir: &Path) -> PathBuf {
    dir.join(if cfg!(windows) { "vela.cmd" } else { "vela" })
}

#[cfg(feature = "gui")]
fn command_paths(dir: &Path) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        // Follow common PATHEXT precedence. Treat every user-owned variant as a conflict rather than
        // writing vela.cmd beside it and incorrectly reporting success.
        return ["vela.com", "vela.exe", "vela.bat", "vela.cmd"]
            .into_iter()
            .map(|name| dir.join(name))
            .collect();
    }
    #[cfg(not(windows))]
    {
        vec![dir.join("vela")]
    }
}

#[cfg(feature = "gui")]
fn is_managed_cli(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|s| s.contains(VELA_SHIM_MARKER))
        .unwrap_or(false)
}

#[cfg(feature = "gui")]
fn write_user_cli_at(dest: &Path, exe: &Path) -> std::io::Result<()> {
    if dest.exists() && !is_managed_cli(dest) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "another 'vela' command already exists at {}",
                dest.display()
            ),
        ));
    }
    std::fs::write(dest, render_user_cli(exe))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

#[cfg(feature = "gui")]
fn remove_user_cli_at(dest: &Path) -> std::io::Result<bool> {
    if !dest.exists() || !is_managed_cli(dest) {
        return Ok(false);
    }
    std::fs::remove_file(dest)?;
    Ok(true)
}

#[cfg(all(feature = "gui", not(target_os = "macos")))]
fn is_safe_user_bin_dir(dir: &Path) -> bool {
    // Allow only known user/package-manager bin directories, never arbitrary writable PATH entries such
    // as project node_modules, temporary directories, or system locations. The write still verifies access.
    #[cfg(unix)]
    if matches!(
        dir.to_str(),
        Some("/usr/local/bin" | "/opt/homebrew/bin" | "/opt/local/bin")
    ) {
        return true;
    }
    let Some(home) = crate::host::home_dir() else {
        return false;
    };
    if !dir.starts_with(home) {
        return false;
    }
    matches!(
        dir.file_name().and_then(|n| n.to_str()),
        Some("bin" | "npm" | "Scripts")
    )
}

#[cfg(feature = "gui")]
fn render_user_cli(exe: &Path) -> String {
    #[cfg(unix)]
    {
        let quoted = exe.to_string_lossy().replace('\'', "'\\''");
        return format!(
            "#!/bin/sh\n# {VELA_SHIM_MARKER}\ncase \"${{1:-}}\" in -h|--help) exec '{quoted}' --vela-help;; esac\nexec '{quoted}' --open-project \"$@\"\n"
        );
    }
    #[cfg(windows)]
    {
        format!(
            "@REM {VELA_SHIM_MARKER}\r\n@IF \"%~1\"==\"-h\" GOTO help\r\n@IF \"%~1\"==\"--help\" GOTO help\r\n@\"{}\" --open-project %*\r\n@EXIT /B %ERRORLEVEL%\r\n:help\r\n@\"{}\" --vela-help\r\n",
            exe.display(), exe.display()
        )
    }
}

/// Install one command shim that invokes `$VLX_EXE <subcmd> <user arguments>`. Session startup injects
/// `VLX_EXE` in `pty/manager.rs`.
///
/// - **Unix**: write `<name>` as an executable `#!/bin/sh` wrapper.
/// - **Windows**: write `<name>.cmd` for cmd/PowerShell PATHEXT lookup and an extensionless shell wrapper
///   for Git Bash, whose MSYS lookup adds `.exe` but not `.cmd`. This mirrors npm shipping both forms.
fn write_shim(bin: &Path, name: &str, subcmd: &str) -> std::io::Result<()> {
    let sh_shim = format!("#!/bin/sh\nexec \"$VLX_EXE\" {subcmd} \"$@\"\n");
    #[cfg(windows)]
    {
        std::fs::write(
            bin.join(format!("{name}.cmd")),
            format!("@\"%VLX_EXE%\" {subcmd} %*\r\n"),
        )?;
        // Extensionless LF/shebang wrapper for Git Bash; Windows needs no executable permission bit.
        std::fs::write(bin.join(name), sh_shim)?;
    }
    #[cfg(unix)]
    {
        let path = bin.join(name);
        std::fs::write(&path, sh_shim)?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn claude_skills_dir() -> std::io::Result<PathBuf> {
    let home = crate::host::home_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "user home directory not found",
        )
    })?;
    Ok(home.join(".claude").join("skills"))
}

fn codex_skills_dir() -> std::io::Result<PathBuf> {
    if let Some(codex_home) = std::env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(codex_home).join("skills"));
    }
    let home = crate::host::home_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "user home directory not found",
        )
    })?;
    Ok(home.join(".codex").join("skills"))
}

fn skill_dirs() -> std::io::Result<[PathBuf; 2]> {
    Ok([claude_skills_dir()?, codex_skills_dir()?])
}

fn sentinel_exists(dir: &Path) -> bool {
    let (sentinel, _) = SKILLS[0];
    dir.join(sentinel).join("SKILL.md").exists()
}

/// Claude and Codex share skill bodies, but three Claude frontmatter fields are outside the Codex schema.
/// Strip them for Codex, including `allowed-tools: Bash(...)`, which would restrict it to a nonexistent tool.
fn codex_skill_content(content: &str) -> String {
    let mut rendered = content
        .lines()
        .filter(|line| {
            !line.starts_with("argument-hint:")
                && !line.starts_with("disable-model-invocation:")
                && !line.starts_with("allowed-tools:")
        })
        .collect::<Vec<_>>()
        .join("\n");
    rendered.push('\n');
    rendered
}

fn write_skills(dir: &Path, for_codex: bool) -> std::io::Result<()> {
    for (name, content) in SKILLS {
        let dest = dir.join(name);
        std::fs::create_dir_all(&dest)?;
        if *name == "vspawn" {
            std::fs::create_dir_all(dest.join("references"))?;
            std::fs::write(dest.join("references/plan-execute.md"), include_str!("../../../skills/vspawn/references/plan-execute.md"))?;
        }
        if for_codex {
            std::fs::write(dest.join("SKILL.md"), codex_skill_content(content))?;
            if *name == "vkb" {
                let agents = dest.join("agents");
                std::fs::create_dir_all(&agents)?;
                std::fs::write(
                    agents.join("openai.yaml"),
                    include_str!("../../../skills/vkb/agents/openai.yaml"),
                )?;
            }
        } else {
            std::fs::write(dest.join("SKILL.md"), content)?;
        }
    }
    remove_retired_skills(dir)
}

/// Remove the former bundled skill on upgrades as well as explicit installation or removal.
fn remove_retired_skills(dir: &Path) -> std::io::Result<()> {
    for name in ["vknowledge", "vorch"] {
        match std::fs::remove_dir_all(dir.join(name)) {
            Ok(()) => {},
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Check whether bundled skills are installed in both user-level Claude and Codex directories. Use the
/// uniquely prefixed first `vspawn` skill as a bundle sentinel. Startup refresh fills skills added after
/// older partial installations without requiring the user to toggle the setting.
pub fn skills_installed() -> bool {
    let dirs = match skill_dirs() {
        Ok(dirs) => dirs,
        Err(_) => return false,
    };
    dirs.iter().all(|dir| sentinel_exists(dir))
}

/// Install every bundled SKILLS entry into Claude and Codex user-level skill directories.
pub fn install_skills() -> std::io::Result<()> {
    let [claude_dir, codex_dir] = skill_dirs()?;
    write_skills(&claude_dir, false)?;
    write_skills(&codex_dir, true)
}

/// Uninstall all bundled skills from Claude and Codex user-level skill directories.
pub fn uninstall_skills() -> std::io::Result<()> {
    for dir in skill_dirs()? {
        remove_retired_skills(&dir)?;
        for (name, _) in SKILLS {
            let dest = dir.join(name);
            if dest.exists() {
                std::fs::remove_dir_all(&dest)?;
            }
        }
    }
    Ok(())
}

/// Refresh installed skills at startup so contents follow application updates, like bin scripts. Leave
/// uninstalled skills untouched because the setting controls installation. Log failures without aborting.
pub fn refresh_installed_skills() {
    if std::env::var("VELA_ISOLATED_RUNTIME").as_deref() == Ok("1") { return; }
    if let Err(e) = skill_dirs().and_then(refresh_skills_in_dirs) {
        crate::diagnostic_warn!("failed to refresh installed agent skills (skill content may lag behind the current version): {e}");
    }
}

fn refresh_skills_in_dirs(dirs: [PathBuf; 2]) -> std::io::Result<()> {
    // Older releases wrote only ~/.claude/skills. A sentinel on either side means the user enabled the
    // bundle, so refresh also fills the other directory.
    let was_installed = dirs.iter().any(|dir| sentinel_exists(dir));
    // Retired skills must also disappear from partial or disabled installations.
    for dir in &dirs {
        remove_retired_skills(dir)?;
    }
    if was_installed {
        write_skills(&dirs[0], false)?;
        write_skills(&dirs[1], true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shim filename: bare on Unix and suffixed with `.cmd` on Windows.
    fn shim_path(bin: &Path, name: &str) -> PathBuf {
        #[cfg(windows)]
        {
            bin.join(format!("{name}.cmd"))
        }
        #[cfg(unix)]
        {
            bin.join(name)
        }
    }

    #[test]
    fn install_writes_shims() {
        let tmp = std::env::temp_dir().join(format!("vlx-spawn-cli-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let bin = install(&tmp).expect("writing out the shims should succeed");

        // Every shim invokes its matching `$VLX_EXE` subcommand and is executable on Unix.
        for (name, subcmd) in SHIMS {
            let path = shim_path(&bin, name);
            let written = std::fs::read_to_string(&path).expect("the shim should be readable again");
            assert!(written.contains("VLX_EXE"), "the {name} shim should delegate to VLX_EXE");
            assert!(written.contains(subcmd), "the {name} shim should carry the {subcmd} subcommand");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(&path).unwrap().permissions().mode();
                assert_eq!(mode & 0o111, 0o111, "{name} should be executable for user, group and other");
            }
            // Windows also installs an extensionless shell wrapper for bare-name Git Bash invocation.
            #[cfg(windows)]
            {
                let bash_shim =
                    std::fs::read_to_string(bin.join(name)).expect("Windows should also have an extensionless shim");
                assert!(
                    bash_shim.starts_with("#!/bin/sh"),
                    "the extensionless {name} shim should be an sh wrapper"
                );
                assert!(
                    bash_shim.contains(subcmd),
                    "the extensionless {name} shim should carry the {subcmd} subcommand"
                );
            }
        }

        // vspawn omits --worktree by default; vspawn-tree always includes it.
        let main = std::fs::read_to_string(shim_path(&bin, "vspawn")).unwrap();
        assert!(main.contains("--spawn"), "vspawn should delegate to --spawn");
        assert!(!main.contains("--worktree"), "vspawn should not carry --worktree by default");
        let tree = std::fs::read_to_string(shim_path(&bin, "vspawn-tree")).unwrap();
        assert!(
            tree.contains("--spawn --worktree"),
            "vspawn-tree should be a --spawn --worktree wrapper"
        );

        // vopen invokes the --view subcommand.
        let view = std::fs::read_to_string(shim_path(&bin, "vopen")).unwrap();
        assert!(view.contains("--view"), "vopen should delegate to --view");

        // bin_dir matches the path returned by install.
        assert_eq!(bin, bin_dir(&tmp));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn codex_skill_content_removes_claude_only_frontmatter() {
        let rendered = codex_skill_content(SKILLS[0].1);
        assert!(rendered.contains("name: vspawn"));
        assert!(rendered.contains("# /vspawn"));
        assert!(!rendered.contains("argument-hint:"));
        assert!(!rendered.contains("disable-model-invocation:"));
        assert!(!rendered.contains("allowed-tools:"));
    }

    #[cfg(unix)]
    #[test]
    fn knowledge_command_preserves_arguments() {
        use std::os::unix::fs::PermissionsExt;
        use std::process::Command;

        let tmp = std::env::temp_dir().join(format!("vlx-vkb-shims-{}", uuid::Uuid::new_v4()));
        let bin = install(&tmp).unwrap();
        let fake_exe = tmp.join("Vela Term");
        std::fs::write(&fake_exe, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        std::fs::set_permissions(&fake_exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let query = "中文 字体 $(echo unexpected) ; & \"quoted\"";
        let output = Command::new(bin.join("vkb"))
            .env("VLX_EXE", &fake_exe)
            .args(["memories", query])
            .output()
            .unwrap();
        assert!(output.status.success(), "vkb should execute successfully");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), format!("--knowledge\nmemories\n{query}\n"));
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn install_removes_retired_command_wrappers() {
        let tmp = std::env::temp_dir().join(format!("vlx-vkb-upgrade-{}", uuid::Uuid::new_v4()));
        let bin = bin_dir(&tmp);
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("vknowledge"), "#!/bin/sh\nexec \"$VLX_EXE\" --knowledge \"$@\"\n").unwrap();
        std::fs::write(bin.join("vknowledge.cmd"), "@\"%VLX_EXE%\" --knowledge %*\r\n").unwrap();
        for name in ["vorch", "vorch.cmd"] { std::fs::write(bin.join(name), "retired").unwrap(); }
        std::fs::write(bin.join("custom-command"), "keep").unwrap();
        for _ in 0..2 {
            install(&tmp).unwrap();
            assert!(!bin.join("vknowledge").exists());
            assert!(!bin.join("vknowledge.cmd").exists());
            assert!(!bin.join("vorch").exists());
            assert!(!bin.join("vorch.cmd").exists());
            assert!(shim_path(&bin, "vkb").is_file());
            assert_eq!(std::fs::read_to_string(bin.join("custom-command")).unwrap(), "keep");
        }
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn knowledge_skills_replace_retired_name_without_implicit_invocation() {
        let tmp = std::env::temp_dir().join(format!("vlx-vkb-skills-{}", uuid::Uuid::new_v4()));
        for (provider, for_codex) in [("claude", false), ("codex", true)] {
            let dir = tmp.join(provider);
            // Simulate an older bundle that has only the original skill.
            std::fs::create_dir_all(dir.join("vknowledge/agents")).unwrap();
            std::fs::write(dir.join("vknowledge/SKILL.md"), "old bundled skill").unwrap();
            std::fs::write(dir.join("vknowledge/agents/openai.yaml"), "old bundled policy").unwrap();
            std::fs::create_dir_all(dir.join("custom-skill")).unwrap();
            std::fs::write(dir.join("custom-skill/SKILL.md"), "keep").unwrap();
            for _ in 0..2 {
                write_skills(&dir, for_codex).unwrap();
                assert!(!dir.join("vknowledge").exists());
                assert_eq!(std::fs::read_to_string(dir.join("custom-skill/SKILL.md")).unwrap(), "keep");
                let content = std::fs::read_to_string(dir.join("vkb/SKILL.md")).unwrap();
                assert!(content.contains("name: vkb\n"));
                assert!(content.contains("only when the user explicitly"));
                assert!(content.contains("vkb memories"));
                if for_codex {
                    assert!(!content.contains("disable-model-invocation:"));
                    let policy = std::fs::read_to_string(dir.join("vkb/agents/openai.yaml")).unwrap();
                    assert!(policy.contains("allow_implicit_invocation: false"));
                } else {
                    assert!(content.contains("disable-model-invocation: true"));
                }
            }
        }
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[test]
    fn startup_removes_retired_skills_and_respects_bundle_setting() {
        let tmp = std::env::temp_dir().join(format!("vlx-vkb-refresh-{}", uuid::Uuid::new_v4()));
        for enabled in [false, true] {
            let root = tmp.join(enabled.to_string());
            let dirs = [root.join("claude"), root.join("codex")];
            for dir in &dirs {
                std::fs::create_dir_all(dir.join("vknowledge")).unwrap();
                std::fs::write(dir.join("vknowledge/SKILL.md"), "old bundled skill").unwrap();
                std::fs::create_dir_all(dir.join("vorch")).unwrap();
                std::fs::write(dir.join("vorch/SKILL.md"), "old task splitter").unwrap();
            }
            if enabled {
                std::fs::create_dir_all(dirs[0].join("vspawn")).unwrap();
                std::fs::write(dirs[0].join("vspawn/SKILL.md"), "bundle sentinel").unwrap();
            }
            for _ in 0..2 {
                refresh_skills_in_dirs(dirs.clone()).unwrap();
                for dir in &dirs {
                    assert!(!dir.join("vknowledge").exists());
                    assert!(!dir.join("vorch").exists());
                    assert_eq!(dir.join("vkb/SKILL.md").exists(), enabled);
                    assert_eq!(dir.join("vask/SKILL.md").exists(), enabled);
                    assert_eq!(dir.join("vself/SKILL.md").exists(), enabled);
                    assert_eq!(dir.join("vflow/SKILL.md").exists(), enabled);
                    if enabled {
                        let skill = std::fs::read_to_string(dir.join("vask/SKILL.md")).unwrap();
                        assert!(skill.contains("name: vask"));
                        assert!(skill.contains("vrefer <session> --ask"));
                    }
                }
            }
        }
        std::fs::remove_dir_all(tmp).unwrap();
    }

    #[cfg(feature = "gui")]
    #[test]
    fn user_cli_forwards_one_quoted_project_path() {
        let rendered = render_user_cli(Path::new("/tmp/Vela Term/velaterm"));
        assert!(rendered.contains(VELA_SHIM_MARKER));
        assert!(rendered.contains("--open-project"));
        #[cfg(unix)]
        assert!(rendered.contains("\"$@\""));
        #[cfg(windows)]
        assert!(rendered.contains("%*"));
    }

    #[cfg(all(feature = "gui", unix))]
    #[test]
    fn user_cli_install_conflict_forward_and_uninstall_are_isolated() {
        use std::os::unix::fs::PermissionsExt;
        use std::process::Command;

        let tmp = std::env::temp_dir().join(format!(
            "vela-user-cli-test-{}-with-space",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let fake_exe = tmp.join("Vela Term");
        std::fs::write(&fake_exe, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        std::fs::set_permissions(&fake_exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let dest = tmp.join("vela");

        write_user_cli_at(&dest, &fake_exe).unwrap();
        let project = tmp.join("project with space");
        let output = Command::new(&dest).arg(&project).output().unwrap();
        assert!(output.status.success());
        let forwarded = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            forwarded.lines().collect::<Vec<_>>(),
            ["--open-project", project.to_str().unwrap()]
        );
        assert!(remove_user_cli_at(&dest).unwrap());
        assert!(!dest.exists());

        std::fs::write(&dest, "#!/bin/sh\necho user-owned\n").unwrap();
        assert!(write_user_cli_at(&dest, &fake_exe).is_err());
        assert!(!remove_user_cli_at(&dest).unwrap());
        assert_eq!(
            std::fs::read_to_string(&dest).unwrap(),
            "#!/bin/sh\necho user-owned\n"
        );
        let _ = std::fs::remove_dir_all(tmp);
    }

    #[cfg(all(feature = "gui", target_os = "macos"))]
    #[test]
    fn admin_script_escapes_applescript_string_metacharacters() {
        let rendered = render_admin_script("cp '/tmp/a b' '/usr/local/bin/vela'");
        assert_eq!(
            rendered,
            "do shell script \"cp '/tmp/a b' '/usr/local/bin/vela'\" with administrator privileges"
        );
        let injected = render_admin_script("echo \"hi\" \\ end");
        assert!(injected.contains("\\\"hi\\\""));
        assert!(injected.contains("\\\\"));
        assert!(!injected.contains("echo \"hi\""));
    }

    #[cfg(all(feature = "gui", target_os = "macos"))]
    #[test]
    fn admin_cancel_detection_matches_numeric_code_not_wording() {
        assert!(is_admin_cancel("execution error: User canceled. (-128)"));
        assert!(is_admin_cancel("execution error: Benutzer hat abgebrochen. (-128)"));
        assert!(!is_admin_cancel("execution error: cp: /x: No such file or directory (-2741)"));
    }

    #[cfg(all(feature = "gui", target_os = "macos"))]
    #[test]
    fn shell_quoting_escapes_single_quotes() {
        assert_eq!(shell_quote(Path::new("/tmp/plain")), "'/tmp/plain'");
        assert_eq!(shell_quote(Path::new("/tmp/it's")), "'/tmp/it'\\''s'");
    }
}
