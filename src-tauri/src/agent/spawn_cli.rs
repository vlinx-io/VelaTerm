//! Installation and discovery of built-in command shims (`vspawn`, `vspawn-tree`, and `vopen`).
//!
//! At startup VelaTerm installs thin shims under the application data `bin/` directory and prepends it
//! to each session shell's PATH, allowing `vspawn "task"` and `vopen <file>` with no separate setup.
//!
//! Each shim invokes a hidden main-program subcommand such as `$VLX_EXE --spawn` or `--view`, implemented
//! in `agent/cli_client.rs`. Unix uses a tiny `#!/bin/sh` wrapper and Windows a `.cmd` file, keeping all
//! platform logic in Rust and working on Windows without Bash.

use std::path::{Path, PathBuf};

/// Built-in `(command name, main-program subcommand arguments)` shims. `vspawn` creates a child session
/// without a worktree, `vspawn-tree` forces `--worktree`, and `vopen` opens a document or browser tab.
///
/// Unique `v`-prefixed names avoid shadowing system commands such as Vim's `/usr/bin/view`, so simply
/// prepending the bin directory is sufficient without ZDOTDIR/path_helper reordering.
const SHIMS: &[(&str, &str)] = &[
    ("vspawn", "--spawn"),
    ("vspawn-tree", "--spawn --worktree"),
    ("vopen", "--view"),
    ("vagent", "--agent-ctl"),
];

#[cfg(feature = "gui")]
const VELA_SHIM_MARKER: &str = "VelaTerm managed vela command";

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

/// Install `vela` into the current login shell's PATH. macOS invokes this explicitly from menus/settings;
/// Windows and Linux releases may run it at startup for packaged-app convenience. Never modify shell
/// profiles, elevate privileges, or overwrite a same-name file without the application marker.
#[cfg(feature = "gui")]
pub fn install_user_cli() -> std::io::Result<UserCliStatus> {
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

/// Remove every VelaTerm-managed `vela` shim in PATH without touching user-owned commands.
#[cfg(feature = "gui")]
pub fn uninstall_user_cli() -> std::io::Result<UserCliStatus> {
    for dir in user_path_dirs() {
        let dest = managed_cli_path(&dir);
        remove_user_cli_at(&dest)?;
    }
    Ok(user_cli_status())
}

#[cfg(feature = "gui")]
fn user_path_dirs() -> Vec<PathBuf> {
    let Some(path_env) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = std::env::split_paths(&path_env).collect();
    // Prefer `/usr/local/bin` on macOS like VS Code, but only when already in PATH so the command is visible.
    #[cfg(target_os = "macos")]
    if let Some(i) = dirs.iter().position(|p| p == Path::new("/usr/local/bin")) {
        let preferred = dirs.remove(i);
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

#[cfg(feature = "gui")]
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
        if for_codex {
            std::fs::write(dest.join("SKILL.md"), codex_skill_content(content))?;
        } else {
            std::fs::write(dest.join("SKILL.md"), content)?;
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
    // Older releases wrote only ~/.claude/skills. A sentinel on either side means the user enabled the
    // bundle, so refresh also fills the other directory.
    let was_installed = skill_dirs()
        .map(|dirs| dirs.iter().any(|dir| sentinel_exists(dir)))
        .unwrap_or(false);
    if was_installed {
        if let Err(e) = install_skills() {
            eprintln!("failed to refresh installed agent skills (skill content may lag behind the current version): {e}");
        }
    }
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
}
