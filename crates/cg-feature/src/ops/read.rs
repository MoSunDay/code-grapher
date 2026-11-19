//! Lock-free query path: MR list, file tree, blob, diff, commits, changelog.
//!
//! Reads resolve through git refs (`ls-tree` / `show` / `log`), never through
//! the worktree, so they observe complete commits only and never need the
//! write lock.

use anyhow::{bail, Result};
use std::collections::HashSet;
use std::path::Path;

use crate::git::{self, CommitInfo, MAIN};
use crate::model::{mr_branch, mr_name_from_branch, render_hint, RenderHint};
use crate::paths::{
    canonical_root, repo_dir, validate_blob_path, validate_repo_id, CHANGELOG_DIR,
};

/// Blob responses larger than this are truncated (front-end also truncates).
pub const MAX_BLOB_BYTES: usize = 512 * 1024;

/// MR lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MrState {
    /// Branch tip not yet in `main`.
    Open,
    /// Branch tip reachable from `main`.
    Merged,
}

/// One MR (branch) row for the MR list / table.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MrSummary {
    pub name: String,
    pub state: MrState,
    /// Branch tip commit hash.
    pub tip: String,
    /// Commits in branch not in main.
    pub ahead: usize,
    /// Commits in main not in branch.
    pub behind: usize,
    /// Tip commit author.
    pub author: String,
    /// Tip commit unix timestamp.
    pub updated_at: i64,
    /// Approver (merge commit author) once merged.
    pub reviewer: Option<String>,
    /// Merge commit hash once merged.
    pub merge_commit: Option<String>,
    /// Merge unix timestamp once merged.
    pub merged_at: Option<i64>,
    /// `_changelog/...` entry file path once merged.
    pub changelog: Option<String>,
}

/// One file in a tree listing.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TreeFile {
    pub path: String,
    pub size: u64,
    /// Draft when the file differs from `main` on this MR branch.
    pub state: FileBadge,
    pub render: RenderHint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileBadge {
    Draft,
    Stable,
}

/// Blob content with size cap.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Blob {
    pub path: String,
    pub size: u64,
    pub truncated: bool,
    pub content: String,
    pub render: RenderHint,
}

/// Changelog entry, newest first.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ChangelogEntry {
    pub file: String,
    pub markdown: String,
}

fn repo(root: &Path, repo_id: &str) -> Result<std::path::PathBuf> {
    validate_repo_id(repo_id)?;
    let repo = repo_dir(&canonical_root(root)?, repo_id)?;
    if !repo.join(".git").exists() {
        bail!("feature repository for {repo_id} not found");
    }
    Ok(repo)
}

/// List all MRs of one repository partition.
pub fn mrs(root: &Path, repo_id: &str) -> Result<Vec<MrSummary>> {
    let repo = match repo(root, repo_id) {
        Ok(repo) => repo,
        Err(_) => return Ok(Vec::new()),
    };
    let changelog_files = changelog_files(&repo)?;
    let mut out = Vec::new();
    for branch in git::mr_branches(&repo)? {
        let Some(name) = mr_name_from_branch(&branch).map(str::to_string) else {
            continue;
        };
        let Some(tip) = git::tip(&repo, &branch)? else {
            continue;
        };
        let info = git::commit_info(&repo, &branch)?.unwrap_or(CommitInfo {
            hash: tip.clone(),
            parents: Vec::new(),
            author: String::new(),
            timestamp: 0,
            subject: String::new(),
        });
        let (behind, ahead) = git::ahead_behind(&repo, MAIN, &branch)?;
        let merged = git::is_ancestor(&repo, &branch, MAIN)?;
        let merge = if merged {
            merge_details(&repo, &branch, &name, &changelog_files)?
        } else {
            None
        };
        let (reviewer, merge_commit, merged_at, changelog) = match merge {
            Some(d) => (Some(d.reviewer), Some(d.merge_commit), Some(d.merged_at), d.changelog),
            None => (None, None, None, None),
        };
        out.push(MrSummary {
            name,
            state: if merged { MrState::Merged } else { MrState::Open },
            tip,
            ahead,
            behind,
            author: info.author,
            updated_at: info.timestamp,
            reviewer,
            merge_commit,
            merged_at,
            changelog,
        });
    }
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.name.cmp(&b.name)));
    Ok(out)
}

struct MergeDetails {
    reviewer: String,
    merge_commit: String,
    merged_at: i64,
    changelog: Option<String>,
}

fn merge_details(
    repo: &Path,
    branch: &str,
    name: &str,
    changelog_files: &HashSet<String>,
) -> Result<Option<MergeDetails>> {
    let Some(hash) = git::find_merge_commit(repo, branch)? else {
        return Ok(None);
    };
    let info = git::commit_info(repo, &hash)?;
    let reviewer = info
        .as_ref()
        .and_then(|i| i.subject.split(" reviewed by ").nth(1))
        .unwrap_or_default()
        .to_string();
    let changelog = changelog_files
        .iter()
        .find(|file| changelog_matches(file, name))
        .cloned();
    Ok(info.map(|i| MergeDetails {
        reviewer,
        merge_commit: hash,
        merged_at: i.timestamp,
        changelog,
    }))
}

