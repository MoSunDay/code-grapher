//! Index a fixture repo on disk; verify db contents + incremental behaviour.

use cg_core::model::{EdgeKind, SymbolKind};
use cg_db::{edges, files, repos, runs, symbols};

fn collect_progress() -> (
    std::sync::Arc<std::sync::Mutex<Vec<cg_indexer::IndexProgress>>>,
    impl FnMut(cg_indexer::IndexProgress) + Send,
) {
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = events.clone();
    let progress = move |p: cg_indexer::IndexProgress| {
        sink.lock().unwrap().push(p);
    };
    (events, progress)
}

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
    std::fs::write(
        dir.path().join("target/debug/ignored.rs"),
        "fn skip_me() {}\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("notes.txt"), "not code").unwrap();

    let db = cg_db::open_local(&dir.path().join("t.db")).await.unwrap();
    cg_db::migrate(&db).await.unwrap();
    let repo = repos::upsert(&db, dir.path().to_str().unwrap(), "fx", None)
        .await
        .unwrap();

    let (progress_events, progress) = collect_progress();
    let out = cg_indexer::index_repo(&db, &repo, false, "test", progress)
        .await
        .unwrap();
    assert_eq!(out.files_scanned, 2, "target/ and .txt skipped");
    assert_eq!(out.files_changed, 2);
    assert_eq!(out.files_removed, 0);
    {
        let events = progress_events.lock().unwrap();
        assert!(!events.is_empty());
        assert!(events.iter().all(|p| p.run_id == out.run_id));
        assert_eq!(events.first().unwrap().done, 0);
        assert_eq!(
            events.first().unwrap().total,
            0,
            "initial preparation event"
        );
        let last = events.last().unwrap();
        assert_eq!(last.done, last.total);
    }

    let list = files::list(&db, repo.id).await.unwrap();
    assert_eq!(list.len(), 2);
    assert!(list.iter().all(|f| f.lang.as_deref() == Some("rust")));

    // direct call a->b resolved via unique name
    edges::resolve_by_name(&db, repo.id).await.unwrap();
    let (total, fns) = symbols::list(
        &db,
        repo.id,
        &symbols::SymbolFilter {
            kind: Some(SymbolKind::Function),
            q: None,
            file_id: None,
            limit: 10,
            offset: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(total, 2, "a, b (flip is a method)");
    let a = fns.iter().find(|s| s.name == "a").unwrap();
    let b_id = fns.iter().find(|s| s.name == "b").map(|s| s.id);
    let callees = edges::for_caller(&db, a.id).await.unwrap();
    let direct = callees
        .iter()
        .find(|e| e.kind == EdgeKind::DirectFn)
        .unwrap();
    assert_eq!(direct.callee_id, b_id);
    // method call recorded as dynamic
    let (_, methods) = symbols::list(
        &db,
        repo.id,
        &symbols::SymbolFilter {
            kind: Some(SymbolKind::Method),
            q: None,
            file_id: None,
            limit: 10,
            offset: 0,
        },
    )
    .await
    .unwrap();
    let flip = methods.iter().find(|s| s.name == "flip").unwrap();
    let flip_callees = edges::for_caller(&db, flip.id).await.unwrap();
    assert!(flip_callees
        .iter()
        .any(|e| e.kind == EdgeKind::DynamicMethod && e.callee_name == "on"));

    // tasks auto-enqueued for function-likes
    let tsks = cg_db::tasks::list(&db, repo.id, None, 100).await.unwrap();
    assert_eq!(tsks.len(), 4, "a, b, flip, on");

    // incremental: no changes -> nothing reparsed
    let out2 = cg_indexer::index_repo(&db, &repo, false, "test2", |_p| {})
        .await
        .unwrap();
    assert_eq!(out2.files_changed, 0);
    assert_eq!(out2.files_removed, 0);

    // change one file -> only that one re-parsed
    std::fs::write(
        dir.path().join("lib.rs"),
        "pub fn a() { }\npub fn b() -> i32 { 8 }\npub fn c() { b(); }\n",
    )
    .unwrap();
    let out3 = cg_indexer::index_repo(&db, &repo, false, "test3", |_p| {})
        .await
        .unwrap();
    assert_eq!(out3.files_scanned, 2);
    assert_eq!(out3.files_changed, 1);

    // deletion sweep
    std::fs::remove_file(dir.path().join("src/nested.rs")).unwrap();
    let out4 = cg_indexer::index_repo(&db, &repo, false, "test4", |_p| {})
        .await
        .unwrap();
    assert_eq!(out4.files_removed, 1);
}

#[tokio::test]
async fn progress_total_tracks_changed_files_not_whole_repo() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("lib.rs"), "pub fn a() {}\n").unwrap();
    std::fs::write(dir.path().join("util.rs"), "pub fn b() {}\n").unwrap();
    let db = cg_db::open_local(&dir.path().join("t.db")).await.unwrap();
    cg_db::migrate(&db).await.unwrap();
    let repo = repos::upsert(&db, dir.path().to_str().unwrap(), "progress", None)
        .await
        .unwrap();
    cg_indexer::index_repo(&db, &repo, false, "seed", |_p| {})
        .await
        .unwrap();

    std::fs::write(dir.path().join("lib.rs"), "pub fn a() { a(); }\n").unwrap();
    let (progress_events, progress) = collect_progress();
    let out = cg_indexer::index_repo(&db, &repo, false, "test", progress)
        .await
        .unwrap();
    assert_eq!(out.files_scanned, 2);
    assert_eq!(out.files_changed, 1);
    assert_eq!(out.files_removed, 0);

    let events = progress_events.lock().unwrap();
    assert!(events.iter().all(|p| p.run_id == out.run_id));
    let last = events.last().unwrap();
    assert_eq!(last.total, 1);
    assert_eq!(last.done, last.total, "final progress is complete");
}

