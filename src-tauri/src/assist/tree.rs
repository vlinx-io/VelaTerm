//! The sidebar tree behind the projects area: collections, projects, groups and sessions.
//!
//! Validation replays a whole request on an in-memory copy of the tree, so a later action may name an item
//! an earlier one creates (`"as": "g1"`, then `"$g1"`), renames or moves. The plan records those names, and
//! each one is replaced by the real ID once the action that creates it has run.

use std::cmp::Ordering;
use serde_json::{json, Map, Value};
use crate::db::system_collections;
use crate::models::Tree;
use super::{call, text, AppCtx, CallOrigin, Problem, Result};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind { Collection, Project, Group, Session }

impl Kind {
    pub(super) fn as_str(self) -> &'static str {
        match self { Kind::Collection => "collection", Kind::Project => "project", Kind::Group => "group", Kind::Session => "session" }
    }
    /// The kind the tree commands take: collections are projects without a folder.
    fn node_kind(self) -> &'static str {
        match self { Kind::Collection | Kind::Project => "project", Kind::Group => "group", Kind::Session => "session" }
    }
}

/// One tree item. For groups `group` is the parent group; for sessions it is the containing group.
#[derive(Clone, Debug)]
pub(super) struct Node {
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub project: Option<String>,
    pub group: Option<String>,
    parent_session: Option<String>,
    pub collection: Option<String>,
    sort_order: i64,
    pub path: String,
    /// A session's agent kind, for listings.
    agent: Value,
    /// Whether a live PTY or chat process is currently backing this session.
    alive: bool,
}

pub(crate) struct Model { nodes: Vec<Node> }

impl Model {
    fn new(tree: Tree, app: &AppCtx) -> Self {
        let mut nodes = Vec::new();
        for p in tree.projects {
            let kind = if p.root_path.is_empty() { Kind::Collection } else { Kind::Project };
            nodes.push(Node { id: p.id, kind, name: p.name, project: None, group: None, parent_session: None,
                collection: p.collection_id, sort_order: p.sort_order, path: p.root_path, agent: Value::Null, alive: false });
        }
        for g in tree.groups {
            nodes.push(Node { id: g.id, kind: Kind::Group, name: g.name, project: Some(g.project_id), group: g.parent_group_id,
                parent_session: None, collection: None, sort_order: g.sort_order, path: String::new(), agent: Value::Null, alive: false });
        }
        for s in tree.sessions {
            let alive = s.kind != crate::models::SessionKind::Browser
                && (app.pty().is_running(&s.id) || app.chat().is_alive(&s.id));
            nodes.push(Node { id: s.id, kind: Kind::Session, name: s.name, project: Some(s.project_id), group: s.group_id,
                parent_session: s.parent_session_id, collection: None, sort_order: s.sort_order, path: String::new(), agent: json!(s.kind), alive });
        }
        Model { nodes }
    }

    pub(super) fn get(&self, id: &str) -> Option<&Node> { self.nodes.iter().find(|n| n.id == id) }
    fn get_mut(&mut self, id: &str) -> Option<&mut Node> { self.nodes.iter_mut().find(|n| n.id == id) }

    /// Projects in the sidebar, with their folders.
    pub(super) fn projects(&self) -> impl Iterator<Item = &Node> { self.nodes.iter().filter(|n| n.kind == Kind::Project) }

    /// Records a project the request imports, so later actions can name it.
    pub(super) fn add_project(&mut self, id: String, name: String, path: String, collection: Option<String>) {
        self.nodes.push(Node { id, kind: Kind::Project, name, project: None, group: None, parent_session: None, collection, sort_order: 0, path, agent: Value::Null, alive: false });
    }

    /// Collections and projects for `vapp catalog`. Groups and sessions are listed per project by `list`.
    pub(super) fn places(&self) -> (Vec<Value>, Vec<Value>) {
        let collections = self.nodes.iter().filter(|n| n.kind == Kind::Collection).map(|n| {
            let mut item = json!({"id": n.id, "name": n.name, "parentId": n.collection});
            if system_collections::is_system(&n.id) { item["builtIn"] = json!(true); }
            item
        }).collect();
        let projects = self.projects().map(|n| json!({"id": n.id, "name": n.name, "path": n.path, "collectionId": n.collection})).collect();
        (collections, projects)
    }

