//! Builds FnContext (function source, surrounding lines, candidate symbols).

use crate::prompt::{name_tokens, FnContext};
use anyhow::Result;
use cg_core::model::{EdgeKind, SymbolRow};
use cg_db::{edges, symbols};
use libsql::Connection;

/// Lines of context shown around the function.
const CONTEXT_PAD: i64 = 15;

pub async fn build_context(conn: &Connection, sym: &SymbolRow, repo_path: &str) -> Result<FnContext> {
    let file_path = sym
        .file_path
        .clone()
        .ok_or_else(|| anyhow::anyhow!("symbol {} has no file", sym.id))?;
    let abs = std::path::Path::new(repo_path).join(&file_path);
    let content = tokio::fs::read_to_string(&abs).await?;
    let lines: Vec<&str> = content.lines().collect();

    let start = (sym.start_line - 1).clamp(0, lines.len() as i64) as usize;
    let end = sym.end_line.clamp(0, lines.len() as i64) as usize;
    let source = lines[start..end].join("\n");

    let ctx_start = start.saturating_sub(CONTEXT_PAD as usize);
    let ctx_end = (end + CONTEXT_PAD as usize).min(lines.len());
    let context = lines[ctx_start..ctx_end].join("\n");

    let candidates = collect_candidates(conn, sym).await;
    Ok(FnContext {
        symbol: sym.clone(),
        source,
        file_path,
        context,
        candidates,
    })
}

/// Recall candidates: unresolved callee names of this function first (most
/// relevant), then name-token matches.
async fn collect_candidates(conn: &Connection, sym: &SymbolRow) -> Vec<SymbolRow> {
    let mut out: Vec<SymbolRow> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    let unresolved = edges::for_caller(conn, sym.id)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.callee_id.is_none() && e.kind == EdgeKind::DynamicMethod)
        .map(|e| e.callee_name)
        .collect::<Vec<_>>();
    for name in unresolved.iter().take(10) {
        for c in symbols::search_candidates(conn, sym.repo_id, name, 5).await.unwrap_or_default() {
            if !seen.contains(&c.qual_path) {
                seen.push(c.qual_path.clone());
                out.push(c);
            }
        }
    }

    for token in name_tokens(&sym.name).into_iter().take(4) {
        for c in symbols::search_candidates(conn, sym.repo_id, &token, 10).await.unwrap_or_default() {
            if !seen.contains(&c.qual_path) && out.len() < 40 {
                seen.push(c.qual_path.clone());
                out.push(c);
            }
        }
    }
    out
}
