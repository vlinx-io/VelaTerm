//! Files on disk: open one in a document tab, or show it in the system file manager.

use std::path::Path;
use serde_json::{json, Value};
use super::{Check, path, AppCtx, CallOrigin, Domain, Result};

pub(super) const DOMAIN: Domain = Domain {
    id: "files",
    guide: GUIDE,
    context,
    check: Check::Each(validate),
    backend: &[],
    run,
};

const GUIDE: &str = "Files on disk. Paths are absolute or start with ~/.
Actions:
- {\"kind\":\"openFile\",\"path\":\"...\"} opens a file in a document tab.
- {\"kind\":\"reveal\",\"path\":\"...\"} shows a file or folder in the system file manager.";

fn context(_app: &AppCtx, _origin: CallOrigin) -> Result<Value> {
    Ok(json!({"home": crate::host::home_dir().map(|p| p.to_string_lossy().into_owned())}))
}

fn validate(app: &AppCtx, raw: &Value, _context: &Value, origin: CallOrigin) -> std::result::Result<Value, &'static str> {
    let target = path(app, &raw["path"], origin)?.ok_or("invalidPath")?;
    if !Path::new(&target).exists() { return Err("notFound"); }
    match raw["kind"].as_str().unwrap_or_default() {
        "openFile" => {
            if Path::new(&target).is_dir() { return Err("notFile"); }
            Ok(json!({"path": target}))
        }
        "reveal" => Ok(json!({"path": target})),
        _ => Err("unsupported"),
    }
}

fn run(_app: &AppCtx, _action: &Value, _args: &Value, _origin: CallOrigin) -> Result<Value> { Err("File actions run in the app window".into()) }
