//! Projects, collections, groups and sessions in the left sidebar tree.
//!
//! Sidebar changes are checked in order on one copy of the tree (`tree.rs`), so a request can create a
//! group and move sessions into it, then sort them. Items are named by ID, an ID prefix, a unique exact
//! name, or a `$name` given to an item created earlier in the request.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use serde_json::{json, Value};
use super::tree::{self, Kind, Model};
use super::{path, text, AppCtx, CallOrigin, Check, Domain, Problem, Result};

pub(super) const DOMAIN: Domain = Domain {
    id: "projects",
    guide: GUIDE,
    context,
    check: Check::Tree(validate),
    backend: &["importProject", "createCollection", "moveToCollection", "createGroup", "rename", "move", "sort", "archiveSession", "deleteNode"],
    run,
};

const GUIDE: &str = "Collections (sidebar folders of projects, not on disk), projects (folders on disk), groups inside a project, and sessions (terminals and agent conversations). Collections marked builtIn are fixed: they cannot be renamed, deleted or moved. Tasks and Chat hold sessions only; projects and collections belong under Projects.
Name an item by its full id, an id prefix of at least 6 characters, its exact name when that is unique, or \"$name\" for an item created earlier in the same request with \"as\": \"name\". The domain may be omitted for these actions.
Changes:
- {\"kind\":\"importProject\",\"path\":\"absolute or ~/ path\",\"collectionId\":\"collection\",\"as\":\"p1\"} or {\"kind\":\"importProject\",\"name\":\"folder name to look for\"}; use name when the user gives only a project name, and the user picks among the folders found.
- {\"kind\":\"createCollection\",\"name\":\"...\",\"parentId\":\"collection\",\"as\":\"c1\"}
- {\"kind\":\"moveToCollection\",\"projectId\":\"project or collection\",\"collectionId\":\"collection, or null for the top of Projects\"}
- {\"kind\":\"createGroup\",\"projectId\":\"project\",\"parentGroupId\":\"group\",\"name\":\"...\",\"as\":\"g1\"}
- {\"kind\":\"rename\",\"id\":\"any item\",\"name\":\"...\"}
- {\"kind\":\"move\",\"id\":\"group or session\",\"groupId\":\"group, or null for the project root\",\"parentSessionId\":\"session (sessions only)\",\"projectId\":\"another project's root\",\"order\":1000}
  Omit groupId, parentSessionId and projectId to stay in place. Without order the item goes last.
- {\"kind\":\"sort\",\"groupId\":\"group\" or \"projectId\":\"project root\" or \"parentSessionId\":\"session\",\"by\":\"name\"}
  or {\"kind\":\"sort\",\"groupId\":\"...\",\"order\":[\"ids, first to last\"]}. by name orders numbers by value (2 before 10); sessions and subgroups are sorted separately; items left out of order keep their relative order after the listed ones.
- {\"kind\":\"archiveSession\",\"id\":\"session\"}
- {\"kind\":\"deleteNode\",\"id\":\"any item\"} removes it from the sidebar; files on disk are kept.
Opening a form or view (the user finishes it there):
- {\"kind\":\"createProject\",\"name\":\"new folder name\",\"parentDir\":\"...\",\"collectionId\":\"...\"} creates an empty folder and imports it.
- {\"kind\":\"cloneProject\",\"url\":\"git URL\",\"parentDir\":\"...\",\"folderName\":\"...\",\"branch\":\"...\"}
- {\"kind\":\"newSession\",\"projectId\":\"...\",\"groupId\":\"...\",\"sessionKind\":\"one of agentKinds\"}
- {\"kind\":\"openSession\",\"sessionId\":\"...\"}
- {\"kind\":\"revealProject\",\"projectId\":\"...\"}";

const AGENT_KINDS: &[&str] = &["terminal", "claude", "codex", "opencode", "copilot", "cursor", "antigravity", "cline", "pi", "omp", "crush", "kimi", "kiro", "grok", "zoo", "zcode", "deepseek"];
/// Folders macOS guards with a privacy prompt; they are searched only when the user names a path inside them.
const PRIVATE_DIRS: &[&str] = &["Documents", "Desktop", "Downloads", "Library", "Pictures", "Movies", "Music", "Public", "Applications"];
const SKIPPED_DIRS: &[&str] = &["node_modules", "target", "build", "dist", "vendor", ".git"];
const PLACES: &[Kind] = &[Kind::Project, Kind::Collection];

/// Collections and projects. Groups and sessions are listed per project by `vapp session list`, so a
/// large tree does not crowd out the other areas.
fn context(app: &AppCtx, _origin: CallOrigin) -> Result<Value> {
    let (collections, projects) = tree::load(app)?.places();
    Ok(json!({"collections": collections, "projects": projects, "agentKinds": AGENT_KINDS,
        "note": "Run `vapp session list <project>` for a project's groups and sessions."}))
}

fn same_path(a: &str, b: &str) -> bool {
    let canonical = |p: &str| std::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p));
    canonical(a) == canonical(b)
}

/// Looks for folders with this name below the usual project locations, nearest and exact matches first.
fn search(model: &Model, name: &str) -> Vec<String> {
    let Some(home) = crate::host::home_dir() else { return Vec::new() };
    let needle = name.to_lowercase();
    let mut roots = Vec::new();
    // Folders that already hold imported projects are the most likely places for another one.
    for project in model.projects() {
        if let Some(parent) = Path::new(&project.path).parent() { roots.push((parent.to_path_buf(), 1)); }
    }
    for dir in ["Projects", "projects", "Code", "code", "src", "Developer", "dev", "work", "workspace", "repos", "git"] { roots.push((home.join(dir), 2)); }
    roots.push((home.clone(), 1));
    let (mut exact, mut partial, mut seen, mut budget) = (Vec::new(), Vec::new(), HashSet::new(), 20_000usize);
    fn walk(dir: &Path, depth: usize, home: &Path, needle: &str, out: (&mut Vec<String>, &mut Vec<String>), seen: &mut HashSet<PathBuf>, budget: &mut usize) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let (exact, partial) = out;
        let mut children = Vec::new();
        for entry in entries.flatten() {
            if *budget == 0 { return; }
            *budget -= 1;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || SKIPPED_DIRS.contains(&name.as_str()) || (dir == home && PRIVATE_DIRS.contains(&name.as_str())) { continue; }
            if !entry.file_type().is_ok_and(|t| t.is_dir()) { continue; }
            let path = entry.path();
            let lower = name.to_lowercase();
            if (lower == needle || lower.contains(needle)) && seen.insert(std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone())) {
                let text = path.to_string_lossy().into_owned();
                if lower == needle { exact.push(text) } else { partial.push(text) }
            }
            children.push(path);
        }
        if depth > 1 { for child in children { walk(&child, depth - 1, home, needle, (&mut *exact, &mut *partial), seen, budget); } }
    }
    let mut visited = HashSet::new();
    for (root, depth) in roots {
        if !visited.insert(root.clone()) { continue; }
        walk(&root, depth, &home, &needle, (&mut exact, &mut partial), &mut seen, &mut budget);
    }
    exact.into_iter().chain(partial).take(8).collect()
}

/// A collection a project or collection may go into: Tasks and Chat hold sessions only.
fn collection(model: &Model, raw: &Value, key: &'static str) -> std::result::Result<Option<(String, String)>, Problem> {
    let Some(found) = model.optional(raw, key, &[Kind::Collection])? else { return Ok(None) };
    if matches!(found.id.as_str(), crate::db::system_collections::TASKS | crate::db::system_collections::CHAT) { return Err((key, "invalid")); }
    Ok(Some((found.id.clone(), model.label(found))))
}

fn validate(app: &AppCtx, raw: &Value, model: &mut Model, index: usize, origin: CallOrigin) -> std::result::Result<Value, Problem> {
    let named = |key: &'static str, max: usize| text(&raw[key], max).map_err(|r| (key, r))?.ok_or((key, "missing"));
    let folder = |key: &'static str| path(app, &raw[key], origin).map_err(|r| (key, r));
    match raw["kind"].as_str().unwrap_or_default() {
        "importProject" => {
            let candidates = match folder("path")? {
                Some(p) => { if !Path::new(&p).is_dir() { return Err(("path", "notFound")); } vec![p] }
                // Searching the disk by name is a host convenience; remote clients name the path.
                None if origin == CallOrigin::Local => search(model, &named("name", 255)?),
                None => return Err(("path", "invalidPath")),
            };
            let candidates = candidates.into_iter().filter(|c| !model.projects().any(|p| same_path(&p.path, c))).collect::<Vec<_>>();
            if candidates.is_empty() { return Err(if raw["path"].is_null() { ("name", "notFound") } else { ("path", "exists") }); }
            let collection = collection(model, raw, "collectionId")?;
            let id = tree::reference(model, raw, index)?;
            // With several folders found the user picks one; the name shown until then is what was asked for.
            let name = match candidates.as_slice() {
                [only] => Path::new(only).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| only.clone()),
                _ => named("name", 255)?,
            };
            model.add_project(id.clone(), name, candidates[0].clone(), collection.as_ref().map(|c| c.0.clone()));
            Ok(json!({"candidates": candidates, "collectionId": collection.as_ref().map(|c| &c.0), "collectionName": collection.as_ref().map(|c| &c.1), "ref": id, "as": raw["as"]}))
        }
        "createProject" => {
            let collection = collection(model, raw, "collectionId")?;
            Ok(json!({"name": named("name", 255)?, "parentDir": folder("parentDir")?, "collectionId": collection.as_ref().map(|c| &c.0), "collectionName": collection.as_ref().map(|c| &c.1)}))
        }
        "cloneProject" => {
            let url = named("url", 2000)?;
            if !["https://", "http://", "ssh://", "git://", "git@", "file://"].iter().any(|p| url.starts_with(p)) { return Err(("url", "invalid")); }
            let optional = |key: &'static str| text(&raw[key], 255).map_err(|r| (key, r));
            Ok(json!({"url": url, "parentDir": folder("parentDir")?, "folderName": optional("folderName")?, "branch": optional("branch")?}))
        }
        "newSession" => {
            let project = model.resolve(&raw["projectId"], PLACES, "projectId")?;
            let group = model.optional(raw, "groupId", &[Kind::Group])?;
            if group.is_some_and(|g| g.project.as_deref() != Some(project.id.as_str())) { return Err(("groupId", "invalid")); }
            let kind = super::one_of(&raw["sessionKind"], AGENT_KINDS, Some("terminal")).map_err(|r| ("sessionKind", r))?;
            Ok(json!({"projectId": project.id, "projectName": model.label(project), "groupId": group.map(|g| &g.id), "groupName": group.map(|g| model.label(g)), "sessionKind": kind}))
        }
        "openSession" => {
            let session = model.resolve(raw.get("sessionId").filter(|v| !v.is_null()).unwrap_or(&raw["id"]), &[Kind::Session], "sessionId")?;
            Ok(json!({"sessionId": session.id, "sessionName": session.name, "where": model.label(session)}))
        }
        "revealProject" => {
            let project = model.resolve(&raw["projectId"], PLACES, "projectId")?;
            Ok(json!({"projectId": project.id, "projectName": model.label(project)}))
        }
        _ => tree::validate(model, raw, index),
    }
}

fn run(app: &AppCtx, action: &Value, args: &Value, origin: CallOrigin) -> Result<Value> {
    if action["kind"] != "importProject" { return tree::run(app, action, origin); }
    let candidates = action["candidates"].as_array().cloned().unwrap_or_default();
    // A single folder needs no choice, so it can be confirmed with the changes around it.
    let choice = match (args["choice"].as_str(), candidates.as_slice()) {
        (Some(choice), _) => choice.to_string(),
        (None, [only]) => only.as_str().unwrap_or_default().to_string(),
        (None, _) => return Err("Choose a folder to import".into()),
    };
    if !candidates.iter().any(|c| c == choice.as_str()) { return Err("Choose one of the listed folders".into()); }
    super::call(app, "import_project", json!({"rootPath": choice, "collectionId": action["collectionId"]}), origin)
}
