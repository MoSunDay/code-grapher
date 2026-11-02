//! tree-sitter based source parsing: extracts symbols (incl. definitions
//! outside functions) and static call sites per file.

use crate::lang::Lang;
use crate::model::{EdgeKind, EdgeSource, ParsedCall, ParsedSymbol, SymbolKind};
use anyhow::{anyhow, Result};
use tree_sitter::{Node, Parser};

/// Full parse result for one source file.
#[derive(Debug, Default, Clone)]
pub struct ParsedFile {
    pub lang: Option<Lang>,
    pub loc: i64,
    pub symbols: Vec<ParsedSymbol>,
    pub calls: Vec<ParsedCall>,
}

/// Parse `source` according to `lang` (language registry dispatch).
pub fn parse(lang: Lang, source: &str) -> Result<ParsedFile> {
    match lang {
        Lang::Rust => parse_rust(source),
    }
}

fn new_rust_parser() -> Result<Parser> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
    Ok(parser)
}

/// FNV-1a 64-bit content hash (stable across runs; hex encoded).
pub fn fnv1a_hex(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn line_of(node: Node) -> i64 {
    node.start_position().row as i64 + 1
}

fn end_line_of(node: Node) -> i64 {
    node.end_position().row as i64 + 1
}

fn text<'a>(node: Node, src: &'a str) -> &'a str {
    node.utf8_text(src.as_bytes()).unwrap_or("")
}

fn is_public(node: Node) -> bool {
    node.child(0).is_some_and(|first| first.kind() == "visibility_modifier")
}

/// Walker accumulator (kept flat & explicit).
#[derive(Default)]
struct Walk {
    symbols: Vec<ParsedSymbol>,
    calls: Vec<ParsedCall>,
    /// Fully-qualified path of the innermost container (impl/trait/mod/fn).
    current_qual: Option<String>,
    /// Qual path of the innermost function-like symbol (call attribution).
    current_fn: Option<String>,
    in_impl: bool,
    in_trait: bool,
}

impl Walk {
    fn prefix(&self) -> String {
        match &self.current_qual {
            Some(q) => format!("{q}::"),
            None => String::new(),
        }
    }

    fn push_symbol(&mut self, kind: SymbolKind, name: String, start: i64, end: i64, is_pub: bool) -> String {
        let qual = format!("{}{}", self.prefix(), name);
        self.symbols.push(ParsedSymbol {
            kind,
            name,
            qual_path: qual.clone(),
            start_line: start,
            end_line: end,
            parent_qual: self.current_qual.clone(),
            is_pub,
        });
        qual
    }

    /// Walk children with `qual` as the enclosing scope.
    fn descend(&mut self, qual: String, f: impl FnOnce(&mut Walk)) {
        let prev = self.current_qual.replace(qual);
        f(self);
        self.current_qual = prev;
    }
}

fn parse_rust(source: &str) -> Result<ParsedFile> {
    let mut parser = new_rust_parser().map_err(|e| anyhow!("tree-sitter init: {e}"))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow!("tree-sitter failed to parse"))?;
    let mut w = Walk::default();
    walk_node(tree.root_node(), source, &mut w);
    Ok(ParsedFile {
        lang: Some(Lang::Rust),
        loc: source.lines().count() as i64,
        symbols: w.symbols,
        calls: w.calls,
    })
}

fn walk_node(node: Node, src: &str, w: &mut Walk) {
    match node.kind() {
        "function_item" | "function_signature_item" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| text(n, src).to_string())
                .unwrap_or_else(|| "<anon>".into());
            let kind = if w.in_impl || w.in_trait { SymbolKind::Method } else { SymbolKind::Function };
            let qual = w.push_symbol(kind, name, line_of(node), end_line_of(node), is_public(node));
            let prev_fn = w.current_fn.replace(qual.clone());
            w.descend(qual, |w| walk_children(node, src, w));
            w.current_fn = prev_fn;
        }
        "closure_expression" => {
            let pos = node.start_position();
            let name = format!("closure@{}:{}", pos.row + 1, pos.column + 1);
            let qual = w.push_symbol(SymbolKind::Closure, name, line_of(node), end_line_of(node), false);
            let prev_fn = w.current_fn.replace(qual.clone());
            w.descend(qual, |w| walk_children(node, src, w));
            w.current_fn = prev_fn;
        }
        "struct_item" | "enum_item" | "trait_item" | "type_item" | "union_item" => {
            let kind = match node.kind() {
                "struct_item" => SymbolKind::Struct,
                "enum_item" => SymbolKind::Enum,
                "trait_item" => SymbolKind::Trait,
                _ => SymbolKind::Type,
            };
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = text(name_node, src).to_string();
                let qual = w.push_symbol(kind, name, line_of(node), end_line_of(node), is_public(node));
                if node.kind() == "trait_item" {
                    let prev = w.in_trait;
                    w.in_trait = true;
                    w.descend(qual, |w| walk_children(node, src, w));
                    w.in_trait = prev;
                    return;
                }
            }
            walk_children(node, src, w);
        }
        "const_item" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                w.push_symbol(SymbolKind::Const, text(name_node, src).to_string(), line_of(node), end_line_of(node), is_public(node));
            }
            // const initialisers may contain closures/calls
            walk_children(node, src, w);
        }
        "static_item" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                w.push_symbol(SymbolKind::Static, text(name_node, src).to_string(), line_of(node), end_line_of(node), is_public(node));
            }
            walk_children(node, src, w);
        }
        "macro_definition" => {
            // macro_rules! name: first identifier child
            let mut cursor = node.walk();
            let mut found = None;
            for c in node.children(&mut cursor) {
                if c.kind() == "identifier" {
                    found = Some(c);
                    break;
                }
            }
            if let Some(id) = found {
                w.push_symbol(SymbolKind::Macro, text(id, src).to_string(), line_of(node), end_line_of(node), true);
            }
        }
        "impl_item" => {
            let label = impl_label(node, src);
            let qual = w.push_symbol(SymbolKind::Impl, label, line_of(node), end_line_of(node), false);
            let prev_in_impl = w.in_impl;
            w.in_impl = true;
            w.descend(qual, |w| walk_children(node, src, w));
            w.in_impl = prev_in_impl;
        }
        "mod_item" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let label = text(name_node, src).to_string();
                let qual = w.push_symbol(SymbolKind::Mod, label, line_of(node), end_line_of(node), is_public(node));
                w.descend(qual, |w| walk_children(node, src, w));
            }
        }
        "call_expression" => {
            record_call(node, src, w);
            walk_children(node, src, w);
        }
        "macro_invocation" => {
            record_macro_call(node, src, w);
            walk_children(node, src, w);
        }
        _ => walk_children(node, src, w),
    }
}

