//! Feature-root resolution and layout: `<root>/<repo_id>/` is one independent
//! git document repository (branches `main` and `mr/<name>`). All worktree
//! writes go through checked helpers that reject symlinks and stay inside the
//! canonical repo directory.

use anyhow::{bail, Context, Result};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Reserved changelog directory inside each document repository.
pub const CHANGELOG_DIR: &str = "_changelog";
/// Lock file name serializing writers per repository (NFS-safe flock).
pub const LOCK_FILE: &str = ".lock";
/// repo_id is exactly 64 lowercase hex characters.
pub const REPO_ID_LEN: usize = 64;

/// Resolve the feature root: explicit override, then `CODE_GRAPHER_DATA_DIR`,
/// `${XDG_DATA_HOME}/code-grapher`, then `${HOME}/.local/share/code-grapher`.
pub fn resolve_feature_root(override_root: Option<&Path>) -> Result<PathBuf> {
    resolve_feature_root_with(override_root, |key| env::var_os(key))
}

pub(crate) fn resolve_feature_root_with<F>(override_root: Option<&Path>, get_env: F) -> Result<PathBuf>
where
    F: Fn(&str) -> Option<OsString>,
{
    if let Some(root) = override_root {
        if root.as_os_str().is_empty() {
            bail!("feature root override must not be empty");
        }
        return Ok(root.to_path_buf());
    }
    if let Some(root) = non_empty(get_env("CODE_GRAPHER_DATA_DIR")) {
        return Ok(PathBuf::from(root));
    }
    if let Some(xdg) = non_empty(get_env("XDG_DATA_HOME")) {
        return Ok(PathBuf::from(xdg).join("code-grapher"));
    }
    let home = non_empty(get_env("HOME"))
        .ok_or_else(|| anyhow::anyhow!("set --feature-root, CODE_GRAPHER_DATA_DIR, XDG_DATA_HOME, or HOME to locate feature data"))?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("code-grapher"))
}

fn non_empty(value: Option<OsString>) -> Option<OsString> {
    value.filter(|value| !value.is_empty())
}

/// Validate a repo_id: 64 lowercase hex characters, no separators.
pub fn validate_repo_id(repo_id: &str) -> Result<()> {
    let valid = repo_id.len() == REPO_ID_LEN
        && repo_id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !valid {
        bail!("repo_id must be {REPO_ID_LEN} lowercase hex characters");
    }
    Ok(())
}

/// The per-repository git directory: `<root>/<repo_id>`.
pub fn repo_dir(root: &Path, repo_id: &str) -> Result<PathBuf> {
    validate_repo_id(repo_id)?;
    Ok(root.join(repo_id))
}

/// Changelog directory (relative) inside a document repository.
pub fn changelog_rel() -> PathBuf {
    PathBuf::from(CHANGELOG_DIR)
}

/// Changelog entry file name: `YYYY-MM-DD-<name>-<hash6>.md`.
pub fn changelog_file_name(date: &str, name: &str, suffix: &str) -> Result<String> {
    validate_one_component(name, "feature name")?;
    if suffix.len() != 6 || !suffix.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("changelog suffix must be 6 hex characters");
    }
    let file = format!("{date}-{name}-{suffix}.md");
    if file.len() > 240 {
        bail!("changelog file name exceeds 240 bytes");
    }
    Ok(file)
}

/// Validate a slash-separated repository-relative file path (docs tree) for
/// writes: additionally rejects the reserved `_changelog` namespace.
pub fn validate_rel_path(path: &str) -> Result<()> {
    check_rel_path(path)?;
    let head = path.split('/').next().unwrap_or_default();
    if head == CHANGELOG_DIR {
        bail!(
            "path must not enter the reserved {CHANGELOG_DIR} namespace, got {path:?}"
        );
    }
    Ok(())
}

/// Validate a repository-relative path for reads (changelog readable too).
pub fn validate_blob_path(path: &str) -> Result<()> {
    check_rel_path(path)
}

