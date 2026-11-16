//! `vela-server` host subcommands for machines that are reached through the account relay rather than SSH:
//! a WSL distribution on another computer, a Docker container, a cloud VM behind a web console.
//!
//! The first run links the machine to a VelaTerm account (device code flow printed in the terminal), shares
//! the workspace with that account and starts serving through the outbound tunnel. Everything else here
//! manages that state on the host itself: full access and its approved devices, a systemd user service, and
//! self-update. None of these commands can be reached remotely.

use crate::db::Db;
use crate::host::{AppCtx, HeadlessHost};
use crate::web::{full_access, public_relay};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const USAGE: &str = "\
Usage: vela-server [command] [--data-dir <dir>]

Commands:
  (none)                     Link this machine to your VelaTerm account if needed, then start serving
  link [--name <name>]       Link this machine to your VelaTerm account
  unlink                     Sign this machine out of your account
  status                     Show the account link, sharing and full access state
  run                        Serve a linked machine (for service managers; never prompts)
  access full|chat           Allow approved devices to use terminals and files, or conversations only
  devices                    List approved devices and devices waiting for approval
  devices approve <id>       Approve a waiting device by the ID shown in `vela-server devices`
  devices revoke <id>        Remove an approved device
  install-service            Run vela-server in the background and start it at login (systemd)
  update [<version>]         Install the latest (or the given) vela-server release
  --serve [options]          Serve the web app on this machine's network (see --serve --help)
  --version                  Print the version
";

const COMMANDS: &[&str] = &[
    "link", "unlink", "status", "run", "access", "devices", "install-service", "update", "help",
];

/// How long a pending link stays valid on the account service.
const LINK_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const LINK_POLL: Duration = Duration::from_secs(2);
const SERVICE_NAME: &str = "vela-server";

struct Options {
    positional: Vec<String>,
    data_dir: Option<String>,
    name: Option<String>,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options { positional: Vec::new(), data_dir: None, name: None };
    let mut it = args.iter().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--data-dir" => options.data_dir = Some(it.next().ok_or("--data-dir requires a value")?.clone()),
            "--name" => options.name = Some(it.next().ok_or("--name requires a value")?.clone()),
            "-h" | "--help" => options.positional.insert(0, "help".into()),
            other if other.starts_with('-') => return Err(format!("unknown option: {other}")),
            other => options.positional.push(other.to_string()),
        }
    }
    Ok(options)
}

/// Runs a host subcommand and exits the process on failure. Returns false when `args` is not a host
/// subcommand, so the caller can fall through to its own handling.
pub fn run(args: &[String]) -> bool {
    let first = args.get(1).map(String::as_str);
    let ours = match first {
        None => true,
        Some(arg) => COMMANDS.contains(&arg) || matches!(arg, "--data-dir" | "--name" | "-h" | "--help"),
    };
    if !ours {
        return false;
    }
    if let Err(e) = parse(args).and_then(|options| execute(&options)) {
        eprintln!("vela-server: {e}");
        std::process::exit(1);
    }
    true
}

fn execute(options: &Options) -> Result<(), String> {
    let words: Vec<&str> = options.positional.iter().map(String::as_str).collect();
    match words.as_slice() {
        [] => first_run(options),
        ["help"] => {
            print!("{USAGE}");
            Ok(())
        }
        ["link"] => {
            let app = open(options)?;
            link(&app, options.name.as_deref())?;
            println!("Start serving with `vela-server run`, or run `vela-server install-service` to keep it running.");
            Ok(())
        }
        ["unlink"] => {
            let app = open(options)?;
            if public_relay::link_status(&app)?.is_none() {
                println!("This machine is not linked to an account.");
                return Ok(());
            }
            public_relay::unlink(&app)?;
            println!("Signed out. Restart the service so the running server disconnects.");
            Ok(())
        }
        ["status"] => status(options),
        ["run"] => {
            let app = open(options)?;
            require_linked(&app)?;
            public_relay::ensure_workspace_share(&app)?;
            drop(app);
            crate::run_relay_serve(options.data_dir.clone())
        }
        ["access", mode] => access(options, mode),
        ["devices"] => devices(options),
        ["devices", "approve", id] => {
            let device = full_access::approve(&data_dir(options)?, id)?;
            println!("Approved \"{}\". It gets full access the next time it opens this machine.", device.name);
            if !full_access::load(&data_dir(options)?).enabled {
                println!("Full access is off. Turn it on with `vela-server access full`.");
            }
            Ok(())
        }
        ["devices", "revoke", id] => {
            let device = full_access::revoke(&data_dir(options)?, id)?;
            println!("Removed \"{}\". Its open sessions end within a minute.", device.name);
            Ok(())
        }
        ["install-service"] => install_service(options),
        ["update"] => update(None),
        ["update", version] => update(Some(version)),
        _ => Err(format!("unrecognized command. Run `vela-server help` for usage.\n\n{USAGE}")),
    }
}