fn walk_children(node: Node, src: &str, w: &mut Walk) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node(child, src, w);
    }
}

fn impl_label(node: Node, src: &str) -> String {
    let type_name = node.child_by_field_name("type").map(|n| type_text(n, src));
    let trait_name = node.child_by_field_name("trait").map(|n| text(n, src).to_string());
    match (trait_name, type_name) {
        (Some(t), Some(ty)) => format!("{t} for {ty}"),
        (None, Some(ty)) => ty,
        _ => "<impl>".to_string(),
    }
}

/// `Foo<T>` generic_type -> "Foo"; otherwise the node's own text.
fn type_text(node: Node, src: &str) -> String {
    if node.kind() == "generic_type" {
        if let Some(inner) = node.named_child(0) {
            return type_text(inner, src);
        }
    }
    text(node, src).to_string()
}

/// Record one call site under the innermost enclosing function-like symbol.
fn record_call(node: Node, src: &str, w: &mut Walk) {
    let Some(caller_qual) = w.current_fn.clone() else { return };
    let Some(func) = node.child_by_field_name("function") else { return };
    let (callee_name, kind) = match func.kind() {
        "identifier" => (text(func, src).to_string(), EdgeKind::DirectFn),
        "scoped_identifier" | "scoped_type_identifier" => (text(func, src).to_string(), EdgeKind::DirectFn),
        "field_expression" => {
            let m = func
                .child_by_field_name("field")
                .map(|f| text(f, src).to_string())
                .unwrap_or_default();
            (m, EdgeKind::DynamicMethod)
        }
        "generic_function" => {
            // `foo::<T>()` / `Vec::<u8>::new()` shapes
            let inner = func.named_child(0).unwrap_or(func);
            let (name, kind) = match inner.kind() {
                "identifier" => (text(inner, src).to_string(), EdgeKind::DirectFn),
                "field_expression" => (
                    inner
                        .child_by_field_name("field")
                        .map(|f| text(f, src).to_string())
                        .unwrap_or_default(),
                    EdgeKind::DynamicMethod,
                ),
                _ => (text(inner, src).to_string(), EdgeKind::DirectFn),
            };
            push_call(w, &caller_qual, name, kind, node);
            record_callback_args(node, src, w);
            return;
        }
        _ => return,
    };
    push_call(w, &caller_qual, callee_name, kind, node);
    record_callback_args(node, src, w);
}

fn record_macro_call(node: Node, src: &str, w: &mut Walk) {
    let Some(caller_qual) = w.current_fn.as_ref() else { return };
    let name = node
        .child_by_field_name("macro")
        .map(|m| text(m, src).to_string())
        .unwrap_or_default();
    if name.is_empty() {
        return;
    }
    w.calls.push(ParsedCall {
        caller_qual: caller_qual.clone(),
        callee_name: name,
        kind: EdgeKind::Macro,
        line: line_of(node),
        source: EdgeSource::Ast,
        confidence: 1.0,
        evidence: Some(format!("macro invocation at line {}", line_of(node))),
    });
}

fn push_call(w: &mut Walk, caller_qual: &str, callee_name: String, kind: EdgeKind, node: Node) {
    if callee_name.is_empty() {
        return;
    }
    w.calls.push(ParsedCall {
        caller_qual: caller_qual.to_string(),
        callee_name,
        kind,
        line: line_of(node),
        source: EdgeSource::Ast,
        confidence: 1.0,
        evidence: None,
    });
}

/// Bare identifiers (or `&ident`) passed as arguments: potential callback edges.
fn record_callback_args(node: Node, src: &str, w: &mut Walk) {
    let Some(args) = node.child_by_field_name("arguments") else { return };
    for arg in args.named_children(&mut args.walk()) {
        let id = match arg.kind() {
            "identifier" => Some(arg),
            "reference_expression" => arg.named_child(0).filter(|c| c.kind() == "identifier"),
            _ => None,
        };
        if let Some(id) = id {
            w.calls.push(ParsedCall {
                caller_qual: w.current_fn.clone().unwrap_or_default(),
                callee_name: text(id, src).to_string(),
                kind: EdgeKind::Callback,
                line: line_of(id),
                source: EdgeSource::Ast,
                confidence: 0.4,
                evidence: Some("identifier passed as call argument".into()),
            });
        }
    }
}
