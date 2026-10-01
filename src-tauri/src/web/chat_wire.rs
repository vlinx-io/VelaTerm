//! Per-connection wire codec for chat events, negotiated by the client with `{"t":"caps","chat":1}`.
//!
//! The engine publishes every changed row whole and the whole `extras` object on every change. Over a slow
//! or metered link that is quadratic for a streamed answer and constant traffic for background workflows.
//! A negotiated connection therefore receives each chat event as a patch against what exactly this
//! connection was sent last; the client decoder (`src/ipc/chatWire.ts`) rebuilds the full objects before any
//! listener sees them. Connections that never negotiate get the engine's frames untouched.
//!
//! Patch grammar (one for rows and `extras`):
//! ```text
//! ObjPatch = { "set"?: {k: v}, "del"?: [k], "app"?: {k: suffix}, "len"?: {k: utf16LengthOfBase},
//!              "sub"?: {k: ObjPatch}, "arr"?: {k: ArrPatch} }
//! ArrPatch = { "key": "id" | "task_id", "order"?: [key], "items"?: {key: ObjPatch | {"new": object}} }
//! ```
//! - `rows`: a row whose base this connection holds travels as `{"id", "patch"}`, any other row whole. Only
//!   live rows (streaming answers, running tools and shells, loading compactions) are kept as bases, at most
//!   [`MAX_RETAINED_ROWS`] per session; the event names them in `retain` and the decoder keeps exactly those.
//!   `replaceRows`, `reset` and an epoch change drop the row bases on both sides.
//! - `extras`: background tasks lose their `workflow_progress` tree (flagged `detail_omitted`) unless this
//!   connection watches `chat://task/{sid}/{taskId}`. The first frame after a watch is whole (`extras`), later
//!   ones are `patch`. A change that only moves the clock fields ([`CLOCK_FIELDS`]) sends nothing: clients
//!   advance elapsed time locally.
//!
//! Encoding happens when an event is queued, under the connection's codec lock, so patches leave in the
//! order they were computed. Negotiated frames are therefore never coalesced in the outbound queue.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::{json, Map, Value};

/// The chat codec version a client announces in `caps`; any other value leaves the connection unnegotiated.
pub(crate) const PROTOCOL_VERSION: u64 = 1;

/// Watched names (sessions plus task details) one connection may hold.
pub(crate) const MAX_WATCHED_NAMES: usize = 256;

/// Longest accepted watch name.
pub(crate) const MAX_NAME_LEN: usize = 300;

/// Live rows kept as patch bases per session.
pub(crate) const MAX_RETAINED_ROWS: usize = 64;

/// Fields that only move with the wall clock. Clients tick them locally, so a change of these alone is
/// not worth a frame; the next real change carries their current values along.
pub(crate) const CLOCK_FIELDS: [&str; 2] = ["elapsed_ms", "elapsedMs"];

/// Identity keys that let a list of objects be patched entry by entry.
const LIST_KEYS: [&str; 2] = ["id", "task_id"];

const EVENT_PREFIX: &str = "chat://event/";
const TASK_PREFIX: &str = "chat://task/";

/// A name a client may watch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WatchName {
    /// `chat://event/{sid}`: the session's chat channel.
    Session(String),
    /// `chat://task/{sid}/{taskId}`: the workflow tree of one background task.
    Task(String, String),
}

impl WatchName {
    pub(crate) fn session(&self) -> &str {
        match self {
            WatchName::Session(sid) | WatchName::Task(sid, _) => sid,
        }
    }
}

/// Whether `id` may appear in a watch name: non-empty, ASCII letters, digits and `._:-`.
pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_NAME_LEN
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
}

/// Parse and validate a watch name; anything else is ignored by the caller.
pub(crate) fn parse_name(name: &str) -> Option<WatchName> {
    if name.len() > MAX_NAME_LEN {
        return None;
    }
    if let Some(sid) = name.strip_prefix(EVENT_PREFIX) {
        return valid_id(sid).then(|| WatchName::Session(sid.to_string()));
    }
    let (sid, task) = name.strip_prefix(TASK_PREFIX)?.split_once('/')?;
    (valid_id(sid) && valid_id(task)).then(|| WatchName::Task(sid.to_string(), task.to_string()))
}

/// What this connection was sent last for one watched session.
#[derive(Default)]
struct SessionWire {
    /// Distinguishes this watch from an earlier one of the same session, so a forwarder of a previous
    /// watch that is still running on another thread cannot encode against the new state.
    generation: u64,
    epoch: Option<u64>,
    /// Live rows as last sent, by id.
    rows: BTreeMap<String, Value>,
    /// The compacted `extras` object as last sent.
    extras: Option<Value>,
    /// `extras` frames sent for this watch, so a caller can tell whether one went out since it last looked.
    extras_sent: u64,
}

/// The codec state of one connection.
#[derive(Default)]
pub(crate) struct ChatWire {
    enabled: bool,
    next_generation: u64,
    sessions: HashMap<String, SessionWire>,
    /// `(sid, taskId)` pairs whose workflow tree this connection receives.
    details: HashSet<(String, String)>,
}

impl ChatWire {
    /// The client announced the codec: from now on chat events are encoded and forwarded only for watched
    /// sessions.
    pub(crate) fn enable(&mut self) {
        self.enabled = true;
    }

    pub(crate) fn enabled(&self) -> bool {
        self.enabled
    }

    /// Watched sessions plus watched task details.
    pub(crate) fn watched_count(&self) -> usize {
        self.sessions.len() + self.details.len()
    }

