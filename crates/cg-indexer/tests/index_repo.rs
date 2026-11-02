//! Index a fixture repo on disk; verify db contents + incremental behaviour.

use cg_core::model::{EdgeKind, SymbolKind};
use cg_db::{edges, files, repos, symbols};

#[tokio::test]
async fn indexes_and_increments() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("lib.rs"),
        "pub fn a() { b(); }\npub fn b() -> i32 { 7 }\n",
    )
    .unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/nested.rs"),
        "pub struct Cfg { flag: bool }\nimpl Cfg {\n    fn on(&self) {}\n    pub fn flip(&mut self) { self.on(); self.flag = !self.flag; }\n}\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("target/debug")).unwrap();
    std::fs::write(dir.path().join("target/debug/ignored.rs"), "fn skip_me() {}\n").unwrap();
    std::fs::write(dir.path().join("notes.txt"), "not code").unwrap();

    let db = cg_db::open_local(&dir.path().join("t.db")).await.unwrap();
    cg_db::migrate(&db).await.unwrap();
    let repo = repos::upsert(&db, dir.path().to_str().unwrap(), "fx", None).await.unwrap();

    let mut progress = |_p: cg_indexer::IndexProgress| {};
    let out = cg_indexer::index_repo(&db, &repo, false, "test", progress).await.unwrap();
    assert_eq!(out.files_scanned, 2, "target/ and .txt skipped");
    assert_eq!(out.files_changed, 2);
    assert_eq!(out.files_removed, 0);

    let list = files::list(&db, repo.id).await.unwrap();
    assert_eq!(list.len(), 2);
    assert!(list.iter().all(|f| f.lang.as_deref() == Some("rust")));

    // direct call a->b resolved via unique name
    edges::resolve_by_name(&db, repo.id).await.unwrap();
    let (total, fns) = symbols::list(&db, repo.id, &symbols::SymbolFilter {
        kind: Some(SymbolKind::Function), q: None, file_id: None, limit: 10, offset: 0 }).await.unwrap();
    assert_eq!(total, 2, "a, b (flip is a method)");
    let a = fns.iter().find(|s| s.name == "a").unwrap();
    let b_id = fns.iter().find(|s| s.name == "b").map(|s| s.id);
    let callees = edges::for_caller(&db, a.id).await.unwrap();
    let direct = callees.iter().find(|e| e.kind == EdgeKind::DirectFn).unwrap();
    assert_eq!(direct.callee_id, b_id);
    // method call recorded as dynamic
    let (_, methods) = symbols::list(&db, repo.id, &symbols::SymbolFilter {
        kind: Some(SymbolKind::Method), q: None, file_id: None, limit: 10, offset: 0 }).await.unwrap();
    let flip = methods.iter().find(|s| s.name == "flip").unwrap();
    let flip_callees = edges::for_caller(&db, flip.id).await.unwrap();
    assert!(flip_callees.iter().any(|e| e.kind == EdgeKind::DynamicMethod && e.callee_name == "on"));

    // tasks auto-enqueued for function-likes
    let tsks = cg_db::tasks::list(&db, repo.id, None, 100).await.unwrap();
    assert_eq!(tsks.len(), 4, "a, b, flip, on");

    // incremental: no changes -> nothing reparsed
    let out2 = cg_indexer::index_repo(&db, &repo, false, "test2", progress).await.unwrap();
    assert_eq!(out2.files_changed, 0);
    assert_eq!(out2.files_removed, 0);

    // change one file -> only that one re-parsed
    std::fs::write(dir.path().join("lib.rs"), "pub fn a() { }\npub fn b() -> i32 { 8 }\npub fn c() { b(); }\n").unwrap();
    let out3 = cg_indexer::index_repo(&db, &repo, false, "test3", progress).await.unwrap();
    assert_eq!(out3.files_changed, 1);

    // deletion sweep
    std::fs::remove_file(dir.path().join("src/nested.rs")).unwrap();
    let out4 = cg_indexer::index_repo(&db, &repo, false, "test4", progress).await.unwrap();
    assert_eq!(out4.files_removed, 1);
}
