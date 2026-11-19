//! Feature API contract: repo_id-addressed routes over the git-backed store.

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

#[tokio::test]
async fn feature_routes_roundtrip() {
    let (_dir, app) = app().await;
    let repo_id = repo_id_from_git_url("https://github.com/owner/repo.git").unwrap();
    let base = format!("/api/features/{repo_id}");
    let name = "AST 解析与领域模型";
    let detail_uri = format!("{base}/{}", urlencode(name));

    let feature = serde_json::json!({
        "name": name,
        "function_entry": "cg_core::parse",
        "request_chain": [],
        "external_dependencies": [],
        "function_summary": "解析",
        "children": []
    });
    let created = json_body(
        app.clone()
            .oneshot(
                Request::post(&base)
                    .header("content-type", "application/json")
                    .body(Body::from(feature.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(created["outcome"], "draft");

    let list = json_body(
        app.clone()
            .oneshot(Request::get(&base).body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(list["features"][0]["name"], name);
    assert_eq!(list["features"][0]["state"], "draft");

    let detail = json_body(
        app.clone()
            .oneshot(Request::get(&detail_uri).body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(detail["state"], "draft");
    let hash = detail["hash"].as_str().unwrap().to_string();

    let diff = json_body(
        app.clone()
            .oneshot(
                Request::get(format!("{detail_uri}/diff"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert!(diff["diff"].as_str().unwrap().contains(name));

    // stale expected_hash is rejected
    let stale = app
        .clone()
        .oneshot(
            Request::post(format!("{detail_uri}/approve"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "reviewer": "alice", "expected_hash": "00" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::BAD_REQUEST);

    let approved = json_body(
        app.clone()
            .oneshot(
                Request::post(format!("{detail_uri}/approve"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "reviewer": "alice",
                            "note": "lgtm",
                            "expected_hash": hash
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(approved["ok"], true);

    let list_after = json_body(
        app.clone()
            .oneshot(Request::get(&base).body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(list_after["features"][0]["state"], "stable");

    let changelog = json_body(
        app.clone()
            .oneshot(
                Request::get(format!("{base}/changelog"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let entries = changelog["changelog"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert!(entries[0]["file"].as_str().unwrap().contains("AST"));
    assert!(entries[0]["markdown"].as_str().unwrap().contains("alice"));
}

#[tokio::test]
async fn invalid_repo_id_is_rejected() {
    let (_dir, app) = app().await;
    let response = app
        .oneshot(
            Request::get("/api/features/not-a-hash")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn legacy_id_in_draft_body_is_rejected() {
    let (_dir, app) = app().await;
    let repo_id = repo_id_from_git_url("https://github.com/owner/repo.git").unwrap();
    let body = serde_json::json!({
        "id": "f-42",
        "name": "旧结构",
        "function_entry": "x",
        "request_chain": [],
        "external_dependencies": [],
        "function_summary": "",
        "children": []
    });
    let response = app
        .oneshot(
            Request::post(format!("/api/features/{repo_id}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

fn urlencode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}
