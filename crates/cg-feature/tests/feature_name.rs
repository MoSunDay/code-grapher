//! MR name rules (branch ref safety + URL path segment safety).

use cg_feature::validate_feature_name;

#[test]
fn safe_names_pass() {
    for name in [
        "AST解析与领域模型",
        "请求链路推断",
        "request-chain_v2",
        "edge.2026",
        "MR-42",
    ] {
        assert!(validate_feature_name(name).is_ok(), "{name}");
    }
}

#[test]
fn spaces_are_no_longer_valid() {
    // v1 allowed spaces (directory names); branch refs never did.
    assert!(validate_feature_name("AST 解析").is_err());
}

#[test]
fn reserved_and_traversal_names_fail() {
    for name in [
        "", ".", "..", ".hidden", "_changelog", "_x", "a/b", "/a", "a/", "a.", "a..b",
        "mr.lock", "a~b", "a^b", "a:b", "a?b", "a*b", "a[b", "a\\b", "a\nb",
    ] {
        assert!(validate_feature_name(name).is_err(), "{name:?}");
    }
}

#[test]
fn byte_length_cap_enforced() {
    let name = "长".repeat(42); // 126 bytes
    assert!(validate_feature_name(&name).is_ok());
    let too_long = "长".repeat(43); // 129 bytes
    assert!(validate_feature_name(&too_long).is_err());
}
