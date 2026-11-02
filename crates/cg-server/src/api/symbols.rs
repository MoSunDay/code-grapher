//! Symbol endpoints: list, detail (callers/callees), subgraph.

use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use cg_core::model::SymbolKind;
use cg_db::{edges, symbols};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ListQuery {
    pub kind: Option<String>,
    pub q: Option<String>,
    pub file_id: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let filter = symbols::SymbolFilter {
        kind: q.kind.as_deref().and_then(SymbolKind::parse),
        q: q.q.as_deref(),
        file_id: q.file_id,
        limit: q.limit.unwrap_or(50).clamp(1, 500),
        offset: q.offset.unwrap_or(0).max(0),
    };
    let (total, list) = symbols::list(&state.db, id, &filter)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "total": total, "symbols": list })))
}

pub async fn detail(State(state): State<AppState>, Path(id): Path<i64>) -> Result<Json<serde_json::Value>, StatusCode> {
    let sym = symbols::get(&state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let callees = edges::for_caller(&state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let callers = edges::for_callee(&state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let callee_views = join_counterparts(&state.db, &callees, false).await;
    let caller_views = join_counterparts(&state.db, &callers, true).await;
    Ok(Json(serde_json::json!({
        "symbol": sym,
        "callees": callee_views,
        "callers": caller_views,
    })))
}

/// Attach the resolved counterpart symbol to each edge view.
async fn join_counterparts(
    db: &libsql::Connection,
    list: &[cg_core::model::EdgeRow],
    is_caller_side: bool,
) -> Vec<serde_json::Value> {
    let mut out = Vec::with_capacity(list.len());
    for e in list {
        let other_id = if is_caller_side { Some(e.caller_id) } else { e.callee_id };
        let other = match other_id {
            Some(oid) => symbols::get(db, oid).await.ok().flatten(),
            None => None,
        };
        let mut v = serde_json::to_value(e).unwrap_or_default();
        let key = if is_caller_side { "caller" } else { "callee" };
        v.as_object_mut().map(|m| m.insert(key.into(), serde_json::to_value(other).unwrap_or(serde_json::Value::Null)));
        out.push(v);
    }
    out
}

#[derive(Deserialize)]
pub struct GraphQuery {
    pub depth: Option<i64>,
}

/// BFS subgraph around a symbol: both directions, unresolved edges kept.
pub async fn graph(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<GraphQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let depth = q.depth.unwrap_or(2).clamp(1, 5) as usize;
    let root = symbols::get(&state.db, id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut frontier: Vec<i64> = vec![id];
    let mut node_ids: Vec<i64> = vec![id];
    let mut all_edges: Vec<cg_core::model::EdgeRow> = Vec::new();
    for _ in 0..depth {
        let batch = edges::graph_edges(&state.db, &frontier)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let mut next: Vec<i64> = Vec::new();
        for e in batch {
            for oid in [Some(e.caller_id), e.callee_id].into_iter().flatten() {
                if !node_ids.contains(&oid) {
                    node_ids.push(oid);
                    next.push(oid);
                }
            }
            if !all_edges.iter().any(|prev| prev.id == e.id) {
                all_edges.push(e);
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }

    let mut nodes = Vec::with_capacity(node_ids.len());
    for nid in &node_ids {
        if let Some(s) = symbols::get(&state.db, *nid).await.unwrap_or(None) {
            nodes.push(serde_json::json!({
                "id": s.id, "name": s.name, "kind": s.kind.as_str(),
                "qual_path": s.qual_path, "file": s.file_path,
            }));
        }
    }
    let edge_views: Vec<serde_json::Value> = all_edges
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id, "source_id": e.caller_id, "target_id": e.callee_id,
                "callee_name": e.callee_name, "kind": e.kind.as_str(),
                "source": e.source.as_str(), "confidence": e.confidence,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({ "nodes": nodes, "edges": edge_views, "root": root.id })))
}
