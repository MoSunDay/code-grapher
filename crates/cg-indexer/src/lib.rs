//! cg-indexer: repository scanning, incremental indexing and file watching.

pub mod clone;
pub mod index;
pub mod scan;
pub mod watcher;

pub use clone::{derive_clone_dir, run_clone, sanitize_repo_name};
pub use index::{index_paths, index_repo, IndexOutcome, IndexProgress};
pub use watcher::{is_watch_relevant, needs_repo_scan, spawn_watcher, WatchEvent};
