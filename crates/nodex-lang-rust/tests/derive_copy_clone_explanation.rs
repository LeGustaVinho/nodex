use nodex_lang_rust::lower_source;

#[test]
fn explains_standard_copy_clone_derives_conditionally_and_checks_requirements() {
    let source = "#[derive(Copy, Clone)] struct S { x: i32 } #[derive(Clone, Copy)] enum E { A } #[derive(Clone)] struct C;";
    let file = lower_source("src/lib.rs", source);
    for name in ["S", "E"] {
        let symbol = file.symbols.iter().find(|s| s.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Derived Copy and Clone").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert!((&source[item.span.start_byte as usize..item.span.end_byte as usize]).starts_with("#[derive("));
        assert!(item.text.contains("If Copy and Clone resolve"));
        assert!(item.text.contains("instead of moving"));
        assert!(item.text.contains("all fields to be Copy"));
        assert!(item.text.contains("not to implement Drop"));
        assert!(item.text.contains("does not duplicate its target"));
    }
    let only_clone = file.symbols.iter().find(|s| s.name == "C").unwrap();
    assert!(!only_clone.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Derived Copy and Clone"));
}
