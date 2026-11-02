//! Round-trip test: migrate + repo + file + symbols + edges + tasks + runs.

use cg_core::{EdgeKind, EdgeSource, NewEdge, ParsedSymbol, SymbolKind};
use cg_db::{edges, files, repos, runs, symbols, tasks};

async fn fresh_db() -> libsql::Connection {
    // `keep()` the tempdir: the connection outlives the scope and must be able
    // to create journal files after this function returns.
    let dir = tempfile::tempdir().unwrap().keep();
    let conn = cg_db::open_local(&dir.join("test.db")).await.unwrap();
    cg_db::migrate(&conn).await.unwrap();
    conn
}

#[tokio::test]
async fn full_roundtrip() {
    let conn = fresh_db().await;

    // repo
    let repo = repos::upsert(&conn, "/tmp/demo", "demo", Some("https://x/demo.git"))
        .await
        .unwrap();
    assert_eq!(repo.name, "demo");
    let again = repos::upsert(&conn, "/tmp/demo", "demo2", None).await.unwrap();
    assert_eq!(again.id, repo.id, "upsert is idempotent on path");

    // file
    let fid = files::upsert(&conn, repo.id, "src/lib.rs", Some("rust"), "abc", 123, 42)
        .await
        .unwrap();

    // symbols (pre-order: parent fn first, closure nested)
    let syms = vec![
        ParsedSymbol { kind: SymbolKind::Function, name: "main".into(), qual_path: "main".into(),
            start_line: 1, end_line: 20, parent_qual: None, is_pub: false },
        ParsedSymbol { kind: SymbolKind::Struct, name: "Cfg".into(), qual_path: "Cfg".into(),
            start_line: 22, end_line: 25, parent_qual: None, is_pub: true },
        ParsedSymbol { kind: SymbolKind::Method, name: "run".into(), qual_path: "Impl Foo::run".into(),
            start_line: 30, end_line: 40, parent_qual: None, is_pub: true },
        ParsedSymbol { kind: SymbolKind::Closure, name: "closure@31".into(),
            qual_path: "Impl Foo::run::closure@31".into(),
            start_line: 31, end_line: 31, parent_qual: Some("Impl Foo::run".into()), is_pub: false },
    ];
    let ids = symbols::replace_file_symbols(&conn, repo.id, fid, &syms).await.unwrap();
    assert_eq!(ids.len(), 4);
    let closure_parent = symbols::get(&conn, ids["Impl Foo::run::closure@31"]).await.unwrap().unwrap();
    assert_eq!(closure_parent.parent_id, Some(ids["Impl Foo::run"]));

    // re-replace is clean (no dupes)
    let ids2 = symbols::replace_file_symbols(&conn, repo.id, fid, &syms).await.unwrap();
    assert_eq!(ids2.len(), 4);
    // replace() rebuilds rows: old ids are stale (cascade-deleted), use fresh map
    let ids = ids2;

    // filters
    let (total, _) = symbols::list(&conn, repo.id, &symbols::SymbolFilter {
        kind: Some(SymbolKind::Struct), q: None, file_id: None, limit: 10, offset: 0 }).await.unwrap();
    assert_eq!(total, 1);

    // edges: main -> run (dynamic_method, unresolved), main -> helper (direct_fn resolvable)
    edges::insert(&conn, repo.id, &[NewEdge {
        caller_id: ids["main"], callee_name: "run".into(), kind: EdgeKind::DynamicMethod,
        source: EdgeSource::Ast, confidence: 1.0, evidence: None, task_id: None },
        NewEdge { caller_id: ids["main"], callee_name: "run".into(), kind: EdgeKind::DirectFn,
        source: EdgeSource::Ast, confidence: 1.0, evidence: None, task_id: None }]).await.unwrap();
    let resolved = edges::resolve_by_name(&conn, repo.id).await.unwrap();
    assert_eq!(resolved, 1, "unique-name direct_fn edge gets bound");
    let callees = edges::for_caller(&conn, ids["main"]).await.unwrap();
    assert_eq!(callees.len(), 2);

    // tasks lifecycle
    tasks::enqueue(&conn, repo.id, "hidden_calls", Some(ids["main"]), None).await.unwrap();
    tasks::enqueue(&conn, repo.id, "hidden_calls", Some(ids["main"]), None).await.unwrap(); // idempotent
    let claimed = tasks::claim(&conn, Some(repo.id), 10).await.unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].status.as_str(), "running");
    tasks::fail(&conn, claimed[0].id, "boom", 3).await.unwrap();
    let st = tasks::fail(&conn, claimed[0].id, "boom", 3).await.unwrap();
    assert_eq!(st.as_str(), "pending", "requeued under max attempts");
    tasks::fail(&conn, claimed[0].id, "boom", 3).await.unwrap();
    let listed = tasks::list(&conn, repo.id, None, 10).await.unwrap();
    assert_eq!(listed[0].status.as_str(), "failed");
    assert_eq!(listed[0].attempts, 3);
    tasks::retry(&conn, claimed[0].id).await.unwrap();
    let claimed2 = tasks::claim(&conn, Some(repo.id), 10).await.unwrap();
    assert_eq!(claimed2.len(), 1);
    tasks::complete(&conn, claimed2[0].id, Some("[{}]")).await.unwrap();

    // runs
    let rid = runs::start(&conn, repo.id, "manual").await.unwrap();
    runs::finish(&conn, rid, "ok", 1, 4, 2, 50).await.unwrap();
    let last = runs::last(&conn, repo.id).await.unwrap().unwrap();
    assert_eq!((last.files_changed, last.symbols, last.edges), (1, 4, 2));

    // sweep missing files
    let keep = serde_json::json!(["src/lib.rs", "src/other.rs"]);
    let del = files::sweep_missing(&conn, repo.id, &keep).await.unwrap();
    assert_eq!(del, 0);
    let keep2 = serde_json::json!([]);
    let del2 = files::sweep_missing(&conn, repo.id, &keep2).await.unwrap();
    assert_eq!(del2, 1);
    let (_, fns) = symbols::list(&conn, repo.id, &symbols::SymbolFilter {
        kind: Some(SymbolKind::Function), q: None, file_id: None, limit: 10, offset: 0 }).await.unwrap();
    assert!(fns.is_empty(), "file delete cascades symbols");
}

#[tokio::test]
async fn repo_delete_cascades() {
    let conn = fresh_db().await;
    let repo = repos::upsert(&conn, "/tmp/x", "x", None).await.unwrap();
    let fid = files::upsert(&conn, repo.id, "a.rs", Some("rust"), "h", 1, 1).await.unwrap();
    let ids = symbols::replace_file_symbols(&conn, repo.id, fid, &[ParsedSymbol {
        kind: SymbolKind::Function, name: "f".into(), qual_path: "f".into(),
        start_line: 1, end_line: 1, parent_qual: None, is_pub: false }]).await.unwrap();
    edges::insert(&conn, repo.id, &[NewEdge { caller_id: ids["f"], callee_name: "g".into(),
        kind: EdgeKind::DynamicMethod, source: EdgeSource::Ast, confidence: 1.0,
        evidence: None, task_id: None }]).await.unwrap();
    repos::delete(&conn, repo.id).await.unwrap();
    let n = cg_db::count(&conn, "SELECT COUNT(*) FROM files", vec![]).await.unwrap();
    assert_eq!(n, 0, "repo delete cascades files/symbols/edges");
}
