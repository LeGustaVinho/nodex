use nodex_lang_rust::lower_source;

#[test]
fn macro_use_distinguishes_module_textual_scope_from_external_crate_prelude() {
    let source = "#[macro_use(foo)] extern crate dep; #[macro_use] mod legacy { macro_rules! foo { () => {} } } #[macro_use] mod external; mod plain {} fn ordinary() {}";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.parent.is_none()).unwrap();
    let root_items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Import macros from an external crate").collect();
    assert_eq!(root_items.len(), 1, "{:?}", root.rust_contract);
    assert_eq!(&source[root_items[0].span.start_byte as usize..root_items[0].span.end_byte as usize], "#[macro_use(foo)]");
    assert!(root_items[0].text.contains("extern crate dep") && root_items[0].text.contains("macro-use prelude") && root_items[0].text.contains("listed set of names"));
    for name in ["legacy", "external"] {
        let symbol = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Import macros from a module").unwrap();
        assert!(item.text.contains("textual scope") && item.text.contains("after the module declaration"));
    }
    for name in ["plain", "ordinary"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group.starts_with("Import macros from")));
    }
}
