//! AppImage runtime environment cleanup for child processes.
//!
//! On Linux the AppImage is started by AppImageKit's `AppRun`, which rewrites the environment so the
//! bundled runtime can find its own files. It exports `PYTHONHOME`, `PYTHONPATH`, `PERLLIB`,
//! `QT_PLUGIN_PATH`, `GST_PLUGIN_SYSTEM_PATH*`, `GSETTINGS_SCHEMA_DIR`, `XDG_DATA_DIRS`, and prepends
//! bundle directories to `PATH` and `LD_LIBRARY_PATH`, all pointing inside the temporary mount
//! (`/tmp/.mount_XXXXXX`). Desktop integration plugins add more of the same, such as
//! `GDK_PIXBUF_MODULE_FILE` and `GI_TYPELIB_PATH`.
//!
//! Those values are correct for this process but wrong for anything the user launches from a terminal:
//! a PTY inherits the full environment, so `python3` looks for its standard library under the mount and
//! fails with `ModuleNotFoundError: No module named 'encodings'`, and every dynamically linked program
//! resolves libraries against the bundle before the system.
//!
//! This module restores the pre-AppImage view of the environment for spawned children. It is deliberately
//! applied per spawn rather than to this process: WebKitGTK launches its own web and network processes,
//! which do need the bundled paths.
//!
//! The cleanup is generic rather than a fixed variable list, so plugin-injected variables are covered
//! too: any value whose path segments point into `$APPDIR` loses those segments, and a variable left with
//! nothing is removed. On other platforms, and on Linux outside an AppImage, every entry point here is a
//! no-op.

use std::sync::OnceLock;

/// Variables the AppImage runtime sets that carry no bundle path and must still be dropped.
///
/// `ARGV0` is the launcher's own `argv[0]`; zsh consumes it to rename itself, so leaking it renames the
/// user's login shell. `OWD` is the directory the AppImage was launched from and only confuses tooling.
const RUNTIME_ONLY_VARS: &[&str] = &["ARGV0", "OWD"];

/// Variables describing the running AppImage that stay visible to children. They contain a bundle path
/// but break nothing, and programs use them to detect that they run inside an AppImage.
const KEEP_VARS: &[&str] = &["APPDIR", "APPIMAGE"];

/// Environment changes that undo the AppImage rewrite: `Some(value)` replaces, `None` removes.
type Overrides = Vec<(String, Option<String>)>;

/// Compute the overrides for one environment snapshot. Split out from process state so it is testable.
fn compute(appdir: &str, vars: impl Iterator<Item = (String, String)>) -> Overrides {
    // Trailing separators would break the prefix comparison against `$APPDIR/...` segments.
    let appdir = appdir.trim_end_matches('/');
    if appdir.is_empty() {
        return Vec::new();
    }
    let mut out: Overrides = Vec::new();
    for (key, value) in vars {
        if KEEP_VARS.contains(&key.as_str()) {
            continue;
        }
        if RUNTIME_ONLY_VARS.contains(&key.as_str()) {
            out.push((key, None));
            continue;
        }
        if !value.contains(appdir) {
            continue;
        }
        // Treat every value as a `:`-separated path list. Single-path variables such as `PYTHONHOME` are
        // the one-element case and end up removed, which is what the pre-AppImage environment looked like.
        let kept: Vec<&str> = value
            .split(':')
            .filter(|seg| !is_under(seg, appdir))
            .collect();
        if kept.len() == value.split(':').count() {
            // The bundle path appears somewhere other than at a segment boundary; leave it alone rather
            // than guess at the syntax.
            continue;
        }
        // AppRun appends the original value after its own entries, so dropping the bundle segments
        // restores what the variable held before launch. Nothing left means it was not set before.
        if kept.iter().all(|seg| seg.is_empty()) {
            out.push((key, None));
        } else {
            out.push((key, Some(kept.join(":"))));
        }
    }
    out
}

/// True when `segment` is the bundle directory itself or a path inside it.
fn is_under(segment: &str, appdir: &str) -> bool {
    match segment.strip_prefix(appdir) {
        Some(rest) => rest.is_empty() || rest.starts_with('/'),
        None => false,
    }
}

/// Bundle directory of the running AppImage, cached because it cannot change for the life of the process.
/// `None` on every other platform and outside an AppImage.
fn appdir() -> Option<&'static str> {
    static CACHE: OnceLock<Option<String>> = OnceLock::new();
    CACHE
        .get_or_init(|| std::env::var("APPDIR").ok())
        .as_deref()
}

