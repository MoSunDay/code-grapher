//! REST router assembly.

use axum::extract::State;
use axum::Json;

/// Liveness probe.
pub async fn health(State(_): State<crate::state::AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

pub mod features;
pub mod files;
pub mod repos;
pub mod stats;
pub mod symbols;

use crate::state::AppState;
use axum::routing::{delete, get, post};
use axum::Router;
use tower_http::cors::CorsLayer;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/repos", get(repos::list).post(repos::create))
        .route("/api/features/{repo_id}/mrs", get(features::mrs))
        .route("/api/features/{repo_id}/tree", get(features::tree))
        .route("/api/features/{repo_id}/blob", get(features::blob))
        .route("/api/features/{repo_id}/changelog", get(features::changelog))
        .route("/api/features/{repo_id}/draft", post(features::draft))
        .route(
            "/api/features/{repo_id}/{feature_name}/diff",
            get(features::diff),
        )
        .route(
            "/api/features/{repo_id}/{feature_name}/commits",
            get(features::commits),
        )
        .route(
            "/api/features/{repo_id}/{feature_name}/approve",
            post(features::approve),
        )
        .route("/api/repos/{id}", delete(repos::remove).get(repos::status))
        .route("/api/repos/{id}/index", post(repos::trigger_index))
        .route("/api/repos/{id}/analyze", post(repos::trigger_analyze))
        .route("/api/repos/{id}/files", get(files::list))
        .route("/api/repos/{id}/file", get(files::content))
        .route("/api/repos/{id}/symbols", get(symbols::list))
        .route("/api/repos/{id}/stats", get(stats::repo_stats))
        .route("/api/symbols/{id}", get(symbols::detail))
        .route("/api/symbols/{id}/graph", get(symbols::graph))
        .route("/api/config", get(config).put(update_config))
        .route("/api/health", get(health))
        .route("/ws/events", get(crate::ws::ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn config(axum::extract::State(state): axum::extract::State<AppState>) -> axum::Json<serde_json::Value> {
    let n = state.analyze_concurrency.load(std::sync::atomic::Ordering::Relaxed);
    axum::Json(serde_json::json!({
        "db": {
            "mode": if std::env::var("TURSO_DATABASE_URL").map(|u| !u.is_empty() && !u.starts_with("file:")).unwrap_or(false) { "remote" } else { "local" },
        },
        "opencode": {
            "url_set": std::env::var("OPENCODER_SERVER_URL").is_ok(),
            "concurrency": n,
            "paused": n == 0,
        },
        "analyzer": { "enabled": state.cfg.analyzer },
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[derive(serde::Deserialize)]
struct ConfigUpdate {
    analyze_concurrency: u64,
}

async fn update_config(axum::extract::State(state): axum::extract::State<AppState>, axum::Json(body): axum::Json<ConfigUpdate>) -> impl axum::response::IntoResponse {
    let n = body.analyze_concurrency.clamp(0, 16) as usize;
    state.analyze_concurrency.store(n, std::sync::atomic::Ordering::Relaxed);
    state.emit(&serde_json::json!({"type":"config_updated","analyze_concurrency":n,"paused":n==0}));
    (
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({"ok":true,"analyze_concurrency":n,"paused":n==0})),
    )
}
