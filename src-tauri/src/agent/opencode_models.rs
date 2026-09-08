//! Model catalogue reported by an OpenCode server.
//!
//! OpenCode knows which providers have credentials and which models each offers, so the list is asked of
//! a server rather than kept here. A session that is already running lends its server; otherwise a
//! short-lived one is started for the question and stopped straight after, the way the Codex catalogue
//! is read from a throwaway app-server.

use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::agent::chat::opencode_protocol::{self, OpencodeModel, Server};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

pub fn list(app: &crate::host::AppCtx, session_id: &str, bin: &str, cwd: Option<&str>, running: Option<Arc<Server>>) -> Result<Vec<OpencodeModel>, String> {
    if let Some(server) = running {
        return fetch(&server);
    }
    let port = opencode_protocol::configured_port(app, &format!("catalog.{session_id}"))?;
    let password = opencode_protocol::random_password();
    let mut command = Command::new(bin);
    command
        .arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .arg("--hostname")
        .arg("127.0.0.1")
        .env("OPENCODE_SERVER_PASSWORD", &password)
        .env("OPENCODE_SERVER_USERNAME", "opencode")
        .env_remove(crate::agent::inject::OPENCODE_CONFIG_ENV)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    for key in crate::pty::manager::AGENT_HARNESS_MARKERS {
        command.env_remove(key);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to start OpenCode: {e}"))?;
    let server = Server::new(port, &password, cwd);
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    let result = loop {
        if server.healthy() {
            break fetch(&server);
        }
        if let Ok(Some(status)) = child.try_wait() {
            break Err(format!("OpenCode exited before answering ({status})"));
        }
        if Instant::now() > deadline {
            break Err("OpenCode's server did not start in time".to_string());
        }
        std::thread::sleep(Duration::from_millis(150));
    };
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn fetch(server: &Server) -> Result<Vec<OpencodeModel>, String> {
    let providers = server.get("/provider")?;
    Ok(opencode_protocol::parse_models(&providers))
}
