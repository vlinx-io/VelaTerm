//! Local Markdown notebooks. The filesystem owns content; SQLite stores registration and rebuildable indexes.
mod files;
mod imports;
mod links;
#[cfg(test)]
mod tests;
mod transfer;

use crate::host::AppCtx;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};

type Result<T> = std::result::Result<T, String>;
const MAX_NOTE: u64 = 5 * 1024 * 1024;
const MAX_ASSET: u64 = 100 * 1024 * 1024;
const MAX_FILES: usize = 50_000;
fn now() -> i64 {
    crate::memory::now()
}
fn err<E: std::fmt::Display>(e: E) -> String {
    format!("kb_io:{e}")
}
fn required<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| s.len() <= 8192)
        .ok_or_else(|| "kb_invalid".into())
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(include_str!("schema.sql")).map_err(err)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vault {
    id: String,
    name: String,
    root: String,
}
fn vaults(app: &AppCtx) -> Result<Vec<Vault>> {
    let conn = app.db().conn.lock().map_err(err)?;
    let mut stmt = conn
        .prepare("SELECT id,name,root FROM kb_vaults WHERE id NOT IN (SELECT vault_id FROM kb_closed_vaults) ORDER BY created_at,id")
        .map_err(err)?;
    let result = stmt
        .query_map([], |r| {
            Ok(Vault {
                id: r.get(0)?,
                name: r.get(1)?,
                root: r.get(2)?,
            })
        })
        .map_err(err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(err)?;
    Ok(result)
}
fn vault(app: &AppCtx, id: &str) -> Result<Vault> {
    vaults(app)?
        .into_iter()
        .find(|v| v.id == id)
        .ok_or_else(|| "kb_not_found".into())
}
fn root(v: &Vault) -> Result<PathBuf> {
    let path = Path::new(&v.root)
        .canonicalize()
        .map_err(|_| "kb_unavailable")?;
    if !path.is_dir() || path.to_string_lossy() != v.root {
        return Err("kb_unavailable".into());
    }
    Ok(path)
}
fn relative(raw: &str, empty: bool) -> Result<PathBuf> {
    if raw.is_empty() {
        return if empty {
            Ok(PathBuf::new())
        } else {
            Err("kb_invalid_path".into())
        };
    }
    if raw.contains(['\\', '\0', ':']) || raw.len() > 4096 {
        return Err("kb_invalid_path".into());
    }
    let path = Path::new(raw);
    for part in path.components() {
        let Component::Normal(name) = part else {
            return Err("kb_invalid_path".into());
        };
        let name = name.to_str().ok_or("kb_invalid_path")?;
        if name.starts_with('.')
            || name.ends_with(['.', ' '])
            || name
                .chars()
                .any(|c| c.is_control() || "<>\"|?*".contains(c))
        {
            return Err("kb_invalid_path".into());
        }
    }
    Ok(path.to_path_buf())
}
fn path(v: &Vault, raw: &str, empty: bool) -> Result<PathBuf> {
    let base = root(v)?;
    let rel = relative(raw, empty)?;
    let mut current = base.clone();
    for part in rel.components() {
        current.push(part);
        match current.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => return Err("kb_invalid_path".into()),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(err(e)),
        }
    }
    Ok(current)
}
fn register(app: &AppCtx, args: &Value) -> Result<Value> {
    let selected = required(args, "root")?;
    let directory = Path::new(selected);
    if args["create"].as_bool() == Some(true) {
        if !directory.is_absolute() || directory.exists() {
            return Err("kb_exists".into());
        }
        std::fs::create_dir(directory).map_err(err)?;
    }
    let canonical = directory.canonicalize().map_err(|_| "kb_unavailable")?;
    if !canonical.is_dir() || canonical.parent().is_none() {
        return Err("kb_invalid_path".into());
    }
    let root = canonical.to_string_lossy().into_owned();
    let name = canonical
        .file_name()
        .ok_or("kb_invalid_path")?
        .to_string_lossy()
        .into_owned();
    let conn = app.db().conn.lock().map_err(err)?;
    conn.execute(
        "INSERT OR IGNORE INTO kb_vaults(id,name,root,created_at) VALUES(?1,?2,?3,?4)",
        params![uuid::Uuid::new_v4().to_string(), name, root, now()],
    )
    .map_err(err)?;
    let id: String = conn
        .query_row("SELECT id FROM kb_vaults WHERE root=?1", [root], |r| {
            r.get(0)
        })
        .map_err(err)?;
    conn.execute("DELETE FROM kb_closed_vaults WHERE vault_id=?1", [&id])
        .map_err(err)?;
    Ok(json!({"id":id}))
}

