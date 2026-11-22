//! Cross-platform plugin CLI client (`--plugin-cli <plugin_id> <cli_name> [args...]`) and
//! loopback hook-server handler (`POST /plugin/<plugin_id>/cli`).

use super::manifest::is_valid_plugin_id;
use crate::host::AppCtx;
use serde_json::{json, Value};
use std::io::Read;
use std::time::{Duration, Instant};

const MAX_BODY_BYTES: usize = 256 * 1024;

/// Hidden main-program subcommand invoked by plugin CLI shims under `<data_dir>/bin/`.
///
/// Expected argv: `[exe, "--plugin-cli", plugin_id, cli_name, ...user_args]`.
pub fn run_cli(args: &[String]) -> ! {
    let Some(plugin_id) = args.get(2).map(String::as_str) else {
        eprintln!("Usage: --plugin-cli <plugin_id> <cli_name> [args...]");
        std::process::exit(2);
    };
    let Some(cli_name) = args.get(3).map(String::as_str) else {
        eprintln!("Usage: --plugin-cli <plugin_id> <cli_name> [args...]");
        std::process::exit(2);
    };
    if !is_valid_plugin_id(plugin_id) {
        eprintln!("Invalid plugin id: {plugin_id}");
        std::process::exit(2);
    }

    let port = match std::env::var("VLX_HOOK_PORT").ok().filter(|s| !s.is_empty()) {
        Some(p) => p,
        None => {
            eprintln!("{cli_name}: must run inside a VelaTerm session (VLX_HOOK_PORT not set).");
            std::process::exit(1);
        }
    };
    let token = crate::agent::session_env::token()
        .or_else(|| std::env::var("VLX_HOOK_TOKEN").ok())
        .unwrap_or_default();
    if token.is_empty() {
        eprintln!("{cli_name}: VLX_HOOK_TOKEN not found.");
        std::process::exit(1);
    }

    let cwd = std::env::current_dir()
        .ok()
        .map(|p| p.to_string_lossy().into_owned());
    let user_args: Vec<String> = args.iter().skip(4).cloned().collect();
    let url = format!("http://127.0.0.1:{port}/plugin/{plugin_id}/cli");
    let body = json!({
        "cli": cli_name,
        "args": user_args,
        "cwd": cwd,
    });

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(600))
        .build();
    let resp = agent
        .post(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string());

    match resp {
        Ok(r) => match r
            .into_string()
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<Value>(&s).map_err(|e| e.to_string()))
        {
            Ok(v) => {
                if let Some(out) = v.get("stdout").and_then(Value::as_str) {
                    print!("{out}");
                }
                if let Some(err) = v.get("stderr").and_then(Value::as_str) {
                    eprint!("{err}");
                }
                let code = v.get("exitCode").and_then(Value::as_i64).unwrap_or(0) as i32;
                std::process::exit(code);
            }
            Err(e) => {
                eprintln!("{cli_name}: failed parsing plugin CLI response: {e}");
                std::process::exit(1);
            }
        },
        Err(ureq::Error::Status(status, r)) => {
            let text = r.into_string().unwrap_or_default();
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                if let Some(err) = v.get("error").and_then(Value::as_str) {
                    eprintln!("{cli_name}: {err}");
                    std::process::exit(1);
                }
            }
            eprintln!("{cli_name}: HTTP {status}: {text}");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("{cli_name}: failed contacting VelaTerm host: {e}");
            std::process::exit(1);
        }
    }
}

/// Loopback hook-server handler for `POST /plugin/<plugin_id>/cli`.
pub fn handle_http(
    app: AppCtx,
    mut request: crate::diagnostics::HttpRequest,
    token: String,
    plugin_id: String,
) {
    let started = Instant::now();
    let method = request.method().as_str().to_string();
    let request_id = request.id().to_string();
    let authorized = request
        .headers()
        .iter()
        .any(|h| h.field.equiv("Authorization") && h.value.as_str() == format!("Bearer {token}"));

    let mut body = String::new();
    let result: Result<Value, (u16, String)> = if !authorized {
        Err((401, "plugin_cli_unauthorized".to_string()))
    } else if !is_valid_plugin_id(&plugin_id) {
        Err((400, "plugin_invalid_id".to_string()))
    } else if request.method() != &tiny_http::Method::Post {
        Err((405, "plugin_cli_method_not_allowed".to_string()))
    } else if request
        .as_reader()
        .take((MAX_BODY_BYTES + 1) as u64)
        .read_to_string(&mut body)
        .is_err()
        || body.len() > MAX_BODY_BYTES
    {
        Err((413, "plugin_cli_payload_too_large".to_string()))
    } else {
        serde_json::from_str::<Value>(&body)
            .map_err(|_| (400, "plugin_cli_invalid_json".to_string()))
            .and_then(|req| {
                let cli = req.get("cli").and_then(Value::as_str).unwrap_or("");
                let args: Vec<String> = req
                    .get("args")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                let cwd = req.get("cwd").and_then(Value::as_str).map(str::to_string);
                app.plugins()
                    .exec_cli(&plugin_id, cli, &args, cwd.as_deref())
                    .map_err(|e| (400, e))
            })
    };

    let (status, payload) = match result {
        Ok(res) => (200u16, res),
        Err((code, err)) => (code, json!({ "error": err })),
    };
    let response = tiny_http::Response::from_string(payload.to_string())
        .with_status_code(status)
        .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap());
    let _ = request.respond(response);
    crate::diagnostics::record(
        if status == 200 { "INFO" } else { "WARN" },
        "plugin_cli_http",
        json!({
            "requestId": request_id,
            "pluginId": plugin_id,
            "method": method,
            "statusCode": status,
            "durationMs": started.elapsed().as_millis() as u64,
        }),
    );
}
