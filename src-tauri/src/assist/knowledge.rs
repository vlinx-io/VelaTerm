//! Knowledge base vaults and notes. Creating, moving, deleting and importing open the knowledge base's
//! own forms; registering a folder and marking favorites run after confirmation.

use std::path::{Component, Path};
use serde_json::{json, Value};
use super::{Check, call, path, text, AppCtx, CallOrigin, Domain, Result};

pub(super) const DOMAIN: Domain = Domain {
    id: "knowledge",
    guide: GUIDE,
    context,
    check: Check::Each(validate),
    backend: &["registerVault", "favorite"],
    run,
};

const GUIDE: &str = "The knowledge base: vaults are folders of Markdown notes. Note paths are relative to their vault.
Actions:
- {\"kind\":\"openVault\",\"vaultId\":\"...\"}
- {\"kind\":\"search\",\"query\":\"...\",\"vaultId\":\"... or omit to search every vault\"}
- {\"kind\":\"openNote\",\"vaultId\":\"...\",\"path\":\"...\"}
- {\"kind\":\"createNote\",\"vaultId\":\"...\",\"name\":\"...\",\"folder\":\"...\"} and {\"kind\":\"createFolder\", same fields} open the form.
- {\"kind\":\"moveNote\",\"vaultId\":\"...\",\"path\":\"...\"} opens the move form; {\"kind\":\"deleteNote\",\"vaultId\":\"...\",\"path\":\"...\"} asks to move it to the trash.
- {\"kind\":\"favorite\",\"vaultId\":\"...\",\"path\":\"...\",\"favorite\":true}
- {\"kind\":\"importFiles\",\"vaultId\":\"...\",\"folder\":\"destination folder\"} opens the import form.
- {\"kind\":\"registerVault\",\"path\":\"existing folder\"} adds a folder as a vault; {\"kind\":\"createVault\"} opens the form for a new one.
- {\"kind\":\"openCollections\"} shows archived sessions.";

fn context(app: &AppCtx, origin: CallOrigin) -> Result<Value> {
    let overview = call(app, "kb_overview", json!({}), origin)?;
    Ok(json!({"vaults": overview["vaults"], "defaultRoot": overview["defaultRoot"]}))
}

/// A path inside the vault: relative, without parent steps.
pub(super) fn inside(value: &Value, required: bool) -> std::result::Result<Option<String>, &'static str> {
    let Some(p) = text(value, 2000)? else { return if required { Err("invalid") } else { Ok(None) } };
    let p = p.trim_matches('/').to_string();
    if Path::new(&p).components().any(|c| !matches!(c, Component::Normal(_))) { return Err("invalidPath"); }
    Ok((!p.is_empty()).then_some(p))
}

fn validate(app: &AppCtx, raw: &Value, context: &Value, origin: CallOrigin) -> std::result::Result<Value, &'static str> {
    let vault = |required: bool| -> std::result::Result<Option<Value>, &'static str> {
        let id = raw["vaultId"].as_str().unwrap_or_default();
        if id.is_empty() { return if required { Err("invalid") } else { Ok(None) }; }
        let found = context["vaults"].as_array().into_iter().flatten().find(|v| v["id"] == id).ok_or("unknownItem")?;
        Ok(Some(json!({"vaultId": found["id"], "vaultName": found["name"], "root": found["root"]})))
    };
    let mut action = vault(false)?.unwrap_or_else(|| json!({}));
    let needs_vault = |action: &Value| if action["vaultId"].is_null() { Err("invalid") } else { Ok(()) };
    match raw["kind"].as_str().unwrap_or_default() {
        "openVault" => needs_vault(&action)?,
        "search" => {
            action["query"] = json!(text(&raw["query"], 500)?.ok_or("invalid")?);
            // Search runs inside a vault page; without a named vault it opens the first one and searches all.
            if action["vaultId"].is_null() {
                let first = context["vaults"].as_array().and_then(|v| v.first()).ok_or("unavailable")?;
                action["vaultId"] = first["id"].clone(); action["allVaults"] = json!(true);
            }
        }
        "openNote" | "moveNote" | "deleteNote" | "favorite" => {
            needs_vault(&action)?;
            let note = inside(&raw["path"], true)?.ok_or("invalid")?;
            let absolute = Path::new(action["root"].as_str().unwrap_or_default()).join(&note);
            if !absolute.exists() { return Err("notFound"); }
            action["path"] = json!(note); action["absolutePath"] = json!(absolute.to_string_lossy());
            if raw["kind"] == "favorite" { action["favorite"] = json!(raw["favorite"].as_bool().unwrap_or(true)); }
        }
        "createNote" | "createFolder" => {
            needs_vault(&action)?;
            action["name"] = json!(text(&raw["name"], 255)?); action["folder"] = json!(inside(&raw["folder"], false)?);
        }
        "importFiles" => { needs_vault(&action)?; action["folder"] = json!(inside(&raw["folder"], false)?); }
        "registerVault" => {
            let root = path(app, &raw["path"], origin)?.ok_or("invalidPath")?;
            if !Path::new(&root).is_dir() { return Err("notFound"); }
            if context["vaults"].as_array().into_iter().flatten().any(|v| v["root"] == root.as_str()) { return Err("exists"); }
            action = json!({"path": root});
        }
        "createVault" | "openCollections" => action = json!({}),
        _ => return Err("unsupported"),
    }
    Ok(action)
}

fn run(app: &AppCtx, action: &Value, _args: &Value, origin: CallOrigin) -> Result<Value> {
    match action["kind"].as_str().unwrap_or_default() {
        "registerVault" => call(app, "kb_register", json!({"root": action["path"], "create": false}), origin),
        "favorite" => call(app, "kb_favorite", json!({"vaultId": action["vaultId"], "path": action["path"], "favorite": action["favorite"]}), origin),
        _ => Err("This action runs in the app window".into()),
    }
}
