use nodex_lang_rust::lower_source;

#[test]
fn transparent_representation_explains_layout_restrictions_without_assuming_validity() {
    let source = "#[repr(transparent)] pub struct Wrapper(u32); #[repr(transparent)] #[allow(dead_code)] enum Single { Value(u32) } #[repr(C)] struct CLayout(u32);";
    let ir = lower_source("src/lib.rs", source);
    for name in ["Wrapper", "Single"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Transparent representation").unwrap_or_else(|| panic!("{:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[repr(transparent)]");
        assert!(item.text.contains("at most one field") && item.text.contains("alignment one") && item.text.contains("or unit `()`"));
        assert!(item.text.contains("does not prove") && item.text.contains("does not make the wrapper the same Rust type"));
    }
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "CLayout").unwrap();
    assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Transparent representation"));
}
