//! Agent installation guidance as the single source of truth for each host platform.
//!
//! When the interactive-shell launch guard reports `not found on PATH`, AgentInstallCard obtains its
//! recommended command, documentation link, and authentication guidance here. Installation changes need
//! only this module, following the same pattern as `inject::permission_flag`.
//!
//! Commands branch by host OS and prefer native installers without Node, falling back to global npm with
//! `needs_node=true`. They run directly in the agent session's PowerShell on Windows or login shell on Unix.
//!
//! Only local agent types have guidance; terminal and browser sessions do not.

/// Platform-specific agent installation guidance serialized to frontend camelCase.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecipe {
    /// Agent display name such as Claude Code or Codex.
    pub label: String,
    /// Executable command name.
    pub bin: String,
    /// Recommended command executable directly in the session shell.
    pub command: String,
    /// Whether installation requires Node/npm, allowing the frontend to warn accordingly.
    pub needs_node: bool,
    /// Official installation documentation URL.
    pub docs_url: String,
    /// English one-line authentication step still required after installing the binary.
    pub auth_hint: String,
}

/// Return platform guidance for a frontend AgentKind, or None for unknown/unsupported types.
pub fn install_recipe(agent: &str) -> Option<InstallRecipe> {
    // Select commands/installers from the compile-time host OS.
    let win = cfg!(target_os = "windows");
    let r = match agent {
        "claude" => InstallRecipe {
            label: "Claude Code".into(),
            bin: "claude".into(),
            // Official native installer: PowerShell on Windows or curl on Unix, with no Node dependency.
            command: if win {
                "irm https://claude.ai/install.ps1 | iex".into()
            } else {
                "curl -fsSL https://claude.ai/install.sh | bash".into()
            },
            needs_node: false,
            docs_url: "https://code.claude.com/docs/en/setup".into(),
            auth_hint: "Run `claude` and log in via the browser when prompted.".into(),
        },
        "codex" => InstallRecipe {
            label: "Codex".into(),
            bin: "codex".into(),
            // No standalone first-party installer. Use scoped @openai/codex; unscoped codex is unrelated.
            command: "npm install -g @openai/codex".into(),
            needs_node: true,
            docs_url: "https://developers.openai.com/codex/cli".into(),
            auth_hint: "Run `codex` and sign in with your ChatGPT account or an API key.".into(),
        },
        "opencode" => InstallRecipe {
            label: "OpenCode".into(),
            bin: "opencode".into(),
            // Unix has a native curl installer; Windows falls back to global npm.
            command: if win {
                "npm install -g opencode-ai".into()
            } else {
                "curl -fsSL https://opencode.ai/install | bash".into()
            },
            needs_node: win,
            docs_url: "https://opencode.ai/docs/".into(),
            auth_hint: "Run `opencode`, then `/login` (or set a provider API key).".into(),
        },
        "copilot" => InstallRecipe {
            label: "GitHub Copilot CLI".into(),
            bin: "copilot".into(),
            // Global npm only, requiring Node 22+ and an existing Copilot subscription.
            command: "npm install -g @github/copilot".into(),
            needs_node: true,
            docs_url: "https://docs.github.com/copilot/how-tos/set-up/install-copilot-cli".into(),
            auth_hint: "Requires Node 22+. Run `copilot`, then `/login` with your GitHub account."
                .into(),
        },
        "cursor" => InstallRecipe {
            label: "Cursor CLI".into(),
            bin: "cursor-agent".into(),
            // Official Node-free installer: Windows PowerShell with win32 or Unix curl.
            command: if win {
                "irm 'https://cursor.com/install?win32=true' | iex".into()
            } else {
                "curl https://cursor.com/install -fsS | bash".into()
            },
            needs_node: false,
            docs_url: "https://cursor.com/docs/cli/installation".into(),
            auth_hint: "Run `cursor-agent login` to authenticate.".into(),
        },
        "cline" => InstallRecipe {
            label: "Cline".into(),
            bin: "cline".into(),
            // Global npm only with the same Node-dependent command on Windows and Unix.
            command: "npm install -g cline".into(),
            needs_node: true,
            docs_url: "https://docs.cline.bot/cli/installation".into(),
            auth_hint: "Run `cline auth` to configure your provider and API key.".into(),
        },
        "pi" => InstallRecipe {
            label: "Pi".into(),
            bin: "pi".into(),
            // Global npm only; --ignore-scripts avoids running dependency packaging scripts.
            command: "npm install -g --ignore-scripts @earendil-works/pi-coding-agent".into(),
            needs_node: true,
            docs_url: "https://pi.dev/".into(),
            auth_hint:
                "Run `pi`, then `/login` (Claude/ChatGPT/Copilot) or set a provider API key.".into(),
        },
        "antigravity" => InstallRecipe {
            label: "Antigravity CLI".into(),
            // The executable is `agy`, matching inject.rs, not `antigravity`.
            bin: "agy".into(),
            // Official Node-free PowerShell/curl installer; `agy install` can configure PATH afterward.
            command: if win {
                "irm https://antigravity.google/cli/install.ps1 | iex".into()
            } else {
                "curl -fsSL https://antigravity.google/cli/install.sh | bash".into()
            },
            needs_node: false,
            docs_url: "https://antigravity.google/docs/cli-overview".into(),
            auth_hint: "Run `agy` and sign in with your Google account when prompted.".into(),
        },
        "crush" => InstallRecipe {
            label: "Crush".into(),
            bin: "crush".into(),
            // macOS uses the official Homebrew tap; Linux/Windows use global npm containing the Go binary.
            command: if cfg!(target_os = "macos") {
                "brew install charmbracelet/tap/crush".into()
            } else {
                "npm install -g @charmland/crush".into()
            },
            needs_node: !cfg!(target_os = "macos"),
            docs_url: "https://github.com/charmbracelet/crush".into(),
            auth_hint:
                "Run `crush` and pick a provider in onboarding (sign in or set a provider API key)."
                    .into(),
        },
        "kimi" => InstallRecipe {
            label: "Kimi Code (K3)".into(),
            bin: "kimi".into(),
            command: if win {
                "irm https://code.kimi.com/kimi-code/install.ps1 | iex".into()
            } else {
                "curl -fsSL https://code.kimi.com/kimi-code/install.sh | bash".into()
            },
            needs_node: false,
            docs_url: "https://www.kimi.com/code/docs/en/kimi-code-cli/guides/getting-started.html"
                .into(),
            auth_hint:
                "Run `kimi`, then `/login` to sign in with Kimi Code or configure an API key."
                    .into(),
        },
        "kiro" => InstallRecipe {
            // The official installer is a POSIX shell script. Windows support is unconfirmed and VelaTerm's
            // Windows sessions default to PowerShell, so hide one-click installation there while keeping the
            // documentation link and any manually configured executable path.
            label: "Kiro".into(),
            bin: "kiro-cli".into(),
            command: if win {
                String::new()
            } else {
                "curl -fsSL https://cli.kiro.dev/install | bash".into()
            },
            needs_node: false,
            docs_url: "https://kiro.dev/docs/cli/".into(),
            auth_hint: if win {
                "The Kiro CLI installer targets macOS and Linux. Install it in a supported environment or set a `kiro-cli` path in Settings > Agents.".into()
            } else {
                "Run `kiro-cli` and complete the sign-in prompt; installing the binary alone does not authenticate it.".into()
            },
        },
        "grok" => InstallRecipe {
            label: "Grok Build (Grok 4.5)".into(),
            bin: "grok".into(),
            command: if win {
                "npm install -g @xai-official/grok".into()
            } else {
                "curl -fsSL https://x.ai/cli/install.sh | bash".into()
            },
            needs_node: win,
            docs_url: "https://docs.x.ai/build/overview".into(),
            auth_hint:
                "Run `grok login` to sign in, or set `XAI_API_KEY`; use `grok models` to list available models."
                    .into(),
        },
        "zoo" => {
            // Zoo Code currently reuses Roo CLI and `roo`. Its installer lacks Windows support and there is
            // no public npm package, so return an empty Windows command to hide one-click installation while
            // retaining documentation and allowing a manually built roo.exe path.
            let unsupported = win || cfg!(all(target_os = "macos", target_arch = "x86_64"));
            InstallRecipe {
                label: "Zoo Code".into(),
                bin: "roo".into(),
                command: if unsupported {
                    String::new()
                } else {
                    "curl -fsSL https://raw.githubusercontent.com/RooCodeInc/Roo-Code/main/apps/cli/install.sh | sh"
                        .into()
                },
                needs_node: true,
                docs_url: "https://docs.zoocode.dev/update-notes/v3.39".into(),
                auth_hint: if unsupported {
                    "Zoo Code CLI currently ships for macOS Apple Silicon and Linux x64/ARM64. Use a supported environment or set a manually built `roo` path in Settings > Agents.".into()
                } else {
                    "Run `roo` with a provider API key (for example `OPENROUTER_API_KEY`) or pass provider/model launch arguments.".into()
                },
            }
        }
        _ => return None,
    };
    Some(r)
}

