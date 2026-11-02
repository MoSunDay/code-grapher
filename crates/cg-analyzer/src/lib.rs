//! cg-analyzer: LLM-assisted hidden-call analysis via opencode server.

pub mod context;
pub mod opencode;
pub mod prompt;
pub mod scheduler;

pub use opencode::OpencodeClient;
pub use scheduler::{enqueue_all, run_once, run_task, spawn_scheduler, AnalyzerConfig, TaskSummary};
