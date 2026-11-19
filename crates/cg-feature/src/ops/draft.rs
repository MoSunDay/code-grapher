//! Draft path: mirror a source file tree onto the `mr/<name>` branch.
//!
//! Drafts are disposable — the branch is a pure function of the source files.
//! On refresh the branch first absorbs `main` (reset to main if that merge
//! would conflict), then the source tree is mirrored **inside the MR's own
//! namespace `docs/<name>/`** (extraneous files there are deleted; anything
//! outside — other MRs' docs merged into main, `_changelog/` — is left
//! untouched) and committed when anything changed.

use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::git::{self, MAIN};
use crate::lock;
use crate::model::{mr_branch, validate_feature_name};
use crate::paths::{
    canonical_root, checked_write, ensure_dir_checked, repo_dir, validate_rel_path,
    validate_repo_id,
};

/// Docs namespace root inside a document repository.
pub const DOCS_DIR: &str = "docs";

/// Namespace of one MR's files: `docs/<name>/`.
pub fn docs_prefix(name: &str) -> String {
    format!("{DOCS_DIR}/{name}/")
}

/// One file in a draft source tree; content must be UTF-8 text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftFile {
    pub path: String,
    pub content: String,
}

/// Result of a draft refresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftOutcome {
    /// Branch content already matched the source; nothing committed.
    Noop,
    /// Branch updated; the new branch tip.
    Updated { commit: String },
}

/// Mirror `files` onto `mr/<name>` inside `<root>/<repo_id>`.
pub fn draft(
    root: &Path,
    repo_id: &str,
    name: &str,
    files: &[DraftFile],
    expected_name: Option<&str>,
) -> Result<DraftOutcome> {
    validate_repo_id(repo_id)?;
    validate_feature_name(name)?;
    if let Some(expected) = expected_name {
        if expected != name {
            bail!("feature name mismatch: expected {expected:?}, got {name:?}");
        }
    }
    let namespace = docs_prefix(name);
    let mut seen = std::collections::HashSet::new();
    for file in files {
        validate_rel_path(&file.path)?;
        if !file.path.starts_with(&namespace) {
            bail!(
                "draft path {:?} must live under {namespace:?} (the MR's own namespace)",
                file.path
            );
        }
        if !seen.insert(file.path.clone()) {
            bail!("duplicate draft path {:?}", file.path);
        }
    }
    let root = canonical_root(root)?;
    let _guard = lock::acquire(&root, repo_id)?;
    let repo = repo_dir(&root, repo_id)?;
    git::ensure_repo(&repo)?;
    let branch = mr_branch(name);
    git::ensure_branch(&repo, &branch, MAIN)?;
    git::checkout(&repo, &branch)?;
    // Absorb main; on conflict reset (drafts are rebuildable from source).
    if git::merge(&repo, MAIN, false, "", "").is_err() {
        git::reset_hard(&repo, MAIN)?;
    }
    mirror_files(&repo, &namespace, files)?;
    git::stage_all(&repo)?;
    if git::staged_is_empty(&repo)? {
        return Ok(DraftOutcome::Noop);
    }
    let commit = git::commit_staged(
        &repo,
        &format!("draft: refresh MR {name:?} ({} files)", files.len()),
        "",
    )?;
    Ok(DraftOutcome::Updated { commit })
}

/// Mirror `files` into the MR namespace; stale files inside the namespace
/// are deleted, everything outside it is preserved.
fn mirror_files(repo: &Path, namespace: &str, files: &[DraftFile]) -> Result<()> {
    let wanted: std::collections::HashSet<&str> =
        files.iter().map(|f| f.path.as_str()).collect();
    for (path, _) in &git::ls_tree(repo, "HEAD")? {
        if path.starts_with(namespace) && !wanted.contains(path.as_str()) {
            let abs = repo.join(path);
            std::fs::remove_file(&abs)
                .with_context(|| format!("delete stale draft file {}", abs.display()))?;
        }
    }
    for file in files {
        let rel = Path::new(&file.path);
        if let Some(parent) = rel.parent() {
            if !parent.as_os_str().is_empty() {
                ensure_dir_checked(repo, parent)?;
            }
        }
        checked_write(repo, rel, file.content.as_bytes())?;
    }
    prune_empty_dirs(repo, Path::new(namespace))?;
    Ok(())
}

/// Remove now-empty directories left behind by deletions (bottom-up), never
/// touching the namespace root itself.
fn prune_empty_dirs(repo: &Path, namespace: &Path) -> Result<()> {
    let mut stack = vec![repo.to_path_buf()];
    let mut dirs = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).with_context(|| format!("scan {}", dir.display()))? {
            let entry = entry.with_context(|| format!("scan {}", dir.display()))?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            }
        }
        if dir != repo && dir != repo.join(namespace) {
            dirs.push(dir);
        }
    }
    // Deepest first so emptied parents can also go away.
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for dir in dirs {
        let _ = std::fs::remove_dir(&dir);
    }
    Ok(())
}
