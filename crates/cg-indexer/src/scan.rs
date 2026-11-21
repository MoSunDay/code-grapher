//! Repository scanning: collect indexable source files.

use anyhow::Context;
use cg_core::lang::{lang_for_path, Lang, SKIP_DIRS};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ScannedFile {
    /// Path relative to repo root, `/`-separated.
    pub rel_path: String,
    pub abs_path: PathBuf,
    pub mtime: i64,
    pub size: u64,
    pub lang: Lang,
}

/// Recursively scan `root` for files the language registry can parse.
/// Skips VCS/build/vendor directories.
///
/// Any filesystem error (unreadable root/subdirectory, unreadable entry or
/// metadata) is propagated so callers can abort *before* sweeping files from
/// the database; a partial scan must never look like a deletion.
pub fn scan_repo(root: &Path) -> anyhow::Result<Vec<ScannedFile>> {
    if !root.is_dir() {
        anyhow::bail!("repository path is not a directory: {}", root.display());
    }
    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<ScannedFile>) -> anyhow::Result<()> {
    let entries =
        std::fs::read_dir(dir).with_context(|| format!("read directory {}", dir.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("read directory entry in {}", dir.display()))?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if SKIP_DIRS.contains(&name.as_ref()) || name.starts_with('.') {
                continue;
            }
            walk(root, &path, out)?;
        } else if let Some(lang) = lang_for_path(&path) {
            let meta = entry
                .metadata()
                .with_context(|| format!("read metadata of {}", path.display()))?;
            let rel_path = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push(ScannedFile {
                rel_path,
                abs_path: path,
                mtime: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
                size: meta.len(),
                lang,
            });
        }
    }
    Ok(())
}