#[tokio::test]
async fn index_paths_opens_and_finishes_a_watch_run() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("lib.rs"), "pub fn a() {}\n").unwrap();
    std::fs::write(dir.path().join("util.rs"), "pub fn b() {}\n").unwrap();
    let db = cg_db::open_local(&dir.path().join("t.db")).await.unwrap();
    cg_db::migrate(&db).await.unwrap();
    let repo = repos::upsert(&db, dir.path().to_str().unwrap(), "paths", None)
        .await
        .unwrap();
    cg_indexer::index_repo(&db, &repo, false, "seed", |_p| {})
        .await
        .unwrap();

    std::fs::write(dir.path().join("lib.rs"), "pub fn a() { a(); }\n").unwrap();
    // util.rs is part of the event batch but unchanged: only lib.rs counts.
    let paths = [dir.path().join("lib.rs"), dir.path().join("util.rs")];
    let (progress_events, progress) = collect_progress();
    let out = cg_indexer::index_paths(&db, &repo, &paths, "watch", progress)
        .await
        .unwrap();
    assert_eq!(out.files_changed, 1);

    {
        let events = progress_events.lock().unwrap();
        assert!(!events.is_empty());
        assert!(events.iter().all(|p| p.run_id == out.run_id));
        let per_file: Vec<_> = events
            .iter()
            .filter(|p| !p.current_file.is_empty())
            .collect();
        assert_eq!(per_file.len(), 1, "one frame per re-parsed file");
        assert_eq!(per_file[0].total, 1);
        assert_eq!(per_file[0].done, 0);
        assert_eq!(per_file[0].current_file, "lib.rs");
        let last = events.last().unwrap();
        assert_eq!(last.total, 1);
        assert_eq!(last.done, last.total, "final progress is complete");
        assert!(last.current_file.is_empty());
    }

    let run = runs::last(&db, repo.id).await.unwrap().unwrap();
    assert_eq!(run.id, out.run_id);
    assert_eq!(run.status, "ok");
    assert_eq!(run.trigger, "watch");
    assert!(run.finished_at.is_some());
}

