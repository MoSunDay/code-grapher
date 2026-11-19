//! Shared server state.

use libsql::Connection;
use std::collections::HashSet;
use std::sync::Mutex;
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Local fallback database file (used unless TURSO_DATABASE_URL is set).
    pub db_path: std::path::PathBuf,
    pub port: u16,
    pub watch: bool,
    pub analyzer: bool,
    /// Built SPA directory served at `/` (web/dist); empty disables static hosting.
    pub web_dist: Option<std::path::PathBuf>,
    /// Git-backed feature review root (`<root>/<repo_id>/<feature_name>/`).
    pub feature_root: std::path::PathBuf,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            db_path: std::path::PathBuf::from("./code-grapher.db"),
            port: 7800,
            watch: true,
            analyzer: true,
            web_dist: default_web_dist(),
            feature_root: default_feature_root(),
        }
    }
}

/// `CG_WEB_DIST` override, else `web/dist` if it exists, else None.
pub fn default_web_dist() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("CG_WEB_DIST") {
        let pb = std::path::PathBuf::from(p);
        return if pb.is_dir() { Some(pb) } else { None };
    }
    let d = std::path::PathBuf::from("web/dist");
    d.is_dir().then_some(d)
}

/// Feature review storage root; env override or platform data home, with a
/// local fallback for environments lacking HOME.
pub fn default_feature_root() -> std::path::PathBuf {
    cg_feature::resolve_feature_root(None).unwrap_or_else(|_| std::path::PathBuf::from("./feature-data"))
}

#[derive(Clone)]
pub struct AppState {
    pub db: Connection,
    pub events: broadcast::Sender<String>,
    /// Repos currently being indexed (prevents concurrent double-runs).
    pub indexing: std::sync::Arc<Mutex<HashSet<i64>>>,
    pub cfg: ServerConfig,
    /// Runtime-adjustable analyzer concurrency (0 = paused).
    pub analyze_concurrency: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl AppState {
    pub fn new(db: Connection, cfg: ServerConfig) -> Self {
        let (events, _) = broadcast::channel(512);
        AppState {
            db,
            events,
            indexing: std::sync::Arc::new(Mutex::new(HashSet::new())),
            cfg,
            analyze_concurrency: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(
                cg_analyzer::AnalyzerConfig::from_env().concurrency,
            )),
        }
    }

    /// Fire-and-forget event to all WS subscribers.
    pub fn emit(&self, value: &serde_json::Value) {
        let _ = self.events.send(value.to_string());
    }
}

pub fn event(kind: &str, fields: &[(&str, serde_json::Value)]) -> String {
    let mut map = serde_json::Map::new();
    map.insert("type".into(), serde_json::Value::String(kind.into()));
    for (k, v) in fields {
        map.insert((*k).into(), v.clone());
    }
    serde_json::Value::Object(map).to_string()
}
