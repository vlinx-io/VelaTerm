//! The earlier Claude conversations of one working directory, for the conversation view's resume picker.
//!
//! Claude records every conversation as `<config>/projects/<key>/<id>.jsonl`. The key is the working
//! directory with every character outside ASCII letters and digits replaced by `-`, so it can never hold
//! a separator or `..`. Different directories can share a key (`/a/b-c` and `/a/b/c`), which is why a
//! file is listed only when the first working directory it records is the session's own: the key finds
//! candidates, the recorded `cwd` decides.
//!
//! Only metadata leaves the listing: a title, the first prompt, the time of last activity and the size.
//! Recordings reach several megabytes. Each file is read line by line from the start until its working
//! directory is known, like the import dialog's scanner does, so a large first prompt (a screenshot, a
//! long paste) never hides a conversation; titles beyond that come from a bounded window at the end.
//! Neither search reads more than `SCAN_BUDGET` of a file, and one page of a listing stops once it has
//! read `REQUEST_BUDGET` or described `MAX_DESCRIBED` files; the next page continues where it stopped.
//!
//! Every read of a recording goes through this module: the listing and the bind (`describe`), the replay
//! (`open_claude`), and every other reader of a conversation by its id (`open_recording`, used by the
//! transcript, export, rewind, context and notification readers), plus the subagent recordings Claude keeps
//! beside a checked one (`read_child`, reached through that file's own folder). Each one opens a regular file without
//! following links, decides on the working directory it records through that same handle and reads the
//! content from it, within `RECORDING_LIMIT`, so what was checked is what is read.

use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::agent::history::{entry_identity, open_regular, same_dir, title, valid_id, SCAN_BUDGET};

/// The most conversations one page of a listing describes; the next page continues after it. The cap
/// counts conversations that are listed, never files that turn out not to belong to the directory.
pub const LIST_LIMIT: usize = 500;
/// Bytes of the start of a recording searched for the first prompt and titles once the working directory
/// is known, and bytes read from its end for later titles.
const WINDOW: u64 = 256 * 1024;
/// With the working directory known, the start of a recording is not searched beyond this for a first
/// prompt; the same bound the import dialog's scanner uses.
const HEAD_LIMIT: u64 = 4 * 1024 * 1024;
/// Claude shortens longer keys and appends a hash of the full path. The hash scheme is not verified
/// here, so a long key matches every folder that starts with the shortened part; the recorded `cwd`
/// still decides.
const MAX_KEY: usize = 200;
/// The most of all recordings one page of a listing reads; the file under way is still read to its own
/// bounds, so no conversation is left out halfway.
const REQUEST_BUDGET: u64 = 256 * 1024 * 1024;
/// The most recordings one page of a listing describes, whether or not they match.
const MAX_DESCRIBED: usize = 2000;
/// The largest recording read whole, for a replay, a transcript or an export; a larger one is refused
/// instead of being buffered.
pub(crate) const RECORDING_LIMIT: u64 = 512 * 1024 * 1024;
/// The most one replay reads of all the subagent recordings it links together, beside the recording
/// itself.
pub(crate) const CHILDREN_LIMIT: u64 = RECORDING_LIMIT;
const MISSING: &str = "Claude transcript file not found";

/// Another VelaTerm session that already continues a conversation.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConversationOwner {
    pub session_id: String,
    pub name: String,
    pub archived: bool,
}

/// One earlier conversation, described without its content.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_prompt: Option<String>,
    /// Last modification of the recording, in milliseconds since the epoch.
    pub updated_at: i64,
    pub size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<ConversationOwner>,
    /// The conversation this session is bound to right now.
    pub current: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub directory: String,
    pub conversations: Vec<Conversation>,
    /// Recordings after this page have not been looked at yet; the next page starts at `next_offset`.
    pub truncated: bool,
    /// Where the next page continues: the position, among the directory's recordings newest first, after
    /// the last one this page looked at.
    pub next_offset: usize,
}

#[cfg(test)]
impl Listing {
    pub fn find(&self, id: &str) -> Option<&Conversation> {
        self.conversations.iter().find(|conversation| conversation.id == id)
    }
}

/// Where Claude keeps its per-directory recordings on this host.
pub fn projects_root() -> Option<PathBuf> {
    crate::agent::resume::claude_home().map(|home| home.join("projects"))
}

/// The folder name Claude derives from a working directory.
///
/// Claude replaces per UTF-16 code unit, so a character outside the Basic Multilingual Plane becomes
/// two dashes.
pub fn project_key(directory: &str) -> String {
    let mut key = String::with_capacity(directory.len());
    for c in directory.chars() {
        if c.is_ascii_alphanumeric() {
            key.push(c);
        } else {
            (0..c.len_utf16()).for_each(|_| key.push('-'));
        }
    }
    key
}

/// Real folders below `root` that may hold recordings of `directory`, for its spelled and its resolved
/// path. Symlinked folders are skipped: Claude never creates them, and following one would read files
/// the key does not name.
fn candidate_dirs(root: &Path, directory: &Path) -> Vec<PathBuf> {
    let mut keys = vec![project_key(&directory.to_string_lossy())];
    if let Ok(resolved) = directory.canonicalize() {
        keys.push(project_key(&resolved.to_string_lossy()));
    }
    keys.dedup();
    let real_dir = |path: &Path| std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir());
    let mut dirs = Vec::new();
    for key in keys {
        if key.len() <= MAX_KEY {
            let dir = root.join(&key);
            if real_dir(&dir) && !dirs.contains(&dir) {
                dirs.push(dir);
            }
            continue;
        }
        let prefix = format!("{}-", &key[..MAX_KEY]);
        let Ok(entries) = std::fs::read_dir(root) else { continue };
        for entry in entries.flatten() {
            let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
            let matches = entry.file_name().to_str().is_some_and(|name| name.starts_with(&prefix));
            if is_dir && matches && !dirs.contains(&entry.path()) {
                dirs.push(entry.path());
            }
        }
    }
    dirs
}

struct Candidate {
    id: String,
    path: PathBuf,
    updated_at: i64,
    size: u64,
    /// Device and inode of the entry as listed; the file opened later must be the same one.
    identity: Option<(u64, u64)>,
}

/// Recordings below `root` that may belong to `directory`, newest first; `only` restricts them to one ID.
///
/// Only regular files named `<id>.jsonl` directly inside a key folder count; subagent folders and links
/// do not.
fn candidates(root: &Path, directory: &Path, only: Option<&str>) -> Result<Vec<Candidate>, String> {
    let mut candidates: Vec<Candidate> = Vec::new();
    for dir in candidate_dirs(root, directory) {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("Cannot read Claude conversations: {e}")),
        };
        for entry in entries.flatten() {
            // `DirEntry::file_type` does not follow links, so a link to a file elsewhere is not a file here.
            if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
                continue;
            }
            let name = entry.file_name();
            let Some(id) = name.to_str().and_then(|name| name.strip_suffix(".jsonl")) else { continue };
            if !valid_id(id) || only.is_some_and(|only| only != id) || candidates.iter().any(|known| known.id == id) {
                continue;
            }
            // Does not follow links either, so the identity is the entry's own.
            let Ok(meta) = entry.metadata() else { continue };
            let updated_at = meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|age| age.as_millis() as i64)
                .unwrap_or(0);
            candidates.push(Candidate {
                id: id.to_string(),
                path: entry.path(),
                updated_at,
                size: meta.len(),
                identity: entry_identity(&meta),
            });
        }
    }
    candidates.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then_with(|| a.id.cmp(&b.id)));
    Ok(candidates)
}

