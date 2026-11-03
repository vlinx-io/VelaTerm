//! Versioned, unmodified Codex Security resources shared by native agent adapters.
use super::{scope, Result};
use rust_embed::RustEmbed;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(RustEmbed)]
#[folder = "resources/codex-security/"]
struct Bundle;

pub const PACKAGE_VERSION: &str = "0.8.2";
pub const PLUGIN_VERSION: &str = "0.1.95";
pub const ADAPTER_VERSION: &str = "vlx-native-mcp-v2";
const MCP_GUARD: &[u8] = include_bytes!("../../resources/security-mcp-guard.mjs");

fn adapted_source(name: &str, bytes: &[u8]) -> Result<Vec<u8>> {
    let replacement = match name {
        "scripts/workbench_db.py" => Some((
            "if not resolved_scope.is_dir():\n        raise SystemExit(\"Scan scope must reference an existing directory inside the target.\")",
            "if not (resolved_scope.is_dir() or resolved_scope.is_file()):\n        raise SystemExit(\"Scan scope must reference an existing directory or file inside the target.\")")),
        "scripts/workbench_target.py" => Some((
            "def directory_snapshot_regular_file_count(target: Path) -> int:\n    paths = git_directory_snapshot_paths(target)",
            "def directory_snapshot_regular_file_count(target: Path) -> int:\n    if target.is_file():\n        return 1\n    paths = git_directory_snapshot_paths(target)")),
        _ => None,
    };
    let Some((before, after)) = replacement else {
        return Ok(bytes.to_vec());
    };
    let source = std::str::from_utf8(bytes).map_err(|_| "security_upstream_invalid")?;
    if source.matches(before).count() != 1 {
        return Err("security_upstream_patch_mismatch".into());
    }
    Ok(source.replacen(before, after, 1).into_bytes())
}

pub fn provenance() -> Result<Value> {
    let resource = Bundle::get("UPSTREAM.json").ok_or("security_upstream_missing")?;
    serde_json::from_slice(&resource.data).map_err(|_| "security_upstream_invalid".into())
}

