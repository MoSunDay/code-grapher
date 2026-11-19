//! Feature-name identity rules: name is the only ID and becomes the directory.

use cg_feature::{parse_feature, validate_feature_name};

fn body(name: serde_json::Value) -> String {
    let mut json = serde_json::json!({
        "function_entry": "main",
        "request_chain": [],
        "external_dependencies": [],
        "function_summary": "…",
        "children": []
    });
    json["name"] = name;
    json.to_string()
}

#[test]
fn legacy_id_field_is_rejected() {
    let mut json: serde_json::Value = serde_json::from_str(&body("功能A".into())).unwrap();
    json["id"] = serde_json::json!("f-42");
    let err = parse_feature(json.to_string().as_bytes()).unwrap_err();
    assert!(err.to_string().contains("id"), "{err}");
}

#[test]
fn missing_name_is_rejected() {
    let json = r#"{"function_entry":"main","request_chain":[],"external_dependencies":[],"function_summary":"","children":[]}"#;
    assert!(parse_feature(json.as_bytes()).is_err());
}

#[test]
fn unsafe_names_are_rejected() {
    for bad in [
        "", ".", "..", "a/b", "a\\b", "a\u{0}b", "a\u{1}b", " padded ", "\tx",
        "_changelog", "_reserved", "_anything", &"x".repeat(129),
    ] {
        assert!(validate_feature_name(bad).is_err(), "{bad:?}");
        assert!(parse_feature(body(bad.to_string().into()).as_bytes()).is_err(), "{bad:?}");
    }
}

#[test]
fn changelog_is_reserved_for_the_namespace() {
    assert!(validate_feature_name("_changelog").is_err());
    assert!(validate_feature_name("changelog").is_ok());
}

#[test]
fn chinese_and_spaces_are_valid_names() {
    for good in ["AST 解析与领域模型", "请求链路推断", "v1.2_rc-1"] {
        let feature = parse_feature(body(good.to_string().into()).as_bytes()).unwrap();
        assert_eq!(feature.name, good);
    }
}

#[test]
fn names_are_case_sensitive_identities() {
    let upper = parse_feature(body("AST".into()).as_bytes()).unwrap();
    let lower = parse_feature(body("ast".into()).as_bytes()).unwrap();
    assert_ne!(upper.name, lower.name);
}
