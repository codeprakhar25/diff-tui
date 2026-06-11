//! Thin wrapper over the `git` CLI. We shell out instead of linking libgit2 to
//! keep the dependency surface small.

use anyhow::{bail, Context, Result};
use std::process::Command;

/// Git's well-known empty-tree object (sha1 repos). Used as the "parent" of a
/// root commit so it diffs cleanly.
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// What the user pointed us at on the command line.
#[derive(Debug, Clone)]
pub enum Mode {
    /// Single working-tree file vs HEAD.
    File(String),
    /// Every uncommitted change vs HEAD.
    All,
    /// Everything a commit changed (commit vs its first parent).
    Commit(String),
}

/// One file's before/after, ready to diff.
#[derive(Debug)]
pub struct FileDiff {
    pub path: String,
    pub status: char,
    /// `None` when the file did not exist on that side (added / deleted).
    pub old: Option<String>,
    pub new: Option<String>,
}

/// Run git, return stdout as String. `ok_fail` = treat non-zero exit as a
/// recoverable "no output" rather than an error (e.g. `git show` on a path that
/// doesn't exist at that rev).
fn git(args: &[&str], ok_fail: bool) -> Result<Option<String>> {
    let out = Command::new("git")
        .args(args)
        .output()
        .with_context(|| format!("failed to spawn git {args:?}"))?;
    if !out.status.success() {
        if ok_fail {
            return Ok(None);
        }
        bail!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned()))
}

/// Resolve the CLI arg into a Mode.
pub fn resolve_mode(arg: &str) -> Result<Mode> {
    if arg == "all" {
        return Ok(Mode::All);
    }
    // File wins over rev on name collision: check disk + tracked first.
    let on_disk = std::path::Path::new(arg).is_file();
    let tracked = git(&["ls-files", "--error-unmatch", arg], true)?.is_some();
    if on_disk || tracked {
        return Ok(Mode::File(arg.to_string()));
    }
    // Otherwise try to read it as a commit-ish.
    if git(&["rev-parse", "--verify", &format!("{arg}^{{commit}}")], true)?.is_some() {
        return Ok(Mode::Commit(arg.to_string()));
    }
    bail!("'{arg}' is not a tracked file, an existing path, 'all', or a commit");
}

/// Contents of `path` at `rev` (e.g. "HEAD", "abc123^"). None if absent there.
fn show(rev: &str, path: &str) -> Result<Option<String>> {
    git(&["show", &format!("{rev}:{path}")], true)
}

/// Working-tree contents of a file. None if it's gone (deleted).
fn read_working(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Parse `git diff --name-status` output into (status, path) pairs.
fn parse_name_status(raw: &str) -> Vec<(char, String)> {
    raw.lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let status = parts.next()?.chars().next()?;
            // Renames/copies emit `R100<TAB>old<TAB>new`; take the new path.
            let path = parts.last()?.to_string();
            Some((status, path))
        })
        .collect()
}

/// Build the list of FileDiffs for a Mode.
pub fn collect(mode: &Mode) -> Result<Vec<FileDiff>> {
    match mode {
        Mode::File(path) => {
            let status = match (show("HEAD", path)?.is_some(), read_working(path).is_some()) {
                (true, true) => 'M',
                (false, true) => 'A',
                (true, false) => 'D',
                (false, false) => bail!("no content for '{path}' at HEAD or on disk"),
            };
            Ok(vec![FileDiff {
                old: show("HEAD", path)?,
                new: read_working(path),
                path: path.clone(),
                status,
            }])
        }
        Mode::All => {
            let raw = git(&["diff", "--name-status", "HEAD"], false)?.unwrap_or_default();
            let mut files: Vec<FileDiff> = parse_name_status(&raw)
                .into_iter()
                .map(|(status, path)| {
                    Ok(FileDiff {
                        old: show("HEAD", &path)?,
                        new: read_working(&path),
                        path,
                        status,
                    })
                })
                .collect::<Result<_>>()?;
            // Untracked files: `git diff` ignores them, but they're real changes
            // while coding. List them as additions.
            let others = git(&["ls-files", "--others", "--exclude-standard"], false)?
                .unwrap_or_default();
            for path in others.lines().filter(|l| !l.is_empty()) {
                files.push(FileDiff {
                    old: None,
                    new: read_working(path),
                    path: path.to_string(),
                    status: 'A',
                });
            }
            Ok(files)
        }
        Mode::Commit(id) => {
            // A root commit has no parent; diff against the empty tree so its
            // files show up as additions instead of erroring.
            let has_parent = git(&["rev-parse", "--verify", &format!("{id}^")], true)?.is_some();
            let parent = if has_parent {
                format!("{id}^")
            } else {
                EMPTY_TREE.to_string()
            };
            let raw = git(&["diff", "--name-status", &parent, id], false)?.unwrap_or_default();
            parse_name_status(&raw)
                .into_iter()
                .map(|(status, path)| {
                    Ok(FileDiff {
                        old: if has_parent { show(&parent, &path)? } else { None },
                        new: show(id, &path)?,
                        path,
                        status,
                    })
                })
                .collect()
        }
    }
}
