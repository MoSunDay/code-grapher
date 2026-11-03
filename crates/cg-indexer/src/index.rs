//! Full & incremental indexing: scan → parse → persist → resolve → enqueue.

use crate::scan::{scan_repo, ScannedFile};
use cg_core::model::*;
use cg_db::{edges, files, repos, runs, symbols, tasks};
use cg_core::parser::{fnv1a_hex, parse};
use libsql::Connection;
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexOutcome {
    pub run_id: i64,
    pub files_scanned: i64,
    pub files_changed: i64,
    pub files_removed: i64,
    pub symbols: i64,
    pub edges: i64,
    pub duration_ms: i64,
}

/// Progress callback payload (drives WS events).
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexProgress {
    pub done: i64,
    pub total: i64,
    pub current_file: String,
}

/// Index a repository. `full = true` reparses everything; otherwise only
/// files whose mtime/hash changed since last run.
pub async fn index_repo(
    conn: &Connection,
    repo: &Repo,
    full: bool,
    trigger: &str,
    mut progress: impl FnMut(IndexProgress) + Send,
) -> anyhow::Result<IndexOutcome> {
    let started = std::time::Instant::now();
    let run_id = runs::start(conn, repo.id, trigger).await?;
    // close out runs left 'running' by a previously killed process
    let _ = conn
        .execute(
            "UPDATE index_runs SET status = 'failed', finished_at = datetime('now')
             WHERE repo_id = ?1 AND status = 'running' AND id != ?2",
            libsql::params![repo.id, run_id],
        )
        .await;
    let root = std::path::Path::new(&repo.path);
    let scanned = scan_repo(root)?;

    let existing: HashMap<String, FileRow> = files::list(conn, repo.id)
        .await?
        .into_iter()
        .map(|f| (f.path.clone(), f))
        .collect();

    let mut changed: Vec<(&ScannedFile, String)> = Vec::new(); // file + content
    for f in &scanned {
        let content = match tokio::fs::read_to_string(&f.abs_path).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(file = %f.rel_path, error = %e, "skip unreadable");
                continue;
            }
        };
        let hash = fnv1a_hex(content.as_bytes());
        let unchanged = !full
            && existing
                .get(&f.rel_path)
                .is_some_and(|prev| prev.hash.as_deref() == Some(hash.as_str()));
        if unchanged {
            continue;
        }
        changed.push((f, content));
    }

    let total = scanned.len() as i64;
    let mut n_symbols = 0i64;
    let mut n_edges = 0i64;
    for (i, (f, content)) in changed.iter().enumerate() {
        progress(IndexProgress {
            done: i as i64,
            total,
            current_file: f.rel_path.clone(),
        });
        let parsed = match parse(f.lang, content) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(file = %f.rel_path, error = %e, "parse failed");
                continue;
            }
        };
        let file_id = files::upsert(conn, repo.id, &f.rel_path, Some(f.lang.as_str()), &fnv1a_hex(content.as_bytes()), f.mtime, parsed.loc).await?;
        let qual_ids = symbols::replace_file_symbols(conn, repo.id, file_id, &parsed.symbols).await?;
        let new_edges: Vec<NewEdge> = parsed
            .calls
            .iter()
            .filter_map(|c| {
                qual_ids.get(&c.caller_qual).map(|&caller_id| NewEdge {
                    caller_id,
                    callee_name: c.callee_name.clone(),
                    kind: c.kind,
                    source: c.source,
                    confidence: c.confidence,
                    evidence: c.evidence.clone(),
                    task_id: None,
                })
            })
            .collect();
        n_symbols += parsed.symbols.len() as i64;
        n_edges += new_edges.len() as i64;
        edges::insert(conn, repo.id, &new_edges).await?;
        enqueue_file_tasks(conn, repo.id, file_id).await?;
    }

    // sweep files that disappeared from disk
    let keep: serde_json::Value = serde_json::json!(scanned.iter().map(|f| f.rel_path.clone()).collect::<Vec<_>>());
    let removed = files::sweep_missing(conn, repo.id, &keep).await? as i64;

    // rebind resolvable edges repo-wide (cheap set-based update)
    edges::resolve_by_name(conn, repo.id).await?;

    progress(IndexProgress { done: total, total, current_file: String::new() });
    let outcome = IndexOutcome {
        run_id,
        files_scanned: total,
        files_changed: changed.len() as i64,
        files_removed: removed,
        symbols: n_symbols,
        edges: n_edges,
        duration_ms: started.elapsed().as_millis() as i64,
    };
    runs::finish(
        conn,
        run_id,
        "ok",
        outcome.files_changed,
        n_symbols,
        n_edges,
        outcome.duration_ms,
    )
    .await?;
    Ok(outcome)
}

