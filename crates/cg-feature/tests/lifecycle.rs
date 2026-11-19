//! Branch-model lifecycle: draft on `mr/<name>` inside `docs/<name>/`,
//! approve = merge into main.

use cg_feature::ops::{self, DraftFile, DraftOutcome};
use cg_feature::repo_id_from_git_url;
use tempfile::TempDir;

const URL: &str = "https://github.com/owner/repo.git";

fn files(entries: &[(&str, &str)]) -> Vec<DraftFile> {
    entries
        .iter()
        .map(|(path, content)| DraftFile {
            path: path.to_string(),
            content: content.to_string(),
        })
        .collect()
}

fn setup() -> (TempDir, String) {
    let dir = TempDir::new().unwrap();
    let repo_id = repo_id_from_git_url(URL).unwrap();
    (dir, repo_id)
}

const NAME: &str = "AST解析与领域模型";

#[test]
fn draft_creates_open_mr_then_approve_merges() {
    let (dir, repo_id) = setup();
    let root = dir.path();

    let outcome = ops::draft(
        root,
        &repo_id,
        NAME,
        &files(&[
            ("docs/AST解析与领域模型/overview.md", "# 概览\n"),
            ("docs/AST解析与领域模型/ast/index.json", "{\"nodes\":1}\n"),
        ]),
        None,
    )
    .unwrap();
    let DraftOutcome::Updated { commit } = outcome else {
        panic!("expected Updated");
    };
    assert!(!commit.is_empty());

    let mrs = ops::mrs(root, &repo_id).unwrap();
    assert_eq!(mrs.len(), 1);
    assert_eq!(mrs[0].name, NAME);
    assert_eq!(mrs[0].state, ops::MrState::Open);
    assert_eq!(mrs[0].ahead, 1);
    assert!(mrs[0].reviewer.is_none());

    let diff = ops::diff(root, &repo_id, NAME).unwrap();
    assert!(diff.contains("docs/AST解析与领域模型/overview.md"));

    let outcome = ops::approve(
        root,
        &repo_id,
        NAME,
        &ops::ApproveParams { reviewer: "alice", note: Some("looks good"), expected_tip: None },
    )
    .unwrap();
    assert!(!outcome.merge_commit.is_empty());
    assert!(outcome.changelog.starts_with("_changelog/"));

    let mrs = ops::mrs(root, &repo_id).unwrap();
    assert_eq!(mrs[0].state, ops::MrState::Merged);
    assert_eq!(mrs[0].reviewer.as_deref(), Some("alice"));
    assert_eq!(mrs[0].ahead, 0, "merged branch is contained in main");
    assert_eq!(mrs[0].changelog.as_deref(), Some(outcome.changelog.as_str()));

    // main tree now holds the files with stable badges
    let tree = ops::tree(root, &repo_id, None).unwrap();
    assert!(tree
        .iter()
        .any(|f| f.path == "docs/AST解析与领域模型/overview.md" && f.state == ops::FileBadge::Stable));

    // changelog entry exists on main
    let log = ops::changelog(root, &repo_id).unwrap();
    assert_eq!(log.len(), 1);
    assert!(log[0].markdown.contains("alice"));

    // double approve is rejected
    let err = ops::approve(
        root,
        &repo_id,
        NAME,
        &ops::ApproveParams { reviewer: "bob", note: None, expected_tip: None },
    )
    .unwrap_err();
    assert!(err.to_string().contains("already merged"));
}

#[test]
fn identical_refresh_is_noop_and_changes_commit() {
    let (dir, repo_id) = setup();
    let root = dir.path();
    let same = files(&[("docs/feature-x/a.md", "same\n")]);
    ops::draft(root, &repo_id, "feature-x", &same, None).unwrap();
    assert_eq!(
        ops::draft(root, &repo_id, "feature-x", &same, None).unwrap(),
        DraftOutcome::Noop
    );
    let changed = files(&[("docs/feature-x/a.md", "different\n")]);
    let DraftOutcome::Updated { commit } =
        ops::draft(root, &repo_id, "feature-x", &changed, None).unwrap()
    else {
        panic!("expected Updated");
    };
    let mrs = ops::mrs(root, &repo_id).unwrap();
    assert_eq!(mrs[0].tip, commit);
    assert_eq!(mrs[0].ahead, 2, "one initial + one refresh commit");
}