/// Rename a knowledge base by renaming its root folder on disk. The registration follows the folder,
/// so the list name always matches the directory that is actually open.
fn rename(app: &AppCtx, args: &Value) -> Result<Value> {
    let v = vault(app, required(args, "vaultId")?)?;
    let name = required(args, "name")?.trim();
    let rel = relative(name, false)?;
    if rel.components().count() != 1 {
        return Err("kb_invalid_path".into());
    }
    let current = root(&v)?;
    let parent = current.parent().ok_or("kb_invalid_path")?;
    let destination = parent.join(name);
    if destination == current {
        return Ok(json!({"id":v.id,"name":v.name,"root":v.root}));
    }
    // symlink_metadata also catches a dangling symlink, which `exists` would miss and rename over.
    if destination.symlink_metadata().is_ok() {
        return Err("kb_exists".into());
    }
    std::fs::rename(&current, &destination).map_err(err)?;
    let canonical = destination.canonicalize().map_err(|_| "kb_unavailable")?;
    let new_root = canonical.to_string_lossy().into_owned();
    let new_name = canonical
        .file_name()
        .ok_or("kb_invalid_path")?
        .to_string_lossy()
        .into_owned();
    let conn = app.db().conn.lock().map_err(err)?;
    conn.execute(
        "UPDATE kb_vaults SET name=?2,root=?3 WHERE id=?1",
        params![v.id, new_name, new_root],
    )
    .map_err(err)?;
    Ok(json!({"id":v.id,"name":new_name,"root":new_root}))
}

