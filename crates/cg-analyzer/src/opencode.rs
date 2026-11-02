//! opencode server HTTP client (contract: docs/opencode-server.md).

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct OpencodeClient {
    base: String,
    token: Option<String>,
    http: reqwest::Client,
}

fn env_url() -> String {
    std::env::var("OPENCODER_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:34096".into())
}

impl OpencodeClient {
    pub fn from_env() -> Self {
        Self::new(env_url(), std::env::var("OPENCODER_API_KEY").ok())
    }

    pub fn new(base: String, token: Option<String>) -> Self {
        OpencodeClient {
            base: base.trim_end_matches('/').to_string(),
            token,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    async fn post_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let mut req = self.http.post(format!("{}{path}", self.base)).json(body);
        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }
        let resp = req.send().await.with_context(|| format!("POST {path}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("POST {path} -> {status}: {text}"));
        }
        Ok(resp.json::<T>().await?)
    }

    pub async fn create_session(&self, title: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct Created {
            id: String,
        }
        let c: Created = self
            .post_json("/api/sessions", &serde_json::json!({ "title": title }))
            .await
            .context("create session")?;
        Ok(c.id)
    }

    pub async fn send_prompt(&self, session_id: &str, prompt: &str) -> Result<()> {
        #[derive(Deserialize)]
        struct Admitted {
            #[allow(dead_code)]
            ok: bool,
        }
        let _: Admitted = self
            .post_json(
                &format!("/api/sessions/{session_id}/prompt"),
                &serde_json::json!({ "prompt": prompt }),
            )
            .await
            .context("send prompt")?;
        Ok(())
    }

    /// Poll messages until an assistant reply newer than `since` appears.
    pub async fn wait_reply(
        &self,
        session_id: &str,
        since_ms: i64,
        timeout: Duration,
        poll: Duration,
    ) -> Result<String> {
        let deadline = Instant::now() + timeout;
        loop {
            if Instant::now() >= deadline {
                return Err(anyhow!("timeout waiting for opencode reply"));
            }
            match self.fetch_reply(session_id, since_ms).await {
                Ok(Some(text)) if !text.trim().is_empty() => return Ok(text),
                Ok(_) => {}
                Err(e) if is_retryable(&e) => tracing::warn!(error = %e, "transient fetch error"),
                Err(e) => return Err(e),
            }
            tokio::time::sleep(poll).await;
        }
    }

    async fn fetch_reply(&self, session_id: &str, since_ms: i64) -> Result<Option<String>> {
        #[derive(Deserialize)]
        struct Block {
            kind: String,
            #[serde(default)]
            text: String,
        }
        #[derive(Deserialize)]
        struct Message {
            role: String,
            #[serde(default)]
            created_at: i64,
            #[serde(default)]
            blocks: Vec<Block>,
        }
        #[derive(Deserialize)]
        struct Msgs {
            #[serde(default)]
            messages: Vec<Message>,
        }
        let url = format!("{}/api/sessions/{session_id}/messages", self.base);
        let mut req = self.http.get(&url);
        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }
        let resp = req.send().await.context("fetch messages")?;
        if !resp.status().is_success() {
            return Err(anyhow!("fetch messages -> {}", resp.status()));
        }
        let msgs: Msgs = resp.json().await?;
        Ok(msgs
            .messages
            .iter()
            .filter(|m| m.role == "assistant" && m.created_at > since_ms)
            .max_by_key(|m| m.created_at)
            .map(|m| {
                m.blocks
                    .iter()
                    .filter(|b| b.kind == "text")
                    .map(|b| b.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            }))
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        let url = format!("{}/api/sessions/{session_id}", self.base);
        let mut req = self.http.delete(&url);
        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }
        let _ = req.send().await;
        Ok(())
    }

    /// One-shot: fresh session, prompt, wait for reply, cleanup.
    pub async fn complete(&self, title: &str, prompt: &str, timeout: Duration) -> Result<String> {
        let sid = self.create_session(title).await?;
        let before = chrono_now_ms();
        let result = async {
            self.send_prompt(&sid, prompt).await?;
            self.wait_reply(&sid, before, timeout, Duration::from_millis(800)).await
        }
        .await;
        let _ = self.delete_session(&sid).await;
        result
    }
}

fn is_retryable(e: &anyhow::Error) -> bool {
    let msg = format!("{e:#}").to_lowercase();
    msg.contains("timeout") || msg.contains("connection") || msg.contains("502") || msg.contains("503")
}

fn chrono_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
