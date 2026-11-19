//! Git plumbing for the per-repository document repo (`<root>/<repo_id>`).
//!
//! One repository = one independent git repo: `main` holds the merged stable
//! state, `mr/<name>` branches hold drafts. Reads go through git refs
//! (`ls-tree`/`show`), never through the worktree, so they are lock-free and
//! never observe half-written state. Writers serialize via `lock::acquire`.
//! All invocations run with user/system config disabled (`GIT_CONFIG_GLOBAL=
//! /dev/null`) and `core.quotepath=false` so paths round-trip as raw UTF-8.

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::{Command, Output, Stdio};
pub const MAIN: &str = "main";
const DEFAULT_AUTHOR: &str = "cg-feature";
const DEFAULT_EMAIL: &str = "cg-feature@code-grapher.invalid";

fn git_cmd(repo: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(repo)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .arg("-c")
        .arg("core.quotepath=false");
    cmd
}

fn git(repo: &Path, args: &[&str]) -> Result<Output> {
    git_cmd(repo)
        .args(args)
        .output()
        .with_context(|| format!("run git {}", args.first().unwrap_or(&"")))
}

fn git_expect(repo: &Path, args: &[&str]) -> Result<Output> {
    let output = git(repo, args)?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output)
}

fn identity_env(cmd: &mut Command, author: &str) {
    let name = if author.is_empty() { DEFAULT_AUTHOR } else { author };
    let email = if author.is_empty() {
        DEFAULT_EMAIL.to_string()
    } else {
        format!("{}@feature-review.invalid", slug(author))
    };
    cmd.env("GIT_AUTHOR_NAME", name)
        .env("GIT_AUTHOR_EMAIL", &email)
        .env("GIT_COMMITTER_NAME", name)
        .env("GIT_COMMITTER_EMAIL", email);
}

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

/// Initialize `<repo>` as a git repository on `main` with one empty commit,
/// ignoring the `.lock` file via `.git/info/exclude`. Idempotent.
pub fn ensure_repo(repo: &Path) -> Result<()> {
    if !repo.join(".git").exists() {
        git_expect(repo, &["init", "--quiet", "-b", MAIN])?;
    }
    let info = repo.join(".git").join("info");
    std::fs::create_dir_all(&info).ok();
    let exclude = info.join("exclude");
    let existing = std::fs::read_to_string(&exclude).unwrap_or_default();
    if !existing.lines().any(|line| line.trim() == ".lock") {
        std::fs::write(&exclude, format!("{existing}.lock\n"))
            .with_context(|| format!("write {}", exclude.display()))?;
    }
    if !has_head(repo)? {
        commit_allow_empty(repo, "init: document repository", "")?;
    }
    Ok(())
}

fn has_head(repo: &Path) -> Result<bool> {
    Ok(git(repo, &["rev-parse", "--verify", "--quiet", "HEAD"])?
        .status
        .success())
}

/// Resolve a ref (branch name or `HEAD`) to its commit hash, None if missing.
pub fn tip(repo: &Path, refname: &str) -> Result<Option<String>> {
    let out = git(repo, &["rev-parse", "--verify", "--quiet", refname])?;
    if !out.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8(out.stdout)?.trim().to_string()))
}

fn commit_allow_empty(repo: &Path, message: &str, author: &str) -> Result<String> {
    let mut cmd = git_cmd(repo);
    identity_env(&mut cmd, author);
    let output = cmd
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=",
            "commit",
            "--allow-empty",
            "--no-verify",
            "--quiet",
            "-m",
            message,
        ])
        .output()
        .context("run git commit")?;
    if !output.status.success() {
        bail!(
            "git commit failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(tip(repo, "HEAD")?.unwrap_or_default())
}

/// True when the branch ref exists.
pub fn has_branch(repo: &Path, branch: &str) -> Result<bool> {
    Ok(tip(repo, &format!("refs/heads/{branch}"))?.is_some())
}

/// Create branch `branch` at `base` when missing.
pub fn ensure_branch(repo: &Path, branch: &str, base: &str) -> Result<()> {
    if !has_branch(repo, branch)? {
        git_expect(repo, &["branch", branch, base])?;
    }
    Ok(())
}

/// Hard-sync the worktree to `refname` (force checkout + drop untracked).
/// `.lock` survives because it is excluded via `.git/info/exclude`.
pub fn checkout(repo: &Path, refname: &str) -> Result<()> {
    git_expect(repo, &["checkout", "--quiet", "--force", refname])?;
    git_expect(repo, &["clean", "-fdq", "-e", ".lock"])?;
    Ok(())
}

/// Stage every change (`git add -A`).
pub fn stage_all(repo: &Path) -> Result<()> {
    git_expect(repo, &["add", "--all"])?;
    Ok(())
}

/// True when nothing is staged (`git diff --cached --quiet`: exit 1 means
/// differences exist, other non-zero codes are real errors).
pub fn staged_is_empty(repo: &Path) -> Result<bool> {
    let out = git(repo, &["diff", "--cached", "--quiet"])?;
    match out.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => bail!(
            "git diff --cached failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ),
    }
}

/// Commit what is staged; identity from `author`. Returns the commit hash.
pub fn commit_staged(repo: &Path, message: &str, author: &str) -> Result<String> {
    commit_allow_empty(repo, message, author)
}

/// Reset the worktree and current branch to `refname` (drafts are disposable).
pub fn reset_hard(repo: &Path, refname: &str) -> Result<()> {
    git_expect(repo, &["reset", "--hard", "--quiet", refname])?;
    git_expect(repo, &["clean", "-fdq", "-e", ".lock"])?;
    Ok(())
}

/// Merge `refname` into the current branch. Conflicts abort cleanly and
/// return an error naming the conflicting side.
pub fn merge(repo: &Path, refname: &str, no_ff: bool, message: &str, author: &str) -> Result<()> {
    let mut cmd = git_cmd(repo);
    identity_env(&mut cmd, author);
    let mut args: Vec<&str> = vec!["merge", "--no-edit", "--no-verify"];
    if no_ff {
        args.push("--no-ff");
    }
    if !message.is_empty() {
        args.push("-m");
        args.push(message);
    }
    args.push(refname);
    let output = cmd
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("run git merge {refname}"))?;
    if !output.status.success() {
        let _ = git(repo, &["merge", "--abort"]);
        bail!(
            "git merge {refname} failed (conflict?): refresh the draft and retry; {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// `a` is an ancestor of (or equal to) `b`.
pub fn is_ancestor(repo: &Path, a: &str, b: &str) -> Result<bool> {
    Ok(git(repo, &["merge-base", "--is-ancestor", a, b])?
        .status
        .success())
}

/// Commit counts `(left, right)` for `left...right` (ahead/behind).
pub fn ahead_behind(repo: &Path, left: &str, right: &str) -> Result<(usize, usize)> {
    let out = git_expect(
        repo,
        &["rev-list", "--left-right", "--count", &format!("{left}...{right}")],
    )?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.split_whitespace().map(|n| n.parse::<usize>());
    let left = parts.next().transpose()?.unwrap_or(0);
    let right = parts.next().transpose()?.unwrap_or(0);
    Ok((left, right))
}

pub mod query;
pub use query::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_today_shape() {
        assert_eq!(utc_today().len(), 10);
        assert!(utc_today().starts_with("20"));
    }
}
