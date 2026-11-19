//! CLI contract: repo-id / draft / mrs / tree / cat / approve / changelog.

use std::path::PathBuf;
use std::process::{Command, Output};
use tempfile::TempDir;

const URL: &str = "https://github.com/owner/repo.git";

fn bin() -> PathBuf {
    env!("CARGO_BIN_EXE_cg-feature").into()
}

fn src_arg(root: &TempDir) -> String {
    root.path().join("src").display().to_string()
}

fn run(root: &TempDir, args: &[&str]) -> Output {
    Command::new(bin())
        .arg(format!("--feature-root={}", root.path().display()))
        .args(args)
        .env_remove("CODE_GRAPHER_DATA_DIR")
        .env_remove("XDG_DATA_HOME")
        .output()
        .expect("run cg-feature")
}

fn write_source(dir: &TempDir, rel: &str, content: &str) {
    let path = dir.path().join("src").join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

#[test]
fn repo_id_stable() {
    let dir = TempDir::new().unwrap();
    let out = run(&dir, &["repo-id", URL]);
    assert!(out.status.success());
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(id.len(), 64);
}

#[test]
fn draft_mrs_tree_cat_approve_changelog_roundtrip() {
    let dir = TempDir::new().unwrap();
    write_source(&dir, "overview.md", "# 概览\n");
    write_source(&dir, "ast/index.json", "{}\n");

    let out = run(&dir, &["draft", URL, "CLI功能", &src_arg(&dir)]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("DRAFT "));

    // identical refresh is NOOP
    let out = run(&dir, &["draft", URL, "CLI功能", &src_arg(&dir)]);
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "NOOP");

    let out = run(&dir, &["mrs", URL]);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("CLI功能") && text.contains("open"), "{text}");

    let out = run(&dir, &["tree", URL, "--mr", "CLI功能"]);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("docs/CLI功能/overview.md\tdraft"), "{text}");
    assert!(text.contains("docs/CLI功能/ast/index.json"), "{text}");

    let out = run(&dir, &["cat", URL, "docs/CLI功能/overview.md", "--mr", "CLI功能"]);
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "# 概览\n");

    let out = run(&dir, &["diff", URL, "CLI功能"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("overview.md"));

    let out = run(&dir, &["approve", URL, "CLI功能", "--reviewer", "alice"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("MERGED "));

    // main tree now stable; cat without --mr works
    let out = run(&dir, &["cat", URL, "docs/CLI功能/overview.md"]);
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "# 概览\n");

    let out = run(&dir, &["changelog", URL]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("alice"));
}

#[test]
fn expected_feature_name_guard_fails() {
    let dir = TempDir::new().unwrap();
    write_source(&dir, "a.md", "x\n");
    let out = run(&dir, &["draft", URL, "name-a", &src_arg(&dir), "--expected-feature-name=name-b"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("mismatch"));
}

#[test]
fn cat_missing_path_fails() {
    let dir = TempDir::new().unwrap();
    write_source(&dir, "a.md", "x\n");
    run(&dir, &["draft", URL, "m", &src_arg(&dir)]);
    let out = run(&dir, &["cat", URL, "docs/m/missing.md"]);
    assert!(!out.status.success());
}
