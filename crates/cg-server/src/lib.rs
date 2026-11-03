//! cg-server: background daemon assembling API, watcher and analyzer.

pub mod api;
pub mod state;
pub mod ws;

use anyhow::Result;
use cg_analyzer::{AnalyzerConfig, OpencodeClient};
use state::{AppState, ServerConfig};
use std::time::Duration;

/// Boot the daemon: db + migrations + watchers + analyzer scheduler + axum.
pub async fn run(cfg: ServerConfig) -> Result<()> {
    let db = cg_db::open_env(&cfg.db_path).await?;
    cg_db::migrate(&db).await?;
    let state = AppState::new(db.clone(), cfg.clone());

    if cfg.watch {
        for repo in cg_db::repos::list(&db).await.unwrap_or_default() {
            spawn_repo_watcher(state.clone(), repo.repo.clone());
        }
        tracing::info!("watchers started for existing repos");
    }

    if cfg.analyzer {
        let client = OpencodeClient::from_env();
        let analyzer_cfg = AnalyzerConfig::from_env();
        tracing::info!(url = client.base_url(), concurrency = analyzer_cfg.concurrency, "analyzer scheduler started");
        if let Ok(n) = cg_db::tasks::reset_running(&db).await {
            if n > 0 { tracing::info!(n, "requeued stale running tasks"); }
        }
        cg_analyzer::spawn_scheduler(db.clone(), client, analyzer_cfg);
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
fn spawn_repo_watcher(state: AppState, repo: cg_core::model::Repo) {
    tokio::spawn(async move {
        let mut rx = match cg_indexer::spawn_watcher(std::path::Path::new(&repo.path), Duration::from_secs(2)) {
            Ok(rx) => rx,
            Err(e) => {
                tracing::warn!(repo = %repo.path, error = %e, "watcher unavailable");
                return;
            }
        };
        loop {
            let Some(first) = rx.recv().await else { break };
            let mut batch = vec![first];
            // collect the debounce window
            let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
            while tokio::time::Instant::now() < deadline {
                match tokio::time::timeout(Duration::from_millis(300), rx.recv()).await {
                    Ok(Some(p)) => batch.push(p),
                    Ok(None) => break,
                    Err(_) => {}
                }
            }
            batch.dedup();
            let relevant: Vec<std::path::PathBuf> = batch
                .into_iter()
                .filter(|p| cg_core::lang::lang_for_path(p).is_some())
                .collect();
            if relevant.is_empty() {
                continue;
            }
            state.emit(&serde_json::json!({
                "type": "log", "level": "info",
                "message": format!("watch: {} changed files", relevant.len()),
            }));
            match cg_indexer::index_paths(&state.db, &repo, &relevant, "watch").await {
                Ok(o) => state.emit(&serde_json::json!({
                    "type": "index_done", "repo_id": repo.id, "run_id": o.run_id,
                    "stats": { "files_changed": o.files_changed, "symbols": o.symbols,
                               "edges": o.edges, "duration_ms": o.duration_ms },
                })),
                Err(e) => tracing::warn!(repo = %repo.path, error = %e, "watch reindex failed"),
            }
        }
    });
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