    /// Finds an item by id, `$name`, id prefix or unique exact name, among the given kinds.
    pub(super) fn resolve(&self, value: &Value, kinds: &[Kind], field: &'static str) -> std::result::Result<&Node, Problem> {
        let wanted = value.as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or((field, "missing"))?;
        let allowed = |n: &&Node| kinds.contains(&n.kind);
        if let Some(node) = self.get(wanted) {
            return if kinds.contains(&node.kind) { Ok(node) } else { Err((field, "wrongKind")) };
        }
        if wanted.starts_with('$') { return Err((field, "unknownItem")); }
        fn unique<'a>(matches: Vec<&'a Node>, field: &'static str) -> std::result::Result<Option<&'a Node>, Problem> {
            match matches.len() { 0 => Ok(None), 1 => Ok(Some(matches[0])), _ => Err((field, "ambiguous")) }
        }
        if wanted.chars().count() >= 6 {
            if let Some(node) = unique(self.nodes.iter().filter(allowed).filter(|n| n.id.starts_with(wanted)).collect(), field)? { return Ok(node); }
        }
        unique(self.nodes.iter().filter(allowed).filter(|n| n.name.trim() == wanted).collect(), field)?.ok_or((field, "unknownItem"))
    }

    pub(super) fn optional(&self, raw: &Value, key: &'static str, kinds: &[Kind]) -> std::result::Result<Option<&Node>, Problem> {
        match &raw[key] { Value::Null => Ok(None), Value::String(s) if s.trim().is_empty() => Ok(None), v => self.resolve(v, kinds, key).map(Some) }
    }

    /// Readable location, e.g. `Course / Ancient / 1.Prehistory`.
    pub(super) fn label(&self, node: &Node) -> String {
        let mut parts = vec![node.name.clone()];
        let parent_of = |n: &Node| match n.kind {
            Kind::Collection | Kind::Project => n.collection.clone(),
            Kind::Group => n.group.clone().or(n.project.clone()),
            Kind::Session => n.parent_session.clone().or(n.group.clone()).or(n.project.clone()),
        };
        let mut current = parent_of(node);
        // Collections nest; the walk stops at the top level, or after a bounded depth if the data loops.
        for _ in 0..64 {
            let Some(id) = current else { break };
            let Some(parent) = self.get(&id) else { break };
            if parent.kind == Kind::Collection && system_collections::is_system(&parent.id) { break; }
            parts.push(parent.name.clone());
            current = parent_of(parent);
        }
        parts.reverse();
        parts.join(" / ")
    }

    pub(super) fn container_label(&self, project: &str, group: Option<&str>, parent_session: Option<&str>) -> String {
        let id = parent_session.or(group).unwrap_or(project);
        self.get(id).map(|n| self.label(n)).unwrap_or_default()
    }

    fn is_group_within(&self, group: &str, ancestor: &str) -> bool {
        let mut current = Some(group.to_string());
        for _ in 0..256 {
            let Some(id) = current else { return false };
            if id == ancestor { return true; }
            current = self.get(&id).and_then(|g| g.group.clone());
        }
        true
    }

    fn is_session_within(&self, session: &str, ancestor: &str) -> bool {
        let mut current = Some(session.to_string());
        for _ in 0..256 {
            let Some(id) = current else { return false };
            if id == ancestor { return true; }
            current = self.get(&id).and_then(|s| s.parent_session.clone());
        }
        true
    }

    fn is_collection_within(&self, collection: &str, ancestor: &str) -> bool {
        let mut current = Some(collection.to_string());
        for _ in 0..256 {
            let Some(id) = current else { return false };
            if id == ancestor { return true; }
            current = self.get(&id).and_then(|c| c.collection.clone());
        }
        true
    }

