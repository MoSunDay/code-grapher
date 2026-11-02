//! Domain model shared across code-grapher crates.
//! Plain data + enums + conversions only; no behaviour, no mutation.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Repo {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub git_url: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileRow {
    pub id: i64,
    pub repo_id: i64,
    pub path: String,
    pub lang: Option<String>,
    pub hash: Option<String>,
    pub mtime: i64,
    pub loc: i64,
    pub parsed_at: Option<String>,
    #[serde(default)]
    pub symbol_count: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Method,
    Closure,
    Struct,
    Enum,
    Trait,
    Type,
    Const,
    Static,
    Macro,
    Impl,
    Mod,
}

impl SymbolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Closure => "closure",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Type => "type",
            SymbolKind::Const => "const",
            SymbolKind::Static => "static",
            SymbolKind::Macro => "macro",
            SymbolKind::Impl => "impl",
            SymbolKind::Mod => "mod",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "function" => SymbolKind::Function,
            "method" => SymbolKind::Method,
            "closure" => SymbolKind::Closure,
            "struct" => SymbolKind::Struct,
            "enum" => SymbolKind::Enum,
            "trait" => SymbolKind::Trait,
            "type" => SymbolKind::Type,
            "const" => SymbolKind::Const,
            "static" => SymbolKind::Static,
            "macro" => SymbolKind::Macro,
            "impl" => SymbolKind::Impl,
            "mod" => SymbolKind::Mod,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SymbolRow {
    pub id: i64,
    pub repo_id: i64,
    pub file_id: i64,
    pub kind: SymbolKind,
    pub name: String,
    pub qual_path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub parent_id: Option<i64>,
    pub is_pub: bool,
    /// Joined from files table for convenience.
    #[serde(default)]
    pub file_path: Option<String>,
}

/// Symbol as produced by the parser, before DB insertion.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSymbol {
    pub kind: SymbolKind,
    pub name: String,
    pub qual_path: String,
    pub start_line: i64,
    pub end_line: i64,
    /// Qual path of the enclosing symbol, if any.
    pub parent_qual: Option<String>,
    pub is_pub: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    DirectFn,
    DynamicMethod,
    Callback,
    FnPtr,
    Macro,
}

impl EdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeKind::DirectFn => "direct_fn",
            EdgeKind::DynamicMethod => "dynamic_method",
            EdgeKind::Callback => "callback",
            EdgeKind::FnPtr => "fn_ptr",
            EdgeKind::Macro => "macro",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "direct_fn" => EdgeKind::DirectFn,
            "dynamic_method" => EdgeKind::DynamicMethod,
            "callback" => EdgeKind::Callback,
            "fn_ptr" => EdgeKind::FnPtr,
            "macro" => EdgeKind::Macro,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EdgeSource {
    Ast,
    Llm,
}

impl EdgeSource {
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeSource::Ast => "ast",
            EdgeSource::Llm => "llm",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EdgeRow {
    pub id: i64,
    pub repo_id: i64,
    pub caller_id: i64,
    pub callee_id: Option<i64>,
    pub callee_name: String,
    pub kind: EdgeKind,
    pub source: EdgeSource,
    pub confidence: f64,
    pub evidence: Option<String>,
    pub task_id: Option<i64>,
}

/// Edge ready for insertion: caller already resolved to a symbol id.
#[derive(Debug, Clone, PartialEq)]
pub struct NewEdge {
    pub caller_id: i64,
    pub callee_name: String,
    pub kind: EdgeKind,
    pub source: EdgeSource,
    pub confidence: f64,
    pub evidence: Option<String>,
    pub task_id: Option<i64>,
}

/// Call site as produced by the parser (caller referenced by qual path).
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCall {
    pub caller_qual: String,
    pub callee_name: String,
    pub kind: EdgeKind,
    pub line: i64,
    pub source: EdgeSource,
    pub confidence: f64,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Done,
    Failed,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Done => "done",
            TaskStatus::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "pending" => TaskStatus::Pending,
            "running" => TaskStatus::Running,
            "done" => TaskStatus::Done,
            "failed" => TaskStatus::Failed,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskRow {
    pub id: i64,
    pub repo_id: i64,
    pub r#type: String,
    pub target_symbol_id: Option<i64>,
    pub payload: Option<String>,
    pub status: TaskStatus,
    pub attempts: i64,
    pub result: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunRow {
    pub id: i64,
    pub repo_id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub trigger: String,
    pub files_changed: i64,
    pub symbols: i64,
    pub edges: i64,
    pub duration_ms: Option<i64>,
    pub status: String,
}
