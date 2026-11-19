//! Git-backed feature review storage.
//!
//! Identity: `repo_id = sha256(canonical_git_url)`; a feature's `name` is its
//! sole identity and its directory name. Lifecycle state lives only in git:
//! uncommitted `feature.json` changes are a draft, an approved commit is the
//! stable state. No database rows, index files, or version directories.

pub mod git;
pub mod git_url;
pub mod model;
pub mod ops;
pub mod paths;

pub use git_url::{canonical_git_url, feature_repo_id, repo_id_from_git_url, sha256_hex};
pub use model::{
    canonical_feature_bytes, parse_feature, validate_feature_name, Feature, MAX_FEATURE_NAME_BYTES,
};
pub use paths::{
    changelog_dir, changelog_file_name, feature_json_path, repo_dir, resolve_feature_root,
    validate_repo_id, CHANGELOG_DIR, FEATURE_JSON,
};
