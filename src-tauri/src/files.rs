//! Filesystem browser for the right-hand Files tab, listing directories and previewing real content from the current
//! session cwd. Directory traversal is lazy, one level at a time.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::Read;

use serde::Serialize;

/// One direct child of a directory.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntryInfo {
    pub name: String,
    pub is_dir: bool,
    /// Whether the name begins with `.`, hidden by default but toggleable like a system file browser. `.git` is
    /// skipped entirely by the backend and never returned.
    pub is_hidden: bool,
    /// Git change badge (M/A/U/D/R/!) for a file or a directory's changed descendants.
    pub git_badge: Option<String>,
}

/// Lists direct children with directories (including symlinks to directories) first, then case-insensitive names,
/// skipping `.git`. Dotfiles are returned with `is_hidden`. Changed files and directories receive Git badges; non-repositories
/// produce none.
pub fn list_dir(path: &str) -> Result<Vec<DirEntryInfo>, String> {
    let badges = git_badges(path)?;

    let mut entries: Vec<DirEntryInfo> = Vec::new();
    let rd = std::fs::read_dir(path).map_err(|e| format!("Failed to read directory: {e}"))?;
    for ent in rd.flatten() {
        let name = ent.file_name().to_string_lossy().to_string();
        if name == ".git" {
            continue;
        }
        let is_hidden = name.starts_with('.') || has_hidden_attribute(&ent);
        let is_dir = match ent.file_type() {
            // DirEntry::file_type does not follow symlinks. Path::is_dir lets directory links remain expandable and
            // selectable while file links and broken links remain non-directories.
            Ok(ft) if ft.is_symlink() => ent.path().is_dir(),
            Ok(ft) => ft.is_dir(),
            Err(_) => ent.path().is_dir(),
        };
        let git_badge = badges.get(&name).cloned();
        entries.push(DirEntryInfo {
            name,
            is_dir,
            is_hidden,
            git_badge,
        });
    }

    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
}

/// Resolve root-relative porcelain paths against the listed directory, aggregating descendant changes.
/// The backend owns badge precedence; clients only display these decorations.
fn git_badges(path: &str) -> Result<HashMap<String, String>, String> {
    let mut map = HashMap::new();
    let out = crate::host::command("git")
        .args(["--no-optional-locks", "-C", path, "rev-parse", "--show-toplevel"])
        .output();
    let out = match out {
        Ok(o) if o.status.success() => o,
        _ => return Ok(map),
    };
    let root = std::fs::canonicalize(String::from_utf8_lossy(&out.stdout).trim_end_matches(['\n', '\r']))
        .map_err(|e| format!("Failed to resolve Git root: {e}"))?;
    let directory = std::fs::canonicalize(path).map_err(|e| format!("Failed to resolve directory: {e}"))?;
    let priority = |badge: &str| match badge { "!" => 6, "M" => 5, "D" => 4, "R" => 3, "A" => 2, _ => 1 };
    let child_name = |relative: &str| {
        let absolute = root.join(relative);
        let local = absolute.strip_prefix(&directory).ok()?;
        Some(local.components().next()?.as_os_str().to_string_lossy().into_owned())
    };
    for entry in crate::git::path_status(path, true)? {
        let badge = crate::git::status_badge(&entry.xy);
        let destination = child_name(&entry.path);
        // Moving between child directories also decorates the source; a rename within one child stays R.
        let source = entry.original_path.as_deref().filter(|_| entry.xy.contains('R'))
            .and_then(child_name).filter(|name| Some(name) != destination.as_ref());
        for (name, mark) in destination.into_iter().map(|name| (name, badge))
            .chain(source.into_iter().map(|name| (name, "D"))) {
            let previous = map.entry(name).or_insert_with(|| mark.to_string());
            if priority(mark) > priority(previous) { *previous = mark.to_string(); }
        }
    }
    Ok(map)
}

#[cfg(test)]
mod git_badge_tests {
    use super::*;

    struct Repository(std::path::PathBuf);
    impl Drop for Repository {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }
    fn git(path: &std::path::Path, args: &[&str]) {
        let result = crate::host::command("git").arg("-C").arg(path).args(args).output().unwrap();
        assert!(result.status.success(), "Git command failed: {:?}", args);
    }

    /// Nested paths, Unicode, literal arrows, renames, and untracked descendants get real badges.
    #[test]
    fn nested_file_and_directory_decorations() {
        let repo = Repository(std::env::temp_dir().join(format!("vlx-files-badges-{}", uuid::Uuid::new_v4())));
        let root = &repo.0;
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        git(root, &["init"]);
        git(root, &["config", "user.name", "Test"]);
        git(root, &["config", "user.email", "test@example.test"]);
        std::fs::write(src.join("文件 -> note.txt"), "original\n").unwrap();
        std::fs::write(src.join("old.txt"), "rename\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-m", "Initial"]);
        std::fs::write(src.join("文件 -> note.txt"), "changed\n").unwrap();
        git(root, &["mv", "src/old.txt", "src/new.txt"]);
        std::fs::create_dir_all(src.join("untracked")).unwrap();
        std::fs::write(src.join("untracked/deep.txt"), "new\n").unwrap();
        let top = list_dir(&root.to_string_lossy()).unwrap();
        assert_eq!(top.iter().find(|e| e.name == "src").unwrap().git_badge.as_deref(), Some("M"));
        let entries = list_dir(&src.to_string_lossy()).unwrap();
        let badge = |name: &str| entries.iter().find(|e| e.name == name).unwrap().git_badge.as_deref();
        assert_eq!(badge("文件 -> note.txt"), Some("M"));
        assert_eq!(badge("new.txt"), Some("R"));
        assert_eq!(badge("untracked"), Some("U"));
        let nested = list_dir(&src.join("untracked").to_string_lossy()).unwrap();
        assert_eq!(nested[0].git_badge.as_deref(), Some("U"));
        let changes = crate::git::changed_files(&src.to_string_lossy()).unwrap();
        assert!(changes.iter().any(|f| f.path == "src/文件 -> note.txt"));
        git(root, &["restore", "src/文件 -> note.txt"]);
        let clean = list_dir(&src.to_string_lossy()).unwrap();
        assert_eq!(clean.iter().find(|e| e.name == "文件 -> note.txt").unwrap().git_badge, None);
        let renamed = list_dir(&root.to_string_lossy()).unwrap();
        assert_eq!(renamed.iter().find(|e| e.name == "src").unwrap().git_badge.as_deref(), Some("R"));
        std::fs::create_dir_all(root.join("destination")).unwrap();
        git(root, &["mv", "src/new.txt", "destination/new.txt"]);
        let moved = list_dir(&root.to_string_lossy()).unwrap();
        assert_eq!(moved.iter().find(|e| e.name == "src").unwrap().git_badge.as_deref(), Some("D"));
        assert_eq!(moved.iter().find(|e| e.name == "destination").unwrap().git_badge.as_deref(), Some("R"));
    }
}

