//! Git plumbing for feature review semantics.
//!
//! The feature root is a git worktree: `git diff HEAD` non-empty means draft,
//! empty means stable, and approve commits only the reviewed paths. All git
//! invocations run with user/system config disabled so results never depend on
//! the invoking machine's global gitignore or identity.

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_AUTHOR: &str = "cg-feature";
const DEFAULT_EMAIL: &str = "cg-feature@code-grapher.invalid";

fn git(root: &Path, args: &[&str]) -> Result<Output> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args(args)
        .output()
        .with_context(|| format!("run git {}", args.first().unwrap_or(&"")))?;
    Ok(output)
}

fn git_expect(root: &Path, args: &[&str]) -> Result<Output> {
    let output = git(root, args)?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output)
}

/// Initialize the feature root as a git repository when it is not one yet.
pub fn ensure_git_repo(root: &Path) -> Result<()> {
    if root.join(".git").exists() {
        return Ok(());
    }
    git_expect(root, &["init", "--quiet"])?;
    Ok(())
}

/// True when the worktree has at least one commit.
pub fn has_head(root: &Path) -> Result<bool> {
    Ok(git(root, &["rev-parse", "--verify", "--quiet", "HEAD"])?
        .status
        .success())
}

/// HEAD commit hash, or None before the first commit.
pub fn head_commit(root: &Path) -> Result<Option<String>> {
    if !has_head(root)? {
        return Ok(None);
    }
    let out = git_expect(root, &["rev-parse", "HEAD"])?;
    Ok(Some(String::from_utf8(out.stdout)?.trim().to_string()))
}

/// True when the path has uncommitted changes (modified, staged, or untracked).
pub fn path_is_dirty(root: &Path, rel: &str) -> Result<bool> {
    let output = git_expect(root, &["status", "--porcelain", "-uall", "--", rel])?;
    Ok(!output.stdout.is_empty())
}

/// True when the path is already tracked in HEAD (exists in git history).
pub fn is_tracked(root: &Path, rel: &str) -> Result<bool> {
    if !has_head(root)? {
        return Ok(false);
    }
    Ok(git(root, &["ls-files", "--error-unmatch", rel, "--"])?
        .status
        .success())
}

/// Unified diff of the path against HEAD; untracked files diff against empty.
pub fn diff_head(root: &Path, rel: &str, abs: &Path) -> Result<String> {
    if is_tracked(root, rel)? {
        let out = git_expect(root, &["diff", "HEAD", "--", rel])?;
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    let out = git(
        root,
        &["diff", "--no-index", "--", "/dev/null", &abs.to_string_lossy()],
    )?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Stage exactly `rels` and commit them (pathspec commit; other dirty files
/// stay out). Returns the new commit hash. `author` names the human reviewer.
pub fn commit_paths(root: &Path, rels: &[&str], message: &str, author: &str) -> Result<String> {
    for rel in rels {
        git_expect(root, &["add", "--", rel])?;
    }
    let name = if author.is_empty() { DEFAULT_AUTHOR } else { author };
    let email = if author.is_empty() {
        DEFAULT_EMAIL.to_string()
    } else {
        format!("{}@feature-review.invalid", slug(author))
    };
    commit_run(root, name, &email, message, rels)
}

fn commit_run(root: &Path, name: &str, email: &str, message: &str, rels: &[&str]) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", name)
        .env("GIT_AUTHOR_EMAIL", email)
        .env("GIT_COMMITTER_NAME", name)
        .env("GIT_COMMITTER_EMAIL", email)
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=",
            "commit",
            "--no-verify",
            "--quiet",
            "-m",
            message,
            "--",
        ])
        .args(rels)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd
        .output()
        .with_context(|| "run git commit")?;
    if !output.status.success() {
        bail!(
            "git commit failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(head_commit(root)?.unwrap_or_default())
}

/// ASCII slug for a reviewer name used inside a generated email local part.
fn slug(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "reviewer".to_string()
    } else {
        trimmed
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_is_well_formed() {
        let today = utc_today();
        let parts: Vec<&str> = today.split('-').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].len(), 4);
        assert!(parts[1].parse::<u32>().unwrap() >= 1);
        assert!(parts[2].parse::<u32>().unwrap() >= 1);
    }

    #[test]
    fn slug_is_ascii() {
        assert_eq!(slug("Alice Reviewer"), "alice-reviewer");
        assert_eq!(slug("评审人"), "reviewer");
    }
}
