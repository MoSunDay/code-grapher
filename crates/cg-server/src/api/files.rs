//! File endpoints: listing + content.

use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use cg_db::{files, repos};
use serde::Deserialize;

pub async fn list(State(state): State<AppState>, Path(id): Path<i64>) -> Result<Json<serde_json::Value>, StatusCode> {
    let list = files::list(&state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "files": list })))
}

#[derive(Deserialize)]
pub struct FileQuery {
    pub path: String,
}

pub async fn content(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<FileQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let repo = repos::get(&state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let file = files::get_by_path(&state.db, id, &q.path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // read from disk with a path-traversal guard
    let root = std::path::Path::new(&repo.path).canonicalize().map_err(|_| StatusCode::NOT_FOUND)?;
    let abs = root.join(&file.path);
    let canonical = abs.canonicalize().map_err(|_| StatusCode::NOT_FOUND)?;
    if !canonical.starts_with(&root) {
        return Err(StatusCode::FORBIDDEN);
    }
    let content = tokio::fs::read_to_string(&canonical)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::json!({ "file": file, "content": content })))
}