fn installed_path(root: &Path, relative: &str) -> Result<PathBuf> {
    if !scope::safe_relative(relative) {
        return Err("security_upstream_invalid".into());
    }
    let mut path = root.to_path_buf();
    let parts: Vec<_> = Path::new(relative).components().collect();
    for (index, part) in parts.iter().enumerate() {
        path.push(part.as_os_str());
        let parent = index + 1 < parts.len();
        match std::fs::symlink_metadata(&path) {
            Ok(metadata)
                if metadata.file_type().is_symlink()
                    || (parent && !metadata.is_dir())
                    || (!parent && !metadata.is_file()) =>
            {
                return Err("security_upstream_invalid".into())
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && parent => {
                if let Err(error) = std::fs::create_dir(&path) {
                    if error.kind() != std::io::ErrorKind::AlreadyExists {
                        return Err(error.to_string());
                    }
                    let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
                    if !metadata.is_dir() || metadata.file_type().is_symlink() {
                        return Err("security_upstream_invalid".into());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(path)
}

fn install_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    // Replace the directory entry instead of following an existing hard link.
    let temporary = path.with_file_name(format!(".vlx-install-{}", uuid::Uuid::new_v4()));
    std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    let result = std::fs::rename(&temporary, path).map_err(|e| e.to_string());
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// Embedded files lose their mode bits. Since 0.8 the skills run `scripts/launch_codex_security_mcp`
/// directly, so a script with a shebang must stay executable, as it is in the npm package.
fn mark_executable(path: &Path, bytes: &[u8]) -> Result<()> {
    #[cfg(unix)]
    if bytes.starts_with(b"#!") {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    #[cfg(not(unix))]
    let _ = (path, bytes);
    Ok(())
}

/// Materialize the embedded release, verifying each byte against its source manifest.
/// This directory is application data, never the user's native agent configuration.
pub fn materialize(data_dir: &Path) -> Result<PathBuf> {
    // The package version is part of the name because upstream releases can keep the plugin version.
    let destination = data_dir
        .join("security")
        .join(format!("codex-security-{PACKAGE_VERSION}-{PLUGIN_VERSION}-{ADAPTER_VERSION}"));
    for path in [data_dir.join("security"), destination.clone()] {
        if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("security_upstream_invalid".into());
        }
    }
    std::fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
    let source = provenance()?;
    let files = source["files"]
        .as_object()
        .ok_or("security_upstream_invalid")?;
    let mut patches = Vec::new();
    for (name, digest) in files {
        if !scope::safe_relative(name) {
            return Err("security_upstream_invalid".into());
        }
        let resource = Bundle::get(name).ok_or("security_upstream_missing")?;
        if digest.as_str() != Some(scope::digest(&resource.data).as_str()) {
            return Err("security_upstream_invalid".into());
        }
        let bytes = adapted_source(name, &resource.data)?;
        if bytes != resource.data.as_ref() {
            patches.push(serde_json::json!({"path":name,"upstreamSha256":digest,"installedSha256":scope::digest(&bytes),"purpose":"single-file scope compatibility"}));
        }
        let path = installed_path(&destination, name)?;
        if !std::fs::read(&path).is_ok_and(|current| current == bytes) {
            install_bytes(&path, &bytes)?;
        }
        mark_executable(&path, &bytes)?;
    }
    install_bytes(
        &installed_path(&destination, "UPSTREAM.json")?,
        &serde_json::to_vec_pretty(&source).map_err(|e| e.to_string())?,
    )?;
    install_bytes(&installed_path(&destination, "vlx-security-mcp.mjs")?, MCP_GUARD)?;
    install_bytes(
        &installed_path(&destination, "ADAPTER.json")?,
        &serde_json::to_vec_pretty(
            &serde_json::json!({"version":ADAPTER_VERSION,"patches":patches,
                "files":{"vlx-security-mcp.mjs":{"sha256":scope::digest(MCP_GUARD),"purpose":"source evidence verification before upstream sealing"}}}),
        )
        .map_err(|e| e.to_string())?,
    )?;

    destination.canonicalize().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("vlx-upstream-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn embedded_release_materializes_every_verified_source_file() {
        let temp = Temp::new();
        let plugin = materialize(&temp.0).unwrap();
        let source = provenance().unwrap();
        assert_eq!(source["license"], "Apache-2.0");
        assert_eq!(source["packageVersion"], PACKAGE_VERSION);
        assert_eq!(source["pluginVersion"], PLUGIN_VERSION);
        assert!(plugin.ends_with(format!("codex-security-{PACKAGE_VERSION}-{PLUGIN_VERSION}-{ADAPTER_VERSION}")));
        for (name, expected) in source["files"].as_object().unwrap() {
            let original = Bundle::get(name).unwrap();
            assert_eq!(
                scope::digest(&original.data),
                expected.as_str().unwrap(),
                "{name}"
            );
            assert_eq!(
                std::fs::read(plugin.join(name)).unwrap(),
                adapted_source(name, &original.data).unwrap(),
                "{name}"
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let launcher = plugin.join("scripts/launch_codex_security_mcp");
            assert_eq!(std::fs::metadata(&launcher).unwrap().permissions().mode() & 0o777, 0o755);
            assert_eq!(std::fs::metadata(plugin.join("mcp/server.mjs")).unwrap().permissions().mode() & 0o111, 0);
        }
        std::fs::write(plugin.join("references/core-scan.md"), "corrupted cache").unwrap();
        materialize(&temp.0).unwrap();
        assert_eq!(
            scope::digest(&std::fs::read(plugin.join("references/core-scan.md")).unwrap()),
            source["files"]["references/core-scan.md"].as_str().unwrap()
        );
    }

    #[test]
    fn native_mcp_arguments_preserve_paths_and_existing_configuration() {
        let temp = Temp::new();
        let plugin = temp.0.join("plugin's files");
        let state = temp.0.join("state with spaces");
        for agent in ["codex", "claude", "deepseek"] {
            let args = launch_args(agent, &plugin, &state, "--existing 'a b'").unwrap();
            let parsed = crate::agent::inject::split_extra_args(Some(&args));
            assert_eq!(&parsed[..2], &["--existing", "a b"]);
            if agent == "deepseek" {
                assert_eq!(parsed[2], "--patch");
                let patch: Value = serde_json::from_slice(&std::fs::read(&parsed[3]).unwrap()).unwrap();
                let config = &patch[0]["insert"][0]["config"];
                assert_eq!(patch[0]["insert"][0]["name"], "@deepseek-ai/dsh-mcp-client");
                assert_eq!(config["serverName"], mcp_server_name(agent));
                assert_eq!(config["args"][0], plugin.join("vlx-security-mcp.mjs").to_string_lossy().as_ref());
                assert_eq!(config["env"]["CODEX_SECURITY_STATE_DIR"], state.to_string_lossy().as_ref());
                // DeepSeek hashes function names over 64 characters; every scan tool must keep its name.
                for tool in ["record_codex_security_candidate_validations", "record_codex_security_discovery_candidates"] {
                    assert!(format!("mcp__{}__{tool}", mcp_server_name(agent)).len() <= 64, "{tool}");
                }
            } else if agent == "codex" {
                assert!(parsed.windows(2).any(|pair| pair == ["-c", "mcp_servers.codex-security.command=\"node\""]));
                assert!(parsed.windows(2).any(|pair| pair == ["-c", "mcp_servers.codex-security.enabled=false"]));
                let table: toml_edit::DocumentMut = parsed[3].parse().unwrap();
                assert_eq!(
                    table["mcp_servers"]["vlx-codex-security"]["env"]["CODEX_SECURITY_STATE_DIR"]
                        .as_str(),
                    state.to_str()
                );
            } else {
                assert_eq!(parsed[2], "--mcp-config");
                let config: Value =
                    serde_json::from_slice(&std::fs::read(&parsed[3]).unwrap()).unwrap();
                assert_eq!(
                    config["mcpServers"]["vlx-codex-security"]["args"][0],
                    plugin.join("vlx-security-mcp.mjs").to_string_lossy().as_ref()
                );
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn installation_rejects_symlinked_subdirectories_and_metadata() {
        let temp = Temp::new();
        let outside = Temp::new();
        let plugin = materialize(&temp.0).unwrap();
        std::fs::rename(plugin.join("references"), outside.0.join("references")).unwrap();
        std::os::unix::fs::symlink(outside.0.join("references"), plugin.join("references"))
            .unwrap();
        assert!(materialize(&temp.0).is_err());
        std::fs::remove_file(plugin.join("references")).unwrap();
        std::fs::rename(outside.0.join("references"), plugin.join("references")).unwrap();
        std::fs::write(outside.0.join("metadata.json"), "untouched").unwrap();
        std::fs::remove_file(plugin.join("ADAPTER.json")).unwrap();
        std::os::unix::fs::symlink(outside.0.join("metadata.json"), plugin.join("ADAPTER.json"))
            .unwrap();
        assert!(materialize(&temp.0).is_err());
        assert_eq!(
            std::fs::read_to_string(outside.0.join("metadata.json")).unwrap(),
            "untouched"
        );
    }

    #[test]
    fn actual_workbench_owns_snapshot_and_rejects_drafts_after_cancellation() {
        let temp = Temp::new();
        let plugin = materialize(&temp.0).unwrap();
        let source = temp.0.join("source");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("main.py"), "def add(a, b):\n    return a + b\n").unwrap();
        let state = temp.0.join("state");
        let result = workbench(
            &plugin,
            &state,
            &[
                "start-prompt-only-scan".into(),
                "--thread-id".into(),
                "transport-test".into(),
                "--target-path".into(),
                source.to_string_lossy().into_owned(),
                "--scope".into(),
                ".".into(),
                "--mode".into(),
                "standard".into(),
                "--scan-root".into(),
                temp.0.join("scans").to_string_lossy().into_owned(),
            ],
        )
        .unwrap();
        let scan = &result["scan"];
        assert_eq!(
            scan["contract"]["target"]["allowedKinds"][0],
            "directory_snapshot"
        );
        assert!(scan["contract"]["target"]["requiredSnapshotDigest"]
            .as_str()
            .unwrap()
            .starts_with("codex-security-snapshot/v1:sha256:"));
        let id = scan["scanId"].as_str().unwrap().to_owned();
        workbench(
            &plugin,
            &state,
            &["cancel-scan".into(), "--scan-id".into(), id.clone()],
        )
        .unwrap();
        let result = workbench(
            &plugin,
            &state,
            &["get-scan".into(), "--scan-id".into(), id.clone()],
        )
        .unwrap();
        assert_eq!(result["scan"]["progress"]["status"], "canceled");
        let result = workbench(
            &plugin,
            &state,
            &[
                "write-scan-draft".into(),
                "--scan-id".into(),
                id,
                "--draft-path".into(),
                temp.0.join("late.json").to_string_lossy().into_owned(),
            ],
        );
        assert!(result.unwrap_err().contains("scan stopped"));
    }

    #[test]
    fn scoped_file_keeps_an_exact_file_contract() {
        let temp = Temp::new();
        let plugin = materialize(&temp.0).unwrap();
        let source = temp.0.join("source");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("main.py"), "print('example')\n").unwrap();
        std::fs::write(source.join("other.py"), "print('outside requested file')\n").unwrap();
        let result = workbench(
            &plugin,
            &temp.0.join("state"),
            &[
                "start-prompt-only-scan".into(),
                "--thread-id".into(),
                "file-test".into(),
                "--target-path".into(),
                source.to_string_lossy().into_owned(),
                "--scope".into(),
                "main.py".into(),
                "--mode".into(),
                "standard".into(),
                "--scan-root".into(),
                temp.0.join("scans").to_string_lossy().into_owned(),
            ],
        )
        .unwrap();
        assert_eq!(
            result["scan"]["contract"]["scope"]["requiredIncludePaths"],
            serde_json::json!(["main.py"])
        );
        assert_eq!(result["scan"]["progress"]["coverage"]["filesTotal"], 1);
    }

    #[test]
    fn upstream_diff_inventory_includes_a_deleted_baseline_file() {
        let temp = Temp::new();
        let plugin = materialize(&temp.0).unwrap();
        let source = temp.0.join("source");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("main.py"), "print('baseline')\n").unwrap();
        let git = |args: &[&str]| {
            let result = crate::host::command("git")
                .arg("-C")
                .arg(&source)
                .args(args)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            String::from_utf8(result.stdout).unwrap().trim().to_owned()
        };
        git(&["init", "-q"]);
        git(&["add", "main.py"]);
        let hooks = temp.0.join("empty-hooks");
        std::fs::create_dir(&hooks).unwrap();
        git(&[
            "-c",
            "user.name=Security integration test",
            "-c",
            "user.email=security-test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            &format!("core.hooksPath={}", hooks.display()),
            "commit",
            "-qm",
            "baseline",
        ]);
        let base = git(&["rev-parse", "HEAD"]);
        std::fs::remove_file(source.join("main.py")).unwrap();
        let result = workbench(
            &plugin,
            &temp.0.join("state"),
            &[
                "start-prompt-only-scan".into(),
                "--thread-id".into(),
                "diff-test".into(),
                "--target-path".into(),
                source.to_string_lossy().into_owned(),
                "--scope".into(),
                ".".into(),
                "--mode".into(),
                "diff".into(),
                "--diff-target-kind".into(),
                "working_tree".into(),
                "--scan-root".into(),
                temp.0.join("scans").to_string_lossy().into_owned(),
            ],
        )
        .unwrap();
        assert_eq!(result["scan"]["diffTarget"]["baseRevision"], base);
        // Since 0.8 the MCP server, not a standalone script, writes the changed-file inventory.
        let scan_id = result["scan"]["scanId"].as_str().unwrap();
        let mut server = crate::host::command("node")
            .arg(plugin.join("mcp/server.mjs"))
            .arg("--stdio")
            .env("CODEX_SECURITY_STATE_DIR", temp.0.join("state"))
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut input = server.stdin.take().unwrap();
        for message in [
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"vlx-test","version":"1"}}}),
            serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"prepare_codex_security_review_items","arguments":{"scanId":scan_id}}}),
        ] {
            use std::io::Write;
            writeln!(input, "{message}").unwrap();
        }
        let output = std::io::BufRead::lines(std::io::BufReader::new(server.stdout.take().unwrap()))
            .map(|line| serde_json::from_str::<Value>(&line.unwrap()).unwrap())
            .find(|message| message["id"] == 2)
            .unwrap();
        drop(input);
        let _ = server.wait();
        assert_eq!(output["result"]["structuredContent"]["reviewItemsTotal"], 1, "{output}");
        let inventory = Path::new(result["scan"]["scanDir"].as_str().unwrap())
            .join("artifacts/02_discovery/in_scope_files.txt");
        assert_eq!(
            std::fs::read_to_string(inventory).unwrap().trim(),
            "main.py"
        );
    }
}

