use nodex_lang_rust::lower_source;

#[test]
fn custom_attribute_is_explained_on_its_declaration_without_assuming_resolution() {
    let source = "#[custom_attribute] fn annotated() {} #[custom_attribute] #[allow(dead_code)] struct Item; #[inline] fn ordinary() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["annotated", "Item"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Custom outer attribute").unwrap_or_else(|| panic!("{:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[custom_attribute]");
        assert!(item.text.contains("If this name resolves") && item.text.contains("procedural attribute macro"));
        assert!(item.text.contains("empty here") && item.text.contains("derive helper") && item.text.contains("invalid"));
    }
    let ordinary = ir.symbols.iter().find(|symbol| symbol.name == "ordinary").unwrap();
    assert!(!ordinary.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Custom outer attribute"));
}