/// Incremental: re-index exactly these paths (relative or absolute accepted).
pub async fn index_paths(
    conn: &Connection,
    repo: &Repo,
    paths: &[std::path::PathBuf],
    trigger: &str,
) -> anyhow::Result<IndexOutcome> {
    let root = std::path::Path::new(&repo.path);
    let all = scan_repo(root)?;
    let wanted: Vec<String> = paths
        .iter()
        .map(|p| {
            p.strip_prefix(root)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    let selected: Vec<ScannedFile> = all.into_iter().filter(|f| wanted.contains(&f.rel_path)).collect();
    index_selected(conn, repo, selected, trigger).await
}

/// Re-index only `selected` files (already resolved from disk), sweeping nothing.
async fn index_selected(
    conn: &Connection,
    repo: &Repo,
    selected: Vec<ScannedFile>,
    trigger: &str,
) -> anyhow::Result<IndexOutcome> {
    let started = std::time::Instant::now();
    let run_id = runs::start(conn, repo.id, trigger).await?;
    // close out runs left 'running' by a previously killed process
    let _ = conn
        .execute(
            "UPDATE index_runs SET status = 'failed', finished_at = datetime('now')
             WHERE repo_id = ?1 AND status = 'running' AND id != ?2",
            libsql::params![repo.id, run_id],
        )
        .await;
    let mut n_symbols = 0i64;
    let mut n_edges = 0i64;
    for f in &selected {
        let Ok(content) = tokio::fs::read_to_string(&f.abs_path).await else {
            continue;
        };
        let Ok(parsed) = parse(f.lang, &content) else {
            continue;
        };
        let file_id = files::upsert(conn, repo.id, &f.rel_path, Some(f.lang.as_str()), &fnv1a_hex(content.as_bytes()), f.mtime, parsed.loc).await?;
        let qual_ids = symbols::replace_file_symbols(conn, repo.id, file_id, &parsed.symbols).await?;
        let new_edges: Vec<NewEdge> = parsed
            .calls
            .iter()
            .filter_map(|c| {
                qual_ids.get(&c.caller_qual).map(|&caller_id| NewEdge {
                    caller_id,
                    callee_name: c.callee_name.clone(),
                    kind: c.kind,
                    source: c.source,
                    confidence: c.confidence,
                    evidence: c.evidence.clone(),
                    task_id: None,
                })
            })
            .collect();
        n_symbols += parsed.symbols.len() as i64;
        n_edges += new_edges.len() as i64;
        edges::insert(conn, repo.id, &new_edges).await?;
        enqueue_file_tasks(conn, repo.id, file_id).await?;
    }
    edges::resolve_by_name(conn, repo.id).await?;
    let outcome = IndexOutcome {
        run_id,
        files_scanned: selected.len() as i64,
        files_changed: selected.len() as i64,
        files_removed: 0,
        symbols: n_symbols,
        edges: n_edges,
        duration_ms: started.elapsed().as_millis() as i64,
    };
    runs::finish(conn, run_id, "ok", outcome.files_changed, n_symbols, n_edges, outcome.duration_ms).await?;
    Ok(outcome)
}

/// (Re-)enqueue hidden-call analysis for every function-like symbol of a file.
async fn enqueue_file_tasks(conn: &Connection, repo_id: i64, file_id: i64) -> anyhow::Result<()> {
    let (n, syms) = symbols::list(conn, repo_id, &symbols::SymbolFilter {
        kind: None,
        q: None,
        file_id: Some(file_id),
        limit: 10_000,
        offset: 0,
    })
    .await?;
    for s in syms.iter().filter(|s| matches!(s.kind, SymbolKind::Function | SymbolKind::Method | SymbolKind::Closure)) {
        tasks::enqueue(conn, repo_id, "hidden_calls", Some(s.id), None).await?;
    }
    tracing::debug!(file_id, fns = n, "analysis tasks enqueued");
    Ok(())
}

/// Ensure a repo exists in db (used by serve/watch flows).
pub async fn ensure_repo(conn: &Connection, path: &str) -> anyhow::Result<Repo> {
    let name = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    repos::upsert(conn, path, &name, None).await
}
