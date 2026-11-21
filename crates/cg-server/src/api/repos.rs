//! Repo endpoints: CRUD, URL/local registration, index + analyze triggers, status.

use crate::index_jobs::{failed_event, spawn_index_job, IndexJob};
use crate::state::AppState;
use crate::stop_repo_watcher;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use cg_core::model::Repo;
use cg_db::{repos, runs};
use cg_feature::{canonical_git_url, feature_repo_id};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

type ApiError = (StatusCode, Json<Value>);

fn err(status: StatusCode, msg: impl Into<String>) -> ApiError {
    (status, Json(json!({ "error": msg.into() })))
}

/// Poison-safe `AppState::cloning` lock: a panicked clone task must not wedge
/// the registration endpoints. Callers must drop the guard before awaiting.
fn lock_cloning(state: &AppState) -> std::sync::MutexGuard<'_, std::collections::HashSet<i64>> {
    state
        .cloning
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// True while an index run holds the repo's lock.
fn is_indexing(state: &AppState, id: i64) -> bool {
    state
        .indexing
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .contains(&id)
}

pub async fn list(State(state): State<AppState>) -> Json<Value> {
    let list = repos::list(&state.db).await.unwrap_or_default();
    Json(json!({ "repos": list }))
}

/// `{"url", "name?"}` clones + indexes in the background;
/// `{"path", "name?", "git_url?"}` registers a local checkout (legacy mode).
/// Both modes index immediately after registration.
#[derive(Deserialize)]
pub struct CreateRepo {
    pub url: Option<String>,
    pub path: Option<String>,
    pub name: Option<String>,
    pub git_url: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateRepo>,
) -> Result<Json<Value>, ApiError> {
    match (body.url.as_deref(), body.path.as_deref()) {
        (Some(url), _) => register_by_url(state, &body, url).await,
        (None, Some(path)) => register_by_path(state, &body, path).await,
        (None, None) => Err(err(
            StatusCode::BAD_REQUEST,
            "body must carry 'url' (clone mode) or 'path' (local mode)",
        )),
    }
}

/// Register a local checkout, then kick off indexing right away.
async fn register_by_path(
    state: AppState,
    body: &CreateRepo,
    path: &str,
) -> Result<Json<Value>, ApiError> {
    let name = body.name.clone().unwrap_or_else(|| path_tail(path));
    let repo = repos::upsert(&state.db, path, &name, body.git_url.as_deref())
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
    state.emit(&json!({"type": "repo_added", "repo_id": repo.id}));
    let index_started = spawn_index_job(
        state.clone(),
        repo.clone(),
        IndexJob::Repo {
            full: true,
            trigger: "api".into(),
        },
    );
    if !index_started {
        // No run_id exists here, so do not emit a misleading index_failed event.
        tracing::warn!(
            repo_id = repo.id,
            path = %repo.path,
            "initial index not started: repo is already indexing"
        );
    }
    let feature_repo_id = feature_repo_id(repo.git_url.as_deref());
    Ok(Json(
        json!({ "repo": repo, "feature_repo_id": feature_repo_id }),
    ))
}

/// Register by URL: insert a `cloning` row, respond immediately, then clone +
/// index + watch in the background (progress over WS: `clone_done`/`clone_failed`).
/// Re-registering a `failed` repo URL resets it to `cloning` and retries.
async fn register_by_url(
    state: AppState,
    body: &CreateRepo,
    url: &str,
) -> Result<Json<Value>, ApiError> {
    let canonical = canonical_git_url(url)
        .map_err(|e| err(StatusCode::BAD_REQUEST, format!("invalid url: {e:#}")))?;
    let dir = cg_indexer::derive_clone_dir(&state.cfg.clones_root, &canonical);
    let name = body.name.clone().unwrap_or_else(|| url_tail(&canonical));
    if let Some(existing) = repos::get_by_git_url(&state.db, &canonical)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?
    {
        if existing.status == cg_core::repo_status::FAILED {
            return retry_failed_clone(state, existing, url, dir).await;
        }
        return Err(duplicate_conflict(existing));
    }
    let repo = match repos::insert(
        &state.db,
        &dir.to_string_lossy(),
        &name,
        Some(&canonical),
        cg_core::repo_status::CLONING,
    )
    .await
    {
        Ok(repo) => repo,
        // Lost a concurrent race on the repositories.path UNIQUE constraint:
        // the other request's row now exists — answer with the usual 409.
        Err(e) if format!("{e:#}").to_lowercase().contains("unique") => {
            let existing = repos::get_by_git_url(&state.db, &canonical)
                .await
                .map_err(|se| err(StatusCode::INTERNAL_SERVER_ERROR, format!("{se:#}")))?;
            return match existing {
                Some(existing) => Err(duplicate_conflict(existing)),
                None => Err(err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))),
            };
        }
        Err(e) => return Err(err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))),
    };
    state.emit(&json!({"type": "repo_added", "repo_id": repo.id}));
    spawn_clone_task(state.clone(), repo.clone(), url.to_string(), dir);

    let feature_repo_id = feature_repo_id(repo.git_url.as_deref());
    Ok(Json(
        json!({ "repo": repo, "feature_repo_id": feature_repo_id }),
    ))
}

/// 409 payload for an already-registered canonical URL (`cloning`/`ready`).
fn duplicate_conflict(existing: Repo) -> ApiError {
    (
        StatusCode::CONFLICT,
        Json(json!({
            "error": format!("该仓库已注册：{}", existing.name),
            "repo": existing,
        })),
    )
}