/// File preview loaded when a file is selected in the right pane.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    /// Text content, empty for binary files.
    pub content: String,
    /// Whether content exceeded the read limit and was truncated.
    pub truncated: bool,
    /// Whether the file is binary and should not display content.
    pub binary: bool,
}

/// Reads up to 64 KB for preview; NUL-containing files are treated as binary and not displayed.
pub fn read_preview(path: &str) -> Result<FilePreview, String> {
    const MAX: usize = 64 * 1024;
    let f = std::fs::File::open(path).map_err(|e| format!("Failed to open file: {e}"))?;
    let mut bytes = Vec::new();
    f.take((MAX as u64) + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read file: {e}"))?;
    let truncated = bytes.len() > MAX;
    bytes.truncate(MAX);
    let binary = bytes.contains(&0);
    if binary {
        return Ok(FilePreview {
            content: String::new(),
            truncated,
            binary: true,
        });
    }
    Ok(FilePreview {
        content: String::from_utf8_lossy(&bytes).to_string(),
        truncated,
        binary: false,
    })
}

// Directory-tree create, rename/move, and delete operations for Files controls.

/// Creates an empty file at an absolute path, erroring if it exists and never overwriting.
pub fn create_file(path: &str) -> Result<(), String> {
    use std::io::ErrorKind;
    // create_new(true) guarantees failure rather than overwrite when the file exists.
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            Err("A file or folder with this name already exists".to_string())
        }
        Err(e) => Err(format!("Failed to create file: {e}")),
    }
}

/// Windows marks entries such as `AppData` and `NTUSER.DAT` hidden by attribute rather than by a leading dot.
#[cfg(windows)]
fn has_hidden_attribute(ent: &std::fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    ent.metadata().map(|m| m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0).unwrap_or(false)
}

#[cfg(not(windows))]
fn has_hidden_attribute(_ent: &std::fs::DirEntry) -> bool {
    false
}

/// File-system roots offered by the server folder picker, with the host OS so the client can choose path
/// syntax and labels: mounted drive roots such as `C:\` on Windows, `/` everywhere else.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsRoots {
    pub os: &'static str,
    pub host_name: Option<String>,
    pub home: Option<String>,
    pub roots: Vec<String>,
}

pub fn list_roots() -> FsRoots {
    #[cfg(windows)]
    let roots = (b'A'..=b'Z')
        .map(|letter| format!("{}:\\", letter as char))
        .filter(|root| std::path::Path::new(root).is_dir())
        .collect();
    #[cfg(not(windows))]
    let roots = vec!["/".to_string()];
    FsRoots {
        os: std::env::consts::OS,
        host_name: sysinfo::System::host_name(),
        home: crate::host::home_dir().map(|p| p.to_string_lossy().to_string()),
        roots,
    }
}

/// Validate a single child name on the executing host; UI checks never replace this rule.
pub fn validate_child_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\')
        || name.chars().any(|c| c == '\0' || c.is_control()) {
        return Err("Enter a single file or folder name without path separators".to_string());
    }
    #[cfg(windows)]
    {
        let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
        let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'));
        if reserved || name.ends_with('.') || name.ends_with(' ')
            || name.chars().any(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*')) {
            return Err("This name is not valid on Windows".to_string());
        }
    }
    Ok(())
}

/// Read-only preview for project forms and Save As. Paths and default names come from this host.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DestinationPreview {
    pub parent: String,
    pub name: String,
    pub path: String,
    pub problem: Option<&'static str>,
    pub field: Option<&'static str>,
    pub existing_kind: Option<&'static str>,
}

pub fn preview_destination(parent: &str, name: Option<&str>, repository: Option<&str>) -> DestinationPreview {
    let name = name.map(str::trim).map(str::to_string)
        .unwrap_or_else(|| repository.and_then(crate::git::derive_clone_dir_name).unwrap_or_default());
    let input = parent.trim();
    let expanded = if input == "~" || input.starts_with("~/") || input.starts_with("~\\") {
        crate::host::home_dir().map(|home| if input == "~" { home } else { home.join(&input[2..]) })
            .unwrap_or_else(|| std::path::PathBuf::from(input))
    } else {
        std::path::PathBuf::from(input)
    };
    let mut result = DestinationPreview {
        parent: expanded.to_string_lossy().to_string(), name,
        path: String::new(), problem: None, field: None, existing_kind: None,
    };
    let fail = |result: &mut DestinationPreview, problem, field| {
        result.problem = Some(problem);
        result.field = Some(field);
    };
    if input.is_empty() {
        fail(&mut result, "emptyParent", "location");
        return result;
    }
    if !expanded.is_absolute() {
        fail(&mut result, "notAbsolute", "location");
        return result;
    }
    if result.name.is_empty() {
        fail(&mut result, "emptyName", "name");
        return result;
    }
    if validate_child_name(&result.name).is_err() {
        fail(&mut result, "invalidName", "name");
        return result;
    }
    // Canonicalization resolves `..` and symlinks before displaying the exact path that will be submitted.
    let canonical = match std::fs::canonicalize(&expanded) {
        Ok(path) if path.is_dir() && std::fs::read_dir(&path).is_ok() => path,
        _ => {
            fail(&mut result, "missingParent", "location");
            return result;
        }
    };
    let display = canonical.to_string_lossy().to_string();
    #[cfg(windows)]
    let display = display.strip_prefix("\\\\?\\UNC\\").map(|p| format!("\\\\{p}"))
        .or_else(|| display.strip_prefix("\\\\?\\").map(str::to_string)).unwrap_or(display);
    result.parent = display;
    let target = std::path::Path::new(&result.parent).join(&result.name);
    result.path = target.to_string_lossy().to_string();
    match std::fs::symlink_metadata(&target) {
        Ok(_) => result.existing_kind = Some(if target.is_dir() { "directory" } else { "file" }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(_) => fail(&mut result, "unavailable", "location"),
    }
    result
}

/// Creates one directory level at an absolute path; errors if it exists or its parent is missing.
pub fn create_dir(path: &str) -> Result<(), String> {
    use std::io::ErrorKind;
    match std::fs::create_dir(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            Err("A file or folder with this name already exists".to_string())
        }
        Err(e) => Err(format!("Failed to create folder: {e}")),
    }
}