    /// Direct children of a container, groups and sessions separately, in their current order.
    fn children(&self, project: &str, group: Option<&str>, parent_session: Option<&str>) -> (Vec<&Node>, Vec<&Node>) {
        let mut groups = if parent_session.is_some() { Vec::new() } else {
            self.nodes.iter().filter(|n| n.kind == Kind::Group && n.project.as_deref() == Some(project) && n.group.as_deref() == group).collect::<Vec<_>>()
        };
        let mut sessions = self.nodes.iter().filter(|n| n.kind == Kind::Session && n.project.as_deref() == Some(project)
            && n.group.as_deref() == group && n.parent_session.as_deref() == parent_session).collect::<Vec<_>>();
        groups.sort_by_key(|n| n.sort_order);
        sessions.sort_by_key(|n| n.sort_order);
        (groups, sessions)
    }

    fn remove(&mut self, id: &str) {
        // Descendants go with the item, so later actions cannot name them.
        let mut doomed = vec![id.to_string()];
        let mut index = 0;
        while index < doomed.len() {
            let current = doomed[index].clone();
            for n in &self.nodes {
                let child = n.project.as_deref() == Some(current.as_str()) || n.group.as_deref() == Some(current.as_str())
                    || n.parent_session.as_deref() == Some(current.as_str());
                if child && !doomed.contains(&n.id) { doomed.push(n.id.clone()); }
            }
            index += 1;
        }
        self.nodes.retain(|n| !doomed.contains(&n.id));
    }
}

pub(super) fn load(app: &AppCtx) -> Result<Model> { Ok(Model::new(crate::command_core::list_tree(app)?, app)) }

/// Compares names so that embedded numbers order by value: `Lesson 2` before `Lesson 10`.
pub(super) fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut digits = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) { digits.push(c); it.next(); }
                    digits
                };
                let (x, y) = (take(&mut a), take(&mut b));
                let (tx, ty) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                let order = tx.len().cmp(&ty.len()).then_with(|| tx.cmp(ty)).then_with(|| x.len().cmp(&y.len()));
                if order != Ordering::Equal { return order; }
            }
            (Some(x), Some(y)) => {
                let order = x.to_lowercase().cmp(y.to_lowercase());
                if order != Ordering::Equal { return order; }
                a.next(); b.next();
            }
        }
    }
}

/// The `$name` an action registers for the item it creates; anonymous items get an index-based one.
pub(super) fn reference(model: &Model, raw: &Value, index: usize) -> std::result::Result<String, Problem> {
    match raw["as"].as_str() {
        None => Ok(format!("$#{index}")),
        Some(name) => {
            let valid = !name.is_empty() && name.len() <= 40 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            if !valid { return Err(("as", "invalid")); }
            let id = format!("${name}");
            if model.get(&id).is_some() { return Err(("as", "exists")); }
            Ok(id)
        }
    }
}

fn name(raw: &Value) -> std::result::Result<String, Problem> {
    text(&raw["name"], 255).map_err(|r| ("name", r))?.ok_or(("name", "missing"))
}

