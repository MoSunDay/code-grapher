fn main() {
    let src = include_str!("../tests/fixtures/sample.rs");
    let out = cg_core::parser::parse(cg_core::lang::Lang::Rust, src).unwrap();
    for s in &out.symbols {
        println!("{:?} {} parent={:?}", s.kind, s.qual_path, s.parent_qual);
    }
    println!("--- calls:");
    for c in &out.calls {
        println!("{} -> {} ({:?})", c.caller_qual, c.callee_name, c.kind);
    }
}