/// Overrides for the environment as it stands right now. Deliberately recomputed per call rather than
/// cached: startup rewrites this process's `PATH` from the login shell, and a cached override would pin
/// the pre-rewrite value and force it back onto every child. Spawns are rare and the scan is a few dozen
/// short strings.
fn overrides() -> Overrides {
    match appdir() {
        Some(dir) => compute(dir, std::env::vars()),
        None => Vec::new(),
    }
}

/// Directory holding the WebKitGTK helper processes shipped in this bundle, or `None` outside an
/// AppImage and when the bundle does not carry them.
///
/// The bundled WebKit resolves `WebKitNetworkProcess` and `WebKitWebProcess` through a path that is
/// relative to the working directory rather than anchored to the bundle, so it finds them only when the
/// process happens to start from a directory that contains a matching `lib/<arch>/webkit2gtk-<abi>/`
/// tree. Callers pass the result to `WEBKIT_EXEC_PATH` to make the lookup independent of that.
pub fn webkit_exec_path() -> Option<std::path::PathBuf> {
    find_webkit_exec_path(std::path::Path::new(appdir()?))
}

/// Scan one bundle root for the helper directory. Split out from process state so it is testable.
///
/// The architecture triple and the WebKit ABI version are discovered rather than hard-coded, so the
/// lookup survives a cross-architecture build and a `webkit2gtk-4.1` to `4.x` bump.
fn find_webkit_exec_path(appdir: &std::path::Path) -> Option<std::path::PathBuf> {
    // $APPDIR/usr/lib/<arch-triple>/webkit2gtk-<abi>/WebKitNetworkProcess
    for arch in std::fs::read_dir(appdir.join("usr").join("lib")).ok()?.flatten() {
        let Ok(entries) = std::fs::read_dir(arch.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if dir
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("webkit2gtk-"))
                && dir.join("WebKitNetworkProcess").is_file()
            {
                return Some(dir);
            }
        }
    }
    None
}

/// Value a variable should have in a child process, with AppImage paths removed. `None` means the child
/// must not receive it at all. Use this instead of `std::env::var` when building a child's environment.
pub fn clean_var(key: &str) -> Option<String> {
    if let Some((_, value)) = overrides().into_iter().find(|(k, _)| k == key) {
        return value;
    }
    std::env::var(key).ok()
}

/// Remove AppImage runtime pollution from a PTY child's environment.
pub fn scrub_pty(cmd: &mut portable_pty::CommandBuilder) {
    for (key, value) in overrides() {
        match value {
            Some(v) => cmd.env(&key, v),
            None => cmd.env_remove(&key),
        }
    }
}

