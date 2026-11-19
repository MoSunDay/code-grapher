//! Repository CRUD + aggregate stats.

use crate::count;
use anyhow::Result;
use cg_core::Repo;
use cg_feature::feature_repo_id;
use libsql::{params, Connection};
use serde::Serialize;

const REPO_COLS: &str = "id, path, name, git_url, created_at, status, error";

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
    /// sha256(canonical git_url); null when git_url is missing or unparseable.
    pub feature_repo_id: Option<String>,
}

/// Computed feature identity for any repo row.
pub fn feature_repo_id_of(repo: &Repo) -> Option<String> {
    feature_repo_id(repo.git_url.as_deref())
}

fn map_repo(row: &libsql::Row) -> Result<Repo> {
    Ok(Repo {
        id: row.get(0)?,
        path: row.get(1)?,
        name: row.get(2)?,
        git_url: row.get(3)?,
        created_at: row.get(4)?,
        status: row.get(5)?,
        error: row.get(6)?,
    })
}

pub async fn upsert(conn: &Connection, path: &str, name: &str, git_url: Option<&str>) -> Result<Repo> {
    let mut rows = conn
        .query(
            "INSERT INTO repositories(path, name, git_url) VALUES (?1, ?2, ?3)
             ON CONFLICT(path) DO UPDATE SET name = excluded.name, git_url = excluded.git_url
             RETURNING id, path, name, git_url, created_at, status, error",
            params![path, name, git_url],
        )
        .await?;
    let row = rows.next().await?.expect("upsert returns row");
    map_repo(&row)
}

/// Insert a fresh row in an explicit lifecycle state (clone flow). Unlike
/// `upsert` this never overwrites an existing path.
pub async fn insert(
    conn: &Connection,
    path: &str,
    name: &str,
    git_url: Option<&str>,
    status: &str,
) -> Result<Repo> {
    let mut rows = conn
        .query(
            &format!(
                "INSERT INTO repositories(path, name, git_url, status) VALUES (?1, ?2, ?3, ?4)
                 RETURNING {REPO_COLS}"
            ),
            params![path, name, git_url, status],
        )
        .await?;
    let row = rows.next().await?.expect("insert returns row");
    map_repo(&row)
}

/// Update lifecycle status (cloning|ready|failed) and optional error detail.
pub async fn set_status(conn: &Connection, id: i64, status: &str, error: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE repositories SET status = ?2, error = ?3 WHERE id = ?1",
        params![id, status, error],
    )
    .await?;
    Ok(())
}

/// Find a repo by canonical git_url (identity dedup for URL registration).
pub async fn get_by_git_url(conn: &Connection, git_url: &str) -> Result<Option<Repo>> {
    let mut rows = conn
        .query(
            &format!("SELECT {REPO_COLS} FROM repositories WHERE git_url = ?1"),
            params![git_url],
        )
        .await?;
    match rows.next().await? {
        Some(row) => Ok(Some(map_repo(&row)?)),
        None => Ok(None),
    }
}

/// Mark rows stuck in `cloning` (server died mid-clone) as failed. Returns count.
pub async fn fail_orphan_cloning(conn: &Connection) -> Result<u64> {
    let n = conn
        .execute(
            &format!(
                "UPDATE repositories SET status = '{}', error = 'clone interrupted by restart'
                 WHERE status = '{}'",
                cg_core::repo_status::FAILED,
                cg_core::repo_status::CLONING
            ),
            (),
        )
        .await? as u64;
    Ok(n)
}

pub async fn list(conn: &Connection) -> Result<Vec<RepoWithStats>> {
    let mut rows = conn
        .query(
            &format!("SELECT {REPO_COLS} FROM repositories ORDER BY id"),
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
        let feature_repo_id = feature_repo_id_of(&repo);
        out.push(RepoWithStats { repo, stats, feature_repo_id });
    }
    Ok(out)
}

pub async fn get(conn: &Connection, id: i64) -> Result<Option<Repo>> {
    let mut rows = conn
        .query(
            &format!("SELECT {REPO_COLS} FROM repositories WHERE id = ?1"),
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
            &format!("SELECT {REPO_COLS} FROM repositories WHERE path = ?1"),
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
