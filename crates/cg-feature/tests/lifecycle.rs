//! Git-backed lifecycle: draft -> diff -> approve -> stable.

use cg_feature::ops::{self, ApproveParams, DraftOutcome, FeatureState};
use cg_feature::repo_id_from_git_url;
use std::path::Path;

const GIT_URL: &str = "https://github.com/owner/repo.git";

fn feature_json(name: &str, summary: &str) -> Vec<u8> {
    serde_json::json!({
        "name": name,
        "function_entry": "cg_core::parse",
        "request_chain": [],
        "external_dependencies": [],
        "function_summary": summary,
        "children": []
    })
    .to_string()
    .into_bytes()
}

fn setup(name: &str) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let repo_id = repo_id_from_git_url(GIT_URL).unwrap();
    ops::draft(dir.path(), &repo_id, &feature_json(name, "first"), None).unwrap();
    (dir, repo_id)
}

#[test]
fn draft_layout_has_no_version_dirs_or_index() {
    let (dir, repo_id) = setup("AST 解析与领域模型");
    let root = dir.path();
    let json = root.join(&repo_id).join("AST 解析与领域模型").join("feature.json");
    assert!(json.is_file());
    // draft stage: only the feature directory; _changelog appears on approve.
    let names = collect_dirs(&root.join(&repo_id));
    assert_eq!(names, vec!["AST 解析与领域模型"]);
    assert!(!root.join(&repo_id).join("index.json").exists());
    assert!(!root.join(&repo_id).join("features").exists());
    for entry in walk(&root.join(&repo_id)) {
        let name = entry.file_name().to_string_lossy().into_owned();
        assert!(!name.starts_with("draft."), "{name}");
        assert!(!name.starts_with("verified"), "{name}");
        assert!(!name.starts_with('v') || !name[1..].chars().all(|c| c.is_ascii_digit()), "{name}");
    }
}

#[test]
fn uncommitted_is_draft_approved_is_stable() {
    let (dir, repo_id) = setup("请求链路推断");
    let root = dir.path();
    assert_eq!(ops::list(root, &repo_id).unwrap()[0].state, FeatureState::Draft);
    assert!(ops::diff(root, &repo_id, Some("请求链路推断")).unwrap().contains("请求链路推断"));

    let outcome = ops::approve(
        root,
        &repo_id,
        "请求链路推断",
        &ApproveParams { reviewer: "alice", note: Some("lgtm"), expected_hash: None },
    )
    .unwrap();
    assert!(!outcome.commit.is_empty());
    assert!(outcome.changelog.contains("_changelog/"));
    assert!(outcome.changelog.contains("请求链路推断"));

    assert_eq!(ops::list(root, &repo_id).unwrap()[0].state, FeatureState::Stable);
    assert!(ops::diff(root, &repo_id, Some("请求链路推断")).unwrap().is_empty());

    let changelog = ops::read_changelog(root, &repo_id).unwrap();
    assert_eq!(changelog.len(), 1);
    assert!(changelog[0].markdown.contains("alice"));
    assert!(changelog[0].markdown.contains("lgtm"));
}

#[test]
fn same_name_modified_is_a_new_draft_not_a_new_directory() {
    let (dir, repo_id) = setup("AST");
    let root = dir.path();
    let second = ops::draft(root, &repo_id, &feature_json("AST", "second"), None).unwrap();
    assert!(matches!(second, DraftOutcome::Created { .. }));
    let identical = ops::draft(root, &repo_id, &feature_json("AST", "second"), None).unwrap();
    assert_eq!(identical, DraftOutcome::Noop);
    assert_eq!(ops::list(root, &repo_id).unwrap().len(), 1, "same name = same feature");
    assert_eq!(ops::list(root, &repo_id).unwrap()[0].state, FeatureState::Draft);
}

#[test]
fn different_names_are_different_features() {
    let (dir, repo_id) = setup("alpha");
    let root = dir.path();
    ops::draft(root, &repo_id, &feature_json("beta", "b"), None).unwrap();
    let names: Vec<String> =
        ops::list(root, &repo_id).unwrap().into_iter().map(|f| f.name).collect();
    assert_eq!(names, vec!["alpha", "beta"]);
}