/// Renames/moves a file or directory. Explicitly reject an existing destination because Unix `std::fs::rename`
/// would silently overwrite a same-named file.
pub fn rename_path(from: &str, to: &str) -> Result<(), String> {
    if std::path::Path::new(to).exists() {
        return Err("A file or folder with this name already exists".to_string());
    }
    std::fs::rename(from, to).map_err(|e| format!("Failed to rename: {e}"))
}

/// Deletes a file or recursively deletes a directory. symlink_metadata ensures links themselves are removed without
/// following and deleting target directory contents.
pub fn delete_path(path: &str) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| format!("Failed to read path: {e}"))?;
    if meta.is_dir() {
        std::fs::remove_dir_all(path).map_err(|e| format!("Failed to delete folder: {e}"))
    } else {
        std::fs::remove_file(path).map_err(|e| format!("Failed to delete file: {e}"))
    }
}

/// Copies files and folders dropped from the OS file manager into `dir`, returning the paths created.
///
/// Nothing is ever overwritten: a name already taken in `dir` gets a ` (2)`, ` (3)`… suffix before its
/// extension, the way file managers keep both. A source that already sits directly in `dir` is skipped, since
/// dragging a file back onto its own folder is almost always a slip rather than a request for a duplicate.
/// Copying stops at the first failure; whatever was copied before it stays and is listed in the error.
pub fn copy_into_dir(sources: &[String], dir: &str) -> Result<Vec<String>, String> {
    use std::path::Path;
    let dir_path = Path::new(dir);
    if !dir_path.is_dir() {
        return Err("The destination folder does not exist".to_string());
    }
    let dir_real = dir_path.canonicalize().map_err(|e| format!("Failed to read destination folder: {e}"))?;
    let mut created = Vec::new();
    for source in sources {
        let src = Path::new(source);
        let result = (|| {
            let meta = std::fs::symlink_metadata(src).map_err(|e| format!("Failed to read {source}: {e}"))?;
            let name = src.file_name().ok_or_else(|| format!("Cannot copy {source}"))?;
            let src_real = src.canonicalize().unwrap_or_else(|_| src.to_path_buf());
            if src_real.parent() == Some(dir_real.as_path()) {
                return Ok(None);
            }
            if meta.is_dir() && dir_real.starts_with(&src_real) {
                return Err("Cannot copy a folder into itself".to_string());
            }
            let dest = unique_destination(dir_path, name);
            copy_entry(src, &dest, &meta).map_err(|e| format!("Failed to copy {source}: {e}"))?;
            Ok(Some(dest.to_string_lossy().into_owned()))
        })();
        match result {
            Ok(Some(dest)) => created.push(dest),
            Ok(None) => {}
            Err(e) if created.is_empty() => return Err(e),
            Err(e) => return Err(format!("{e} (copied before the failure: {})", created.join(", "))),
        }
    }
    Ok(created)
}

/// First free path for `name` in `dir`: the name itself, then `stem (2).ext`, `stem (3).ext`, and so on.
fn unique_destination(dir: &std::path::Path, name: &std::ffi::OsStr) -> std::path::PathBuf {
    let first = dir.join(name);
    if std::fs::symlink_metadata(&first).is_err() {
        return first;
    }
    let as_path = std::path::Path::new(name);
    let stem = as_path.file_stem().unwrap_or(name).to_string_lossy();
    let ext = as_path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (2u32..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| std::fs::symlink_metadata(p).is_err())
        .expect("an unused name exists")
}

/// Copies one file, folder (recursively), or symlink to a destination that must not exist yet.
fn copy_entry(src: &std::path::Path, dest: &std::path::Path, meta: &std::fs::Metadata) -> std::io::Result<()> {
    if meta.file_type().is_symlink() {
        // Recreate the link itself on Unix; Windows needs privileges for that, so copy what it points to.
        #[cfg(unix)]
        return std::os::unix::fs::symlink(std::fs::read_link(src)?, dest);
        #[cfg(not(unix))]
        {
            let target = std::fs::metadata(src)?;
            return copy_entry_followed(src, dest, &target);
        }
    }
    copy_entry_followed(src, dest, meta)
}

fn copy_entry_followed(src: &std::path::Path, dest: &std::path::Path, meta: &std::fs::Metadata) -> std::io::Result<()> {
    if meta.is_dir() {
        std::fs::create_dir(dest)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            copy_entry(&entry.path(), &dest.join(entry.file_name()), &entry.metadata()?)?;
        }
        return Ok(());
    }
    // create_new refuses an existing destination, so a file that appeared since the name was chosen survives.
    let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(dest)?;
    std::io::copy(&mut std::fs::File::open(src)?, &mut out)?;
    std::fs::set_permissions(dest, meta.permissions())
}

// Document editor read/write/stat.

/// Full editor-read limit. Larger files load only the first `EDIT_MAX` bytes as read-only to protect memory/UI.
const EDIT_MAX: u64 = 10 * 1024 * 1024;

/// Editor-read result with content, disk mtime, truncation, and real size. mtime is the optimistic-lock baseline.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextFile {
    pub content: String,
    pub mtime_ms: u64,
    /// File exceeded `EDIT_MAX`; only a valid UTF-8 prefix was read. Frontend switches to read-only with a truncation
    /// notice and forbids saving so incomplete content cannot overwrite the remainder.
    pub truncated: bool,
    /// Real disk size used by the frontend's displayed-prefix/total notice.
    pub full_size: u64,
}

/// Write result. `conflict=true` means mtime differed and nothing was written; success returns a new baseline mtime
/// so the local save does not trigger an external-change banner.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteOutcome {
    pub conflict: bool,
    pub mtime_ms: u64,
}

/// Lightweight mtime/size stat for frontend external-change polling.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStat {
    pub mtime_ms: u64,
    pub size: u64,
}

/// Gets file mtime in Unix milliseconds.
fn mtime_ms_of(meta: &std::fs::Metadata) -> Result<u64, String> {
    let t = meta
        .modified()
        .map_err(|e| format!("Failed to read file time: {e}"))?;
    let d = t
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("Invalid file time: {e}"))?;
    Ok(d.as_millis() as u64)
}

