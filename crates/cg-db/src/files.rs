//! File table access.

use anyhow::Result;
use cg_core::FileRow;
use libsql::{params, Connection};

fn map_file(row: &libsql::Row) -> Result<FileRow> {
    Ok(FileRow {
        id: row.get(0)?,
        repo_id: row.get(1)?,
        path: row.get(2)?,
        lang: row.get(3)?,
        hash: row.get(4)?,
        mtime: row.get(5)?,
        loc: row.get(6)?,
        parsed_at: row.get(7)?,
        symbol_count: row.get(8)?,
    })
}

const COLS: &str = "f.id, f.repo_id, f.path, f.lang, f.hash, f.mtime, f.loc, f.parsed_at, \
                    (SELECT COUNT(*) FROM symbols s WHERE s.file_id = f.id)";

/// Insert or refresh a parsed file; returns its id.
pub async fn upsert(
    conn: &Connection,
    repo_id: i64,
    path: &str,
    lang: Option<&str>,
    hash: &str,
    mtime: i64,
    loc: i64,
) -> Result<i64> {
    let mut rows = conn
        .query(
            "INSERT INTO files(repo_id, path, lang, hash, mtime, loc, parsed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
             ON CONFLICT(repo_id, path) DO UPDATE SET
                lang = excluded.lang, hash = excluded.hash, mtime = excluded.mtime,
                loc = excluded.loc, parsed_at = excluded.parsed_at
             RETURNING id",
            params![repo_id, path, lang, hash, mtime, loc],
        )
        .await?;
    let row = rows.next().await?.expect("upsert returns row");
    Ok(row.get(0)?)
}

pub async fn list(conn: &Connection, repo_id: i64) -> Result<Vec<FileRow>> {
    let mut rows = conn
        .query(
            &format!("SELECT {COLS} FROM files f WHERE f.repo_id = ?1 ORDER BY f.path"),
            params![repo_id],
        )
        .await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(map_file(&row)?);
    }
    Ok(out)
}

pub async fn get_by_path(conn: &Connection, repo_id: i64, path: &str) -> Result<Option<FileRow>> {
    let mut rows = conn
        .query(
            &format!("SELECT {COLS} FROM files f WHERE f.repo_id = ?1 AND f.path = ?2"),
            params![repo_id, path],
        )
        .await?;
    match rows.next().await? {
        Some(row) => Ok(Some(map_file(&row)?)),
        None => Ok(None),
    }
}

/// Delete files of a repo whose path is NOT in `keep` (json array of strings).
pub async fn sweep_missing(conn: &Connection, repo_id: i64, keep: &serde_json::Value) -> Result<u64> {
    Ok(conn.execute(
        "DELETE FROM files WHERE repo_id = ?1 AND path NOT IN (SELECT value FROM json_each(?2))",
        params![repo_id, keep.to_string()],
    )
    .await?)
}
