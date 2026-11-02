//! Symbol table access.

use std::collections::HashMap;

use anyhow::Result;
use cg_core::{ParsedSymbol, SymbolKind, SymbolRow};
use libsql::{params, Connection};

const COLS: &str = "s.id, s.repo_id, s.file_id, s.kind, s.name, s.qual_path, s.start_line, s.end_line, \
                    s.parent_id, s.is_pub, f.path";

/// Positional params: `None` binds NULL so indices stay stable.
fn positional(vals: Vec<Option<libsql::Value>>) -> Vec<libsql::Value> {
    vals.into_iter().map(|v| v.unwrap_or(libsql::Value::Null)).collect()
}

fn map_symbol(row: &libsql::Row) -> Result<SymbolRow> {
    let kind: String = row.get(3)?;
    Ok(SymbolRow {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        file_id: row.get(2)?,
        kind: SymbolKind::parse(&kind).unwrap_or(SymbolKind::Function),
        name: row.get(4)?,
        qual_path: row.get(5)?,
        start_line: row.get(6)?,
        end_line: row.get(7)?,
        parent_id: row.get(8)?,
        is_pub: row.get::<i64>(9)? != 0,
        file_path: row.get(10)?,
    })
}

/// Replace all symbols of a file with `syms` (pre-order: parents before children).
/// Returns qual_path -> symbol id mapping for edge construction.
pub async fn replace_file_symbols(
    conn: &Connection,
    repo_id: i64,
    file_id: i64,
    syms: &[ParsedSymbol],
) -> Result<HashMap<String, i64>> {
    conn.execute("DELETE FROM symbols WHERE file_id = ?1", params![file_id])
        .await?;
    let mut ids: HashMap<String, i64> = HashMap::with_capacity(syms.len());
    for s in syms {
        let parent_id = s.parent_qual.as_ref().and_then(|q| ids.get(q).copied());
        let mut rows = conn
            .query(
                "INSERT INTO symbols(repo_id, file_id, kind, name, qual_path, start_line, end_line, parent_id, is_pub)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(repo_id, qual_path) DO UPDATE SET
                    file_id = excluded.file_id, kind = excluded.kind, name = excluded.name,
                    start_line = excluded.start_line, end_line = excluded.end_line,
                    parent_id = excluded.parent_id, is_pub = excluded.is_pub
                 RETURNING id",
                params![
                    repo_id,
                    file_id,
                    s.kind.as_str(),
                    s.name.as_str(),
                    s.qual_path.as_str(),
                    s.start_line,
                    s.end_line,
                    parent_id,
                    s.is_pub as i64
                ],
            )
            .await?;
        let row = rows.next().await?.expect("insert returns id");
        let id: i64 = row.get(0)?;
        ids.insert(s.qual_path.clone(), id);
    }
    Ok(ids)
}

pub struct SymbolFilter<'a> {
    pub kind: Option<SymbolKind>,
    pub q: Option<&'a str>,
    pub file_id: Option<i64>,
    pub limit: i64,
    pub offset: i64,
}

pub async fn list(conn: &Connection, repo_id: i64, f: &SymbolFilter<'_>) -> Result<(i64, Vec<SymbolRow>)> {
    let mut where_clauses = vec!["s.repo_id = ?1".to_string()];
    if f.kind.is_some() {
        where_clauses.push("s.kind = ?2".into());
    }
    if f.q.is_some() {
        where_clauses.push("(s.name LIKE ?3 OR s.qual_path LIKE ?3)".into());
    }
    if f.file_id.is_some() {
        where_clauses.push("s.file_id = ?4".into());
    }
    let cond = where_clauses.join(" AND ");
    let total = crate::count(
        conn,
        &format!("SELECT COUNT(*) FROM symbols s WHERE {cond}"),
        positional(vec![
            Some(repo_id.into()),
            f.kind.map(|k| k.as_str().into()),
            f.q.map(|q| format!("%{q}%").into()),
            f.file_id.map(|v| v.into()),
        ]),
    )
    .await?;
    let mut rows = conn
        .query(
            &format!(
                "SELECT {COLS} FROM symbols s JOIN files f ON f.id = s.file_id
                 WHERE {cond} ORDER BY s.qual_path LIMIT ?5 OFFSET ?6"
            ),
            positional(vec![
                Some(repo_id.into()),
                f.kind.map(|k| k.as_str().into()),
                f.q.map(|q| format!("%{q}%").into()),
                f.file_id.map(|v| v.into()),
                Some(f.limit.into()),
                Some(f.offset.into()),
            ]),
        )
        .await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(map_symbol(&row)?);
    }
    Ok((total, out))
}

pub async fn get(conn: &Connection, id: i64) -> Result<Option<SymbolRow>> {
    let mut rows = conn
        .query(
            &format!("SELECT {COLS} FROM symbols s JOIN files f ON f.id = s.file_id WHERE s.id = ?1"),
            params![id],
        )
        .await?;
    match rows.next().await? {
        Some(row) => Ok(Some(map_symbol(&row)?)),
        None => Ok(None),
    }
}

/// Fuzzy candidate search used to build LLM prompts (small, ranked by name length).
pub async fn search_candidates(
    conn: &Connection,
    repo_id: i64,
    q: &str,
    limit: i64,
) -> Result<Vec<SymbolRow>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {COLS} FROM symbols s JOIN files f ON f.id = s.file_id
                 WHERE s.repo_id = ?1 AND s.kind IN ('function','method','closure')
                   AND (s.name LIKE ?2 OR s.qual_path LIKE ?2)
                 ORDER BY LENGTH(s.name) LIMIT ?3"
            ),
            params![repo_id, format!("%{q}%"), limit],
        )
        .await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(map_symbol(&row)?);
    }
    Ok(out)
}

/// Function-like symbols of a repo (analysis targets).
pub async fn list_functions(conn: &Connection, repo_id: i64, limit: i64, offset: i64) -> Result<Vec<SymbolRow>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {COLS} FROM symbols s JOIN files f ON f.id = s.file_id
                 WHERE s.repo_id = ?1 AND s.kind IN ('function','method','closure')
                 ORDER BY s.id LIMIT ?2 OFFSET ?3"
            ),
            params![repo_id, limit, offset],
        )
        .await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(map_symbol(&row)?);
    }
    Ok(out)
}
