//! Opt-in tests of the argv received by a native process, rather than only the generated shell text.
//! Set VLX_TEST_PWSH to a pwsh executable and run this module with `cargo test ... -- --ignored`.

use super::*;
use std::path::{Path, PathBuf};
use std::process::Command;

struct NativeProbe {
    root: PathBuf,
    executable: PathBuf,
}

impl NativeProbe {
    fn new() -> Self {
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target"));
        let root = target.join(format!("pwsh-args-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("argv_probe.rs");
        std::fs::write(
            &source,
            r#"fn main() {
                let mut output = Vec::new();
                for argument in std::env::args().skip(1) {
                    output.extend_from_slice(argument.as_bytes());
                    output.push(0);
                }
                std::fs::write(std::env::var_os("VLX_TEST_ARGV_OUTPUT").unwrap(), output).unwrap();
            }"#,
        )
        .unwrap();
        let executable = root.join(if cfg!(windows) {
            "argv-probe.exe"
        } else {
            "argv-probe"
        });
        let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let compiled = Command::new(compiler)
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("rustc is required to build the native argv probe");
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        Self { root, executable }
    }

    fn copy_as(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        std::fs::copy(&self.executable, &path).unwrap();
        path
    }

    fn assert_argv(
        &self,
        shell: &str,
        setup: &str,
        launch: &str,
        env: &[(String, String)],
        expected: &[String],
    ) {
        let output = self.root.join("arguments.bin");
        let _ = std::fs::remove_file(&output);
        // Run the same launch fragment used by a PTY, after a simulated profile sets its preference.
        // Deliberately shadow a parent variable to ensure the launcher keeps its state in a child scope.
        let script = format!(
            "Set-StrictMode -Version Latest; $ErrorActionPreference = 'Stop'; \
             {setup}; \
             $vlxLegacyArguments = 'parent'; \
             $beforeMode = [string](Get-Variable PSNativeCommandArgumentPassing -ValueOnly -ErrorAction SilentlyContinue); \
             {launch}; \
             if ($vlxLegacyArguments -cne 'parent') {{ throw 'Launcher variable escaped its scope' }}; \
             $afterMode = [string](Get-Variable PSNativeCommandArgumentPassing -ValueOnly -ErrorAction SilentlyContinue); \
             if ($beforeMode -cne $afterMode) {{ throw 'Launcher changed the argument preference' }}"
        );
        let mut command = Command::new(shell);
        command.args(["-NoLogo", "-NoProfile", "-NonInteractive"]);
        if cfg!(windows) {
            command.args(["-ExecutionPolicy", "Bypass"]);
        }
        let result = command
            .arg("-Command")
            .arg(script)
            .env("POWERSHELL_TELEMETRY_OPTOUT", "1")
            .env("VLX_TEST_ARGV_OUTPUT", &output)
            .envs(env.iter().map(|(key, value)| (key, value)))
            .output()
            .unwrap_or_else(|error| panic!("could not execute {shell}: {error}"));
        assert!(
            result.status.success(),
            "PowerShell failed for {setup}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let bytes = std::fs::read(&output).expect("the launcher did not invoke the native probe");
        let actual: Vec<String> = bytes
            .strip_suffix(&[0])
            .expect("the native probe must terminate its last argument")
            .split(|byte| *byte == 0)
            .map(|argument| String::from_utf8(argument.to_vec()).unwrap())
            .collect();
        assert_eq!(actual, expected, "argv mismatch for {setup}");
    }
}

impl Drop for NativeProbe {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn shell_capabilities(shell: &str) -> ((u32, u32), bool) {
    let result = Command::new(shell)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$PSVersionTable.PSVersion.ToString(); $EnabledExperimentalFeatures",
        ])
        .env("POWERSHELL_TELEMETRY_OPTOUT", "1")
        .output()
        .unwrap_or_else(|error| panic!("PowerShell is required; set VLX_TEST_PWSH: {error}"));
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let text = String::from_utf8(result.stdout).unwrap();
    let mut lines = text.lines();
    let mut version = lines.next().unwrap().trim().split('.');
    (
        (
            version.next().unwrap().parse().unwrap(),
            version.next().unwrap().parse().unwrap(),
        ),
        lines.any(|line| line.trim() == "PSNativeCommandArgumentPassing"),
    )
}

fn path_text(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
#[ignore = "requires pwsh and rustc; set VLX_TEST_PWSH and run with --ignored"]
fn pwsh_native_arguments_round_trip() {
    let shell = std::env::var("VLX_TEST_PWSH").unwrap_or_else(|_| "pwsh".into());
    let (version, experimental) = shell_capabilities(&shell);
    let setups = if version >= (7, 3) {
        vec![
            "$null = 0",
            "$PSNativeCommandArgumentPassing = 'Legacy'",
            "$PSNativeCommandArgumentPassing = 'Standard'",
            "$PSNativeCommandArgumentPassing = 'Windows'",
            "Remove-Variable PSNativeCommandArgumentPassing -ErrorAction SilentlyContinue",
        ]
    } else if experimental {
        vec![
            "$null = 0",
            "$PSNativeCommandArgumentPassing = 'Legacy'",
            "$PSNativeCommandArgumentPassing = 'Standard'",
            "Remove-Variable PSNativeCommandArgumentPassing -ErrorAction SilentlyContinue",
        ]
    } else {
        vec!["$null = 0"]
    };
    let probe = NativeProbe::new();
    let endpoint = HookEndpoint {
        port: 38147,
        token: "argv-test-token".into(),
    };
    let values = vec![
        build_claude_settings(&endpoint, "argv-test-session"),
        serde_json::json!({
            "path": "C:\\Program Files\\VelaTerm\\",
            "text": "say \"hello world\"; $(literal) `literal` & | %PATH% 中文\nnext line",
        })
        .to_string(),
    ];
    let notify = build_codex_notify(r"C:\Program Files\VelaTerm\vela.exe");
    let effort = "model_reasoning_effort=\"high\"".to_string();
    let args = format!(
        "--settings {} -c notify={} -c {}",
        value_ref(ShellKind::Pwsh, "VLX_TEST_SETTINGS"),
        value_ref(ShellKind::Pwsh, "VLX_TEST_NOTIFY"),
        value_ref(ShellKind::Pwsh, "VLX_TEST_EFFORT"),
    );

    let quoted_path = probe.copy_as(if cfg!(windows) {
        "argv probe [literal] 'quoted'.exe"
    } else {
        "argv probe [literal] 'quoted'"
    });
    let ps1 = probe.root.join("argv-wrapper.ps1");
    std::fs::write(
        &ps1,
        format!("& '{}' @args", sq_pwsh(path_text(&probe.executable))),
    )
    .unwrap();

    // Native executables renamed to .cmd/.bat exercise PowerShell's automatic Legacy selection on
    // Unix too. They intentionally do not claim coverage of cmd.exe's additional batch-file parsing.
    let fallback_names = [
        "argv-probe.cmd",
        "argv-probe.bat",
        "cmd.exe",
        "argv probe [literal] 'quoted'.cmd",
    ];
    let fallback_paths: Vec<_> = fallback_names
        .iter()
        .map(|name| probe.copy_as(name))
        .collect();

    for setup in setups {
        for value in &values {
            let env = vec![
                ("VLX_TEST_SETTINGS".into(), value.clone()),
                ("VLX_TEST_NOTIFY".into(), notify.clone()),
                ("VLX_TEST_EFFORT".into(), effort.clone()),
            ];
            let expected = vec![
                "--settings".into(),
                value.clone(),
                "-c".into(),
                format!("notify={notify}"),
                "-c".into(),
                effort.clone(),
            ];
            for path in [&probe.executable, &quoted_path, &ps1] {
                let launch = launch_cmd_at(ShellKind::Pwsh, path_text(path), &args);
                probe.assert_argv(&shell, setup, &launch, &env, &expected);
            }
            let alias_setup = format!(
                "{setup}; Set-Alias claude '{}'",
                sq_pwsh(path_text(&probe.executable))
            );
            probe.assert_argv(
                &shell,
                &alias_setup,
                &launch_cmd(ShellKind::Pwsh, "claude", &args),
                &env,
                &expected,
            );
            // On Windows, .cmd/.bat files are interpreted by cmd.exe, so a renamed binary is not a
            // faithful batch-file fixture. Real Windows wrapper checks belong in their own test.
            if !cfg!(windows) {
                for path in &fallback_paths {
                    let alias_setup =
                        format!("{setup}; Set-Alias claude '{}'", sq_pwsh(path_text(path)));
                    probe.assert_argv(
                        &shell,
                        setup,
                        &launch_cmd_at(ShellKind::Pwsh, path_text(path), &args),
                        &env,
                        &expected,
                    );
                    probe.assert_argv(
                        &shell,
                        &alias_setup,
                        &launch_cmd(ShellKind::Pwsh, "claude", &args),
                        &env,
                        &expected,
                    );
                }
            }
        }
        // Exercise the production preparation entry point with both structured settings and a prompt.
        let spawn = prepare_with_args(
            SessionKind::Claude,
            &shell,
            "/unused/vela",
            &endpoint,
            "argv-test-session",
            None,
            false,
            Some("review this project"),
            None,
            Some(path_text(&probe.executable)),
            None,
        );
        probe.assert_argv(
            &shell,
            setup,
            &spawn.launch.unwrap(),
            &spawn.env,
            &[
                "--settings".into(),
                values[0].clone(),
                "review this project".into(),
            ],
        );

        // Model/effort fragments are generated outside inject.rs and must share the launcher's scope.
        let selection = crate::agent::session_settings::Selection {
            model: Some("example/model".into()),
            effort: Some("high".into()),
        };
        let (extra, selection_env) = crate::agent::session_settings::terminal_args(
            SessionKind::Codex,
            ShellKind::Pwsh,
            None,
            &selection,
        );
        let mut spawn = prepare_with_args_capabilities(
            SessionKind::Codex,
            &shell,
            r"C:\Program Files\VelaTerm\vela.exe",
            &endpoint,
            "argv-test-session",
            None,
            false,
            None,
            Some(&extra),
            Some(path_text(&probe.executable)),
            None,
            None,
            false,
        );
        spawn.env.extend(selection_env);
        probe.assert_argv(
            &shell,
            setup,
            &spawn.launch.unwrap(),
            &spawn.env,
            &[
                "-c".into(),
                format!("notify={notify}"),
                "--no-alt-screen".into(),
                "--model".into(),
                "example/model".into(),
                "-c".into(),
                effort.clone(),
            ],
        );
    }
}