    /// Whether one more watch of `name` would exceed [`MAX_WATCHED_NAMES`]; repeating a held watch never does.
    pub(crate) fn over_limit(&self, name: &WatchName) -> bool {
        let held = match name {
            WatchName::Session(sid) => self.sessions.contains_key(sid),
            WatchName::Task(sid, task) => self.details.contains(&(sid.clone(), task.clone())),
        };
        !held && self.watched_count() >= MAX_WATCHED_NAMES
    }

    /// Start a watch. Returns its generation, or None when the session is already watched.
    pub(crate) fn watch_session(&mut self, sid: &str) -> Option<u64> {
        if self.sessions.contains_key(sid) {
            return None;
        }
        self.next_generation += 1;
        let generation = self.next_generation;
        self.sessions.insert(sid.to_string(), SessionWire { generation, ..SessionWire::default() });
        Some(generation)
    }

    /// End a watch and forget what was sent; a later watch starts again with whole frames.
    pub(crate) fn unwatch_session(&mut self, sid: &str) -> bool {
        self.sessions.remove(sid).is_some()
    }

    pub(crate) fn watch_task(&mut self, sid: &str, task: &str) -> bool {
        self.details.insert((sid.to_string(), task.to_string()))
    }

    pub(crate) fn unwatch_task(&mut self, sid: &str, task: &str) -> bool {
        self.details.remove(&(sid.to_string(), task.to_string()))
    }

    /// `extras` frames encoded for the current watch of `sid`, or None when it is not watched.
    pub(crate) fn extras_sent(&self, sid: &str) -> Option<u64> {
        self.sessions.get(sid).map(|session| session.extras_sent)
    }

    /// Whether a forwarder registered for `generation` still serves the current watch of `sid`.
    pub(crate) fn is_current(&self, sid: &str, generation: u64) -> bool {
        self.sessions.get(sid).is_some_and(|session| session.generation == generation)
    }

    /// The client lost track of a session: forget every base so everything after the barrier is whole.
    /// Returns whether the session is watched (only then is a barrier worth sending).
    pub(crate) fn resync(&mut self, sid: &str) -> bool {
        match self.sessions.get_mut(sid) {
            Some(session) => {
                session.epoch = None;
                session.rows.clear();
                session.extras = None;
                true
            }
            None => false,
        }
    }

    /// Encode one chat event of `sid` for this connection. None means nothing is sent: the session is not
    /// watched, or the event changed nothing a client shows.
    pub(crate) fn encode(&mut self, sid: &str, payload: Value) -> Option<Value> {
        let Self { sessions, details, .. } = self;
        let session = sessions.get_mut(sid)?;
        match payload.get("type").and_then(Value::as_str) {
            Some("rows") => Some(session.encode_rows(payload)),
            Some("replaceRows" | "reset") => {
                session.rows.clear();
                session.epoch = payload.get("epoch").and_then(Value::as_u64).or(session.epoch);
                Some(payload)
            }
            Some("extras") => session.encode_extras(payload, |task| details.contains(&(sid.to_string(), task.to_string()))),
            _ => Some(payload),
        }
    }
}

impl SessionWire {
    fn encode_rows(&mut self, mut payload: Value) -> Value {
        let epoch = payload.get("epoch").and_then(Value::as_u64);
        if epoch != self.epoch {
            self.rows.clear();
            self.epoch = epoch;
        }
        let Some(rows) = payload.get_mut("rows").and_then(Value::as_array_mut) else {
            return payload;
        };
        for row in rows.iter_mut() {
            let Some(id) = row.get("id").and_then(Value::as_str).map(str::to_owned) else { continue };
            let full = row.clone();
            if let Some(base) = self.rows.remove(&id) {
                if let (Some(old), Some(new)) = (base.as_object(), full.as_object()) {
                    *row = json!({"id": id, "patch": diff_object(old, new)});
                }
            }
            if is_live(&full) && self.rows.len() < MAX_RETAINED_ROWS {
                self.rows.insert(id, full);
            }
        }
        if !self.rows.is_empty() {
            payload["retain"] = Value::Array(self.rows.keys().map(|id| Value::String(id.clone())).collect());
        }
        payload
    }

    fn encode_extras(&mut self, mut payload: Value, detail: impl Fn(&str) -> bool) -> Option<Value> {
        let Some(extras) = payload.as_object_mut().and_then(|p| p.remove("extras")) else {
            return Some(payload);
        };
        let compacted = compact_extras(extras, detail);
        let patch = match (self.extras.as_ref().and_then(Value::as_object), compacted.as_object()) {
            (Some(old), Some(new)) => diff_object(old, new),
            _ => {
                payload["extras"] = compacted.clone();
                self.extras = Some(compacted);
                self.extras_sent += 1;
                return Some(payload);
            }
        };
        if is_clock_only(&patch) {
            return None;
        }
        self.extras = Some(compacted);
        self.extras_sent += 1;
        payload["patch"] = patch;
        Some(payload)
    }
}

/// Rows that are still changing and therefore worth keeping as a patch base.
fn is_live(row: &Value) -> bool {
    match row.get("kind").and_then(Value::as_str) {
        Some("assistant" | "reasoning") => row.get("streaming") == Some(&Value::Bool(true)),
        Some("tool" | "shell") => row.get("status").and_then(Value::as_str) == Some("running"),
        Some("compaction") => row.get("status").and_then(Value::as_str) == Some("loading"),
        _ => false,
    }
}

/// Drop the workflow tree of every background task `detail` rejects, flagging that one existed.
pub(crate) fn compact_extras(mut extras: Value, detail: impl Fn(&str) -> bool) -> Value {
    if let Some(tasks) = extras.get_mut("backgroundTasks").and_then(Value::as_array_mut) {
        for task in tasks.iter_mut().filter_map(Value::as_object_mut) {
            let keep = task.get("task_id").and_then(Value::as_str).is_some_and(&detail);
            if !keep && task.remove("workflow_progress").is_some() {
                task.insert("detail_omitted".into(), Value::Bool(true));
            }
        }
    }
    extras
}

