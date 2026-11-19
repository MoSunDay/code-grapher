//! Feature review API: MR branches + document tree over the git-backed store.
//!
//! Addresses are repo_id-keyed (`sha256(canonical git_url)`); drafts live on
//! `mr/<name>` branches, approve merges into `main`. See docs/api.md.

use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use cg_feature::ops::{self, DraftFile};
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

fn conflict(error: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    (StatusCode::CONFLICT, Json(json!({ "error": error.to_string() })))
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

/// GET /api/features/{repo_id}/mrs — MR list (open | merged).
pub async fn mrs(State(state): State<AppState>, Path(repo_id): Path<String>) -> ApiResult {
    check_repo_id(&repo_id)?;
    let partition = repo_id.clone();
    let mrs = run(state.cfg.feature_root.clone(), move |root| {
        ops::mrs(root, &partition)
    })
    .await?;
    Ok(Json(json!({ "repo_id": repo_id, "mrs": mrs })))
}

#[derive(Deserialize, Default)]
pub struct MrQuery {
    pub mr: Option<String>,
}

/// GET /api/features/{repo_id}/tree?mr=NAME — file tree with draft badges.
pub async fn tree(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
    Query(query): Query<MrQuery>,
) -> ApiResult {
    check_repo_id(&repo_id)?;
    let partition = repo_id.clone();
    let mr = query.mr.clone();
    let files = run(state.cfg.feature_root.clone(), move |root| {
        ops::tree(root, &partition, mr.as_deref())
    })
    .await?;
    Ok(Json(json!({ "repo_id": repo_id, "mr": query.mr, "files": files })))
}

#[derive(Deserialize)]
pub struct BlobQuery {
    pub path: String,
    pub mr: Option<String>,
}

/// GET /api/features/{repo_id}/blob?path=&mr= — one file's content.
pub async fn blob(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
    Query(query): Query<BlobQuery>,
) -> ApiResult {
    check_repo_id(&repo_id)?;
    let partition = repo_id.clone();
    let BlobQuery { path, mr } = query;
    let blob = run(state.cfg.feature_root.clone(), move |root| {
        ops::blob(root, &partition, mr.as_deref(), &path)
    })
    .await?;
    Ok(Json(serde_json::to_value(blob).map_err(internal)?))
}

/// GET /api/features/{repo_id}/changelog — merged approvals, newest first.
pub async fn changelog(State(state): State<AppState>, Path(repo_id): Path<String>) -> ApiResult {
    check_repo_id(&repo_id)?;
    let partition = repo_id.clone();
    let entries = run(state.cfg.feature_root.clone(), move |root| {
        ops::changelog(root, &partition)
    })
    .await?;
    Ok(Json(json!({ "repo_id": repo_id, "changelog": entries })))
}

#[derive(Deserialize)]
pub struct DraftRequest {
    pub name: String,
    pub files: Vec<DraftFileBody>,
}

#[derive(Deserialize)]
pub struct DraftFileBody {
    pub path: String,
    pub content: String,
}

/// POST /api/features/{repo_id}/draft — refresh the `mr/<name>` branch.
pub async fn draft(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
    Json(body): Json<DraftRequest>,
) -> ApiResult {
    check_repo_id(&repo_id)?;
    let files: Vec<DraftFile> = body
        .files
        .into_iter()
        .map(|f| DraftFile { path: f.path, content: f.content })
        .collect();
    let partition = repo_id.clone();
    let name = body.name;
    let outcome = run(state.cfg.feature_root.clone(), move |root| {
        ops::draft(root, &partition, &name, &files, None)
    })
    .await?;
    let payload = match outcome {
        ops::DraftOutcome::Noop => json!({ "repo_id": repo_id, "outcome": "noop" }),
        ops::DraftOutcome::Updated { commit } => {
            json!({ "repo_id": repo_id, "outcome": "draft", "commit": commit })
        }
    };
    Ok(Json(payload))
}

/// GET /api/features/{repo_id}/{feature_name}/diff — `main...mr/<name>`.
pub async fn diff(
    State(state): State<AppState>,
    Path((repo_id, feature_name)): Path<(String, String)>,
) -> ApiResult {
    check_repo_id(&repo_id)?;
    let partition = repo_id.clone();
    let fname = feature_name.clone();
    let diff = run(state.cfg.feature_root.clone(), move |root| {
        ops::diff(root, &partition, &fname)
    })
    .await?;
    Ok(Json(json!({ "repo_id": repo_id, "name": feature_name, "diff": diff })))
}

/// GET /api/features/{repo_id}/{feature_name}/commits — MR commit list.
pub async fn commits(
    State(state): State<AppState>,
    Path((repo_id, feature_name)): Path<(String, String)>,
) -> ApiResult {
    check_repo_id(&repo_id)?;
    let partition = repo_id.clone();
    let fname = feature_name.clone();
    let commits = run(state.cfg.feature_root.clone(), move |root| {
        ops::commits(root, &partition, &fname)
    })
    .await?;
    Ok(Json(json!({ "repo_id": repo_id, "name": feature_name, "commits": commits })))
}

#[derive(Deserialize)]
pub struct ApproveRequest {
    pub reviewer: String,
    pub note: Option<String>,
    pub expected_tip: Option<String>,
}

/// POST /api/features/{repo_id}/{feature_name}/approve — merge into main.
pub async fn approve(
    State(state): State<AppState>,
    Path((repo_id, feature_name)): Path<(String, String)>,
    Json(body): Json<ApproveRequest>,
) -> ApiResult {
    check_repo_id(&repo_id)?;
    let ApproveRequest { reviewer, note, expected_tip } = body;
    let partition = repo_id.clone();
    let fname = feature_name.clone();
    let outcome = run(state.cfg.feature_root.clone(), move |root| {
        let params = ops::ApproveParams {
            reviewer: &reviewer,
            note: note.as_deref(),
            expected_tip: expected_tip.as_deref(),
        };
        ops::approve(root, &partition, &fname, &params)
    })
    .await?;
    Ok(Json(json!({
        "repo_id": repo_id,
        "ok": true,
        "merge_commit": outcome.merge_commit,
        "mr_commit": outcome.mr_commit,
        "changelog": outcome.changelog,
    })))
}

/// Run a blocking storage operation, mapping failure classes to status codes.
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
            } else if message.contains("tip changed") || message.contains("already merged") {
                conflict(message)
            } else {
                bad_request(message)
            }
        })
}