/// Checks one sidebar action against the tree copy and applies it there, so later actions see the change.
pub(super) fn validate(model: &mut Model, raw: &Value, index: usize) -> std::result::Result<Value, Problem> {
    const PLACES: &[Kind] = &[Kind::Project, Kind::Collection];
    const ANY: &[Kind] = &[Kind::Collection, Kind::Project, Kind::Group, Kind::Session];
    match raw["kind"].as_str().unwrap_or_default() {
        "createCollection" => {
            let name = name(raw)?;
            let parent = model.optional(raw, "parentId", &[Kind::Collection])?.map(|p| (p.id.clone(), model.label(p)));
            if parent.as_ref().is_some_and(|(id, _)| matches!(id.as_str(), system_collections::TASKS | system_collections::CHAT)) { return Err(("parentId", "invalid")); }
            let wanted = name.to_lowercase();
            if model.nodes.iter().any(|n| n.kind == Kind::Collection && n.name.trim().to_lowercase() == wanted) { return Err(("name", "exists")); }
            let id = reference(model, raw, index)?;
            model.nodes.push(Node { id: id.clone(), kind: Kind::Collection, name: name.clone(), project: None, group: None, parent_session: None,
                collection: parent.as_ref().map(|p| p.0.clone()), sort_order: 0, path: String::new(), agent: Value::Null, alive: false });
            Ok(json!({"name": name, "parentId": parent.as_ref().map(|p| &p.0), "parentName": parent.as_ref().map(|p| &p.1), "ref": id, "as": raw["as"]}))
        }
        "moveToCollection" => {
            let project = model.resolve(&raw["projectId"], PLACES, "projectId")?.clone();
            if system_collections::is_system(&project.id) { return Err(("projectId", "builtIn")); }
            let collection = model.optional(raw, "collectionId", &[Kind::Collection])?.cloned();
            if let Some(c) = &collection {
                if project.kind == Kind::Collection && model.is_collection_within(&c.id, &project.id) { return Err(("collectionId", "cycle")); }
                if matches!(c.id.as_str(), system_collections::TASKS | system_collections::CHAT) { return Err(("collectionId", "invalid")); }
            }
            let action = json!({"projectId": project.id, "projectName": model.label(&project), "collectionId": collection.as_ref().map(|c| &c.id),
                "collectionName": collection.as_ref().map(|c| model.label(c))});
            if let Some(node) = model.get_mut(&project.id) { node.collection = collection.map(|c| c.id); }
            Ok(action)
        }
        "createGroup" => {
            let name = name(raw)?;
            let project = model.resolve(&raw["projectId"], PLACES, "projectId")?.clone();
            let parent = model.optional(raw, "parentGroupId", &[Kind::Group])?.cloned();
            if parent.as_ref().is_some_and(|g| g.project.as_deref() != Some(project.id.as_str())) { return Err(("parentGroupId", "invalid")); }
            let id = reference(model, raw, index)?;
            let action = json!({"projectId": project.id, "parentGroupId": parent.as_ref().map(|g| &g.id),
                "where": model.container_label(&project.id, parent.as_ref().map(|g| g.id.as_str()), None), "name": name, "ref": id, "as": raw["as"]});
            model.nodes.push(Node { id, kind: Kind::Group, name, project: Some(project.id), group: parent.map(|g| g.id), parent_session: None,
                collection: None, sort_order: i64::MAX, path: String::new(), agent: Value::Null, alive: false });
            Ok(action)
        }
        "rename" => {
            let node = model.resolve(&raw["id"], ANY, "id")?.clone();
            if system_collections::is_system(&node.id) { return Err(("id", "builtIn")); }
            let name = name(raw)?;
            if node.kind == Kind::Collection {
                let wanted = name.to_lowercase();
                if model.nodes.iter().any(|n| n.kind == Kind::Collection && n.id != node.id && n.name.trim().to_lowercase() == wanted) { return Err(("name", "exists")); }
            }
            let action = json!({"nodeKind": node.kind.as_str(), "id": node.id, "currentName": node.name, "where": model.label(&node), "name": name});
            if let Some(n) = model.get_mut(&node.id) { n.name = name; }
            Ok(action)
        }
        "move" => validate_move(model, raw),
        "sort" => validate_sort(model, raw),
        "killProcess" => {
            let node = model.resolve(raw.get("id").filter(|v| !v.is_null()).unwrap_or(&raw["sessionId"]), &[Kind::Session], "id")?.clone();
            if node.agent == "browser" { return Err(("id", "wrongKind")); }
            if !node.alive { return Err(("id", "notRunning")); }
            let action = json!({"id": node.id, "nodeName": node.name, "where": model.label(&node)});
            if let Some(n) = model.get_mut(&node.id) { n.alive = false; }
            Ok(action)
        }
        "archiveSession" => {
            let node = model.resolve(raw.get("id").filter(|v| !v.is_null()).unwrap_or(&raw["sessionId"]), &[Kind::Session], "id")?.clone();
            let action = json!({"id": node.id, "nodeName": node.name, "where": model.label(&node)});
            model.remove(&node.id);
            Ok(action)
        }
        "deleteNode" => {
            let node = model.resolve(&raw["id"], ANY, "id")?.clone();
            if system_collections::is_system(&node.id) { return Err(("id", "builtIn")); }
            let action = json!({"nodeKind": node.kind.as_str(), "id": node.id, "nodeName": node.name, "where": model.label(&node)});
            model.remove(&node.id);
            Ok(action)
        }
        _ => Err(("kind", "unsupported")),
    }
}

