//! Repo deletion contract: conflict while cloning/indexing, stop watcher on delete.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cg_server::index_jobs::{claim, release};
use cg_server::state::{AppState, ServerConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tower::ServiceExt;

#[tokio::test]
async fn delete_conflicts_during_indexing_and_stops_watcher() {
    let repo_dir = tempfile::tempdir().unwrap();
    let data_dir = tempfile::tempdir().unwrap();
    let db = cg_db::open_local(&data_dir.path().join("test.db"))
        .await
        .unwrap();
    cg_db::migrate(&db).await.unwrap();
    let cfg = ServerConfig {
        feature_root: data_dir.path().to_path_buf(),
        ..ServerConfig::default()
    };
    let state = AppState::new(db.clone(), cfg);
    let repo = cg_db::repos::upsert(&db, repo_dir.path().to_str().unwrap(), "rm", None)
        .await
        .unwrap();
    let app = cg_server::api::router(state.clone());

    let stop = Arc::new(AtomicBool::new(false));
    state
        .watcher_stops
        .lock()
        .unwrap()
        .insert(repo.id, Arc::clone(&stop));

    assert!(claim(&state, repo.id));
    let conflict = delete(&app, repo.id).await;
    assert_eq!(
        conflict,
        StatusCode::CONFLICT,
        "indexing repo must not delete"
    );
    assert!(cg_db::repos::get(&db, repo.id).await.unwrap().is_some());
    release(&state, repo.id);

    cg_db::repos::set_status(&db, repo.id, cg_core::repo_status::CLONING, None)
        .await
        .unwrap();
    let conflict = delete(&app, repo.id).await;
    assert_eq!(
        conflict,
        StatusCode::CONFLICT,
        "cloning repo must not delete"
    );
    cg_db::repos::set_status(&db, repo.id, cg_core::repo_status::READY, None)
        .await
        .unwrap();

    let ok = delete(&app, repo.id).await;
    assert_eq!(ok, StatusCode::OK);
    assert!(cg_db::repos::get(&db, repo.id).await.unwrap().is_none());
    assert!(stop.load(Ordering::SeqCst), "delete must stop the watcher");
    assert!(
        !state.watcher_stops.lock().unwrap().contains_key(&repo.id),
        "delete must forget the watcher stop flag"
    );
}

async fn delete(app: &axum::Router, id: i64) -> StatusCode {
    app.clone()
        .oneshot(
            Request::delete(format!("/api/repos/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}
