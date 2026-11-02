//! Repo endpoints: CRUD, index trigger, analyze trigger, status.

use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use cg_db::{repos, runs};
use serde::Deserialize;

pub async fn list(State(state): State<AppState>) -> Json<serde_json::Value> {
    let list = repos::list(&state.db).await.unwrap_or_default();
    Json(serde_json::json!({ "repos": list }))
}

#[derive(Deserialize)]
pub struct CreateRepo {
    pub path: String,
    pub name: Option<String>,
    pub git_url: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateRepo>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let name = body
        .name
        .clone()
        .unwrap_or_else(|| path_tail(&body.path));
    let repo = repos::upsert(&state.db, &body.path, &name, body.git_url.as_deref())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.emit(&serde_json::json!({"type": "repo_added", "repo_id": repo.id}));
    Ok(Json(serde_json::json!({ "repo": repo })))
}

pub async fn remove(State(state): State<AppState>, Path(id): Path<i64>) -> Result<Json<serde_json::Value>, StatusCode> {
    repos::delete(&state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize, Default)]
pub struct IndexQuery {
    #[serde(default)]
    pub full: Option<bool>,
}

/// Trigger an indexing run in the background; progress flows over /ws/events.
pub async fn trigger_index(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    body: Option<Json<IndexQuery>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let full = body.and_then(|Json(q)| q.full).unwrap_or(false);
    let repo = repos::get(&state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    {
        let mut guard = state.indexing.lock().unwrap();
        if !guard.insert(id) {
            return Ok(Json(serde_json::json!({ "run_id": null, "already_running": true })));
        }
    }
    let st = state.clone();
    tokio::spawn(async move {
        let progress = |p: cg_indexer::IndexProgress| {
            st.emit(&serde_json::json!({
                "type": "index_progress", "repo_id": id, "run_id": serde_json::Value::Null,
                "done": p.done, "total": p.total, "phase": "parsing", "current_file": p.current_file,
            }));
        };
        let outcome = cg_indexer::index_repo(&st.db, &repo, full, "api", progress).await;
        st.indexing.lock().unwrap().remove(&id);
        match outcome {
            Ok(o) => {
                st.emit(&serde_json::json!({
                    "type": "index_done", "repo_id": id, "run_id": o.run_id,
                    "stats": { "files_changed": o.files_changed, "symbols": o.symbols,
                               "edges": o.edges, "duration_ms": o.duration_ms },
                }));
            }
            Err(e) => {
                tracing::error!(repo_id = id, error = %e, "index run failed");
                st.emit(&serde_json::json!({ "type": "index_failed", "repo_id": id, "error": format!("{e:#}") }));
            }
        }
    });
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// Enqueue hidden-call analysis for every function-like symbol.
pub async fn trigger_analyze(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let n = cg_analyzer::enqueue_all(&state.db, id, 100_000)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.emit(&serde_json::json!({"type": "analyze_enqueued", "repo_id": id, "count": n}));
    Ok(Json(serde_json::json!({ "enqueued": n })))
}

pub async fn status(State(state): State<AppState>, Path(id): Path<i64>) -> Result<Json<serde_json::Value>, StatusCode> {
    let repo = repos::get(&state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let stats = repos::list(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .find(|r| r.repo.id == id)
        .map(|r| r.stats);
    let last_run = runs::last(&state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "repo": repo, "stats": stats, "last_run": last_run })))
}

fn path_tail(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}