fn data_dir(options: &Options) -> Result<PathBuf, String> {
    let dir = crate::resolve_data_dir(options.data_dir.as_deref(), &crate::serve_identifier())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create data directory {}: {e}", dir.display()))?;
    Ok(dir)
}

/// Opens the host state the account commands operate on, without starting any service.
fn open(options: &Options) -> Result<AppCtx, String> {
    let dir = data_dir(options)?;
    let db = Db::open(&dir.join("vlx-term.db"))?;
    crate::server_region::load(&db);
    Ok(AppCtx::Headless(std::sync::Arc::new(HeadlessHost::new(dir, db))))
}

fn linked(app: &AppCtx) -> Result<bool, String> {
    Ok(public_relay::link_status(app)?.is_some_and(|status| status["linked"] == true))
}

fn require_linked(app: &AppCtx) -> Result<(), String> {
    if linked(app)? {
        Ok(())
    } else {
        Err("this machine is not linked to a VelaTerm account. Run `vela-server link` first.".into())
    }
}

/// `vela-server` with no command: link interactively when needed, then serve.
fn first_run(options: &Options) -> Result<(), String> {
    let app = open(options)?;
    if !linked(&app)? {
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            return Err("this machine is not linked yet. Run `vela-server link` in a terminal first.".into());
        }
        link(&app, options.name.as_deref())?;
    }
    public_relay::ensure_workspace_share(&app)?;
    drop(app);
    println!("Tip: run `vela-server install-service` to keep this server running in the background.");
    crate::run_relay_serve(options.data_dir.clone())
}

/// Default device name: the host name, marked when it runs inside WSL or a container, where host names are
/// often generic or random.
fn default_name() -> String {
    let host = sysinfo::System::host_name().unwrap_or_else(|| "VelaTerm".into());
    match environment() {
        Some(kind) => format!("{host} ({kind})"),
        None => host,
    }
}

fn environment() -> Option<&'static str> {
    let read = |path: &str| std::fs::read_to_string(path).unwrap_or_default().to_lowercase();
    if std::env::var_os("WSL_DISTRO_NAME").is_some() || read("/proc/sys/kernel/osrelease").contains("microsoft") {
        return Some("WSL");
    }
    let cgroup = read("/proc/1/cgroup");
    if Path::new("/.dockerenv").exists() || cgroup.contains("docker") || cgroup.contains("containerd") {
        return Some("Docker");
    }
    None
}

fn link(app: &AppCtx, name: Option<&str>) -> Result<(), String> {
    if linked(app)? {
        println!("This machine is already linked to your account.");
        return Ok(());
    }
    // A credential the account service no longer accepts is stale; clear it before linking again.
    if public_relay::link_status(app)?.is_some() {
        public_relay::unlink(app)?;
    }
    let name: String = name.map(str::to_string).unwrap_or_else(default_name).chars().take(120).collect();
    let (url, _) = public_relay::begin_link(app, Some(&name))?;
    println!();
    println!("Link this machine to your VelaTerm account:");
    println!();
    println!("  {url}");
    println!();
    println!("Open the link on any device, sign in and confirm \"{name}\". Waiting for confirmation…");
    let deadline = Instant::now() + LINK_TIMEOUT;
    loop {
        std::thread::sleep(LINK_POLL);
        // Transient network errors are retried until the link expires.
        if let Ok(true) = public_relay::finish_link(app) {
            break;
        }
        if Instant::now() >= deadline {
            return Err("the link was not confirmed within 10 minutes. Run `vela-server link` to try again.".into());
        }
    }
    public_relay::ensure_workspace_share(app)?;
    let account = public_relay::link_status(app)?
        .and_then(|status| status["account"]["displayName"].as_str().map(str::to_string))
        .unwrap_or_else(|| "your account".into());
    println!("Linked to {account} as \"{name}\". Open it from VelaTerm on another computer: Connect → Remote.");
    Ok(())
}

