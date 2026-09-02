//! Bundled minimal Git Bash plus on-demand full download for Windows terminals, replacing uutils/coreutils.
//!
//! Windows terminal sessions default to Bash while agent sessions still use PowerShell. At build time,
//! `fetch-gitbash.ps1` trims PortableGit into a roughly 25MB **real Bash** bundle containing bash,
//! msys-2.0.dll, core GNU tools sharing that runtime, and `/etc`, under `resources/gitbash/`.
//!
//! Runtime reads the resource tree in place rather than copying it to data_dir. `BaseDirectory::Resource`
//! resolves the tree beside nonbundle executables, `lib.rs::setup` registers its absolute path through
//! [`set_bundled_dir`], and [`default_bash`] / [`bundled_tool`] use it directly. This replaces copying up to
//! 8,300 files/326MB at every version change, which made Defender scans delay startup dramatically.
//!
//! Release packaging bakes `etc/profile.d/zz-vlx-term.sh` into the tree. It re-prepends `$VLX_BIN_DIR`
//! after PATH reconstruction and supplies command_not_found_handle for omitted commands. Runtime writes
//! nothing because `$VLX_BIN_DIR` is read from each session environment.
//!
//! Missing full tools such as Git, SSH, Perl, and mingw are downloaded on demand from matching PortableGit
//! into `<data_dir>/gitbash-full/`, which then takes priority. This is the only Git Bash tree written to data_dir.
//!
//! Pipeline: fetch into ignored resources/gitbash, bundle through tauri.windows.conf.json, resolve and
//! register at runtime, choose downloaded full over bundled minimal, then prepend `usr/bin` to session PATH
//! because the Windows nonlogin Bash does not run `/etc/profile`.
//!
//! The module compiles and tests cross-platform, but only Windows setup registers and uses the bundled tree.
#![cfg_attr(not(windows), allow(dead_code))]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Resource filename recording the trimmed source version, used to align on-demand full downloads.
const VERSION_FILE: &str = "VLX_GITBASH_VERSION.txt";

/// Absolute bundled resource path registered by GUI setup and read in place. Headless mode has no Tauri
/// resources, so bundled_dir returns None and falls back to system Git Bash or PowerShell.
static BUNDLED_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Register the absolute bundled tree once; called only by GUI setup.
pub fn set_bundled_dir(dir: PathBuf) {
    let _ = BUNDLED_DIR.set(dir);
}

/// Return the registered bundled tree, or None in headless/before setup.
pub fn bundled_dir() -> Option<&'static Path> {
    BUNDLED_DIR.get().map(PathBuf::as_path)
}

/// On-demand full extraction directory and the only Git Bash directory written to data_dir.
pub fn full_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("gitbash-full")
}

/// Return `usr/bin/bash.exe` under a Git Bash installation root.
fn bash_in(root: &Path) -> PathBuf {
    root.join("usr").join("bin").join("bash.exe")
}

/// Select an existing absolute Bash path, preferring downloaded full over bundled minimal, or None for the
/// caller to fall back to system Git Bash/PowerShell.
pub fn default_bash(data_dir: &Path) -> Option<PathBuf> {
    pick_bash(&full_dir(data_dir), bundled_dir())
}

/// Pure default_bash logic for tests: full, then bundled, then None.
fn pick_bash(full: &Path, bundled: Option<&Path>) -> Option<PathBuf> {
    let full_bash = bash_in(full);
    if full_bash.is_file() {
        return Some(full_bash);
    }
    let bash = bash_in(bundled?);
    bash.is_file().then_some(bash)
}

/// Locate an existing `usr/bin` executable, preferring full over bundled.
///
/// Lets SSH remote connections prefer controlled bundled OpenSSH tools over broken or missing PATH versions,
/// including implementations that advertise unsupported sntrup761 KEX. Return None for PATH fallback.
pub fn bundled_tool(data_dir: &Path, tool: &str) -> Option<PathBuf> {
    find_tool(&full_dir(data_dir), bundled_dir(), tool)
}

/// Pure bundled_tool logic accepting injected paths for tests.
fn find_tool(full: &Path, bundled: Option<&Path>, tool: &str) -> Option<PathBuf> {
    let name = format!("{tool}.exe");
    for root in [Some(full), bundled].into_iter().flatten() {
        let exe = root.join("usr").join("bin").join(&name);
        if exe.is_file() {
            return Some(exe);
        }
    }
    None
}

/// Architecture prefix directories paired with the MSYSTEM value Git Bash expects for each. Only full
/// trees carry one of these; the minimal bundle keeps every tool in `usr/bin`.
const ARCH_PREFIXES: [(&str, &str); 3] = [
    ("mingw64", "MINGW64"),
    ("clangarm64", "CLANGARM64"),
    ("mingw32", "MINGW32"),
];

