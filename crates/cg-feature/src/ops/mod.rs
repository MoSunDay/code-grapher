//! Feature review operations over the per-repository document git repo.
//!
//! Split by responsibility: `draft` (write path), `approve` (merge path),
//! `read` (lock-free query path). Mutating operations take the per-repo
//! `flock` first; every repository partition has exactly one writer (the
//! cg-server instance owning the feature root).

pub mod approve;
pub mod draft;
pub mod read;

pub use approve::{approve, ApproveOutcome, ApproveParams};
pub use draft::{draft, DraftFile, DraftOutcome};
pub use crate::git::CommitInfo;
pub use read::{
    blob, changelog, commits, diff, mrs, tree, Blob, ChangelogEntry, FileBadge, MrState,
    MrSummary, TreeFile, MAX_BLOB_BYTES,
};
