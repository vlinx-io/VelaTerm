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
            // Unix has a native curl installer; Windows falls back to global npm. The package's postinstall is what
            // creates bin/opencode.exe, and newer npm skips install scripts unless allowed, so allow it explicitly
            // (older npm only warns about the unknown flag and runs scripts anyway).
            command: if win {
                "npm install -g --allow-scripts=opencode-ai opencode-ai".into()
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
        "omp" => InstallRecipe {
            label: "OMP".into(),
            bin: "omp".into(),
            // Official Node-free installer; it drops a single prebuilt binary into ~/.local/bin.
            command: if win {
                "irm https://omp.sh/install.ps1 | iex".into()
            } else {
                "curl -fsSL https://omp.sh/install | sh".into()
            },
            needs_node: false,
            docs_url: "https://omp.sh/".into(),
            auth_hint: "Run `omp`, complete the setup, or use `/login` inside a session.".into(),
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
            // macOS uses the official Homebrew tap; Linux/Windows use global npm, whose postinstall downloads the Go
            // binary. Allow that script so the binary arrives during install instead of on the first launch.
            command: if cfg!(target_os = "macos") {
                "brew install charmbracelet/tap/crush".into()
            } else {
                "npm install -g --allow-scripts=@charmland/crush @charmland/crush".into()
            },
            needs_node: !cfg!(target_os = "macos"),
            docs_url: "https://github.com/charmbracelet/crush".into(),
            auth_hint:
                "Run `crush` and pick a provider in onboarding (sign in or set a provider API key)."
                    .into(),
        },
        "deepseek" => InstallRecipe {
            label: "DeepSeek Harness".into(),
            bin: "dsh".into(),
            // Published on npm only; the package's `dsh` launcher runs every profile, including `acp`.
            command: "npm install -g @deepseek-ai/dsh".into(),
            needs_node: true,
            docs_url: "https://github.com/deepseek-ai/deepseek-harness".into(),
            auth_hint:
                "Set `DEEPSEEK_API_KEY`, or run `dsh web` and save your API key on its Models page."
                    .into(),
        },
        "kimi" => InstallRecipe {
            label: "Kimi Code".into(),
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
            label: "Grok Build".into(),
            bin: "grok".into(),
            // Windows uses global npm; allow its postinstall, which places grok.exe under ~/.grok/bin.
            command: if win {
                "npm install -g --allow-scripts=@xai-official/grok @xai-official/grok".into()
            } else {
                "curl -fsSL https://x.ai/cli/install.sh | bash".into()
            },
            needs_node: win,
            docs_url: "https://docs.x.ai/build/overview".into(),
            auth_hint:
                "Run `grok login` to sign in, or set `XAI_API_KEY`; use `grok models` to list available models."
                    .into(),
        },
        "zcode" => {
            // ZCode publishes no command-line package; its agent ships inside the desktop release. macOS takes
            // only the agent from the release (`ZCODE_MAC_INSTALL`) and runs it with Node; Windows installs the
            // desktop app silently for the current user, and its agent then runs through the app's binary (see
            // `agent::zcode::command`). There is no Linux release.
            let mac = cfg!(target_os = "macos");
            InstallRecipe {
                label: "ZCode".into(),
                bin: "zcode".into(),
                command: if win {
                    ZCODE_WINDOWS_INSTALL.into()
                } else if mac {
                    ZCODE_MAC_INSTALL.trim().into()
                } else {
                    String::new()
                },
                needs_node: mac,
                docs_url: "https://github.com/zai-org/ZCode/releases/latest".into(),
                auth_hint: if win {
                    "Open the ZCode desktop app once and sign in; VelaTerm uses its agent and account.".into()
                } else if mac {
                    "Run `zcode login` to sign in, or sign in once in the ZCode desktop app; both use the same account.".into()
                } else {
                    "ZCode ships desktop releases for macOS and Windows only. If you built it from source, set a `zcode` path in Settings > Agents.".into()
                },
            }
        }
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

/// macOS one-click install for ZCode: copy the agent, its provider config and (on Apple silicon, the only
/// architecture the release targets) its search tools out of the latest desktop release into
/// `~/.zcode/vlx-term/agent`, then write a `~/.local/bin/zcode` shim that runs it with Node, unless another
/// `zcode` is already there. The agent reads its store through `node:sqlite`, hence the Node check. The
/// subshell keeps `set -e` and `exit` from reaching an interactive shell the command is pasted into.
const ZCODE_MAC_INSTALL: &str = r##"
(
set -e
node -e 'require("node:sqlite")' >/dev/null 2>&1 || { echo "ZCode needs Node.js 22.13 or later." >&2; exit 1; }
url=$(curl -fsSL https://api.github.com/repos/zai-org/ZCode/releases/latest | grep -o 'https://[^"]*-mac-arm64\.dmg' | head -n 1)
[ -n "$url" ] || { echo "The latest ZCode release has no macOS download." >&2; exit 1; }
tmp=$(mktemp -d)
trap 'hdiutil detach "$tmp/mnt" -quiet >/dev/null 2>&1; rm -rf "$tmp"' EXIT
curl -fL --progress-bar -o "$tmp/ZCode.dmg" "$url"
hdiutil attach -nobrowse -readonly -mountpoint "$tmp/mnt" "$tmp/ZCode.dmg" >/dev/null
res="$tmp/mnt/ZCode.app/Contents/Resources"
dir="$HOME/.zcode/vlx-term/agent"
rm -rf "$dir.new"
mkdir -p "$dir.new"
cp -R "$res/glm" "$res/config" "$dir.new/"
if [ "$(uname -m)" = arm64 ]; then cp -R "$res/tools" "$dir.new/"; fi
rm -rf "$dir"
mv "$dir.new" "$dir"
bin="$HOME/.local/bin/zcode"
if [ -e "$bin" ] && ! grep -q 'Installed by VelaTerm' "$bin"; then
  echo "Kept the existing $bin; the agent is in $dir."
else
  mkdir -p "$HOME/.local/bin"
  {
    echo '#!/bin/sh'
    echo '# ZCode agent from the ZCode desktop release. Installed by VelaTerm.'
    echo "root='$dir'"
    echo 'export ZCODE_BUILTIN_PROVIDER_CONFIG_FILE="${ZCODE_BUILTIN_PROVIDER_CONFIG_FILE:-$root/config/provider/zcode-builtin.json}"'
    echo '[ -x "$root/tools/ripgrep/rg" ] && export ZCODE_RG_BINARY="${ZCODE_RG_BINARY:-$root/tools/ripgrep/rg}"'
    echo '[ -x "$root/tools/bfs/bfs" ] && export ZCODE_BFS_BINARY="${ZCODE_BFS_BINARY:-$root/tools/bfs/bfs}"'
    echo '[ -x "$root/tools/ugrep/ugrep" ] && export ZCODE_UGREP_BINARY="${ZCODE_UGREP_BINARY:-$root/tools/ugrep/ugrep}"'
    echo 'exec node "$root/glm/zcode.cjs" "$@"'
  } > "$bin"
  chmod +x "$bin"
fi
)
"##;

/// Windows one-click install for ZCode: run the latest desktop installer silently for the current user. It
/// is an NSIS installer that asks for no elevation, so it lands in `%LOCALAPPDATA%\Programs\ZCode`.
const ZCODE_WINDOWS_INSTALL: &str = "$ErrorActionPreference = 'Stop'; $ProgressPreference = 'SilentlyContinue'; \
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; \
$release = Invoke-RestMethod 'https://api.github.com/repos/zai-org/ZCode/releases/latest'; \
$asset = $release.assets | Where-Object { $_.name -like '*-win-x64.exe' } | Select-Object -First 1; \
if (-not $asset) { throw 'The latest ZCode release has no Windows installer.' }; \
$setup = Join-Path $env:TEMP $asset.name; \
Invoke-WebRequest $asset.browser_download_url -OutFile $setup -UseBasicParsing; \
Start-Process -FilePath $setup -ArgumentList '/S', '/currentuser' -Wait; \
Remove-Item $setup";

/// A one-click install as typed into the session shell, serialized to frontend camelCase.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRun {
    /// The recommended command, followed by a step that leaves a completion marker once it returns.
    pub command: String,
    /// Marker token for `install_finished`. None when the marker path cannot be quoted for the shell; the
    /// command is then the bare recipe and the frontend falls back to location polling alone.
    pub token: Option<String>,
}

/// Directory under the data directory where finished one-click installs leave their markers.
const INSTALL_MARKER_DIR: &str = "install-markers";

/// Markers left behind by an abandoned poll are removed on the next install after this age.
const INSTALL_MARKER_TTL: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// Build the one-click install command for `agent`.
///
/// An executable appearing on disk does not mean its installer is done: Zoo's script extracts `bin/roo`
/// first and fetches its npm dependencies afterwards, and npm creates a wrapper before the package is in
/// place. Declaring completion on the binary alone lets "Restart now" kill the installer midway. The shell
/// therefore creates a marker after the recipe returns, and completion is only judged once it exists.
pub fn begin_install(data_dir: &std::path::Path, agent: &str) -> Option<InstallRun> {
    let command = install_recipe(agent)?.command.trim().to_string();
    if command.is_empty() {
        return None;
    }
    let dir = data_dir.join(INSTALL_MARKER_DIR);
    let token = uuid::Uuid::new_v4().simple().to_string();
    let wrapped = std::fs::create_dir_all(&dir).ok().and_then(|_| {
        prune_install_markers(&dir);
        with_install_marker(&command, &dir.join(&token), cfg!(target_os = "windows"))
    });
    Some(match wrapped {
        Some(command) => InstallRun { command, token: Some(token) },
        None => InstallRun { command, token: None },
    })
}

/// Whether the install started with `token` has returned. A found marker is consumed.
pub fn install_finished(data_dir: &std::path::Path, token: &str) -> bool {
    // The token names a file, so accept only the hex form `begin_install` generates.
    if token.len() != 32 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    let marker = data_dir.join(INSTALL_MARKER_DIR).join(token);
    marker.is_file() && std::fs::remove_file(&marker).is_ok()
}

/// Longest a backgrounded install may run before it is stopped and reported as failed.
const BACKGROUND_INSTALL_LIMIT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Run the one-click install for an agent that has no terminal interface, and wait for it to return.
///
/// Such an agent has no session shell to type the recipe into, so it runs here instead: `/bin/sh -c` on Unix,
/// PowerShell on Windows, both with the user's login environment so `node` and `npm` resolve as they would in
/// a terminal. Its output goes to `<data dir>/install-logs/<agent>.log` for support, never to the caller: the
/// view shows its own wording. Errors are fixed codes the frontend maps to text.
pub fn run_install(data_dir: &std::path::Path, agent: &str) -> Result<(), String> {
    let kind = crate::models::SessionKind::from_db(agent);
    if kind.has_terminal_interface() {
        return Err("agent_install_unsupported".into());
    }
    let command = install_recipe(agent).map(|r| r.command.trim().to_string()).unwrap_or_default();
    if command.is_empty() {
        return Err("agent_install_unsupported".into());
    }
    let logs = data_dir.join("install-logs");
    std::fs::create_dir_all(&logs).map_err(|_| "agent_install_failed")?;
    let log = std::fs::File::create(logs.join(format!("{agent}.log"))).map_err(|_| "agent_install_failed")?;
    let err = log.try_clone().map_err(|_| "agent_install_failed")?;
    #[cfg(windows)]
    let mut cmd = {
        let mut cmd = crate::host::command("powershell");
        cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &command]);
        cmd
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut cmd = crate::host::command("/bin/sh");
        cmd.arg("-c").arg(&command);
        cmd
    };
    if let Some(home) = crate::host::home_dir() {
        cmd.current_dir(home);
    }
    crate::login_env::refresh_command(&mut cmd);
    cmd.stdin(std::process::Stdio::null()).stdout(log).stderr(err);
    let mut child = cmd.spawn().map_err(|_| "agent_install_failed")?;
    let deadline = std::time::Instant::now() + BACKGROUND_INSTALL_LIMIT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => return Err("agent_install_failed".into()),
            Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(std::time::Duration::from_millis(250)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("agent_install_failed".into());
            }
        }
    }
}

