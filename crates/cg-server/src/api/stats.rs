//! Dashboard statistics.

use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use cg_db::runs;

pub async fn repo_stats(State(state): State<AppState>, Path(id): Path<i64>) -> Result<Json<serde_json::Value>, StatusCode> {
    let db = &state.db;
    let err = || StatusCode::INTERNAL_SERVER_ERROR;
    let group = |col: &str, table: &str| {
        format!("SELECT {col} AS k, COUNT(*) AS n FROM {table} WHERE repo_id = ?1 GROUP BY {col}")
    };

    let symbols_by_kind = query_pairs(db, &group("kind", "symbols"), id).await.map_err(|_| err())?;
    let edges_by_kind = query_pairs(db, &group("kind", "call_edges"), id).await.map_err(|_| err())?;
    let edges_by_source = query_pairs(db, &group("source", "call_edges"), id).await.map_err(|_| err())?;
    let task_status = query_pairs(db, &group("status", "analysis_tasks"), id).await.map_err(|_| err())?;

    let totals = serde_json::json!({
        "files": cg_db::count(db, "SELECT COUNT(*) FROM files WHERE repo_id=?1", vec![id.into()]).await.map_err(|_| err())?,
        "symbols": cg_db::count(db, "SELECT COUNT(*) FROM symbols WHERE repo_id=?1", vec![id.into()]).await.map_err(|_| err())?,
        "edges": cg_db::count(db, "SELECT COUNT(*) FROM call_edges WHERE repo_id=?1", vec![id.into()]).await.map_err(|_| err())?,
        "functions": cg_db::count(db, "SELECT COUNT(*) FROM symbols WHERE repo_id=?1 AND kind IN ('function','method','closure')", vec![id.into()]).await.map_err(|_| err())?,
        "unresolved": cg_db::count(db, "SELECT COUNT(*) FROM call_edges WHERE repo_id=?1 AND source='llm' AND callee_id IS NULL", vec![id.into()]).await.map_err(|_| err())?,
    });
    let recent_runs = runs::recent(db, id, 10).await.map_err(|_| err())?;
    Ok(Json(serde_json::json!({
        "totals": totals,
        "symbols_by_kind": symbols_by_kind,
        "edges_by_kind": edges_by_kind,
        "edges_by_source": edges_by_source,
        "task_status": task_status,
        "recent_runs": recent_runs,
    })))
}

async fn query_pairs(db: &libsql::Connection, sql: &str, repo_id: i64) -> anyhow::Result<serde_json::Value> {
    let mut rows = db.query(sql, libsql::params![repo_id]).await?;
    let mut map = serde_json::Map::new();
    while let Some(row) = rows.next().await? {
        let k: String = row.get(0)?;
        let n: i64 = row.get(1)?;
        map.insert(k, serde_json::Value::from(n));
    }
    Ok(serde_json::Value::Object(map))
}
