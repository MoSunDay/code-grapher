//! cg-server: background daemon assembling API, watcher and analyzer.

pub mod api;
pub mod index_jobs;
pub mod state;
pub mod ws;

use anyhow::Result;
use cg_analyzer::{AnalyzerConfig, OpencodeClient};
use index_jobs::{claim, failed_event, release, spawn_claimed, IndexJob};
use state::{AppState, ServerConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Boot the daemon: db + migrations + watchers + analyzer scheduler + axum.
pub async fn run(cfg: ServerConfig) -> Result<()> {
    let db = cg_db::open_env(&cfg.db_path).await?;
    cg_db::migrate(&db).await?;
    let state = AppState::new(db.clone(), cfg.clone());

    // clones interrupted by a previous shutdown are permanently failed
    if let Ok(n) = cg_db::repos::fail_orphan_cloning(&db).await {
        if n > 0 {
            tracing::warn!(n, "marked interrupted clones as failed");
        }
    }

    if cfg.watch {
        for repo in cg_db::repos::list(&db).await.unwrap_or_default() {
            if repo.repo.status != cg_core::repo_status::READY {
                continue; // cloning dirs do not exist yet; failed dirs are unusable
            }
            spawn_repo_watcher(state.clone(), repo.repo.clone());
        }
        tracing::info!("watchers started for existing repos");
    }

    if cfg.analyzer {
        let client = OpencodeClient::from_env();
        let analyzer_cfg = AnalyzerConfig::from_env();
        tracing::info!(
            url = client.base_url(),
            concurrency = state
                .analyze_concurrency
                .load(std::sync::atomic::Ordering::Relaxed),
            "analyzer scheduler started"
        );
        if let Ok(n) = cg_db::tasks::reset_running(&db).await {
            if n > 0 {
                tracing::info!(n, "requeued stale running tasks");
            }
        }
        cg_analyzer::spawn_scheduler(
            db.clone(),
            client,
            analyzer_cfg,
            state.analyze_concurrency.clone(),
        );
    }

    let mut app = api::router(state.clone());
    if let Some(dist) = &cfg.web_dist {
        let index = dist.join("index.html");
        let spa = tower_http::services::ServeDir::new(dist)
            .fallback(tower_http::services::ServeFile::new(index));
        app = app.fallback_service(spa);
        tracing::info!(dist = %dist.display(), "serving web UI at /");
    }
    let addr = format!("0.0.0.0:{}", cfg.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "code-grapher server listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Watch one repo; re-indexes changed source files (debounced batches).
pub(crate) fn spawn_repo_watcher(state: AppState, repo: cg_core::model::Repo) {
    let (mut rx, stop) =
        match cg_indexer::spawn_watcher(std::path::Path::new(&repo.path), Duration::from_secs(2)) {
            Ok(pair) => pair,
            Err(e) => {
                emit_watch_failure(&state, repo.id, &format!("watcher unavailable: {e:#}"));
                return;
            }
        };
    // Register the stop flag before spawning the task; retire any older watcher
    // for the same repo so replacing it cannot leak a second running watcher.
    {
        let mut stops = state
            .watcher_stops
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(previous) = stops.insert(repo.id, Arc::clone(&stop)) {
            previous.store(true, Ordering::SeqCst);
        }
    }
    tokio::spawn(async move {
        'watch: loop {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            let first = match rx.recv().await {
                Some(cg_indexer::WatchEvent::Changed(p)) => p,
                Some(cg_indexer::WatchEvent::Error(e)) => {
                    emit_watch_failure(&state, repo.id, &e);
                    continue;
                }
                None => {
                    if !stop.load(Ordering::SeqCst) {
                        emit_watch_failure(&state, repo.id, "watcher stopped (channel closed)");
                    }
                    break;
                }
            };
            let mut batch = vec![first];
            // collect the debounce window
            let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
            let mut closed = false;
            while tokio::time::Instant::now() < deadline {
                match tokio::time::timeout(Duration::from_millis(300), rx.recv()).await {
                    Ok(Some(cg_indexer::WatchEvent::Changed(p))) => batch.push(p),
                    Ok(Some(cg_indexer::WatchEvent::Error(e))) => {
                        emit_watch_failure(&state, repo.id, &e);
                    }
                    Ok(None) => {
                        if !stop.load(Ordering::SeqCst) {
                            emit_watch_failure(&state, repo.id, "watcher stopped (channel closed)");
                        }
                        closed = true;
                        break;
                    }
                    Err(_) => {}
                }
            }
            // Never queue more index work once this watcher has been stopped.
            if closed || stop.load(Ordering::SeqCst) {
                break;
            }
            batch.dedup();
            let relevant: Vec<std::path::PathBuf> = batch
                .into_iter()
                .filter(|p| cg_indexer::is_watch_relevant(p))
                .collect();
            if relevant.is_empty() {
                continue;
            }
            state.emit(&serde_json::json!({
                "type": "log", "level": "info",
                "message": format!("watch: {} changed files", relevant.len()),
            }));
            // Directory create/delete/rename may not emit per-file events; fall
            // back to an incremental repo scan so nested changes are not missed.
            let job = if cg_indexer::needs_repo_scan(&relevant) {
                IndexJob::Repo {
                    full: false,
                    trigger: "watch".into(),
                }
            } else {
                IndexJob::Paths {
                    paths: relevant,
                    trigger: "watch".into(),
                }
            };
            // API and watcher share the per-repo lock: wait instead of dropping
            // events when an API-triggered run is already in progress.
            while !claim(&state, repo.id) {
                if stop.load(Ordering::SeqCst) {
                    break 'watch;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            if stop.load(Ordering::SeqCst) {
                release(&state, repo.id);
                break;
            }
            spawn_claimed(state.clone(), repo.clone(), job);
        }
        unregister_watcher_stop(&state, repo.id, &stop);
    });
}

/// Ask the repo's watcher (if any) to stop and forget its flag. The watcher
/// task then exits when its channel closes without reporting a failure.
pub(crate) fn stop_repo_watcher(state: &AppState, repo_id: i64) {
    let stop = {
        let mut stops = state
            .watcher_stops
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        stops.remove(&repo_id)
    };
    if let Some(stop) = stop {
        stop.store(true, Ordering::SeqCst);
    }
}

/// Remove the map entry only while it still points at this watcher's flag, so
/// a newer watcher for the same repo is never unregistered by an older task.
fn unregister_watcher_stop(state: &AppState, repo_id: i64, stop: &Arc<AtomicBool>) {
    let mut stops = state
        .watcher_stops
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let is_current = stops
        .get(&repo_id)
        .is_some_and(|current| Arc::ptr_eq(current, stop));
    if is_current {
        stops.remove(&repo_id);
    }
}

fn emit_watch_failure(state: &AppState, repo_id: i64, message: &str) {
    tracing::warn!(repo_id, error = message, "watcher failed");
    state.emit(&failed_event(repo_id, message, None));
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}
