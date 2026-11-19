//! Approve path: merge `mr/<name>` into `main` (`--no-ff`) as the reviewer,
//! then record a `_changelog/` entry on `main`.

use anyhow::{bail, Result};
use std::path::Path;

use crate::git::{self, MAIN};
use crate::lock;
use crate::model::{mr_branch, validate_feature_name};
use crate::paths::{
    canonical_root, changelog_file_name, changelog_rel, checked_write, ensure_dir_checked,
    repo_dir, validate_repo_id, CHANGELOG_DIR,
};

/// Approve request; `reviewer` is the human approver.
#[derive(Debug, Clone)]
pub struct ApproveParams<'a> {
    pub reviewer: &'a str,
    pub note: Option<&'a str>,
    /// Require this `mr/<name>` tip hash before merging (guard against races).
    pub expected_tip: Option<&'a str>,
}

/// Approve result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApproveOutcome {
    pub merge_commit: String,
    pub changelog: String,
    pub mr_commit: String,
}

/// Merge the reviewed MR branch into `main` and write its changelog entry.
pub fn approve(
    root: &Path,
    repo_id: &str,
    name: &str,
    params: &ApproveParams,
) -> Result<ApproveOutcome> {
    validate_repo_id(repo_id)?;
    validate_feature_name(name)?;
    if params.reviewer.trim().is_empty() {
        bail!("reviewer must not be empty");
    }
    let root = canonical_root(root)?;
    let repo = repo_dir(&root, repo_id)?;
    if !repo.join(".git").exists() {
        bail!("feature repository for {repo_id} not found");
    }
    let branch = mr_branch(name);
    let _guard = lock::acquire(&root, repo_id)?;
    let mr_tip = git::tip(&repo, &branch)?
        .ok_or_else(|| anyhow::anyhow!("MR {name:?} not found"))?;
    if let Some(expected) = params.expected_tip {
        if expected != mr_tip {
            bail!(
                "MR tip changed: expected {expected}, current {mr_tip}; review the new draft"
            );
        }
    }
    if git::is_ancestor(&repo, &branch, MAIN)? {
        bail!("MR {name:?} is already merged");
    }
    let message = format!("Merge MR {name:?} reviewed by {}", params.reviewer);
    git::checkout(&repo, MAIN)?;
    git::merge(&repo, &branch, true, &message, params.reviewer)?;
    let merge_commit = git::tip(&repo, "HEAD")?.unwrap_or_default();
    let changelog = write_changelog(&repo, name, &merge_commit, &mr_tip, params)?;
    Ok(ApproveOutcome {
        merge_commit,
        changelog,
        mr_commit: mr_tip,
    })
}

/// Append the `_changelog/YYYY-MM-DD-<name>-<hash6>.md` entry on `main`.
fn write_changelog(
    repo: &Path,
    name: &str,
    merge_commit: &str,
    mr_tip: &str,
    params: &ApproveParams,
) -> Result<String> {
    let date = git::utc_today();
    let suffix: String = merge_commit.chars().take(6).collect();
    let file = changelog_file_name(&date, name, &suffix)?;
    let note = params.note.unwrap_or("").trim();
    let markdown = format!(
        "# MR {name} merged\n\n\
         - reviewer: {}\n\
         - merged_at: {date}\n\
         - merge_commit: {merge_commit}\n\
         - mr_branch_tip: {mr_tip}\n{}",
        params.reviewer,
        if note.is_empty() {
            String::new()
        } else {
            format!("- note: {note}\n")
        }
    );
    let rel = changelog_rel().join(&file);
    ensure_dir_checked(repo, &changelog_rel())?;
    checked_write(repo, &rel, markdown.as_bytes())?;
    git::stage_all(repo)?;
    if !git::staged_is_empty(repo)? {
        git::commit_staged(repo, &format!("changelog: MR {name:?}"), params.reviewer)?;
    }
    Ok(format!("{CHANGELOG_DIR}/{file}"))
}