/// Reads a text document for editing, unlike the 64 KB `read_preview`.
///
/// - Up to 10 MB: read fully and allow editing.
/// - Above 10 MB: read a valid UTF-8 prefix, mark truncated, and open read-only rather than refusing all viewing.
/// - Require strict UTF-8 to prevent lossy round-trip corruption. A cut multibyte character at the truncation edge
///   may be dropped, but invalid bytes within content error.
/// - NUL bytes classify the file as binary and reject editing.
pub fn read_text_file(path: &str) -> Result<TextFile, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Failed to read file metadata: {e}"))?;
    if !meta.is_file() {
        return Err("Not a regular file".to_string());
    }
    let full_size = meta.len();
    let truncated = full_size > EDIT_MAX;

    // Read only EDIT_MAX bytes for oversized read-only files; otherwise read the complete file.
    let bytes = if truncated {
        let mut buf = Vec::new();
        let f = std::fs::File::open(path).map_err(|e| format!("Failed to open file: {e}"))?;
        f.take(EDIT_MAX)
            .read_to_end(&mut buf)
            .map_err(|e| format!("Failed to read file: {e}"))?;
        buf
    } else {
        std::fs::read(path).map_err(|e| format!("Failed to read file: {e}"))?
    };

    if bytes.contains(&0) {
        return Err("Binary file; cannot edit as text".to_string());
    }

    let content = match std::str::from_utf8(&bytes) {
        Ok(s) => s.to_string(),
        // If truncation cuts a final multibyte character, accept the valid prefix. Other UTF-8 errors are real corruption.
        Err(e) if truncated && e.error_len().is_none() => {
            std::str::from_utf8(&bytes[..e.valid_up_to()])
                .map_err(|_| {
                    "File is not UTF-8 encoded; refusing to edit to avoid corruption".to_string()
                })?
                .to_string()
        }
        Err(_) => {
            return Err(
                "File is not UTF-8 encoded; refusing to edit to avoid corruption".to_string(),
            )
        }
    };
    // Read mtime after content to minimize the race window; optimistic locking still protects save.
    let meta = std::fs::metadata(path).map_err(|e| format!("Failed to read file metadata: {e}"))?;
    Ok(TextFile {
        content,
        mtime_ms: mtime_ms_of(&meta)?,
        truncated,
        full_size,
    })
}

/// Writes a text file with optimistic mtime locking.
///
/// When a provided expected mtime differs, do **not write** and return conflict. Frontend may confirm and retry with None.
pub fn write_text_file(
    path: &str,
    content: &str,
    expected_mtime_ms: Option<u64>,
) -> Result<WriteOutcome, String> {
    if let Some(expected) = expected_mtime_ms {
        let meta =
            std::fs::metadata(path).map_err(|e| format!("Failed to read file metadata: {e}"))?;
        let current = mtime_ms_of(&meta)?;
        if current != expected {
            return Ok(WriteOutcome {
                conflict: true,
                mtime_ms: current,
            });
        }
    }
    std::fs::write(path, content).map_err(|e| format!("Failed to write file: {e}"))?;
    let meta = std::fs::metadata(path).map_err(|e| format!("Failed to read file metadata: {e}"))?;
    Ok(WriteOutcome {
        conflict: false,
        mtime_ms: mtime_ms_of(&meta)?,
    })
}

/// Overwrites a path with arbitrary bytes for PDF and similar output, without mtime conflict detection. Shared by
/// native Tauri and Web/Electron transports.
pub fn write_bytes(path: &str, data: &[u8]) -> Result<(), String> {
    std::fs::write(path, data).map_err(|e| format!("Failed to write file: {e}"))
}

/// Appends one chunk of an upload, creating and truncating the file at offset 0.
///
/// Counterpart of `read_file_base64`: whole-file writes cannot cross the WebSocket frame limit, so an upload
/// walks the file sequentially and each call carries the offset it believes it is at. That offset must equal
/// the file's current length — a mismatch means a stale retry or a concurrent writer raced us, and silently
/// seeking would splice two different uploads into one corrupt file. Returns the new length so the caller can
/// verify its own progress accounting.
///
/// Callers upload into a temporary sibling and rename on completion, so an interrupted transfer never leaves a
/// truncated file at the destination path.
pub fn write_file_chunk(path: &str, offset: u64, data: &[u8]) -> Result<u64, String> {
    use std::io::{Seek, SeekFrom, Write};

    if data.len() as u64 > CHUNK_MAX {
        return Err(format!(
            "Chunk too large ({} bytes, limit {CHUNK_MAX})",
            data.len()
        ));
    }
    let mut f = if offset == 0 {
        // A restarted upload reuses the same temporary path, so the first chunk must discard any leftovers.
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .map_err(|e| format!("Failed to create file: {e}"))?
    } else {
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|e| format!("Failed to open file: {e}"))?;
        let len = f
            .metadata()
            .map_err(|e| format!("Failed to read file metadata: {e}"))?
            .len();
        if len != offset {
            return Err(format!("Upload offset mismatch (file is {len}, chunk is {offset})"));
        }
        f
    };
    f.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("Failed to seek file: {e}"))?;
    f.write_all(data)
        .map_err(|e| format!("Failed to write file: {e}"))?;
    Ok(offset + data.len() as u64)
}

/// Lightweight stat used by the frontend's two-second external-change poll.
pub fn stat_file(path: &str) -> Result<FileStat, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Failed to read file metadata: {e}"))?;
    if !meta.is_file() {
        return Err("Not a regular file".to_string());
    }
    Ok(FileStat {
        mtime_ms: mtime_ms_of(&meta)?,
        size: meta.len(),
    })
}

// Chunked binary reading and writing for the image viewer and for file transfer.

/// Per-call byte ceiling for chunked binary reads and writes.
///
/// This bounds a single WebSocket message: base64 inflates payloads by a third and tokio-tungstenite defaults
/// to a 16 MiB frame, so one call must stay well under it.
const CHUNK_MAX: u64 = 8 * 1024 * 1024;

/// Whole-file ceiling for `read_file_base64`, whose caller holds the result in memory as one Blob.
///
/// Uploads are not bounded by it (they are written chunk by chunk with no whole-file read) and neither are
/// downloads, which stream over `/api/download` instead of passing through this function.
const BLOB_MAX: u64 = 50 * 1024 * 1024;

/// One `read_file_base64` chunk with base64 bytes, total size, and current mtime.
///
/// Each byte range is encoded independently, so frontend atob assembly does not require three-byte-aligned offsets.
/// Every chunk carries total size for progress and mtime for cross-chunk consistency; changed mtime triggers reread.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChunk {
    pub base64: String,
    pub size: u64,
    pub mtime_ms: u64,
}