fn order(raw: &Value) -> std::result::Result<Option<i64>, Problem> {
    match &raw["order"] { Value::Null => Ok(None), v => v.as_i64().map(Some).ok_or(("order", "invalid")) }
}

fn validate_move(model: &mut Model, raw: &Value) -> std::result::Result<Value, Problem> {
    let node = model.resolve(&raw["id"], &[Kind::Group, Kind::Session], "id")?.clone();
    let object = raw.as_object().cloned().unwrap_or_default();
    let present = |key: &str| object.contains_key(key);
    let order = order(raw)?;
    let (project, group, parent_session) = if present("parentSessionId") && !raw["parentSessionId"].is_null() {
        if node.kind != Kind::Session { return Err(("parentSessionId", "wrongKind")); }
        let parent = model.resolve(&raw["parentSessionId"], &[Kind::Session], "parentSessionId")?.clone();
        if model.is_session_within(&parent.id, &node.id) { return Err(("parentSessionId", "cycle")); }
        (parent.project.clone().unwrap_or_default(), parent.group.clone(), Some(parent.id))
    } else if present("groupId") && model.optional(raw, "groupId", &[Kind::Group])?.is_some() {
        let target = model.optional(raw, "groupId", &[Kind::Group])?.cloned().unwrap();
        if node.kind == Kind::Group && model.is_group_within(&target.id, &node.id) { return Err(("groupId", "cycle")); }
        (target.project.clone().unwrap_or_default(), Some(target.id), None)
    } else if present("groupId") || (present("projectId") && !raw["projectId"].is_null()) {
        // An explicit null group, or only a project: the project's root.
        let project = match model.optional(raw, "projectId", &[Kind::Project, Kind::Collection])? {
            Some(p) => p.id.clone(),
            None => node.project.clone().unwrap_or_default(),
        };
        (project, None, None)
    } else {
        if order.is_none() { return Err(("groupId", "missing")); }
        (node.project.clone().unwrap_or_default(), node.group.clone(), node.parent_session.clone())
    };
    let action = json!({"nodeKind": node.kind.as_str(), "id": node.id, "nodeName": node.name, "from": model.label(&node),
        "projectId": project, "groupId": group, "parentSessionId": parent_session, "order": order,
        "to": model.container_label(&project, group.as_deref(), parent_session.as_deref())});
    if node.kind == Kind::Group && node.project.as_deref() != Some(project.as_str()) {
        // A group carries its whole subtree into the other project.
        let mut moved = vec![node.id.clone()];
        let mut index = 0;
        while index < moved.len() {
            let current = moved[index].clone();
            for n in &model.nodes { if n.group.as_deref() == Some(current.as_str()) && !moved.contains(&n.id) { moved.push(n.id.clone()); } }
            index += 1;
        }
        for n in model.nodes.iter_mut().filter(|n| moved.contains(&n.id)) { n.project = Some(project.clone()); }
    }
    if let Some(n) = model.get_mut(&node.id) {
        n.project = Some(project); n.group = group; n.parent_session = parent_session;
        n.sort_order = order.unwrap_or(i64::MAX);
    }
    Ok(action)
}

fn validate_sort(model: &mut Model, raw: &Value) -> std::result::Result<Value, Problem> {
    // `target` names the container without saying its kind, as `vapp session sort` does.
    let target = model.optional(raw, "target", &[Kind::Group, Kind::Project, Kind::Collection, Kind::Session])?.cloned();
    let (project, group, parent_session) = if let Some(t) = target {
        match t.kind {
            Kind::Group => (t.project.clone().unwrap_or_default(), Some(t.id.clone()), None),
            Kind::Session => (t.project.clone().unwrap_or_default(), t.group.clone(), Some(t.id.clone())),
            _ => (t.id.clone(), None, None),
        }
    } else if let Some(s) = model.optional(raw, "parentSessionId", &[Kind::Session])? {
        (s.project.clone().unwrap_or_default(), s.group.clone(), Some(s.id.clone()))
    } else if let Some(g) = model.optional(raw, "groupId", &[Kind::Group])? {
        (g.project.clone().unwrap_or_default(), Some(g.id.clone()), None)
    } else {
        (model.resolve(&raw["projectId"], &[Kind::Project, Kind::Collection], "projectId")?.id.clone(), None, None)
    };
    let (groups, sessions) = model.children(&project, group.as_deref(), parent_session.as_deref());
    let count = groups.len() + sessions.len();
    let mut action = json!({"projectId": project, "groupId": group, "parentSessionId": parent_session, "count": count,
        "where": model.container_label(&project, group.as_deref(), parent_session.as_deref())});
    match (&raw["order"], raw["by"].as_str()) {
        (Value::Array(list), _) => {
            let mut ids = Vec::new();
            for item in list {
                let node = model.resolve(item, &[Kind::Group, Kind::Session], "order")?;
                if !groups.iter().chain(sessions.iter()).any(|c| c.id == node.id) { return Err(("order", "notChild")); }
                if !ids.contains(&node.id) { ids.push(node.id.clone()); }
            }
            action["by"] = json!("order"); action["ids"] = json!(ids);
        }
        (Value::Null, None | Some("name")) => action["by"] = json!("name"),
        _ => return Err(("by", "invalid")),
    }
    Ok(action)
}

