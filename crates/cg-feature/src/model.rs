//! MR name model: a name is the sole feature identity and becomes the branch
//! `mr/<name>` inside the per-repository git document repo.

use anyhow::{bail, Result};

/// Longest allowed MR name, in UTF-8 bytes.
pub const MAX_FEATURE_NAME_BYTES: usize = 128;

/// Branch namespace prefix for MR (draft) branches.
pub const MR_PREFIX: &str = "mr/";

/// Full branch ref name for an MR: `mr/<name>`.
pub fn mr_branch(name: &str) -> String {
    format!("{MR_PREFIX}{name}")
}

/// Extract the MR name from a branch ref name, if it is one.
pub fn mr_name_from_branch(branch: &str) -> Option<&str> {
    branch.strip_prefix(MR_PREFIX)
}

/// Validate an MR name before it becomes a git branch component.
///
/// Allowed: unicode text (including Chinese), letters, digits, `-`, `_`,
/// `.`, `/`-free single component. Rejected: empty, `.`/`..`, whitespace and
/// control characters, git ref metacharacters (`~ ^ : ? * [ \\ /`), the
/// sequences `..` and trailing `.lock`, a leading `.` or `/`, the `_`-prefixed
/// reserved namespace (e.g. `_changelog`), and names longer than
/// [`MAX_FEATURE_NAME_BYTES`].
pub fn validate_feature_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("feature name must not be empty");
    }
    if name.len() > MAX_FEATURE_NAME_BYTES {
        bail!(
            "feature name exceeds {MAX_FEATURE_NAME_BYTES} bytes (UTF-8), got {}",
            name.len()
        );
    }
    if name == "." || name == ".." {
        bail!("feature name must not be {name:?}");
    }
    if name.starts_with('.') {
        bail!("feature name must not start with '.', got {name:?}");
    }
    if name.starts_with('_') {
        bail!("feature name must not start with '_' (reserved namespace), got {name:?}");
    }
    if name.starts_with('/') || name.ends_with('/') || name.ends_with('.') {
        bail!("feature name must not start with '/' or end with '/' or '.', got {name:?}");
    }
    if name.ends_with(".lock") {
        bail!("feature name must not end with '.lock', got {name:?}");
    }
    if name.contains("..") {
        bail!("feature name must not contain '..', got {name:?}");
    }
    for ch in name.chars() {
        if ch.is_control() || ch.is_whitespace() {
            bail!("feature name must not contain whitespace or control characters, got {name:?}");
        }
        if matches!(ch, '~' | '^' | ':' | '?' | '*' | '[' | '\\' | '/' | '\x7f') {
            bail!("feature name must not contain git ref metacharacter {ch:?}, got {name:?}");
        }
    }
    Ok(())
}

/// Render hint derived from a file path's extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RenderHint {
    Markdown,
    Json,
    Text,
}

/// Map a path to a rendering hint for the web viewer.
pub fn render_hint(path: &str) -> RenderHint {
    let lower = path.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "md" | "markdown" => RenderHint::Markdown,
        "json" | "jsonc" => RenderHint::Json,
        _ => RenderHint::Text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_safe_names() {
        for name in [
            "AST解析与领域模型",
            "request-chain_v2",
            "edge.2026",
            "a",
            &"x".repeat(MAX_FEATURE_NAME_BYTES),
        ] {
            assert!(validate_feature_name(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn rejects_unsafe_names() {
        for name in [
            "",
            ".",
            "..",
            ".hidden",
            "_changelog",
            "a b",
            "a/b",
            "a~b",
            "a^b",
            "a:b",
            "a?b",
            "a*b",
            "a[b",
            "a\\b",
            "a..b",
            "mr.lock",
            "a.",
            &"x".repeat(MAX_FEATURE_NAME_BYTES + 1),
        ] {
            assert!(validate_feature_name(name).is_err(), "{name}");
        }
    }

    #[test]
    fn branch_roundtrip() {
        assert_eq!(mr_branch("概览"), "mr/概览");
        assert_eq!(mr_name_from_branch("mr/概览"), Some("概览"));
        assert_eq!(mr_name_from_branch("main"), None);
    }

    #[test]
    fn hints_follow_extension() {
        assert_eq!(render_hint("docs/a.MD"), RenderHint::Markdown);
        assert_eq!(render_hint("x/index.json"), RenderHint::Json);
        assert_eq!(render_hint("x/y.txt"), RenderHint::Text);
        assert_eq!(render_hint("noext"), RenderHint::Text);
    }
}
