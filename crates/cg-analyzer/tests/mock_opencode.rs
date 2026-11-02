//! End-to-end analyzer test against a hand-rolled mock opencode server.

use cg_analyzer::opencode::OpencodeClient;
use cg_analyzer::scheduler::{run_once, AnalyzerConfig};
use cg_core::model::SymbolKind;
use cg_db::{edges, symbols, tasks};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Minimal HTTP/1.1 mock of the opencode server API (docs/opencode-server.md).
async fn spawn_mock(addr: std::net::SocketAddr) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let prompts = Arc::new(AtomicUsize::new(0));
    tokio::spawn(async move {
        loop {
            let (mut stream, _) = match listener.accept().await {
                Ok(s) => s,
                Err(_) => break,
            };
            let prompts = prompts.clone();
            let _ = tokio::task::spawn_blocking(move || {
                use std::io::{Read, Write};
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut stream = stream;
                let mut stream = stream.into_std().unwrap();
                stream.set_nonblocking(false).unwrap();
                let mut buf = [0u8; 8192];
                let mut read = stream.read(&mut buf).unwrap_or(0);
                let mut req = String::from_utf8_lossy(&buf[..read]).to_string();
                if let Some(len) = content_length(&req) {
                    while req.len() < header_end(&req).unwrap_or(0) + len {
                        read = match stream.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => n,
                            Err(_) => break,
                        };
                        req.push_str(&String::from_utf8_lossy(&buf[..read]));
                    }
                }
                let (method, path) = request_line(&req);
                let body = body_text(&req);
                let reply = route(&method, &path, &body, &prompts);
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        reply.len(),
                        reply
                    )
                    .as_bytes(),
                );
            })
            .await;
        }
    });
    Ok(())
}

fn header_end(req: &str) -> Option<usize> {
    req.find("\r\n\r\n").map(|i| i + 4)
}
fn content_length(req: &str) -> Option<usize> {
    for line in req.lines() {
        let l = line.to_lowercase();
        if l.starts_with("content-length:") {
            return l[15..].trim().parse().ok();
        }
    }
    None
}
fn request_line(req: &str) -> (String, String) {
    let mut parts = req.lines().next().unwrap_or("").split(' ');
    (parts.next().unwrap_or("").into(), parts.next().unwrap_or("").into())
}
fn body_text(req: &str) -> String {
    match header_end(req) {
        Some(i) => req[i..].to_string(),
        None => String::new(),
    }
}

fn route(method: &str, path: &str, body: &str, prompts: &AtomicUsize) -> String {
    match (method, path) {
        ("POST", "/api/sessions") => r#"{"id":"sess-1"}"#.into(),
        ("POST", "/api/sessions/sess-1/prompt") => {
            prompts.fetch_add(1, Ordering::SeqCst);
            r#"{"ok":true,"admitted_seq":1}"#.into()
        }
        ("GET", "/api/sessions/sess-1/messages") => {
            let target_fn = body_target_fn(body);
            // Only reply after the prompt was sent: reply content depends on target.
            if prompts.load(Ordering::SeqCst) == 0 && body.is_empty() {
                return r#"{"messages":[]}"#.into();
            }
            let text = if body.contains("TARGET FUNCTION") {
                fixture_reply(&target_fn)
            } else {
                // GET has no body: reuse the last known target by replying for main.
                r#"[{"callee_name":"helper","kind":"callback","confidence":0.9,"evidence":"f = helper"}]"#.into()
            };
            format!(
                r#"{{"messages":[{{"id":"u1","role":"user","created_at":1,"blocks":[]}},{{"id":"a1","role":"assistant","created_at":9999999999999,"blocks":[{{"kind":"text","text":{}}}]}}]}}"#,
                serde_json::json!(text)
            )
        }
        ("DELETE", "/api/sessions/sess-1") => "{}".into(),
        _ => "{}".into(),
    }
}

fn body_target_fn(body: &str) -> String {
    body.split("TARGET FUNCTION: `").nth(1).and_then(|s| s.split('`').next()).unwrap_or("").to_string()
}

fn fixture_reply(target: &str) -> String {
    if target.ends_with("::main") || target == "main" {
        r#"[{"callee_name":"helper","kind":"callback","confidence":0.9,"evidence":"f = helper"}]"#.into()
    } else {
        "[]".into()
    }
}

#[tokio::test]
async fn analyzer_end_to_end_with_mock() {
    // fixture repo on disk
    let dir = tempfile::tempdir().unwrap();
    let src = r#"
pub fn helper(x: i32) -> i32 { x + 1 }
pub fn main() {
    let f = helper;
    let _r = f(2);
    let _v = vec![1];
}
"#;
    std::fs::write(dir.path().join("lib.rs"), src).unwrap();
    let db = cg_db::open_local(&dir.path().join("t.db")).await.unwrap();
    cg_db::migrate(&db).await.unwrap();
    let repo = cg_db::repos::upsert(&db, dir.path().to_str().unwrap(), "fixture", None)
        .await
        .unwrap();
    let mut noop = |_p: cg_indexer::IndexProgress| {};
    cg_indexer::index_repo(&db, &repo, false, "test", noop).await.unwrap();

    // analyzer against mock
    let port = pick_port().await;
    spawn_mock(([127, 0, 0, 1], port).into()).await.unwrap();
    let client = OpencodeClient::new(format!("http://127.0.0.1:{port}"), Some("t".into()));
    let cfg = AnalyzerConfig {
        concurrency: 1,
        max_attempts: 2,
        request_timeout: std::time::Duration::from_secs(10),
        poll_interval: std::time::Duration::from_millis(50),
    };
    let summaries = run_once(&db, &client, &cfg, 10).await.unwrap();
    assert_eq!(summaries.len(), 2, "main + helper analyzed: {summaries:?}");

    // llm edge persisted and resolved
    let (total, syms) = symbols::list(&db, repo.id, &symbols::SymbolFilter {
        kind: Some(SymbolKind::Function), q: None, file_id: None, limit: 10, offset: 0 }).await.unwrap();
    assert_eq!(total, 2);
    let main_id = syms.iter().find(|s| s.name == "main").unwrap().id;
    let helper_id = syms.iter().find(|s| s.name == "helper").unwrap().id;
    let callees = edges::for_caller(&db, main_id).await.unwrap();
    let llm_cb = callees
        .iter()
        .find(|e| e.source == cg_core::model::EdgeSource::Llm && e.kind == cg_core::model::EdgeKind::Callback)
        .expect("llm callback edge stored");
    assert_eq!(llm_cb.callee_id, Some(helper_id), "unique-name validation binds callee");
    // ast edges still there
    assert!(callees.iter().any(|e| e.source == cg_core::model::EdgeSource::Ast && e.kind == cg_core::model::EdgeKind::Macro));

    // tasks all done
    let listed = tasks::list(&db, repo.id, None, 10).await.unwrap();
    assert!(listed.iter().all(|t| t.status.as_str() == "done"), "{listed:?}");
}

async fn pick_port() -> u16 {
    tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
