use nodex_lang_rust::lower_source;

#[test]
fn any_cfg_explains_disjunction_and_empty_case_without_assuming_configuration() {
    let source = "#[cfg(any(a, b))] fn either() {} #[cfg(any())] fn never() {} #[cfg(any(all(a,b), feature = \"x\"))] struct Nested; #[cfg(all(a,b))] fn both() {} #[cfg_attr(a, cfg(any(b,c)))] fn conditional() {}";
    let file = lower_source("src/lib.rs", source);
    for name in ["either", "never", "Nested"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Any configuration predicate").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert!((&source[item.span.start_byte as usize..item.span.end_byte as usize]).starts_with("#[cfg(any("));
        assert!(item.text.contains("compile-time configuration"));
        assert!(item.text.contains("does not establish"));
        assert!(item.text.contains("Other attributes and conditions"));
        if name == "never" { assert!(item.text.contains("false for every compilation configuration")); }
        else { assert!(item.text.contains("at least one") && item.text.contains("none are true")); }
    }
    for name in ["both", "conditional"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Any configuration predicate"));
    }
}
