//! Analysis task execution: worker pool + retry + edge validation.

use crate::context::build_context;
use crate::opencode::OpencodeClient;
use crate::prompt::{build_prompt, parse_reply};
use anyhow::Result;
use cg_core::model::{EdgeSource, NewEdge, SymbolRow, TaskRow};
use cg_db::{edges, symbols, tasks};
use libsql::Connection;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    pub concurrency: usize,
    pub max_attempts: i64,
    pub request_timeout: std::time::Duration,
    pub poll_interval: std::time::Duration,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        AnalyzerConfig {
            concurrency: 2,
            max_attempts: 3,
            request_timeout: std::time::Duration::from_secs(180),
            poll_interval: std::time::Duration::from_secs(2),
        }
    }
}

impl AnalyzerConfig {
    pub fn from_env() -> Self {
        let mut cfg = AnalyzerConfig::default();
        if let Ok(v) = std::env::var("CG_ANALYZE_CONCURRENCY") {
            if let Ok(n) = v.parse::<usize>() {
                cfg.concurrency = n.clamp(1, 16);
            }
        }
        if let Ok(v) = std::env::var("CG_ANALYZE_TIMEOUT_SECS") {
            if let Ok(n) = v.parse::<u64>() {
                cfg.request_timeout = std::time::Duration::from_secs(n.clamp(10, 3600));
            }
        }
        cfg
    }
}

#[derive(Debug, Serialize)]
pub struct TaskSummary {
    pub task_id: i64,
    pub symbol: String,
    pub calls_found: usize,
    pub resolved: usize,
    pub unresolved: usize,
}

/// Execute one hidden_calls task: prompt LLM, validate, persist edges.
pub async fn run_task(conn: &Connection, client: &OpencodeClient, cfg: &AnalyzerConfig, task: &TaskRow) -> Result<TaskSummary> {
    let symbol_id = task
        .target_symbol_id
        .ok_or_else(|| anyhow::anyhow!("task {} has no target symbol", task.id))?;
    let sym = symbols::get(conn, symbol_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("symbol {symbol_id} vanished"))?;
    let repo = cg_db::repos::get(conn, task.repo_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("repo {} vanished", task.repo_id))?;

    let ctx = build_context(conn, &sym, &repo.path).await?;
    let prompt = build_prompt(&ctx);
    let reply = client
        .complete(&format!("cg-hidden-calls:{}", sym.qual_path), &prompt, cfg.request_timeout)
        .await?;
    let calls = parse_reply(&reply);

    let mut resolved = 0usize;
    let mut unresolved = 0usize;
    for c in &calls {
        let Some(kind) = c.edge_kind() else { continue };
        let callee_id = unique_symbol_by_name(conn, task.repo_id, &c.callee_name).await;
        if callee_id.is_some() {
            resolved += 1;
        } else {
            unresolved += 1;
        }
        let evidence = c
            .evidence
            .clone()
            .or_else(|| c.file.clone().map(|f| format!("{f}:{}", c.line.unwrap_or(0))));
        edges::insert(
            conn,
            task.repo_id,
            &[NewEdge {
                caller_id: symbol_id,
                callee_name: c.callee_name.clone(),
                kind,
                source: EdgeSource::Llm,
                confidence: c.confidence.clamp(0.0, 1.0),
                evidence,
                task_id: Some(task.id),
            }],
        )
        .await?;
    }
    // bind name-resolvable edges (unique function/method/closure name match)
    edges::resolve_by_name(conn, task.repo_id).await?;
    let summary = TaskSummary {
        task_id: task.id,
        symbol: sym.qual_path,
        calls_found: calls.len(),
        resolved,
        unresolved,
    };
    tasks::complete(conn, task.id, Some(&serde_json::to_string(&summary)?)).await?;
    Ok(summary)
}

async fn unique_symbol_by_name(conn: &Connection, repo_id: i64, name: &str) -> Option<i64> {
    let hits = symbols::search_candidates(conn, repo_id, name, 10)
        .await
        .unwrap_or_default();
    let exact: Vec<&SymbolRow> = hits.iter().filter(|s| s.name == name || s.qual_path == name).collect();
    match exact.as_slice() {
        [only] => Some(only.id),
        _ => None,
    }
}

/// Long-running daemon loop: claims pending tasks (all repos) and executes
/// them with bounded concurrency. Stops never; abort the JoinHandle to stop.
pub fn spawn_scheduler(conn: Connection, client: OpencodeClient, cfg: AnalyzerConfig) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let batch = tasks::claim(&conn, None, cfg.concurrency as i64)
                .await
                .unwrap_or_default();
            if batch.is_empty() {
                tokio::time::sleep(cfg.poll_interval).await;
                continue;
            }
            let mut handles = Vec::new();
            for task in batch {
                let conn = conn.clone();
                let client = client.clone();
                let cfg = cfg.clone();
                handles.push(tokio::spawn(async move {
                    match run_task(&conn, &client, &cfg, &task).await {
                        Ok(s) => tracing::info!(
                            task = s.task_id, symbol = %s.symbol,
                            found = s.calls_found, resolved = s.resolved, "analysis done"
                        ),
                        Err(e) => {
                            tracing::warn!(task = task.id, error = %e, "analysis failed");
                            let _ = tasks::fail(&conn, task.id, &format!("{e:#}"), cfg.max_attempts).await;
                        }
                    }
                }));
            }
            for h in handles {
                let _ = h.await;
            }
        }
    })
}

/// One-shot mode (CLI `analyze`): drain up to `max_tasks` pending tasks.
pub async fn run_once(
    conn: &Connection,
    client: &OpencodeClient,
    cfg: &AnalyzerConfig,
    max_tasks: usize,
) -> Result<Vec<TaskSummary>> {
    let mut summaries = Vec::new();
    let mut done = 0usize;
    while done < max_tasks {
        let batch = tasks::claim(conn, None, cfg.concurrency as i64).await?;
        if batch.is_empty() {
            break;
        }
        let take = (max_tasks - done).min(batch.len());
        let mut handles = Vec::with_capacity(take);
        for task in batch.into_iter().take(take) {
            let conn = conn.clone();
            let client = client.clone();
            let cfg = cfg.clone();
            handles.push(tokio::spawn(async move {
                match run_task(&conn, &client, &cfg, &task).await {
                    Ok(s) => Ok(s),
                    Err(e) => {
                        tracing::warn!(task = task.id, error = %e, "task failed");
                        let _ = tasks::fail(&conn, task.id, &format!("{e:#}"), cfg.max_attempts).await;
                        Err(task.id)
                    }
                }
            }));
        }
        for h in handles {
            match h.await {
                Ok(Ok(s)) => summaries.push(s),
                _ => {}
            }
            done += 1;
        }
    }
    Ok(summaries)
}

/// Enqueue hidden_calls tasks for every function-like symbol of a repo.
pub async fn enqueue_all(conn: &Connection, repo_id: i64, limit: i64) -> Result<usize> {
    let mut n = 0usize;
    let mut offset = 0i64;
    loop {
        let fns = symbols::list_functions(conn, repo_id, 500, offset).await?;
        if fns.is_empty() {
            break;
        }
        for s in &fns {
            tasks::enqueue(conn, repo_id, "hidden_calls", Some(s.id), None).await?;
            n += 1;
        }
        offset += fns.len() as i64;
        if (n as i64) >= limit {
            break;
        }
    }
    Ok(n)
}
