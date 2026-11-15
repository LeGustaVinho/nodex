use nodex_lang_rust::lower_source;

#[test]
fn doc_hidden_explains_documentation_not_visibility() {
    let source = "#[doc(hidden)] pub fn f() {} #[doc(hidden)] #[allow(dead_code)] pub struct Hidden; #[doc = \"hidden\"] pub fn documented() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "Hidden"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Hidden from generated documentation").unwrap_or_else(|| panic!("{:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[doc(hidden)]");
        assert!(item.text.contains("rustdoc") && item.text.contains("ordinary generated API documentation"));
        assert!(item.text.contains("not the item's Rust visibility") && item.text.contains("does not remove the item from compilation"));
    }
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "documented").unwrap();
    assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Hidden from generated documentation"));
}