/// Whether a conversation matches a search over what its row shows: title, first prompt, branch and id.
pub fn matches(conversation: &Conversation, query: &str) -> bool {
    let needle = query.trim().to_lowercase();
    needle.is_empty()
        || [Some(&conversation.title), conversation.first_prompt.as_ref(), Some(&conversation.id), conversation.git_branch.as_ref()]
            .into_iter()
            .flatten()
            .any(|field| field.to_lowercase().contains(&needle))
}

/// One page of the conversations recorded for `directory` below `root` that match `query`, newest first,
/// looking at the recordings from position `offset` on (a page's `next_offset`).
///
/// `root` is explicit so tests run against a fixture instead of the user's own recordings. The search and
/// the paging happen here, after each file has been checked, so every conversation of the directory can
/// be found and reached however many there are, and recordings of other directories never take the place
/// of listable ones. A page ends after `limit` matches, after `MAX_DESCRIBED` files or once it has read
/// `REQUEST_BUDGET`, so what one request costs is bounded however many files there are or how few match;
/// an offset past the end costs no read at all.
pub fn list_claude(root: &Path, directory: &Path, query: &str, offset: usize, limit: usize) -> Result<Listing, String> {
    list_within(root, directory, query, offset, limit, (MAX_DESCRIBED, REQUEST_BUDGET))
}

/// `list_claude` with its per-page bounds explicit: at most this many files described and bytes read.
fn list_within(
    root: &Path,
    directory: &Path,
    query: &str,
    offset: usize,
    limit: usize,
    (max_described, budget): (usize, u64),
) -> Result<Listing, String> {
    let candidates = candidates(root, directory, None)?;
    let total = candidates.len();
    let mut conversations = Vec::new();
    let mut next_offset = offset.min(total);
    let (mut described, mut spent) = (0, 0);
    for candidate in candidates.into_iter().skip(offset) {
        if conversations.len() == limit || described == max_described || spent >= budget {
            break;
        }
        next_offset += 1;
        described += 1;
        let Some(conversation) = describe(candidate, directory, &mut spent) else { continue };
        if matches(&conversation, query) {
            conversations.push(conversation);
        }
    }
    Ok(Listing {
        directory: directory.to_string_lossy().into_owned(),
        conversations,
        truncated: next_offset < total,
        next_offset,
    })
}

/// The one conversation `id` recorded for `directory`, described exactly as the listing would, but
/// without its cap: a conversation older than the listed ones can still be continued.
pub fn find_claude(root: &Path, directory: &Path, id: &str) -> Result<Option<Conversation>, String> {
    if !valid_id(id) {
        return Ok(None);
    }
    Ok(candidates(root, directory, Some(id))?.into_iter().find_map(|candidate| describe(candidate, directory, &mut 0)))
}

/// The recording of conversation `id` for `directory`, for the replay: the same key folders,
/// the same regular-file and identity checks and the same directory decision as the listing, so no file
/// elsewhere below `root` and no link can stand in for the one that was checked and that the agent
/// continues.
///
/// Returns the checked handle itself; read from it (`read_whole`), never reopen the path.
pub fn open_claude(root: &Path, directory: &Path, id: &str) -> Result<(PathBuf, std::fs::File), String> {
    if !valid_id(id) {
        return Err(MISSING.into());
    }
    for candidate in candidates(root, directory, Some(id))? {
        let Ok(file) = open_regular(&candidate.path, candidate.identity) else { continue };
        if first_cwd(&file).is_some_and(|cwd| same_dir(Path::new(&cwd), directory)) {
            return Ok((candidate.path, file));
        }
    }
    Err(MISSING.into())
}

/// The recording of conversation `id` wherever below `root` it lies, for the readers that know the id
/// but not the directory: the transcript, the export, rewind targets, context information and
/// notification previews. The returned handle is the file that was checked; read from it, never reopen
/// the path.
///
/// Only a regular file directly in a real key folder counts, and only when the first working directory
/// it records belongs to that folder, which is where Claude itself writes it. A link, or another
/// directory's recording placed in a folder under the same id, therefore never stands in for the real
/// one. Two recordings that both pass cannot be told apart without the session's directory, so neither
/// is read.
pub fn open_recording(root: &Path, id: &str) -> Result<(PathBuf, std::fs::File), String> {
    if !valid_id(id) {
        return Err(MISSING.into());
    }
    let mut found = None;
    for entry in std::fs::read_dir(root).map_err(|_| MISSING)?.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let folder = entry.path();
        let path = folder.join(format!("{id}.jsonl"));
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if !meta.is_file() {
            continue;
        }
        let Ok(file) = open_regular(&path, entry_identity(&meta)) else { continue };
        let Some(cwd) = first_cwd(&file) else { continue };
        if !candidate_dirs(root, Path::new(&cwd)).contains(&folder) {
            continue;
        }
        if found.is_some() {
            return Err("Several Claude transcripts carry this id, so none is read".into());
        }
        found = Some((path, file));
    }
    found.ok_or_else(|| MISSING.into())
}

/// A checked recording read whole from its start through its own handle, or refused beyond
/// `RECORDING_LIMIT` instead of being buffered.
pub(crate) fn read_whole(file: &mut std::fs::File) -> Result<String, String> {
    read_within(file, RECORDING_LIMIT).map_err(|e| e.unwrap_or_else(|| "The Claude transcript is too large to read".into()))
}

/// A file read whole from its start within `limit` bytes; `Err(None)` when it holds more than that.
fn read_within(file: &mut std::fs::File, limit: u64) -> Result<String, Option<String>> {
    file.seek(SeekFrom::Start(0)).map_err(|e| Some(format!("Failed to read transcript: {e}")))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| Some(format!("Failed to read transcript: {e}")))?;
    if bytes.len() as u64 > limit {
        return Err(None);
    }
    String::from_utf8(bytes).map_err(|_| Some("Failed to read transcript: it is not valid UTF-8".into()))
}

/// A subagent recording Claude keeps beside a checked recording, `<id>/subagents/agent-<child>.jsonl`.
///
/// `checked` is the handle of the recording at `recording`, as `open_claude` or `open_recording` returned
/// it. The child is reached from the folder that holds that very file, never through a link on any
/// component below it. `remaining` is what one replay may still read of all its children together
/// (start with `CHILDREN_LIMIT`); each child is read within it and within `RECORDING_LIMIT`, and what it
/// read is taken off, so a small recording that links many or large children cannot make one replay read
/// without bound.
pub(crate) fn read_child(recording: &Path, checked: &std::fs::File, child: &str, remaining: &mut u64) -> Result<String, String> {
    const UNAVAILABLE: &str = "The subagent recording is unavailable";
    const EXHAUSTED: &str = "Subagent history exceeds the replay limit";
    // Only provider identifiers are accepted; recording content cannot select arbitrary paths.
    if child.is_empty() || child.len() > 200 || !child.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return Err("Invalid Claude subagent identifier".to_string());
    }
    if *remaining == 0 {
        return Err(EXHAUSTED.into());
    }
    let identity = checked.metadata().ok().as_ref().and_then(entry_identity);
    let mut file = open_child(recording, identity, &format!("agent-{child}.jsonl")).map_err(|_| UNAVAILABLE.to_string())?;
    let limit = (*remaining).min(RECORDING_LIMIT);
    match read_within(&mut file, limit) {
        Ok(text) => {
            *remaining -= text.len() as u64;
            Ok(text)
        }
        Err(Some(error)) => Err(error),
        // Too large for what is left: nothing more is read for this replay.
        Err(None) => {
            *remaining = 0;
            Err(EXHAUSTED.into())
        }
    }
}

