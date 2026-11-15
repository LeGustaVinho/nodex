use nodex_lang_rust::lower_source;

#[test]
fn explains_simple_derive_clone_conditionally_without_guessing_other_macros() {
    let source = "#[derive(Clone)] struct S { a: u32 } #[derive(Debug, Clone)] enum E { A } #[derive(crate::Clone)] struct Other; #[derive(Copy)] struct Plain;";
    let ir = lower_source("src/lib.rs", source);
    for name in ["S", "E"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Derive Clone").unwrap();
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("#[derive("));
        assert!(item.text.contains("If `Clone` resolves") && item.text.contains("clones its fields") && item.text.contains("not an automatic copy") && item.text.contains("may share data"));
    }
    for name in ["Other", "Plain"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Derive Clone"));
    }
}
