use nodex_lang_rust::lower_source;

#[test]
fn all_cfg_explains_conjunction_empty_case_and_excludes_any() {
    let source = "#[cfg(all(a, b))] fn both() {} #[cfg(all())] fn always() {} #[cfg(all(any(a,b), feature = \"x\"))] fn nested() {} #[cfg(any(a, b))] fn either() {}";
    let file = lower_source("src/lib.rs", source);
    for name in ["both", "always", "nested"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "All configuration predicates").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert!((&source[item.span.start_byte as usize..item.span.end_byte as usize]).starts_with("#[cfg(all("));
        assert!(item.text.contains("compile-time configuration"));
        assert!(item.text.contains("syntax alone does not establish"));
        if name == "always" { assert!(item.text.contains("true for every configuration")); }
        else { assert!(item.text.contains("every predicate") && item.text.contains("any predicate is false")); }
    }
    let either = file.symbols.iter().find(|symbol| symbol.name == "either").unwrap();
    assert!(!either.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "All configuration predicates"));
}