/// JavaScript string length of `s`: the unit the decoder checks an append base against.
pub(crate) fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// The patch that turns `old` into `new` (see the module documentation for the grammar).
pub(crate) fn diff_object(old: &Map<String, Value>, new: &Map<String, Value>) -> Value {
    let mut set = Map::new();
    let mut app = Map::new();
    let mut len = Map::new();
    let mut sub = Map::new();
    let mut arr = Map::new();
    for (key, next) in new {
        let Some(prev) = old.get(key) else {
            set.insert(key.clone(), next.clone());
            continue;
        };
        if prev == next {
            continue;
        }
        match (prev, next) {
            (Value::String(a), Value::String(b)) if !a.is_empty() && b.len() > a.len() && b.starts_with(a.as_str()) => {
                app.insert(key.clone(), Value::String(b[a.len()..].to_string()));
                len.insert(key.clone(), json!(utf16_len(a)));
            }
            (Value::Object(a), Value::Object(b)) => {
                sub.insert(key.clone(), diff_object(a, b));
            }
            (Value::Array(a), Value::Array(b)) => match diff_array(a, b) {
                Some(patch) => {
                    arr.insert(key.clone(), patch);
                }
                None => {
                    set.insert(key.clone(), next.clone());
                }
            },
            _ => {
                set.insert(key.clone(), next.clone());
            }
        }
    }
    let del: Vec<Value> = old.keys().filter(|key| !new.contains_key(*key)).map(|key| Value::String(key.clone())).collect();
    let mut patch = Map::new();
    for (name, part) in [("set", set), ("app", app), ("len", len), ("sub", sub), ("arr", arr)] {
        if !part.is_empty() {
            patch.insert(name.into(), Value::Object(part));
        }
    }
    if !del.is_empty() {
        patch.insert("del".into(), Value::Array(del));
    }
    Value::Object(patch)
}

/// The identity of every entry under `key`, when all entries are objects with distinct string values there.
fn list_keys<'a>(list: &'a [Value], key: &str) -> Option<Vec<&'a str>> {
    let mut seen = HashSet::new();
    list.iter()
        .map(|item| item.get(key).and_then(Value::as_str).filter(|id| seen.insert(*id)))
        .collect()
}

/// A keyed list patch, or None when the lists cannot be keyed (the caller then sets the whole list).
fn diff_array(old: &[Value], new: &[Value]) -> Option<Value> {
    let (key, old_keys, new_keys) = LIST_KEYS
        .iter()
        .find_map(|key| Some((*key, list_keys(old, key)?, list_keys(new, key)?)))?;
    let bases: HashMap<&str, &Value> = old_keys.iter().copied().zip(old.iter()).collect();
    let mut items = Map::new();
    for (id, item) in new_keys.iter().zip(new.iter()) {
        match bases.get(id) {
            Some(base) if *base == item => {}
            Some(base) => {
                items.insert((*id).to_string(), diff_object(base.as_object()?, item.as_object()?));
            }
            None => {
                items.insert((*id).to_string(), json!({ "new": item }));
            }
        }
    }
    let mut patch = Map::new();
    patch.insert("key".into(), Value::String(key.into()));
    if old_keys != new_keys {
        patch.insert("order".into(), json!(new_keys));
    }
    if !items.is_empty() {
        patch.insert("items".into(), Value::Object(items));
    }
    Some(Value::Object(patch))
}

/// Whether a patch changes nothing but [`CLOCK_FIELDS`] (an empty patch included).
pub(crate) fn is_clock_only(patch: &Value) -> bool {
    let Some(parts) = patch.as_object() else { return false };
    parts.iter().all(|(name, part)| match name.as_str() {
        "set" => part.as_object().is_some_and(|set| set.keys().all(|key| CLOCK_FIELDS.contains(&key.as_str()))),
        "sub" => part.as_object().is_some_and(|sub| sub.values().all(is_clock_only)),
        "arr" => part.as_object().is_some_and(|arr| {
            arr.values().all(|list| {
                list.get("order").is_none()
                    && list.get("items").and_then(Value::as_object).is_none_or(|items| {
                        items.values().all(|item| item.get("new").is_none() && is_clock_only(item))
                    })
            })
        }),
        _ => false,
    })
}

/// Reference decoder mirroring `src/ipc/chatWire.ts`, for round-trip tests.
#[cfg(test)]
pub(crate) mod reference {
    use super::*;

    pub(crate) fn apply_object(base: &Value, patch: &Value) -> Result<Value, String> {
        let mut out = base.as_object().cloned().ok_or("base is not an object")?;
        let patch = patch.as_object().ok_or("patch is not an object")?;
        if let Some(del) = patch.get("del").and_then(Value::as_array) {
            for key in del {
                out.remove(key.as_str().unwrap_or_default());
            }
        }
        if let Some(set) = patch.get("set").and_then(Value::as_object) {
            for (key, value) in set {
                out.insert(key.clone(), value.clone());
            }
        }
        if let Some(app) = patch.get("app").and_then(Value::as_object) {
            let len = patch.get("len").and_then(Value::as_object);
            for (key, suffix) in app {
                let expected = len.and_then(|len| len.get(key)).and_then(Value::as_u64);
                let current = out.get(key).and_then(Value::as_str).ok_or("append without a string base")?;
                if Some(utf16_len(current) as u64) != expected {
                    return Err(format!("append base length mismatch for {key}"));
                }
                let joined = format!("{current}{}", suffix.as_str().unwrap_or_default());
                out.insert(key.clone(), Value::String(joined));
            }
        }
        if let Some(sub) = patch.get("sub").and_then(Value::as_object) {
            for (key, inner) in sub {
                let current = out.get(key).ok_or("nested patch without a base")?;
                let next = apply_object(current, inner)?;
                out.insert(key.clone(), next);
            }
        }
        if let Some(arr) = patch.get("arr").and_then(Value::as_object) {
            for (key, list) in arr {
                let current = out.get(key).and_then(Value::as_array).ok_or("list patch without a base")?;
                let next = apply_array(current, list)?;
                out.insert(key.clone(), next);
            }
        }
        Ok(Value::Object(out))
    }