/// Detect an installed executable only at known locations produced by the recommended command. Stat each
/// candidate, require Unix executability, return an absolute match, and never guess.
///
/// One-click installers often modify a profile that the current session has not reloaded. Retry Launch uses
/// this result to fill an empty executable-path setting so the next launch uses the absolute path.
///
/// Strategies mirror install_recipe: native installers stat fixed locations; global npm installations query
/// `npm prefix -g`. Unix uses a login shell for profile/nvm/fnm accuracy; Windows uses cmd /C.
pub fn locate_installed_bin(agent: &str) -> Option<String> {
    let win = cfg!(target_os = "windows");
    let home = crate::host::home_dir()?;
    // Fixed-location candidates in priority order; npm-only types leave this list empty.
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    let (bin, use_npm) = match agent {
        // Claude's official installer targets ~/.local/bin.
        "claude" => {
            candidates.push(
                home.join(".local")
                    .join("bin")
                    .join(exe_name("claude", win)),
            );
            ("claude", false)
        }
        // Cursor's official installer also targets ~/.local/bin as cursor-agent.
        "cursor" => {
            candidates.push(
                home.join(".local")
                    .join("bin")
                    .join(exe_name("cursor-agent", win)),
            );
            ("cursor-agent", false)
        }
        // OpenCode's Unix script uses ~/.opencode/bin or, in some versions, ~/.local/bin; Windows uses npm.
        "opencode" => {
            if !win {
                candidates.push(home.join(".opencode").join("bin").join("opencode"));
                candidates.push(home.join(".local").join("bin").join("opencode"));
            }
            ("opencode", win)
        }
        "codex" => ("codex", true),
        "copilot" => ("copilot", true),
        // Cline is global npm only, so infer it from the npm prefix.
        "cline" => ("cline", true),
        // Pi has no native installer and uses global npm only.
        "pi" => ("pi", true),
        // Grok's native installer targets ~/.grok/bin; also probe ~/.local/bin for manually linked installs.
        // Windows uses the official npm fallback.
        "grok" => {
            if !win {
                candidates.push(home.join(".grok").join("bin").join("grok"));
                candidates.push(home.join(".local").join("bin").join("grok"));
            }
            ("grok", win)
        }
        // Antigravity's documented installer path is unclear; probe conventional ~/.local/bin/agy and
        // otherwise fall back to command-name launch, `agy install`, or a manual setting.
        "antigravity" => {
            candidates.push(home.join(".local").join("bin").join(exe_name("agy", win)));
            ("agy", false)
        }
        // Crush uses Homebrew prefixes on macOS and global npm prefixes on Linux/Windows.
        "crush" => {
            if cfg!(target_os = "macos") {
                candidates.push(std::path::PathBuf::from("/opt/homebrew/bin/crush"));
                candidates.push(std::path::PathBuf::from("/usr/local/bin/crush"));
                ("crush", false)
            } else {
                ("crush", true)
            }
        }
        // Kimi installers have used ~/.kimi-code/bin and ~/.local/bin; prefer KIMI_CODE_HOME/bin when set.
        "kimi" => {
            if let Some(root) = std::env::var_os("KIMI_CODE_HOME") {
                candidates.push(
                    std::path::PathBuf::from(root)
                        .join("bin")
                        .join(exe_name("kimi", win)),
                );
            }
            candidates.push(
                home.join(".kimi-code")
                    .join("bin")
                    .join(exe_name("kimi", win)),
            );
            candidates.push(home.join(".local").join("bin").join(exe_name("kimi", win)));
            ("kimi", false)
        }
        // The Kiro installer drops the binary in ~/.local/bin; honor KIRO_HOME/bin when it is set.
        "kiro" => {
            if let Some(root) = std::env::var_os("KIRO_HOME") {
                candidates.push(
                    std::path::PathBuf::from(root)
                        .join("bin")
                        .join(exe_name("kiro-cli", win)),
                );
            }
            candidates.push(
                home.join(".local")
                    .join("bin")
                    .join(exe_name("kiro-cli", win)),
            );
            candidates.push(
                home.join(".kiro")
                    .join("bin")
                    .join(exe_name("kiro-cli", win)),
            );
            ("kiro-cli", false)
        }
        // Zoo/Roo installs at ~/.roo/cli/bin/roo with a ~/.local/bin symlink, so probe both.
        "zoo" => {
            candidates.push(home.join(".local").join("bin").join(exe_name("roo", win)));
            candidates.push(
                home.join(".roo")
                    .join("cli")
                    .join("bin")
                    .join(exe_name("roo", win)),
            );
            ("roo", false)
        }
        _ => return None,
    };
    if use_npm {
        if let Some(prefix) = npm_global_prefix() {
            candidates.push(npm_bin_candidate(&prefix, bin, win));
        }
    }
    candidates
        .into_iter()
        .find(|p| is_executable(p))
        .map(|p| p.to_string_lossy().to_string())
}

