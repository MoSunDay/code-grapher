//! Read-only git queries: tree/blob/log/diff access via git refs.
//!
//! Everything here resolves content from refs (`ls-tree`, `show`, `log`,
//! `diff`), never from the worktree, so callers never need the write lock.

use super::{git, git_expect, MAIN};
use anyhow::{bail, Result};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const FS: &str = "\x1f";

/// One `git log`/`git show` record (parsed from a `--format` line).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CommitInfo {
    pub hash: String,
    pub parents: Vec<String>,
    pub author: String,
    pub timestamp: i64,
    pub subject: String,
}
/// Tracked files of `refname` with blob sizes, sorted by path.
pub fn ls_tree(repo: &Path, refname: &str) -> Result<Vec<(String, u64)>> {
    let out = git_expect(repo, &["ls-tree", "-r", "-l", "--", refname])?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut files = Vec::new();
    for line in text.lines() {
        let Some((meta, path)) = line.split_once('\t') else {
            continue;
        };
        let size = meta
            .split_whitespace()
            .nth(3)
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        files.push((path.to_string(), size));
    }
    files.sort();
    Ok(files)
}

/// Blob content of `path` at `refname` (Err when the path does not exist).
/// Blob content of `path` at `refname` (Err when the path does not exist).
pub fn show(repo: &Path, refname: &str, path: &str) -> Result<Vec<u8>> {
    let out = git(repo, &["show", &format!("{refname}:{path}")])?;
    if !out.status.success() {
        bail!("path {path:?} not found at {refname}");
    }
    Ok(out.stdout)
}

/// Unified diff `from...to` (since merge-base).
/// Unified diff `from...to` (since merge-base).
pub fn diff_refs(repo: &Path, from: &str, to: &str) -> Result<String> {
    let out = git_expect(repo, &["diff", &format!("{from}...{to}")])?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Paths changed between `from...to`.
/// Paths changed between `from...to`.
pub fn changed_paths(repo: &Path, from: &str, to: &str) -> Result<Vec<String>> {
    let out = git_expect(repo, &["diff", "--name-only", &format!("{from}...{to}")])?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .filter(|l| !l.is_empty())
        .collect())
}

/// Commits in `range` (rev-list syntax), oldest first.
/// Commits in `range` (rev-list syntax), oldest first.
pub fn log_range(repo: &Path, range: &str) -> Result<Vec<CommitInfo>> {
    let out = git_expect(
        repo,
        &["log", &format!("--format=%H{FS}%an{FS}%at{FS}%P{FS}%s"), "--reverse", range],
    )?;
    let text = String::from_utf8_lossy(&out.stdout);
    let commits = text.lines().filter_map(parse_commit_line).collect();
    Ok(commits)
}

/// Commit info of one ref, None when missing.
/// Commit info of one ref, None when missing.
pub fn commit_info(repo: &Path, refname: &str) -> Result<Option<CommitInfo>> {
    let out = git(
        repo,
        &[
            "show",
            "--no-patch",
            &format!("--format=%H{FS}%an{FS}%at{FS}%P{FS}%s"),
            refname,
        ],
    )?;
    if !out.status.success() {
        return Ok(None);
    }
    let line = String::from_utf8_lossy(&out.stdout);
    Ok(parse_commit_line(line.trim()).map(std::iter::once).map(|mut i| i.next().unwrap()))
}

/// Parse one `--format` commit line into [`CommitInfo`].
fn parse_commit_line(line: &str) -> Option<CommitInfo> {
    let fields: Vec<&str> = line.split(FS).collect();
    if fields.len() < 5 {
        return None;
    }
    Some(CommitInfo {
        hash: fields[0].to_string(),
        author: fields[1].to_string(),
        timestamp: fields[2].parse().unwrap_or(0),
        parents: fields[3].split_whitespace().map(str::to_string).collect(),
        subject: fields[4].to_string(),
    })
}

/// The `--no-ff` merge commit that first brought `branch` into MAIN, if any.
/// The `--no-ff` merge commit that first brought `branch` into MAIN, if any.
pub fn find_merge_commit(repo: &Path, branch: &str) -> Result<Option<String>> {
    let out = git(
        repo,
        &[
            "rev-list",
            "--merges",
            "--ancestry-path",
            &format!("{branch}..{MAIN}"),
        ],
    )?;
    if !out.status.success() {
        return Ok(None);
    }
    // rev-list prints newest-first; the oldest merge is the one we want.
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .last()
        .map(str::to_string))
}

/// List `mr/*` branch names.
/// List `mr/*` branch names.
pub fn mr_branches(repo: &Path) -> Result<Vec<String>> {
    let out = git_expect(repo, &["for-each-ref", "--format=%(refname:short)", "refs/heads/mr/"])?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .filter(|l| !l.is_empty())
        .collect())
}

/// UTC date as `YYYY-MM-DD` for changelog file names.
pub fn utc_today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Howard Hinnant's civil-from-days conversion.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
