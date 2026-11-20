//! Background index jobs shared by the REST API and filesystem watchers.
//!
//! `AppState.indexing` is the single per-repo lock; a job must be claimed before
//! spawning. Terminal events are emitted before the claim is released.

use crate::state::AppState;
use cg_core::model::Repo;
use cg_indexer::{IndexOutcome, IndexProgress};
use futures::FutureExt;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum IndexJob {
    Repo {
        full: bool,
        trigger: String,
    },
    Paths {
        paths: Vec<PathBuf>,
        trigger: String,
    },
}

/// Try to take the per-repo index lock.
pub fn claim(state: &AppState, repo_id: i64) -> bool {
    let mut guard = state
        .indexing
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.insert(repo_id)
}

pub fn release(state: &AppState, repo_id: i64) {
    let mut guard = state
        .indexing
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.remove(&repo_id);
}

/// Claim and spawn a job. Returns false when another run already holds the repo.
pub fn spawn_index_job(state: AppState, repo: Repo, job: IndexJob) -> bool {
    if !claim(&state, repo.id) {
        return false;
    }
    spawn_claimed(state, repo, job);
    true
}

/// Spawn a job whose repo lock has already been claimed by the caller.
pub fn spawn_claimed(state: AppState, repo: Repo, job: IndexJob) {
    tokio::spawn(async move {
        let repo_id = repo.id;
        let last_run_id = Arc::new(AtomicI64::new(0));
        let progress = {
            let state = state.clone();
            let last_run_id = last_run_id.clone();
            move |p: IndexProgress| {
                if p.run_id > 0 {
                    last_run_id.store(p.run_id, Ordering::SeqCst);
                }
                state.emit(&progress_event(repo_id, &p));
            }
        };
        let job_state = state.clone();
        let job_repo = repo.clone();
        let result = std::panic::AssertUnwindSafe(async move {
            match job {
                IndexJob::Repo { full, trigger } => {
                    cg_indexer::index_repo(&job_state.db, &job_repo, full, &trigger, progress).await
                }
                IndexJob::Paths { paths, trigger } => {
                    cg_indexer::index_paths(&job_state.db, &job_repo, &paths, &trigger, progress)
                        .await
                }
            }
        })
        .catch_unwind()
        .await;
        let known_run = last_run_id.load(Ordering::SeqCst);
        let known_run = (known_run > 0).then_some(known_run);
        match result {
            Ok(Ok(outcome)) => state.emit(&done_event(repo_id, &outcome)),
            Ok(Err(e)) => {
                let error = format!("{e:#}");
                tracing::error!(repo_id, error = %error, "index run failed");
                fail_known_run(&state, known_run).await;
                state.emit(&failed_event(repo_id, &error, known_run));
            }
            Err(payload) => {
                let error = format!("index task panicked: {}", panic_message(payload));
                tracing::error!(repo_id, error = %error, "index task panicked");
                fail_known_run(&state, known_run).await;
                state.emit(&failed_event(repo_id, &error, known_run));
            }
        }
        release(&state, repo_id);
    });
}

pub fn progress_event(repo_id: i64, p: &IndexProgress) -> Value {
    json!({
        "type": "index_progress", "repo_id": repo_id, "run_id": p.run_id,
        "done": p.done, "total": p.total, "phase": "parsing",
        "current_file": p.current_file,
    })
}

pub fn done_event(repo_id: i64, o: &IndexOutcome) -> Value {
    json!({
        "type": "index_done", "repo_id": repo_id, "run_id": o.run_id,
        "stats": {
            "files_changed": o.files_changed,
            "symbols": o.symbols,
            "edges": o.edges,
            "symbols_total": o.symbols_total,
            "edges_total": o.edges_total,
            "duration_ms": o.duration_ms,
        },
    })
}

pub fn failed_event(repo_id: i64, error: &str, run_id: Option<i64>) -> Value {
    json!({
        "type": "index_failed", "repo_id": repo_id, "run_id": run_id, "error": error,
    })
}

async fn fail_known_run(state: &AppState, run_id: Option<i64>) {
    let Some(run_id) = run_id else { return };
    if let Err(e) = state
        .db
        .execute(
            "UPDATE index_runs SET status = 'failed', finished_at = datetime('now')
             WHERE id = ?1 AND status = 'running'",
            libsql::params![run_id],
        )
        .await
    {
        tracing::warn!(run_id, error = %e, "failed to finalize panicked index run");
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}
