//! Embedded (local) mode: commands run against a local/remote db directly.

use anyhow::{anyhow, Context, Result};
use cg_core::model::Repo;
use cg_db::{edges, files, repos, runs, symbols, tasks};
use libsql::Connection;

pub async fn open(db_path: &std::path::Path) -> Result<Connection> {
    let conn = cg_db::open_env(db_path).await?;
    cg_db::migrate(&conn).await?;
    Ok(conn)
}

pub async fn add_repo(conn: &Connection, path: &str, name: Option<&str>, git_url: Option<&str>) -> Result<Repo> {
    let canonical = std::fs::canonicalize(path)
        .with_context(|| format!("repo path {path} not found"))?
        .to_string_lossy()
        .to_string();
    let name = name
        .map(str::to_string)
        .unwrap_or_else(|| std::path::Path::new(path).file_name().unwrap().to_string_lossy().to_string());
    repos::upsert(conn, &canonical, &name, git_url).await
}

pub async fn resolve_repo(conn: &Connection, sel: Option<&str>) -> Result<Repo> {
    let list = repos::list(conn).await?;
    match sel {
        Some(s) => list
            .into_iter()
            .map(|r| r.repo)
            .find(|r| r.path == s || r.name == s || r.id.to_string() == s)
            .ok_or_else(|| anyhow!("repo '{s}' not found")),
        None => list
            .into_iter()
            .map(|r| r.repo)
            .next()
            .ok_or_else(|| anyhow!("no repositories; add one with `repo add <path>`")),
    }
}

pub async fn index(conn: &Connection, repo: &Repo, full: bool) -> Result<cg_indexer::IndexOutcome> {
    let progress = |p: cg_indexer::IndexProgress| {
        eprintln!("\r[{}/{}] {}", p.done, p.total, p.current_file);
    };
    cg_indexer::index_repo(conn, repo, full, "cli", progress).await
}

pub async fn analyze(conn: &Connection, limit: usize) -> Result<Vec<cg_analyzer::TaskSummary>> {
    let client = cg_analyzer::OpencodeClient::from_env();
    let cfg = cg_analyzer::AnalyzerConfig::from_env();
    let _ = cg_db::tasks::reset_running(conn).await?;
    cg_analyzer::run_once(conn, &client, &cfg, limit).await
}

pub async fn query_files(conn: &Connection, repo: &Repo) -> Result<serde_json::Value> {
    Ok(serde_json::to_value(files::list(conn, repo.id).await?)?)
}

pub async fn query_symbols(
    conn: &Connection,
    repo: &Repo,
    kind: Option<&str>,
    q: Option<&str>,
) -> Result<serde_json::Value> {
    let filter = symbols::SymbolFilter {
        kind: kind.and_then(cg_core::model::SymbolKind::parse),
        q,
        file_id: None,
        limit: 500,
        offset: 0,
    };
    let (_, list) = symbols::list(conn, repo.id, &filter).await?;
    Ok(serde_json::to_value(list)?)
}

pub async fn query_calls(conn: &Connection, _repo: &Repo, symbol_id: i64) -> Result<serde_json::Value> {
    let sym = symbols::get(conn, symbol_id)
        .await?
        .ok_or_else(|| anyhow!("symbol {symbol_id} not found"))?;
    let callees = edges::for_caller(conn, symbol_id).await?;
    let callers = edges::for_callee(conn, symbol_id).await?;
    Ok(serde_json::json!({ "symbol": sym, "callees": callees, "callers": callers }))
}

pub async fn file_show(conn: &Connection, repo: &Repo, rel: &str) -> Result<serde_json::Value> {
    let f = files::get_by_path(conn, repo.id, rel)
        .await?
        .ok_or_else(|| anyhow!("file {rel} not indexed in repo {}", repo.name))?;
    let abs = std::path::Path::new(&repo.path).join(&f.path);
    let content = tokio::fs::read_to_string(&abs).await?;
    let (_, syms) = symbols::list(conn, repo.id, &symbols::SymbolFilter {
        kind: None, q: None, file_id: Some(f.id), limit: 1000, offset: 0 }).await?;
    Ok(serde_json::json!({ "file": f, "symbols": syms, "content": content }))
}

pub async fn tasks_list(conn: &Connection, repo: &Repo, status: Option<&str>) -> Result<serde_json::Value> {
    let st = status.and_then(cg_core::model::TaskStatus::parse);
    Ok(serde_json::to_value(tasks::list(conn, repo.id, st, 100).await?)?)
}

#[allow(dead_code)]
pub async fn run_status(conn: &Connection, repo: &Repo) -> Result<serde_json::Value> {
    Ok(serde_json::to_value(runs::recent(conn, repo.id, 5).await?)?)
}

/// Local watch loop (no HTTP server): re-index on file changes.
pub async fn watch(conn: Connection, repo: Repo) -> Result<()> {
    let mut rx = cg_indexer::spawn_watcher(std::path::Path::new(&repo.path), std::time::Duration::from_secs(2))?;
    println!("watching {} (ctrl-c to stop)", repo.path);
    loop {
        let mut batch = vec![rx.recv().await.ok_or_else(|| anyhow!("watcher stopped"))?];
        while let Ok(p) = rx.try_recv() {
            batch.push(p);
        }
        batch.dedup();
        let relevant: Vec<std::path::PathBuf> = batch
            .into_iter()
            .filter(|p| cg_core::lang::lang_for_path(p).is_some())
            .collect();
        if relevant.is_empty() {
            continue;
        }
        match cg_indexer::index_paths(&conn, &repo, &relevant, "watch").await {
            Ok(o) => println!("re-indexed {} files (+{} symbols, +{} edges)", o.files_changed, o.symbols, o.edges),
            Err(e) => eprintln!("reindex failed: {e:#}"),
        }
    }
}