    fn apply_array(base: &[Value], patch: &Value) -> Result<Value, String> {
        let key = patch.get("key").and_then(Value::as_str).ok_or("list patch without a key")?;
        let bases: HashMap<&str, &Value> =
            base.iter().filter_map(|item| Some((item.get(key)?.as_str()?, item))).collect();
        let order: Vec<String> = match patch.get("order").and_then(Value::as_array) {
            Some(order) => order.iter().map(|id| id.as_str().unwrap_or_default().to_string()).collect(),
            None => base.iter().filter_map(|item| item.get(key)?.as_str().map(str::to_owned)).collect(),
        };
        let items = patch.get("items").and_then(Value::as_object);
        order
            .iter()
            .map(|id| match items.and_then(|items| items.get(id)) {
                Some(item) if item.get("new").is_some() => Ok(item["new"].clone()),
                Some(item) => apply_object(bases.get(id.as_str()).ok_or("list entry without a base")?, item),
                None => bases.get(id.as_str()).map(|v| (*v).clone()).ok_or_else(|| "list entry without a base".to_string()),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array)
    }

    /// Decoder state for one session, as `chatWire.ts` keeps it.
    #[derive(Default)]
    pub(crate) struct Decoder {
        rows: HashMap<String, Value>,
        epoch: Option<u64>,
        extras: Option<Value>,
    }

    impl Decoder {
        /// Rebuild the engine's payload from one encoded payload.
        pub(crate) fn decode(&mut self, payload: &Value) -> Result<Value, String> {
            let mut out = payload.clone();
            match payload.get("type").and_then(Value::as_str) {
                Some("rows") => {
                    let epoch = payload.get("epoch").and_then(Value::as_u64);
                    if epoch != self.epoch {
                        self.rows.clear();
                        self.epoch = epoch;
                    }
                    let mut working = self.rows.clone();
                    if let Some(rows) = out.get_mut("rows").and_then(Value::as_array_mut) {
                        for row in rows.iter_mut() {
                            let id = row.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
                            if row.get("kind").is_none() && row.get("patch").is_some() {
                                let base = working.get(&id).ok_or("row patch without a base")?;
                                *row = apply_object(base, &row["patch"])?;
                            }
                            working.insert(id, row.clone());
                        }
                    }
                    let retain: Vec<String> = out
                        .as_object_mut()
                        .and_then(|o| o.remove("retain"))
                        .and_then(|r| r.as_array().cloned())
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|id| id.as_str().map(str::to_owned))
                        .collect();
                    self.rows = retain.into_iter().filter_map(|id| working.remove(&id).map(|row| (id, row))).collect();
                }
                Some("replaceRows" | "reset") => {
                    self.rows.clear();
                    self.epoch = payload.get("epoch").and_then(Value::as_u64).or(self.epoch);
                }
                Some("extras") => {
                    if let Some(patch) = out.as_object_mut().and_then(|o| o.remove("patch")) {
                        let base = self.extras.as_ref().ok_or("extras patch without a base")?;
                        out["extras"] = apply_object(base, &patch)?;
                    }
                    self.extras = out.get("extras").cloned();
                }
                _ => {}
            }
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::reference::{apply_object, Decoder};
    use super::*;

    /// Deterministic pseudo-random numbers (xorshift64*), so property tests need no new dependency.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n.max(1)
        }
        fn chance(&mut self, percent: u64) -> bool {
            self.below(100) < percent
        }
        fn text(&mut self, max: u64) -> String {
            const PIECES: [&str; 8] = ["a", "b", " ", "é", "中", "😀", "\n", "z"];
            (0..self.below(max)).map(|_| PIECES[self.below(PIECES.len() as u64) as usize]).collect()
        }
    }

    fn rows_event(epoch: u64, rows: Vec<Value>) -> Value {
        json!({"type":"rows","epoch":epoch,"revision":1,"positions":{},"rows":rows})
    }

    /// The payload the client must see: the engine's payload, with `extras` compacted for this connection.
    fn expected(payload: &Value, detail: &dyn Fn(&str) -> bool) -> Value {
        let mut out = payload.clone();
        if out["type"] == "extras" {
            out["extras"] = compact_extras(out["extras"].clone(), detail);
        }
        out
    }

    /// Strip the clock fields so a suppressed clock-only change still compares equal.
    fn without_clock(value: &Value) -> Value {
        match value {
            Value::Object(map) => Value::Object(
                map.iter()
                    .filter(|(k, _)| !CLOCK_FIELDS.contains(&k.as_str()))
                    .map(|(k, v)| (k.clone(), without_clock(v)))
                    .collect(),
            ),
            Value::Array(list) => Value::Array(list.iter().map(without_clock).collect()),
            other => other.clone(),
        }
    }

    #[test]
    fn utf16_length_counts_like_javascript() {
        assert_eq!(utf16_len("abc"), 3);
        assert_eq!(utf16_len("中文"), 2);
        assert_eq!(utf16_len("😀"), 2, "an astral character is a surrogate pair");
        assert_eq!(utf16_len("é😀x"), 4);
    }

