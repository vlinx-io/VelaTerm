//! What the current branch changed relative to a base branch: the files committed since the two
//! histories split (`merge-base..HEAD`), plus how far each side has moved. This is the view a
//! worktree session needs before merging or opening a pull request, which the working-tree groups
//! cannot give once the work is committed.
//!
//! The base defaults to the remote's default branch (`origin/HEAD`), then the usual names. On that
//! base branch itself there is nothing to compare unless the user picks another base explicitly.

use super::*;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchCompare {
    /// Ref compared against, as the user would type it (`origin/main`).
    pub base: String,
    /// Where the histories split; the left side of every file diff.
    pub merge_base: String,
    /// HEAD at the time of the comparison; the right side of every file diff.
    pub head: String,
    /// Commits on this branch since the split.
    pub ahead: u32,
    /// Commits on the base since the split.
    pub behind: u32,
    pub files: Vec<ChangedFile>,
    /// Branches that can be picked as the base: local first, then remote-tracking.
    pub candidates: Vec<String>,
    /// Suggested pull request title derived from the single commit subject or branch name.
    pub suggested_title: String,
    /// Suggested pull request description derived from the commit body or commit list.
    pub suggested_body: String,
}

fn strip_vlx_slug_suffix(name: &str) -> &str {
    let Some((stem, tail)) = name.rsplit_once('-') else { return name };
    let hexish = (tail.len() == 6 && tail.chars().all(|c| c.is_ascii_hexdigit()))
        || (tail.len() >= 8 && tail.chars().all(|c| c.is_ascii_hexdigit()));
    if hexish && !stem.is_empty() { stem } else { name }
}

/// Turn a branch name like `vlx/fix-auth-a1b2c3` or `feat/user_login` into a readable PR title.
pub(crate) fn humanize_branch(branch: &str) -> String {
    let raw = branch.trim();
    let is_vlx = raw.starts_with("vlx/");
    let stripped = [
        "vlx/",
        "feat/",
        "feature/",
        "fix/",
        "bugfix/",
        "hotfix/",
        "chore/",
        "refactor/",
        "docs/",
        "perf/",
        "test/",
        "ci/",
    ]
    .iter()
    .find_map(|prefix| raw.strip_prefix(prefix))
    .unwrap_or(raw);
    let cleaned = if is_vlx { strip_vlx_slug_suffix(stripped) } else { stripped };
    let words: Vec<&str> = cleaned
        .split(['-', '_', '/'])
        .filter(|part| !part.is_empty())
        .collect();
    if words.is_empty() {
        return raw.to_string();
    }
    let joined = words.join(" ");
    let mut chars = joined.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
        None => raw.to_string(),
    }
}