/// Open `<stem>/subagents/<child>` below the folder of `recording`, each directory relative to the one
/// opened before and none of them through a link; the folder must still hold the checked file (`identity`).
#[cfg(unix)]
fn open_child(recording: &Path, identity: Option<(u64, u64)>, child: &str) -> std::io::Result<std::fs::File> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    let refused = || std::io::Error::new(std::io::ErrorKind::InvalidInput, "not the checked recording's folder");
    let name = |bytes: &[u8]| CString::new(bytes).map_err(|_| refused());
    let (Some(folder), Some(file_name), Some(stem)) = (recording.parent(), recording.file_name(), recording.file_stem()) else {
        return Err(refused());
    };
    let expected = identity.ok_or_else(refused)?;
    let open = |at: Option<&OwnedFd>, path: &CString, flags: libc::c_int| -> std::io::Result<OwnedFd> {
        // SAFETY: `path` is a valid NUL-terminated string and `at` a directory descriptor this function owns.
        let fd = unsafe {
            match at {
                Some(at) => libc::openat(at.as_raw_fd(), path.as_ptr(), flags),
                None => libc::open(path.as_ptr(), flags),
            }
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: `fd` was just returned by open/openat and is owned by nobody else.
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    };
    let directory = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let key = open(None, &name(folder.as_os_str().as_bytes())?, directory)?;
    // The folder opened is the one that holds the checked recording, not one swapped in since.
    // SAFETY: an all-zero `stat` is a valid value that fstatat overwrites.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    let listed = name(file_name.as_bytes())?;
    // SAFETY: `key` is an open directory, `listed` a valid NUL-terminated name and `stat` writable.
    if unsafe { libc::fstatat(key.as_raw_fd(), listed.as_ptr(), &mut stat, libc::AT_SYMLINK_NOFOLLOW) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    if (stat.st_dev as u64, stat.st_ino as u64) != expected {
        return Err(refused());
    }
    let own = open(Some(&key), &name(stem.as_bytes())?, directory)?;
    let subagents = open(Some(&own), &name(b"subagents")?, directory)?;
    let fd = open(Some(&subagents), &name(child.as_bytes())?, libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)?;
    let file = std::fs::File::from(fd);
    if !file.metadata()?.is_file() {
        return Err(refused());
    }
    Ok(file)
}

/// On Windows every directory from the key folder down is opened itself, not a link or junction in its
/// place, and held open without delete sharing while the child is opened, so none of them can be renamed
/// or replaced between that check and the open; the child is opened by `open_regular`, which refuses a
/// link as well. The checked recording's identity is not available here (see `open_regular`).
#[cfg(windows)]
fn open_child(recording: &Path, identity: Option<(u64, u64)>, child: &str) -> std::io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let _ = identity;
    let refused = || std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a real folder");
    let folder = recording.parent().ok_or_else(refused)?;
    let own = recording.with_extension("");
    let subagents = own.join("subagents");
    let mut held = Vec::with_capacity(3);
    for directory in [folder, own.as_path(), subagents.as_path()] {
        let handle = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(directory)?;
        // A link or junction opened as itself reports as a link, not as a directory.
        if !handle.metadata()?.is_dir() {
            return Err(refused());
        }
        held.push(handle);
    }
    let file = open_regular(&subagents.join(child), None);
    drop(held);
    file
}

/// Elsewhere without descriptor-relative opens: every directory is checked not to be a link before the
/// file is opened.
#[cfg(not(any(unix, windows)))]
fn open_child(recording: &Path, identity: Option<(u64, u64)>, child: &str) -> std::io::Result<std::fs::File> {
    let _ = identity;
    let refused = || std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a real folder");
    let own = recording.with_extension("");
    let subagents = own.join("subagents");
    for directory in [&own, &subagents] {
        if !std::fs::symlink_metadata(directory)?.is_dir() {
            return Err(refused());
        }
    }
    open_regular(&subagents.join(child), None)
}

/// The first working directory a recording names, read line by line within `SCAN_BUDGET`; nothing for a
/// subagent's recording or one that names none within the budget.
fn first_cwd(file: &std::fs::File) -> Option<String> {
    let mut reader = BufReader::new(Read::take(file, SCAN_BUDGET));
    let mut line = Vec::new();
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line).ok()? == 0 {
            return None;
        }
        let Ok(value) = serde_json::from_slice::<Value>(&line) else { continue };
        if value["isSidechain"] == true {
            return None;
        }
        if let Some(cwd) = value["cwd"].as_str().filter(|cwd| !cwd.is_empty()) {
            return Some(cwd.to_string());
        }
    }
}

/// The complete lines of a byte window. A window that starts inside the file drops its first, partial
/// line; one that ends before the end of the file drops its last.
fn window_lines(bytes: &[u8], starts_inside: bool, ends_inside: bool) -> Vec<&[u8]> {
    let mut lines: Vec<&[u8]> = bytes.split(|b| *b == b'\n').collect();
    if starts_inside && !lines.is_empty() {
        lines.remove(0);
    }
    if ends_inside {
        lines.pop();
    }
    lines
}

