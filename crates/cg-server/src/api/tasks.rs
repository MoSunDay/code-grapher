//! Task endpoints: queue inspection, retry, human review of LLM edges.

use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use cg_core::model::TaskStatus;
use cg_db::{edges, symbols, tasks};

#[derive(Deserialize)]
pub struct TaskQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
}

pub async fn list(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<TaskQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let status = q.status.as_deref().and_then(TaskStatus::parse);
    let list = tasks::list(&state.db, id, status, q.limit.unwrap_or(100).clamp(1, 500))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut views = Vec::with_capacity(list.len());
    for t in list {
        let target = match t.target_symbol_id {
            Some(sid) => symbols::get(&state.db, sid).await.ok().flatten(),
            None => None,
        };
        let mut v = serde_json::to_value(&t).unwrap_or_default();
        v.as_object_mut()
            .map(|m| m.insert("target_symbol".into(), serde_json::to_value(target).unwrap_or(serde_json::Value::Null)));
        views.push(v);
    }
    Ok(Json(serde_json::json!({ "tasks": views })))
}

pub async fn retry(State(state): State<AppState>, Path(id): Path<i64>) -> Result<Json<serde_json::Value>, StatusCode> {
    tasks::retry(&state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.emit(&serde_json::json!({"type": "task_update", "task_id": id, "status": "pending"}));
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn unresolved(State(state): State<AppState>, Path(id): Path<i64>) -> Result<Json<serde_json::Value>, StatusCode> {
    let list = edges::unresolved(&state.db, id, 500)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut views = Vec::with_capacity(list.len());
    for e in list {
        let caller = symbols::get(&state.db, e.caller_id).await.ok().flatten();
        let mut v = serde_json::to_value(&e).unwrap_or_default();
        v.as_object_mut()
            .map(|m| m.insert("caller".into(), serde_json::to_value(caller).unwrap_or(serde_json::Value::Null)));
        views.push(v);
    }
    Ok(Json(serde_json::json!({ "edges": views })))
}

#[derive(Deserialize)]
pub struct ConfirmBody {
    pub callee_id: i64,
}

pub async fn confirm(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    axum::Json(body): axum::Json<ConfirmBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    edges::confirm(&state.db, id, body.callee_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn dismiss(State(state): State<AppState>, Path(id): Path<i64>) -> Result<Json<serde_json::Value>, StatusCode> {
    edges::dismiss(&state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
