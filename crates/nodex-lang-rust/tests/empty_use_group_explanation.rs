use nodex_lang_rust::lower_source;

#[test]
fn empty_use_group_imports_no_name_but_path_must_be_accessible() {
    let source = "use a::{}; use b::{/* empty */}; use c::{x}; mod nested { use crate::d::{}; }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.kind == nodex_ir::SymbolKind::Module && symbol.parent.is_none()).unwrap();
    let nested = ir.symbols.iter().find(|symbol| symbol.name == "nested").unwrap();
    for (symbol, expected) in [(root, vec!["a", "b"]), (nested, vec!["crate::d"])] {
        let facts: Vec<_> = symbol.rust_contract.as_ref().into_iter().flatten().filter(|item| item.group == "Empty import group").collect();
        assert_eq!(facts.len(), expected.len(), "{}: {:?}", symbol.name, symbol.rust_contract);
        for name in expected {
            let item = facts.iter().find(|item| item.text.contains(&format!("`use {name}::{{}};`"))).unwrap();
            assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("use "));
            for phrase in ["imports no names", "creates no binding", "resolves and is accessible", "does not establish"] { assert!(item.text.contains(phrase), "{}", item.text); }
        }
    }
}
