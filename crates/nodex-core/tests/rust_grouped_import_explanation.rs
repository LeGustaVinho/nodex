use nodex_lang_rust::lower_source;

#[test]
fn grouped_import_explains_shared_prefix_and_module_scope() {
    let source = "use a::{b, c}; mod inner { use z::{d, e}; }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.parent.is_none()).unwrap();
    let items = root.rust_contract.as_ref().unwrap();
    let item = items.iter().find(|item| item.group == "Grouped import").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "use a::{b, c};");
    assert!(item.text.contains("a::b") && item.text.contains("a::c"));
    assert!(!item.text.contains("z::d"));
    let inner = ir.symbols.iter().find(|s| s.name == "inner").unwrap();
    assert!(inner.rust_contract.as_ref().unwrap().iter().any(|item|
        item.group == "Grouped import" && item.text.contains("z::d") && item.text.contains("z::e")));
}
