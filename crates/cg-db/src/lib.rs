//! cg-db: libsql/Turso storage layer.
//! Pure functions over `libsql::Connection`; migrations embedded from `migrations/`.

pub mod edges;
pub mod files;
pub mod repos;
pub mod runs;
pub mod symbols;
pub mod tasks;

use anyhow::{Context, Result};
use libsql::{Connection, Builder};

/// Embedded migrations: (name, sql) applied in order.
const MIGRATIONS: &[(&str, &str)] = &[("0001_init", include_str!("../../../migrations/0001_init.sql"))];

/// Open a local file database.
pub async fn open_local(path: &std::path::Path) -> Result<Connection> {
    let db = Builder::new_local(path)
        .build()
        .await
        .context("open local libsql database")?;
    let conn = db.connect()?;
    conn.execute("PRAGMA foreign_keys = ON", ()).await?;
    Ok(conn)
}

/// Open from environment: `TURSO_DATABASE_URL` (libsql://… remote, or file path)
/// with `TURSO_AUTH_TOKEN`. Falls back to a local file when unset.
pub async fn open_env(fallback_local: &std::path::Path) -> Result<Connection> {
    let url = std::env::var("TURSO_DATABASE_URL").unwrap_or_default();
    if url.is_empty() || url.starts_with("file:") {
        let p = url.strip_prefix("file:").map(str::to_string).unwrap_or(url);
        let path = if p.is_empty() {
            fallback_local.to_path_buf()
        } else {
            p.into()
        };
        return open_local(&path).await;
    }
    let token = std::env::var("TURSO_AUTH_TOKEN").unwrap_or_default();
    let db = Builder::new_remote(url.clone(), token)
        .build()
        .await
        .with_context(|| format!("open remote Turso database {url}"))?;
    let conn = db.connect()?;
    conn.execute("PRAGMA foreign_keys = ON", ()).await?;
    Ok(conn)
}

/// Apply pending migrations (idempotent).
pub async fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            name TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )
    .await?;
    for (name, sql) in MIGRATIONS {
        let applied = conn
            .execute(
                "INSERT OR IGNORE INTO schema_migrations(name) VALUES (?1)",
                libsql::params![name],
            )
            .await?;
        if applied == 0 {
            continue; // already applied
        }
        conn.execute_batch(sql)
            .await
            .with_context(|| format!("apply migration {name}"))?;
        tracing::info!(migration = name, "applied");
    }
    Ok(())
}

/// Single-value helper: run `SELECT COUNT(*)…` style queries.
pub async fn count(conn: &Connection, sql: &str, params: Vec<libsql::Value>) -> Result<i64> {
    let mut rows = conn.query(sql, libsql::params_from_iter(params)).await?;
    match rows.next().await? {
        Some(row) => Ok(row.get::<i64>(0)?),
        None => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrates_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_local(&dir.path().join("t.db")).await.unwrap();
        migrate(&conn).await.unwrap();
        migrate(&conn).await.unwrap();
        let n = count(&conn, "SELECT COUNT(*) FROM sqlite_master WHERE type='table'", vec![]).await.unwrap();
        assert!(n >= 7, "expected schema tables, got {n}");
    }
}
