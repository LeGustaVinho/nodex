use nodex_lang_rust::lower_source;

#[test]
fn cfg_attr_conditionally_applies_debug_derive_without_assuming_configuration() {
    let source = "#[cfg_attr(any(feature = \"a\", test), derive(Debug, Clone))] struct S; #[cfg_attr(feature = \"x\", derive(Debug))] struct T; #[cfg_attr(test, derive(Clone))] struct U; #[cfg_attr(test, helper(derive(Debug)))] struct V; struct Plain;";
    let ir = lower_source("src/lib.rs", source);
    for (name, expected) in [("S", "any(feature = \"a\", test)"), ("T", "feature = \"x\"")] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Conditional derive attribute").collect();
        assert_eq!(items.len(), 1, "{}: {:?}", name, symbol.rust_contract);
        assert!(items[0].text.contains(expected) && items[0].text.contains("If it is true") && items[0].text.contains("if false") && items[0].text.contains("does not establish"));
        assert!(source[items[0].span.start_byte as usize..items[0].span.end_byte as usize].starts_with("#[cfg_attr("));
    }
    for name in ["U", "V", "Plain"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Conditional derive attribute"));
    }
}
