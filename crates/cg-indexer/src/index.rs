//! Full & incremental indexing: scan → parse → persist → resolve → enqueue.

use crate::scan::{scan_repo, ScannedFile};
use cg_core::model::*;
use cg_core::parser::{fnv1a_hex, parse};
use cg_db::{edges, files, repos, runs, symbols, tasks};
use libsql::Connection;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexOutcome {
    pub run_id: i64,
    pub files_scanned: i64,
    pub files_changed: i64,
    pub files_removed: i64,
    pub symbols: i64,
    pub edges: i64,
    pub symbols_total: i64,
    pub edges_total: i64,
    pub duration_ms: i64,
}

/// Progress callback payload (drives WS events).
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexProgress {
    pub run_id: i64,
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
    let started = Instant::now();
    let run_id = begin_run(conn, repo.id, trigger).await?;
    emit_initial(&mut progress, run_id);
    let result = index_repo_inner(conn, repo, full, run_id, started, &mut progress).await;
    finish_run(conn, run_id, started, result).await
}

/// Incremental: re-index exactly these paths (relative or absolute accepted).
pub async fn index_paths(
    conn: &Connection,
    repo: &Repo,
    paths: &[std::path::PathBuf],
    trigger: &str,
    mut progress: impl FnMut(IndexProgress) + Send,
) -> anyhow::Result<IndexOutcome> {
    let started = Instant::now();
    let run_id = begin_run(conn, repo.id, trigger).await?;
    emit_initial(&mut progress, run_id);
    let result = index_paths_inner(conn, repo, paths, run_id, started, &mut progress).await;
    finish_run(conn, run_id, started, result).await
}

async fn begin_run(conn: &Connection, repo_id: i64, trigger: &str) -> anyhow::Result<i64> {
    let run_id = runs::start(conn, repo_id, trigger).await?;
    // close out runs left 'running' by a previously killed process
    let _ = conn
        .execute(
            "UPDATE index_runs SET status = 'failed', finished_at = datetime('now')
             WHERE repo_id = ?1 AND status = 'running' AND id != ?2",
            libsql::params![repo_id, run_id],
        )
        .await;
    Ok(run_id)
}

fn emit_initial(progress: &mut impl FnMut(IndexProgress), run_id: i64) {
    progress(IndexProgress {
        run_id,
        done: 0,
        total: 0,
        current_file: String::new(),
    });
}

/// Finalize the run: success writes `ok`; any error writes `failed`.
async fn finish_run(
    conn: &Connection,
    run_id: i64,
    started: Instant,
    result: anyhow::Result<IndexOutcome>,
) -> anyhow::Result<IndexOutcome> {
    let duration_ms = started.elapsed().as_millis() as i64;
    match result {
        Ok(outcome) => {
            if let Err(e) = runs::finish(
                conn,
                run_id,
                "ok",
                outcome.files_changed,
                outcome.symbols,
                outcome.edges,
                outcome.duration_ms,
            )
            .await
            {
                mark_failed(conn, run_id, duration_ms).await;
                return Err(e);
            }
            Ok(outcome)
        }
        Err(e) => {
            mark_failed(conn, run_id, duration_ms).await;
            Err(e)
        }
    }
}

async fn mark_failed(conn: &Connection, run_id: i64, duration_ms: i64) {
    if let Err(e) = runs::finish(conn, run_id, "failed", 0, 0, 0, duration_ms).await {
        tracing::error!(run_id, error = %e, "failed to mark index run failed");
    }
}

async fn index_repo_inner(
    conn: &Connection,
    repo: &Repo,
    full: bool,
    run_id: i64,
    started: Instant,
    progress: &mut (impl FnMut(IndexProgress) + Send),
) -> anyhow::Result<IndexOutcome> {
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

    let total = changed.len() as i64;
    let mut parsed_ok = 0i64;
    let mut n_symbols = 0i64;
    let mut n_edges = 0i64;
    for (i, (f, content)) in changed.iter().enumerate() {
        progress(IndexProgress {
            run_id,
            done: i as i64,
            total,
            current_file: f.rel_path.clone(),
        });
        if let Some(counts) = persist_file(conn, repo.id, f, content).await? {
            parsed_ok += 1;
            n_symbols += counts.symbols;
            n_edges += counts.edges;
        }
    }

    // sweep files that disappeared from disk
    let keep: serde_json::Value = serde_json::json!(scanned
        .iter()
        .map(|f| f.rel_path.clone())
        .collect::<Vec<_>>());
    let removed = files::sweep_missing(conn, repo.id, &keep).await? as i64;

    // rebind resolvable edges repo-wide (cheap set-based update)
    edges::resolve_by_name(conn, repo.id).await?;
    let (symbols_total, edges_total) = graph_totals(conn, repo.id).await?;

    progress(IndexProgress {
        run_id,
        done: total,
        total,
        current_file: String::new(),
    });
    Ok(IndexOutcome {
        run_id,
        files_scanned: scanned.len() as i64,
        files_changed: parsed_ok,
        files_removed: removed,
        symbols: n_symbols,
        edges: n_edges,
        symbols_total,
        edges_total,
        duration_ms: started.elapsed().as_millis() as i64,
    })
}

