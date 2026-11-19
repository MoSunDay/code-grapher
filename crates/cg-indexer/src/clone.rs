//! Git clone support for URL-registered repositories.
//!
//! Pure functions + a subprocess `git clone` (matching the repo-wide
//! shell-out-to-git convention). No credentials are handled here: prompts are
//! disabled and private-repo auth flows through the server's git environment.

use anyhow::{bail, Context, Result};
use cg_core::parser::fnv1a_hex;
use std::path::{Path, PathBuf};

/// Sanitize a raw name fragment into a safe single directory component:
/// alphanumerics plus `-_.`, everything else collapsed to `-`; no leading dot
/// or dash (blocks traversal and hidden dirs); never empty.
pub fn sanitize_repo_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_dash = false;
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    let stripped = trimmed.strip_prefix('.').map(str::to_string).unwrap_or(trimmed);
    let name = stripped.trim_matches('-').trim_end_matches('.');
    if name.is_empty() {
        "repo".into()
    } else {
        name.chars().take(64).collect()
    }
}

/// Managed clone directory: `<root>/<sanitized tail>-<fnv1a16(url)>`.
/// The content hash suffix prevents both same-name collisions and any path
/// traversal smuggled through the URL.
pub fn derive_clone_dir(root: &Path, canonical_url: &str) -> PathBuf {
    let tail = canonical_url.rsplit('/').next().unwrap_or(canonical_url);
    let name = sanitize_repo_name(tail);
    let digest = &fnv1a_hex(canonical_url.as_bytes());
    root.join(format!("{name}-{digest}"))
}

/// Clone timeout in seconds, from `CG_CLONE_TIMEOUT`.
/// Unset -> default 1800; `0` or unparsable -> no timeout.
fn clone_timeout_secs() -> Option<u64> {
    match std::env::var("CG_CLONE_TIMEOUT") {
        Ok(v) => v.trim().parse::<u64>().ok().filter(|s| *s > 0),
        Err(_) => Some(1800),
    }
}

/// Clone `url` into `dir`, a fresh managed directory under the clones root.
/// `GIT_TERMINAL_PROMPT=0` keeps headless servers from hanging on auth
/// prompts; failures (and timeouts, see `CG_CLONE_TIMEOUT`) clean up any
/// partial clone left behind. The child runs with `kill_on_drop` so a
/// cancelled/timed-out future reaps the git process instead of leaking it.
pub async fn run_clone(url: &str, dir: &Path) -> Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir).with_context(|| format!("clear stale clone dir {}", dir.display()))?;
    }
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create clones root {}", parent.display()))?;
    }
    let child = tokio::process::Command::new("git")
        .arg("clone")
        .arg("--quiet")
        .arg(url)
        .arg(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("spawn git clone {url}"))?;
    let waited = tokio::time::Duration::from_secs;
    let output = match clone_timeout_secs() {
        Some(secs) => match tokio::time::timeout(waited(secs), child.wait_with_output()).await {
            Ok(res) => res.with_context(|| format!("git clone {url}"))?,
            // dropping the future drops the Child -> kill_on_drop kills git
            Err(_) => {
                let _ = std::fs::remove_dir_all(dir);
                bail!("git clone {url} timed out after {secs}s (CG_CLONE_TIMEOUT)");
            }
        },
        None => child.wait_with_output().await.with_context(|| format!("git clone {url}"))?,
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        // best-effort cleanup of the partial clone
        let _ = std::fs::remove_dir_all(dir);
        bail!("git clone {url} failed: {stderr}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_ok(dir: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.email=t@t.invalid", "-c", "user.name=t"])
            .args(args)
            .output()
            .expect("git binary");
        assert!(out.status.success(), "git {:?} failed", args);
    }

    #[test]
    fn sanitize_strips_traversal_and_unsafe_chars() {
        assert_eq!(sanitize_repo_name(".."), "repo");
        assert_eq!(sanitize_repo_name("../../etc/passwd"), "etc-passwd");
        assert_eq!(sanitize_repo_name("a/b\\c d"), "a-b-c-d");
        assert_eq!(sanitize_repo_name(".hidden"), "hidden");
        assert_eq!(sanitize_repo_name("组织_仓库"), "_"); // non-ascii collapses
        assert_eq!(sanitize_repo_name("!!!"), "repo");
        assert_eq!(sanitize_repo_name(&"x".repeat(100)).len(), 64);
    }

    #[test]
    fn derive_dir_is_deterministic_and_under_root() {
        let a = derive_clone_dir(Path::new("/srv/repos"), "github.com/org/repo");
        let b = derive_clone_dir(Path::new("/srv/repos"), "github.com/org/repo");
        let c = derive_clone_dir(Path::new("/srv/repos"), "github.com/other/repo");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("/srv/repos"));
        assert_eq!(a.parent(), Some(Path::new("/srv/repos")));
        // traversal tail cannot escape the root: component stays sanitized
        let evil = derive_clone_dir(Path::new("/srv/repos"), "github.com/org/..%2Fetc");
        assert!(evil.starts_with("/srv/repos"));
        assert_eq!(evil.components().count(), 4); // RootDir + srv + repos + one sanitized leaf
        assert!(evil
            .components()
            .all(|c| !matches!(c, std::path::Component::ParentDir)));
    }

    #[tokio::test]
    async fn clone_local_repo_and_cleanup_on_failure() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path().join("origin");
        let target_root = tmp.path().join("clones");
        std::fs::create_dir_all(&origin).unwrap();
        git_ok(&origin, &["init", "--quiet"]);
        std::fs::write(origin.join("README.md"), "# hi\n").unwrap();
        git_ok(&origin, &["add", "."]);
        git_ok(&origin, &["commit", "--quiet", "-m", "init"]);

        let dir = derive_clone_dir(&target_root, "example.com/org/origin");
        run_clone(origin.to_str().unwrap(), &dir).await.expect("clone ok");
        assert!(dir.join(".git").exists());
        assert!(dir.join("README.md").exists());

        // failure path: bogus URL, partial dir must be cleaned up
        let bad = derive_clone_dir(&target_root, "example.com/org/nope");
        let err = run_clone(&tmp.path().join("missing").to_string_lossy(), &bad)
            .await
            .expect_err("clone should fail");
        assert!(err.to_string().contains("git clone"));
        assert!(!bad.exists());
    }
}
