//! Prompt construction and LLM reply parsing for hidden-call analysis.

use cg_core::model::{EdgeKind, SymbolRow};
use serde::{Deserialize, Serialize};

/// Max source lines of the target function included in the prompt.
const MAX_FN_LINES: usize = 120;
/// Max candidate symbols listed in the prompt.
const MAX_CANDIDATES: usize = 40;

#[derive(Debug, Clone)]
pub struct FnContext {
    pub symbol: SymbolRow,
    /// Function source text (already sliced by caller).
    pub source: String,
    /// Repository-relative file path.
    pub file_path: String,
    /// Extra file context: a few enclosing lines.
    pub context: String,
    pub candidates: Vec<SymbolRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmCall {
    pub callee_name: String,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub line: Option<i64>,
    pub kind: String,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default)]
    pub evidence: Option<String>,
}

fn default_confidence() -> f64 {
    0.5
}

impl LlmCall {
    pub fn edge_kind(&self) -> Option<EdgeKind> {
        EdgeKind::parse(&self.kind)
    }
}

pub fn build_prompt(ctx: &FnContext) -> String {
    let mut candidates = String::new();
    for (i, c) in ctx.candidates.iter().take(MAX_CANDIDATES).enumerate() {
        candidates.push_str(&format!(
            "{}. {} `{}` — {}:{}\n",
            i + 1,
            c.kind.as_str(),
            c.qual_path,
            c.file_path.as_deref().unwrap_or("?"),
            c.start_line
        ));
    }
    if candidates.is_empty() {
        candidates.push_str("(none)\n");
    }
    format!(
        r#"You are a static-analysis assistant for a Rust repository.
Identify call relationships in the target function that a syntax-only parser CANNOT resolve:
- method calls on values/traits (x.foo()) -> kind "dynamic_method"
- function values passed as arguments and invoked later -> kind "callback"
- calls through function pointers, fields, or tables -> kind "fn_ptr"
- macros that expand to real function calls -> kind "macro"

TARGET FUNCTION: `{qual}`
FILE: {file}:{start}-{end}

```rust
{source}
```

SURROUNDING CONTEXT:
```rust
{context}
```

KNOWN SYMBOLS in this repository (candidates for resolution):
{candidates}

Rules:
- Only include calls MADE BY the target function (directly or via the values it passes around).
- Prefer callee_name matching a KNOWN SYMBOL name exactly; otherwise use the name as written in code.
- confidence: 0.0-1.0.

Reply with ONLY a JSON array (no markdown fences, no prose):
[{{"callee_name":"...","file":"relative/path.rs","line":123,"kind":"dynamic_method","confidence":0.8,"evidence":"short quote/reason"}}]
If nothing qualifies, reply exactly: []"#,
        qual = ctx.symbol.qual_path,
        file = ctx.file_path,
        start = ctx.symbol.start_line,
        end = ctx.symbol.end_line,
        source = truncate_lines(&ctx.source, MAX_FN_LINES),
        context = truncate_lines(&ctx.context, 40),
        candidates = candidates,
    )
}

fn truncate_lines(s: &str, max: usize) -> String {
    let n = s.lines().count();
    if n <= max {
        return s.to_string();
    }
    let kept: Vec<&str> = s.lines().take(max).collect();
    format!("{}\n... ({} more lines truncated)", kept.join("\n"), n - max)
}

/// Parse the assistant reply into validated call suggestions.
/// Tolerates markdown fences and stray prose around the JSON array.
pub fn parse_reply(reply: &str) -> Vec<LlmCall> {
    let Some(start) = reply.find('[') else {
        return Vec::new();
    };
    let Some(end) = reply.rfind(']') else {
        return Vec::new();
    };
    if end <= start {
        return Vec::new();
    }
    let slice = &reply[start..=end];
    match serde_json::from_str::<Vec<LlmCall>>(slice) {
        Ok(mut calls) => {
            calls.retain(|c| !c.callee_name.is_empty() && (0.0..=1.0).contains(&c.confidence));
            calls.dedup_by(|a, b| a.callee_name == b.callee_name && a.kind == b.kind);
            calls
        }
        Err(e) => {
            tracing::warn!(error = %e, "invalid LLM JSON reply");
            Vec::new()
        }
    }
}

/// Snake_case name tokens used to recall candidate symbols.
pub fn name_tokens(name: &str) -> Vec<String> {
    name.split(['_', ':', '<', '>'])
        .filter(|t| t.len() >= 3)
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clean_and_fenced_replies() {
        let clean = r#"[{"callee_name":"handler","kind":"callback","confidence":0.9,"evidence":"passed to apply"}]"#;
        assert_eq!(parse_reply(clean).len(), 1);
        let fenced = "Here you go:\n```json\n[{\"callee_name\":\"run\",\"kind\":\"dynamic_method\",\"confidence\":0.7}]\n```\nDone.";
        let calls = parse_reply(fenced);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].callee_name, "run");
        assert_eq!(calls[0].edge_kind(), Some(EdgeKind::DynamicMethod));
    }

    #[test]
    fn empty_and_garbage_replies() {
        assert!(parse_reply("[]").is_empty());
        assert!(parse_reply("I cannot help with that.").is_empty());
        // confidence out of range dropped
        assert!(parse_reply(r#"[{"callee_name":"x","kind":"callback","confidence":1.5}]"#).is_empty());
    }

    #[test]
    fn tokens_split_snake_case() {
        assert_eq!(name_tokens("parse_all_files"), vec!["parse", "all", "files"]);
    }
}