    #[test]
    fn a_growing_string_travels_as_an_append_with_its_base_length() {
        let old = json!({"text":"héllo 😀"});
        let new = json!({"text":"héllo 😀 world"});
        let patch = diff_object(old.as_object().unwrap(), new.as_object().unwrap());
        assert_eq!(patch, json!({"app":{"text":" world"},"len":{"text":8}}));
        assert_eq!(apply_object(&old, &patch).unwrap(), new);
        // A tampered base length is a gap, never a silent join.
        let tampered = json!({"app":{"text":" world"},"len":{"text":7}});
        assert!(apply_object(&old, &tampered).is_err());
        // An empty base or a rewrite is set whole.
        let patch = diff_object(json!({"t":""}).as_object().unwrap(), json!({"t":"x"}).as_object().unwrap());
        assert_eq!(patch, json!({"set":{"t":"x"}}));
        let patch = diff_object(json!({"t":"abc"}).as_object().unwrap(), json!({"t":"abd!"}).as_object().unwrap());
        assert_eq!(patch, json!({"set":{"t":"abd!"}}));
    }

    #[test]
    fn keyed_lists_patch_entries_and_reorder() {
        let old = json!({"tasks":[{"task_id":"a","n":1},{"task_id":"b","n":2}]});
        let new = json!({"tasks":[{"task_id":"b","n":2},{"task_id":"c","n":3},{"task_id":"a","n":9}]});
        let patch = diff_object(old.as_object().unwrap(), new.as_object().unwrap());
        assert_eq!(
            patch,
            json!({"arr":{"tasks":{"key":"task_id","order":["b","c","a"],"items":{"a":{"set":{"n":9}},"c":{"new":{"task_id":"c","n":3}}}}}})
        );
        assert_eq!(apply_object(&old, &patch).unwrap(), new);
        // Duplicate or missing keys fall back to setting the whole list.
        let dup = json!({"tasks":[{"task_id":"a"},{"task_id":"a","x":1}]});
        let patch = diff_object(old.as_object().unwrap(), dup.as_object().unwrap());
        assert_eq!(patch, json!({"set":{"tasks":dup["tasks"]}}));
        let plain = json!({"tasks":[1,2]});
        assert_eq!(diff_object(old.as_object().unwrap(), plain.as_object().unwrap()), json!({"set":{"tasks":[1,2]}}));
    }

    #[test]
    fn clock_only_changes_are_recognized_and_nothing_else() {
        assert!(is_clock_only(&json!({})));
        assert!(is_clock_only(&json!({"set":{"elapsed_ms":5}})));
        assert!(is_clock_only(&json!({"arr":{"t":{"key":"task_id","items":{"a":{"set":{"elapsed_ms":1},"arr":{"w":{"key":"id","items":{"x":{"set":{"elapsedMs":3}}}}}}}}}})));
        assert!(!is_clock_only(&json!({"set":{"elapsed_ms":5,"tokens":1}})));
        assert!(!is_clock_only(&json!({"del":["elapsed_ms"]})));
        assert!(!is_clock_only(&json!({"arr":{"t":{"key":"task_id","order":["a"]}}})));
        assert!(!is_clock_only(&json!({"arr":{"t":{"key":"task_id","items":{"a":{"new":{"elapsed_ms":1}}}}}})));
        assert!(!is_clock_only(&json!({"app":{"elapsed_ms":"1"},"len":{"elapsed_ms":1}})));
    }

    #[test]
    fn watch_names_are_validated() {
        assert_eq!(parse_name("chat://event/abc-1"), Some(WatchName::Session("abc-1".into())));
        assert_eq!(parse_name("chat://task/s_1/task:9.x"), Some(WatchName::Task("s_1".into(), "task:9.x".into())));
        for bad in ["chat://event/", "chat://event/a/b", "chat://event/a b", "chat://task/s", "chat://task/s/", "pty://status/s", "chat://event/ä"] {
            assert_eq!(parse_name(bad), None, "{bad}");
        }
        assert_eq!(parse_name(&format!("chat://event/{}", "a".repeat(MAX_NAME_LEN))), None);
    }

    #[test]
    fn a_streaming_row_is_sent_whole_once_then_as_appends_and_forgotten_when_done() {
        let mut wire = ChatWire::default();
        wire.enable();
        wire.watch_session("s").unwrap();
        let row = |text: &str, streaming: bool| json!({"kind":"assistant","id":"r1","text":text,"streaming":streaming,"at":1});
        let first = wire.encode("s", rows_event(7, vec![row("Hel", true)])).unwrap();
        assert_eq!(first["rows"][0], row("Hel", true));
        assert_eq!(first["retain"], json!(["r1"]));
        let second = wire.encode("s", rows_event(7, vec![row("Hello", true)])).unwrap();
        assert_eq!(second["rows"][0], json!({"id":"r1","patch":{"app":{"text":"lo"},"len":{"text":3}}}));
        let done = wire.encode("s", rows_event(7, vec![row("Hello!", false)])).unwrap();
        assert_eq!(done["rows"][0]["patch"], json!({"app":{"text":"!"},"len":{"text":5},"set":{"streaming":false}}));
        assert!(done.get("retain").is_none(), "a finished row is no longer a base");
        let again = wire.encode("s", rows_event(7, vec![row("Hello!", false)])).unwrap();
        assert_eq!(again["rows"][0], row("Hello!", false), "without a base the row travels whole");
    }

    #[test]
    fn an_epoch_change_or_replacement_drops_the_row_bases() {
        let mut wire = ChatWire::default();
        wire.watch_session("s").unwrap();
        let row = json!({"kind":"tool","id":"t","status":"running","name":"Bash"});
        wire.encode("s", rows_event(1, vec![row.clone()]));
        let next = wire.encode("s", rows_event(2, vec![row.clone()])).unwrap();
        assert_eq!(next["rows"][0], row, "a new epoch starts without bases");
        wire.encode("s", json!({"type":"replaceRows","epoch":2,"rows":[]}));
        let after = wire.encode("s", rows_event(2, vec![row.clone()])).unwrap();
        assert_eq!(after["rows"][0], row);
    }

