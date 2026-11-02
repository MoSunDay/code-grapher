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
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            db_path: std::path::PathBuf::from("./code-grapher.db"),
            port: 7800,
            watch: true,
            analyzer: true,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: Connection,
    pub events: broadcast::Sender<String>,
    /// Repos currently being indexed (prevents concurrent double-runs).
    pub indexing: std::sync::Arc<Mutex<HashSet<i64>>>,
    pub cfg: ServerConfig,
}

impl AppState {
    pub fn new(db: Connection, cfg: ServerConfig) -> Self {
        let (events, _) = broadcast::channel(512);
        AppState {
            db,
            events,
            indexing: std::sync::Arc::new(Mutex::new(HashSet::new())),
            cfg,
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
