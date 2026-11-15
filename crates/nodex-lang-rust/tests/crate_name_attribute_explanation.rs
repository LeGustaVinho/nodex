use nodex_lang_rust::lower_source;
#[test]
fn crate_name_attribute_is_explained_on_file_module_only_without_assuming_build_identity() {
    let source = "#![crate_name = \"my_crate\"]\n#![allow(unused)]\nfn run() {} mod nested { #![crate_name = \"not_root\"] fn f() {} } #[crate_name = \"outer\"] fn wrong() {}";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.id == "ns:crate::lib").or_else(|| ir.symbols.iter().find(|symbol| symbol.kind == nodex_ir::SymbolKind::Module && symbol.parent.is_none())).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Crate name attribute").collect();
    assert_eq!(items.len(), 1, "{:?}", root.rust_contract);
    let item = items[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#![crate_name = \"my_crate\"]");
    assert!(item.text.contains("when placed at the crate root") && item.text.contains("\"my_crate\"") && item.text.contains("must agree") && item.text.contains("does not by itself establish"));
    for name in ["run", "nested", "wrong"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Crate name attribute"), "{name}");
    }
}
