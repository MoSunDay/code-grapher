//! Snapshot-style assertions over tests/fixtures/sample.rs.

use cg_core::lang::Lang;
use cg_core::model::{EdgeKind, SymbolKind};
use cg_core::parser::{fnv1a_hex, parse};
use std::collections::HashMap;

fn kinds(syms: &[cg_core::model::ParsedSymbol]) -> HashMap<cg_core::model::SymbolKind, usize> {
    let mut m: HashMap<cg_core::model::SymbolKind, usize> = HashMap::new();
    for s in syms {
        *m.entry(s.kind).or_default() += 1;
    }
    m
}

#[test]
fn parses_sample_fixture() {
    let src = include_str!("fixtures/sample.rs");
    let out = parse(Lang::Rust, src).expect("parse");

    let k = kinds(&out.symbols);
    assert_eq!(k[&SymbolKind::Function], 7, "main, abs_diff, apply, run_all, nested, hidden, inner");
    assert_eq!(k[&SymbolKind::Method], 6, "fmt, new, norm, distance, draw, desc");
    assert_eq!(k[&SymbolKind::Closure], 2, "handler, add");
    assert_eq!(k[&SymbolKind::Struct], 1);
    assert_eq!(k[&SymbolKind::Enum], 1);
    assert_eq!(k[&SymbolKind::Trait], 1);
    assert_eq!(k[&SymbolKind::Const], 1);
    assert_eq!(k[&SymbolKind::Static], 1);
    assert_eq!(k[&SymbolKind::Type], 1);
    assert_eq!(k[&SymbolKind::Macro], 1);
    assert_eq!(k[&SymbolKind::Impl], 2);
    assert_eq!(k[&SymbolKind::Mod], 1);

    // qualified paths
    let quals: Vec<&str> = out.symbols.iter().map(|s| s.qual_path.as_str()).collect();
    assert!(quals.contains(&"Point::new"), "impl-scoped method: {quals:?}");
    assert!(quals.contains(&"fmt::Display for Point::fmt"));
    assert!(quals.contains(&"inner_mod::hidden"));
    assert!(quals.contains(&"Drawable::desc"));
    assert!(quals.contains(&"square"));
    // closure inside method carries parent
    let handler = out
        .symbols
        .iter()
        .find(|s| s.kind == SymbolKind::Closure && s.parent_qual.as_deref() == Some("Point::norm"))
        .expect("closure symbol");
    assert!(handler.qual_path.starts_with("Point::norm::closure@"));

    // visibility
    let pt = out.symbols.iter().find(|s| s.name == "Point").unwrap();
    assert!(pt.is_pub);
    let nd = out.symbols.iter().find(|s| s.qual_path == "Point::norm").unwrap();
    assert!(!nd.is_pub);

    // calls attributed correctly
    let find_calls = |caller: &str, name: &str, kind: EdgeKind| {
        out.calls
            .iter()
            .any(|c| c.caller_qual == caller && c.callee_name == name && c.kind == kind)
    };
    assert!(find_calls("main", "Point::new", EdgeKind::DirectFn));
    assert!(find_calls("main", "clone", EdgeKind::DynamicMethod));
    assert!(find_calls("main", "vec", EdgeKind::Macro));
    assert!(find_calls("main", "square", EdgeKind::Macro));
    assert!(find_calls("main", "format_args", EdgeKind::Macro));
    assert!(find_calls("main", "abs_diff", EdgeKind::Callback), "ident passed as arg");
    assert!(find_calls("main", "abs_diff", EdgeKind::Callback), "&ident passed as arg");
    assert!(find_calls("Point::norm", "handler", EdgeKind::DirectFn));
    assert!(find_calls("Point::distance", "abs_diff", EdgeKind::DirectFn));
    assert!(find_calls("nested", "inner", EdgeKind::DirectFn));
    assert!(find_calls("fmt::Display for Point::fmt", "write", EdgeKind::Macro));
    // closure value called as a local: attributed to main
    assert!(find_calls("main", "add", EdgeKind::DirectFn));
    let add_qual = out
        .symbols
        .iter()
        .find(|s| s.kind == SymbolKind::Closure && s.parent_qual.as_deref() == Some("main"))
        .unwrap()
        .qual_path
        .clone();
    assert!(add_qual.starts_with("main::closure@"));

    // pre-order guarantee: parents appear before children
    let pos = |q: &str| out.symbols.iter().position(|s| s.qual_path == q).unwrap();
    assert!(pos("Point") < pos("Point::norm"));
    assert!(pos("Point::norm") < pos(handler.qual_path.as_str()));

    // loc
    assert_eq!(out.loc, src.lines().count() as i64);
}

#[test]
fn hash_is_stable_fnv1a() {
    assert_eq!(fnv1a_hex(b""), "cbf29ce484222325");
    assert_eq!(fnv1a_hex(b"a"), fnv1a_hex(b"a"));
    assert_ne!(fnv1a_hex(b"a"), fnv1a_hex(b"b"));
}