#[tokio::test]
async fn index_paths_unchanged_content_reports_zero_total() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("lib.rs"), "pub fn a() {}\n").unwrap();
    std::fs::write(dir.path().join("util.rs"), "pub fn b() {}\n").unwrap();
    let db = cg_db::open_local(&dir.path().join("t.db")).await.unwrap();
    cg_db::migrate(&db).await.unwrap();
    let repo = repos::upsert(&db, dir.path().to_str().unwrap(), "noop", None)
        .await
        .unwrap();
    cg_indexer::index_repo(&db, &repo, false, "seed", |_p| {})
        .await
        .unwrap();

    // Same content, only the mtime changed (touch-like watcher event).
    std::fs::write(dir.path().join("lib.rs"), "pub fn a() {}\n").unwrap();
    let paths = [dir.path().join("lib.rs"), dir.path().join("util.rs")];
    let (progress_events, progress) = collect_progress();
    let out = cg_indexer::index_paths(&db, &repo, &paths, "watch", progress)
        .await
        .unwrap();
    assert_eq!(out.files_scanned, 2, "both event paths were selected");
    assert_eq!(out.files_changed, 0, "unchanged content is not reparsed");

    let events = progress_events.lock().unwrap();
    assert!(!events.is_empty());
    assert!(events.iter().all(|p| p.run_id == out.run_id));
    assert!(
        events.iter().all(|p| p.total == 0),
        "no positive progress for unchanged files: {events:?}"
    );
    let last = events.last().unwrap();
    assert_eq!(last.total, 0);
    assert_eq!(last.done, 0);
}

#[tokio::test]
async fn index_paths_sweeps_a_deleted_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("lib.rs"), "pub fn a() {}\n").unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/nested.rs"),
        "pub struct Gone { flag: bool }\nimpl Gone { pub fn flip(&mut self) { self.flag = !self.flag; } }\n",
    )
    .unwrap();
    let db = cg_db::open_local(&dir.path().join("t.db")).await.unwrap();
    cg_db::migrate(&db).await.unwrap();
    let repo = repos::upsert(&db, dir.path().to_str().unwrap(), "sweep", None)
        .await
        .unwrap();
    cg_indexer::index_repo(&db, &repo, false, "seed", |_p| {})
        .await
        .unwrap();
    let files_before = files::list(&db, repo.id).await.unwrap().len();
    let (symbols_before, _) = symbols::list(
        &db,
        repo.id,
        &symbols::SymbolFilter {
            kind: None,
            q: None,
            file_id: None,
            limit: 10,
            offset: 0,
        },
    )
    .await
    .unwrap();

    std::fs::remove_file(dir.path().join("src/nested.rs")).unwrap();
    let out = cg_indexer::index_paths(
        &db,
        &repo,
        &[dir.path().join("src/nested.rs")],
        "watch",
        |_p| {},
    )
    .await
    .unwrap();
    assert_eq!(out.files_removed, 1);
    assert_eq!(
        files::list(&db, repo.id).await.unwrap().len(),
        files_before - 1
    );
    let (symbols_after, _) = symbols::list(
        &db,
        repo.id,
        &symbols::SymbolFilter {
            kind: None,
            q: None,
            file_id: None,
            limit: 10,
            offset: 0,
        },
    )
    .await
    .unwrap();
    assert!(
        symbols_after < symbols_before,
        "{symbols_after} < {symbols_before}"
    );
}

