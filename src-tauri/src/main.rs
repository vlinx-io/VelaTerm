// Prevents additional console window on Windows in release, DO NOT REMOVE!!
// Use the Windows GUI subsystem only for GUI builds. The minimal `vela-server` build needs a console
// for SSH logs, so gate the subsystem on the `gui` feature.
#![cfg_attr(
    all(not(debug_assertions), feature = "gui"),
    windows_subsystem = "windows"
)]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // --version / -V prints the version and Git commit, then exits without a window or service. Keep
    // this first so it returns before raising file-descriptor limits, starting --serve, or creating GUI
    // state. SSH remote connection uses it to identify and pin the remote binary version and commit.
    if matches!(
        args.get(1).map(String::as_str),
        Some("--version") | Some("-V")
    ) {
        velaterm_lib::print_version();
        return;
    }
    if args.get(1).map(String::as_str) == Some("--vela-help") {
        velaterm_lib::print_vela_help();
        return;
    }
    // Hidden lifecycle-hook/notify subcommands: forward the event and exit without starting the GUI.
    if matches!(
        args.get(1).map(String::as_str),
        Some("--notify") | Some("--notify-env") | Some("--codex-hook") | Some("--grok-hook")
    ) {
        match args.get(1).map(String::as_str) {
            Some("--codex-hook") => velaterm_lib::run_codex_hook(&args),
            Some("--grok-hook") => velaterm_lib::run_grok_hook(&args),
            _ => velaterm_lib::run_notify(&args),
        }
        return;
    }
    // Hidden built-in command subcommands. Thin PATH-prepended shims such as vspawn and vopen invoke
    // this binary as a cross-platform replacement for the old shell scripts, read injected VLX_*
    // variables, POST to the local hook service, and exit after forwarding.
    match args.get(1).map(String::as_str) {
        Some("--spawn") => velaterm_lib::run_spawn(&args),
        Some("--view") => velaterm_lib::run_view(&args),
        Some("--agent-ctl") => velaterm_lib::run_agent_ctl(&args),
        _ => {}
    }
    // Raise the process's open-file soft limit on paths that can launch PTY sessions: --serve and the
    // desktop app. Short-lived shims above have already returned. Child agents inherit this limit;
    // without it, Node-based agents can fail under macOS's default limit of 256 descriptors.
    raise_fd_limit();
    // Headless server mode starts browser remote access (HTTPS, login, WebSocket, and PTY) from the CLI
    // without creating a window or requiring a display server.
    if args.get(1).map(String::as_str) == Some("--serve") {
        velaterm_lib::run_serve(&args);
        return;
    }
    // Everything below is GUI-only. The minimal server either ran --serve above or reaches the
    // not(gui) misuse branch below when invoked without a subcommand.
    #[cfg(feature = "gui")]
    {
        // Dock and desktop launches inherit a minimal PATH. Recover the login shell's PATH before any
        // subprocess is spawned so host::command() can find Homebrew and other user-installed tools.
        #[cfg(not(windows))]
        hydrate_path_from_login_shell();

        // Linux/WebKitGTK black-screen fallback: webkit2gtk 2.40+ enables the DMABUF renderer, which can
        // produce a completely black WebView on virtual GPUs and some drivers without reporting errors.
        // Disabling DMABUF selects the stable path while retaining accelerated compositing and has
        // little effect on physical GPUs. Set it before WebKit initialization and only when the user
        // has not supplied an override.
        #[cfg(target_os = "linux")]
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        let open_project = match velaterm_lib::open_project_from_args(
            &args,
            &std::env::current_dir().unwrap_or_default(),
        ) {
            Ok(path) => path,
            Err(e) => {
                eprintln!("vela: {e}");
                std::process::exit(2);
            }
        };
        velaterm_lib::run(open_project)
    }
    // The minimal server has no GUI. Reaching this branch means no valid server or shim subcommand was
    // supplied, so print the correct usage and exit with a nonzero status.
    #[cfg(not(feature = "gui"))]
    {
        eprintln!(
            "vela-server: headless build (no GUI). usage: vela-server --serve [--port <p>] [--data-dir <dir>]; password via VELA_SERVE_PASSWORD env. also: --version"
        );
        std::process::exit(2);
    }
}