fn status(options: &Options) -> Result<(), String> {
    let app = open(options)?;
    match public_relay::link_status(&app)? {
        None => println!("Account:      not linked (run `vela-server link`)"),
        Some(status) if status["linked"] != true => {
            println!("Account:      signed out on velaterm.com (run `vela-server link`)")
        }
        Some(status) => {
            println!("Account:      {}", status["account"]["displayName"].as_str().unwrap_or("linked"));
            println!("Device ID:    {}", status["deviceId"].as_str().unwrap_or("-"));
            let shared = status["shares"].as_array().map(|s| s.len()).unwrap_or(0);
            println!("Sharing:      {}", if shared > 0 { format!("{shared} range(s)") } else { "nothing".into() });
        }
    }
    let dir = data_dir(options)?;
    let config = full_access::load(&dir);
    println!(
        "Full access:  {} · {} approved device(s), {} waiting",
        if config.enabled { "on" } else { "off" },
        config.devices.len(),
        full_access::pending(&dir).len()
    );
    println!("Data:         {}", dir.display());
    Ok(())
}

fn access(options: &Options, mode: &str) -> Result<(), String> {
    let dir = data_dir(options)?;
    match mode {
        "full" => {
            full_access::set_enabled(&dir, true)?;
            println!("Full access is on: approved devices can use terminals and files on this machine.");
            if full_access::load(&dir).devices.is_empty() {
                println!("No device is approved yet. Open this machine from VelaTerm on your computer, then run `vela-server devices`.");
            }
        }
        "chat" => {
            full_access::set_enabled(&dir, false)?;
            println!("Full access is off: devices can use AI conversations only. Open full sessions end within a minute.");
        }
        other => return Err(format!("unknown access mode `{other}`; use `full` or `chat`")),
    }
    Ok(())
}

fn date(secs: u64) -> String {
    time::OffsetDateTime::from_unix_timestamp(secs as i64)
        .map(|t| format!("{:04}-{:02}-{:02} {:02}:{:02} UTC", t.year(), t.month() as u8, t.day(), t.hour(), t.minute()))
        .unwrap_or_else(|_| "-".into())
}

fn devices(options: &Options) -> Result<(), String> {
    let dir = data_dir(options)?;
    let config = full_access::load(&dir);
    let pending = full_access::pending(&dir);
    println!("Full access is {}.", if config.enabled { "on" } else { "off (turn it on with `vela-server access full`)" });
    println!();
    println!("Approved devices:");
    if config.devices.is_empty() {
        println!("  none");
    }
    for d in &config.devices {
        println!("  {}  {}  approved {}", full_access::fingerprint(&d.public_key).unwrap_or_default(), d.name, date(d.at));
    }
    println!();
    println!("Waiting for approval:");
    if pending.is_empty() {
        println!("  none");
    }
    for d in &pending {
        println!("  {}  {}  asked {}", full_access::fingerprint(&d.public_key).unwrap_or_default(), d.name, date(d.at));
    }
    if let Some(d) = pending.last() {
        println!();
        println!("Approve with: vela-server devices approve {}", full_access::fingerprint(&d.public_key).unwrap_or_default());
    }
    Ok(())
}

fn systemctl(args: &[&str]) -> bool {
    std::process::Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn install_service(options: &Options) -> Result<(), String> {
    let exe = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|e| format!("cannot locate vela-server: {e}"))?;
    let app = open(options)?;
    require_linked(&app)?;
    drop(app);
    let mut command = format!("\"{}\" run", exe.display());
    if let Some(dir) = &options.data_dir {
        let dir = Path::new(dir).canonicalize().map_err(|e| format!("invalid --data-dir: {e}"))?;
        command.push_str(&format!(" --data-dir \"{}\"", dir.display()));
    }
    if !cfg!(target_os = "linux") || !systemctl(&["show-environment"]) {
        println!("systemd is not available here. Start the server in the background with:");
        println!();
        println!("  nohup {command} >> \"$HOME/.velaterm/server.log\" 2>&1 &");
        println!();
        println!("In a container, add that command to the container's startup script. Keep the data directory on a volume so the link survives rebuilding the container.");
        return Ok(());
    }
    let home = dirs::home_dir().ok_or("cannot resolve the home directory")?;
    let unit_dir = home.join(".config/systemd/user");
    std::fs::create_dir_all(&unit_dir).map_err(|e| format!("cannot create {}: {e}", unit_dir.display()))?;
    let unit = format!(
        "[Unit]\nDescription=VelaTerm server\nWants=network-online.target\nAfter=network-online.target\n\n\
         [Service]\nExecStart={command}\nRestart=on-failure\nRestartSec=5\n\n\
         [Install]\nWantedBy=default.target\n"
    );
    let path = unit_dir.join(format!("{SERVICE_NAME}.service"));
    std::fs::write(&path, unit).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    if !systemctl(&["daemon-reload"]) || !systemctl(&["enable", "--now", SERVICE_NAME]) {
        return Err(format!("wrote {} but systemd could not start it; check `systemctl --user status {SERVICE_NAME}`", path.display()));
    }
    println!("vela-server is running in the background and starts when you log in.");
    println!("Logs: journalctl --user -u {SERVICE_NAME} -f");
    println!("To keep it running after you log out, run once: loginctl enable-linger \"$USER\"");
    Ok(())
}