    #[test]
    fn at_most_the_cap_of_live_rows_is_retained() {
        let mut wire = ChatWire::default();
        wire.watch_session("s").unwrap();
        let rows: Vec<Value> = (0..MAX_RETAINED_ROWS + 10)
            .map(|i| json!({"kind":"shell","id":format!("r{i:03}"),"status":"running","stdout":""}))
            .collect();
        let out = wire.encode("s", rows_event(1, rows)).unwrap();
        assert_eq!(out["retain"].as_array().unwrap().len(), MAX_RETAINED_ROWS);
    }

    #[test]
    fn unwatched_sessions_encode_to_nothing_and_a_new_watch_starts_whole() {
        let mut wire = ChatWire::default();
        assert!(wire.encode("s", json!({"type":"queued","items":[]})).is_none());
        let first = wire.watch_session("s").unwrap();
        assert_eq!(wire.watch_session("s"), None, "a second watch is a no-op");
        assert!(wire.is_current("s", first));
        let extras = json!({"type":"extras","extras":{"fastMode":false}});
        assert!(wire.encode("s", extras.clone()).unwrap().get("extras").is_some());
        assert!(wire.unwatch_session("s"));
        assert!(wire.encode("s", extras.clone()).is_none());
        let second = wire.watch_session("s").unwrap();
        assert!(!wire.is_current("s", first), "a stale forwarder cannot encode against the new watch");
        assert!(wire.is_current("s", second));
        assert!(wire.encode("s", extras).unwrap().get("extras").is_some(), "the new watch starts whole");
    }

    #[test]
    fn extras_compaction_hides_trees_unless_the_task_is_watched() {
        let mut wire = ChatWire::default();
        wire.watch_session("s").unwrap();
        let tree = json!([{"type":"workflow_agent","id":"a1","label":"x","elapsedMs":1}]);
        let extras = |elapsed: u64, tokens: u64| {
            json!({"type":"extras","extras":{"backgroundTasks":[
                {"task_id":"T","status":"running","elapsed_ms":elapsed,"usage":{"total_tokens":tokens},"workflow_progress":tree},
                {"task_id":"U","status":"running","elapsed_ms":elapsed}
            ]}})
        };
        let first = wire.encode("s", extras(1, 10)).unwrap();
        assert_eq!(first["extras"]["backgroundTasks"][0]["detail_omitted"], true);
        assert!(first["extras"]["backgroundTasks"][0].get("workflow_progress").is_none());
        assert!(first["extras"]["backgroundTasks"][1].get("detail_omitted").is_none(), "no tree, nothing omitted");
        assert!(wire.encode("s", extras(2, 10)).is_none(), "a clock-only change sends nothing");
        let tokens = wire.encode("s", extras(3, 11)).unwrap();
        assert_eq!(
            tokens["patch"],
            json!({"arr":{"backgroundTasks":{"key":"task_id","items":{
                "T":{"set":{"elapsed_ms":3},"sub":{"usage":{"set":{"total_tokens":11}}}},
                "U":{"set":{"elapsed_ms":3}}}}}}),
            "the real change carries the current clock along"
        );
        assert!(wire.watch_task("s", "T"));
        let detail = wire.encode("s", extras(3, 11)).unwrap();
        assert_eq!(
            detail["patch"],
            json!({"arr":{"backgroundTasks":{"key":"task_id","items":{"T":{"del":["detail_omitted"],"set":{"workflow_progress":tree}}}}}})
        );
        assert!(wire.unwatch_task("s", "T"));
        let hidden = wire.encode("s", extras(3, 11)).unwrap();
        assert_eq!(
            hidden["patch"],
            json!({"arr":{"backgroundTasks":{"key":"task_id","items":{"T":{"del":["workflow_progress"],"set":{"detail_omitted":true}}}}}})
        );
    }

    #[test]
    fn resync_forgets_every_base() {
        let mut wire = ChatWire::default();
        assert!(!wire.resync("s"), "an unwatched session gets no barrier");
        wire.watch_session("s").unwrap();
        let row = json!({"kind":"reasoning","id":"r","text":"a","streaming":true});
        wire.encode("s", rows_event(1, vec![row.clone()]));
        wire.encode("s", json!({"type":"extras","extras":{"fastMode":true}}));
        assert!(wire.resync("s"));
        assert_eq!(wire.encode("s", rows_event(1, vec![row.clone()])).unwrap()["rows"][0], row);
        assert!(wire.encode("s", json!({"type":"extras","extras":{"fastMode":true}})).unwrap().get("extras").is_some());
    }

