//! Feature model: `name` is the sole identity and doubles as the directory name.
//!
//! The legacy `id` field is rejected as an unknown field, so a dual-identity
//! JSON can never re-enter the system.

use crate::git_url::sha256_hex;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Longest allowed feature name, in UTF-8 bytes.
pub const MAX_FEATURE_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feature {
    pub name: String,
    pub function_entry: Value,
    pub request_chain: Value,
    pub external_dependencies: Value,
    pub function_summary: Value,
    pub children: Value,
}

/// Parse feature JSON, rejecting unknown fields (including the legacy `id`)
/// and any unsafe `name`.
pub fn parse_feature(bytes: &[u8]) -> Result<Feature> {
    let feature: Feature = serde_json::from_slice(bytes).map_err(|error| {
        if error.to_string().contains("unknown field") {
            anyhow::anyhow!(
                "{error}; the `id` field is gone: `name` is the feature identity"
            )
        } else {
            anyhow::Error::new(error)
        }
    })?;
    validate_feature_name(&feature.name)?;
    Ok(feature)
}

/// Stable canonical serialization for on-disk bytes and hashing.
pub fn canonical_feature_bytes(feature: &Feature) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(feature)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// SHA-256 of the exact feature.json bytes, used as the review hash.
pub fn feature_hash(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}

/// Validate a feature name before it becomes a directory component.
///
/// Allowed: unicode text (including Chinese), spaces, letters, digits, `-`,
/// `_`, `.`. Rejected: empty, `.`/`..`, separators, control characters,
/// leading/trailing whitespace, the `_`-prefixed reserved namespace (e.g.
/// `_changelog`), leading `.`, and names longer than [`MAX_FEATURE_NAME_BYTES`].
pub fn validate_feature_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("feature name must not be empty");
    }
    if name == "." || name == ".." {
        bail!("feature name must not be {name:?}");
    }
    if name != name.trim() {
        bail!("feature name must not start or end with whitespace");
    }
    if name.starts_with('_') {
        bail!("feature names starting with '_' are reserved (e.g. _changelog)");
    }
    if name.starts_with('.') {
        bail!("feature name must not start with '.'");
    }
    if name.contains('/') || name.contains('\\') {
        bail!("feature name must not contain path separators");
    }
    if name.chars().any(|c| c.is_control()) {
        bail!("feature name must not contain control characters");
    }
    if name.len() > MAX_FEATURE_NAME_BYTES {
        bail!(
            "feature name exceeds {} bytes (got {})",
            MAX_FEATURE_NAME_BYTES,
            name.len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal(name: &str) -> String {
        serde_json::json!({
            "name": name,
            "function_entry": "main",
            "request_chain": [],
            "external_dependencies": [],
            "function_summary": "",
            "children": []
        })
        .to_string()
    }

    #[test]
    fn rejects_legacy_id_field() {
        let json = format!(
            "{{\"id\":\"f1\",{}",
            minimal("AST 解析").replacen('{', "", 1)
        );
        assert!(parse_feature(json.as_bytes()).is_err());
    }

    #[test]
    fn rejects_missing_or_unsafe_names() {
        let no_name = serde_json::json!({
            "function_entry": "main", "request_chain": [], "external_dependencies": [],
            "function_summary": "", "children": []
        });
        assert!(parse_feature(no_name.to_string().as_bytes()).is_err());
        for bad in [
            "", ".", "..", "a/b", "a\\b", " lead", "trail ", "_changelog", "_reserved",
            "\u{0}x", "x\u{7}y", ".hidden",
        ] {
            assert!(validate_feature_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn accepts_chinese_spaces_and_case() {
        for good in ["AST 解析与领域模型", "请求链路 推断", "v2.1-fast", "under_score", "A"] {
            validate_feature_name(good).unwrap();
            assert!(parse_feature(minimal(good).as_bytes()).is_ok(), "{good}");
        }
    }
}
