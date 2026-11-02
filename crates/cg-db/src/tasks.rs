//! Analysis task queue.

use anyhow::Result;
use cg_core::{TaskRow, TaskStatus};
use libsql::{params, Connection};

const COLS: &str = "id, repo_id, type, target_symbol_id, payload, status, attempts, result, error, created_at, updated_at";

fn map_task(row: &libsql::Row) -> Result<TaskRow> {
    let status: String = row.get(5)?;
    Ok(TaskRow {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        r#type: row.get(2)?,
        target_symbol_id: row.get(3)?,
        payload: row.get(4)?,
        status: TaskStatus::parse(&status).unwrap_or(TaskStatus::Pending),
        attempts: row.get(6)?,
        result: row.get(7)?,
        error: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

/// Enqueue (or re-arm) a task; unique key (repo, symbol, type) keeps it idempotent.
pub async fn enqueue(
    conn: &Connection,
    repo_id: i64,
    task_type: &str,
    target_symbol_id: Option<i64>,
    payload: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO analysis_tasks(repo_id, type, target_symbol_id, payload, status)
         VALUES (?1, ?2, ?3, ?4, 'pending')
         ON CONFLICT(repo_id, target_symbol_id, type) DO UPDATE SET
            status = 'pending', error = NULL, payload = excluded.payload,
            updated_at = datetime('now')",
        params![repo_id, task_type, target_symbol_id, payload],
    )
    .await?;
    Ok(())
}

/// Atomically claim up to `limit` pending tasks (marks them running).
/// `repo_id = None` claims across all repos (daemon scheduler).
pub async fn claim(conn: &Connection, repo_id: Option<i64>, limit: i64) -> Result<Vec<TaskRow>> {
    let (cond, p): (&str, Vec<libsql::Value>) = match repo_id {
        Some(id) => ("repo_id = ?1", vec![id.into()]),
        None => ("1=1", Vec::new()),
    };
    let mut p = p;
    p.push(libsql::Value::Integer(limit));
    let mut rows = conn
        .query(
            &format!(
                "UPDATE analysis_tasks SET status = 'running', updated_at = datetime('now')
                 WHERE id IN (
                     SELECT id FROM analysis_tasks
                     WHERE {cond} AND status = 'pending'
                     ORDER BY id LIMIT ?{}
                 )
                 RETURNING id, repo_id, type, target_symbol_id, payload, status, attempts, result, error, created_at, updated_at",
                p.len()
            ),
            libsql::params_from_iter(p),
        )
        .await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(map_task(&row)?);
    }
    Ok(out)
}

pub async fn complete(conn: &Connection, id: i64, result: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE analysis_tasks SET status = 'done', result = ?2, error = NULL, updated_at = datetime('now') WHERE id = ?1",
        params![id, result],
    )
    .await?;
    Ok(())
}

/// Fail an attempt; requeue with backoff until max attempts reached.
pub async fn fail(conn: &Connection, id: i64, error: &str, max_attempts: i64) -> Result<TaskStatus> {
    conn.execute(
        "UPDATE analysis_tasks SET
            attempts = attempts + 1,
            status = CASE WHEN attempts + 1 >= ?2 THEN 'failed' ELSE 'pending' END,
            error = ?3, updated_at = datetime('now')
         WHERE id = ?1",
        params![id, max_attempts, error],
    )
    .await?;
    let mut rows = conn.query("SELECT status FROM analysis_tasks WHERE id = ?1", params![id]).await?;
    let status: Option<String> = rows.next().await?.map(|r| r.get(0)).transpose()?.flatten();
    Ok(TaskStatus::parse(&status.unwrap_or_else(|| "failed".into())).unwrap_or(TaskStatus::Failed))
}

/// Requeue tasks stuck in `running` (e.g. after a crashed worker process).
/// Returns the number of requeued rows.
pub async fn reset_running(conn: &Connection) -> Result<u64> {
    Ok(conn.execute(
        "UPDATE analysis_tasks SET status = 'pending', updated_at = datetime('now') WHERE status = 'running'",
        params![],
    )
    .await?)
}

pub async fn retry(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE analysis_tasks SET status = 'pending', attempts = 0, error = NULL, updated_at = datetime('now') WHERE id = ?1",
        params![id],
    )
    .await?;
    Ok(())
}

pub async fn list(conn: &Connection, repo_id: i64, status: Option<TaskStatus>, limit: i64) -> Result<Vec<TaskRow>> {
    let (cond, mut p): (&str, Vec<libsql::Value>) = match status {
        Some(s) => ("repo_id = ?1 AND status = ?2", vec![repo_id.into(), s.as_str().into()]),
        None => ("repo_id = ?1", vec![repo_id.into()]),
    };
    p.push(libsql::Value::Integer(limit));
    let mut rows = conn
        .query(
            &format!(
                "SELECT {COLS} FROM analysis_tasks WHERE {cond}
                 ORDER BY CASE status WHEN 'running' THEN 0 WHEN 'pending' THEN 1 ELSE 2 END, id DESC LIMIT ?{}",
                p.len()
            ),
            libsql::params_from_iter(p),
        )
        .await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(map_task(&row)?);
    }
    Ok(out)
}