/// Shared rel-path rules: non-empty, no leading/trailing `/`, no empty or
/// `.`/`..` components, no control characters, bounded depth, never inside
/// `.git`/`.lock` internals.
fn check_rel_path(path: &str) -> Result<()> {
    if path.is_empty() {
        bail!("path must not be empty");
    }
    if path.starts_with('/') || path.ends_with('/') {
        bail!("path must be relative without leading/trailing '/', got {path:?}");
    }
    if path.contains('\0') || path.chars().any(|c| c.is_control()) {
        bail!("path must not contain control characters, got {path:?}");
    }
    let mut depth = 0usize;
    for component in path.split('/') {
        if component.is_empty() {
            bail!("path must not contain empty '/'-separated components, got {path:?}");
        }
        if component == "." || component == ".." {
            bail!("path must not contain '.' or '..' components, got {path:?}");
        }
        depth += 1;
    }
    if depth > 16 {
        bail!("path nesting exceeds 16 components, got {path:?}");
    }
    if path
        .split('/')
        .any(|c| c == ".git" || c == LOCK_FILE || c == ".gitignore")
    {
        bail!("path must not touch repository internals, got {path:?}");
    }
    Ok(())
}

/// Reject a relative path that is not exactly one safe component.
pub fn validate_one_component(value: &str, label: &str) -> Result<()> {
    let valid = !value.is_empty()
        && value != "."
        && value != ".."
        && matches!(
            Path::new(value).components().next(),
            Some(Component::Normal(_))
        )
        && Path::new(value).components().count() == 1;
    if !valid {
        bail!("{label} must be a single safe directory component, got {value:?}");
    }
    Ok(())
}

/// Create `dir` (and parents) below `root`, rejecting symlinks on the way.
pub fn ensure_dir_checked(root: &Path, dir: &Path) -> Result<()> {
    let canonical_root = canonical_root(root)?;
    let target = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        canonical_root.join(dir)
    };
    let mut current = canonical_root.clone();
    for component in target.strip_prefix(&canonical_root).unwrap_or(&target).components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => bail!("{} exists and is not a directory", current.display()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)
                    .with_context(|| format!("create {}", current.display()))?;
            }
            Err(error) => {
                return Err(error).with_context(|| format!("inspect {}", current.display()));
            }
        }
    }
    Ok(())
}

/// Canonicalize the root, creating it when missing.
pub fn canonical_root(root: &Path) -> Result<PathBuf> {
    match fs::create_dir_all(root) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).with_context(|| format!("create {}", root.display())),
    }
    let canonical = root
        .canonicalize()
        .with_context(|| format!("canonicalize {}", root.display()))?;
    if !canonical.is_dir() {
        bail!("feature root {} is not a directory", canonical.display());
    }
    Ok(canonical)
}

/// Resolve `path` below `root`, rejecting symlink escapes along the way.
pub fn checked_path(root: &Path, path: &Path) -> Result<PathBuf> {
    let canonical_root = canonical_root(root)?;
    let target = if path.is_absolute() {
        path.to_path_buf()
    } else {
        canonical_root.join(path)
    };
    let mut current = canonical_root.clone();
    for component in target.strip_prefix(&canonical_root).unwrap_or(&target).components() {
        current.push(component);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() {
                bail!("symlink rejected in feature storage: {}", current.display());
            }
        }
    }
    Ok(current)
}

/// Read a file under the root with symlink rejection.
pub fn checked_read(root: &Path, path: &Path) -> Result<Vec<u8>> {
    let full = checked_path(root, path)?;
    fs::read(&full).with_context(|| format!("read {}", full.display()))
}

/// Overwrite (or create) a file under the root with symlink rejection.
pub fn checked_write(root: &Path, path: &Path, bytes: &[u8]) -> Result<()> {
    let full = checked_path(root, path)?;
    fs::write(&full, bytes).with_context(|| format!("write {}", full.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_id_shape_is_enforced() {
        assert!(validate_repo_id(&"a".repeat(64)).is_ok());
        assert!(validate_repo_id(&"A".repeat(64)).is_err());
        assert!(validate_repo_id(&"a".repeat(63)).is_err());
        assert!(validate_repo_id("../escape").is_err());
    }

    #[test]
    fn rel_paths_reject_traversal_and_reserved() {
        assert!(validate_rel_path("docs/a.md").is_ok());
        assert!(validate_rel_path("a/b/c.json").is_ok());
        for bad in [
            "",
            "/abs",
            "a/",
            "a//b",
            "./a",
            "a/../b",
            "..",
            "_changelog/x.md",
            "_changelog",
            ".git/config",
            "a/.lock",
            "a/.git/x",
            "a/\u{1}b",
        ] {
            assert!(validate_rel_path(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn one_component_rejects_traversal() {
        assert!(validate_one_component("ok-dir", "x").is_ok());
        for bad in ["", ".", "..", "a/b", "/abs"] {
            assert!(validate_one_component(bad, "x").is_err(), "{bad}");
        }
    }
}