/// Runs a confirmed sidebar action. Items an action creates are returned, so their IDs reach later actions.
pub(super) fn run(app: &AppCtx, action: &Value, origin: CallOrigin) -> Result<Value> {
    // Collections are projects without a folder in the tree commands.
    let node_kind = || if action["nodeKind"] == "collection" { json!("project") } else { action["nodeKind"].clone() };
    match action["kind"].as_str().unwrap_or_default() {
        "createCollection" => call(app, "create_virtual_project", json!({"name": action["name"], "collectionId": action["parentId"]}), origin),
        "moveToCollection" => call(app, "set_project_collection", json!({"projectId": action["projectId"], "collectionId": action["collectionId"]}), origin),
        "createGroup" => call(app, "create_group", json!({"projectId": action["projectId"], "parentGroupId": action["parentGroupId"], "name": action["name"]}), origin),
        "rename" => call(app, "rename_node", json!({"kind": node_kind(), "id": action["id"], "name": action["name"]}), origin),
        "move" => run_move(app, action, origin).map(|_| Value::Null),
        "sort" => run_sort(app, action, origin).map(|_| Value::Null),
        "killProcess" => run_kill_process(app, action, origin),
        "archiveSession" => call(app, "set_session_archived", json!({"id": action["id"], "archived": true}), origin),
        "deleteNode" => call(app, "delete_node", json!({"kind": node_kind(), "id": action["id"]}), origin),
        _ => Err("This change is not supported".into()),
    }
}

fn run_kill_process(app: &AppCtx, action: &Value, origin: CallOrigin) -> Result<Value> {
    let id = action["id"].as_str().ok_or("The session no longer exists")?;
    let session = {
        let conn = app.db().conn.lock().map_err(|e| e.to_string())?;
        crate::db::repo::get_session(&conn, id)?
    }.ok_or("The session no longer exists")?;
    if session.kind == crate::models::SessionKind::Browser {
        return Err("This session does not have a process to stop".into());
    }
    let mut stopped_chat = false;
    if session.engine == "chat" || app.chat().is_alive(id) {
        call(app, "chat_stop", json!({"sessionId": id}), origin)?;
        stopped_chat = true;
    }
    if !stopped_chat || app.pty().is_running(id) {
        // Use a dedicated source so every open client (including the desktop host) closes its pane on `pty://killed`.
        crate::web::dispatch::dispatch(
            app,
            "pty_kill",
            &json!({"sessionId": id, "reason": "close"}),
            super::ASSIST_SOURCE,
            origin,
        )?;
    }
    Ok(json!({"id": id}))
}

fn move_node(app: &AppCtx, node: &Node, project: &str, group: Option<&str>, parent_session: Option<&str>, order: i64, origin: CallOrigin) -> Result<()> {
    call(app, "move_node", json!({"kind": node.kind.node_kind(), "id": node.id, "targetProjectId": project, "targetGroupId": group,
        "targetParentSessionId": parent_session, "sortOrder": order}), origin).map(|_| ())
}