#[test]
fn refresh_absorbs_main_but_never_deletes_other_mrs_docs() {
    let (dir, repo_id) = setup();
    let root = dir.path();
    ops::draft(root, &repo_id, "one", &files(&[("docs/one/a.md", "1\n")]), None).unwrap();
    ops::draft(
        root,
        &repo_id,
        "two",
        &files(&[("docs/two/b.md", "2\n"), ("docs/two/old.md", "x\n")]),
        None,
    )
    .unwrap();
    ops::approve(
        root,
        &repo_id,
        "one",
        &ops::ApproveParams { reviewer: "r1", note: None, expected_tip: None },
    )
    .unwrap();

    // refresh "two" without old.md
    ops::draft(root, &repo_id, "two", &files(&[("docs/two/b.md", "2\n")]), None).unwrap();
    let tree = ops::tree(root, &repo_id, Some("two")).unwrap();
    assert!(tree.iter().any(|f| f.path == "docs/one/a.md"), "branch absorbed main");
    assert!(
        tree.iter().any(|f| f.path == "docs/two/b.md" && f.state == ops::FileBadge::Draft),
        "own changed file is draft"
    );
    assert!(
        tree.iter().any(|f| f.path == "docs/one/a.md" && f.state == ops::FileBadge::Stable),
        "other MR's file is stable"
    );
    assert!(!tree.iter().any(|f| f.path == "docs/two/old.md"), "stale file removed");

    // approving "two" merges cleanly and keeps "one"'s docs on main
    ops::approve(
        root,
        &repo_id,
        "two",
        &ops::ApproveParams { reviewer: "r2", note: None, expected_tip: None },
    )
    .unwrap();
    let tree = ops::tree(root, &repo_id, None).unwrap();
    assert!(tree.iter().any(|f| f.path == "docs/one/a.md"), "other MR docs survive");
    assert!(tree.iter().any(|f| f.path == "docs/two/b.md"));
    let changelog = ops::changelog(root, &repo_id).unwrap();
    assert_eq!(changelog.len(), 2);
    assert!(changelog.iter().any(|e| e.file.contains("-two-")));
}

#[test]
fn draft_paths_outside_own_namespace_are_rejected() {
    let (dir, repo_id) = setup();
    let root = dir.path();
    let err = ops::draft(
        root,
        &repo_id,
        "mine",
        &files(&[("docs/other/file.md", "x\n")]),
        None,
    )
    .unwrap_err();
    assert!(err.to_string().contains("own namespace"), "{err}");
    let err = ops::draft(
        root,
        &repo_id,
        "mine",
        &files(&[("_changelog/x.md", "x\n")]),
        None,
    )
    .unwrap_err();
    assert!(err.to_string().contains("namespace") || err.to_string().contains("_changelog"));
}

#[test]
fn expected_tip_and_expected_name_guards() {
    let (dir, repo_id) = setup();
    let root = dir.path();
    ops::draft(root, &repo_id, "guard", &files(&[("docs/guard/g.md", "v1\n")]), None).unwrap();
    let err = ops::draft(
        root,
        &repo_id,
        "guard",
        &files(&[("docs/guard/g.md", "v2\n")]),
        Some("other"),
    )
    .unwrap_err();
    assert!(err.to_string().contains("mismatch"));

    let err = ops::approve(
        root,
        &repo_id,
        "guard",
        &ops::ApproveParams { reviewer: "r", note: None, expected_tip: Some("deadbeef") },
    )
    .unwrap_err();
    assert!(err.to_string().contains("tip changed"), "{err}");
    assert_eq!(ops::mrs(root, &repo_id).unwrap().len(), 1);
}

#[test]
fn concurrent_drafts_on_distinct_mrs_do_not_conflict() {
    let (dir, repo_id) = setup();
    let root = dir.path();
    for name in ["alpha", "beta"] {
        ops::draft(
            root,
            &repo_id,
            name,
            &files(&[(format!("docs/{name}/f.md").as_str(), "x\n")]),
            None,
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    let mrs = ops::mrs(root, &repo_id).unwrap();
    assert_eq!(mrs.len(), 2);
    assert!(mrs.iter().all(|m| m.state == ops::MrState::Open));
    assert!(mrs.iter().all(|m| m.behind == 0 && m.ahead == 1));
}

#[test]
fn commits_endpoint_spans_open_and_merged() {
    let (dir, repo_id) = setup();
    let root = dir.path();
    ops::draft(root, &repo_id, "c", &files(&[("docs/c/a.md", "1\n")]), None).unwrap();
    ops::draft(root, &repo_id, "c", &files(&[("docs/c/a.md", "2\n")]), None).unwrap();
    let open = ops::commits(root, &repo_id, "c").unwrap();
    assert_eq!(open.len(), 2);
    assert!(open[0].subject.contains("draft"));
    ops::approve(
        root,
        &repo_id,
        "c",
        &ops::ApproveParams { reviewer: "r", note: None, expected_tip: None },
    )
    .unwrap();
    let merged = ops::commits(root, &repo_id, "c").unwrap();
    assert_eq!(merged.len(), 2, "merged MR still lists its branch commits");
}

#[test]
fn reapprove_after_new_draft_reopens_mr() {
    let (dir, repo_id) = setup();
    let root = dir.path();
    ops::draft(root, &repo_id, "loop", &files(&[("docs/loop/a.md", "1\n")]), None).unwrap();
    ops::approve(
        root,
        &repo_id,
        "loop",
        &ops::ApproveParams { reviewer: "r", note: None, expected_tip: None },
    )
    .unwrap();
    assert_eq!(ops::mrs(root, &repo_id).unwrap()[0].state, ops::MrState::Merged);
    ops::draft(root, &repo_id, "loop", &files(&[("docs/loop/a.md", "2\n")]), None).unwrap();
    assert_eq!(ops::mrs(root, &repo_id).unwrap()[0].state, ops::MrState::Open);
    ops::approve(
        root,
        &repo_id,
        "loop",
        &ops::ApproveParams { reviewer: "r", note: None, expected_tip: None },
    )
    .unwrap();
    assert_eq!(ops::changelog(root, &repo_id).unwrap().len(), 2);
    assert!(ops::diff(root, &repo_id, "loop").unwrap().is_empty(), "merged == main");
}
