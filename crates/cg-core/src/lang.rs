//! Language registry: maps file extensions to parsers.
//! Rust is the first-class citizen; more languages slot in here.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Rust,
}

impl Lang {
    pub fn as_str(self) -> &'static str {
        match Lang::Rust {
            Lang::Rust => "rust",
        }
    }

    /// File extensions handled by this language.
    pub fn extensions(self) -> &'static [&'static str] {
        match Lang::Rust {
            Lang::Rust => &["rs"],
        }
    }
}

/// Resolve a language from a file path (by extension). None = not indexed.
pub fn lang_for_path(path: &Path) -> Option<Lang> {
    let ext = path.extension()?.to_str()?;
    if Lang::Rust.extensions().contains(&ext) {
        Some(Lang::Rust)
    } else {
        None
    }
}

/// Directory names skipped while scanning any repository.
pub const SKIP_DIRS: &[&str] = &[
    ".git", "target", "node_modules", "dist", "build", "out", ".venv", "venv", "__pycache__",
    ".idea", ".vscode", ".next", ".turbo", "coverage", ".cache",
];