/// Append the marker step in the session shell's syntax: PowerShell on Windows, and on Unix a form that
/// sh, Bash, zsh and Fish all read alike. PowerShell runs it from `finally`, so a terminating error in the
/// recipe still reports completion; a Unix recipe interrupted with Ctrl-C leaves no marker and polling
/// simply times out. Returns None for a path that single quotes cannot carry in every Unix shell.
fn with_install_marker(command: &str, marker: &std::path::Path, win: bool) -> Option<String> {
    let path = marker.to_str()?;
    if path.chars().any(char::is_control) {
        return None;
    }
    if win {
        let quoted = path.replace('\'', "''");
        return Some(format!(
            "try {{ {command} }} finally {{ New-Item -ItemType File -Force -Path '{quoted}' | Out-Null }}"
        ));
    }
    // Fish treats `\` inside single quotes as an escape, unlike POSIX shells, so it cannot be quoted uniformly.
    if path.contains('\'') || path.contains('\\') {
        return None;
    }
    Some(format!("{command}; touch '{path}'"))
}

/// Remove markers older than `INSTALL_MARKER_TTL`. Best effort: a failure only leaves a stale empty file.
fn prune_install_markers(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return; };
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > INSTALL_MARKER_TTL);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Whether an executable at `path` belongs to an installation that stopped partway.
///
/// Zoo's installer extracts `bin/roo` before `npm install` fetches the packages it imports, so an interrupted
/// or failed install leaves a launcher that dies on its first import. Count it only once every dependency in
/// its manifest is present. A missing manifest means a hand-built layout, which is left to the user.
pub(crate) fn incomplete_install(agent: &str, path: &std::path::Path) -> bool {
    if agent != "zoo" {
        return false;
    }
    let Ok(real) = std::fs::canonicalize(path) else {
        return false;
    };
    // The launcher is `<root>/bin/roo`; the ~/.local/bin entry is a symlink resolved above.
    match real.parent().and_then(std::path::Path::parent) {
        Some(root) => node_dependencies_missing(root),
        None => false,
    }
}

