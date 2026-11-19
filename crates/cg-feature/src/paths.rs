//! Feature-root resolution and layout: `<root>/<repo_id>/<name>/feature.json`
//! plus `<root>/<repo_id>/_changelog/`. All traversal goes through checked
//! helpers that reject symlinks and stay inside the canonical root.

use anyhow::{bail, Context, Result};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Reserved changelog directory below each repository partition.
pub const CHANGELOG_DIR: &str = "_changelog";
/// Feature payload file name.
pub const FEATURE_JSON: &str = "feature.json";
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
        && repo_id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !valid {
        bail!("repo_id must be {REPO_ID_LEN} lowercase hex characters");
    }
    Ok(())
}

/// `<root>/<repo_id>` after validation.
pub fn repo_dir(root: &Path, repo_id: &str) -> Result<PathBuf> {
    validate_repo_id(repo_id)?;
    Ok(root.join(repo_id))
}

/// `<root>/<repo_id>/<name>/feature.json`.
pub fn feature_json_path(root: &Path, repo_id: &str, name: &str) -> Result<PathBuf> {
    Ok(repo_dir(root, repo_id)?.join(name).join(FEATURE_JSON))
}

/// `<root>/<repo_id>/_changelog`.
pub fn changelog_dir(root: &Path, repo_id: &str) -> Result<PathBuf> {
    Ok(repo_dir(root, repo_id)?.join(CHANGELOG_DIR))
}

/// Changelog file name `YYYY-MM-DD-<feature_name>-<suffix>.md`, length-checked.
pub fn changelog_file_name(date: &str, name: &str, suffix: &str) -> Result<String> {
    let file = format!("{date}-{name}-{suffix}.md");
    if file.len() > 240 {
        bail!("changelog file name exceeds 240 bytes");
    }
    Ok(file)
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

/// True when a directory entry name belongs to the reserved namespace.
pub fn is_reserved_entry(name: &str) -> bool {
    name.starts_with('_')
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
    for component in target.strip_prefix(&canonical_root).unwrap_or(&target).components()
    {
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
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("create {}", root.display())),
    }
    root.canonicalize()
        .with_context(|| format!("canonicalize {}", root.display()))
}

/// Verify a path under the canonical root has no symlink components, returning it.
pub fn checked_path(root: &Path, path: &Path) -> Result<PathBuf> {
    let canonical_root = root.canonicalize().with_context(|| {
        format!("feature root {} does not exist", root.display())
    })?;
    let mut current = canonical_root.clone();
    for component in path.components() {
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

/// List a directory's entry names under the root with symlink rejection.
pub fn checked_read_dir(root: &Path, path: &Path) -> Result<Vec<String>> {
    let full = checked_path(root, path)?;
    let entries = fs::read_dir(&full).with_context(|| format!("read {}", full.display()))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| format!("scan {}", full.display()))?;
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    Ok(names)
}

/// Root-relative slash path for git pathspecs (repo_id is pre-validated).
pub fn repo_relative(repo_id: &str, tail: &str) -> String {
    format!("{repo_id}/{tail}")
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
    fn one_component_rejects_traversal() {
        assert!(validate_one_component("ok-dir", "x").is_ok());
        for bad in ["", ".", "..", "a/b", "/abs"] {
            assert!(validate_one_component(bad, "x").is_err(), "{bad}");
        }
    }
}
