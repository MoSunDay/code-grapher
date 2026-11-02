//! cg-indexer: repository scanning, incremental indexing and file watching.

pub mod index;
pub mod scan;
pub mod watcher;

pub use index::{index_paths, index_repo, IndexOutcome, IndexProgress};
pub use watcher::spawn_watcher;