#[test]
fn approve_commits_only_the_reviewed_paths() {
    let (dir, repo_id) = setup("clean-feature");
    let root = dir.path();
    ops::draft(root, &repo_id, &feature_json("dirty-feature", "d"), None).unwrap();
    std::fs::write(root.join(&repo_id).join("unrelated.txt"), b"stray").unwrap();

    ops::approve(
        root,
        &repo_id,
        "clean-feature",
        &ApproveParams { reviewer: "bob", note: None, expected_hash: None },
    )
    .unwrap();

    let committed = git_ls_files(root);
    assert!(committed.contains(&format!("{repo_id}/clean-feature/feature.json")));
    assert!(committed.iter().any(|p| p.contains("_changelog/") && p.contains("clean-feature")));
    assert!(!committed.contains(&format!("{repo_id}/unrelated.txt")));
    assert!(!committed.contains(&format!("{repo_id}/dirty-feature/feature.json")));
    assert!(root.join(&repo_id).join("unrelated.txt").exists(), "stray file untouched");
}

#[test]
fn approve_guards_hash_and_clean_state() {
    let (dir, repo_id) = setup("guarded");
    let root = dir.path();
    let err = ops::approve(
        root,
        &repo_id,
        "guarded",
        &ApproveParams { reviewer: "a", note: None, expected_hash: Some("deadbeef") },
    )
    .unwrap_err();
    assert!(err.to_string().contains("hash"), "{err}");

    ops::approve(root, &repo_id, "guarded", &ApproveParams::default()).unwrap_err();
    ops::approve(
        root,
        &repo_id,
        "guarded",
        &ApproveParams { reviewer: "a", note: None, expected_hash: None },
    )
    .unwrap();
    let again = ops::approve(
        root,
        &repo_id,
        "guarded",
        &ApproveParams { reviewer: "a", note: None, expected_hash: None },
    )
    .unwrap_err();
    assert!(again.to_string().contains("stable"), "{again}");
}

#[test]
fn expected_name_mismatch_fails_before_writing() {
    let (dir, repo_id) = setup("real-name");
    let err = ops::draft(
        dir.path(),
        &repo_id,
        &feature_json("real-name", "v2"),
        Some("other-name"),
    )
    .unwrap_err();
    assert!(err.to_string().contains("expected feature name"), "{err}");
}

#[test]
fn all_features_diff_covers_each_dirty_feature() {
    let (dir, repo_id) = setup("one");
    let root = dir.path();
    ops::draft(root, &repo_id, &feature_json("two", "t"), None).unwrap();
    let diff = ops::diff(root, &repo_id, None).unwrap();
    assert!(diff.contains("one/feature.json"), "{diff}");
    assert!(diff.contains("two/feature.json"), "{diff}");
}

fn collect_dirs(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn walk(path: &Path) -> Vec<std::fs::DirEntry> {
    let mut out = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                stack.push(entry.path());
            }
            out.push(entry);
        }
    }
    out
}

fn git_ls_files(root: &Path) -> Vec<String> {
    let out = std::process::Command::new("git")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .current_dir(root)
        .args(["ls-files"])
        .output()
        .unwrap();
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn relative_feature_root_resolves_without_path_doubling() {
    let dir = tempfile::tempdir().unwrap();
    let repo_id = repo_id_from_git_url(GIT_URL).unwrap();
    let prev = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();
    let drafted = ops::draft(
        Path::new("feat-root"),
        &repo_id,
        &feature_json("相对根功能", "v1"),
        None,
    );
    let listing = ops::list(Path::new("feat-root"), &repo_id);
    std::env::set_current_dir(prev).unwrap();
    drafted.unwrap();
    let root = dir.path().join("feat-root");
    assert!(root.join(&repo_id).join("相对根功能").join("feature.json").is_file());
    assert!(!root.join("feat-root").exists(), "relative root must not double");
    assert_eq!(listing.unwrap().len(), 1);
}
