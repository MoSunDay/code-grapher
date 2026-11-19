//! Canonical git URL identity.
//!
//! The logical repository identity is the canonicalized `git_url`; the physical
//! directory name is `repo_id = sha256(canonical_git_url)` as 64 lowercase hex
//! characters. Credentials embedded in a URL never reach the identity.

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

/// Canonicalize any accepted git remote spelling to `host/path`.
///
/// Rules: drop the scheme, drop userinfo, lowercase the host, ignore query and
/// fragment, trim trailing `/`, drop a trailing `.git`, keep path casing. URLs
/// that do not carry a host and a non-empty path are rejected.
pub fn canonical_git_url(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("git_url must not be empty");
    }
    let mut rest = trimmed
        .split('#')
        .next()
        .unwrap_or(trimmed)
        .split('?')
        .next()
        .unwrap_or(trimmed);
    if let Some((scheme, after)) = rest.split_once("://") {
        if !valid_scheme(scheme) {
            bail!("invalid scheme in git_url {raw:?}");
        }
        rest = after;
    }
    rest = strip_userinfo(rest);
    let (host, path) = split_host_path(rest).with_context(|| format!("parse git_url {raw:?}"))?;
    if host.is_empty() {
        bail!("git_url {raw:?} has an empty host");
    }
    if host
        .chars()
        .any(|c| !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_'))
    {
        bail!("git_url {raw:?} has an invalid host {host:?}");
    }
    let mut path = path.trim_matches('/').to_string();
    if path.is_empty() {
        bail!("git_url {raw:?} has no repository path");
    }
    if path.len() >= 4 && path[path.len() - 4..].eq_ignore_ascii_case(".git") {
        path.truncate(path.len() - 4);
        path = path.trim_matches('/').to_string();
    }
    if path.is_empty() {
        bail!("git_url {raw:?} has no repository path");
    }
    if path.chars().any(|c| c.is_control()) {
        bail!("git_url {raw:?} contains control characters");
    }
    Ok(format!("{}/{}", host.to_ascii_lowercase(), path))
}

/// `repo_id = sha256(canonical_git_url)`, 64 lowercase hex characters.
pub fn repo_id_from_git_url(raw: &str) -> Result<String> {
    let canonical = canonical_git_url(raw)?;
    Ok(sha256_hex(canonical.as_bytes()))
}

/// Computed API field: `None` when the URL is missing or unparseable.
pub fn feature_repo_id(git_url: Option<&str>) -> Option<String> {
    git_url.and_then(|url| repo_id_from_git_url(url).ok())
}

/// SHA-256 digest as 64 lowercase hex characters.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn valid_scheme(scheme: &str) -> bool {
    let mut chars = scheme.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '.' || c == '-')
}

/// Remove `user[:pass]@` when the `@` precedes the first `/`.
fn strip_userinfo(rest: &str) -> &str {
    let before_slash = rest.split('/').next().unwrap_or(rest);
    match before_slash.rfind('@') {
        Some(at) if at > 0 => &rest[at + 1..],
        _ => rest,
    }
}

/// Split the remaining authority into a lowercase-ready host and a clean path,
/// accepting `host/path`, `host:port/path`, and scp-like `host:path`.
fn split_host_path(rest: &str) -> Result<(String, String)> {
    let (head, after_slash) = match rest.split_once('/') {
        Some((head, tail)) => (head, Some(tail)),
        None => (rest, None),
    };
    let (host, tail) = match head.split_once(':') {
        Some((host, tail)) => (host, Some(tail)),
        None => (head, None),
    };
    let path = match tail {
        // `host:1234/...` is a port; drop it and keep the path after the slash.
        Some(port) if !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) => {
            after_slash.unwrap_or_default().to_string()
        }
        // scp-like `host:owner/repo`; rejoin anything already split at `/`.
        Some(scp) => match after_slash {
            Some(rest_path) => format!("{scp}/{rest_path}"),
            None => scp.to_string(),
        },
        None => after_slash.unwrap_or_default().to_string(),
    };
    if host.is_empty() {
        bail!("empty host");
    }
    Ok((host.to_string(), path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_common_spellings_to_one_identity() {
        let expected = "github.com/owner/repo";
        for url in [
            "https://github.com/owner/repo.git",
            "ssh://git@github.com/owner/repo.git",
            "git@github.com:owner/repo.git",
            "HTTPS://GitHub.com/owner/repo/",
            "https://user:token@github.com/owner/repo.git?x=1#frag",
        ] {
            assert_eq!(canonical_git_url(url).unwrap(), expected, "{url}");
        }
        assert_eq!(
            repo_id_from_git_url("https://github.com/owner/repo.git").unwrap(),
            repo_id_from_git_url("git@github.com:owner/repo").unwrap()
        );
    }

    #[test]
    fn path_casing_and_distinct_urls_matter() {
        assert_ne!(
            repo_id_from_git_url("https://github.com/owner/Repo.git").unwrap(),
            repo_id_from_git_url("https://github.com/owner/repo.git").unwrap()
        );
        assert_ne!(
            repo_id_from_git_url("https://github.com/a/b").unwrap(),
            repo_id_from_git_url("https://gitlab.com/a/b").unwrap()
        );
    }

    #[test]
    fn rejects_invalid_urls() {
        for url in ["", "   ", "github.com", "file:///tmp/repo", "https:///nohost", "not a url!"] {
            assert!(canonical_git_url(url).is_err(), "{url}");
        }
        assert_eq!(feature_repo_id(None), None);
        assert_eq!(feature_repo_id(Some("github.com")), None);
    }
}