/// Reset a `failed` repo back to `cloning` and re-run the clone pipeline.
async fn retry_failed_clone(
    state: AppState,
    existing: Repo,
    url: &str,
    dir: PathBuf,
) -> Result<Json<Value>, ApiError> {
    {
        let guard = lock_cloning(&state);
        if guard.contains(&existing.id) {
            return Err(err(
                StatusCode::CONFLICT,
                "该仓库克隆任务进行中，请稍后重试",
            ));
        }
    }
    repos::set_status(&state.db, existing.id, cg_core::repo_status::CLONING, None)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
    let repo = Repo {
        status: cg_core::repo_status::CLONING.to_string(),
        error: None,
        ..existing
    };
    spawn_clone_task(state, repo.clone(), url.to_string(), dir);
    let feature_repo_id = feature_repo_id(repo.git_url.as_deref());
    Ok(Json(
        json!({ "repo": repo, "feature_repo_id": feature_repo_id, "retried": true }),
    ))
}

/// Spawn the background clone task: the `cloning` guard set is claimed
/// synchronously (blocks double-spawns across retry + fresh registration),
/// then clone → `ready` + watcher + index, or → `failed` + error detail.
fn spawn_clone_task(state: AppState, repo: Repo, raw_url: String, dir: PathBuf) {
    {
        let mut guard = lock_cloning(&state);
        if !guard.insert(repo.id) {
            return; // a clone task is already running for this repo
        }
    }
    let id = repo.id;
    tokio::spawn(async move {
        let outcome = cg_indexer::run_clone(&raw_url, &dir).await;
        lock_cloning(&state).remove(&id);
        match outcome {
            Ok(()) => {
                tracing::info!(repo_id = repo.id, dir = %dir.display(), "clone finished");
                if let Err(e) =
                    repos::set_status(&state.db, repo.id, cg_core::repo_status::READY, None).await
                {
                    tracing::error!(repo_id = repo.id, error = %e, "set ready failed");
                }
                state.emit(&json!({"type": "clone_done", "repo_id": repo.id}));
                // Claim the index lock before starting the watcher: the initial
                // full scan must not race a watch-triggered incremental run.
                let index_started = spawn_index_job(
                    state.clone(),
                    repo.clone(),
                    IndexJob::Repo {
                        full: true,
                        trigger: "api".into(),
                    },
                );
                if !index_started {
                    const MSG: &str = "initial index not started: repo is already indexing";
                    tracing::warn!(repo_id = repo.id, "{}", MSG);
                    state.emit(&failed_event(repo.id, MSG, None));
                }
                // The watcher runs independently of the initial index, so start
                // it anyway; watch events may queue changes for a later run.
                if state.cfg.watch {
                    crate::spawn_repo_watcher(state.clone(), repo.clone());
                }
            }
            Err(e) => {
                let msg = format!("{e:#}");
                tracing::warn!(repo_id = repo.id, url = %raw_url, error = %msg, "clone failed");
                if let Err(se) =
                    repos::set_status(&state.db, repo.id, cg_core::repo_status::FAILED, Some(&msg))
                        .await
                {
                    tracing::error!(repo_id = repo.id, error = %se, "set failed status errored");
                }
                state.emit(&json!({"type": "clone_failed", "repo_id": repo.id, "error": msg}));
            }
        }
    });
}

pub async fn remove(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let repo = repos::get(&state.db, id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "repo not found"))?;
    if repo.status == cg_core::repo_status::CLONING {
        return Err(err(StatusCode::CONFLICT, "克隆进行中，稍后再删"));
    }
    if is_indexing(&state, id) {
        return Err(err(StatusCode::CONFLICT, "索引进行中，稍后再删"));
    }
    // Stop the watcher first so no new watch-triggered index runs are queued
    // for a repo that is about to disappear.
    stop_repo_watcher(&state, id);
    repos::delete(&state.db, id)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
    Ok(Json(json!({ "ok": true })))
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
) -> Result<Json<Value>, StatusCode> {
    let full = body.and_then(|Json(q)| q.full).unwrap_or(false);
    let repo = repos::get(&state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if repo.status == cg_core::repo_status::CLONING {
        return Err(StatusCode::CONFLICT);
    }
    if spawn_index_job(
        state,
        repo,
        IndexJob::Repo {
            full,
            trigger: "api".into(),
        },
    ) {
        Ok(Json(json!({ "ok": true })))
    } else {
        Ok(Json(
            json!({ "run_id": Value::Null, "already_running": true }),
        ))
    }
}

/// Enqueue hidden-call analysis for every function-like symbol.
pub async fn trigger_analyze(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
    let n = cg_analyzer::enqueue_all(&state.db, id, 100_000)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.emit(&json!({"type": "analyze_enqueued", "repo_id": id, "count": n}));
    Ok(Json(json!({ "enqueued": n })))
}

pub async fn status(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, StatusCode> {
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
    let feature_repo_id = feature_repo_id(repo.git_url.as_deref());
    Ok(Json(json!({
        "repo": repo,
        "stats": stats,
        "last_run": last_run,
        "feature_repo_id": feature_repo_id,
    })))
}

fn path_tail(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// Last path segment of a canonical `host/path` URL, e.g. `org/repo` -> `repo`.
fn url_tail(canonical: &str) -> String {
    canonical
        .rsplit('/')
        .next()
        .unwrap_or(canonical)
        .to_string()
}
