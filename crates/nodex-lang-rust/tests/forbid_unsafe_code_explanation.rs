use nodex_lang_rust::lower_source;

#[test]
fn forbid_unsafe_code_explains_scope_and_lint_cap_without_conflating_other_attributes() {
    let source = "#![forbid(unsafe_code)]\n#[forbid(unsafe_code, dead_code)]\n#[inline]\nfn guarded() {}\n#[allow(unsafe_code)] fn allowed() {}\n#[forbid(my_tool::unsafe_code)] fn custom() {}\n#[cfg_attr(feature = \"x\", forbid(unsafe_code))] fn indirect() {}\nmod inner { #![forbid(unsafe_code)] fn nested() {} }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.parent.is_none()).unwrap();
    let inner = ir.symbols.iter().find(|s| s.name == "inner").unwrap();
    let guarded = ir.symbols.iter().find(|s| s.name == "guarded").unwrap();
    for (symbol, prefix, scope) in [(root, "#![", "enclosing"), (inner, "#![", "enclosing"), (guarded, "#[", "following item")] {
        let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Forbid unsafe-code lint").collect();
        assert_eq!(facts.len(), 1, "{}: {:?}", symbol.name, symbol.rust_contract);
        let item = facts[0];
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with(prefix));
        assert!(item.text.contains(scope));
        assert!(item.text.contains("normally makes the diagnostic an error"));
        assert!(item.text.contains("cannot lower"));
        assert!(item.text.contains("--cap-lints warn"));
        assert!(item.text.contains("--force-warn"));
        assert!(item.text.contains("does not establish the active compiler flags"));
    }
    for name in ["allowed", "custom", "indirect", "nested"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Forbid unsafe-code lint"), "{name}: {:?}", symbol.rust_contract);
    }
}
