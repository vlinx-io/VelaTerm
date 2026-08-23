//! Short-lived JSON-RPC client for the installed Codex app-server.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::Value;

/// Upper bound on retained stderr. Diagnostics are short, and the cap keeps a server that streams output
/// for the client's whole lifetime from growing without limit.
const STDERR_LIMIT: usize = 4096;

pub struct CodexAppServer {
    child: Child,
    input: Option<ChildStdin>,
    responses: Receiver<Result<Value, String>>,
    reader: Option<JoinHandle<()>>,
    errors: Arc<Mutex<String>>,
    error_reader: Option<JoinHandle<()>>,
    next_id: i64,
}

impl CodexAppServer {
    /// `config_overrides` hold `key=value` pairs passed as `-c` before the subcommand, matching how PTY
    /// sessions launch Codex. They must be literal TOML: this spawn has no shell to expand an environment
    /// reference such as `$VLX_CODEX_HOOKS`.
    pub fn start(bin_path: Option<&str>, config_overrides: &[String]) -> Result<Self, String> {
        let program = bin_path
            .filter(|path| !path.trim().is_empty())
            .unwrap_or("codex");
        let mut args: Vec<String> = Vec::new();
        for pair in config_overrides {
            args.push("-c".to_string());
            args.push(pair.clone());
        }
        args.extend(["app-server", "--listen", "stdio://"].map(String::from));

        #[cfg(windows)]
        let mut command = {
            let lower = program.to_ascii_lowercase();
            if lower.ends_with(".cmd") || lower.ends_with(".bat") {
                // cmd.exe reinterprets `&`, `|`, `<`, `>`, and `^` inside `/C` arguments. The hook TOML this
                // client passes contains none of them, so Rust's argument quoting survives the wrapper.
                let mut command = crate::host::command("cmd.exe");
                command.args(["/D", "/C", program]);
                command.args(&args);
                command
            } else {
                let mut command = crate::host::command(program);
                command.args(&args);
                command
            }
        };
        #[cfg(not(windows))]
        let mut command = {
            let mut command = crate::host::command(program);
            command.args(&args);
            command
        };

        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("failed to start Codex app-server: {error}"))?;
        let input = child
            .stdin
            .take()
            .ok_or("Codex app-server stdin unavailable")?;
        let output = child
            .stdout
            .take()
            .ok_or("Codex app-server stdout unavailable")?;
        let diagnostics = child
            .stderr
            .take()
            .ok_or("Codex app-server stderr unavailable")?;

        let (sender, responses) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let parsed = line
                    .map_err(|error| format!("failed to read Codex app-server response: {error}"))
                    .and_then(|line| {
                        serde_json::from_str::<Value>(&line)
                            .map_err(|error| format!("invalid Codex app-server JSON: {error}"))
                    });
                if sender.send(parsed).is_err() {
                    break;
                }
            }
        });

        let errors = Arc::new(Mutex::new(String::new()));
        let collected = Arc::clone(&errors);
        let error_reader = std::thread::spawn(move || {
            for line in BufReader::new(diagnostics).lines().map_while(Result::ok) {
                let Ok(mut buffer) = collected.lock() else {
                    return;
                };
                if buffer.len() >= STDERR_LIMIT {
                    return;
                }
                buffer.push_str(line.trim());
                buffer.push('\n');
            }
        });

        let mut server = Self {
            child,
            input: Some(input),
            responses,
            reader: Some(reader),
            errors,
            error_reader: Some(error_reader),
            next_id: 0,
        };
        server.request_with_id(
            0,
            "initialize",
            serde_json::json!({
                "clientInfo": {
                    "name": "vlx_term",
                    "title": "VelaTerm",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
            Duration::from_secs(4),
        )?;
        server.notify("initialized", serde_json::json!({}))?;
        Ok(server)
    }

    pub fn request(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        self.next_id += 1;
        self.request_with_id(self.next_id, method, params, timeout)
    }

    fn request_with_id(
        &mut self,
        id: i64,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        self.write(&serde_json::json!({ "method": method, "id": id, "params": params }))?;
        wait_for_response(&self.responses, id, timeout).map_err(|error| self.with_diagnostics(error))
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.write(&serde_json::json!({ "method": method, "params": params }))
    }

    fn write(&mut self, message: &Value) -> Result<(), String> {
        let input = self
            .input
            .as_mut()
            .ok_or("Codex app-server stdin unavailable")?;
        writeln!(input, "{message}")
            .and_then(|_| input.flush())
            .map_err(|error| format!("failed to write Codex app-server request: {error}"))
    }

    /// A refused startup reports its reason only on stderr, never as a JSON-RPC error.
    fn with_diagnostics(&self, error: String) -> String {
        match self.errors.lock() {
            Ok(errors) if !errors.trim().is_empty() => format!("{error} ({})", errors.trim()),
            _ => error,
        }
    }
}

impl Drop for CodexAppServer {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        if let Some(error_reader) = self.error_reader.take() {
            let _ = error_reader.join();
        }
    }
}