pub fn entry_skill(diff: bool) -> &'static str {
    if diff {
        "skills/security-diff-scan/SKILL.md"
    } else {
        "skills/security-scan/SKILL.md"
    }
}

pub fn workbench(plugin: &Path, state: &Path, args: &[String]) -> Result<Value> {
    let python = std::env::var("PYTHON")
        .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.into());
    let output = crate::host::command(&python)
        .arg(plugin.join("scripts/workbench_db.py"))
        .args(args)
        .env("CODEX_SECURITY_STATE_DIR", state)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .map_err(|_| "security_python_unavailable")?;
    if !output.status.success() {
        // Diagnostics may contain paths or repository-controlled text; keep them out of logs.
        return Err(format!(
            "security_upstream_failed:{}",
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .filter(|c| !c.is_control() || *c == '\n')
                .take(4000)
                .collect::<String>()
        ));
    }
    if output.stdout.len() > 16 * 1024 * 1024 {
        return Err("security_upstream_output_too_large".into());
    }
    serde_json::from_slice(&output.stdout).map_err(|_| "security_upstream_invalid".into())
}

fn quote_argument(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Add a session-local MCP server without replacing native authentication or configuration.
/// Explicit per-audit selections must not be overridden by inherited Claude flags.
pub fn selection_args(existing: &str, model: bool, effort: bool) -> String {
    let args = crate::agent::inject::split_extra_args(Some(existing));
    let flags = [("--model", model), ("--effort", effort)];
    let mut kept = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if flags
            .iter()
            .any(|(flag, selected)| *selected && arg == flag)
        {
            index += 2;
            continue;
        }
        if !flags
            .iter()
            .any(|(flag, selected)| *selected && arg.starts_with(&format!("{flag}=")))
        {
            kept.push(quote_argument(arg));
        }
        index += 1;
    }
    kept.join(" ")
}

