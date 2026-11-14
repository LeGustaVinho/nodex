use nodex_lang_rust::lower_source;

#[test]
fn explains_specific_import_on_its_owning_module_only() {
    let ir = lower_source("src/lib.rs", "use a::b::C; mod inner { use x::y::Z; }");
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none() && symbol.kind == nodex_ir::SymbolKind::Module).unwrap();
    let root_imports = root.rust_contract.as_ref().unwrap();
    let explanation = root_imports.iter().find(|item| item.text.contains("a::b::C")).unwrap();
    assert!(explanation.text.contains("name C into this module's scope"));
    assert!(explanation.text.contains("not every item"));
    assert!(!root_imports.iter().any(|item| item.text.contains("x::y::Z")));
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    assert!(inner.rust_contract.as_ref().unwrap().iter().any(|item| item.text.contains("x::y::Z")));
}