/// Numeric comparison of dotted versions; non-numeric parts compare as zero.
fn newer(candidate: &str, current: &str) -> bool {
    let parts = |v: &str| v.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    parts(candidate) > parts(current)
}

fn update(requested: Option<&str>) -> Result<(), String> {
    use crate::server_supply;
    if cfg!(windows) {
        return Err("self-update is not available on Windows; download the new release instead.".into());
    }
    let current = env!("CARGO_PKG_VERSION");
    let version = match requested {
        Some(v) => v.trim_start_matches('v').to_string(),
        None => server_supply::fetch_latest_version()?,
    };
    if requested.is_none() && !newer(&version, current) {
        println!("vela-server {current} is up to date.");
        return Ok(());
    }
    let os = if cfg!(target_os = "macos") { "macos" } else { "linux" };
    let arch = std::env::consts::ARCH;
    let key = server_supply::platform_key(os, arch);
    let manifest = server_supply::fetch_manifest(&version)?;
    if manifest.version != version {
        return Err("the release manifest does not match the requested version".into());
    }
    let entry = manifest.platforms.get(&key).ok_or(format!("release {version} has no build for {key}"))?;
    println!("Downloading vela-server {version} for {key}…");
    let bytes = server_supply::download(&entry.url, &|_| {})?;
    if server_supply::sha256_hex(&bytes) != entry.sha256.to_lowercase() {
        return Err("the download does not match the release checksum; nothing was changed".into());
    }
    server_supply::verify_signature(&bytes, &entry.signature)
        .map_err(|e| format!("{e}; nothing was changed"))?;
    let exe = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|e| format!("cannot locate vela-server: {e}"))?;
    let staged = exe.with_file_name(".vela-server.update");
    std::fs::write(&staged, &bytes).map_err(|e| format!("cannot write {}: {e}", staged.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("cannot mark the new binary executable: {e}"))?;
    }
    std::fs::rename(&staged, &exe).map_err(|e| format!("cannot replace {}: {e}", exe.display()))?;
    println!("Installed vela-server {version} at {}.", exe.display());
    if cfg!(target_os = "linux") && systemctl(&["is-active", SERVICE_NAME]) {
        if systemctl(&["restart", SERVICE_NAME]) {
            println!("Restarted the background service. Open sessions reconnect automatically.");
        } else {
            println!("Restart the service to use the new version: systemctl --user restart {SERVICE_NAME}");
        }
    } else {
        println!("Restart vela-server to use the new version.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(items: &[&str]) -> Vec<String> {
        std::iter::once("vela-server").chain(items.iter().copied()).map(String::from).collect()
    }

    #[test]
    fn parses_commands_and_options() {
        let o = parse(&argv(&["devices", "approve", "ab12-cd34", "--data-dir", "/x"])).unwrap();
        assert_eq!(o.positional, vec!["devices", "approve", "ab12-cd34"]);
        assert_eq!(o.data_dir.as_deref(), Some("/x"));
        let o = parse(&argv(&["--name", "build box", "link"])).unwrap();
        assert_eq!(o.name.as_deref(), Some("build box"));
        assert_eq!(o.positional, vec!["link"]);
        assert!(parse(&argv(&["link", "--bogus"])).is_err());
        assert!(parse(&argv(&["--data-dir"])).is_err());
    }

    #[test]
    fn version_comparison() {
        assert!(newer("0.2.10", "0.2.9"));
        assert!(newer("0.3.0", "0.2.99"));
        assert!(!newer("0.2.5", "0.2.5"));
        assert!(!newer("0.2.4", "0.2.5"));
    }
}