/// MSYSTEM value for a Git Bash tree, or None for a minimal tree without an architecture prefix.
///
/// Git Bash keys its login profile on MSYSTEM: `/etc/msystem` defaults it to `MSYS` when the variable is
/// absent, and `/etc/profile` prepends `<prefix>/bin` to PATH only for MINGW*/CLANG*/UCRT* values. A full
/// tree keeps `git.exe` and `curl.exe` under that prefix and nowhere else, so a login shell started
/// without MSYSTEM cannot see them. Minimal trees need no value because MSYS mode already covers
/// `usr/bin`.
pub fn msystem_for_tree(root: &Path) -> Option<&'static str> {
    ARCH_PREFIXES.iter().find_map(|(prefix, msystem)| {
        root.join(prefix)
            .join("bin")
            .join("git.exe")
            .is_file()
            .then_some(*msystem)
    })
}

/// MSYSTEM for the tree owning `<root>/usr/bin/bash.exe`, covering bundled, downloaded, and
/// user-selected system Git Bash because all three share that layout. None keeps the MSYS default.
pub fn msystem_for_bash(bash: &Path) -> Option<&'static str> {
    msystem_for_tree(bash.parent()?.parent()?.parent()?)
}

/// Whether a tree is full, determined by `git.exe` under its architecture prefix: mingw64, mingw32, or
/// clangarm64. Full trees may be downloaded or bundled as a full release variant.
fn is_full_tree(root: &Path) -> bool {
    msystem_for_tree(root).is_some()
}

/// Whether a downloaded or full bundled Git Bash is ready, controlling download prompts/menu visibility.
pub fn full_installed(data_dir: &Path) -> bool {
    is_full_tree(&full_dir(data_dir)) || bundled_dir().is_some_and(is_full_tree)
}

/// Read one whitespace-separated field of VLX_GITBASH_VERSION.txt, which `fetch-gitbash.ps1` writes as
/// `<version> <variant> <arch>`, for example `2.55.0.windows.2 full 64`.
fn installed_min_field(index: usize) -> Option<String> {
    let v = std::fs::read_to_string(bundled_dir()?.join(VERSION_FILE)).ok()?;
    v.split_whitespace().nth(index).map(str::to_string)
}

/// Read the version field so full downloads match the bundled tree. Return None to fall back to the
/// latest GitHub release.
pub fn installed_min_version() -> Option<String> {
    installed_min_field(0)
}

/// Map the marker's architecture field to the PortableGit asset suffix. An absent or unrecognized field
/// falls back to this build's own architecture, since each vlx-term build ships the matching tree.
fn asset_suffix_for(arch: Option<&str>) -> &'static str {
    match arch {
        Some("arm64") => "arm64",
        Some("32") => "32-bit",
        Some("64") => "64-bit",
        _ if cfg!(target_arch = "aarch64") => "arm64",
        _ if cfg!(target_arch = "x86") => "32-bit",
        _ => "64-bit",
    }
}

/// PortableGit asset suffix for the architecture this install actually runs, read from the bundled marker.
fn portable_git_asset_suffix() -> &'static str {
    asset_suffix_for(installed_min_field(2).as_deref())
}

/// Full Git Bash download progress translated into frontend events by the command layer.
#[derive(Clone, Copy)]
pub enum FullProgress {
    /// Downloaded and total bytes; total is zero without Content-Length.
    Downloading { received: u64, total: u64 },
    /// Download complete and noninterruptible extraction in progress.
    Extracting,
}