    /// AC7: decode(encode(sequence)) == sequence for random row sequences.
    #[test]
    fn random_row_sequences_round_trip() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for _ in 0..200 {
            let mut wire = ChatWire::default();
            wire.watch_session("s").unwrap();
            let mut decoder = Decoder::default();
            let mut rows: Vec<Value> = Vec::new();
            let mut epoch = 1;
            for _ in 0..60 {
                if rng.chance(3) {
                    epoch += 1;
                }
                let mut changed = Vec::new();
                for _ in 0..=rng.below(3) {
                    if rows.is_empty() || rng.chance(25) {
                        let kinds = ["assistant", "reasoning", "tool", "shell", "compaction", "user"];
                        let kind = kinds[rng.below(kinds.len() as u64) as usize];
                        let row = json!({"kind":kind,"id":format!("r{}",rows.len()),"text":rng.text(4),
                            "streaming":rng.chance(70),"status":if rng.chance(60) {"running"} else {"completed"}});
                        rows.push(row.clone());
                        changed.push(row);
                    } else {
                        let i = rng.below(rows.len() as u64) as usize;
                        let row = rows[i].as_object_mut().unwrap();
                        match rng.below(6) {
                            0 | 1 => {
                                let text = format!("{}{}", row["text"].as_str().unwrap_or(""), rng.text(5));
                                row.insert("text".into(), json!(text));
                            }
                            2 => {
                                row.insert("text".into(), json!(rng.text(6)));
                            }
                            3 => {
                                row.remove("status");
                            }
                            4 => {
                                row.insert("streaming".into(), json!(rng.chance(50)));
                                row.insert("status".into(), json!(if rng.chance(50) { "running" } else { "failed" }));
                            }
                            _ => {
                                row.insert("input".into(), json!({"n": rng.below(4), "list":[{"id":"a","v":rng.below(3)}]}));
                            }
                        }
                        changed.push(Value::Object(row.clone()));
                    }
                }
                let payload = if rng.chance(4) {
                    json!({"type":"replaceRows","epoch":epoch,"rows":rows.clone()})
                } else {
                    rows_event(epoch, changed)
                };
                let encoded = wire.encode("s", payload.clone()).unwrap();
                assert_eq!(decoder.decode(&encoded).unwrap(), payload);
            }
        }
    }

    fn random_tree(rng: &mut Rng) -> Value {
        let n = rng.below(5);
        Value::Array((0..n).map(|i| json!({"type":"workflow_agent","id":format!("a{i}"),"label":rng.text(3),
            "tokens":rng.below(3),"elapsedMs":rng.below(1000)})).collect())
    }

    /// AC7: decode(encode(sequence)) == sequence for random extras sequences, clock-only changes excepted.
    #[test]
    fn random_extras_sequences_round_trip_except_suppressed_clock_changes() {
        let mut rng = Rng(0xD1B5_4A32_D192_ED03);
        for _ in 0..200 {
            let mut wire = ChatWire::default();
            wire.watch_session("s").unwrap();
            let mut decoder = Decoder::default();
            let mut tasks: Vec<Value> = Vec::new();
            let mut last_sent: Option<Value> = None;
            for step in 0..60 {
                if rng.chance(20) {
                    let task = format!("t{}", rng.below(6));
                    if rng.chance(50) { wire.watch_task("s", &task); } else { wire.unwatch_task("s", &task); }
                }
                match rng.below(8) {
                    0 => {
                        let id = format!("t{step}");
                        let at = rng.below(tasks.len() as u64 + 1) as usize;
                        tasks.insert(at, json!({"task_id":id,"status":"running","elapsed_ms":0,"description":rng.text(4)}));
                    }
                    1 if !tasks.is_empty() => {
                        let i = rng.below(tasks.len() as u64) as usize;
                        tasks.remove(i);
                    }
                    2 if tasks.len() > 1 => {
                        let a = rng.below(tasks.len() as u64) as usize;
                        let b = rng.below(tasks.len() as u64) as usize;
                        tasks.swap(a, b);
                    }
                    3 | 4 if !tasks.is_empty() => {
                        let i = rng.below(tasks.len() as u64) as usize;
                        let tree = random_tree(&mut rng);
                        let task = tasks[i].as_object_mut().unwrap();
                        if rng.chance(20) { task.remove("workflow_progress"); } else { task.insert("workflow_progress".into(), tree); }
                    }
                    5 if !tasks.is_empty() => {
                        let i = rng.below(tasks.len() as u64) as usize;
                        let task = tasks[i].as_object_mut().unwrap();
                        let text = format!("{}{}", task["description"].as_str().unwrap_or(""), rng.text(3));
                        task.insert("description".into(), json!(text));
                        if rng.chance(30) { task.remove("summary"); } else { task.insert("summary".into(), json!(rng.text(2))); }
                    }
                    _ => {
                        for task in tasks.iter_mut() {
                            let elapsed = task["elapsed_ms"].as_u64().unwrap_or(0) + 1000;
                            task["elapsed_ms"] = json!(elapsed);
                        }
                    }
                }
                let payload = json!({"type":"extras","extras":{"fastMode":false,"backgroundTasks":tasks.clone()}});
                let detail = |task: &str| wire.details.contains(&("s".to_string(), task.to_string()));
                let want = expected(&payload, &detail);
                match wire.encode("s", payload) {
                    Some(encoded) => {
                        let got = decoder.decode(&encoded).unwrap();
                        assert_eq!(got, want);
                        last_sent = Some(got);
                    }
                    None => {
                        let sent = last_sent.as_ref().expect("the first frame is never suppressed");
                        assert_eq!(without_clock(sent), without_clock(&want), "only clock fields may be withheld");
                    }
                }
            }
        }
    }

    /// The fixed event sequence behind `src/ipc/chatWire.fixture.json`: the Rust encoder must produce exactly
    /// the frames checked in there, and the TypeScript decoder test decodes the same file. Regenerate with
    /// `VLX_UPDATE_CHAT_WIRE_FIXTURE=1` after an intentional encoder change.
    fn fixture_sequence() -> Vec<Value> {
        let tree = |tokens: u64| json!([
            {"type":"workflow_phase","id":"p1","index":1,"title":"Build"},
            {"type":"workflow_agent","id":"a1","label":"dev 😀","phaseIndex":1,"state":"start","tokens":tokens,"elapsedMs":400}
        ]);
        vec![
            json!({"type":"rows","epoch":10,"revision":2,"positions":{"u1":0,"r1":1},"rows":[
                {"kind":"user","id":"u1","text":"hi","images":[],"at":1},
                {"kind":"assistant","id":"r1","text":"Hel","streaming":true,"at":2}]}),
            json!({"type":"rows","epoch":10,"revision":3,"positions":{"r1":1},"rows":[
                {"kind":"assistant","id":"r1","text":"Hello 中文 😀","streaming":true,"at":3}]}),
            json!({"type":"rows","epoch":10,"revision":4,"positions":{"r1":1,"t1":2},"rows":[
                {"kind":"assistant","id":"r1","text":"Hello 中文 😀!","streaming":false,"at":4,"durationMs":9},
                {"kind":"tool","id":"t1","name":"Bash","input":{"command":"ls"},"isError":false,"status":"running","childCount":0,"children":[],"detailAvailable":false}]}),
            json!({"type":"rows","epoch":10,"revision":5,"positions":{"t1":2},"rows":[
                {"kind":"tool","id":"t1","name":"Bash","input":{"command":"ls"},"output":"a\nb","isError":false,"status":"completed","childCount":0,"children":[],"detailAvailable":false}]}),
            json!({"type":"extras","extras":{"fastMode":false,"contextTokens":100,"backgroundTasks":[
                {"task_id":"w1","task_type":"local_workflow","description":"Build: dev","status":"running","finished":false,"can_stop":true,"elapsed_ms":1000,"usage":{"total_tokens":5},"workflow_progress":tree(5)},
                {"task_id":"b1","task_type":"local_bash","description":"sleep","status":"running","finished":false,"can_stop":true,"elapsed_ms":1000}]}}),
            json!({"type":"extras","extras":{"fastMode":false,"contextTokens":100,"backgroundTasks":[
                {"task_id":"w1","task_type":"local_workflow","description":"Build: dev","status":"running","finished":false,"can_stop":true,"elapsed_ms":2000,"usage":{"total_tokens":5},"workflow_progress":tree(5)},
                {"task_id":"b1","task_type":"local_bash","description":"sleep","status":"running","finished":false,"can_stop":true,"elapsed_ms":2000}]}}),
            json!({"type":"extras","extras":{"fastMode":false,"backgroundTasks":[
                {"task_id":"n1","task_type":"local_agent","description":"new","status":"running","finished":false,"can_stop":true,"elapsed_ms":0},
                {"task_id":"w1","task_type":"local_workflow","description":"Build: dev","status":"running","finished":false,"can_stop":true,"elapsed_ms":3000,"usage":{"total_tokens":7},"workflow_progress":tree(7)},
                {"task_id":"b1","task_type":"local_bash","description":"sleep","status":"completed","finished":true,"can_stop":false,"elapsed_ms":2500,"summary":"done"}]}}),
            // Between these two the connection watches chat://task/s1/w1 (see fixture_frames).
            json!({"type":"extras","extras":{"fastMode":false,"backgroundTasks":[
                {"task_id":"n1","task_type":"local_agent","description":"new","status":"running","finished":false,"can_stop":true,"elapsed_ms":0},
                {"task_id":"w1","task_type":"local_workflow","description":"Build: dev","status":"running","finished":false,"can_stop":true,"elapsed_ms":3000,"usage":{"total_tokens":7},"workflow_progress":tree(7)}]}}),
            json!({"type":"extras","extras":{"fastMode":true,"backgroundTasks":[
                {"task_id":"w1","task_type":"local_workflow","description":"Build: dev","status":"running","finished":false,"can_stop":true,"elapsed_ms":4000,"usage":{"total_tokens":8},"workflow_progress":tree(8)}]}}),
            json!({"type":"queued","items":[],"revision":1,"epoch":10}),
            json!({"type":"reset","epoch":11,"revision":1,"rows":[],"positions":{},"hasMore":false}),
            json!({"type":"rows","epoch":11,"revision":2,"positions":{"r1":0},"rows":[
                {"kind":"reasoning","id":"r1","text":"think","streaming":true}]}),
        ]
    }

    fn fixture_frames() -> Value {
        let mut wire = ChatWire::default();
        wire.enable();
        wire.watch_session("s1").unwrap();
        let mut decoder = Decoder::default();
        let mut frames = Vec::new();
        for (index, payload) in fixture_sequence().into_iter().enumerate() {
            if index == 7 {
                wire.watch_task("s1", "w1");
            }
            let detail = |task: &str| wire.details.contains(&("s1".to_string(), task.to_string()));
            let decoded = expected(&payload, &detail);
            if let Some(encoded) = wire.encode("s1", payload) {
                assert_eq!(decoder.decode(&encoded).unwrap(), decoded, "the reference decoder agrees at step {index}");
                frames.push(json!({"name":"chat://event/s1","payload":encoded,"decoded":decoded}));
            }
        }
        json!({"version": PROTOCOL_VERSION, "frames": frames})
    }

    #[test]
    fn the_checked_in_fixture_matches_the_encoder() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/ipc/chatWire.fixture.json");
        let frames = fixture_frames();
        let wanted = format!("{}\n", serde_json::to_string_pretty(&frames).unwrap());
        if std::env::var_os("VLX_UPDATE_CHAT_WIRE_FIXTURE").is_some() {
            std::fs::write(&path, &wanted).unwrap();
        }
        let actual = std::fs::read_to_string(&path).expect("the fixture exists; regenerate it with VLX_UPDATE_CHAT_WIRE_FIXTURE=1");
        assert_eq!(actual, wanted, "the encoder changed: regenerate the fixture with VLX_UPDATE_CHAT_WIRE_FIXTURE=1");
        let list = frames["frames"].as_array().unwrap();
        assert_eq!(list.len(), fixture_sequence().len() - 1, "exactly the clock-only publish is withheld");
        assert!(list.iter().any(|f| f["payload"].get("patch").is_some()));
        assert!(list.iter().any(|f| f["payload"]["rows"].as_array().is_some_and(|rows| rows.iter().any(|r| r.get("patch").is_some()))));
    }
}