/// Reads `[offset, offset+max_len)` and base64-encodes it for chunked image loading.
///
/// Requires a regular file no larger than `BLOB_MAX`, with no extension restriction for future binary
/// previews. The ceiling exists because the only caller assembles the whole file in memory as one Blob;
/// downloads do not come through here at all, they stream over `/api/download`. An out-of-range offset returns
/// an empty chunk; the frontend stops after collecting `size` bytes.
pub fn read_file_base64(path: &str, offset: u64, max_len: u64) -> Result<FileChunk, String> {
    use std::io::{Seek, SeekFrom};

    let meta = std::fs::metadata(path).map_err(|e| format!("Failed to read file metadata: {e}"))?;
    if !meta.is_file() {
        return Err("Not a regular file".to_string());
    }
    let size = meta.len();
    if size > BLOB_MAX {
        return Err(format!(
            "File too large ({:.1}MB, limit 50MB)",
            size as f64 / 1024.0 / 1024.0
        ));
    }
    let mtime_ms = mtime_ms_of(&meta)?;

    let mut bytes = Vec::new();
    if offset < size {
        let mut f = std::fs::File::open(path).map_err(|e| format!("Failed to open file: {e}"))?;
        f.seek(SeekFrom::Start(offset))
            .map_err(|e| format!("Failed to seek file: {e}"))?;
        f.take(max_len.min(CHUNK_MAX))
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Failed to read file: {e}"))?;
    }
    Ok(FileChunk {
        base64: crate::agent::inject::base64_encode(&bytes),
        size,
        mtime_ms,
    })
}

/// Saves a pasted/dropped image to a local temporary directory and returns its absolute path.
///
/// Browser/desktop uploads clipboard images to the server, writes the resulting path into the terminal, and lets
/// agents such as Claude read it.
///
/// Uses `temp_dir()/vlx-uploads/` for a space-free shell-safe path, random collision-free names, and system cleanup.
pub fn save_pasted_image(bytes: &[u8], ext: &str) -> Result<String, String> {
    let safe_ext = sanitize_ext(ext);
    let dir = std::env::temp_dir().join("vlx-uploads");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create upload directory: {e}"))?;
    let name = format!("{}.{}", uuid::Uuid::new_v4(), safe_ext);
    let path = dir.join(name);
    std::fs::write(&path, bytes).map_err(|e| format!("Failed to write image: {e}"))?;
    // Register for process-exit cleanup rather than relying solely on system temp cleanup.
    if let Ok(mut reg) = pasted_image_registry().lock() {
        reg.push(path.clone());
    }
    Ok(path.to_string_lossy().to_string())
}

/// Temporary images created by this process. Clean them on desktop/headless exit so Windows temp storage does not
/// grow indefinitely. Track **only this process's files** so concurrent instances never delete one another's images.
fn pasted_image_registry() -> &'static std::sync::Mutex<Vec<std::path::PathBuf>> {
    static REG: std::sync::OnceLock<std::sync::Mutex<Vec<std::path::PathBuf>>> =
        std::sync::OnceLock::new();
    REG.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Deletes this process's pasted images at exit and returns the successful count. Ignore missing/busy files and clear registration.
pub fn cleanup_pasted_images() -> usize {
    let paths = match pasted_image_registry().lock() {
        Ok(mut reg) => std::mem::take(&mut *reg),
        Err(_) => return 0,
    };
    remove_all(&paths)
}

/// Deletes provided files individually, counting successes and ignoring missing/busy failures; extracted for testing.
fn remove_all(paths: &[std::path::PathBuf]) -> usize {
    paths
        .iter()
        .filter(|p| std::fs::remove_file(p).is_ok())
        .count()
}

/// Removes stale `vlx-uploads` images older than `max_age` at startup, recovering files left by crashes or forced
/// termination. Age filtering preserves images recently created by another instance. Missing directories and individual
/// deletion failures are harmless.
pub fn sweep_stale_pasted_images(max_age: std::time::Duration) -> usize {
    sweep_stale_in(&std::env::temp_dir().join("vlx-uploads"), max_age)
}

/// Deletes files under `dir` older than `max_age`; parameterized for isolated testing.
fn sweep_stale_in(dir: &std::path::Path, max_age: std::time::Duration) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let now = std::time::SystemTime::now();
    let mut removed = 0;
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|age| age > max_age);
        if stale && std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Immediately clears **all** pasted temporary images for the settings action and returns `(count, freed bytes)`.
/// Unlike automatic cleanup, this explicit action ignores process ownership and age, clears registration, and never
/// touches document images stored in sibling `assets/` directories.
pub fn purge_pasted_images() -> (usize, u64) {
    if let Ok(mut reg) = pasted_image_registry().lock() {
        reg.clear();
    }
    let dir = std::env::temp_dir().join("vlx-uploads");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return (0, 0);
    };
    let mut removed = 0;
    let mut freed = 0u64;
    for entry in entries.flatten() {
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if entry.path().is_file() && std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
            freed += size;
        }
    }
    (removed, freed)
}

/// Sanitizes extensions to short alphanumeric text, falling back to png to prevent path injection.
fn sanitize_ext(ext: &str) -> String {
    let safe: String = ext
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(5)
        .collect::<String>()
        .to_ascii_lowercase();
    if safe.is_empty() {
        "png".to_string()
    } else {
        safe
    }
}

/// Saves a markdown image into a sibling **`assets/` directory** and returns `assets/<uuid>.png` for insertion.
///
/// Unlike temporary agent images, document assets persist and move with the document through Git/sharing, using a
/// relative path like Typora.
///
/// `doc_path` must be absolute. Unsaved drafts fall back to temporary storage in the frontend. Markdown references
/// always use forward slashes across platforms.
pub fn save_doc_image(doc_path: &str, bytes: &[u8], ext: &str) -> Result<String, String> {
    let safe_ext = sanitize_ext(ext);
    let assets = doc_assets_dir(doc_path)?;
    std::fs::create_dir_all(&assets).map_err(|e| format!("failed to create assets dir: {e}"))?;
    let name = format!("{}.{}", uuid::Uuid::new_v4(), safe_ext);
    std::fs::write(assets.join(&name), bytes).map_err(|e| format!("failed to write image: {e}"))?;
    Ok(format!("assets/{name}"))
}

/// The effective directory [`save_doc_image`] writes into: `<docPath parent>/assets`. Exposed so
/// callers that enforce access control (the remote data-dir ACL in web dispatch) can gate the path
/// actually written, not just the document path itself.
pub fn doc_assets_dir(doc_path: &str) -> Result<std::path::PathBuf, String> {
    let parent = std::path::Path::new(doc_path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| {
            "document path has no parent directory, cannot locate assets dir".to_string()
        })?;
    Ok(parent.join("assets"))
}

#[cfg(test)]
mod tests {
    use super::save_pasted_image;
    use super::{read_file_base64, read_text_file, stat_file, write_text_file};