/// Executable filename with `.exe` for native Windows installers and a bare name on Unix.
fn exe_name(name: &str, win: bool) -> String {
    if win {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Global npm executable path: `<prefix>/bin/<name>` on Unix or `<prefix>\<name>.cmd` on Windows.
fn npm_bin_candidate(prefix: &std::path::Path, bin: &str, win: bool) -> std::path::PathBuf {
    if win {
        prefix.join(format!("{bin}.cmd"))
    } else {
        prefix.join("bin").join(bin)
    }
}

/// Whether an agent binary exists at a known location or on PATH, at the cost of a shell subprocess.
pub fn agent_available(agent: &str) -> bool {
    if locate_installed_bin(agent).is_some() {
        return true;
    }
    match install_recipe(agent) {
        Some(recipe) => bin_on_path(&recipe.bin),
        None => false,
    }
}

/// Whether a shell resolves a command name. A login shell skips `~/.zshrc`, where nvm and Homebrew often
/// extend PATH, so an interactive retry follows.
fn bin_on_path(bin: &str) -> bool {
    if !valid_probe_name(bin) {
        return false;
    }
    if cfg!(target_os = "windows") {
        return crate::host::command("cmd")
            .args(["/C", "where", bin])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    }
    let shell = login_shell();
    let probe = format!("command -v {bin}");
    ["-lc", "-lic"].iter().any(|flags| {
        crate::host::command(&shell)
            .args([flags, probe.as_str()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Whether a name can enter a shell command unquoted: ASCII alphanumerics plus `-`, `_`, and `.`.
fn valid_probe_name(bin: &str) -> bool {
    !bin.is_empty()
        && bin
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Interactive login shell used for PATH-accurate probes: `$SHELL`, then zsh/bash, then sh.
fn login_shell() -> String {
    std::env::var("SHELL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            ["/bin/zsh", "/bin/bash"]
                .iter()
                .find(|p| std::path::Path::new(p).exists())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "/bin/sh".to_string())
}

/// Query `npm prefix -g` through a login shell so GUI launches load the profile-managed nvm/fnm prefix.
/// Any failure returns None.
fn npm_global_prefix() -> Option<std::path::PathBuf> {
    let out = if cfg!(target_os = "windows") {
        crate::host::command("cmd")
            .args(["/C", "npm prefix -g"])
            .output()
            .ok()?
    } else {
        crate::host::command(login_shell())
            .args(["-lc", "npm prefix -g"])
            .output()
            .ok()?
    };
    if !out.status.success() {
        return None;
    }
    // Shell profiles may print noise; npm's response is the last nonempty line.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())?
        .trim()
        .to_string();
    let p = std::path::PathBuf::from(line);
    p.is_dir().then_some(p)
}

/// Whether a regular file is executable; check mode on Unix and existence for Windows .exe/.cmd.
fn is_executable(p: &std::path::Path) -> bool {
    let Ok(md) = std::fs::metadata(p) else {
        return false;
    };
    if !md.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        md.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_agents_have_nonempty_recipe() {
        for a in [
            "claude",
            "codex",
            "opencode",
            "copilot",
            "cursor",
            "antigravity",
            "cline",
            "pi",
            "crush",
        ] {
            let r = install_recipe(a).unwrap_or_else(|| panic!("no install recipe for {a}"));
            assert!(!r.command.is_empty(), "the install command for {a} must not be empty");
            assert!(!r.bin.is_empty(), "the bin for {a} must not be empty");
            assert!(r.docs_url.starts_with("https://"), "the documentation link for {a} should be https");
        }
    }

    #[test]
    fn pi_recipe_is_scoped_npm_package() {
        // Pi is scoped global npm only and its binary name matches inject.rs.
        let r = install_recipe("pi").unwrap();
        assert_eq!(r.bin, "pi");
        assert!(r.needs_node, "pi installs through npm, so Node is required first");
        assert!(
            r.command.contains("@earendil-works/pi-coding-agent"),
            "the pi install command should point at the scoped npm package"
        );
    }

    #[test]
    fn unknown_agent_has_no_recipe() {
        assert!(install_recipe("terminal").is_none());
        assert!(install_recipe("").is_none());
        assert!(!agent_available("terminal"));
    }

    #[test]
    fn probe_names_reject_shell_metacharacters() {
        for bin in ["claude", "cursor-agent", "kiro-cli", "gpt.5"] {
            assert!(valid_probe_name(bin), "{bin} is a plain command name");
        }
        for bin in ["", "a b", "a;b", "a$(id)", "a`id`", "a|b", "a/b", "a&b", "a'b"] {
            assert!(!valid_probe_name(bin), "{bin:?} must never reach a shell");
        }
        for agent in ["claude", "codex", "cursor", "copilot", "opencode", "kiro", "zoo"] {
            let recipe = install_recipe(agent).unwrap();
            assert!(valid_probe_name(&recipe.bin), "{agent} has an unprobeable bin");
        }
    }

    #[test]
    fn cursor_bin_is_cursor_agent() {
        // Binary names must match inject.rs, including cursor-agent.
        assert_eq!(install_recipe("cursor").unwrap().bin, "cursor-agent");
    }

    #[test]
    fn npm_candidate_layout_per_platform() {
        // Unix uses prefix/bin/name; Windows places the .cmd shim directly under prefix.
        let prefix = std::path::Path::new("/usr/local");
        assert_eq!(
            npm_bin_candidate(prefix, "codex", false),
            std::path::PathBuf::from("/usr/local/bin/codex")
        );
        // Build the Windows prefix/name.cmd expectation with path joins so Unix-hosted tests remain portable.
        let winp = std::path::Path::new(r"C:\Users\x\AppData\Roaming\npm");
        assert_eq!(
            npm_bin_candidate(winp, "codex", true),
            winp.join("codex.cmd")
        );
    }

    #[test]
    fn exe_name_suffix() {
        assert_eq!(exe_name("claude", false), "claude");
        assert_eq!(exe_name("claude", true), "claude.exe");
    }

    #[test]
    fn locate_unknown_agent_is_none() {
        // Do not probe unknown or unguided types, matching install_recipe coverage.
        assert!(locate_installed_bin("terminal").is_none());
        assert!(locate_installed_bin("").is_none());
    }
}
