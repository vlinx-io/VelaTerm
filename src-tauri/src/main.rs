// Prevents additional console window on Windows in release, DO NOT REMOVE!!
// Use the Windows GUI subsystem only for GUI builds. The minimal `vela-server` build needs a console
// for SSH logs, so gate the subsystem on the `gui` feature.
#![cfg_attr(
    all(not(debug_assertions), feature = "gui"),
    windows_subsystem = "windows"
)]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    #[cfg(feature = "gui")]
    if args.get(1).map(String::as_str) == Some("--font-catalog") {
        velaterm_lib::print_font_catalog();
        return;
    }
    if args.get(1).map(String::as_str) == Some("--security-verify-draft") {
        velaterm_lib::run_draft_verifier(&args);
        return;
    }
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
        Some("--notify") | Some("--notify-env") | Some("--codex-hook") | Some("--grok-hook") | Some("--cursor-hook")
    ) {
        match args.get(1).map(String::as_str) {
            Some("--codex-hook") => velaterm_lib::run_codex_hook(&args),
            Some("--grok-hook") => velaterm_lib::run_grok_hook(&args),
            Some("--cursor-hook") => velaterm_lib::run_cursor_hook(&args),
            _ => velaterm_lib::run_notify(&args),
        }
        return;
    }
    // Hidden built-in command subcommands. Thin PATH-prepended shims such as vspawn, vopen, vrefer, and
    // vsearch invoke
    // this binary as a cross-platform replacement for the old shell scripts, read injected VLX_*
    // variables, POST to the local hook service, and exit after forwarding.
    match args.get(1).map(String::as_str) {
        Some("--spawn") => velaterm_lib::run_spawn(&args),
        Some("--tell") => velaterm_lib::run_tell(&args),
        Some("--run") => velaterm_lib::run_wait(&args),
        Some("--flow") => velaterm_lib::run_flow(&args),
        Some("--view") => velaterm_lib::run_view(&args),
        Some("--refer") => velaterm_lib::run_refer(&args),
        Some("--search") => velaterm_lib::run_search(&args),
        Some("--orch") => velaterm_lib::run_orch(&args),
        Some("--stat") => velaterm_lib::run_stat(&args),
        Some("--knowledge") => velaterm_lib::run_knowledge(&args),
        _ => {}
    }
    // Raise the process's open-file soft limit on paths that can launch PTY sessions: --serve and the
    // desktop app. Short-lived shims above have already returned. Child agents inherit this limit;
    // without it, Node-based agents can fail under macOS's default limit of 256 descriptors.
    raise_fd_limit();
    // Headless server mode starts browser remote access (HTTPS, login, WebSocket, and PTY) from the CLI
    // without creating a window or requiring a display server. It must run before anything below spawns
    // a thread or child: run_serve first removes the access password from the environment and only then
    // recovers the login-shell environment itself, so the login-shell probe never inherits the password.
    if args.get(1).map(String::as_str) == Some("--serve") {
        velaterm_lib::run_serve(&args);
        return;
    }
    // Dock and desktop launches inherit a minimal environment: no login-shell PATH, no variables a
    // startup file exports. The GUI needs both, so recover them before anything spawns a child process
    // (--serve does the same inside run_serve, after its password scrub). Terminal launches already
    // carry the shell environment and skip this.
    #[cfg(unix)]
    velaterm_lib::login_env::hydrate();
    // Everything below is GUI-only. The minimal server either ran --serve above or reaches the
    // not(gui) misuse branch below when invoked without a subcommand.
    #[cfg(feature = "gui")]
    {
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
                velaterm_lib::diagnostic_warn!("vela: {e}");
                std::process::exit(2);
            }
        };
        velaterm_lib::run(open_project)
    }
    // The minimal server has no GUI. Reaching this branch means no valid server or shim subcommand was
    // supplied, so print the correct usage and exit with a nonzero status.
    #[cfg(not(feature = "gui"))]
    {
        velaterm_lib::diagnostic_warn!(
            "vela-server: headless build (no GUI). usage: vela-server --serve [--port <p>] [--data-dir <dir>] [--password <pw> | --password-file <path>]; password precedence: --password > --password-file (chmod 600) > VELA_SERVE_PASSWORD env > VLX_SERVE_PASSWORD env. also: --version"
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
        velaterm_lib::diagnostic_warn!("failed to raise fd limit (does not affect startup): {e}");
    }
}
