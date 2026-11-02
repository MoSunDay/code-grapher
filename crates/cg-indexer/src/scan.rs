//! Repository scanning: collect indexable source files.

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
pub fn scan_repo(root: &Path) -> anyhow::Result<Vec<ScannedFile>> {
    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<ScannedFile>) -> anyhow::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(dir = %dir.display(), error = %e, "unreadable dir");
            return Ok(());
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if SKIP_DIRS.contains(&name.as_ref()) || name.starts_with('.') {
                continue;
            }
            walk(root, &path, out)?;
        } else if let Some(lang) = lang_for_path(&path) {
            let meta = entry.metadata()?;
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