/// Remove AppImage runtime pollution from a plain child process's environment.
pub fn scrub_command(cmd: &mut std::process::Command) {
    for (key, value) in overrides() {
        match value {
            Some(v) => {
                cmd.env(&key, v);
            }
            None => {
                cmd.env_remove(&key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const APPDIR: &str = "/tmp/.mount_VelaTeGDcjCp";

    fn overrides_for(pairs: &[(&str, &str)]) -> Overrides {
        compute(
            APPDIR,
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>()
                .into_iter(),
        )
    }

    fn lookup<'a>(o: &'a Overrides, key: &str) -> Option<&'a Option<String>> {
        o.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// `PYTHONHOME` points only at the bundle, so the child must not see it. This is the reported failure:
    /// with it set, the system `python3` cannot find its standard library.
    #[test]
    fn removes_variables_that_only_point_into_the_bundle() {
        let o = overrides_for(&[("PYTHONHOME", "/tmp/.mount_VelaTeGDcjCp/usr/")]);
        assert_eq!(lookup(&o, "PYTHONHOME"), Some(&None));
    }

    /// AppRun writes `PYTHONPATH=$APPDIR/usr/share/pyshared/:$old`, leaving a trailing separator when the
    /// user had none. An empty remainder means the variable was unset before launch.
    #[test]
    fn removes_variables_whose_remainder_is_empty() {
        let o = overrides_for(&[(
            "PYTHONPATH",
            "/tmp/.mount_VelaTeGDcjCp/usr/share/pyshared/:",
        )]);
        assert_eq!(lookup(&o, "PYTHONPATH"), Some(&None));
    }

    /// A user-set value survives; only the prepended bundle segments are dropped.
    #[test]
    fn keeps_the_user_value_behind_the_bundle_segments() {
        let o = overrides_for(&[(
            "PATH",
            "/tmp/.mount_VelaTeGDcjCp/usr/bin/:/tmp/.mount_VelaTeGDcjCp/usr/sbin/:/usr/local/bin:/usr/bin",
        )]);
        assert_eq!(
            lookup(&o, "PATH"),
            Some(&Some("/usr/local/bin:/usr/bin".to_string()))
        );
    }

    /// Library resolution is the most damaging leak: children would link against bundled libraries before
    /// the system ones.
    #[test]
    fn strips_bundle_library_directories() {
        let o = overrides_for(&[(
            "LD_LIBRARY_PATH",
            "/tmp/.mount_VelaTeGDcjCp/usr/lib/:/tmp/.mount_VelaTeGDcjCp/lib64/:/opt/cuda/lib64",
        )]);
        assert_eq!(
            lookup(&o, "LD_LIBRARY_PATH"),
            Some(&Some("/opt/cuda/lib64".to_string()))
        );
    }

    /// Variables with no bundle path are untouched, so the child keeps the user's environment.
    #[test]
    fn leaves_unrelated_variables_alone() {
        let o = overrides_for(&[("HOME", "/home/vlinx"), ("LANG", "en_US.UTF-8")]);
        assert!(lookup(&o, "HOME").is_none());
        assert!(lookup(&o, "LANG").is_none());
    }

    /// `ARGV0` and `OWD` carry no bundle path but still belong to the launcher.
    #[test]
    fn drops_launcher_only_variables() {
        let o = overrides_for(&[("ARGV0", "./VelaTerm.AppImage"), ("OWD", "/home/vlinx")]);
        assert_eq!(lookup(&o, "ARGV0"), Some(&None));
        assert_eq!(lookup(&o, "OWD"), Some(&None));
    }

    /// Bundle identity stays visible so programs can still detect the AppImage.
    #[test]
    fn keeps_appimage_identity_variables() {
        let o = overrides_for(&[
            ("APPDIR", APPDIR),
            ("APPIMAGE", "/home/vlinx/Apps/VelaTerm.AppImage"),
        ]);
        assert!(o.is_empty());
    }

    /// A bundle path that is not its own segment is left as-is; the value's syntax is unknown.
    #[test]
    fn ignores_bundle_paths_inside_a_segment() {
        let o = overrides_for(&[("SOME_FLAGS", "--prefix=/tmp/.mount_VelaTeGDcjCp/usr")]);
        assert!(o.is_empty());
    }

    /// A prefix that merely shares leading characters is not inside the bundle.
    #[test]
    fn does_not_match_sibling_directories() {
        let o = overrides_for(&[("PATH", "/tmp/.mount_VelaTeGDcjCpOTHER/bin:/usr/bin")]);
        assert!(o.is_empty());
    }

    /// Outside an AppImage there is nothing to undo.
    #[test]
    fn empty_appdir_produces_no_overrides() {
        let o = compute(
            "",
            [("PATH".to_string(), "/usr/bin".to_string())].into_iter(),
        );
        assert!(o.is_empty());
    }

    /// Unique bundle root for the helper-lookup tests, which need real directories to scan. Removed
    /// again by the test that creates it.
    fn bundle_root(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("velaterm-{}-{}", name, std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        dir
    }

    /// The helpers sit under `usr/lib/<arch>/webkit2gtk-<abi>/`, the tree WebKit's own
    /// working-directory-relative lookup misses. Neither the triple nor the ABI version is hard-coded,
    /// so a value unlike the current build is used here on purpose.
    #[test]
    fn finds_bundled_webkit_helpers() {
        let root = bundle_root("finds-helpers");
        let helpers = root.join("usr/lib/aarch64-linux-gnu/webkit2gtk-4.2");
        std::fs::create_dir_all(&helpers).unwrap();
        std::fs::write(helpers.join("WebKitNetworkProcess"), b"").unwrap();
        assert_eq!(find_webkit_exec_path(&root), Some(helpers));
        std::fs::remove_dir_all(&root).ok();
    }

    /// A bundle without the helpers must yield nothing, so the caller leaves `WEBKIT_EXEC_PATH` unset
    /// rather than pointing it at a directory that holds no helpers.
    #[test]
    fn ignores_directories_without_helpers() {
        let root = bundle_root("ignores-empty");
        std::fs::create_dir_all(root.join("usr/lib/x86_64-linux-gnu/webkit2gtk-4.1")).unwrap();
        std::fs::create_dir_all(root.join("usr/lib/x86_64-linux-gnu/gio/modules")).unwrap();
        assert_eq!(find_webkit_exec_path(&root), None);
        std::fs::remove_dir_all(&root).ok();
    }

    /// Outside an AppImage there is no bundle to scan.
    #[test]
    fn finds_no_helpers_without_a_bundle() {
        let root = bundle_root("no-bundle");
        assert_eq!(find_webkit_exec_path(&root), None);
    }
}
