use nodex_lang_rust::lower_source;

#[test]
fn derive_debug_is_explained_conditionally_for_type_symbols() {
    let source = "#[derive(Debug)] struct S { x: i32 } #[derive(Clone, Debug)] enum E { A } #[derive(custom::Debug)] struct C; #[derive(Debug)] union U { x: u32 } #[derive(Debugger)] struct Other; #[cfg_attr(feature = \"Debug\", derive(Clone))] struct Indirect;";
    let ir = lower_source("src/lib.rs", source);
    for name in ["S", "E", "C", "U"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Derive Debug").collect();
        assert_eq!(facts.len(), 1, "{name}: {:?}", symbol.rust_contract);
        let item = facts[0];
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "Debug");
        assert!(item.text.contains("If Debug resolves"));
        assert!(item.text.contains("Debug formatting trait"));
        assert!(item.text.contains("does not itself print"));
        assert!(item.text.contains("written spelling alone does not prove"));
        if name == "U" { assert!(item.text.contains("cannot be derived for a union")); }
    }
    for name in ["Other", "Indirect"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Derive Debug"));
    }
}
