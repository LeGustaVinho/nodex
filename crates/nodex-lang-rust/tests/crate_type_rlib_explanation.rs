use nodex_lang_rust::lower_source;

#[test]
fn rlib_crate_type_explains_artifact_and_command_line_precedence() {
    let source = "#![crate_type = \"rlib\"] #![crate_type = \"cdylib\"] #![cfg_attr(test, crate_type = \"rlib\")] mod nested { #![crate_type = \"rlib\"] fn f() {} } #[crate_type = \"rlib\"] fn other() {}";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.kind == nodex_ir::SymbolKind::Module && symbol.parent.is_none()).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Rust library crate type").collect();
    assert_eq!(items.len(), 1, "{:?}", root.rust_contract);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "#![crate_type = \"rlib\"]");
    assert!(items[0].text.contains("when this file is used as the crate root") && items[0].text.contains("metadata") && items[0].text.contains("staticlib") && items[0].text.contains("command-line choices") && items[0].text.contains("not established"));
    for name in ["nested", "other"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Rust library crate type"));
    }
}
