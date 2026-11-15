use nodex_lang_rust::lower_source;
#[test]
fn direct_allow_dead_code_explains_lint_scope_without_matching_other_attributes() {
    let source = "#[allow(dead_code)] // note\n#[inline] fn unused() {} #[allow(dead_code, unused_variables)] struct S; fn ordinary() {} #[warn(dead_code)] fn warn() {} #[allow(unused_variables)] fn other() {} #[cfg_attr(test, allow(dead_code))] fn conditional() {} #[allow(clippy::dead_code)] fn qualified() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["unused", "S"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Allow dead-code lint").collect();
        assert_eq!(items.len(), 1, "{name}: {:?}", symbol.rust_contract);
        assert!(source[items[0].span.start_byte as usize..items[0].span.end_byte as usize].starts_with("#[allow(dead_code"));
        assert!(items[0].text.contains("nested scope") && items[0].text.contains("unused, unexported") && items[0].text.contains("does not remove") && items[0].text.contains("forbid"));
    }
    for name in ["ordinary", "warn", "other", "conditional", "qualified"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Allow dead-code lint"), "{name}");
    }
}
