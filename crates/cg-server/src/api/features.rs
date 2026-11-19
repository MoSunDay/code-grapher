//! Feature review API: content-addressed by repo_id = sha256(canonical git_url).

use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use cg_feature::ops::{self, ApproveParams};
use cg_feature::validate_repo_id;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

fn bad_request(error: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": error.to_string() })))
}

fn not_found(error: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_FOUND, Json(json!({ "error": error.to_string() })))
}

fn internal(error: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": error.to_string() })),
    )
}

fn check_repo_id(repo_id: &str) -> Result<(), (StatusCode, Json<Value>)> {
    validate_repo_id(repo_id).map_err(bad_request)
}

/// GET /api/features/{repo_id}
pub async fn list(State(state): State<AppState>, Path(repo_id): Path<String>) -> ApiResult {
    check_repo_id(&repo_id)?;
    let partition = repo_id.clone();
    let features = run(state.cfg.feature_root.clone(), move |root| {
        ops::list(root, &partition)
    })
    .await?;
    Ok(Json(json!({ "repo_id": repo_id, "features": features })))
}

/// POST /api/features/{repo_id} — refresh a draft from feature JSON.
pub async fn draft(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult {
    check_repo_id(&repo_id)?;
    let partition = repo_id.clone();
    let source = serde_json::to_vec(&body).map_err(bad_request)?;
    let outcome = run(state.cfg.feature_root.clone(), move |root| {
        ops::draft(root, &partition, &source, None)
    })
    .await?;
    let payload = match outcome {
        ops::DraftOutcome::Noop => json!({ "repo_id": repo_id, "outcome": "noop" }),
        ops::DraftOutcome::Created { rel_path } => {
            json!({ "repo_id": repo_id, "outcome": "draft", "rel_path": rel_path })
        }
    };
    Ok(Json(payload))
}

/// GET /api/features/{repo_id}/{feature_name}
pub async fn get(
    State(state): State<AppState>,
    Path((repo_id, feature_name)): Path<(String, String)>,
) -> ApiResult {
    check_repo_id(&repo_id)?;
    let detail = run(state.cfg.feature_root.clone(), move |root| {
        ops::read_feature(root, &repo_id, &feature_name)
    })
    .await?;
    Ok(Json(serde_json::to_value(detail).map_err(internal)?))
}

/// GET /api/features/{repo_id}/{feature_name}/diff
pub async fn diff(
    State(state): State<AppState>,
    Path((repo_id, feature_name)): Path<(String, String)>,
) -> ApiResult {
    check_repo_id(&repo_id)?;
    let partition = repo_id.clone();
    let fname = feature_name.clone();
    let diff = run(state.cfg.feature_root.clone(), move |root| {
        ops::diff(root, &partition, Some(&fname))
    })
    .await?;
    Ok(Json(json!({ "repo_id": repo_id, "name": feature_name, "diff": diff })))
}

#[derive(Deserialize)]
pub struct ApproveRequest {
    pub reviewer: String,
    pub note: Option<String>,
    pub expected_hash: Option<String>,
}

/// POST /api/features/{repo_id}/{feature_name}/approve
pub async fn approve(
    State(state): State<AppState>,
    Path((repo_id, feature_name)): Path<(String, String)>,
    Json(body): Json<ApproveRequest>,
) -> ApiResult {
    check_repo_id(&repo_id)?;
    let ApproveRequest { reviewer, note, expected_hash } = body;
    let partition = repo_id.clone();
    let fname = feature_name.clone();
    let outcome = run(state.cfg.feature_root.clone(), move |root| {
        let params = ApproveParams {
            reviewer: &reviewer,
            note: note.as_deref(),
            expected_hash: expected_hash.as_deref(),
        };
        ops::approve(root, &partition, &fname, &params)
    })
    .await?;
    Ok(Json(json!({
        "repo_id": repo_id,
        "ok": true,
        "commit": outcome.commit,
        "changelog": outcome.changelog,
    })))
}

/// GET /api/features/{repo_id}/changelog
pub async fn changelog(State(state): State<AppState>, Path(repo_id): Path<String>) -> ApiResult {
    check_repo_id(&repo_id)?;
    let partition = repo_id.clone();
    let entries = run(state.cfg.feature_root.clone(), move |root| {
        ops::read_changelog(root, &partition)
    })
    .await?;
    Ok(Json(json!({ "repo_id": repo_id, "changelog": entries })))
}

/// Run a blocking storage operation, mapping "not found"-style failures to 404.
async fn run<T, F>(root: PathBuf, op: F) -> Result<T, (StatusCode, Json<Value>)>
where
    T: Send + 'static,
    F: FnOnce(&std::path::Path) -> anyhow::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || op(&root))
        .await
        .map_err(internal)?
        .map_err(|error| {
            let message = error.to_string();
            if message.contains("not found") {
                not_found(message)
            } else {
                bad_request(message)
            }
        })
}
