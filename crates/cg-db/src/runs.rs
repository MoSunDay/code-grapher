//! Index run bookkeeping.

use anyhow::Result;
use cg_core::RunRow;
use libsql::{params, Connection};

const COLS: &str = "id, repo_id, started_at, finished_at, trigger, files_changed, symbols, edges, duration_ms, status";

fn map_run(row: &libsql::Row) -> Result<RunRow> {
    Ok(RunRow {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        started_at: row.get(2)?,
        finished_at: row.get(3)?,
        trigger: row.get(4)?,
        files_changed: row.get(5)?,
        symbols: row.get(6)?,
        edges: row.get(7)?,
        duration_ms: row.get(8)?,
        status: row.get(9)?,
    })
}

pub async fn start(conn: &Connection, repo_id: i64, trigger: &str) -> Result<i64> {
    let mut rows = conn
        .query(
            "INSERT INTO index_runs(repo_id, trigger, status) VALUES (?1, ?2, 'running') RETURNING id",
            params![repo_id, trigger],
        )
        .await?;
    let row = rows.next().await?.expect("insert returns id");
    Ok(row.get(0)?)
}

pub async fn finish(
    conn: &Connection,
    id: i64,
    status: &str,
    files_changed: i64,
    symbols: i64,
    edges: i64,
    duration_ms: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE index_runs SET finished_at = datetime('now'), status = ?2, files_changed = ?3,
            symbols = ?4, edges = ?5, duration_ms = ?6 WHERE id = ?1",
        params![id, status, files_changed, symbols, edges, duration_ms],
    )
    .await?;
    Ok(())
}

pub async fn recent(conn: &Connection, repo_id: i64, limit: i64) -> Result<Vec<RunRow>> {
    let mut rows = conn
        .query(
            &format!("SELECT {COLS} FROM index_runs WHERE repo_id = ?1 ORDER BY id DESC LIMIT ?2"),
            params![repo_id, limit],
        )
        .await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(map_run(&row)?);
    }
    Ok(out)
}

pub async fn last(conn: &Connection, repo_id: i64) -> Result<Option<RunRow>> {
    let mut rows = conn
        .query(
            &format!("SELECT {COLS} FROM index_runs WHERE repo_id = ?1 ORDER BY id DESC LIMIT 1"),
            params![repo_id],
        )
        .await?;
    match rows.next().await? {
        Some(row) => Ok(Some(map_run(&row)?)),
        None => Ok(None),
    }
}