/// Whether a self-contained Node package at `root` lacks any dependency its `package.json` declares.
fn node_dependencies_missing(root: &std::path::Path) -> bool {
    let Ok(text) = std::fs::read_to_string(root.join("package.json")) else {
        return false;
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    let Some(deps) = manifest.get("dependencies").and_then(|d| d.as_object()) else {
        return false;
    };
    deps.keys().any(|name| {
        let dir = name.split('/').fold(root.join("node_modules"), |dir, part| dir.join(part));
        !dir.join("package.json").is_file()
    })
}

/// Detect an installed executable only at known locations produced by the recommended command. Stat each
/// candidate, require Unix executability, return an absolute match, and never guess.
///
/// One-click installers often modify a profile that the current session has not reloaded. Retry Launch uses
/// this result to fill an empty executable-path setting so the next launch uses the absolute path.
///
/// A generated Windows command wrapper whose payload is gone is not a match: that state comes from a failed
/// or interrupted install, and handing out the wrapper only produces an opaque cmd.exe path error.
///
/// Strategies mirror install_recipe: native installers stat fixed locations; global npm installations query
/// `npm prefix -g`. Unix probes interactive profiles for nvm/fnm accuracy; Windows uses cmd /C.
pub fn locate_installed_bin(agent: &str) -> Option<String> {
    let (bin, mut candidates, use_npm) = agent_layout(agent)?;
    if use_npm {
        push_npm_candidates(&mut candidates, bin);
    }
    candidates
        .into_iter()
        .find(|p| super::executable::is_executable_file(p) && !incomplete_install(agent, p))
        .map(|p| p.to_string_lossy().to_string())
}

/// The launch catalogue owns the complete agent list. Share npm probes across the whole terminal, rather than
/// starting a shell for each npm-based agent. Every returned path is validated by the same install guard.
pub(crate) fn locate_installed_bins(shell: &str, cwd: Option<&std::path::Path>, agent: bool) -> (Vec<String>, Option<crate::login_env::SessionEnvironment>) {
    // A plain terminal is itself a login shell and finds commands through its own profiles; probing first only
    // delays its start by a full profile load. Agent sessions still probe to locate the program they launch.
    #[cfg(unix)]
    let (prefixes, discovered, path) = if agent {
        shell_installations_in(shell, cwd)
    } else {
        (Vec::new(), Vec::new(), Some(crate::login_env::launch_baseline()))
    };
    #[cfg(windows)]
    let (prefixes, discovered, path) = (npm_global_prefixes(), Vec::<String>::new(), crate::login_env::latest_terminal_environment(shell, cwd, agent));
    let current_path = path.as_ref().and_then(|env| env.path());
    let mut binaries = locate_installed_bins_with(agent_layout, || prefixes.clone());
    binaries.extend(discovered);
    for option in super::launch_options::catalog() {
        if !matches!(option.id, crate::models::SessionKind::Terminal | crate::models::SessionKind::Browser) {
            let bin = super::executable::command_name(option.id);
            if let Some(found) = current_path.as_ref().and_then(|path| super::executable::find_on_path_in(bin, path))
                .or_else(|| super::executable::find_on_path(bin))
            {
                binaries.push(found);
            }
        }
    }
    (binaries, path)
}

fn locate_installed_bins_with(
    layout: impl Fn(&str) -> Option<(&'static str, Vec<std::path::PathBuf>, bool)>,
    npm_prefixes: impl Fn() -> Vec<std::path::PathBuf>,
) -> Vec<String> {
    let mut prefixes = None;
    let mut binaries = Vec::new();
    for option in super::launch_options::catalog() {
        let Some((bin, mut candidates, use_npm)) = layout(option.id.as_str()) else { continue; };
        if use_npm {
            let prefixes = prefixes.get_or_insert_with(&npm_prefixes);
            for prefix in prefixes.iter() { push_npm_candidates_at(&mut candidates, bin, prefix); }
        }
        if let Some(path) = candidates.into_iter()
            .find(|path| super::executable::is_executable_file(path) && !incomplete_install(option.id.as_str(), path))
        {
            binaries.push(path.to_string_lossy().into_owned());
        }
    }
    binaries
}

/// Whether the recommended npm installation exists in name only: the generated command wrapper is on
/// disk but the program it forwards to is gone.
///
/// A failed, interrupted, or quarantined install leaves exactly that state, because npm removes the
/// package while keeping the wrapper it created. Launching that wrapper prints only cmd.exe's bare path
/// error, so the launch path reports the agent as missing and shows the installation guidance instead.
pub fn dangling_npm_install(agent: &str) -> bool {
    let Some((bin, _, true)) = agent_layout(agent) else {
        return false;
    };
    npm_global_prefixes().iter().any(|prefix| {
        let shim = npm_bin_candidate(prefix, bin, cfg!(target_os = "windows"));
        shim.is_file() && !super::executable::is_executable_file(&shim)
    })
}

/// Install layout for one agent on this host: the command name, fixed-location candidates in priority
/// order, and whether the recommended installation is a global npm package. npm-only types leave the
/// fixed list empty; their candidate comes from `npm_bin_candidate`.
fn agent_layout(agent: &str) -> Option<(&'static str, Vec<std::path::PathBuf>, bool)> {
    let home = crate::host::home_dir()?;
    agent_layout_at(agent, &home)
}

fn agent_layout_at(agent: &str, home: &std::path::Path) -> Option<(&'static str, Vec<std::path::PathBuf>, bool)> {
    let win = cfg!(target_os = "windows");
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    let layout = match agent {
        // Claude's official installer targets ~/.local/bin.
        "claude" => {
            candidates.push(
                home.join(".local")
                    .join("bin")
                    .join(exe_name("claude", win)),
            );
            ("claude", false)
        }
        // Cursor's Unix installer links ~/.local/bin/cursor-agent. The Windows installer copies its
        // cursor-agent* launchers into %LOCALAPPDATA%\cursor-agent instead.
        "cursor" => {
            if win {
                let root = local_app_data(&home).join("cursor-agent");
                candidates.push(root.join("cursor-agent.exe"));
                candidates.push(root.join("cursor-agent.cmd"));
            }
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
        // OMP's official installer writes a single binary to PI_INSTALL_DIR when set, otherwise ~/.local/bin/omp
        // on Unix and %LOCALAPPDATA%\omp\omp.exe on Windows, so stat those paths rather than an npm prefix.
        "omp" => {
            if let Some(dir) = std::env::var_os("PI_INSTALL_DIR").filter(|d| !d.is_empty()) {
                candidates.push(std::path::PathBuf::from(dir).join(exe_name("omp", win)));
            }
            if win {
                candidates.push(local_app_data(&home).join("omp").join("omp.exe"));
            }
            candidates.push(home.join(".local").join("bin").join(exe_name("omp", win)));
            ("omp", false)
        }
        // Grok's native installer targets ~/.grok/bin; also probe ~/.local/bin for manually linked installs.
        // Windows uses the official npm fallback.
        "grok" => {
            if !win {
                candidates.push(home.join(".grok").join("bin").join("grok"));
                candidates.push(home.join(".local").join("bin").join("grok"));
            }
            ("grok", win)
        }
        // Antigravity's installer writes ~/.local/bin/agy on Unix and %LOCALAPPDATA%\agy\bin\agy.exe on Windows;
        // otherwise fall back to command-name launch, `agy install`, or a manual setting.
        "antigravity" => {
            if win {
                candidates.push(local_app_data(&home).join("agy").join("bin").join("agy.exe"));
            }
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
        // DeepSeek Harness installs through global npm on every platform.
        "deepseek" => ("dsh", true),
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
        // A `zcode` command comes first: ZCode's source-built installer writes it to ZCODE_DIST_BIN_DIR, default
        // ~/.local/bin, where VelaTerm's macOS install puts its shim too. Then the desktop app, whose binary runs
        // the agent it bundles; only an install that carries that bundle counts.
        "zcode" => {
            if let Some(dir) = std::env::var_os("ZCODE_DIST_BIN_DIR").filter(|d| !d.is_empty()) {
                candidates.push(std::path::PathBuf::from(dir).join(exe_name("zcode", win)));
            }
            candidates.push(home.join(".local").join("bin").join(exe_name("zcode", win)));
            let desktop = if win {
                let mut apps = vec![local_app_data(home).join("Programs").join("ZCode").join("ZCode.exe")];
                if let Some(dir) = std::env::var_os("ProgramFiles").filter(|d| !d.is_empty()) {
                    apps.push(std::path::PathBuf::from(dir).join("ZCode").join("ZCode.exe"));
                }
                apps
            } else if cfg!(target_os = "macos") {
                let app = |root: &std::path::Path| root.join("ZCode.app").join("Contents").join("MacOS").join("ZCode");
                vec![app(std::path::Path::new("/Applications")), app(&home.join("Applications"))]
            } else {
                Vec::new()
            };
            candidates.extend(desktop.into_iter().filter(|bin| super::zcode::desktop_resources(bin).is_some()));
            ("zcode", false)
        }
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
    Some((layout.0, candidates, layout.1))
}

/// Adds the global npm candidates for one command: first the real program a generated wrapper forwards
/// to, then the wrapper itself as the fallback.
///
/// Launching the payload directly avoids cmd.exe re-parsing the wrapper's arguments, which can mangle
/// structured values such as JSON.
fn push_npm_candidates(candidates: &mut Vec<std::path::PathBuf>, bin: &str) {
    for prefix in npm_global_prefixes() { push_npm_candidates_at(candidates, bin, &prefix); }
}

fn push_npm_candidates_at(candidates: &mut Vec<std::path::PathBuf>, bin: &str, prefix: &std::path::Path) {
    let shim = npm_bin_candidate(prefix, bin, cfg!(target_os = "windows"));
    if let Some(exe) = super::executable::shim_payload_exe(&shim) {
        candidates.push(exe);
    }
    candidates.push(shim);
}

/// Executable filename with `.exe` for native Windows installers and a bare name on Unix.
fn exe_name(name: &str, win: bool) -> String {
    if win {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// `%LOCALAPPDATA%`, falling back to `<home>\AppData\Local` when the variable is missing.
fn local_app_data(home: &std::path::Path) -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| home.join("AppData").join("Local"))
}

/// Global npm executable path: `<prefix>/bin/<name>` on Unix or `<prefix>\<name>.cmd` on Windows.
fn npm_bin_candidate(prefix: &std::path::Path, bin: &str, win: bool) -> std::path::PathBuf {
    if win {
        prefix.join(format!("{bin}.cmd"))
    } else {
        prefix.join("bin").join(bin)
    }
}

/// Read every npm prefix exposed by the user's interactive environment. Bash may initialize nvm/fnm only
/// in .bashrc; an interactive login probe alone would miss it. Windows retains cmd /C semantics.
fn npm_global_prefixes() -> Vec<std::path::PathBuf> {
    #[cfg(windows)]
    {
        let mut prefixes = Vec::new();
        let mut command = crate::host::command("cmd");
        if let Some(path) = crate::login_env::latest_path() { command.env("PATH", path); }
        if let Ok(out) = command.args(["/C", "npm prefix -g"]).output() {
            if out.status.success() {
                if let Some(prefix) = npm_prefix_from_output(&out.stdout) { prefixes.push(prefix); }
            }
        }
        prefixes
    }
    #[cfg(unix)]
    {
        shell_installations(&probe_shell()).0
    }
}

#[cfg(unix)]
pub(crate) fn probe_shell() -> String {
    crate::appimage::clean_var("SHELL")
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            ["/bin/zsh", "/bin/bash"]
                .iter()
                .find(|p| std::path::Path::new(p).exists())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "/bin/sh".to_string())
}

/// Probe npm and every supported command together. Sentinels exclude startup banners from discovery.
/// These simple commands also work in Fish; all names originate in the backend launch catalogue.
#[cfg(unix)]
fn shell_installations(shell: &str) -> (Vec<std::path::PathBuf>, Vec<String>, Option<crate::login_env::SessionEnvironment>) {
    shell_installations_in(shell, None)
}

#[cfg(unix)]
fn shell_installations_in(shell: &str, cwd: Option<&std::path::Path>) -> (Vec<std::path::PathBuf>, Vec<String>, Option<crate::login_env::SessionEnvironment>) {
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let prefix_marker = format!("VLX_PREFIX_{nonce}");
    let bin_marker = format!("VLX_BINS_{nonce}");
    let end_marker = format!("VLX_END_{nonce}");
    let path_marker = format!("VLX_PATH_{nonce}");
    let path_end_marker = format!("VLX_PATH_END_{nonce}");
    let names = super::launch_options::catalog().into_iter()
        .filter(|option| !matches!(option.id, crate::models::SessionKind::Terminal | crate::models::SessionKind::Browser))
        .map(|option| format!("command -v {}", super::executable::command_name(option.id)))
        .collect::<Vec<_>>().join("; ");
    let path_script = crate::login_env::environment_capture(&path_marker, &path_end_marker);
    let script = format!("{path_script}; printf '\\n{prefix_marker}\\n'; npm prefix -g; printf '\\n{bin_marker}\\n'; {names}; printf '\\n{end_marker}\\n'");
    let mut prefixes = Vec::new();
    let mut binaries = Vec::new();
    let mut paths = Vec::new();
    for mode in super::executable::shell_probe_modes(shell) {
        if let Some(out) = super::executable::shell_probe_output_in(shell, mode, &script, crate::login_env::CAPTURE_LIMIT, cwd) {
            if let Some(path) = crate::login_env::captured_environment(&out, &path_marker, &path_end_marker) { paths.push(path); }
            let output = String::from_utf8_lossy(&out);
            let Some((_, body)) = output.split_once(&prefix_marker) else { continue; };
            let Some((npm, body)) = body.split_once(&bin_marker) else { continue; };
            if let Some(prefix) = npm_prefix_from_output(npm.as_bytes()) {
                if !prefixes.contains(&prefix) { prefixes.push(prefix); }
            }
            let Some((paths, _)) = body.split_once(&end_marker) else { continue; };
            for line in paths.lines().map(str::trim) {
                if std::path::Path::new(line).is_absolute() && super::executable::is_executable_file(std::path::Path::new(line)) {
                    if !binaries.iter().any(|bin| bin == line) { binaries.push(line.to_string()); }
                }
            }
        }
    }
    (prefixes, binaries, crate::login_env::combine_environments(&paths))
}

fn npm_prefix_from_output(stdout: &[u8]) -> Option<std::path::PathBuf> {
    // Shell profiles may print noise; npm's response is the last nonempty line.
    let stdout = String::from_utf8_lossy(stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())?
        .trim()
        .to_string();
    let p = std::path::PathBuf::from(line);
    (p.is_absolute() && p.is_dir()).then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn fixture_executable(path: &std::path::Path, script: &str) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, script).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn all_agent_installations_are_refreshed_and_callable_in_bash_and_zsh() {
        let root = std::env::temp_dir().join(format!("vlx-all-agents-' 中文-{}", uuid::Uuid::new_v4()));
        let home = root.join("home");
        let prefix = root.join("nvm/prefix");
        std::fs::create_dir_all(&home).unwrap();
        let layout = |agent: &str| {
            let (bin, candidates, npm) = agent_layout_at(agent, &home)?;
            // Map host-wide Homebrew paths into the fixture, and exclude real user environment overrides.
            let mut candidates: Vec<_> = candidates.into_iter().filter(|path| path.starts_with(&root)).collect();
            if !npm && candidates.is_empty() { candidates.push(root.join("brew/bin").join(bin)); }
            Some((bin, candidates, npm))
        };
        let probes = std::cell::Cell::new(0);
        let prefixes = || { probes.set(probes.get() + 1); vec![prefix.clone()] };
        assert!(locate_installed_bins_with(layout, prefixes).is_empty());
        assert_eq!(probes.get(), 1, "one shared prefix lookup before installation");

        let mut kinds = Vec::new();
        for option in super::super::launch_options::catalog() {
            let Some((bin, candidates, npm)) = layout(option.id.as_str()) else { continue; };
            let path = if npm { npm_bin_candidate(&prefix, bin, false) } else { candidates[0].clone() };
            let script = if option.id == crate::models::SessionKind::Codex {
                // The command must also find Node beside the newly discovered npm wrapper.
                fixture_executable(&prefix.join("bin/node"), "#!/bin/sh\nprintf 'FIXTURE:codex\\n'\n");
                "#!/usr/bin/env node\n".to_string()
            } else {
                format!("#!/bin/sh\nprintf 'FIXTURE:{}\\n'\n", option.id.as_str())
            };
            fixture_executable(&path, &script);
            kinds.push(option.id);
        }
        assert_eq!(kinds.len(), 16);
        probes.set(0);
        let binaries = locate_installed_bins_with(layout, prefixes);
        assert_eq!(probes.get(), 1, "all npm agents share one prefix lookup");
        assert_eq!(binaries.len(), kinds.len(), "refresh sees every newly installed agent");
        std::fs::write(home.join(".bash_profile"), "export PATH=/usr/bin:/bin\n").unwrap();
        std::fs::write(home.join(".zprofile"), "export PATH=/usr/bin:/bin\n").unwrap();
        std::fs::write(home.join(".zlogin"), "export PATH=/usr/bin:/bin\n").unwrap();
        let script = kinds.iter().map(|kind| super::super::executable::command_name(*kind))
            .collect::<Vec<_>>().join("; ");
        let dirs = super::super::executable::binary_dirs(&binaries);
        let launch = format!("{} {script}", super::super::executable::PATH_APPEND_COMMAND);
        // Typed sessions retain Bash's native login startup and receive this single launch command afterwards.
        let output = std::process::Command::new("/bin/bash")
            .env_clear().env("HOME", &home).env("PATH", "/usr/bin:/bin")
            .env(super::super::executable::PATH_APPEND_VAR, super::super::executable::path_append_value(&dirs))
            .args(["-lic", &launch]).output().unwrap();
        assert!(output.status.success());
        let output = String::from_utf8(output.stdout).unwrap();
        for kind in &kinds { assert!(output.contains(&format!("FIXTURE:{}\n", kind.as_str())), "{output}"); }
        for shell in ["/bin/bash", "/bin/zsh"] {
            if !std::path::Path::new(shell).exists() { continue; }
            let (state, _) = crate::pty::completion::install(&root, shell).unwrap().unwrap();
            let mut command = std::process::Command::new(shell);
            command.env_clear().env("HOME", &home).env("PATH", "/usr/bin:/bin");
            if shell.ends_with("bash") {
                let rcfile = crate::pty::completion::configure_bash_startup(&state, &binaries).unwrap();
                command.arg("--rcfile").arg(rcfile).args(["-ic", &script]);
            } else {
                let mut pty = portable_pty::CommandBuilder::new(shell);
                crate::pty::completion::configure_zsh_startup(&state, &mut pty, &binaries).unwrap();
                command.env("ZDOTDIR", pty.get_env("ZDOTDIR").unwrap()).args(["-lic", &script]);
            }
            let output = command.output().unwrap();
            assert!(output.status.success(), "{shell}: {}", String::from_utf8_lossy(&output.stderr));
            let output = String::from_utf8(output.stdout).unwrap();
            for kind in &kinds { assert!(output.contains(&format!("FIXTURE:{}\n", kind.as_str())), "{shell}: {output}"); }
            drop(state);
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn bashrc_only_npm_and_custom_agent_paths_are_detected_together() {
        let root = std::env::temp_dir().join(format!("vlx-bash-probe-{}", uuid::Uuid::new_v4()));
        let home = root.join("home");
        let prefix = root.join("nvm/prefix");
        let custom = root.join("custom/bin");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(root.join("empty")).unwrap();
        std::fs::write(home.join(".bash_profile"), format!("export PATH='{}'\n", root.join("empty").display())).unwrap();
        let shell = root.join("bash");
        fixture_executable(&shell, &format!("#!/bin/sh\nexec /usr/bin/env -i HOME='{}' PATH=/usr/bin:/bin /bin/bash \"$@\"\n", home.display()));
        let (prefixes, binaries, _) = shell_installations(shell.to_str().unwrap());
        assert!(prefixes.is_empty() && binaries.is_empty());

        fixture_executable(&prefix.join("bin/npm"), &format!("#!/bin/sh\nprintf '{}\\n'\n", prefix.display()));
        let mut expected = Vec::new();
        for option in super::super::launch_options::catalog() {
            if agent_layout_at(option.id.as_str(), &home).is_none() { continue; }
            let bin = custom.join(super::super::executable::command_name(option.id));
            fixture_executable(&bin, "#!/bin/sh\nexit 0\n");
            expected.push(bin.to_string_lossy().into_owned());
        }
        let noise = root.join("banner-only");
        fixture_executable(&noise, "#!/bin/sh\nexit 0\n");
        std::fs::write(home.join(".bashrc"), format!("printf '{}\\n'\nexport PATH='{}:{}'\n", noise.display(), prefix.join("bin").display(), custom.display())).unwrap();
        let (prefixes, mut binaries, _) = shell_installations(shell.to_str().unwrap());
        assert_eq!(prefixes, vec![prefix]);
        binaries.sort();
        expected.sort();
        assert_eq!(binaries, expected, "all 15 commands come from .bashrc, excluding the startup banner");
        std::fs::write(home.join(".bash_profile"), "source \"$HOME/.bashrc\"\n").unwrap();
        let (prefixes, mut binaries, _) = shell_installations(shell.to_str().unwrap());
        binaries.sort();
        assert_eq!(prefixes.len(), 1, "matching login and non-login prefixes are deduplicated");
        assert_eq!(binaries, expected);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn npm_layout_matches_the_host() {
        // The npm flag drives both candidate probing and dangling-wrapper detection, so it must follow
        // the platform conditions the recipes document.
        let win = cfg!(target_os = "windows");
        let mac = cfg!(target_os = "macos");
        let expected = [
            ("opencode", win),
            ("grok", win),
            ("crush", !mac),
            ("codex", true),
            ("copilot", true),
            ("cline", true),
            ("pi", true),
            ("claude", false),
            ("cursor", false),
            ("omp", false),
            ("antigravity", false),
            ("kimi", false),
            ("deepseek", true),
            ("kiro", false),
            ("zoo", false),
            ("zcode", false),
        ];
        for (agent, npm) in expected {
            let (bin, _, use_npm) =
                agent_layout(agent).unwrap_or_else(|| panic!("no layout for {agent}"));
            assert!(!bin.is_empty(), "the command name for {agent} must not be empty");
            assert_eq!(use_npm, npm, "the npm flag for {agent} on this host");
        }
    }

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
            "omp",
            "crush",
        ] {
            let r = install_recipe(a).unwrap_or_else(|| panic!("no install recipe for {a}"));
            assert!(!r.command.is_empty(), "the install command for {a} must not be empty");
            assert!(!r.bin.is_empty(), "the bin for {a} must not be empty");
            assert!(r.docs_url.starts_with("https://"), "the documentation link for {a} should be https");
        }
    }

    #[test]
    fn omp_recipe_uses_the_official_installer() {
        // OMP ships a prebuilt binary through its own installer, so the recipe must not require Node, and its
        // binary name has to match the command inject.rs launches.
        let r = install_recipe("omp").unwrap();
        assert_eq!(r.bin, "omp");
        assert!(!r.needs_node, "OMP's installer downloads a binary and needs no Node");
        assert!(r.command.contains("omp.sh/install"), "the recipe should use the official installer");
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
    fn npm_recipes_allow_the_postinstall_that_creates_the_binary() {
        // Newer npm skips install scripts unless allowed; these packages produce their binary there.
        if cfg!(target_os = "windows") {
            let r = install_recipe("opencode").unwrap();
            assert_eq!(r.command, "npm install -g --allow-scripts=opencode-ai opencode-ai");
            let r = install_recipe("grok").unwrap();
            assert!(r.command.contains("--allow-scripts=@xai-official/grok"));
        }
        if !cfg!(target_os = "macos") {
            let r = install_recipe("crush").unwrap();
            assert!(r.command.contains("--allow-scripts=@charmland/crush"));
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_native_installers_are_probed_under_local_app_data() {
        // Cursor, OMP, and Antigravity install under %LOCALAPPDATA% on Windows, not ~/.local/bin.
        let local = local_app_data(&crate::host::home_dir().unwrap());
        let (_, cursor, _) = agent_layout("cursor").unwrap();
        assert_eq!(cursor[0], local.join("cursor-agent").join("cursor-agent.exe"));
        let (_, omp, _) = agent_layout("omp").unwrap();
        assert!(omp.contains(&local.join("omp").join("omp.exe")));
        let (_, agy, _) = agent_layout("antigravity").unwrap();
        assert_eq!(agy[0], local.join("agy").join("bin").join("agy.exe"));
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

    #[test]
    fn install_marker_step_matches_the_session_shell() {
        let unix = with_install_marker("curl -fsSL https://x/install.sh | sh", std::path::Path::new("/data/install-markers/ab"), false);
        assert_eq!(unix.as_deref(), Some("curl -fsSL https://x/install.sh | sh; touch '/data/install-markers/ab'"));
        // A quote or backslash cannot be carried by single quotes in both POSIX shells and Fish.
        assert!(with_install_marker("x", std::path::Path::new("/it's/ab"), false).is_none());
        assert!(with_install_marker("x", std::path::Path::new("/a\\b/ab"), false).is_none());
        assert!(with_install_marker("x", std::path::Path::new("/a\nb/ab"), false).is_none());
        // PowerShell reports completion from `finally`, doubling single quotes inside the literal.
        let win = with_install_marker("irm https://x/install.ps1 | iex", std::path::Path::new("C:\\Users\\O'Neil\\m\\ab"), true);
        assert_eq!(
            win.as_deref(),
            Some("try { irm https://x/install.ps1 | iex } finally { New-Item -ItemType File -Force -Path 'C:\\Users\\O''Neil\\m\\ab' | Out-Null }")
        );
    }

    #[test]
    fn install_finishes_only_after_the_marker_appears() {
        let data = std::env::temp_dir().join(format!("vlx-install-marker-{}", uuid::Uuid::new_v4()));
        let run = begin_install(&data, "codex").unwrap();
        let token = run.token.expect("a plain temp path takes the marker step");
        assert!(run.command.starts_with(install_recipe("codex").unwrap().command.as_str()));
        assert!(!install_finished(&data, &token));
        std::fs::write(data.join(INSTALL_MARKER_DIR).join(&token), b"").unwrap();
        assert!(install_finished(&data, &token));
        // The marker is consumed, and only generated tokens name a file.
        assert!(!install_finished(&data, &token));
        assert!(!install_finished(&data, "../../etc/passwd"));
        assert!(begin_install(&data, "terminal").is_none());
        let _ = std::fs::remove_dir_all(&data);
    }

    #[cfg(unix)]
    #[test]
    fn install_marker_step_runs_in_every_unix_shell() {
        let dir = std::env::temp_dir().join(format!("vlx-install-shells-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let shells = ["/bin/sh", "/bin/bash", "/bin/zsh", "/usr/bin/fish", "/opt/homebrew/bin/fish", "/usr/local/bin/fish"];
        for (i, shell) in shells.iter().filter(|s| std::path::Path::new(s).exists()).enumerate() {
            let marker = dir.join(format!("m{i}"));
            // A failing recipe still reports that it returned; completion is then judged by the located binary.
            let command = with_install_marker("false | cat && false", &marker, false).unwrap();
            let status = std::process::Command::new(shell).arg("-c").arg(&command).status().unwrap();
            assert!(status.success(), "{shell}");
            assert!(marker.is_file(), "{shell} left no marker");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn zoo_launcher_counts_only_with_its_dependencies() {
        let root = std::env::temp_dir().join(format!("vlx-zoo-partial-{}", uuid::Uuid::new_v4()));
        let cli = root.join(".roo").join("cli");
        let roo = cli.join("bin").join("roo");
        fixture_executable(&roo, "#!/usr/bin/env node\n");
        std::fs::write(cli.join("package.json"), r#"{"dependencies":{"commander":"^12.1.0","@inkjs/ui":"^2.0.0"}}"#).unwrap();
        // The release archive ships only ripgrep; `npm install` adds the rest.
        std::fs::create_dir_all(cli.join("node_modules").join("@vscode").join("ripgrep")).unwrap();
        let link = root.join(".local").join("bin").join("roo");
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&roo, &link).unwrap();
        assert!(incomplete_install("zoo", &roo));
        assert!(incomplete_install("zoo", &link));
        assert!(!incomplete_install("codex", &roo));
        for dep in ["commander", "@inkjs/ui"] {
            let dir = dep.split('/').fold(cli.join("node_modules"), |d, p| d.join(p));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("package.json"), b"{}").unwrap();
        }
        assert!(!incomplete_install("zoo", &roo));
        assert!(!incomplete_install("zoo", &link));
        // A hand-built launcher without a manifest is left to the user.
        let custom = root.join("custom").join("bin").join("roo");
        fixture_executable(&custom, "#!/bin/sh\n");
        assert!(!incomplete_install("zoo", &custom));
        let _ = std::fs::remove_dir_all(&root);
    }
}
