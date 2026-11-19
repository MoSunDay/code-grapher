//! Repo identity rules: canonical git_url -> repo_id.

use cg_feature::{canonical_git_url, feature_repo_id, repo_id_from_git_url};

const HTTPS: &str = "https://github.com/owner/repo.git";
const SSH: &str = "ssh://git@github.com/owner/repo.git";
const SCP: &str = "git@github.com:owner/repo.git";

#[test]
fn three_spellings_share_one_repo_id() {
    let expected = repo_id_from_git_url(HTTPS).unwrap();
    assert_eq!(expected.len(), 64);
    assert!(expected.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_eq!(repo_id_from_git_url(SSH).unwrap(), expected);
    assert_eq!(repo_id_from_git_url(SCP).unwrap(), expected);
}

#[test]
fn trailing_slash_and_git_suffix_do_not_matter() {
    let base = repo_id_from_git_url("https://github.com/o/r").unwrap();
    for url in [
        "https://github.com/o/r/",
        "https://github.com/o/r.git",
        "https://github.com/o/r.git/",
        "git@github.com:o/r.git",
    ] {
        assert_eq!(repo_id_from_git_url(url).unwrap(), base, "{url}");
    }
}

#[test]
fn host_casing_ignored_path_casing_significant() {
    assert_eq!(
        repo_id_from_git_url("https://GITHUB.COM/o/r").unwrap(),
        repo_id_from_git_url("https://github.com/o/r").unwrap()
    );
    assert_ne!(
        repo_id_from_git_url("https://github.com/o/R").unwrap(),
        repo_id_from_git_url("https://github.com/o/r").unwrap()
    );
}

#[test]
fn userinfo_never_reaches_identity() {
    assert_eq!(
        canonical_git_url("https://alice:secret@github.com/o/r.git").unwrap(),
        "github.com/o/r"
    );
    assert_eq!(
        repo_id_from_git_url("https://alice:secret@github.com/o/r.git").unwrap(),
        repo_id_from_git_url("https://github.com/o/r.git").unwrap()
    );
    let id = feature_repo_id(Some("https://alice:secret@github.com/o/r.git")).unwrap();
    assert!(!id.contains("alice") && !id.contains("secret"));
}

#[test]
fn query_and_fragment_are_ignored() {
    assert_eq!(
        repo_id_from_git_url("https://github.com/o/r.git?token=x#section").unwrap(),
        repo_id_from_git_url("https://github.com/o/r").unwrap()
    );
}

#[test]
fn invalid_urls_error_and_missing_disables_feature() {
    for url in ["", "github.com", "https://", "file:///only/local", "://missing"] {
        assert!(repo_id_from_git_url(url).is_err(), "{url}");
    }
    assert_eq!(feature_repo_id(None), None);
    assert_eq!(feature_repo_id(Some("not a repository at all!"),), None);
}

#[test]
fn different_urls_produce_different_directories() {
    let a = repo_id_from_git_url("https://github.com/a/one").unwrap();
    let b = repo_id_from_git_url("https://gitlab.com/a/one").unwrap();
    let c = repo_id_from_git_url("https://github.com/a/two").unwrap();
    assert_ne!(a, b);
    assert_ne!(a, c);
}
