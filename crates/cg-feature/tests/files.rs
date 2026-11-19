//! Tree/blob contract: render hints, draft badges, size cap, traversal.

use cg_feature::ops::{self, DraftFile};
use cg_feature::repo_id_from_git_url;
use tempfile::TempDir;

const URL: &str = "https://github.com/owner/repo.git";

fn draft(root: &std::path::Path, repo_id: &str, entries: &[(&str, &str)]) {
    let files: Vec<DraftFile> = entries
        .iter()
        .map(|(path, content)| DraftFile { path: path.to_string(), content: content.to_string() })
        .collect();
    ops::draft(root, repo_id, "f", &files, None).unwrap();
}

#[test]
fn tree_carries_render_hints_and_badges() {
    let dir = TempDir::new().unwrap();
    let repo_id = repo_id_from_git_url(URL).unwrap();
    let root = dir.path();
    draft(
        root,
        &repo_id,
        &[
            ("docs/f/README.md", "# f\n"),
            ("docs/f/data.json", "[1]\n"),
            ("docs/f/raw.txt", "x\n"),
        ],
    );
    let tree = ops::tree(root, &repo_id, Some("f")).unwrap();
    let by = |p: &str| tree.iter().find(|f| f.path == p).unwrap().clone();
    assert_eq!(by("docs/f/README.md").render, cg_feature::model::RenderHint::Markdown);
    assert_eq!(by("docs/f/data.json").render, cg_feature::model::RenderHint::Json);
    assert_eq!(by("docs/f/raw.txt").render, cg_feature::model::RenderHint::Text);
    assert_eq!(by("docs/f/README.md").state, ops::FileBadge::Draft);

    // main tree is empty of these files (nothing merged yet)
    let main_tree = ops::tree(root, &repo_id, None).unwrap();
    assert!(main_tree.iter().all(|f| !f.path.starts_with("docs/f/")));
}

#[test]
fn blob_reads_branch_and_main_and_enforces_cap() {
    let dir = TempDir::new().unwrap();
    let repo_id = repo_id_from_git_url(URL).unwrap();
    let root = dir.path();
    let big = "x".repeat(ops::MAX_BLOB_BYTES + 100);
    draft(root, &repo_id, &[("docs/f/big.md", &big), ("docs/f/small.md", "ok")]);
    let blob = ops::blob(root, &repo_id, Some("f"), "docs/f/small.md").unwrap();
    assert_eq!(blob.content, "ok");
    assert!(!blob.truncated);
    let big_blob = ops::blob(root, &repo_id, Some("f"), "docs/f/big.md").unwrap();
    assert!(big_blob.truncated);
    assert_eq!(big_blob.size as usize, ops::MAX_BLOB_BYTES + 100);
    assert!(big_blob.content.len() <= ops::MAX_BLOB_BYTES);

    ops::approve(
        root,
        &repo_id,
        "f",
        &ops::ApproveParams { reviewer: "r", note: None, expected_tip: None },
    )
    .unwrap();
    let main_blob = ops::blob(root, &repo_id, None, "docs/f/small.md").unwrap();
    assert_eq!(main_blob.content, "ok");
}

#[test]
fn blob_rejects_traversal_and_reserved_paths() {
    let dir = TempDir::new().unwrap();
    let repo_id = repo_id_from_git_url(URL).unwrap();
    let root = dir.path();
    draft(root, &repo_id, &[("docs/f/a.md", "1")]);
    for bad in ["../x", "/abs", ".git/config", "a/../../b", "docs/f/\u{0}x"] {
        assert!(ops::blob(root, &repo_id, None, bad).is_err(), "{bad}");
    }
    // changelog readable after merge
    ops::approve(
        root,
        &repo_id,
        "f",
        &ops::ApproveParams { reviewer: "r", note: None, expected_tip: None },
    )
    .unwrap();
    let log = ops::changelog(root, &repo_id).unwrap();
    let file = log[0].file.clone();
    assert!(ops::blob(root, &repo_id, None, &file).is_ok());
}

#[test]
fn missing_repo_yields_empty_reads() {
    let dir = TempDir::new().unwrap();
    let repo_id = repo_id_from_git_url(URL).unwrap();
    let root = dir.path();
    assert!(ops::mrs(root, &repo_id).unwrap().is_empty());
    assert!(ops::changelog(root, &repo_id).unwrap().is_empty());
    assert!(ops::tree(root, &repo_id, None).is_err());
}
