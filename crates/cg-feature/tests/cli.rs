//! End-to-end CLI checks through the built `cg-feature` binary.

use std::path::Path;
use std::process::{Command, Output};

const GIT_URL: &str = "https://github.com/owner/repo.git";

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cg-feature"))
        .arg("--feature-root")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap()
}

fn ok(root: &Path, args: &[&str]) -> String {
    let out = run(root, args);
    assert!(out.status.success(), "args {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

fn source(name: &str) -> Vec<u8> {
    serde_json::json!({
        "name": name,
        "function_entry": "main",
        "request_chain": [],
        "external_dependencies": [],
        "function_summary": "",
        "children": []
    })
    .to_string()
    .into_bytes()
}

#[test]
fn repo_id_command_normalizes_url_spellings() {
    let dir = tempfile::tempdir().unwrap();
    let a = ok(dir.path(), &["repo-id", GIT_URL]);
    let b = ok(dir.path(), &["repo-id", "git@github.com:owner/repo.git"]);
    assert_eq!(a.trim(), b.trim());
    assert_eq!(a.trim().len(), 64);
    let bad = run(dir.path(), &["repo-id", "github.com"]);
    assert!(!bad.status.success());
}

#[test]
fn draft_list_diff_approve_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let src = dir.path().join("src.json");
    std::fs::write(&src, source("CLI 功能")).unwrap();

    let draft = ok(root, &["draft", GIT_URL, "--expected-feature-name=CLI 功能", "--", src.to_str().unwrap()]);
    assert!(draft.starts_with("DRAFT "), "{draft}");
    let again = ok(root, &["draft", GIT_URL, "--expected-feature-name=CLI 功能", "--", src.to_str().unwrap()]);
    assert_eq!(again.trim(), "NOOP");

    let list = ok(root, &["list", GIT_URL]);
    assert!(list.contains("CLI 功能\tdraft"), "{list}");

    let diff = ok(root, &["diff", GIT_URL]);
    assert!(diff.contains("+  \"name\": \"CLI 功能\""), "{diff}");

    let approve = ok(root, &["approve", GIT_URL, "CLI 功能", "--reviewer", "cli-bot", "--note", "ship it"]);
    assert!(approve.starts_with("APPROVED "), "{approve}");

    let status = ok(root, &["status", GIT_URL]);
    assert!(status.contains("\"stable\""), "{status}");
    assert!(status.contains("CLI 功能"), "{status}");
}

#[test]
fn expected_name_mismatch_fails() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src.json");
    std::fs::write(&src, source("actual")).unwrap();
    let out = run(
        dir.path(),
        &["draft", GIT_URL, "--expected-feature-name=expected", "--", src.to_str().unwrap()],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("expected feature name"));
}