/// Reuse the existing transport's filesystem policy for both registered and newly selected roots.
pub fn guard_paths(
    app: &AppCtx,
    cmd: &str,
    args: &Value,
    guard: impl Fn(&str) -> Result<()>,
) -> Result<()> {
    if cmd == "kb_register" {
        guard(required(args, "root")?)?;
    }
    if let Some(id) = args.get("vaultId").and_then(Value::as_str) {
        guard(&vault(app, id)?.root)?;
    }
    // A vault-scoped `kb_search` is already covered by the branch above; only a vault-less
    // search spans every root.
    let scoped = args.get("vaultId").and_then(Value::as_str).is_some();
    if cmd == "kb_overview" || (cmd == "kb_search" && !scoped) {
        for v in vaults(app)? {
            guard(&v.root)?;
        }
    }
    Ok(())
}
pub fn dispatch(app: &AppCtx, cmd: &str, args: &Value) -> Result<Value> {
    // Serialize index refresh and filesystem mutations across application windows.
    static OPERATIONS: OnceLock<Mutex<()>> = OnceLock::new();
    let _lock = OPERATIONS
        .get_or_init(Default::default)
        .lock()
        .map_err(err)?;
    let started = std::time::Instant::now();
    let audited = !matches!(
        cmd,
        "kb_overview"
            | "kb_tree"
            | "kb_get"
            | "kb_stat"
            | "kb_search"
            | "kb_asset"
            | "kb_import_chunk"
            | "kb_imports"
            | "kb_import_detail"
    );
    let operation = uuid::Uuid::new_v4().to_string();
    let step = if cmd.starts_with("kb_import") {
        "import"
    } else if cmd == "kb_register" {
        "mount"
    } else {
        "write"
    };
    if audited {
        crate::diagnostics::record(
            "INFO",
            "knowledge",
            json!({"operationId":operation,"vaultId":args["vaultId"],"command":cmd,"step":step,"method":"program","status":"started","inputCount":args["files"].as_array().map(Vec::len).unwrap_or(1)}),
        );
    }
    let result = dispatch_inner(app, cmd, args);
    if audited {
        crate::diagnostics::record(
            if result.is_ok() { "INFO" } else { "WARN" },
            "knowledge",
            json!({"operationId":operation,"vaultId":args["vaultId"],"command":cmd,"step":step,"method":"program","status":if result.is_ok(){"completed"}else{"failed"},"errorCode":result.as_ref().err().map(|e|e.split(':').next().unwrap_or("kb_io")),"outputCount":result.as_ref().ok().and_then(|v|v["imported"].as_u64()).unwrap_or(u64::from(result.is_ok())),"durationMs":started.elapsed().as_millis() as u64}),
        );
    }
    if result.is_ok()
        && matches!(
            cmd,
            "kb_register"
                | "kb_unregister"
                | "kb_vault_rename"
                | "kb_create"
                | "kb_save"
                | "kb_move"
                | "kb_trash"
                | "kb_restore"
                | "kb_favorite"
                | "kb_import_commit"
                | "kb_import_delete"
        )
    {
        app.emit("kb://changed", ());
    }
    result
}
fn dispatch_inner(app: &AppCtx, cmd: &str, args: &Value) -> Result<Value> {
    match cmd {
        "kb_overview" => Ok(
            json!({"vaults":vaults(app)?,"limits":{"noteBytes":MAX_NOTE,"assetBytes":MAX_ASSET,"files":MAX_FILES,"chunkBytes":1024*1024},"defaultRoot":crate::host::home_dir().map(|p|p.join("Notes")).map(|p|p.to_string_lossy().into_owned()),"home":crate::host::home_dir().map(|p|p.to_string_lossy().into_owned())}),
        ),
        "kb_register" => register(app, args),
        "kb_search" => files::search(app, args),
        _ => {
            let v = vault(app, required(args, "vaultId")?)?;
            match cmd {
                "kb_unregister" => {
                    let conn = app.db().conn.lock().map_err(err)?;
                    let tx = conn.unchecked_transaction().map_err(err)?;
                    tx.execute(
                        "INSERT OR IGNORE INTO kb_closed_vaults(vault_id) VALUES(?1)",
                        [&v.id],
                    )
                    .map_err(err)?;
                    tx.execute("DELETE FROM kb_files WHERE vault_id=?1", [&v.id])
                        .map_err(err)?;
                    tx.commit().map_err(err)?;
                    Ok(json!(null))
                }
                "kb_vault_rename" => rename(app, args),
                "kb_tree" => files::tree(app, &v, args),
                "kb_get" => files::get(app, &v, required(args, "path")?),
                "kb_stat" => {
                    let p = path(&v, required(args, "path")?, false)?;
                    Ok(json!({"digest":hash(&files::read_bytes(&p,MAX_NOTE)?)}))
                }
                "kb_save" => files::save(app, &v, args),
                "kb_create" => files::create(app, &v, args),
                "kb_move" => files::move_path(app, &v, args),
                "kb_trash" => files::trash(app, &v, required(args, "path")?),
                "kb_restore" => files::restore(app, &v, required(args, "id")?),
                "kb_favorite" => {
                    let p = required(args, "path")?;
                    path(&v, p, false)?;
                    let conn = app.db().conn.lock().map_err(err)?;
                    if args["favorite"].as_bool() == Some(true) {
                        conn.execute(
                            "INSERT OR IGNORE INTO kb_favorites(vault_id,path) VALUES(?1,?2)",
                            params![v.id, p],
                        )
                        .map_err(err)?;
                    } else {
                        conn.execute(
                            "DELETE FROM kb_favorites WHERE vault_id=?1 AND path=?2",
                            params![v.id, p],
                        )
                        .map_err(err)?;
                    }
                    Ok(json!(null))
                }
                "kb_asset" => transfer::asset(&v, args),
                "kb_import_begin" => transfer::begin(app, &v, args),
                "kb_import_chunk" => transfer::chunk(app, &v, args),
                "kb_import_commit" => transfer::commit(app, &v, args),
                "kb_import_abort" => transfer::abort(app, &v, args),
                "kb_imports" => imports::list(app, &v, args),
                "kb_import_detail" => imports::detail(app, &v, args),
                "kb_import_delete" => imports::delete(app, &v, args),
                _ => Err("kb_invalid".into()),
            }
        }
    }
}