fn run_move(app: &AppCtx, action: &Value, origin: CallOrigin) -> Result<()> {
    let model = load(app)?;
    let node = model.get(action["id"].as_str().unwrap_or_default()).ok_or("The item no longer exists")?;
    let project = action["projectId"].as_str().ok_or("The destination no longer exists")?;
    let (group, parent_session) = (action["groupId"].as_str(), action["parentSessionId"].as_str());
    let order = match action["order"].as_i64() {
        Some(order) => order,
        // Last among the destination's items of the same kind.
        None => {
            let (groups, sessions) = model.children(project, group, parent_session);
            let siblings = if node.kind == Kind::Group { groups } else { sessions };
            siblings.iter().filter(|n| n.id != node.id).map(|n| n.sort_order).max().map_or(1000, |max| max.saturating_add(1000))
        }
    };
    move_node(app, node, project, group, parent_session, order, origin)
}

fn run_sort(app: &AppCtx, action: &Value, origin: CallOrigin) -> Result<()> {
    let model = load(app)?;
    let project = action["projectId"].as_str().ok_or("The project no longer exists")?;
    let (group, parent_session) = (action["groupId"].as_str(), action["parentSessionId"].as_str());
    let (groups, sessions) = model.children(project, group, parent_session);
    let listed = action["ids"].as_array().cloned().unwrap_or_default();
    let arrange = |mut items: Vec<&Node>| -> Vec<Node> {
        if action["by"] == "order" {
            let rank = |n: &Node| listed.iter().position(|id| id == n.id.as_str()).unwrap_or(usize::MAX);
            items.sort_by_key(|n| rank(n));
        } else {
            items.sort_by(|a, b| natural_cmp(&a.name, &b.name));
        }
        items.into_iter().cloned().collect()
    };
    for list in [arrange(groups), arrange(sessions)] {
        for (index, node) in list.iter().enumerate() {
            let order = (index as i64 + 1) * 1000;
            if node.sort_order == order { continue; }
            move_node(app, node, project, node.group.as_deref(), node.parent_session.as_deref(), order, origin)?;
        }
    }
    Ok(())
}

/// Groups and sessions of one project or collection, for `vapp session list`. Without a project, the
/// calling session's own project is listed.
pub(super) fn list(app: &AppCtx, project: Option<&str>, session_id: &str) -> Result<Value> {
    let model = load(app)?;
    let project = match project.map(str::trim).filter(|p| !p.is_empty()) {
        Some(wanted) => model.resolve(&json!(wanted), &[Kind::Project, Kind::Collection], "project").map_err(|(_, reason)| match reason {
            "ambiguous" => format!("Several projects match \"{wanted}\"; use its id"),
            _ => format!("No project or collection matches \"{wanted}\""),
        })?,
        None => {
            let session = model.get(session_id).filter(|n| n.kind == Kind::Session).ok_or("Name the project to list")?;
            model.get(session.project.as_deref().unwrap_or_default()).ok_or("This session's project no longer exists")?
        }
    };
    let entry = |n: &Node| {
        let mut item = Map::new();
        item.insert("id".into(), json!(n.id));
        item.insert("name".into(), json!(n.name));
        item.insert("sortOrder".into(), json!(n.sort_order));
        if n.kind == Kind::Group { item.insert("parentGroupId".into(), json!(n.group)); }
        else { item.insert("groupId".into(), json!(n.group)); item.insert("parentSessionId".into(), json!(n.parent_session)); }
        Value::Object(item)
    };
    let mut groups = model.nodes.iter().filter(|n| n.kind == Kind::Group && n.project.as_deref() == Some(project.id.as_str())).collect::<Vec<_>>();
    let mut sessions = model.nodes.iter().filter(|n| n.kind == Kind::Session && n.project.as_deref() == Some(project.id.as_str())).collect::<Vec<_>>();
    groups.sort_by_key(|n| n.sort_order);
    sessions.sort_by_key(|n| n.sort_order);
    Ok(json!({
        "project": {"id": project.id, "name": project.name, "kind": project.kind.as_str(), "path": project.path},
        "groups": groups.into_iter().map(entry).collect::<Vec<_>>(),
        "sessions": sessions.into_iter().map(|n| { let mut e = entry(n); e["kind"] = n.agent.clone(); e["alive"] = json!(n.alive); e }).collect::<Vec<_>>(),
    }))
}