/// Download and install the full PortableGit self-extractor under `<data_dir>/gitbash-full`.
///
/// Match installed_min_version or latest GitHub, stream progress, self-extract to `.partial`, verify Bash,
/// then atomically rename to gitbash-full so incomplete installations are never selected.
///
/// Implemented only on Windows; other platforms return an error and never expose the frontend entry.
#[cfg(windows)]
pub fn download_full(data_dir: &Path, progress: &dyn Fn(FullProgress)) -> Result<(), String> {
    use std::io::Read;

    let version = installed_min_version();
    let url = portable_git_url(version.as_deref())?;

    // 1. Stream into a temporary self-extracting executable while reporting progress.
    let client = reqwest::blocking::Client::builder()
        .user_agent("vlx-term")
        .build()
        .map_err(|e| format!("failed to create download client: {e}"))?;
    let mut resp = client
        .get(&url)
        .send()
        .map_err(|e| format!("download request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("download response error: {e}"))?;
    let total = resp.content_length().unwrap_or(0);

    let tmp_sfx =
        std::env::temp_dir().join(format!("vlx-PortableGit-{}.7z.exe", uuid::Uuid::new_v4()));
    {
        let mut file = std::fs::File::create(&tmp_sfx)
            .map_err(|e| format!("failed to create temp file: {e}"))?;
        let mut buf = [0u8; 64 * 1024];
        let mut received: u64 = 0;
        loop {
            let n = resp
                .read(&mut buf)
                .map_err(|e| format!("download read failed: {e}"))?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut file, &buf[..n])
                .map_err(|e| format!("failed to write temp file: {e}"))?;
            received += n as u64;
            progress(FullProgress::Downloading { received, total });
        }
    }

    // 2. Extract to `.partial` so default_bash cannot select incomplete contents.
    progress(FullProgress::Extracting);
    let partial = full_dir(data_dir).with_extension("partial");
    if partial.exists() {
        let _ = std::fs::remove_dir_all(&partial);
    }
    std::fs::create_dir_all(&partial).map_err(|e| format!("failed to create extract dir: {e}"))?;
    let status = std::process::Command::new(&tmp_sfx)
        .arg("-y")
        .arg(format!("-o{}", partial.display()))
        .status()
        .map_err(|e| format!("failed to start self-extractor: {e}"))?;
    let _ = std::fs::remove_file(&tmp_sfx);
    if !status.success() {
        let _ = std::fs::remove_dir_all(&partial);
        return Err(format!(
            "self-extraction failed (exit code {:?})",
            status.code()
        ));
    }
    if !bash_in(&partial).is_file() {
        let _ = std::fs::remove_dir_all(&partial);
        return Err(
            "usr/bin/bash.exe not found after extraction (unexpected package layout)".to_string(),
        );
    }

    // 3. Atomically rename to the final directory.
    let dest = full_dir(data_dir);
    if dest.exists() {
        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("failed to remove old full version: {e}"))?;
    }
    std::fs::rename(&partial, &dest).map_err(|e| format!("failed to install full version: {e}"))?;
    Ok(())
}