#[tokio::test]
async fn reports_parsed_counts_and_deduplicated_graph_totals() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("lib.rs"),
        "pub fn a() { b(); b(); }\npub fn b() {}\n",
    )
    .unwrap();

    let db = cg_db::open_local(&dir.path().join("t.db")).await.unwrap();
    cg_db::migrate(&db).await.unwrap();
    let repo = repos::upsert(&db, dir.path().to_str().unwrap(), "dedup", None)
        .await
        .unwrap();

    let out = cg_indexer::index_repo(&db, &repo, true, "test", |_p| {})
        .await
        .unwrap();
    assert_eq!(out.edges, 2, "parsed edge count keeps both calls");
    assert_eq!(
        out.edges_total, 1,
        "graph total dedups caller+name+kind+source"
    );

    let (symbols_total, _) = symbols::list(
        &db,
        repo.id,
        &symbols::SymbolFilter {
            kind: None,
            q: None,
            file_id: None,
            limit: 10,
            offset: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(symbols_total, out.symbols_total);
}

#[tokio::test]
async fn deleted_repository_path_fails_instead_of_sweeping() {
    let repo_dir = tempfile::tempdir().unwrap();
    let db_dir = tempfile::tempdir().unwrap();
    std::fs::write(repo_dir.path().join("lib.rs"), "pub fn keep_me() {}\n").unwrap();
    let db = cg_db::open_local(&db_dir.path().join("t.db"))
        .await
        .unwrap();
    cg_db::migrate(&db).await.unwrap();
    let repo = repos::upsert(&db, repo_dir.path().to_str().unwrap(), "gone", None)
        .await
        .unwrap();
    cg_indexer::index_repo(&db, &repo, false, "seed", |_p| {})
        .await
        .unwrap();
    assert_eq!(files::list(&db, repo.id).await.unwrap().len(), 1);

    std::fs::remove_dir_all(repo_dir.path()).unwrap();
    let err = cg_indexer::index_repo(&db, &repo, false, "test", |_p| {})
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not a directory"), "{err:#}");

    let run = runs::last(&db, repo.id).await.unwrap().unwrap();
    assert_eq!(run.status, "failed");
    assert!(run.finished_at.is_some());
    assert_eq!(
        files::list(&db, repo.id).await.unwrap().len(),
        1,
        "failed scan must not sweep existing files"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn unreadable_subdir_fails_before_sweeping() {
    use std::os::unix::fs::PermissionsExt;

    let repo_dir = tempfile::tempdir().unwrap();
    let db_dir = tempfile::tempdir().unwrap();
    std::fs::write(repo_dir.path().join("lib.rs"), "pub fn a() {}\n").unwrap();
    std::fs::create_dir(repo_dir.path().join("locked")).unwrap();
    std::fs::write(repo_dir.path().join("locked/keep.rs"), "pub fn b() {}\n").unwrap();

    let db = cg_db::open_local(&db_dir.path().join("t.db"))
        .await
        .unwrap();
    cg_db::migrate(&db).await.unwrap();
    let repo = repos::upsert(&db, repo_dir.path().to_str().unwrap(), "locked", None)
        .await
        .unwrap();
    cg_indexer::index_repo(&db, &repo, false, "seed", |_p| {})
        .await
        .unwrap();
    assert_eq!(files::list(&db, repo.id).await.unwrap().len(), 2);

    let locked = repo_dir.path().join("locked");
    let locked_perms = std::fs::metadata(&locked).unwrap().permissions();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

    // Running as root (or on an OS that ignores mode bits) can still read the
    // directory; restore and skip rather than asserting a failure that cannot
    // happen in this environment.
    let unreadable = std::fs::read_dir(&locked).is_err();
    if !unreadable {
        std::fs::set_permissions(&locked, locked_perms).unwrap();
        return;
    }

    let result = cg_indexer::index_repo(&db, &repo, false, "test", |_p| {}).await;
    // Restore before asserting so a panic (or tempdir cleanup) never leaves a
    // 000-mode directory behind.
    std::fs::set_permissions(&locked, locked_perms).unwrap();

    let err = result.expect_err("unreadable subdir must fail the scan");
    assert!(
        format!("{err:#}").contains("locked"),
        "scan error should name the unreadable dir: {err:#}"
    );
    assert_eq!(
        files::list(&db, repo.id).await.unwrap().len(),
        2,
        "failed scan must not sweep existing files"
    );
    let run = runs::last(&db, repo.id).await.unwrap().unwrap();
    assert_eq!(run.status, "failed");
    assert!(run.finished_at.is_some());
}

#[tokio::test]
async fn index_paths_matches_absolute_events_for_relative_root() {
    let dir = tempfile::Builder::new()
        .prefix("cg-relative-root-")
        .tempdir_in(".")
        .unwrap();
    std::fs::write(dir.path().join("lib.rs"), "pub fn a() {}\n").unwrap();
    let relative_root = dir.path().to_string_lossy().to_string();
    let absolute_event = dir.path().canonicalize().unwrap().join("lib.rs");
    let db = cg_db::open_local(&dir.path().join("t.db")).await.unwrap();
    cg_db::migrate(&db).await.unwrap();
    let repo = repos::upsert(&db, &relative_root, "relative", None)
        .await
        .unwrap();
    cg_indexer::index_repo(&db, &repo, false, "seed", |_p| {})
        .await
        .unwrap();

    std::fs::write(dir.path().join("lib.rs"), "pub fn a() { a(); }\n").unwrap();
    let out = cg_indexer::index_paths(&db, &repo, &[absolute_event], "watch", |_p| {})
        .await
        .unwrap();
    assert_eq!(
        out.files_changed, 1,
        "absolute watcher path must map onto the relative repo root"
    );
}
