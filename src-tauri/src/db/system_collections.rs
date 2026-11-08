//! Fixed sidebar collections. Projects, Tasks and Chat are ordinary collection rows (`root_path = ''`) with
//! well-known IDs, created at startup and protected from deletion, renaming and moving. Every folder project
//! and user collection lives under Projects; task and chat sessions are created directly in Tasks and Chat.
//! The sidebar also shows Agents and Terminals, which are runtime lists in the frontend with no row here.

use rusqlite::{params, Connection, OptionalExtension};

pub const PROJECTS: &str = "vlx-system-projects";
pub const TASKS: &str = "vlx-system-tasks";
pub const CHAT: &str = "vlx-system-chat";

/// ID, stored name and sort order. The sidebar shows translated names; the stored name is what other
/// clients and the CLI see.
const ALL: [(&str, &str, i64); 3] = [(PROJECTS, "Projects", -30), (CHAT, "Chats", -20), (TASKS, "Tasks", -10)];

pub fn is_system(id: &str) -> bool {
    ALL.iter().any(|(system, _, _)| *system == id)
}

/// Whether the Projects collection exists. Databases opened by tests and by commands that never call
/// `ensure` keep the old top-level behavior.
pub fn projects_collection(conn: &Connection) -> Result<Option<&'static str>, String> {
    let found = conn
        .query_row("SELECT 1 FROM projects WHERE id = ?1 AND deleted_at IS NULL", [PROJECTS], |_| Ok(()))
        .optional()
        .map_err(|e| format!("Failed to read the Projects collection: {e}"))?;
    Ok(found.map(|_| PROJECTS))
}

/// Where a project without a destination goes: Projects when it exists, otherwise the top level.
pub fn default_parent<'a>(conn: &Connection, project_id: &str, wanted: Option<&'a str>) -> Result<Option<&'a str>, String> {
    if wanted.is_some() || is_system(project_id) { return Ok(wanted); }
    projects_collection(conn)
}

/// Reject operations that would remove, rename or move a fixed collection.
pub fn guard(id: &str) -> Result<(), String> {
    if is_system(id) { Err("This collection is built in and cannot be changed".into()) } else { Ok(()) }
}

/// Tasks and Chat hold sessions only. A project placed there would carry its sessions out of the type
/// the collection stands for.
pub fn guard_destination(conn: &Connection, collection_id: Option<&str>) -> Result<(), String> {
    let mut current = collection_id.map(str::to_string);
    let mut seen = std::collections::HashSet::new();
    while let Some(id) = current {
        if id == TASKS || id == CHAT { return Err("Projects can only be placed under Projects".into()); }
        if !seen.insert(id.clone()) { break; }
        current = conn
            .query_row("SELECT collection_id FROM projects WHERE id = ?1", [&id], |r| r.get::<_, Option<String>>(0))
            .optional()
            .map_err(|e| format!("Failed to read collection: {e}"))?
            .flatten();
    }
    Ok(())
}

/// Create the fixed collections and keep them at the top level, then gather every other top-level project
/// and collection under Projects. Runs at every startup, so projects created at the top level by an older
/// client sharing this database are gathered too.
pub fn ensure(conn: &Connection) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| format!("Failed to begin fixed collection setup: {e}"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    for (id, name, order) in ALL {
        tx.execute(
            "INSERT OR IGNORE INTO projects (id, name, root_path, sort_order, collapsed, created_at) VALUES (?1, ?2, '', ?3, 0, ?4)",
            params![id, name, order, now],
        )
        .map_err(|e| format!("Failed to create the {name} collection: {e}"))?;
        tx.execute(
            "UPDATE projects SET name = ?2, root_path = '', collection_id = NULL, deleted_at = NULL, sort_order = ?3 WHERE id = ?1",
            params![id, name, order],
        )
        .map_err(|e| format!("Failed to restore the {name} collection: {e}"))?;
    }
    tx.execute(
        "UPDATE projects SET collection_id = ?1 WHERE collection_id IS NULL AND id NOT IN (?1, ?2, ?3)",
        params![PROJECTS, TASKS, CHAT],
    )
    .map_err(|e| format!("Failed to move projects under Projects: {e}"))?;
    tx.commit().map_err(|e| format!("Failed to commit fixed collection setup: {e}"))
}
