//! REST router assembly.

use axum::extract::State;
use axum::Json;

/// Liveness probe.
pub async fn health(State(_): State<crate::state::AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

pub mod files;
pub mod repos;
pub mod stats;
pub mod symbols;
pub mod tasks;

use crate::state::AppState;
use axum::routing::{delete, get, post};
use axum::Router;
use tower_http::cors::CorsLayer;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/repos", get(repos::list).post(repos::create))
        .route("/api/repos/{id}", delete(repos::remove).get(repos::status))
        .route("/api/repos/{id}/index", post(repos::trigger_index))
        .route("/api/repos/{id}/analyze", post(repos::trigger_analyze))
        .route("/api/repos/{id}/files", get(files::list))
        .route("/api/repos/{id}/file", get(files::content))
        .route("/api/repos/{id}/symbols", get(symbols::list))
        .route("/api/repos/{id}/stats", get(stats::repo_stats))
        .route("/api/repos/{id}/tasks", get(tasks::list))
        .route("/api/repos/{id}/edges/unresolved", get(tasks::unresolved))
        .route("/api/symbols/{id}", get(symbols::detail))
        .route("/api/symbols/{id}/graph", get(symbols::graph))
        .route("/api/tasks/{id}/retry", post(tasks::retry))
        .route("/api/edges/{id}/confirm", post(tasks::confirm))
        .route("/api/edges/{id}/dismiss", post(tasks::dismiss))
        .route("/api/config", get(config))
        .route("/api/health", get(health))
        .route("/ws/events", get(crate::ws::ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn config(axum::extract::State(state): axum::extract::State<AppState>) -> axum::Json<serde_json::Value> {
    let analyzer_cfg = cg_analyzer::AnalyzerConfig::from_env();
    axum::Json(serde_json::json!({
        "db": {
            "mode": if std::env::var("TURSO_DATABASE_URL").map(|u| !u.is_empty() && !u.starts_with("file:")).unwrap_or(false) { "remote" } else { "local" },
        },
        "opencode": {
            "url_set": std::env::var("OPENCODER_SERVER_URL").is_ok(),
            "concurrency": analyzer_cfg.concurrency,
        },
        "analyzer": { "enabled": state.cfg.analyzer },
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
