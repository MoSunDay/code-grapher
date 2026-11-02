//! Remote mode: same commands via the daemon REST API.

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Value};

#[derive(Clone)]
pub struct Remote {
    base: String,
    http: Client,
}

impl Remote {
    pub fn new(base: &str) -> Self {
        Remote {
            base: base.trim_end_matches('/').to_string(),
            http: Client::new(),
        }
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let resp = self.http.get(format!("{}{path}", self.base)).send().await?;
        self.unwrap(resp).await
    }

    async fn post(&self, path: &str, body: Value) -> Result<Value> {
        let resp = self
            .http
            .post(format!("{}{path}", self.base))
            .json(&body)
            .send()
            .await?;
        self.unwrap(resp).await
    }

    async fn unwrap(&self, resp: reqwest::Response) -> Result<Value> {
        let status = resp.status();
        let value: Value = resp.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(anyhow!("{status}: {value}"));
        }
        Ok(value)
    }

    pub async fn repo_id(&self, sel: Option<&str>) -> Result<i64> {
        let list = self
            .get("/api/repos")
            .await?
            .get("repos")
            .and_then(|r| r.as_array().cloned())
            .unwrap_or_default();
        if list.is_empty() {
            return Err(anyhow!("no repositories on server"));
        }
        match sel {
            Some(s) => list
                .iter()
                .find(|r| {
                    r["id"].as_i64().map(|i| i.to_string() == s).unwrap_or(false)
                        || r["path"].as_str() == Some(s)
                        || r["name"].as_str() == Some(s)
                })
                .and_then(|r| r["id"].as_i64())
                .ok_or_else(|| anyhow!("repo '{s}' not found on server")),
            None => list[0]["id"]
                .as_i64()
                .ok_or_else(|| anyhow!("malformed server response")),
        }
    }

    pub async fn add_repo(&self, path: &str, name: Option<&str>, git_url: Option<&str>) -> Result<Value> {
        self.post("/api/repos", json!({ "path": path, "name": name, "git_url": git_url })).await
    }

    pub async fn list_repos(&self) -> Result<Value> {
        self.get("/api/repos").await
    }

    pub async fn remove_repo(&self, id: i64) -> Result<Value> {
        let resp = self
            .http
            .delete(format!("{}/api/repos/{id}", self.base))
            .send()
            .await?;
        self.unwrap(resp).await
    }

    pub async fn index(&self, id: i64, full: bool) -> Result<Value> {
        self.post(&format!("/api/repos/{id}/index"), json!({ "full": full })).await
    }

    pub async fn analyze(&self, id: i64) -> Result<Value> {
        self.post(&format!("/api/repos/{id}/analyze"), json!({})).await
    }

    #[allow(dead_code)]
    pub async fn status(&self, id: i64) -> Result<Value> {
        self.get(&format!("/api/repos/{id}/status")).await
    }

    pub async fn files(&self, id: i64) -> Result<Value> {
        self.get(&format!("/api/repos/{id}/files")).await
    }

    pub async fn symbols(&self, id: i64, kind: Option<&str>, q: Option<&str>) -> Result<Value> {
        let mut path = format!("/api/repos/{id}/symbols?limit=500");
        if let Some(k) = kind {
            path.push_str(&format!("&kind={k}"));
        }
        if let Some(query) = q {
            path.push_str(&format!("&q={}", urlencode(query)));
        }
        self.get(&path).await
    }

    pub async fn calls(&self, symbol_id: i64) -> Result<Value> {
        self.get(&format!("/api/symbols/{symbol_id}")).await
    }

    pub async fn file(&self, id: i64, rel: &str) -> Result<Value> {
        self.get(&format!("/api/repos/{id}/file?path={}", urlencode(rel))).await
    }

    pub async fn tasks(&self, id: i64, status: Option<&str>) -> Result<Value> {
        let mut path = format!("/api/repos/{id}/tasks?limit=100");
        if let Some(s) = status {
            path.push_str(&format!("&status={s}"));
        }
        self.get(&path).await
    }

    pub async fn retry_task(&self, task_id: i64) -> Result<Value> {
        self.post(&format!("/api/tasks/{task_id}/retry"), json!({})).await
    }
}

fn urlencode(s: &str) -> String {
    s.bytes().fold(String::new(), |mut out, b| {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
        out
    })
}