fn read_window(file: &mut std::fs::File, offset: u64) -> Option<Vec<u8>> {
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut bytes = Vec::new();
    file.by_ref().take(WINDOW).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// What a user typed, or nothing for a line that only carries tool results or injected context.
fn prompt_text(value: &Value) -> Option<String> {
    if value["type"] != "user" || value["isMeta"] == true {
        return None;
    }
    let content = &value["message"]["content"];
    let text = content.as_str().map(str::to_string).unwrap_or_else(|| {
        content
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|block| block["type"] == "text")
                    .filter_map(|block| block["text"].as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default()
    });
    let text = text.trim();
    if text.is_empty() || text.starts_with('<') || crate::agent::transcript::is_injected_context(text) {
        return None;
    }
    Some(title(text))
}

#[derive(Default)]
struct Titles {
    custom: Option<String>,
    generated: Option<String>,
}

impl Titles {
    fn observe(&mut self, value: &Value) {
        let text = |field: &str| value[field].as_str().map(title).filter(|text| !text.is_empty());
        match value["type"].as_str() {
            Some("custom-title") => self.custom = text("customTitle").or(self.custom.take()),
            Some("ai-title") => self.generated = text("aiTitle").or(self.generated.take()),
            _ => {}
        }
    }
}

/// Describe one recording, or leave it out when it is a subagent's, records no working directory at all,
/// or records another directory than `directory`.
///
/// The start is read line by line until the first working directory appears, however far in that is
/// within `SCAN_BUDGET`, and then on to the end of the head window (or until a first prompt is found, at
/// most `HEAD_LIMIT`). The budget bounds both what is read and what one line can buffer. Every byte read
/// is added to `spent`, also for a recording that is left out.
fn describe(candidate: Candidate, directory: &Path, spent: &mut u64) -> Option<Conversation> {
    let file = open_regular(&candidate.path, candidate.identity).ok()?;
    let mut reader = BufReader::new(file.take(SCAN_BUDGET));
    let mut cwd: Option<String> = None;
    let mut first_prompt: Option<String> = None;
    let mut git_branch: Option<String> = None;
    let mut titles = Titles::default();
    let mut consumed: u64 = 0;
    let mut line = Vec::new();
    loop {
        if cwd.is_some() && consumed >= WINDOW && (first_prompt.is_some() || consumed >= HEAD_LIMIT) {
            break;
        }
        line.clear();
        let read = reader.read_until(b'\n', &mut line).ok()?;
        if read == 0 {
            break;
        }
        consumed += read as u64;
        *spent += read as u64;
        let Ok(value) = serde_json::from_slice::<Value>(&line) else { continue };
        if value["isSidechain"] == true {
            return None;
        }
        if cwd.is_none() {
            cwd = value["cwd"].as_str().filter(|cwd| !cwd.is_empty()).map(str::to_string);
            // Decided as soon as it is known: a foreign recording is not read any further.
            if cwd.as_deref().is_some_and(|cwd| !same_dir(Path::new(cwd), directory)) {
                return None;
            }
        }
        if git_branch.is_none() {
            git_branch = value["gitBranch"].as_str().filter(|branch| !branch.is_empty()).map(str::to_string);
        }
        if first_prompt.is_none() {
            first_prompt = prompt_text(&value);
        }
        titles.observe(&value);
    }
    cwd?;
    if candidate.size > consumed {
        // The tail window never reaches back into lines already read, so no line is seen twice.
        let offset = candidate.size.saturating_sub(WINDOW).max(consumed);
        let mut file = reader.into_inner().into_inner();
        if let Some(tail) = read_window(&mut file, offset) {
            *spent += tail.len() as u64;
            for line in window_lines(&tail, offset > consumed, false) {
                if let Ok(value) = serde_json::from_slice::<Value>(line) {
                    titles.observe(&value);
                }
            }
        }
    }
    let title = titles
        .custom
        .or(titles.generated)
        .or_else(|| first_prompt.clone())
        .unwrap_or_else(|| candidate.id.clone());
    Some(Conversation {
        id: candidate.id,
        title,
        first_prompt,
        updated_at: candidate.updated_at,
        size_bytes: candidate.size,
        git_branch,
        owner: None,
        current: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn read_claude(root: &Path, directory: &Path, id: &str) -> Result<(PathBuf, String), String> {
        let (path, mut file) = open_claude(root, directory, id)?;
        Ok((path, read_whole(&mut file)?))
    }

    fn read_recording(root: &Path, id: &str) -> Result<(PathBuf, String), String> {
        let (path, mut file) = open_recording(root, id)?;
        Ok((path, read_whole(&mut file)?))
    }

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("vlx-conversations-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path.canonicalize().unwrap())
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write(path: &Path, values: &[Value]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = values.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
        std::fs::write(path, text + "\n").unwrap();
    }

    fn set_mtime(path: &Path, seconds: u64) {
        let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds);
        std::fs::File::options().write(true).open(path).unwrap().set_modified(time).unwrap();
    }

    fn prompt(cwd: &Path, text: &str) -> Value {
        json!({"type":"user","sessionId":"x","cwd":cwd,"message":{"role":"user","content":text}})
    }

    #[test]
    fn keys_replace_everything_but_ascii_letters_and_digits() {
        assert_eq!(project_key("/Users/hermes/Projekte/VelaTerm"), "-Users-hermes-Projekte-VelaTerm");
        assert_eq!(project_key("/Users/hermes/.hermes/x"), "-Users-hermes--hermes-x");
        assert_eq!(project_key("/tmp/a b/ü"), "-tmp-a-b--");
        // Outside the Basic Multilingual Plane a character is two UTF-16 units, and Claude counts units.
        assert_eq!(project_key("/x/😀"), "-x---");
        for hostile in ["/../../etc", "..\\..\\x", "/a/./b"] {
            let key = project_key(hostile);
            assert!(!key.contains('/') && !key.contains('\\') && !key.contains(".."), "{key}");
        }
    }

    #[test]
    fn only_conversations_recorded_in_the_directory_are_listed() {
        let temp = Temp::new();
        let project = temp.0.join("a").join("b").join("c");
        let colliding = temp.0.join("a").join("b-c");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&colliding).unwrap();
        let root = temp.0.join("projects");
        let folder = root.join(project_key(&project.to_string_lossy()));
        assert_eq!(folder, root.join(project_key(&colliding.to_string_lossy())), "the fixture must collide");

        write(&folder.join("good-1.jsonl"), &[prompt(&project, "Fix the editor")]);
        write(&folder.join("foreign.jsonl"), &[prompt(&colliding, "Another project")]);
        write(&folder.join("sidechain.jsonl"), &[json!({"type":"user","isSidechain":true,"cwd":project})]);
        write(&folder.join("good-1/subagents/agent-1.jsonl"), &[prompt(&project, "child")]);
        write(&folder.join("no-cwd.jsonl"), &[json!({"type":"queue-operation","sessionId":"no-cwd"})]);
        write(&folder.join("bad;id.jsonl"), &[prompt(&project, "Hostile name")]);
        write(&temp.0.join("outside.jsonl"), &[prompt(&project, "Outside the folder")]);
        #[cfg(unix)]
        std::os::unix::fs::symlink(temp.0.join("outside.jsonl"), folder.join("linked.jsonl")).unwrap();

        let listing = list_claude(&root, &project, "", 0, LIST_LIMIT).unwrap();
        let ids: Vec<_> = listing.conversations.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["good-1"]);
        assert_eq!(listing.conversations[0].title, "Fix the editor");
        assert!(!listing.truncated);
        assert!(list_claude(&root, &colliding, "", 0, LIST_LIMIT).unwrap().conversations.iter().all(|c| c.id == "foreign"));
        // A directory without recordings is an empty listing, not an error.
        assert!(list_claude(&temp.0.join("missing"), &project, "", 0, LIST_LIMIT).unwrap().conversations.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_key_folder_is_not_followed() {
        let temp = Temp::new();
        let project = temp.0.join("p");
        std::fs::create_dir_all(&project).unwrap();
        let elsewhere = temp.0.join("elsewhere");
        write(&elsewhere.join("hidden.jsonl"), &[prompt(&project, "Should stay hidden")]);
        let root = temp.0.join("projects");
        std::fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(&elsewhere, root.join(project_key(&project.to_string_lossy()))).unwrap();
        assert!(list_claude(&root, &project, "", 0, LIST_LIMIT).unwrap().conversations.is_empty());
    }

    /// A subagent recording is read only below the folder that holds the checked recording, and no link on
    /// any component below it is followed.
    #[cfg(unix)]
    #[test]
    fn a_child_recording_is_read_through_the_checked_folder_only() {
        use std::os::unix::fs::symlink;
        let temp = Temp::new();
        let project = temp.0.join("p");
        std::fs::create_dir_all(&project).unwrap();
        let root = temp.0.join("projects");
        let folder = root.join(project_key(&project.to_string_lossy()));
        write(&folder.join("conv-1.jsonl"), &[prompt(&project, "Parent")]);
        let child = folder.join("conv-1").join("subagents").join("agent-a1.jsonl");
        write(&child, &[json!({"type":"user","message":{"content":"legit child"}})]);
        let elsewhere = temp.0.join("elsewhere");
        write(&elsewhere.join("subagents").join("agent-a1.jsonl"), &[json!({"type":"user","message":{"content":"SECRET"}})]);
        write(&elsewhere.join("agent-a1.jsonl"), &[json!({"type":"user","message":{"content":"SECRET"}})]);
        let (path, file) = open_claude(&root, &project, "conv-1").unwrap();
        let mut budget = CHILDREN_LIMIT;
        assert!(read_child(&path, &file, "a1", &mut budget).unwrap().contains("legit child"));
        assert!(read_child(&path, &file, "../a1", &mut budget).is_err());
        assert!(read_child(&path, &file, "", &mut budget).is_err());

        // A linked child file, a linked `subagents` folder and a linked `<id>` folder are all refused.
        std::fs::remove_file(&child).unwrap();
        symlink(elsewhere.join("agent-a1.jsonl"), &child).unwrap();
        assert!(read_child(&path, &file, "a1", &mut budget).is_err());
        std::fs::remove_dir_all(folder.join("conv-1").join("subagents")).unwrap();
        symlink(&elsewhere, folder.join("conv-1").join("subagents")).unwrap();
        assert!(read_child(&path, &file, "a1", &mut budget).is_err());
        std::fs::remove_dir_all(folder.join("conv-1")).unwrap();
        symlink(&elsewhere, folder.join("conv-1")).unwrap();
        assert!(read_child(&path, &file, "a1", &mut budget).is_err());

        // A key folder swapped after the check holds another file under the same name: its children are
        // not the checked recording's, so none is read.
        std::fs::rename(&folder, temp.0.join("moved")).unwrap();
        write(&folder.join("conv-1.jsonl"), &[prompt(&project, "Impostor")]);
        write(&folder.join("conv-1").join("subagents").join("agent-a1.jsonl"), &[json!({"type":"user","message":{"content":"SECRET"}})]);
        assert!(read_child(&path, &file, "a1", &mut budget).is_err());
    }

    /// A directory junction, which Windows creates without any privilege.
    #[cfg(windows)]
    fn junction(link: &Path, target: &Path) {
        let status = std::process::Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(link)
            .arg(target)
            .status()
            .unwrap();
        assert!(status.success(), "mklink /J failed");
    }

    #[cfg(windows)]
    #[test]
    fn a_junctioned_key_folder_is_not_followed_on_windows() {
        let temp = Temp::new();
        let project = temp.0.join("p");
        std::fs::create_dir_all(&project).unwrap();
        let elsewhere = temp.0.join("elsewhere");
        write(&elsewhere.join("hidden.jsonl"), &[prompt(&project, "Should stay hidden")]);
        let root = temp.0.join("projects");
        std::fs::create_dir_all(&root).unwrap();
        junction(&root.join(project_key(&project.to_string_lossy())), &elsewhere);
        assert!(list_claude(&root, &project, "", 0, LIST_LIMIT).unwrap().conversations.is_empty());
    }

    /// The Windows counterpart of `a_child_recording_is_read_through_the_checked_folder_only`: a junction
    /// in place of `subagents` or `<id>`, and a file symlink in place of the child, are all refused. The
    /// swapped-key-folder case is Unix-only because Windows has no file identity to compare here.
    #[cfg(windows)]
    #[test]
    fn a_child_recording_is_read_through_the_checked_folder_only_on_windows() {
        let temp = Temp::new();
        let project = temp.0.join("p");
        std::fs::create_dir_all(&project).unwrap();
        let root = temp.0.join("projects");
        let folder = root.join(project_key(&project.to_string_lossy()));
        write(&folder.join("conv-1.jsonl"), &[prompt(&project, "Parent")]);
        let child = folder.join("conv-1").join("subagents").join("agent-a1.jsonl");
        write(&child, &[json!({"type":"user","message":{"content":"legit child"}})]);
        let elsewhere = temp.0.join("elsewhere");
        write(&elsewhere.join("subagents").join("agent-a1.jsonl"), &[json!({"type":"user","message":{"content":"SECRET"}})]);
        write(&elsewhere.join("agent-a1.jsonl"), &[json!({"type":"user","message":{"content":"SECRET"}})]);
        let (path, file) = open_claude(&root, &project, "conv-1").unwrap();
        let mut budget = CHILDREN_LIMIT;
        assert!(read_child(&path, &file, "a1", &mut budget).unwrap().contains("legit child"));
        assert!(read_child(&path, &file, "../a1", &mut budget).is_err());

        std::fs::remove_dir_all(folder.join("conv-1").join("subagents")).unwrap();
        junction(&folder.join("conv-1").join("subagents"), &elsewhere);
        assert!(read_child(&path, &file, "a1", &mut budget).is_err());
        std::fs::remove_dir(folder.join("conv-1").join("subagents")).unwrap();
        std::fs::remove_dir_all(folder.join("conv-1")).unwrap();
        junction(&folder.join("conv-1"), &elsewhere);
        assert!(read_child(&path, &file, "a1", &mut budget).is_err());
        std::fs::remove_dir(folder.join("conv-1")).unwrap();

        // A file symlink in place of the child needs the symlink privilege (CI runners have it).
        write(&child, &[json!({"type":"user","message":{"content":"legit child"}})]);
        std::fs::remove_file(&child).unwrap();
        match std::os::windows::fs::symlink_file(elsewhere.join("agent-a1.jsonl"), &child) {
            Ok(()) => assert!(read_child(&path, &file, "a1", &mut budget).is_err()),
            Err(e) => eprintln!("skipped the file-link case: cannot create a file symlink here ({e})"),
        }
    }

    /// One replay reads its children within one bound together: a small recording that links the same
    /// or many children cannot make it read more than that, and a child larger than what is left is not
    /// read at all.
    #[test]
    fn children_of_one_replay_are_read_within_one_bound() {
        let temp = Temp::new();
        let project = temp.0.join("p");
        std::fs::create_dir_all(&project).unwrap();
        let root = temp.0.join("projects");
        let folder = root.join(project_key(&project.to_string_lossy()));
        write(&folder.join("conv-1.jsonl"), &[prompt(&project, "Parent")]);
        let subagents = folder.join("conv-1").join("subagents");
        write(&subagents.join("agent-a1.jsonl"), &[json!({"type":"user","message":{"content":"x".repeat(1000)}})]);
        let size = std::fs::metadata(subagents.join("agent-a1.jsonl")).unwrap().len();
        let (path, file) = open_claude(&root, &project, "conv-1").unwrap();

        let mut remaining = size * 2;
        assert!(read_child(&path, &file, "a1", &mut remaining).is_ok());
        assert_eq!(remaining, size);
        assert!(read_child(&path, &file, "a1", &mut remaining).is_ok());
        assert_eq!(remaining, 0);
        // Linked a third time, nothing more is read.
        assert_eq!(read_child(&path, &file, "a1", &mut remaining).unwrap_err(), "Subagent history exceeds the replay limit");

        // A child larger than what is left is refused, and ends the reading of children for this replay.
        let mut remaining = size - 1;
        assert_eq!(read_child(&path, &file, "a1", &mut remaining).unwrap_err(), "Subagent history exceeds the replay limit");
        assert_eq!(remaining, 0);
    }

    #[test]
    fn titles_prefer_custom_then_generated_then_first_prompt() {
        let temp = Temp::new();
        let root = temp.0.join("projects");
        let folder = root.join(project_key(&temp.0.to_string_lossy()));
        let cwd = temp.0.clone();
        write(&folder.join("prompt.jsonl"), &[
            json!({"type":"user","isMeta":true,"cwd":cwd,"message":{"content":"Meta line"}}),
            json!({"type":"user","cwd":cwd,"message":{"content":"<command-name>/model</command-name>"}}),
            json!({"type":"user","cwd":cwd,"message":{"content":[{"type":"tool_result","content":"x"}]}}),
            json!({"type":"user","cwd":cwd,"gitBranch":"main","message":{"content":[{"type":"text","text":"Real\n  question"}]}}),
        ]);
        write(&folder.join("generated.jsonl"), &[
            prompt(&cwd, "First words"),
            json!({"type":"ai-title","aiTitle":"Old title"}),
            json!({"type":"ai-title","aiTitle":"Generated title"}),
        ]);
        write(&folder.join("custom.jsonl"), &[
            prompt(&cwd, "First words"),
            json!({"type":"custom-title","customTitle":"Named by hand"}),
            json!({"type":"ai-title","aiTitle":"Generated title"}),
        ]);
        write(&folder.join("bare.jsonl"), &[json!({"type":"system","cwd":cwd})]);
        // A title written near the end of a recording larger than both windows is still found, while the
        // first prompt comes from the head.
        let mut large = vec![prompt(&cwd, "Opening prompt")];
        let filler = "x".repeat(4096);
        large.extend((0..300).map(|_| json!({"type":"assistant","cwd":cwd,"message":{"content":filler}})));
        large.push(json!({"type":"custom-title","customTitle":"Late title"}));
        write(&folder.join("large.jsonl"), &large);
        assert!(std::fs::metadata(folder.join("large.jsonl")).unwrap().len() > 2 * WINDOW);

        let listing = list_claude(&root, &cwd, "", 0, LIST_LIMIT).unwrap();
        let by_id = |id: &str| listing.find(id).unwrap().clone();
        assert_eq!(by_id("prompt").title, "Real question");
        assert_eq!(by_id("prompt").git_branch.as_deref(), Some("main"));
        assert_eq!(by_id("generated").title, "Generated title");
        assert_eq!(by_id("generated").first_prompt.as_deref(), Some("First words"));
        assert_eq!(by_id("custom").title, "Named by hand");
        assert_eq!(by_id("bare").title, "bare");
        assert!(by_id("bare").first_prompt.is_none());
        assert_eq!(by_id("large").title, "Late title");
        assert_eq!(by_id("large").first_prompt.as_deref(), Some("Opening prompt"));
        assert!(by_id("large").size_bytes > 2 * WINDOW);
    }

    #[test]
    fn newest_first_and_capped() {
        let temp = Temp::new();
        let root = temp.0.join("projects");
        let folder = root.join(project_key(&temp.0.to_string_lossy()));
        for (index, id) in ["old", "mid", "new"].iter().enumerate() {
            let path = folder.join(format!("{id}.jsonl"));
            write(&path, &[prompt(&temp.0, id)]);
            set_mtime(&path, 1_000_000 + index as u64 * 100);
        }
        let all = list_claude(&root, &temp.0, "", 0, LIST_LIMIT).unwrap();
        let ids: Vec<_> = all.conversations.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["new", "mid", "old"]);
        assert_eq!(all.conversations[0].updated_at, 1_000_200_000);
        let capped = list_claude(&root, &temp.0, "", 0, 2).unwrap();
        assert!(capped.truncated);
        assert_eq!(capped.conversations.len(), 2);
        assert_eq!(capped.conversations[1].id, "mid");
    }

    #[test]
    fn a_large_first_prompt_does_not_hide_the_conversation() {
        let temp = Temp::new();
        let root = temp.0.join("projects");
        let folder = root.join(project_key(&temp.0.to_string_lossy()));
        let image = "A".repeat(400 * 1024);
        // Claude's own first lines carry no working directory; the first one that does holds a screenshot
        // larger than the head window.
        write(&folder.join("big.jsonl"), &[
            json!({"type":"file-history-snapshot","snapshot":{"files":"x".repeat(300 * 1024)}}),
            json!({"type":"queue-operation","sessionId":"big"}),
            json!({"type":"user","cwd":temp.0,"message":{"content":[
                {"type":"image","source":{"type":"base64","data":image}},
                {"type":"text","text":"What is wrong here?"}]}}),
            json!({"type":"ai-title","aiTitle":"Screenshot question"}),
        ]);
        assert!(std::fs::metadata(folder.join("big.jsonl")).unwrap().len() > 2 * WINDOW);
        let listing = list_claude(&root, &temp.0, "", 0, LIST_LIMIT).unwrap();
        let big = listing.find("big").expect("the conversation is listed");
        assert_eq!(big.title, "Screenshot question");
        assert_eq!(big.first_prompt.as_deref(), Some("What is wrong here?"));
        // And the bind check finds it too.
        assert!(find_claude(&root, &temp.0, "big").unwrap().is_some());
        // A recording of another directory stays out however late its working directory appears.
        let other = temp.0.join("other");
        std::fs::create_dir_all(&other).unwrap();
        write(&folder.join("foreign.jsonl"), &[
            json!({"type":"file-history-snapshot","snapshot":{"files":"x".repeat(300 * 1024)}}),
            prompt(&other, "Not mine"),
        ]);
        assert!(list_claude(&root, &temp.0, "", 0, LIST_LIMIT).unwrap().find("foreign").is_none());
        assert!(find_claude(&root, &temp.0, "foreign").unwrap().is_none());
    }

    #[test]
    fn the_cap_counts_listed_conversations_only() {
        let temp = Temp::new();
        let root = temp.0.join("projects");
        let folder = root.join(project_key(&temp.0.to_string_lossy()));
        let foreign = temp.0.join("foreign");
        std::fs::create_dir_all(&foreign).unwrap();
        for (index, id) in ["mine-old", "mine-mid"].iter().enumerate() {
            let path = folder.join(format!("{id}.jsonl"));
            write(&path, &[prompt(&temp.0, id)]);
            set_mtime(&path, 1_000_000 + index as u64 * 100);
        }
        // Newer files that do not belong to the directory must not push the older listable ones out.
        for (index, id) in ["foreign-1", "no-cwd-1", "sidechain-1"].iter().enumerate() {
            let path = folder.join(format!("{id}.jsonl"));
            let line = match *id {
                "foreign-1" => prompt(&foreign, "Foreign"),
                "no-cwd-1" => json!({"type":"queue-operation"}),
                _ => json!({"type":"user","isSidechain":true,"cwd":temp.0}),
            };
            write(&path, &[line]);
            set_mtime(&path, 2_000_000 + index as u64 * 100);
        }
        let capped = list_claude(&root, &temp.0, "", 0, 2).unwrap();
        let ids: Vec<_> = capped.conversations.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["mine-mid", "mine-old"]);
        assert!(!capped.truncated, "nothing listable was left out");
        let one = list_claude(&root, &temp.0, "", 0, 1).unwrap();
        assert!(one.truncated);
        assert_eq!(one.conversations.len(), 1);
        // A conversation beyond the cap can still be continued.
        assert!(find_claude(&root, &temp.0, "mine-old").unwrap().is_some());
        assert!(find_claude(&root, &temp.0, "foreign-1").unwrap().is_none());
        assert!(find_claude(&root, &temp.0, "missing").unwrap().is_none());
    }

    #[test]
    fn every_conversation_is_reachable_by_search_and_paging() {
        let temp = Temp::new();
        let root = temp.0.join("projects");
        let folder = root.join(project_key(&temp.0.to_string_lossy()));
        let ids = ["c1", "c2", "c3", "c4", "c5"];
        for (index, id) in ids.iter().enumerate() {
            let path = folder.join(format!("{id}.jsonl"));
            let mut line = prompt(&temp.0, &format!("Prompt number {id}"));
            line["gitBranch"] = json!(format!("branch-{id}"));
            write(&path, &[line]);
            set_mtime(&path, 1_000_000 + index as u64 * 100);
        }
        // Page by page, newest first, every conversation comes up exactly once.
        let mut seen = Vec::new();
        let mut offset = 0;
        loop {
            let page = list_claude(&root, &temp.0, "", offset, 2).unwrap();
            offset = page.next_offset;
            seen.extend(page.conversations.into_iter().map(|c| c.id));
            if !page.truncated {
                break;
            }
        }
        assert_eq!(seen, ["c5", "c4", "c3", "c2", "c1"]);
        // The oldest one is found by a search even when a page holds a single conversation.
        for query in ["NUMBER C1", "branch-c1", "c1"] {
            let found = list_claude(&root, &temp.0, query, 0, 1).unwrap();
            let found: Vec<_> = found.conversations.iter().map(|c| c.id.as_str()).collect();
            assert_eq!(found, ["c1"], "{query}");
        }
        let none = list_claude(&root, &temp.0, "nothing like this", 0, 1).unwrap();
        assert!(none.conversations.is_empty() && !none.truncated);
    }

    #[test]
    fn the_replay_reads_the_recording_that_was_checked() {
        let temp = Temp::new();
        let root = temp.0.join("projects");
        let folder = root.join(project_key(&temp.0.to_string_lossy()));
        let other = temp.0.join("other");
        std::fs::create_dir_all(&other).unwrap();
        write(&folder.join("conv-1.jsonl"), &[prompt(&temp.0, "legit")]);
        let secret = root.join(project_key(&other.to_string_lossy())).join("secret.jsonl");
        write(&secret, &[prompt(&other, "FOREIGN SECRET")]);
        // Another folder below the root holds the same name first; the replay never looks there.
        #[cfg(unix)]
        {
            std::fs::create_dir_all(root.join("-A")).unwrap();
            std::os::unix::fs::symlink(&secret, root.join("-A").join("conv-1.jsonl")).unwrap();
        }
        std::fs::create_dir_all(root.join("-B")).unwrap();
        std::fs::copy(&secret, root.join("-B").join("conv-1.jsonl")).unwrap();
        let (path, content) = read_claude(&root, &temp.0, "conv-1").unwrap();
        assert_eq!(path, folder.join("conv-1.jsonl"));
        assert!(content.contains("legit") && !content.contains("FOREIGN"));
        assert!(find_claude(&root, &temp.0, "conv-1").unwrap().is_some_and(|c| c.title == "legit"));
        // Swapped for a link, or for another directory's recording, the checked file is gone: nothing is read.
        #[cfg(unix)]
        {
            std::fs::remove_file(folder.join("conv-1.jsonl")).unwrap();
            std::os::unix::fs::symlink(&secret, folder.join("conv-1.jsonl")).unwrap();
            assert!(read_claude(&root, &temp.0, "conv-1").is_err());
            std::fs::remove_file(folder.join("conv-1.jsonl")).unwrap();
            std::fs::hard_link(&secret, folder.join("conv-1.jsonl")).unwrap();
            assert!(read_claude(&root, &temp.0, "conv-1").is_err());
        }
        std::fs::remove_file(folder.join("conv-1.jsonl")).unwrap();
        assert!(read_claude(&root, &temp.0, "conv-1").is_err(), "no other folder stands in");
        for hostile in ["../other/secret", "-r", ""] {
            assert!(read_claude(&root, &temp.0, hostile).is_err(), "{hostile}");
        }
    }

    #[test]
    fn a_recording_is_never_read_beyond_the_scan_budget() {
        let temp = Temp::new();
        let root = temp.0.join("projects");
        let folder = root.join(project_key(&temp.0.to_string_lossy()));
        std::fs::create_dir_all(&folder).unwrap();
        // One line longer than the budget, with the working directory only after it.
        let mut file = std::fs::File::create(folder.join("huge.jsonl")).unwrap();
        let chunk = vec![b'x'; 1024 * 1024];
        for _ in 0..=(SCAN_BUDGET / chunk.len() as u64) {
            std::io::Write::write_all(&mut file, &chunk).unwrap();
        }
        std::io::Write::write_all(&mut file, format!("\n{}\n", prompt(&temp.0, "Too late")).as_bytes()).unwrap();
        drop(file);
        write(&folder.join("small.jsonl"), &[prompt(&temp.0, "Small")]);
        let ids: Vec<_> = list_claude(&root, &temp.0, "", 0, LIST_LIMIT).unwrap().conversations.into_iter().map(|c| c.id).collect();
        assert_eq!(ids, ["small"]);
        assert!(find_claude(&root, &temp.0, "huge").unwrap().is_none());
    }

    #[test]
    fn an_id_that_would_read_as_a_flag_is_not_listed() {
        let temp = Temp::new();
        let root = temp.0.join("projects");
        let folder = root.join(project_key(&temp.0.to_string_lossy()));
        write(&folder.join("--dangerously-skip-permissions.jsonl"), &[
            prompt(&temp.0, "Harmless"),
            json!({"type":"custom-title","customTitle":"Looks harmless"}),
        ]);
        write(&folder.join("-r.jsonl"), &[prompt(&temp.0, "Short flag")]);
        write(&folder.join("fine.jsonl"), &[prompt(&temp.0, "Fine")]);
        let ids: Vec<_> = list_claude(&root, &temp.0, "", 0, LIST_LIMIT).unwrap().conversations.into_iter().map(|c| c.id).collect();
        assert_eq!(ids, ["fine"]);
        assert!(find_claude(&root, &temp.0, "--dangerously-skip-permissions").unwrap().is_none());
        assert!(find_claude(&root, &temp.0, "../fine").unwrap().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn a_file_swapped_for_a_link_after_listing_is_not_read() {
        let temp = Temp::new();
        let root = temp.0.join("projects");
        let folder = root.join(project_key(&temp.0.to_string_lossy()));
        let path = folder.join("swapped.jsonl");
        write(&path, &[prompt(&temp.0, "Original")]);
        write(&temp.0.join("secret.jsonl"), &[prompt(&temp.0, "Secret prompt")]);
        let listed = candidates(&root, &temp.0, None).unwrap();
        assert_eq!(listed.len(), 1);
        // The swap happens between the directory scan and the read.
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(temp.0.join("secret.jsonl"), &path).unwrap();
        let candidate = listed.into_iter().next().unwrap();
        assert!(describe(candidate, &temp.0, &mut 0).is_none());
        // A regular file put in its place is another file than the one listed, and is not read either.
        let listed = candidates(&root, &temp.0, None).unwrap();
        assert!(listed.is_empty(), "the link itself is not a candidate");
        std::fs::remove_file(&path).unwrap();
        write(&path, &[prompt(&temp.0, "Original")]);
        let listed = candidates(&root, &temp.0, None).unwrap();
        // Created while the listed file still exists, so it cannot reuse its inode.
        std::fs::copy(temp.0.join("secret.jsonl"), temp.0.join("replacement")).unwrap();
        std::fs::rename(temp.0.join("replacement"), &path).unwrap();
        assert!(describe(listed.into_iter().next().unwrap(), &temp.0, &mut 0).is_none());
    }

    #[test]
    fn one_page_costs_what_its_bounds_allow_however_many_files_there_are() {
        let temp = Temp::new();
        let root = temp.0.join("projects");
        let folder = root.join(project_key(&temp.0.to_string_lossy()));
        for index in 0..10u64 {
            let path = folder.join(format!("c{index}.jsonl"));
            write(&path, &[prompt(&temp.0, &format!("Prompt {index}"))]);
            set_mtime(&path, 1_000_000 + index * 100);
        }
        let size = std::fs::metadata(folder.join("c0.jsonl")).unwrap().len();
        // A search without a hit stops after the files it may describe and says where to go on.
        let page = list_within(&root, &temp.0, "no such text", 0, LIST_LIMIT, (4, u64::MAX)).unwrap();
        assert!(page.conversations.is_empty() && page.truncated);
        assert_eq!(page.next_offset, 4);
        // The same for the bytes it may read: the file under way is finished, then the page ends.
        let page = list_within(&root, &temp.0, "no such text", 0, LIST_LIMIT, (usize::MAX, 2 * size)).unwrap();
        assert!(page.truncated);
        assert_eq!(page.next_offset, 2);
        // Continuing from `next_offset` reaches the oldest conversation without looking at any file twice.
        let mut offset = 0;
        let mut found = Vec::new();
        loop {
            let page = list_within(&root, &temp.0, "prompt 0", offset, LIST_LIMIT, (3, u64::MAX)).unwrap();
            assert!(page.next_offset > offset || !page.truncated);
            found.extend(page.conversations.into_iter().map(|c| c.id));
            offset = page.next_offset;
            if !page.truncated {
                break;
            }
        }
        assert_eq!(found, ["c0"]);
        assert_eq!(offset, 10);
        // An offset past the end reads nothing and ends the listing.
        let page = list_claude(&root, &temp.0, "", usize::MAX, LIST_LIMIT).unwrap();
        assert!(page.conversations.is_empty() && !page.truncated);
        assert_eq!(page.next_offset, 10);
    }

    #[test]
    fn a_reader_by_id_gets_the_recording_claude_wrote_for_its_directory() {
        let temp = Temp::new();
        let root = temp.0.join("projects");
        let project = temp.0.join("project");
        let other = temp.0.join("other");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let folder = root.join(project_key(&project.to_string_lossy()));
        write(&folder.join("conv-1.jsonl"), &[prompt(&project, "legit")]);
        let secret = root.join(project_key(&other.to_string_lossy())).join("secret.jsonl");
        write(&secret, &[prompt(&other, "FOREIGN SECRET")]);
        // Folders that may come first below the root hold the id as a hard link, a link and a copy of
        // another directory's recording, and a subagent's recording.
        for folder in ["-0", "-A", "-B", "-C"] {
            std::fs::create_dir_all(root.join(folder)).unwrap();
        }
        #[cfg(unix)]
        {
            std::fs::hard_link(&secret, root.join("-0").join("conv-1.jsonl")).unwrap();
            std::os::unix::fs::symlink(&secret, root.join("-A").join("conv-1.jsonl")).unwrap();
        }
        std::fs::copy(&secret, root.join("-B").join("conv-1.jsonl")).unwrap();
        write(&root.join("-C").join("conv-1.jsonl"), &[json!({"type":"user","isSidechain":true,"cwd":"/C"})]);
        let (path, content) = read_recording(&root, "conv-1").unwrap();
        assert_eq!(path, folder.join("conv-1.jsonl"));
        assert!(content.contains("legit") && !content.contains("FOREIGN"));
        let (_, file) = open_recording(&root, "conv-1").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let (opened, listed) = (file.metadata().unwrap(), std::fs::metadata(&path).unwrap());
            assert_eq!((opened.dev(), opened.ino()), (listed.dev(), listed.ino()), "the handle is the checked file");
        }
        // A second recording that also belongs to its own folder cannot be told apart: neither is read.
        std::fs::copy(&secret, secret.with_file_name("conv-1.jsonl")).unwrap();
        assert!(read_recording(&root, "conv-1").unwrap_err().contains("Several"));
        // The replay, which knows the directory, still reads the checked one.
        assert!(read_claude(&root, &project, "conv-1").unwrap().1.contains("legit"));
        // Gone from its own folder, a copy placed elsewhere does not stand in, and hostile ids read nothing.
        std::fs::remove_file(secret.with_file_name("conv-1.jsonl")).unwrap();
        std::fs::remove_file(folder.join("conv-1.jsonl")).unwrap();
        assert!(read_recording(&root, "conv-1").is_err());
        for hostile in ["../other/secret", "-r", ""] {
            assert!(read_recording(&root, hostile).is_err(), "{hostile}");
        }
    }

    #[test]
    fn long_directories_match_shortened_keys_and_still_check_the_directory() {
        let temp = Temp::new();
        let mut project = temp.0.clone();
        while project.to_string_lossy().len() <= MAX_KEY + 20 {
            project = project.join("a-rather-long-directory-name");
        }
        std::fs::create_dir_all(&project).unwrap();
        let other = project.parent().unwrap().join("a-rather-long-directory-name-too");
        std::fs::create_dir_all(&other).unwrap();
        let root = temp.0.join("projects");
        let key = project_key(&project.to_string_lossy());
        let shortened = &key[..MAX_KEY];
        write(&root.join(format!("{shortened}-hash1")).join("mine.jsonl"), &[prompt(&project, "Mine")]);
        write(&root.join(format!("{shortened}-hash2")).join("theirs.jsonl"), &[prompt(&other, "Theirs")]);
        write(&root.join(format!("{shortened}x-hash3")).join("unrelated.jsonl"), &[prompt(&project, "No")]);
        let ids: Vec<_> = list_claude(&root, &project, "", 0, LIST_LIMIT).unwrap().conversations.into_iter().map(|c| c.id).collect();
        assert_eq!(ids, ["mine"]);
    }
}
