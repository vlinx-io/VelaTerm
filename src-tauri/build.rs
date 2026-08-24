use std::process::Command;

fn main() {
    // Inject the Git commit into the binary at build time for `--version` and SSH remote-connection version
    // locking. Plan §8 pins commits rather than versions because one semver may identify different builds; only
    // the commit is a reliable matching key.
    //
    // Resolution priority:
    //   1. VLX_GIT_COMMIT explicitly set in the build environment, supporting builds without .git such as
    //      cross-machine release.sh or Windows remote rebuilds from a tar-synchronized worktree.
    //   2. `git rev-parse HEAD` executed by build.rs, the normal local development/release path.
    //   3. "unknown" fallback, never blocking compilation because a commit is unavailable.
    let commit = std::env::var("VLX_GIT_COMMIT")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .filter(|out| out.status.success())
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=VLX_GIT_COMMIT={commit}");

    // Rerun when the externally injected value changes.
    println!("cargo:rerun-if-env-changed=VLX_GIT_COMMIT");
    // Rerun when Git HEAD changes so incremental builds do not retain a stale commit. In a normal clone this is
    // `<repo>/.git/HEAD` (build.rs runs from src-tauri, hence one parent). This is best-effort: in a worktree,
    // `.git` is a file rather than a directory, so Cargo ignores the missing path and an incremental build may not
    // refresh automatically. Full `cargo build` and release paths always produce a fresh value.
    println!("cargo:rerun-if-changed=../.git/HEAD");

    // Ensure ../dist exists before anything resolves it. `rust_embed::Embed` in `web` resolves its `#[folder]`
    // at COMPILE time and fails the whole crate when the directory is absent, which is the state of every fresh
    // clone until the frontend has been built once. `beforeDevCommand` is `pnpm dev` — a dev server that never
    // writes `dist/` — so `tauri dev` cannot bootstrap a clean checkout without this. Creating the directory is
    // enough; its contents are not needed here, because debug builds read it from disk at runtime and release
    // builds populate it through `beforeBuildCommand`. Best-effort: a failure here is not worth blocking a build
    // that may not need the directory at all.
    let _ = std::fs::create_dir_all("../dist");

    // Creating the directory trades a loud compile error for a silent one: a release build with nothing in
    // `../dist` now embeds an empty asset bundle and boots to a blank window instead of refusing to compile.
    // `pnpm release` and `tauri build` populate it through `beforeBuildCommand` and never hit this, so the
    // warning is aimed at a bare `cargo build --release` that skipped the frontend.
    let dist_is_empty = std::fs::read_dir("../dist")
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(true);
    if dist_is_empty && std::env::var("PROFILE").as_deref() == Ok("release") {
        println!("cargo:warning=../dist is empty; this build embeds no frontend assets and will show a blank window. Run `pnpm build` first, or build through `pnpm release`.");
    }

    // Run tauri-build only for GUI builds. It parses tauri.conf.json, requires frontendDist (../dist), and
    // generates permission scaffolding for generate_context!. The minimal server (`--no-default-features`) does
    // not call generate_context! or need frontend output; skipping this step allows builds without ../dist.
    // Cargo injects CARGO_FEATURE_GUI when the feature is enabled.
    if std::env::var_os("CARGO_FEATURE_GUI").is_some() {
        tauri_build::build();
    }
}