/// Build the PortableGit self-extractor URL from a version, or select the matching asset from the latest
/// GitHub release when no bundled version is registered. The architecture suffix follows the bundled tree
/// (`64-bit`, `arm64`, `32-bit`) so an ARM install does not pull the emulated x64 package.
#[cfg(windows)]
fn portable_git_url(version: Option<&str>) -> Result<String, String> {
    let suffix = portable_git_asset_suffix();
    if let Some(ver) = version {
        // Asset names remove the `.windows.N` suffix from versions such as 2.54.0.windows.1.
        let asset_ver = ver.split(".windows").next().unwrap_or(ver);
        return Ok(format!(
            "https://github.com/git-for-windows/git/releases/download/v{ver}/PortableGit-{asset_ver}-{suffix}.7z.exe"
        ));
    }
    // Fall back to assets from the latest release.
    let client = reqwest::blocking::Client::builder()
        .user_agent("vlx-term")
        .build()
        .map_err(|e| format!("failed to create client: {e}"))?;
    let json: serde_json::Value = client
        .get("https://api.github.com/repos/git-for-windows/git/releases/latest")
        .send()
        .map_err(|e| format!("failed to query latest version: {e}"))?
        .error_for_status()
        .map_err(|e| format!("query response error: {e}"))?
        .json()
        .map_err(|e| format!("failed to parse release JSON: {e}"))?;
    json.get("assets")
        .and_then(|a| a.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|asset| {
                let name = asset.get("name")?.as_str()?;
                if name.starts_with("PortableGit-") && name.ends_with(&format!("-{suffix}.7z.exe")) {
                    asset
                        .get("browser_download_url")?
                        .as_str()
                        .map(String::from)
                } else {
                    None
                }
            })
        })
        .ok_or_else(|| format!("PortableGit {suffix} self-extractor not found in latest release"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("vlx-gitbash-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    fn put_bash(root: &Path) {
        std::fs::create_dir_all(root.join("usr/bin")).unwrap();
        std::fs::write(bash_in(root), b"bash").unwrap();
    }

    #[test]
    fn pick_bash_prefers_full_then_bundled_else_none() {
        let base = tmp("pick");
        let full = base.join("full");
        let bundled = base.join("bundled");

        // Neither exists, so return None regardless of registration.
        assert!(pick_bash(&full, None).is_none());
        assert!(pick_bash(&full, Some(&bundled)).is_none());

        // Select the bundled tree when it is the only option.
        put_bash(&bundled);
        assert_eq!(pick_bash(&full, Some(&bundled)).unwrap(), bash_in(&bundled));
        // Headless without a bundled path remains None.
        assert!(pick_bash(&full, None).is_none());

        // A downloaded full tree takes priority.
        put_bash(&full);
        assert_eq!(pick_bash(&full, Some(&bundled)).unwrap(), bash_in(&full));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn find_tool_prefers_full_then_bundled_else_none() {
        let base = tmp("tool");
        let full = base.join("full");
        let bundled = base.join("bundled");

        // Neither tool exists; the caller falls back to system PATH.
        assert!(find_tool(&full, Some(&bundled), "ssh-keyscan").is_none());

        // Select bundled ssh-keyscan when it is the only copy.
        std::fs::create_dir_all(bundled.join("usr/bin")).unwrap();
        let b_ks = bundled.join("usr/bin/ssh-keyscan.exe");
        std::fs::write(&b_ks, b"ks").unwrap();
        assert_eq!(
            find_tool(&full, Some(&bundled), "ssh-keyscan").unwrap(),
            b_ks
        );
        // Return None for a tool absent from the selected tree.
        assert!(find_tool(&full, Some(&bundled), "scp").is_none());
        // Headless without a registered bundle returns None.
        assert!(find_tool(&full, None, "ssh-keyscan").is_none());

        // Prefer ssh-keyscan from the full tree.
        std::fs::create_dir_all(full.join("usr/bin")).unwrap();
        let f_ks = full.join("usr/bin/ssh-keyscan.exe");
        std::fs::write(&f_ks, b"ks").unwrap();
        assert_eq!(
            find_tool(&full, Some(&bundled), "ssh-keyscan").unwrap(),
            f_ks
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn msystem_follows_the_tree_architecture_prefix() {
        let base = tmp("msystem");
        // A minimal tree keeps every tool in usr/bin, so it needs no MSYSTEM override.
        let min = base.join("min");
        put_bash(&min);
        assert_eq!(msystem_for_tree(&min), None);
        assert_eq!(msystem_for_bash(&bash_in(&min)), None);

        // A full x64 tree must run as MINGW64 or /etc/profile never puts mingw64/bin on PATH.
        let full = base.join("full");
        put_bash(&full);
        std::fs::create_dir_all(full.join("mingw64/bin")).unwrap();
        std::fs::write(full.join("mingw64/bin/git.exe"), b"git").unwrap();
        assert_eq!(msystem_for_tree(&full), Some("MINGW64"));
        assert_eq!(msystem_for_bash(&bash_in(&full)), Some("MINGW64"));

        // ARM64 trees use the clangarm64 prefix and the matching MSYSTEM name.
        let arm = base.join("arm");
        put_bash(&arm);
        std::fs::create_dir_all(arm.join("clangarm64/bin")).unwrap();
        std::fs::write(arm.join("clangarm64/bin/git.exe"), b"git").unwrap();
        assert_eq!(msystem_for_bash(&bash_in(&arm)), Some("CLANGARM64"));

        // A path shallower than <root>/usr/bin/bash.exe has no tree to inspect.
        assert_eq!(msystem_for_bash(Path::new("bash.exe")), None);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn asset_suffix_follows_the_marker_architecture() {
        assert_eq!(asset_suffix_for(Some("64")), "64-bit");
        assert_eq!(asset_suffix_for(Some("arm64")), "arm64");
        assert_eq!(asset_suffix_for(Some("32")), "32-bit");
        // An absent or unexpected field falls back to this build's own architecture.
        let own = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else if cfg!(target_arch = "x86") {
            "32-bit"
        } else {
            "64-bit"
        };
        assert_eq!(asset_suffix_for(None), own);
        assert_eq!(asset_suffix_for(Some("riscv")), own);
    }

    #[test]
    fn is_full_tree_detects_arch_prefixes() {
        let base = tmp("full-tree");
        let root = base.join("tree");
        // Bash without architecture Git is not a full tree.
        put_bash(&root);
        assert!(!is_full_tree(&root));
        // x64 uses mingw64/bin/git.exe.
        std::fs::create_dir_all(root.join("mingw64/bin")).unwrap();
        std::fs::write(root.join("mingw64/bin/git.exe"), b"git").unwrap();
        assert!(is_full_tree(&root));

        // ARM64 uses clangarm64/bin for native Git and also counts as full.
        let arm = base.join("arm");
        put_bash(&arm);
        assert!(!is_full_tree(&arm));
        std::fs::create_dir_all(arm.join("clangarm64/bin")).unwrap();
        std::fs::write(arm.join("clangarm64/bin/git.exe"), b"git").unwrap();
        assert!(is_full_tree(&arm));

        let _ = std::fs::remove_dir_all(&base);
    }
}