fn suggest_pr_text(root: &str, merge_base: &str, head: &str, ahead: u32, branch: Option<&str>) -> (String, String) {
    if ahead == 0 {
        return (String::new(), String::new());
    }
    if ahead == 1 {
        if let Some(out) = run_git(root, &["log", "-1", "--format=%s%x00%b", head]) {
            let (subj, body) = out.split_once('\0').unwrap_or((&out, ""));
            let title = subj.trim().to_string();
            if !title.is_empty() {
                return (title, body.trim().to_string());
            }
        }
    }
    let title = branch.map(humanize_branch).unwrap_or_default();
    let range = format!("{merge_base}..{head}");
    let body = run_git(root, &["log", "--reverse", "--format=%s", &range])
        .map(|s| {
            s.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(|l| format!("- {l}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    (title, body)
}

/// A ref the user may name. Leading dashes would read as options; the rest is up to git.
fn check_ref(name: &str) -> Result<(), String> {
    let n = name.trim();
    if n.is_empty() || n.starts_with('-') || n.contains("..") || n.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(format!("Not a branch name: {name}"));
    }
    Ok(())
}

fn commit_of(root: &str, name: &str) -> Option<String> {
    run_git(root, &["rev-parse", "--verify", "--quiet", &format!("{name}^{{commit}}")]).filter(|s| !s.is_empty())
}

/// Local and remote-tracking branch names, without the current branch and symbolic `*/HEAD`.
fn candidates(root: &str, current: Option<&str>) -> Vec<String> {
    let out = run_git(root, &["for-each-ref", "--format=%(refname:short)%00%(symref)", "refs/heads", "refs/remotes"])
        .unwrap_or_default();
    out.lines()
        .filter_map(|line| {
            let (name, symref) = line.split_once('\0').unwrap_or((line, ""));
            (symref.is_empty() && Some(name) != current && !name.is_empty()).then(|| name.to_string())
        })
        .collect()
}

/// The base to use when the user has not chosen one.
fn default_base(root: &str, current: Option<&str>) -> Option<String> {
    let remote_default = run_git(root, &["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"]);
    let mut options: Vec<String> = remote_default.into_iter().collect();
    options.extend(["origin/main", "origin/master", "main", "master"].map(String::from));
    options.into_iter().find(|name| {
        // Comparing main with origin/main is what ahead/behind already says; skip it.
        let short = name.rsplit_once('/').map(|(_, b)| b).unwrap_or(name);
        Some(short) != current && commit_of(root, name).is_some()
    })
}

/// Compare HEAD with `base`, or with the default base. None when there is no commit yet or no
/// base to compare with.
pub fn compare(cwd: &str, base: Option<&str>) -> Result<Option<BranchCompare>, String> {
    require_repo(cwd)?;
    let root = repo_top(cwd);
    let Some(head) = commit_of(&root, "HEAD") else { return Ok(None) };
    let current = run_git(&root, &["symbolic-ref", "--quiet", "--short", "HEAD"]).filter(|s| !s.is_empty());
    let base = match base.map(str::trim).filter(|b| !b.is_empty()) {
        Some(b) => {
            check_ref(b)?;
            b.to_string()
        }
        None => match default_base(&root, current.as_deref()) {
            Some(b) => b,
            None => return Ok(None),
        },
    };
    let base_commit = commit_of(&root, &base).ok_or_else(|| format!("Branch not found: {base}"))?;
    let merge_base = run_git(&root, &["merge-base", &head, &base_commit])
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("This branch shares no history with {base}"))?;
    let count = |range: String| {
        run_git(&root, &["rev-list", "--count", &range]).and_then(|s| s.parse().ok()).unwrap_or(0)
    };
    let ahead = count(format!("{merge_base}..{head}"));
    let behind = count(format!("{merge_base}..{base_commit}"));
    let files = tree_diff_files(&root, Some(&merge_base), &head)?;
    let (suggested_title, suggested_body) =
        suggest_pr_text(&root, &merge_base, &head, ahead, current.as_deref());
    Ok(Some(BranchCompare {
        candidates: candidates(&root, current.as_deref()),
        base,
        merge_base,
        head,
        ahead,
        behind,
        files,
        suggested_title,
        suggested_body,
    }))
}

/// Full name of a branch (`refs/heads/main`, `refs/remotes/origin/main`), for callers that need to
/// know which side of a remote a short name lives on.
pub fn full_ref(cwd: &str, name: &str) -> Result<String, String> {
    check_ref(name)?;
    run_git(cwd, &["rev-parse", "--symbolic-full-name", name])
        .filter(|s| s.starts_with("refs/"))
        .ok_or_else(|| format!("Branch not found: {name}"))
}

/// Files changed from commit `from` to commit `to`.
pub fn range_files(cwd: &str, from: &str, to: &str) -> Result<Vec<ChangedFile>, String> {
    require_repo(cwd)?;
    check_commit_hash(from)?;
    check_commit_hash(to)?;
    tree_diff_files(&repo_top(cwd), Some(from), to)
}

/// Both sides of one file between two commits; a renamed file reads its old path on the left.
pub fn range_file_diff(cwd: &str, from: &str, to: &str, path: &str) -> Result<FileDiff, String> {
    require_repo(cwd)?;
    check_commit_hash(from)?;
    check_commit_hash(to)?;
    check_rel_path(path)?;
    let root = repo_top(cwd);
    let files = tree_diff_files(&root, Some(from), to)?;
    let entry = files.iter().find(|f| f.path == path).ok_or("This file did not change in the selected range")?;
    let original = panel::blob(&root, Some(from), entry.original_path.as_deref().unwrap_or(path))?;
    let modified = panel::blob(&root, Some(to), path)?;
    Ok(panel::make_diff(path, original, modified))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &std::path::Path, args: &[&str]) -> String {
        run_git_checked(dir.to_str().unwrap(), args).unwrap()
    }

    #[test]
    fn compares_a_feature_branch_with_the_default_base() {
        let dir = std::env::temp_dir().join(format!("vlx-git-compare-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let cwd = dir.to_str().unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        git(&dir, &["config", "user.email", "t@example.com"]);
        git(&dir, &["config", "user.name", "T"]);
        git(&dir, &["config", "commit.gpgsign", "false"]);
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        std::fs::write(dir.join("old.txt"), "keep this text\nsecond line\n").unwrap();
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "base"]);
        // On main itself there is no default base to compare with.
        assert!(compare(cwd, None).unwrap().is_none());

        git(&dir, &["checkout", "-qb", "feature"]);
        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        git(&dir, &["mv", "old.txt", "new.txt"]);
        git(&dir, &["commit", "-qam", "feature work"]);
        git(&dir, &["checkout", "-q", "main"]);
        std::fs::write(dir.join("main.txt"), "m\n").unwrap();
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "main moves on"]);
        git(&dir, &["checkout", "-q", "feature"]);
        // Uncommitted edits are not part of the branch comparison.
        std::fs::write(dir.join("a.txt"), "dirty\n").unwrap();

        let c = compare(cwd, None).unwrap().unwrap();
        assert_eq!(c.base, "main");
        assert_eq!((c.ahead, c.behind), (1, 1));
        assert_eq!(c.suggested_title, "feature work");
        assert_eq!(humanize_branch("vlx/fix-login-a1b2c3"), "Fix login");
        let paths: Vec<_> = c.files.iter().map(|f| (f.path.as_str(), f.status.as_str())).collect();
        assert_eq!(paths, [("a.txt", "modified"), ("new.txt", "renamed")]);
        assert!(c.candidates.contains(&"main".to_string()));
        assert!(!c.candidates.contains(&"feature".to_string()));

        let diff = range_file_diff(cwd, &c.merge_base, &c.head, "a.txt").unwrap();
        assert_eq!((diff.original.as_str(), diff.modified.as_str()), ("one\n", "one\ntwo\n"));
        let renamed = range_file_diff(cwd, &c.merge_base, &c.head, "new.txt").unwrap();
        assert_eq!(renamed.original, renamed.modified);
        assert_eq!(range_files(cwd, &c.merge_base, &c.head).unwrap().len(), 2);

        // An explicit base, and names that could be read as options or ranges, are checked.
        assert_eq!(compare(cwd, Some("main")).unwrap().unwrap().ahead, 1);
        assert!(compare(cwd, Some("--output=x")).is_err());
        assert!(compare(cwd, Some("main..feature")).is_err());
        assert!(compare(cwd, Some("missing")).unwrap_err().contains("not found"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
