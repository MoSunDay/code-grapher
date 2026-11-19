//! High-level feature operations: draft, list, status, diff, approve, changelog.

use crate::git;
use crate::model::{
    canonical_feature_bytes, feature_hash, parse_feature, validate_feature_name, Feature,
};
use crate::paths::{
    changelog_dir, changelog_file_name, checked_read, checked_read_dir, checked_write,
    ensure_dir_checked, feature_json_path, is_reserved_entry, repo_dir, validate_repo_id,
    CHANGELOG_DIR, FEATURE_JSON,
};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureState {
    Draft,
    Stable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FeatureSummary {
    pub name: String,
    pub state: FeatureState,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RepoStatus {
    pub repo_id: String,
    pub head: Option<String>,
    pub features: Vec<FeatureSummary>,
    pub changelog: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftOutcome {
    Noop,
    Created { rel_path: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApproveOutcome {
    pub commit: String,
    pub changelog: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ApproveParams<'a> {
    pub reviewer: &'a str,
    pub note: Option<&'a str>,
    pub expected_hash: Option<&'a str>,
}

fn ensure_repo_partition(root: &Path, repo_id: &str) -> Result<()> {
    let dir = repo_dir(root, repo_id)?;
    ensure_dir_checked(root, &dir)?;
    git::ensure_git_repo(root).context("initialize feature git repository")?;
    Ok(())
}

/// Write (or refresh) the draft for one feature; never commits.
pub fn draft(root: &Path, repo_id: &str, source: &[u8], expected_name: Option<&str>) -> Result<DraftOutcome> {
    validate_repo_id(repo_id)?;
    let root = crate::paths::canonical_root(root)?;
    let feature = parse_feature(source)?;
    if let Some(expected) = expected_name {
        if feature.name != expected {
            bail!(
                "expected feature name {expected:?}, but source contains {:?}",
                feature.name
            );
        }
    }
    ensure_repo_partition(&root, repo_id)?;
    write_feature(&root, repo_id, &feature)
}

fn write_feature(root: &Path, repo_id: &str, feature: &Feature) -> Result<DraftOutcome> {
    let path = feature_json_path(root, repo_id, &feature.name)?;
    let bytes = canonical_feature_bytes(feature)?;
    if path.exists() && checked_read(root, &path)? == bytes {
        return Ok(DraftOutcome::Noop);
    }
    ensure_dir_checked(root, path.parent().expect("feature dir"))?;
    checked_write(root, &path, &bytes)?;
    Ok(DraftOutcome::Created {
        rel_path: format!("{repo_id}/{}/{FEATURE_JSON}", feature.name),
    })
}

/// List every feature in the partition with its git-derived state.
pub fn list(root: &Path, repo_id: &str) -> Result<Vec<FeatureSummary>> {
    let root = crate::paths::canonical_root(root)?;
    let summaries = feature_json_files(&root, repo_id)?;
    let mut out = Vec::with_capacity(summaries.len());
    for (name, json_path) in summaries {
        let rel = format!("{repo_id}/{name}/{FEATURE_JSON}");
        let bytes = checked_read(&root, &json_path)?;
        let state = if git_is_dirty(&root, &rel)? {
            FeatureState::Draft
        } else {
            FeatureState::Stable
        };
        out.push(FeatureSummary {
            name,
            state,
            hash: Some(feature_hash(&bytes)),
        });
    }
    Ok(out)
}

fn git_is_dirty(root: &Path, rel: &str) -> Result<bool> {
    if !root.join(".git").exists() {
        bail!("feature root {} is not a git repository; run draft first", root.display());
    }
    git::path_is_dirty(root, rel)
}

fn feature_json_files(root: &Path, repo_id: &str) -> Result<Vec<(String, std::path::PathBuf)>> {
    validate_repo_id(repo_id)?;
    let dir = repo_dir(root, repo_id)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for name in checked_read_dir(root, &dir)? {
        if is_reserved_entry(&name) {
            continue;
        }
        let json = dir.join(&name).join(FEATURE_JSON);
        if json.is_file() {
            out.push((name, json));
        }
    }
    Ok(out)
}

/// Aggregate status for one repository partition.
pub fn status(root: &Path, repo_id: &str) -> Result<RepoStatus> {
    let root = crate::paths::canonical_root(root)?;
    let features = list(&root, repo_id)?;
    let changelog = list_changelog(&root, repo_id)?;
    let head = if root.join(".git").exists() {
        git::head_commit(&root)?
    } else {
        None
    };
    Ok(RepoStatus {
        repo_id: repo_id.to_string(),
        head,
        features,
        changelog,
    })
}

/// Unified diff against HEAD for one feature (or all features when `name` is None).
pub fn diff(root: &Path, repo_id: &str, name: Option<&str>) -> Result<String> {
    validate_repo_id(repo_id)?;
    let root = crate::paths::canonical_root(root)?;
    let targets: Vec<(String, std::path::PathBuf)> = match name {
        Some(name) => {
            validate_feature_name(name)?;
            let path = feature_json_path(&root, repo_id, name)?;
            if !path.is_file() {
                bail!("feature {name:?} not found in {repo_id}");
            }
            vec![(name.to_string(), path)]
        }
        None => feature_json_files(&root, repo_id)?,
    };
    let mut combined = String::new();
    for (name, json_path) in targets {
        let rel = format!("{repo_id}/{name}/{FEATURE_JSON}");
        let dirty = git_is_dirty(&root, &rel)?;
        if dirty {
            let abs = json_path.canonicalize().with_context(|| {
                format!("resolve {}", json_path.display())
            })?;
            combined.push_str(&git::diff_head(&root, &rel, &abs)?);
        }
    }
    Ok(combined)
}

/// Commit the reviewed feature plus its changelog entry; the commit *is* the
/// transition to stable. Nothing else enters the commit.
pub fn approve(root: &Path, repo_id: &str, name: &str, params: &ApproveParams) -> Result<ApproveOutcome> {
    validate_repo_id(repo_id)?;
    let root = crate::paths::canonical_root(root)?;
    validate_feature_name(name)?;
    if params.reviewer.trim().is_empty() {
        bail!("approve requires a non-empty --reviewer");
    }
    ensure_repo_partition(&root, repo_id)?;
    let json_path = feature_json_path(&root, repo_id, name)?;
    if !json_path.is_file() {
        bail!("feature {name:?} not found in {repo_id}");
    }
    let bytes = checked_read(&root, &json_path)?;
    let hash = feature_hash(&bytes);
    if let Some(expected) = params.expected_hash {
        if !expected.eq_ignore_ascii_case(&hash) {
            bail!("expected hash {expected} does not match current content {hash}");
        }
    }
    let feature_rel = format!("{repo_id}/{name}/{FEATURE_JSON}");
    if !git_is_dirty(&root, &feature_rel)? {
        bail!("feature {name:?} is already stable; nothing to approve");
    }
    let suffix = hash.get(..6).unwrap_or("000000").to_string();
    let date = git::utc_today();
    let file = changelog_file_name(&date, name, &suffix)?;
    let cl_dir = changelog_dir(&root, repo_id)?;
    ensure_dir_checked(&root, &cl_dir)?;
    let cl_rel = format!("{repo_id}/{CHANGELOG_DIR}/{file}");
    let cl_abs = cl_dir.join(&file);
    if cl_abs.exists() {
        bail!("changelog {cl_rel} already exists; this content was already reviewed");
    }
    checked_write(&root, &cl_abs, changelog_markdown(&date, name, params, &hash).as_bytes())?;
    let message = format!("approve: {name} ({})", params.reviewer);
    let commit = match git::commit_paths(&root, &[&feature_rel, &cl_rel], &message, params.reviewer) {
        Ok(commit) => commit,
        Err(error) => {
            let _ = std::fs::remove_file(&cl_abs);
            return Err(error);
        }
    };
    Ok(ApproveOutcome {
        commit,
        changelog: cl_rel,
    })
}

fn changelog_markdown(date: &str, name: &str, params: &ApproveParams, hash: &str) -> String {
    let note = params.note.unwrap_or("").trim();
    format!(
        "# approve {name}\n\n\
         - date: {date}\n\
         - reviewer: {}\n\
         - feature.json sha256: {hash}\n\n\
         ## note\n\n{}\n",
        params.reviewer,
        if note.is_empty() { "(none)" } else { note }
    )
}

/// Changelog file names (newest first) for one repository partition.
pub fn list_changelog(root: &Path, repo_id: &str) -> Result<Vec<String>> {
    let root = crate::paths::canonical_root(root)?;
    let dir = changelog_dir(&root, repo_id)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut names = checked_read_dir(&root, &dir)?
        .into_iter()
        .filter(|n| n.ends_with(".md"))
        .collect::<Vec<_>>();
    names.sort();
    names.reverse();
    Ok(names)
}

/// Changelog entries with content, newest first.
pub fn read_changelog(root: &Path, repo_id: &str) -> Result<Vec<ChangelogEntry>> {
    let root = crate::paths::canonical_root(root)?;
    let dir = changelog_dir(&root, repo_id)?;
    let mut out = Vec::new();
    for file in list_changelog(&root, repo_id)? {
        let bytes = checked_read(&root, &dir.join(&file))?;
        out.push(ChangelogEntry {
            file,
            markdown: String::from_utf8(bytes).context("changelog must be UTF-8")?,
        });
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChangelogEntry {
    pub file: String,
    pub markdown: String,
}

/// Read one feature.json plus its state, for the detail API.
pub fn read_feature(root: &Path, repo_id: &str, name: &str) -> Result<FeatureDetail> {
    validate_repo_id(repo_id)?;
    let root = crate::paths::canonical_root(root)?;
    validate_feature_name(name)?;
    let json_path = feature_json_path(&root, repo_id, name)?;
    if !json_path.is_file() {
        bail!("feature {name:?} not found in {repo_id}");
    }
    let bytes = checked_read(&root, &json_path)?;
    let feature: Feature = serde_json::from_slice(&bytes)?;
    let rel = format!("{repo_id}/{name}/{FEATURE_JSON}");
    let state = if git_is_dirty(&root, &rel)? {
        FeatureState::Draft
    } else {
        FeatureState::Stable
    };
    Ok(FeatureDetail {
        repo_id: repo_id.to_string(),
        name: name.to_string(),
        state,
        hash: feature_hash(&bytes),
        feature,
    })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeatureDetail {
    pub repo_id: String,
    pub name: String,
    pub state: FeatureState,
    pub hash: String,
    pub feature: Feature,
}
