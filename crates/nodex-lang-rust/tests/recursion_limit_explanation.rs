use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn recursion_limit_preserves_written_value_on_file_module_without_scanning_nested_scope() {
    let source = "#![recursion_limit = \"256\"] #![type_length_limit = \"512\"] #[recursion_limit = \"42\"] fn invalid_outer() {} mod inner { #![recursion_limit = \"12\"] fn child() {} }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.kind == SymbolKind::Module && symbol.parent.is_none()).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Compile-time recursion limit").collect();
    assert_eq!(items.len(), 1, "{:?}", root.rust_contract);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "#![recursion_limit = \"256\"]");
    assert!(items[0].text.contains("string \"256\""));
    assert!(items[0].text.contains("macro expansion"));
    assert!(items[0].text.contains("not how deeply the finished program may recurse at runtime"));
    assert!(items[0].text.contains("does not prove"));
    for name in ["invalid_outer", "inner", "child"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Compile-time recursion limit"));
    }
}
