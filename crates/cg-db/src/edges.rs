//! Call-edge access + name-based resolution.

use anyhow::Result;
use cg_core::{EdgeKind, EdgeRow, EdgeSource, NewEdge};
use libsql::{params, Connection};

const COLS: &str = "e.id, e.repo_id, e.caller_id, e.callee_id, e.callee_name, e.kind, e.source, \
                    e.confidence, e.evidence, e.task_id";

fn map_edge(row: &libsql::Row) -> Result<EdgeRow> {
    let kind: String = row.get(5)?;
    let source: String = row.get(6)?;
    Ok(EdgeRow {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        caller_id: row.get(2)?,
        callee_id: row.get(3)?,
        callee_name: row.get(4)?,
        kind: EdgeKind::parse(&kind).unwrap_or(EdgeKind::DirectFn),
        source: if source == "llm" { EdgeSource::Llm } else { EdgeSource::Ast },
        confidence: row.get(7)?,
        evidence: row.get(8)?,
        task_id: row.get(9)?,
    })
}

/// Insert edges (dedup on caller+name+kind+source via upsert).
pub async fn insert(conn: &Connection, repo_id: i64, edges: &[NewEdge]) -> Result<u64> {
    let mut n = 0;
    for e in edges {
        n += conn
            .execute(
                "INSERT INTO call_edges(repo_id, caller_id, callee_name, kind, source, confidence, evidence, task_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(caller_id, callee_name, kind, source) DO UPDATE SET
                    confidence = excluded.confidence,
                    evidence = excluded.evidence,
                    task_id = excluded.task_id",
                params![
                    repo_id,
                    e.caller_id,
                    e.callee_name.as_str(),
                    e.kind.as_str(),
                    e.source.as_str(),
                    e.confidence,
                    e.evidence.as_deref(),
                    e.task_id
                ],
            )
            .await?;
    }
    Ok(n)
}

/// Resolve callee_id by unique function-name match for name-resolvable edges.
/// Idempotent; run after symbol replacements.
pub async fn resolve_by_name(conn: &Connection, repo_id: i64) -> Result<u64> {
    Ok(conn.execute(
        "UPDATE call_edges SET callee_id = (
             SELECT s.id FROM symbols s
             WHERE s.repo_id = call_edges.repo_id
               AND s.name = call_edges.callee_name
               AND s.kind IN ('function','method','closure')
         )
         WHERE repo_id = ?1 AND callee_id IS NULL
           AND kind IN ('direct_fn','callback','fn_ptr','macro')
           AND (
             SELECT COUNT(*) FROM symbols s2
             WHERE s2.repo_id = ?1
               AND s2.name = call_edges.callee_name
               AND s2.kind IN ('function','method','closure')
           ) = 1",
        params![repo_id],
    )
    .await?)
}

pub async fn for_caller(conn: &Connection, caller_id: i64) -> Result<Vec<EdgeRow>> {
    query_edges(conn, "e.caller_id = ?1 ORDER BY e.id", vec![caller_id.into()]).await
}

pub async fn for_callee(conn: &Connection, callee_id: i64) -> Result<Vec<EdgeRow>> {
    query_edges(conn, "e.callee_id = ?1 ORDER BY e.id", vec![callee_id.into()]).await
}

async fn query_edges(conn: &Connection, cond: &str, params: Vec<libsql::Value>) -> Result<Vec<EdgeRow>> {
    let mut rows = conn
        .query(&format!("SELECT {COLS} FROM call_edges e WHERE {cond}"), libsql::params_from_iter(params))
        .await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(map_edge(&row)?);
    }
    Ok(out)
}

/// LLM-proposed edges that failed symbol validation: awaiting human review.
pub async fn unresolved(conn: &Connection, repo_id: i64, limit: i64) -> Result<Vec<EdgeRow>> {
    query_edges(
        conn,
        "e.repo_id = ?1 AND e.source = 'llm' AND e.callee_id IS NULL ORDER BY e.id DESC LIMIT ?2",
        vec![repo_id.into(), limit.into()],
    )
    .await
}

/// Human confirms an unresolved edge by binding it to a symbol.
pub async fn confirm(conn: &Connection, edge_id: i64, callee_id: i64) -> Result<()> {
    conn.execute(
        "UPDATE call_edges SET callee_id = ?2, confidence = 1.0 WHERE id = ?1 AND source = 'llm'",
        params![edge_id, callee_id],
    )
    .await?;
    Ok(())
}

pub async fn dismiss(conn: &Connection, edge_id: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM call_edges WHERE id = ?1 AND source = 'llm' AND callee_id IS NULL",
        params![edge_id],
    )
    .await?;
    Ok(())
}

/// Edges touching any of `symbol_ids` (both directions).
pub async fn graph_edges(conn: &Connection, symbol_ids: &[i64]) -> Result<Vec<EdgeRow>> {
    let ids = serde_json::json!(symbol_ids).to_string();
    query_edges(
        conn,
        "e.caller_id IN (SELECT value FROM json_each(?1)) OR e.callee_id IN (SELECT value FROM json_each(?1)) ORDER BY e.id",
        vec![ids.into()],
    )
    .await
}