async fn index_paths_inner(
    conn: &Connection,
    repo: &Repo,
    paths: &[std::path::PathBuf],
    run_id: i64,
    started: Instant,
    progress: &mut (impl FnMut(IndexProgress) + Send),
) -> anyhow::Result<IndexOutcome> {
    let root = std::path::Path::new(&repo.path);
    let all = scan_repo(root)?;
    let keep: serde_json::Value =
        serde_json::json!(all.iter().map(|f| f.rel_path.clone()).collect::<Vec<_>>());
    let wanted: Vec<String> = paths
        .iter()
        .map(|p| {
            p.strip_prefix(root)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    let selected: Vec<ScannedFile> = all
        .into_iter()
        .filter(|f| wanted.contains(&f.rel_path))
        .collect();
    let selected_count = selected.len() as i64;
    let removed = files::sweep_missing(conn, repo.id, &keep).await? as i64;
    let written = index_selected(conn, repo, &selected, run_id, progress).await?;
    edges::resolve_by_name(conn, repo.id).await?;
    let (symbols_total, edges_total) = graph_totals(conn, repo.id).await?;
    Ok(IndexOutcome {
        run_id,
        files_scanned: selected_count,
        files_changed: written.files_changed,
        files_removed: removed,
        symbols: written.symbols,
        edges: written.edges,
        symbols_total,
        edges_total,
        duration_ms: started.elapsed().as_millis() as i64,
    })
}

struct SelectedStats {
    files_changed: i64,
    symbols: i64,
    edges: i64,
}

/// Re-index only `selected` files (already resolved from disk). This is the
/// file-writing inner half of `index_paths`; run lifecycle is handled outside.
async fn index_selected(
    conn: &Connection,
    repo: &Repo,
    selected: &[ScannedFile],
    run_id: i64,
    progress: &mut (impl FnMut(IndexProgress) + Send),
) -> anyhow::Result<SelectedStats> {
    let total = selected.len() as i64;
    let mut stats = SelectedStats {
        files_changed: 0,
        symbols: 0,
        edges: 0,
    };
    for (i, f) in selected.iter().enumerate() {
        progress(IndexProgress {
            run_id,
            done: i as i64,
            total,
            current_file: f.rel_path.clone(),
        });
        let Ok(content) = tokio::fs::read_to_string(&f.abs_path).await else {
            continue;
        };
        let hash = fnv1a_hex(content.as_bytes());
        // Content unchanged since last parse: skip symbol rebuild so that
        // existing symbols, call edges and analysis tasks survive touch/atime
        // events and spurious watcher triggers.
        if let Some(prev) = files::get_by_path(conn, repo.id, &f.rel_path).await? {
            if prev.hash.as_deref() == Some(hash.as_str()) {
                continue;
            }
        }
        if let Some(counts) = persist_file(conn, repo.id, f, &content).await? {
            stats.files_changed += 1;
            stats.symbols += counts.symbols;
            stats.edges += counts.edges;
        }
    }
    progress(IndexProgress {
        run_id,
        done: total,
        total,
        current_file: String::new(),
    });
    Ok(stats)
}

struct FileWriteCounts {
    symbols: i64,
    edges: i64,
}

async fn persist_file(
    conn: &Connection,
    repo_id: i64,
    f: &ScannedFile,
    content: &str,
) -> anyhow::Result<Option<FileWriteCounts>> {
    let parsed = match parse(f.lang, content) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(file = %f.rel_path, error = %e, "parse failed");
            return Ok(None);
        }
    };
    let hash = fnv1a_hex(content.as_bytes());
    let file_id = files::upsert(
        conn,
        repo_id,
        &f.rel_path,
        Some(f.lang.as_str()),
        &hash,
        f.mtime,
        parsed.loc,
    )
    .await?;
    let qual_ids = symbols::replace_file_symbols(conn, repo_id, file_id, &parsed.symbols).await?;
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
    let edge_count = new_edges.len() as i64;
    edges::insert(conn, repo_id, &new_edges).await?;
    enqueue_file_tasks(conn, repo_id, file_id).await?;
    Ok(Some(FileWriteCounts {
        symbols: parsed.symbols.len() as i64,
        edges: edge_count,
    }))
}

/// Current (deduplicated) graph size for this repository.
async fn graph_totals(conn: &Connection, repo_id: i64) -> anyhow::Result<(i64, i64)> {
    let symbols_total = cg_db::count(
        conn,
        "SELECT COUNT(*) FROM symbols WHERE repo_id=?1",
        vec![repo_id.into()],
    )
    .await?;
    let edges_total = cg_db::count(
        conn,
        "SELECT COUNT(*) FROM call_edges WHERE repo_id=?1",
        vec![repo_id.into()],
    )
    .await?;
    Ok((symbols_total, edges_total))
}

/// (Re-)enqueue hidden-call analysis for every function-like symbol of a file.
async fn enqueue_file_tasks(conn: &Connection, repo_id: i64, file_id: i64) -> anyhow::Result<()> {
    let (n, syms) = symbols::list(
        conn,
        repo_id,
        &symbols::SymbolFilter {
            kind: None,
            q: None,
            file_id: Some(file_id),
            limit: 10_000,
            offset: 0,
        },
    )
    .await?;
    for s in syms.iter().filter(|s| {
        matches!(
            s.kind,
            SymbolKind::Function | SymbolKind::Method | SymbolKind::Closure
        )
    }) {
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
