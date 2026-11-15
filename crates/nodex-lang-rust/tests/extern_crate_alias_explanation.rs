use nodex_lang_rust::lower_source;

#[test]
fn explains_crate_alias_in_its_module_without_renaming_underlying_crate() {
    let source = "extern crate foo as bar; extern crate quux; mod inner { extern crate other as local; }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.kind == nodex_ir::SymbolKind::Module && symbol.parent.is_none()).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "External crate alias").collect();
    assert_eq!(items.len(), 1, "{:?}", root.rust_contract);
    let item = items[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "extern crate foo as bar;");
    assert!(item.text.contains("crate named foo") && item.text.contains("local name bar") && item.text.contains("not the underlying crate") && item.text.contains("does not prove"));
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    assert!(inner.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "External crate alias" && item.text.contains("crate named other") && item.text.contains("local name local")));
}