#[test]
fn explicit_selection_preserves_other_native_arguments() {
    let args = selection_args(
        "--model old --effort=low --mcp-config 'a b.json' --verbose",
        true,
        true,
    );
    assert_eq!(
        crate::agent::inject::split_extra_args(Some(&args)),
        ["--mcp-config", "a b.json", "--verbose"]
    );
    let args = selection_args("--model old --effort low", false, true);
    assert_eq!(
        crate::agent::inject::split_extra_args(Some(&args)),
        ["--model", "old"]
    );
}

/// The name each agent registers the audit MCP server under, which prefixes every tool it exposes.
///
/// DeepSeek limits a function name to 64 characters and hashes anything longer, so the long default
/// prefix would rename tools the workflow names explicitly, such as
/// `record_codex_security_candidate_validations`. A shorter server name keeps every scan tool intact.
pub fn mcp_server_name(agent: &str) -> &'static str {
    if agent == "deepseek" {
        "vlx-security"
    } else {
        "vlx-codex-security"
    }
}

pub fn launch_args(agent: &str, plugin: &Path, state: &Path, existing: &str) -> Result<String> {
    let verifier = std::env::current_exe().map_err(|_| "security_verifier_unavailable")?;
    let server = serde_json::json!({
        "command":"node", "args":[plugin.join("vlx-security-mcp.mjs"), plugin.join("mcp/server.mjs"), verifier, "--stdio"],
        "env":{"CODEX_SECURITY_STATE_DIR":state,"PYTHONDONTWRITEBYTECODE":"1"}
    });
    match agent {
        "claude" => {
            std::fs::create_dir_all(state).map_err(|e| e.to_string())?;
            let config = state.join("mcp.json");
            std::fs::write(
                &config,
                serde_json::to_vec_pretty(&serde_json::json!({
                    "mcpServers":{"vlx-codex-security":server}
                }))
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            Ok(format!(
                "{existing} --mcp-config {}",
                quote_argument(&config.to_string_lossy())
            ))
        }
        "codex" => {
            let toml_string = |v: &str| serde_json::to_string(v).map_err(|e| e.to_string());
            let command = format!(
                "mcp_servers.vlx-codex-security={{command=\"node\",args=[{},{},{},\"--stdio\"],env={{CODEX_SECURITY_STATE_DIR={},PYTHONDONTWRITEBYTECODE=\"1\"}},startup_timeout_sec=120,tool_timeout_sec=600}}",
                toml_string(&plugin.join("vlx-security-mcp.mjs").to_string_lossy())?,
                toml_string(&plugin.join("mcp/server.mjs").to_string_lossy())?,
                toml_string(&verifier.to_string_lossy())?,
                toml_string(&state.to_string_lossy())?
            );
            // Codex also loads a Codex Security plugin installed from its own marketplace, under the server
            // name `codex-security`. Offered both, an agent once called that copy and started a separate deep
            // scan the audit could neither see nor stop; the plugin's own workers switch it off the same way.
            // A bare `enabled=false` is rejected as an entry without a transport, so it names one as they do.
            Ok(format!(
                "{existing} -c {} -c {} -c {}",
                quote_argument(&command),
                quote_argument("mcp_servers.codex-security.command=\"node\""),
                quote_argument("mcp_servers.codex-security.enabled=false")
            ))
        }
        "deepseek" => {
            // DeepSeek Harness composes its profile from patch layers; an inserted MCP client entry adds
            // the server for this session only. JSON is valid YAML, so the patch is written as JSON.
            std::fs::create_dir_all(state).map_err(|e| e.to_string())?;
            let patch = state.join("dsh-mcp.json");
            std::fs::write(
                &patch,
                serde_json::to_vec_pretty(&serde_json::json!([{"insert":[{
                    "id":"mcp-vlx-security", "name":"@deepseek-ai/dsh-mcp-client",
                    "config":{"serverName":mcp_server_name(agent), "transport":"stdio",
                        "command":server["command"], "args":server["args"], "env":server["env"],
                        "toolCallTimeoutMs":600_000, "failOnStartupError":true}
                }]}]))
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            Ok(format!("{existing} --patch {}", quote_argument(&patch.to_string_lossy())))
        }
        _ => Err("security_invalid:agent".into()),
    }
}
