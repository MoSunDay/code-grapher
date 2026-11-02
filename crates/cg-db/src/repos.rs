//! Repository CRUD + aggregate stats.

use crate::count;
use anyhow::Result;
use cg_core::Repo;
use libsql::{params, Connection};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RepoStats {
    pub files: i64,
    pub symbols: i64,
    pub edges: i64,
    pub pending_tasks: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoWithStats {
    #[serde(flatten)]
    pub repo: Repo,
    pub stats: RepoStats,
}

fn map_repo(row: &libsql::Row) -> Result<Repo> {
    Ok(Repo {
        id: row.get(0)?,
        path: row.get(1)?,
        name: row.get(2)?,
        git_url: row.get(3)?,
        created_at: row.get(4)?,
    })
}

pub async fn upsert(conn: &Connection, path: &str, name: &str, git_url: Option<&str>) -> Result<Repo> {
    let mut rows = conn
        .query(
            "INSERT INTO repositories(path, name, git_url) VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO UPDATE SET name = excluded.name, git_url = excluded.git_url
             RETURNING id, path, name, git_url, created_at",
            params![path, name, git_url],
        )
        .await?;
    let row = rows.next().await?.expect("upsert returns row");
    map_repo(&row)
}

pub async fn list(conn: &Connection) -> Result<Vec<RepoWithStats>> {
    let mut rows = conn
        .query(
            "SELECT id, path, name, git_url, created_at FROM repositories ORDER BY id",
            (),
        )
        .await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let repo = map_repo(&row)?;
        let stats = RepoStats {
            files: count(conn, "SELECT COUNT(*) FROM files WHERE repo_id=?1", vec![repo.id.into()]).await?,
            symbols: count(conn, "SELECT COUNT(*) FROM symbols WHERE repo_id=?1", vec![repo.id.into()]).await?,
            edges: count(conn, "SELECT COUNT(*) FROM call_edges WHERE repo_id=?1", vec![repo.id.into()]).await?,
            pending_tasks: count(
                conn,
                "SELECT COUNT(*) FROM analysis_tasks WHERE repo_id=?1 AND status IN ('pending','running')",
                vec![repo.id.into()],
            )
            .await?,
        };
        out.push(RepoWithStats { repo, stats });
    }
    Ok(out)
}

pub async fn get(conn: &Connection, id: i64) -> Result<Option<Repo>> {
    let mut rows = conn
        .query(
            "SELECT id, path, name, git_url, created_at FROM repositories WHERE id = ?1",
            params![id],
        )
        .await?;
    match rows.next().await? {
        Some(row) => Ok(Some(map_repo(&row)?)),
        None => Ok(None),
    }
}

pub async fn get_by_path(conn: &Connection, path: &str) -> Result<Option<Repo>> {
    let mut rows = conn
        .query(
            "SELECT id, path, name, git_url, created_at FROM repositories WHERE path = ?1",
            params![path],
        )
        .await?;
    match rows.next().await? {
        Some(row) => Ok(Some(map_repo(&row)?)),
        None => Ok(None),
    }
}

pub async fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM repositories WHERE id = ?1", params![id]).await?;
    Ok(())
}
