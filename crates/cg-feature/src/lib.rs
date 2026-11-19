//! Git-backed feature review storage: one independent git document repository
//! per source repo, keyed by `repo_id = sha256(canonical git_url)`.
//!
//! Layout: `<root>/<repo_id>/` is a git repo; `main` is the merged stable
//! state, `mr/<name>` branches are drafts, `_changelog/` records approvals.
//! No database rows, no version metadata; lifecycle state is pure git.

pub mod git;
pub mod git_url;
pub mod lock;
pub mod model;
pub mod ops;
pub mod paths;

pub use git_url::{canonical_git_url, feature_repo_id, repo_id_from_git_url, sha256_hex};
pub use model::{
    mr_branch, mr_name_from_branch, validate_feature_name, MAX_FEATURE_NAME_BYTES,
};
pub use ops::draft::{docs_prefix, DOCS_DIR};
pub use paths::{
    changelog_file_name, repo_dir, resolve_feature_root, validate_rel_path, validate_repo_id,
    CHANGELOG_DIR,
};
