//! Feature API contract: MR branches, tree/blob, approve = merge to main.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cg_feature::repo_id_from_git_url;
use tower::ServiceExt;

async fn app() -> (tempfile::TempDir, axum::Router) {
    let dir = tempfile::tempdir().unwrap();
    let db = cg_db::open_local(&dir.path().join("test.db")).await.unwrap();
    cg_db::migrate(&db).await.unwrap();
    let cfg = cg_server::state::ServerConfig {
        feature_root: dir.path().to_path_buf(),
        ..cg_server::state::ServerConfig::default()
    };
    (dir, cg_server::api::router(cg_server::state::AppState::new(db, cfg)))
}

async fn json_body(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn get(app: &axum::Router, uri: &str) -> serde_json::Value {
    json_body(
        app.clone()
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await
}

async fn post(app: &axum::Router, uri: &str, body: serde_json::Value) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::post(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn mr_lifecycle_roundtrip() {
    let (_dir, app) = app().await;
    let repo_id = repo_id_from_git_url("https://github.com/owner/repo.git").unwrap();
    let base = format!("/api/features/{repo_id}");
    let name = "AST解析与领域模型";
    let mr_uri = format!("{base}/mrs");

    // empty partition: empty MR list, no crash
    assert_eq!(get(&app, &mr_uri).await["mrs"].as_array().unwrap().len(), 0);

    // draft: two files onto the mr branch
    let created = json_body(
        post(
            &app,
            &format!("{base}/draft"),
            serde_json::json!({
                "name": name,
                "files": [
                    { "path": format!("docs/{name}/overview.md"), "content": "# 概览\n" },
                    { "path": format!("docs/{name}/ast/index.json"), "content": "{\"n\":1}\n" }
                ]
            }),
        )
        .await,
    )
    .await;
    assert_eq!(created["outcome"], "draft");

    // noop on identical refresh
    let again = json_body(
        post(
            &app,
            &format!("{base}/draft"),
            serde_json::json!({
                "name": name,
                "files": [
                    { "path": format!("docs/{name}/overview.md"), "content": "# 概览\n" },
                    { "path": format!("docs/{name}/ast/index.json"), "content": "{\"n\":1}\n" }
                ]
            }),
        )
        .await,
    )
    .await;
    assert_eq!(again["outcome"], "noop");

    // MR list: open with tip
    let mrs = get(&app, &mr_uri).await;
    assert_eq!(mrs["mrs"][0]["name"], name);
    assert_eq!(mrs["mrs"][0]["state"], "open");
    let tip = mrs["mrs"][0]["tip"].as_str().unwrap().to_string();

    // tree with draft badges + render hints
    let tree = get(&app, &format!("{base}/tree?mr={}", urlencode(name))).await;
    let files = tree["files"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert!(files.iter().all(|f| f["state"] == "draft"));
    let by = |p: &str| files.iter().find(|f| f["path"].as_str().unwrap().contains(p)).unwrap();
    assert_eq!(by("overview.md")["render"], "markdown");
    assert_eq!(by("index.json")["render"], "json");

    // blob read
    let blob = get(&app, &format!("{base}/blob?mr={}&path={}", urlencode(name), urlencode(&format!("docs/{name}/overview.md")))).await;
    assert_eq!(blob["content"], "# 概览\n");
    assert_eq!(blob["truncated"], false);

    // missing blob -> 404
    let missing = app
        .clone()
        .oneshot(
            Request::get(format!("{base}/blob?path=docs/none.md"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    // diff main...branch
    let diff = get(&app, &format!("{base}/{}/diff", urlencode(name))).await;
    assert!(diff["diff"].as_str().unwrap().contains("overview.md"));

    // commits
    let commits = get(&app, &format!("{base}/{}/commits", urlencode(name))).await;
    assert_eq!(commits["commits"].as_array().unwrap().len(), 1);

    // approve merges
    let approved = json_body(
        post(
            &app,
            &format!("{base}/{}/approve", urlencode(name)),
            serde_json::json!({ "reviewer": "alice", "note": "ok" }),
        )
        .await,
    )
    .await;
    assert_eq!(approved["ok"], true);
    assert!(approved["changelog"].as_str().unwrap().starts_with("_changelog/"));

    // MR now merged with reviewer
    let mrs = get(&app, &mr_uri).await;
    assert_eq!(mrs["mrs"][0]["state"], "merged");
    assert_eq!(mrs["mrs"][0]["reviewer"], "alice");
    assert_eq!(mrs["mrs"][0]["merge_commit"], approved["merge_commit"]);

    // main tree: stable badges
    let tree = get(&app, &format!("{base}/tree")).await;
    assert_eq!(tree["mr"], serde_json::Value::Null);
    assert!(tree["files"]
        .as_array()
        .unwrap()
        .iter()
        .all(|f| f["state"] == "stable"));

    // stale tip approve -> 409
    let conflict = post(
        &app,
        &format!("{base}/{}/approve", urlencode(name)),
        serde_json::json!({ "reviewer": "bob", "expected_tip": tip }),
    )
    .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);

    // changelog
    let changelog = get(&app, &format!("{base}/changelog")).await;
    assert!(changelog["changelog"][0]["markdown"]
        .as_str()
        .unwrap()
        .contains("alice"));
}

#[tokio::test]
async fn draft_rejects_paths_outside_mr_namespace() {
    let (_dir, app) = app().await;
    let repo_id = repo_id_from_git_url("https://github.com/owner/repo.git").unwrap();
    let response = post(
        &app,
        &format!("/api/features/{repo_id}/draft"),
        serde_json::json!({
            "name": "mine",
            "files": [{ "path": "docs/other/a.md", "content": "x" }]
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn bad_repo_id_is_rejected() {
    let (_dir, app) = app().await;
    let response = app
        .oneshot(
            Request::get("/api/features/not-a-repo-id/mrs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

fn urlencode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