/// Whether a message is a protocol-level failure that carries no `id`.
///
/// Codex answers an unrecognized method with a JSON-RPC error that omits `id`, so matching on `id` alone
/// would discard it and let the caller wait out its whole timeout. Server-initiated notifications also omit
/// `id`, and they are distinguished by carrying `method`.
fn is_untagged_error(message: &Value) -> bool {
    message.get("id").is_none()
        && message.get("method").is_none()
        && message.get("error").is_some()
}

fn wait_for_response(
    responses: &Receiver<Result<Value, String>>,
    id: i64,
    timeout: Duration,
) -> Result<Value, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(format!(
                "timed out waiting for Codex app-server response {id}"
            ));
        }
        let message = responses
            .recv_timeout(remaining)
            .map_err(|error| format!("Codex app-server closed before response {id}: {error}"))??;
        if is_untagged_error(&message) {
            return Err(format!(
                "Codex app-server rejected the request: {}",
                message["error"]
            ));
        }
        if message.get("id").and_then(Value::as_i64) != Some(id) {
            continue;
        }
        if let Some(error) = message.get("error") {
            return Err(format!("Codex app-server request failed: {error}"));
        }
        return message
            .get("result")
            .cloned()
            .ok_or_else(|| format!("Codex app-server response {id} has no result"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untagged_error_is_reported_instead_of_timing_out() {
        let (sender, responses) = mpsc::channel();
        // Codex emits exactly this shape for a method the installed version does not implement.
        sender
            .send(Ok(serde_json::json!({
                "error": { "code": -32600, "message": "Invalid request: unknown variant `hooks/list`" }
            })))
            .expect("test channel should accept the message");

        let error = wait_for_response(&responses, 1, Duration::from_millis(50))
            .expect_err("an untagged error should fail the pending request");

        assert!(error.contains("unknown variant"), "{error}");
        assert!(!error.contains("timed out"), "{error}");
    }

    #[test]
    fn notifications_without_id_are_skipped() {
        let (sender, responses) = mpsc::channel();
        sender
            .send(Ok(
                serde_json::json!({ "method": "remoteControl/status/changed", "params": {} }),
            ))
            .expect("test channel should accept the notification");
        sender
            .send(Ok(serde_json::json!({ "id": 1, "result": { "ok": true } })))
            .expect("test channel should accept the response");

        let result = wait_for_response(&responses, 1, Duration::from_millis(50))
            .expect("the matching response should be returned");

        assert_eq!(result["ok"], serde_json::json!(true));
    }

    #[test]
    fn errors_tagged_with_the_request_id_are_reported() {
        let (sender, responses) = mpsc::channel();
        sender
            .send(Ok(serde_json::json!({
                "id": 4,
                "error": { "code": -32600, "message": "Invalid request: expected a sequence" }
            })))
            .expect("test channel should accept the message");

        let error = wait_for_response(&responses, 4, Duration::from_millis(50))
            .expect_err("a tagged error should fail the pending request");

        assert!(error.contains("expected a sequence"), "{error}");
    }
}