    /// Builds a uniquely named temporary test path without creating the file.
    fn tmp_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "vlx-files-test-{}-{}-{}",
            tag,
            std::process::id(),
            uuid::Uuid::new_v4()
        ))
    }

    #[cfg(unix)]
    #[test]
    fn list_dir_follows_symlinks_when_classifying_directories() {
        use std::os::unix::fs::symlink;

        let base = tmp_path("list-symlinks");
        let target_dir = base.join("target-dir");
        let target_file = base.join("target-file");
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(&target_file, "x").unwrap();
        symlink(&target_dir, base.join("link-dir")).unwrap();
        symlink(&target_file, base.join("link-file")).unwrap();
        symlink(base.join("missing"), base.join("broken-link")).unwrap();

        let entries = super::list_dir(&base.to_string_lossy()).expect("the test directory should be listable");
        let is_dir = |name: &str| {
            entries
                .iter()
                .find(|entry| entry.name == name)
                .unwrap_or_else(|| panic!("missing entry: {name}"))
                .is_dir
        };

        assert!(is_dir("link-dir"), "a symlink to a directory should be reported as a directory");
        assert!(!is_dir("link-file"), "a symlink to a file should still be reported as a file");
        assert!(!is_dir("broken-link"), "a broken symlink should not be reported as a directory");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn read_write_round_trip() {
        let path = tmp_path("roundtrip");
        let p = path.to_string_lossy();
        std::fs::write(&path, "# Heading\nbody αβ\n").unwrap();

        let read = read_text_file(&p).expect("reading should succeed");
        assert_eq!(read.content, "# Heading\nbody αβ\n");
        assert!(read.mtime_ms > 0);

        // The correct baseline writes successfully and returns a new mtime.
        let out = write_text_file(&p, "edited content", Some(read.mtime_ms)).expect("writing should succeed");
        assert!(!out.conflict);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "edited content");
        let read2 = read_text_file(&p).unwrap();
        assert_eq!(
            read2.mtime_ms, out.mtime_ms,
            "the mtime returned by the write is the new baseline on disk"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_text_file_rejects_non_utf8_and_binary() {
        // Invalid UTF-8 errors rather than allowing a lossy, corrupting round trip.
        let path = tmp_path("nonutf8");
        std::fs::write(&path, [0xFFu8, 0xFE, 0x41]).unwrap();
        let err = read_text_file(&path.to_string_lossy()).unwrap_err();
        assert!(err.contains("UTF-8"), "got: {err}");
        let _ = std::fs::remove_file(&path);

        // NUL classifies the file as binary and rejects editing.
        let path = tmp_path("binary");
        std::fs::write(&path, b"abc\0def").unwrap();
        let err = read_text_file(&path.to_string_lossy()).unwrap_err();
        assert!(err.contains("Binary"), "got: {err}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_text_file_truncates_oversize() {
        let path = tmp_path("oversize");
        let p = path.to_string_lossy().to_string();
        // Oversized ASCII becomes truncated read-only content, not an error or binary classification.
        let big = vec![b'a'; (super::EDIT_MAX + 1024) as usize];
        std::fs::write(&path, &big).unwrap();
        let f = read_text_file(&p).expect("exceeding the limit should truncate rather than error");
        assert!(f.truncated, "it should be marked as truncated");
        assert_eq!(f.full_size, super::EDIT_MAX + 1024, "the real file size should be returned");
        assert_eq!(
            f.content.len() as u64,
            super::EDIT_MAX,
            "the content should be cut exactly at the limit"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_text_file_truncates_at_utf8_boundary() {
        let path = tmp_path("oversize-utf8");
        let p = path.to_string_lossy().to_string();
        // Fill with a three-byte character so EDIT_MAX lands inside one and verify truncation backs up to a valid boundary.
        let ni = "你"; // 0xE4 0xBD 0xA0
        let count = (super::EDIT_MAX / 3 + 16) as usize;
        let big = ni.repeat(count);
        std::fs::write(&path, big.as_bytes()).unwrap();
        let f = read_text_file(&p).expect("exceeding the limit should truncate");
        assert!(f.truncated, "it should be marked as truncated");
        // Content must remain valid UTF-8, with complete characters and a byte count within the limit.
        assert_eq!(
            f.content.len() % 3,
            0,
            "it should truncate on a valid UTF-8 boundary, keeping a whole number of characters"
        );
        assert!(f.content.len() as u64 <= super::EDIT_MAX, "it must not exceed the limit");
        assert!(
            f.content.chars().all(|c| c == '你'),
            "every character in the content should be complete"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn write_text_file_detects_mtime_conflict() {
        let path = tmp_path("conflict");
        let p = path.to_string_lossy().to_string();
        std::fs::write(&path, "original content").unwrap();
        let base = read_text_file(&p).unwrap().mtime_ms;

        // Simulate an external change by rewriting and moving mtime two seconds to avoid same-millisecond ambiguity.
        let earlier = std::time::SystemTime::now() - std::time::Duration::from_secs(2);
        let f = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        f.set_modified(earlier).unwrap();
        drop(f);

        let out = write_text_file(&p, "my edit", Some(base)).expect("it should report a conflict rather than error");
        assert!(out.conflict, "a mismatched mtime counts as a conflict");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "original content",
            "nothing may be written while there is a conflict"
        );

        // No baseline represents confirmed overwrite and forces the write.
        let out = write_text_file(&p, "forced overwrite", None).unwrap();
        assert!(!out.conflict);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "forced overwrite");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn stat_file_matches_read_mtime() {
        let path = tmp_path("stat");
        let p = path.to_string_lossy();
        std::fs::write(&path, "hello").unwrap();
        let read = read_text_file(&p).unwrap();
        let stat = stat_file(&p).unwrap();
        assert_eq!(stat.mtime_ms, read.mtime_ms);
        assert_eq!(stat.size, 5);
        // Directories error.
        assert!(stat_file(&std::env::temp_dir().to_string_lossy()).is_err());
        let _ = std::fs::remove_file(&path);
    }

    /// Standard padded base64 decoder for verifying independent chunk round trips.
    fn base64_decode(s: &str) -> Vec<u8> {
        const T: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        let chars: Vec<u8> = s.bytes().filter(|&b| b != b'=').collect();
        for quad in chars.chunks(4) {
            let vals: Vec<u32> = quad
                .iter()
                .map(|&b| T.find(b as char).expect("invalid base64 character") as u32)
                .collect();
            let mut n = 0u32;
            for (i, v) in vals.iter().enumerate() {
                n |= v << (18 - 6 * i);
            }
            out.push((n >> 16) as u8);
            if vals.len() > 2 {
                out.push((n >> 8) as u8);
            }
            if vals.len() > 3 {
                out.push(n as u8);
            }
        }
        out
    }

    #[test]
    fn read_file_base64_chunks_reassemble() {
        // Use nontrivial 5000-byte content and a deliberately non-multiple-of-three chunk size to prove independent encoding.
        let data: Vec<u8> = (0..5000u32).map(|i| (i * 7 % 251) as u8).collect();
        let path = tmp_path("chunks");
        let p = path.to_string_lossy().to_string();
        std::fs::write(&path, &data).unwrap();
        let expect_mtime = stat_file(&p).unwrap().mtime_ms;

        let mut got = Vec::new();
        let mut offset = 0u64;
        let chunk_len = 2048u64;
        loop {
            let c = read_file_base64(&p, offset, chunk_len).expect("reading should succeed");
            assert_eq!(c.size, 5000, "every chunk should carry the total file size");
            assert_eq!(c.mtime_ms, expect_mtime, "every chunk should carry the current mtime");
            let bytes = base64_decode(&c.base64);
            assert!(bytes.len() as u64 <= chunk_len);
            got.extend_from_slice(&bytes);
            offset += bytes.len() as u64;
            if offset >= c.size {
                break;
            }
        }
        assert_eq!(got, data, "decoding the chunks and joining them should reproduce the original bytes");
        // The final 904-byte chunk is returned normally below max_len.
        let last = read_file_base64(&p, 4096, chunk_len).unwrap();
        assert_eq!(base64_decode(&last.base64).len(), 904);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_file_base64_out_of_range_returns_empty() {
        let path = tmp_path("oob");
        let p = path.to_string_lossy();
        std::fs::write(&path, b"hello").unwrap();
        // An offset at or beyond size returns an empty chunk while preserving size/mtime.
        let c = read_file_base64(&p, 5, 1024).unwrap();
        assert_eq!(c.base64, "");
        assert_eq!(c.size, 5);
        assert!(c.mtime_ms > 0);
        let c = read_file_base64(&p, 99999, 1024).unwrap();
        assert_eq!(c.base64, "");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_file_base64_rejects_oversize_and_dir() {
        // The viewer assembles the whole file in memory, so anything past the ceiling is refused. Sparse
        // set_len keeps the test cheap.
        let path = tmp_path("bin-oversize");
        let f = std::fs::File::create(&path).unwrap();
        f.set_len(super::BLOB_MAX + 1).unwrap();
        drop(f);
        let err = read_file_base64(&path.to_string_lossy(), 0, 1024).unwrap_err();
        assert!(err.contains("too large"), "got: {err}");
        let _ = std::fs::remove_file(&path);

        // A single call never returns more than the per-message ceiling, whatever length is asked for.
        let path = tmp_path("bin-chunkcap");
        let f = std::fs::File::create(&path).unwrap();
        f.set_len(super::CHUNK_MAX + 1024).unwrap();
        drop(f);
        let c = read_file_base64(&path.to_string_lossy(), 0, u64::MAX).unwrap();
        assert_eq!(c.size, super::CHUNK_MAX + 1024, "total size is reported in full");
        assert_eq!(c.base64.len() as u64, super::CHUNK_MAX.div_ceil(3) * 4);
        let _ = std::fs::remove_file(&path);

        // Reject directories.
        let err = read_file_base64(&std::env::temp_dir().to_string_lossy(), 0, 1024).unwrap_err();
        assert!(err.contains("Not a regular file"), "got: {err}");
        // Missing paths error.
        assert!(read_file_base64(&tmp_path("missing").to_string_lossy(), 0, 1).is_err());
    }

    #[test]
    fn write_file_chunk_appends_sequentially_and_rejects_gaps() {
        use super::write_file_chunk;

        let path = tmp_path("upload-chunks");
        let p = path.to_string_lossy().to_string();

        // Sequential chunks assemble the original bytes, each call returning the new length.
        assert_eq!(write_file_chunk(&p, 0, b"hello ").unwrap(), 6);
        assert_eq!(write_file_chunk(&p, 6, b"world").unwrap(), 11);
        assert_eq!(std::fs::read(&path).unwrap(), b"hello world");

        // An offset that does not match the current length is refused rather than seeking past a hole or
        // overwriting the middle of another upload.
        let err = write_file_chunk(&p, 99, b"x").unwrap_err();
        assert!(err.contains("offset mismatch"), "got: {err}");
        let err = write_file_chunk(&p, 3, b"x").unwrap_err();
        assert!(err.contains("offset mismatch"), "got: {err}");
        assert_eq!(std::fs::read(&path).unwrap(), b"hello world", "a refused chunk writes nothing");

        // Offset 0 truncates, so a restarted upload never inherits the previous attempt's tail.
        assert_eq!(write_file_chunk(&p, 0, b"new").unwrap(), 3);
        assert_eq!(std::fs::read(&path).unwrap(), b"new");

        // Chunks larger than the per-call ceiling are refused before any IO.
        let err = write_file_chunk(&p, 0, &vec![0u8; (super::CHUNK_MAX + 1) as usize]).unwrap_err();
        assert!(err.contains("Chunk too large"), "got: {err}");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn create_file_creates_and_rejects_existing() {
        use super::create_file;
        let dir = std::env::temp_dir().join(format!("vlx-create-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let p = dir.join("a.txt");
        let ps = p.to_string_lossy().to_string();
        // First creation succeeds with an empty file.
        create_file(&ps).expect("creation should succeed");
        assert!(p.is_file());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "");
        // Recreating the same name errors without overwriting existing content.
        std::fs::write(&p, "keep").unwrap();
        let err = create_file(&ps).unwrap_err();
        assert!(err.contains("already exists"), "got: {err}");
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "keep", "an existing name must not be overwritten");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_into_dir_keeps_both_copies_folders_and_skips_own_folder() {
        use super::copy_into_dir;
        let base = std::env::temp_dir().join(format!("vlx-copy-into-{}", uuid::Uuid::new_v4()));
        let src = base.join("src");
        let dest = base.join("dest");
        std::fs::create_dir_all(src.join("folder/inner")).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(src.join("a.txt"), "new").unwrap();
        std::fs::write(src.join("folder/inner/b.txt"), "deep").unwrap();
        std::fs::write(dest.join("a.txt"), "old").unwrap();
        std::fs::write(dest.join("here.txt"), "here").unwrap();
        let s = |p: std::path::PathBuf| p.to_string_lossy().to_string();

        let created = copy_into_dir(
            &[s(src.join("a.txt")), s(src.join("folder")), s(dest.join("here.txt"))],
            &s(dest.clone()),
        )
        .expect("copy should succeed");
        // A taken name gets a suffix and the existing file is untouched.
        assert_eq!(std::fs::read_to_string(dest.join("a.txt")).unwrap(), "old");
        assert_eq!(std::fs::read_to_string(dest.join("a (2).txt")).unwrap(), "new");
        // Folders copy recursively.
        assert_eq!(std::fs::read_to_string(dest.join("folder/inner/b.txt")).unwrap(), "deep");
        // A file already in the destination folder is skipped, not duplicated.
        assert!(!dest.join("here (2).txt").exists());
        assert_eq!(created.len(), 2, "got: {created:?}");

        // A folder cannot be copied into its own subtree.
        let err = copy_into_dir(&[s(src.clone())], &s(src.join("folder"))).unwrap_err();
        assert!(err.contains("into itself"), "got: {err}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn create_dir_creates_and_rejects_existing() {
        use super::create_dir;
        let base = std::env::temp_dir().join(format!("vlx-mkdir-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&base).unwrap();
        let sub = base.join("sub");
        let subs = sub.to_string_lossy().to_string();
        create_dir(&subs).expect("creation should succeed");
        assert!(sub.is_dir());
        let err = create_dir(&subs).unwrap_err();
        assert!(err.contains("already exists"), "got: {err}");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn rename_path_moves_and_rejects_existing_dest() {
        use super::rename_path;
        let base = std::env::temp_dir().join(format!("vlx-rename-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&base).unwrap();
        let a = base.join("a.txt");
        let b = base.join("b.txt");
        std::fs::write(&a, "hello").unwrap();
        // Rename succeeds and carries content.
        rename_path(&a.to_string_lossy(), &b.to_string_lossy()).expect("renaming should succeed");
        assert!(!a.exists() && b.is_file());
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "hello");
        // An existing destination errors without overwrite.
        let c = base.join("c.txt");
        std::fs::write(&c, "ccc").unwrap();
        let err = rename_path(&b.to_string_lossy(), &c.to_string_lossy()).unwrap_err();
        assert!(err.contains("already exists"), "got: {err}");
        assert_eq!(
            std::fs::read_to_string(&c).unwrap(),
            "ccc",
            "the destination must not be overwritten"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn delete_path_removes_file_and_dir() {
        use super::delete_path;
        let base = std::env::temp_dir().join(format!("vlx-del-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&base).unwrap();
        // Delete a file.
        let f = base.join("f.txt");
        std::fs::write(&f, "x").unwrap();
        delete_path(&f.to_string_lossy()).expect("the file should be deleted");
        assert!(!f.exists());
        // Recursively delete a nonempty directory.
        let d = base.join("d");
        std::fs::create_dir(&d).unwrap();
        std::fs::write(d.join("inner.txt"), "y").unwrap();
        delete_path(&d.to_string_lossy()).expect("the directory should be deleted recursively");
        assert!(!d.exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn save_pasted_image_writes_file_with_clean_path() {
        let data = b"\x89PNG\r\n\x1a\n test bytes";
        let path = save_pasted_image(data, "PNG").expect("saving should succeed");
        // Path has no spaces that a shell would split when inserted into the terminal.
        assert!(!path.contains(' '), "the upload path should contain no spaces: {path}");
        // File exists on disk with identical content.
        let got = std::fs::read(&path).expect("should be readable again");
        assert_eq!(got, data);
        // Extension is sanitized to lowercase.
        assert!(path.ends_with(".png"), "the extension should be normalised to lowercase png: {path}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_pasted_image_rejects_bad_ext() {
        // Invalid extensions containing separators/dots fall back to png to prevent injection.
        let path = save_pasted_image(b"x", "../evil").expect("saving should succeed");
        assert!(path.ends_with(".evil") || path.ends_with(".png"));
        assert!(!path.contains(".."), ".. should not survive: {path}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_pasted_image_registers_path_for_cleanup() {
        use super::pasted_image_registry;
        // Saved paths enter this process's exit-cleanup registry.
        let path = save_pasted_image(b"reg", "png").expect("saving should succeed");
        let registered = pasted_image_registry()
            .lock()
            .unwrap()
            .iter()
            .any(|p| p.to_string_lossy() == path);
        assert!(registered, "a pasted image written to disk should be registered for cleanup: {path}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn remove_all_deletes_existing_ignores_missing() {
        use super::remove_all;
        let a = tmp_path("rm-a");
        let b = tmp_path("rm-b");
        std::fs::write(&a, "x").unwrap();
        std::fs::write(&b, "y").unwrap();
        let missing = tmp_path("rm-missing"); // Never created.
                                              // Delete two real files and one missing path; count two and ignore the missing one.
        let removed = remove_all(&[a.clone(), b.clone(), missing]);
        assert_eq!(removed, 2);
        assert!(!a.exists() && !b.exists());
    }

    #[test]
    fn sweep_stale_in_removes_only_old_files() {
        use super::sweep_stale_in;
        let dir = std::env::temp_dir().join(format!("vlx-sweep-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let old = dir.join("old.png");
        let fresh = dir.join("fresh.png");
        std::fs::write(&old, "old").unwrap();
        std::fs::write(&fresh, "fresh").unwrap();
        // Move old's mtime back two hours while fresh remains newly written.
        let two_hours_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 3600);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(two_hours_ago)
            .unwrap();

        // A one-hour threshold removes only old and preserves fresh.
        let removed = sweep_stale_in(&dir, std::time::Duration::from_secs(3600));
        assert_eq!(removed, 1, "exactly one stale file should be removed");
        assert!(!old.exists(), "the stale file should be gone");
        assert!(fresh.exists(), "the fresh file should remain");

        // A missing directory returns zero without error.
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(sweep_stale_in(&dir, std::time::Duration::from_secs(0)), 0);
    }

    #[test]
    fn save_doc_image_writes_to_sibling_assets_dir() {
        use super::save_doc_image;
        let base = std::env::temp_dir().join(format!("vlx-docimg-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let doc = base.join("note.md");
        std::fs::write(&doc, "# hi").unwrap();
        let data = b"\x89PNG\r\n\x1a\n test bytes";

        let rel = save_doc_image(&doc.to_string_lossy(), data, "PNG").expect("saving should succeed");
        // Return an assets/-prefixed relative path with forward slashes and lowercase extension.
        assert!(rel.starts_with("assets/"), "it should be a relative path under assets/: {rel}");
        assert!(rel.ends_with(".png"), "the extension should be normalised to lowercase png: {rel}");
        assert!(!rel.contains(".."), "it should contain no ..: {rel}");
        // Persist identical content in the document's sibling assets directory.
        let got = std::fs::read(base.join(&rel)).expect("should be readable again");
        assert_eq!(got, data);

        let _ = std::fs::remove_dir_all(&base);
    }
}