/// Raise this process's RLIMIT_NOFILE soft limit, never lower it, to a safe value up to 65,536 and the
/// hard limit. PTY shells and the agents they launch inherit it.
///
/// macOS defaults to 256, below the number of files Node-based agents may open at startup. Raising a
/// process's own soft limit is unprivileged; the `rlimit` crate handles macOS's OPEN_MAX caveat.
/// Windows has no equivalent, and failures are logged without preventing startup.
fn raise_fd_limit() {
    #[cfg(unix)]
    if let Err(e) = rlimit::increase_nofile_limit(65536) {
        eprintln!("failed to raise fd limit (does not affect startup): {e}");
    }
}

/// Recover the user's login-shell PATH and replace this process's PATH.
///
/// Apps launched from the macOS Dock or Finder receive launchd's minimal
/// `/usr/bin:/bin:/usr/sbin:/sbin` PATH; Linux desktop launches have a similar issue. Paths for
/// Homebrew, Node, Cargo, and other tools configured in shell startup files are otherwise missing.
///
/// Direct subprocesses from `host::command()`, including Git probes, worktree operations, and agent
/// verification, would fail to find tools. A known symptom is `git-lfs: command not found` while
/// creating a worktree: `/usr/bin/git` is visible, but its `/opt/homebrew/bin/git-lfs` child is not.
/// PTY terminals are unaffected because their login shell reconstructs PATH; host::command bypasses it.
///
/// Run `$SHELL -i -l -c` once, print `$PATH` between sentinel strings, and extract it from stdout.
/// Sentinels isolate noise emitted by startup files, while interactive mode includes paths commonly
/// configured in `.zshrc` or `.bashrc` that login mode alone would miss.
///
/// Skip when TERM is set because terminal launches, including `pnpm tauri dev`, already have a complete
/// PATH. Windows GUI processes inherit the system PATH normally and do not compile this logic.
#[cfg(all(feature = "gui", not(windows)))]
fn hydrate_path_from_login_shell() {
    if std::env::var_os("TERM").is_some() {
        return;
    }

    const BEGIN: &str = "__VLX_PATH_BEGIN__";
    const END: &str = "__VLX_PATH_END__";

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let script = format!(r#"printf '{BEGIN}%s{END}' "$PATH""#);

    // Startup files may contain interactive prompts such as read or select. Run the shell in a worker
    // with a timeout so such prompts cannot block application startup; retain inherited PATH on timeout.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut probe = std::process::Command::new(&shell);
        // Probe the shell with the system environment; otherwise the captured `PATH` would carry the
        // AppImage's bundle directories back into this process.
        velaterm_lib::appimage::scrub_command(&mut probe);
        let out = probe
            .arg("-i")
            .arg("-l")
            .arg("-c")
            .arg(&script)
            .output();
        let _ = tx.send(out);
    });

    let Ok(Ok(out)) = rx.recv_timeout(std::time::Duration::from_secs(5)) else {
        eprintln!("failed to read PATH from login shell (keeping inherited PATH)");
        return;
    };

    let stdout = String::from_utf8_lossy(&out.stdout);
    let Some(path) = stdout
        .split_once(BEGIN)
        .and_then(|(_, rest)| rest.split_once(END))
        .map(|(p, _)| p.trim())
    else {
        eprintln!("login shell printed no PATH marker (keeping inherited PATH)");
        return;
    };

    // Accept only a nonempty value containing `/usr/bin`; malformed expansions such as fish's
    // space-separated PATH array should leave the inherited value untouched.
    if path.is_empty() || !path.split(':').any(|p| p == "/usr/bin") {
        eprintln!("login shell PATH looks malformed (keeping inherited PATH)");
        return;
    }
    std::env::set_var("PATH", path);
}