/// `_changelog/<YYYY-MM-DD>-<name>-<hash6>.md` matches this MR name.
fn changelog_matches(file: &str, name: &str) -> bool {
    let Some(rest) = file.strip_prefix(&format!("{CHANGELOG_DIR}/")) else {
        return false;
    };
    let Some(rest) = rest.strip_suffix(".md") else {
        return false;
    };
    rest.len() >= 11 + name.len() + 7 && rest[11..].starts_with(&format!("{name}-"))
}

/// File tree of one MR branch (draft badges vs main), or of `main`.
pub fn tree(root: &Path, repo_id: &str, mr: Option<&str>) -> Result<Vec<TreeFile>> {
    let repo = repo(root, repo_id)?;
    let refname = match mr {
        Some(name) => {
            crate::model::validate_feature_name(name)?;
            let branch = mr_branch(name);
            if git::tip(&repo, &branch)?.is_none() {
                bail!("MR {name:?} not found");
            }
            branch
        }
        None => MAIN.to_string(),
    };
    let changed: HashSet<String> = match mr {
        Some(_) => git::changed_paths(&repo, MAIN, &refname)?
            .into_iter()
            .collect(),
        None => HashSet::new(),
    };
    Ok(git::ls_tree(&repo, &refname)?
        .into_iter()
        .map(|(path, size)| TreeFile {
            render: render_hint(&path),
            state: if changed.contains(&path) {
                FileBadge::Draft
            } else {
                FileBadge::Stable
            },
            path,
            size,
        })
        .collect())
}

/// Blob content at one MR branch tip (or main).
pub fn blob(root: &Path, repo_id: &str, mr: Option<&str>, path: &str) -> Result<Blob> {
    let repo = repo(root, repo_id)?;
    validate_blob_path(path)?;
    let refname = match mr {
        Some(name) => {
            crate::model::validate_feature_name(name)?;
            mr_branch(name)
        }
        None => MAIN.to_string(),
    };
    let bytes = git::show(&repo, &refname, path)?;
    let size = bytes.len() as u64;
    let truncated = size as usize > MAX_BLOB_BYTES;
    let content = if truncated {
        String::from_utf8_lossy(&bytes[..MAX_BLOB_BYTES]).into_owned()
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };
    Ok(Blob {
        path: path.to_string(),
        size,
        truncated,
        content,
        render: render_hint(path),
    })
}

/// Unified diff `main...mr/<name>`.
pub fn diff(root: &Path, repo_id: &str, name: &str) -> Result<String> {
    let repo = repo(root, repo_id)?;
    crate::model::validate_feature_name(name)?;
    let branch = mr_branch(name);
    if git::tip(&repo, &branch)?.is_none() {
        bail!("MR {name:?} not found");
    }
    git::diff_refs(&repo, MAIN, &branch)
}

/// Commits of one MR: open → `main..branch`; merged → the merge's branch side.
pub fn commits(root: &Path, repo_id: &str, name: &str) -> Result<Vec<CommitInfo>> {
    let repo = repo(root, repo_id)?;
    crate::model::validate_feature_name(name)?;
    let branch = mr_branch(name);
    if git::tip(&repo, &branch)?.is_none() {
        bail!("MR {name:?} not found");
    }
    if git::is_ancestor(&repo, &branch, MAIN)? {
        if let Some(merge) = git::find_merge_commit(&repo, &branch)? {
            return git::log_range(&repo, &format!("{merge}^1..{merge}^2"));
        }
        return Ok(Vec::new());
    }
    git::log_range(&repo, &format!("{MAIN}..{branch}"))
}

/// Changelog entries of `main`, newest first.
pub fn changelog(root: &Path, repo_id: &str) -> Result<Vec<ChangelogEntry>> {
    let repo = match repo(root, repo_id) {
        Ok(repo) => repo,
        Err(_) => return Ok(Vec::new()),
    };
    let mut files: Vec<String> = changelog_files(&repo)?
        .into_iter()
        .collect();
    files.sort();
    files.reverse();
    let mut out = Vec::new();
    for file in files {
        let bytes = git::show(&repo, MAIN, &file)?;
        out.push(ChangelogEntry {
            markdown: String::from_utf8_lossy(&bytes).into_owned(),
            file,
        });
    }
    Ok(out)
}

/// `_changelog/*.md` paths on `main`.
fn changelog_files(repo: &Path) -> Result<HashSet<String>> {
    let prefix = format!("{CHANGELOG_DIR}/");
    Ok(git::ls_tree(repo, MAIN)?
        .into_iter()
        .map(|(path, _)| path)
        .filter(|path| path.starts_with(&prefix) && path.ends_with(".md"))
        .collect())
}
